use std::{io::BufRead, path::Path};

use kate_bot::{dictionary::NLevel, open_reader};

pub struct JlptEntry {
    pub hiragana: String,
    pub kanji: Option<String>,
    pub level: NLevel,
}

/// Gets JLPT entries at `level`.
pub fn pool(dir: &Path, level: NLevel) -> Vec<JlptEntry> {
    let mut entries = Vec::new();

    let path = dir.join(format!("jlpt-voc-{}.utf.txt", i32::from(level)));
    let reader = open_reader(&path);

    for line in reader.lines() {
        let line = line.unwrap_or_else(|e| panic!("Invalid byte read in jfile:\n{e}"));

        let Some((hiragana, kanji)) = extract_entry(&line) else {
            continue;
        };

        entries.push(JlptEntry {
            hiragana,
            kanji,
            level,
        });
    }

    entries
}

fn extract_entry(line: &str) -> Option<(String, Option<String>)> {
    if line.starts_with("#") || line.is_empty() || line.contains("~") {
        return None;
    }

    // Remove parenthesized note
    let trimmed = line.split_once("（").map_or(line, |(left, _)| left);
    let fields: Vec<&str> = trimmed.split_whitespace().collect();

    match fields.len() {
        // Kanji isn't present, hiragana is first in line
        1 => Some((fields[0].to_string(), None)),
        // Kanji is present, hiragana is second in line
        2 => Some((fields[1].to_string(), Some(fields[0].to_string()))),
        _ => panic!("Error extracting hiragana:\n\t{line}"),
    }
}

// cat
