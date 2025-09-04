use std::sync::Arc;

use kate_bot::dictionary::{DictEntry, Pos};
use serde::{Deserialize, Deserializer};
use tracing::{info, instrument};

use crate::{models::dictionary::Dictionary, util::IndexMap};

#[derive(Debug, Clone)]
pub struct TVerbPooledPair {
    pair: TVerbPair,
    intrans_nouns: Vec<String>,
    trans_nouns: Vec<String>,
    trans_subjects: Vec<String>,
}

impl TVerbPooledPair {
    pub fn intrans_kanji(&self) -> &str {
        self.pair.intrans_kanji()
    }

    pub fn trans_kanji(&self) -> &str {
        self.pair.trans_kanji()
    }

    pub fn intrans_entry(&self) -> &Arc<DictEntry> {
        self.pair.intrans_entry()
    }

    pub fn trans_entry(&self) -> &Arc<DictEntry> {
        self.pair.trans_entry()
    }

    pub fn intrans_nouns(&self) -> &[String] {
        &self.intrans_nouns
    }

    pub fn trans_nouns(&self) -> &[String] {
        &self.trans_nouns
    }

    pub fn trans_subjects(&self) -> &[String] {
        &self.trans_subjects
    }
}

/// Contains an intransitive verb and its transitive counterpart.
#[derive(Debug, Clone)]
pub struct TVerbPair {
    /// The intransitive verb in kanji and its entry.
    intrans: (String, Arc<DictEntry>),
    /// The transitive verb in kanji and its entry.
    trans: (String, Arc<DictEntry>),
}

impl TVerbPair {
    pub fn intrans_kanji(&self) -> &str {
        &self.intrans.0
    }

    pub fn trans_kanji(&self) -> &str {
        &self.trans.0
    }

    pub fn intrans_entry(&self) -> &Arc<DictEntry> {
        &self.intrans.1
    }

    pub fn trans_entry(&self) -> &Arc<DictEntry> {
        &self.trans.1
    }
}

/// Represents an intransitive suffix that may be commonly interchanged
/// with a transitive suffix
#[derive(Debug, Clone, Deserialize)]
struct TSuffix {
    intrans: String,
    trans: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntransPool {
    pub intrans_nouns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransPool {
    pub trans_nouns: Vec<String>,
    pub trans_subjects: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ContextPoolRecord {
    intrans: String,
    trans: String,
    #[serde(deserialize_with = "string_to_vec")]
    intrans_nouns: Vec<String>,
    #[serde(deserialize_with = "string_to_vec")]
    trans_nouns: Vec<String>,
    #[serde(deserialize_with = "string_to_vec")]
    trans_subjects: Vec<String>,
}

/// Collects transitivity verb pairs from `dictionary` and annotates them
/// with nouns and subjects usable in their context.
#[instrument(level = "info", skip(dictionary))]
pub fn tverb_pooled_pairs(dictionary: &Dictionary) -> Vec<TVerbPooledPair> {
    let pairs = tverb_pairs(dictionary);
    let pools = context_pools();

    let mut res = pairs
        .into_iter()
        .filter_map(|pair| {
            let key = (
                pair.intrans_kanji().to_string(),
                pair.trans_kanji().to_string(),
            );
            let (
                IntransPool { intrans_nouns },
                TransPool {
                    trans_nouns,
                    trans_subjects,
                },
                // We must use .get() instead of .remove()
                // because a pair like (回る, 回す) exists
                // twice with the intransitive ref'ing
                // different dict entries.
                // i.e., Both will use same pools.
            ) = pools.get(&key).cloned().or_else(|| {
                info!(?key, "Pair with no pool");
                None
            })?;

            Some(TVerbPooledPair {
                pair,
                intrans_nouns,
                trans_nouns,
                trans_subjects,
            })
        })
        .collect::<Vec<TVerbPooledPair>>();

    apply_manual_changes(&mut res);

    res
}

struct ManualChange {
    intrans_id: u32,
    trans_id: u32,
    apply: fn(&mut TVerbPooledPair),
}

const MANUAL_CHANGES: [ManualChange; 1] = [ManualChange {
    intrans_id: 1593430,
    trans_id: 1170650,
    apply: |p| p.pair.intrans.0 = "籠る".to_string(),
}];

fn apply_manual_changes(pairs: &mut [TVerbPooledPair]) {
    for pair in pairs {
        if let Some(ManualChange { apply, .. }) = MANUAL_CHANGES.iter().find(|change| {
            change.intrans_id == pair.intrans_entry().id && change.trans_id == pair.trans_entry().id
        }) {
            apply(pair);
        }
    }
}

/// Collects transitivity verb pairs from `dictionary`.
pub fn tverb_pairs(dictionary: &Dictionary) -> Vec<TVerbPair> {
    let (intrans, trans) = tverbs(dictionary);
    let suffixes = tsuffixes();

    // Make pairs based off intransitive per consultant's directions.
    intrans
        .iter()
        .filter_map(|intrans_ent| {
            let mut pairs: Vec<TVerbPair> = intrans_possible_pairs(intrans_ent, &suffixes)
                .into_iter()
                .flat_map(|(intrans_wrd, trans_wrd)| {
                    trans.iter().filter_map(move |trans_ent| {
                        if trans_ent
                            .kanjis
                            .iter()
                            .any(|kanji| kanji.text == *trans_wrd)
                        {
                            Some(TVerbPair {
                                intrans: (intrans_wrd.clone(), intrans_ent.clone()),
                                trans: (trans_wrd.clone(), trans_ent.clone()),
                            })
                        } else {
                            None
                        }
                    })
                })
                .collect();

            pairs.sort_by_key(|p| p.trans.1.id);
            pairs.dedup_by_key(|p| p.trans.1.id);

            // Ignore ambigious matches
            if pairs.len() > 1 {
                return None;
            }

            pairs.pop()
        })
        .collect()
}

#[instrument(level = "info")]
fn context_pools() -> IndexMap<(String, String), (IntransPool, TransPool)> {
    static POOLS_FILE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/content/transitivity_context_pools.csv"
    ));

    let mut map = IndexMap::new();

    let mut rdr = csv::Reader::from_reader(POOLS_FILE);
    for record in rdr.deserialize::<ContextPoolRecord>() {
        let ContextPoolRecord {
            intrans,
            trans,
            intrans_nouns,
            trans_nouns,
            trans_subjects,
        } = record.unwrap();

        let key = (intrans, trans);
        let intrans_pool = IntransPool { intrans_nouns };
        let trans_pool = TransPool {
            trans_nouns,
            trans_subjects,
        };

        if map.contains_key(&key) {
            info!(?key, "Duplicate key, pools overwritten");
        }
        map.insert((key, (intrans_pool, trans_pool)));
    }

    map
}

/// Get all entries tagged with [`Pos::Vi`] and not tagged with [`Pos::Vt`]
/// and all entries tagged with [`Pos::Vt`] and not tagged with [`Pos::Vi`].
fn tverbs(dictionary: &Dictionary) -> (Vec<Arc<DictEntry>>, Vec<Arc<DictEntry>>) {
    let mut intrans = Vec::new();
    let mut trans = Vec::new();

    for entry in &dictionary.entries {
        let contains_intrans = entry.senses.iter().any(|s| s.pos.contains(&Pos::Vi));
        let contains_trans = entry.senses.iter().any(|s| s.pos.contains(&Pos::Vt));

        if contains_intrans && !contains_trans {
            intrans.push(entry.clone());
        } else if contains_trans && !contains_intrans {
            trans.push(entry.clone());
        }
    }

    (intrans, trans)
}

// Get all possible transitivity suffix pairings.
fn tsuffixes() -> Vec<TSuffix> {
    static SUFFIX_FILE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/content/transitivity_suffixes.csv"
    ));
    let mut rdr = csv::Reader::from_reader(SUFFIX_FILE);
    rdr.deserialize().map(|record| record.unwrap()).collect()
}

/// Create (intransitive, possible transitive) pairings. Each intransitive
/// verb in kanji in the given entry has its intransitive suffix
/// replaced by a possible transitive suffix.
fn intrans_possible_pairs(intrans: &DictEntry, suffixes: &[TSuffix]) -> Vec<(String, String)> {
    let intrans_words: Vec<String> = intrans.kanjis.iter().map(|k| k.text.clone()).collect();

    intrans_words
        .iter()
        .flat_map(|intrans| {
            suffixes.iter().filter_map(
                move |TSuffix {
                          intrans: intrans_suf,
                          trans: trans_suf,
                      }| {
                    let root = intrans.strip_suffix(intrans_suf)?.to_string();
                    let possible_trans = root + trans_suf;
                    Some((intrans.clone(), possible_trans))
                },
            )
        })
        .collect()
}

fn string_to_vec<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    Ok(s.split(",").map(|s| s.trim().to_string()).collect())
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use tracing::Level;

    use super::*;
    use crate::models::dictionary::Dictionary;

    static INIT: LazyLock<()> = LazyLock::new(|| {
        tracing_subscriber::fmt()
            .with_max_level(Level::INFO)
            .with_test_writer()
            .init();
    });

    #[test]
    fn test_verb_pairs() {
        LazyLock::force(&INIT);

        let dictionary = Dictionary::new();
        let pairs = tverb_pairs(&dictionary);

        pairs.iter().enumerate().for_each(|(idx, p)| {
            // println!("{},{}", p.intrans.0, p.trans.0);
            println!("({idx}) Intrans: {}, Trans: {}", p.intrans.0, p.trans.0);
            println!(
                "[{},{}]\n",
                serde_json::to_string(p.intrans.1.as_ref()).unwrap(),
                serde_json::to_string(p.trans.1.as_ref()).unwrap()
            )
        });
    }

    #[test]
    fn test_context_pools() {
        LazyLock::force(&INIT);

        let dict = Dictionary::new();
        tverb_pooled_pairs(&dict);
    }
}
