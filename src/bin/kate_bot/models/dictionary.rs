use std::{
    io::{BufRead, Cursor},
    sync::Arc,
};

use jplearnbot::dictionary::{DictEntry, NLevel, Pos};
use strum_macros::{EnumIter, EnumString};

/// Contains [`DictEntry`]'s.
#[derive(Debug)]
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

    pub fn subset(&self, levels: &[NLevel], pos: &[Pos]) -> Vec<Arc<DictEntry>> {
        self.entries
            .iter()
            .filter(|entry| {
                entry.levels().iter().any(|level| levels.contains(level))
                    && entry
                        .senses
                        .iter()
                        .any(|s| s.pos.iter().any(|p| pos.contains(p)))
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, EnumString, EnumIter, strum_macros::Display)]
pub enum PosFilter {
    #[strum(to_string = "Nouns 名詞")]
    Nouns,
    #[strum(to_string = "Verbs 動詞")]
    Verbs,
    #[strum(to_string = "Prenominals 敬称略")]
    Prenominals,
    #[strum(to_string = "Expression 表現")]
    Expressions,
    #[strum(to_string = "Conjunctions 接続詞")]
    Conjunctions,
    Other,
}

impl PosFilter {
    pub const fn as_pos(&self) -> &'static [Pos] {
        const NOUNS: [Pos; 7] = [
            Pos::N,
            Pos::NPr,
            Pos::NAdv,
            Pos::NPref,
            Pos::NSuf,
            Pos::NT,
            Pos::Pn,
        ];

        const VERBS: [Pos; 59] = [
            Pos::VUnspec,
            Pos::V1,
            Pos::V1S,
            Pos::V2aS,
            Pos::V2bK,
            Pos::V2bS,
            Pos::V2dK,
            Pos::V2dS,
            Pos::V2gk,
            Pos::V2gS,
            Pos::V2hK,
            Pos::V2hS,
            Pos::V2kK,
            Pos::V2kS,
            Pos::V2mK,
            Pos::V2mS,
            Pos::V2nS,
            Pos::V2rK,
            Pos::V2rS,
            Pos::V2sS,
            Pos::V2tK,
            Pos::V2tS,
            Pos::V2wS,
            Pos::V2yK,
            Pos::V2yS,
            Pos::V2zS,
            Pos::V4b,
            Pos::V4g,
            Pos::V4h,
            Pos::V4k,
            Pos::V4m,
            Pos::V4n,
            Pos::V4r,
            Pos::V4s,
            Pos::V4t,
            Pos::V5aru,
            Pos::V5b,
            Pos::V5g,
            Pos::V5k,
            Pos::V5kS,
            Pos::V5m,
            Pos::V5n,
            Pos::V5r,
            Pos::V5rI,
            Pos::V5s,
            Pos::V5t,
            Pos::V5u,
            Pos::V5uS,
            Pos::V5uru,
            Pos::Vi,
            Pos::Vk,
            Pos::Vn,
            Pos::Vr,
            Pos::Vs,
            Pos::VsC,
            Pos::VsI,
            Pos::VsS,
            Pos::Vt,
            Pos::Vz,
        ];

        const PRENOMINALS: [Pos; 3] = [Pos::AdjF, Pos::AdjPn, Pos::AdjNo];

        const EXPRESSIONS: [Pos; 2] = [Pos::Exp, Pos::Int];

        const CONJUNCTIONS: [Pos; 1] = [Pos::Conj];

        const OTHER: [Pos; 20] = [
            Pos::AdjI,
            Pos::AdjIx,
            Pos::AdjKari,
            Pos::AdjKu,
            Pos::AdjNa,
            Pos::AdjNari,
            Pos::AdjShiku,
            Pos::AdjT,
            Pos::Adv,
            Pos::AdvTo,
            Pos::Aux,
            Pos::AuxAdj,
            Pos::AuxV,
            Pos::Cop,
            Pos::Ctr,
            Pos::Num,
            Pos::Pref,
            Pos::Prt,
            Pos::Suf,
            Pos::Unc,
        ];

        match self {
            PosFilter::Nouns => &NOUNS,
            PosFilter::Verbs => &VERBS,
            PosFilter::Prenominals => &PRENOMINALS,
            PosFilter::Expressions => &EXPRESSIONS,
            PosFilter::Conjunctions => &CONJUNCTIONS,
            PosFilter::Other => &OTHER,
        }
    }
}
