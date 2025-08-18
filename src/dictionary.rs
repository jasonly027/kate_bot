use serde::{self, Deserialize, Serialize};
use strum_macros::{EnumIter, EnumString};

/// An entry in the JMDict dictionary
///
/// # See also
/// <https://en.wikipedia.org/wiki/JMdict>
#[derive(Debug, Deserialize, Serialize)]
pub struct DictEntry {
    #[serde(alias = "ent_seq")]
    pub id: u32,

    #[serde(rename = "k_ele", default, skip_serializing_if = "Vec::is_empty")]
    pub kanjis: Vec<Kanji>,

    #[serde(rename = "r_ele")]
    pub readings: Vec<Reading>,

    #[serde(rename = "sense")]
    pub senses: Vec<Sense>,
}

impl PartialEq for DictEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for DictEntry {}

impl DictEntry {
    /// Determines whether there is any [reading](`DictEntry::readings`)
    /// or [kanji](`DictEntry::kanjis`) annotated with at least one [`NLevel`]
    pub fn is_annotated(&self) -> bool {
        self.kanjis.iter().any(|kanji| !kanji.levels.is_empty())
            || self
                .readings
                .iter()
                .any(|reading| !reading.levels.is_empty())
    }

    /// Retrieves a copy of all the levels this entry is tagged with.
    pub fn levels(&self) -> Vec<NLevel> {
        let mut levels: Vec<_> = self
            .readings
            .iter()
            .flat_map(|r| r.levels.iter().copied())
            .collect();
        levels.sort_unstable();
        levels.dedup();

        levels
    }

    /// Annotates a [reading](`DictEntry::readings`) that matches `hiragana`
    /// with `level`. Annotates all [kanjis](`DictEntry::kanjis`) with the
    /// same `level` or only the ones in [relevant_to](`Reading::relevant_to`) if that
    /// list isn't empty.
    pub fn add_level(&mut self, hiragana: &str, level: NLevel) {
        let Some(reading) = self.readings.iter_mut().find(|h| h.text == hiragana) else {
            return;
        };

        if reading.levels.contains(&level) {
            return;
        }

        reading.levels.push(level);

        // Set all Kanjis to the same JLPT level.
        // If this hiragana has a specific relevant_to list,
        // set only those kanjis instead.
        for kanji in self.kanjis.iter_mut().filter(|kanji| {
            if reading.relevant_to.is_empty() {
                !kanji.levels.contains(&level)
            } else {
                !kanji.levels.contains(&level) && reading.relevant_to.contains(&kanji.text)
            }
        }) {
            kanji.levels.push(level);
        }
    }

    /// Removes any [kanjis](`DictEntry::kanjis`) and/or [readings](`DictEntry::readings`)
    /// that aren't annotated with at least one [`NLevel`].
    pub fn trim(&mut self) {
        self.readings.retain(|r| !r.levels.is_empty());
        self.kanjis.retain(|k| !k.levels.is_empty());
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Kanji {
    #[serde(rename = "keb")]
    pub text: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<NLevel>,

    #[serde(rename = "ke_inf", default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<KTag>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Reading {
    #[serde(rename = "reb")]
    pub text: String,

    #[serde(rename = "re_restr", default, skip_serializing_if = "Vec::is_empty")]
    pub relevant_to: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<NLevel>,

    #[serde(rename = "re_inf", default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<RTag>,
}

#[derive(
    Debug,
    Deserialize,
    Serialize,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Clone,
    Copy,
    strum_macros::Display,
    EnumIter,
    EnumString,
)]
pub enum NLevel {
    N1,
    N2,
    N3,
    N4,
}

impl From<NLevel> for i32 {
    fn from(value: NLevel) -> Self {
        match value {
            NLevel::N1 => 1,
            NLevel::N2 => 2,
            NLevel::N3 => 3,
            NLevel::N4 => 4,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum KTag {
    #[serde(rename = "&ateji;")]
    Ateji,
    #[serde(rename = "&ik;")]
    IrKana,
    #[serde(rename = "&iK;")]
    IrKanji,
    #[serde(rename = "&io;")]
    IrOkurigana,
    #[serde(rename = "&oK;")]
    Outdated,
    #[serde(rename = "&rK;")]
    Rare,
    #[serde(rename = "&sK;")]
    SearchOnly,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum RTag {
    #[serde(rename = "&gikun;")]
    Gikun,
    #[serde(rename = "&ik;")]
    IrKana,
    #[serde(rename = "&ok;")]
    Outdated,
    #[serde(rename = "&sk;")]
    SearchOnly,
    #[serde(rename = "&rk;")]
    Archaic,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Sense {
    #[serde(rename = "stagk", default, skip_serializing_if = "Vec::is_empty")]
    pub relevant_kanji: Vec<String>,

    #[serde(rename = "stagr", default, skip_serializing_if = "Vec::is_empty")]
    pub relevant_reading: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pos: Vec<Pos>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gloss: Vec<Gloss>,
}

macro_rules! pos_enum {
    (
        $(
            ($name:ident, $tag:literal, $desc:literal)
        ),* $(,)?
    ) => {
        #[derive(
            Debug,
            serde::Deserialize,
            serde::Serialize,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Clone,
            Copy,
            strum_macros::EnumMessage,
        )]
        pub enum Pos {
            $(
                #[serde(rename = $tag)]
                #[strum(message = $desc)]
                $name,
            )*
        }
    };
}

#[rustfmt::skip]
pos_enum!(
    (AdjF, "&adj-f;", "noun or verb acting prenominally"),
    (AdjI, "&adj-i;", "adjective (keiyoushi)"),
    (AdjIx, "&adj-ix;", "adjective (keiyoushi) - yoi/ii class"),
    (AdjKari, "&adj-kari;", "'kari' adjective (archaic)"),
    (AdjKu, "&adj-ku;", "'ku' adjective (archaic)"),
    (AdjNa, "&adj-na;", "adjectival nouns or quasi-adjectives (keiyodoshi)"),
    (AdjNari, "&adj-nari;", "archaic/formal form of na-adjective"),
    (AdjNo, "&adj-no;", "nouns which may take the genitive case particle 'no'"),
    (AdjPn, "&adj-pn;", "pre-noun adjectival (rentaishi)"),
    (AdjShiku, "&adj-shiku;", "'shiku' adjective (archaic)"),
    (AdjT, "&adj-t;", "'taru' adjective"),
    (Adv, "&adv;", "adverb (fukushi)"),
    (AdvTo, "&adv-to;", "adverb taking the 'to' particle"),
    (Aux, "&aux;", "auxiliary"),
    (AuxAdj, "&aux-adj;", "auxiliary adjective"),
    (AuxV, "&aux-v;", "auxiliary verb"),
    (Conj, "&conj;", "conjunction"),
    (Cop, "&cop;", "copula"),
    (Ctr, "&ctr;", "counter"),
    (Exp, "&exp;", "expressions (phrases, clauses, etc.)"),
    (Int, "&int;", "interjection (kandoushi)"),
    (N, "&n;", "noun (common) (futsuumeishi)"),
    (NAdv, "&n-adv;", "adverbial noun (fukushitekimeishi)"),
    (NPr, "&n-pr;", "proper noun"),
    (NPref, "&n-pref;", "noun, used as a prefix"),
    (NSuf, "&n-suf;", "noun, used as a suffix"),
    (NT, "&n-t;", "noun (temporal) (jisoumeishi)"),
    (Num, "&num;", "numeric"),
    (Pn, "&pn;", "pronoun"),
    (Pref, "&pref;", "prefix"),
    (Prt, "&prt;", "particle"),
    (Suf, "&suf;", "suffix"),
    (Unc, "&unc;", "unclassified"),
    (VUnspec, "&v-unspec;", "verb unspecified"),
    (V1, "&v1;", "Ichidan verb"),
    (V1S, "&v1-s;", "Ichidan verb - kureru special class"),
    (V2aS, "&v2a-s;", "Nidan verb with 'u' ending (archaic)"),
    (V2bK, "&v2b-k;", "Nidan verb (upper class) with 'bu' ending (archaic)"),
    (V2bS, "&v2b-s;", "Nidan verb (lower class) with 'bu' ending (archaic)"),
    (V2dK, "&v2d-k;", "Nidan verb (upper class) with 'dzu' ending (archaic)"),
    (V2dS, "&v2d-s;", "Nidan verb (lower class) with 'dzu' ending (archaic)"),
    (V2gk, "&v2g-k;", "Nidan verb (upper class) with 'gu' ending (archaic)"),
    (V2gS, "&v2g-s;", "Nidan verb (lower class) with 'gu' ending (archaic)"),
    (V2hK, "&v2h-k;", "Nidan verb (upper class) with 'hu/fu' ending (archaic)"),
    (V2hS, "&v2h-s;", "Nidan verb (lower class) with 'hu/fu' ending (archaic)"),
    (V2kK, "&v2k-k;", "Nidan verb (upper class) with 'ku' ending (archaic)"),
    (V2kS, "&v2k-s;", "Nidan verb (lower class) with 'ku' ending (archaic)"),
    (V2mK, "&v2m-k;", "Nidan verb (upper class) with 'mu' ending (archaic)"),
    (V2mS, "&v2m-s;", "Nidan verb (lower class) with 'mu' ending (archaic)"),
    (V2nS, "&v2n-s;", "Nidan verb (lower class) with 'nu' ending (archaic)"),
    (V2rK, "&v2r-k;", "Nidan verb (upper class) with 'ru' ending (archaic)"),
    (V2rS, "&v2r-s;", "Nidan verb (lower class) with 'ru' ending (archaic)"),
    (V2sS, "&v2s-s;", "Nidan verb (lower class) with 'su' ending (archaic)"),
    (V2tK, "&v2t-k;", "Nidan verb (upper class) with 'tsu' ending (archaic)"),
    (V2tS, "&v2t-s;", "Nidan verb (lower class) with 'tsu' ending (archaic)"),
    (V2wS, "&v2w-s;", "Nidan verb (lower class) with 'u' ending and 'we' conjugation (archaic)"),
    (V2yK, "&v2y-k;", "Nidan verb (upper class) with 'yu' ending (archaic)"),
    (V2yS, "&v2y-s;", "Nidan verb (lower class) with 'yu' ending (archaic)"),
    (V2zS, "&v2z-s;", "Nidan verb (lower class) with 'zu' ending (archaic)"),
    (V4b, "&v4b;", "Yodan verb with 'bu' ending (archaic)"),
    (V4g, "&v4g;", "Yodan verb with 'gu' ending (archaic)"),
    (V4h, "&v4h;", "Yodan verb with 'hu/fu' ending (archaic)"),
    (V4k, "&v4k;", "Yodan verb with 'ku' ending (archaic)"),
    (V4m, "&v4m;", "Yodan verb with 'mu' ending (archaic)"),
    (V4n, "&v4n;", "Yodan verb with 'nu' ending (archaic)"),
    (V4r, "&v4r;", "Yodan verb with 'ru' ending (archaic)"),
    (V4s, "&v4s;", "Yodan verb with 'su' ending (archaic)"),
    (V4t, "&v4t;", "Yodan verb with 'tsu' ending (archaic)"),
    (V5aru, "&v5aru;", "Godan verb - -aru special class"),
    (V5b, "&v5b;", "Godan verb with 'bu' ending"),
    (V5g, "&v5g;", "Godan verb with 'gu' ending"),
    (V5k, "&v5k;", "Godan verb with 'ku' ending"),
    (V5kS, "&v5k-s;", "Godan verb - Iku/Yuku special class"),
    (V5m, "&v5m;", "Godan verb with 'mu' ending"),
    (V5n, "&v5n;", "Godan verb with 'nu' ending"),
    (V5r, "&v5r;", "Godan verb with 'ru' ending"),
    (V5rI, "&v5r-i;", "Godan verb with 'ru' ending (irregular verb)"),
    (V5s, "&v5s;", "Godan verb with 'su' ending"),
    (V5t, "&v5t;", "Godan verb with 'tsu' ending"),
    (V5u, "&v5u;", "Godan verb with 'u' ending"),
    (V5uS, "&v5u-s;", "Godan verb with 'u' ending (special class)"),
    (V5uru, "&v5uru;", "Godan verb - Uru old class verb (old form of Eru)"),
    (Vi, "&vi;", "intransitive verb"),
    (Vk, "&vk;", "Kuru verb - special class"),
    (Vn, "&vn;", "irregular nu verb"),
    (Vr, "&vr;", "irregular ru verb, plain form ends with -ri"),
    (Vs, "&vs;", "noun or participle which takes the aux. verb suru"),
    (VsC, "&vs-c;", "su verb - precursor to the modern suru"),
    (VsI, "&vs-i;", "suru verb - included"),
    (VsS, "&vs-s;", "suru verb - special class"),
    (Vt, "&vt;", "transitive verb"),
    (Vz, "&vz;", "Ichidan verb - zuru verb (alternative form of -jiru verbs)"),
);

#[derive(Debug, Deserialize, Serialize)]
pub struct Gloss {
    pub content: String,
}
