use std::{cell::RefCell, collections::HashMap, io::BufRead, path::Path, rc::Rc};

use kate_bot::{dictionary::DictEntry, open_reader};

/// Gets a dictionary where a key is hiragana and a value
/// is a list of [`DictEntry`]'s that contain that hiragana.
/// [NLevel](`kate_bot::dictionary::NLevel`) of kanjis and
/// readings aren't annotated.
pub fn dict(file: &Path) -> HashMap<String, Vec<Rc<RefCell<DictEntry>>>> {
    let entries: Vec<_> = entries(file)
        .into_iter()
        .map(RefCell::new)
        .map(Rc::new)
        .collect();

    let mut map: HashMap<String, Vec<_>> = HashMap::new();
    for entry in entries {
        for reading in &entry.borrow().readings {
            map.entry(reading.text.clone())
                .or_default()
                .push(entry.clone());
        }
    }

    map
}

/// Parses each line of a file into [`DictEntry`]'s
fn entries(file: &Path) -> Vec<DictEntry> {
    let reader = open_reader(file);

    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line.unwrap_or_else(|e| panic!("Invalid byte read in dfile:\n{e}"));

        let entry =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("JSON Parse error:\n{e}"));

        entries.push(entry);
    }

    entries
}
