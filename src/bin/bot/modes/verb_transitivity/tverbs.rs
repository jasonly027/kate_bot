use std::{
    io::{BufRead, Cursor},
    sync::Arc,
};

use kate_bot::dictionary::{DictEntry, Pos};

use crate::models::dictionary::Dictionary;

#[derive(Debug)]
pub struct TVerbPair {
    pub intrans: (String, Arc<DictEntry>),
    pub trans: (String, Arc<DictEntry>),
}

#[derive(Debug, Clone)]
struct TSuffix {
    intrans: String,
    trans: String,
}

/// Collects transitivity verb pairs from `dictionary`.
pub fn tverb_pairs(dictionary: &Dictionary) -> Vec<TVerbPair> {
    let (intrans, trans) = tverbs(dictionary);
    let suffixes = tsuffixes();

    // Make pairs based off intransitive per consultant's directions.
    intrans
        .iter()
        .flat_map(|intrans_ent| {
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
                return Vec::new();
            }

            pairs
        })
        .collect()
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

    Cursor::new(SUFFIX_FILE)
        .lines()
        .skip(1)
        .map(|line| {
            let line = line.unwrap();
            let (intrans, trans) = line
                .split_once(",")
                .expect("expected format: intransitive,transitive");

            TSuffix {
                intrans: intrans.to_string(),
                trans: trans.to_string(),
            }
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::dictionary::Dictionary;

    #[test]
    fn test_verb_pairs() {
        let dictionary = Dictionary::new();
        let pairs = tverb_pairs(&dictionary);

        pairs.iter().for_each(|p| {
            println!("{},{}", p.intrans.0, p.trans.0);
            // println!("({idx}) Intrans: {}, Trans: {}", p.intrans.0, p.trans.0);
            // println!(
            //     "[{},{}]\n",
            //     serde_json::to_string(p.intrans.1.as_ref()).unwrap(),
            //     serde_json::to_string(p.trans.1.as_ref()).unwrap()
            // )
        });
    }
}
