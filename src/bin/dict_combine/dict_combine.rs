use std::{
    cell::RefCell,
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    rc::Rc,
};

use kate_bot::dictionary::{DictEntry, NLevel};

use crate::{
    dictionary,
    jlpt::{self, JlptEntry},
};

pub fn run(dir: &Path, overwrite: bool) {
    let entries = dict_entries(dir);
    let mut writer = writer(dir, overwrite);

    for entry in entries {
        let mut str = serde_json::to_string(&*entry.borrow()).unwrap();
        str.push('\n');

        writer
            .write_all(str.as_bytes())
            .unwrap_or_else(|e| panic!("Failed to write to output:\n{e}"));
    }

    writer.flush().expect("Failed to flush to output");
}

fn dict_entries(dir: &Path) -> Vec<Rc<RefCell<DictEntry>>> {
    let dict = annotated_dict(dir);

    let mut set: HashMap<u32, _> = HashMap::new();
    for entries in dict.into_values() {
        for entry in entries {
            if !entry.borrow().is_annotated() || BLACKLIST_IDS.contains(&entry.borrow().id) {
                continue;
            }

            entry.borrow_mut().trim();

            let id = entry.borrow().id;
            set.insert(id, entry);
        }
    }

    set.into_values().collect()
}

const BLACKLIST_IDS: [u32; 1] = [1577100];

/// Gets a dictionary where a key is hiragana and a value
/// is a list of [`DictEntry`]'s that contain that hiragana.
fn annotated_dict(dir: &Path) -> HashMap<String, Vec<Rc<RefCell<DictEntry>>>> {
    let dict = dictionary::dict(&dir.join("jmdict.jsonl"));

    for pool in [NLevel::N1, NLevel::N2, NLevel::N3, NLevel::N4]
        .into_iter()
        .map(|lvl| jlpt::pool(dir, lvl))
    {
        for JlptEntry {
            hiragana,
            kanji,
            level,
        } in &pool
        {
            let Some(matches) = dict.get(hiragana) else {
                continue;
            };

            // No definition ambiguity, mutate the exact match
            if matches.len() == 1 {
                matches[0].borrow_mut().add_level(hiragana, *level);
                continue;
            }

            // Entry has kanji, mutate the only match with the same kanji, if it exists
            if let Some(kanji) = kanji {
                let matches: Vec<_> = matches
                    .iter()
                    .filter(|m| m.borrow().kanjis.iter().any(|k| k.text == *kanji))
                    .collect();

                if matches.len() == 1 {
                    matches[0].borrow_mut().add_level(hiragana, *level);
                }

                continue;
            }

            // Entry has no kanji, mutate the only match without kanji too, if it exists
            let matches: Vec<_> = matches
                .iter()
                .filter(|m| m.borrow().kanjis.is_empty())
                .collect();

            if matches.len() == 1 {
                matches[0].borrow_mut().add_level(hiragana, *level);
            }
        }
    }

    dict
}

/// Open output file for writing
fn writer(dir: &Path, overwrite: bool) -> BufWriter<File> {
    let path = dir.join("dictionary.jsonl");

    let file = OpenOptions::new()
        .write(true)
        .create_new(!overwrite)
        .create(overwrite)
        .truncate(overwrite)
        .open(&path)
        .unwrap_or_else(|e| panic!("Error writing output to {}:\n{e}", path.display()));

    BufWriter::new(file)
}
