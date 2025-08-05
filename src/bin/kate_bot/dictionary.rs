use std::{
    io::{BufRead, Cursor},
    sync::Arc,
};

use jplearnbot::dictionary::{DictEntry, NLevel, Pos};
use rand::seq::SliceRandom;

/// Contains [`DictEntry`]'s.
pub struct Dictionary {
    /// Contains all of the entries.
    pub entries: Vec<Arc<DictEntry>>,
}

impl Default for Dictionary {
    fn default() -> Self {
        static DICT_FILE: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/content/dictionary.jsonl"
        ));

        let entries = Cursor::new(DICT_FILE)
            .lines()
            .map(|line| {
                let entry = serde_json::from_str::<DictEntry>(&line.unwrap()).unwrap();
                Arc::new(entry)
            })
            .collect();

        Dictionary { entries }
    }
}

impl Dictionary {
    pub fn new() -> Self {
        Dictionary::default()
    }

    /// Creates a randomized subset of the entries based on the parameter filters.
    pub async fn sample(&self, levels: &[NLevel], pos: &[Pos]) -> Vec<Arc<DictEntry>> {
        let mut sample: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| {
                entry.levels().iter().any(|lvl| levels.contains(lvl))
                    && entry
                        .senses
                        .iter()
                        .any(|s| s.pos.iter().any(|p| pos.contains(p)))
            })
            .cloned()
            .collect();
        sample.shuffle(&mut rand::rng());

        sample
    }
}
