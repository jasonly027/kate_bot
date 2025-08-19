use std::sync::Arc;

use kate_bot::dictionary::{DictEntry, Kanji, NLevel, Pos, Reading, Sense};
use poise::serenity_prelude::UserId;
use rand::{
    rng,
    seq::{IteratorRandom, SliceRandom},
};

use crate::{
    models::{
        dictionary::{Dictionary, PosFilter},
        question,
        scoreboard::Scoreboard,
    },
    modes::multi_choice::MultiChoiceMode,
};

type Question = question::Question<5>;

#[derive(Debug)]
pub struct GameSettings {
    pub mode: MultiChoiceMode,
    pub levels: Vec<NLevel>,
    pub filters: Vec<PosFilter>,
}

#[derive(Debug)]
pub struct Service {
    dictionary: Arc<Dictionary>,
    pos: Vec<Pos>,
    levels: Vec<NLevel>,
    sample: Vec<Arc<DictEntry>>,
    sample_idx: usize,
    mode: MultiChoiceMode,
    question: Option<Question>,
    scoreboard: Scoreboard,
}

impl Service {
    pub fn new(
        dictionary: Arc<Dictionary>,
        GameSettings {
            mode,
            levels,
            filters,
        }: GameSettings,
    ) -> Self {
        let pos: Vec<_> = filters
            .iter()
            .flat_map(|f| f.as_pos().iter().cloned())
            .collect();

        let mut sample = dictionary.subset(&levels, &pos);
        sample.shuffle(&mut rand::rng());

        Self {
            dictionary,
            pos,
            levels,
            sample,
            sample_idx: 0,
            mode,
            question: None,
            scoreboard: Scoreboard::new(),
        }
    }

    #[allow(dead_code)]
    pub fn levels(&self) -> &Vec<NLevel> {
        &self.levels
    }

    pub fn round(&self) -> u32 {
        self.scoreboard.rounds()
    }

    pub fn mode(&self) -> MultiChoiceMode {
        self.mode
    }

    pub fn question(&self) -> Option<&Question> {
        self.question.as_ref()
    }

    pub fn scoreboard(&self) -> &Scoreboard {
        &self.scoreboard
    }

    pub fn next_round(&mut self) {
        while let Some(entry) = self.next_entry() {
            self.pos.shuffle(&mut rng());

            let Some(question) = self
                .pos
                .iter()
                .find_map(|pos| make_question(self.mode, &entry, *pos, &self.dictionary))
            else {
                continue;
            };
            self.question = Some(question);
            self.scoreboard.next_round();

            return;
        }
        self.question = None;
    }

    pub fn select_choice(&mut self, user: UserId, choice: usize) -> bool {
        let Some(question) = self.question.as_mut() else {
            return false;
        };

        let correct = question.guess(choice);
        self.scoreboard.record(user, correct);
        correct
    }

    fn next_entry(&mut self) -> Option<Arc<DictEntry>> {
        if self.sample_idx < self.sample.len() {
            let entry = &self.sample[self.sample_idx];
            self.sample_idx += 1;
            return Some(entry.clone());
        }
        None
    }
}

fn make_question(
    mode: MultiChoiceMode,
    entry: &DictEntry,
    pos: Pos,
    dictionary: &Dictionary,
) -> Option<Question> {
    match mode {
        MultiChoiceMode::EngToHir => new_eng_to_hir(entry, pos, dictionary),
        MultiChoiceMode::HirToEng => new_hir_to_eng(entry, pos, dictionary),
        MultiChoiceMode::HirToKan => new_hir_to_kan(entry, pos, dictionary),
        MultiChoiceMode::KanToHir => new_kan_to_hir(entry, pos, dictionary),
        MultiChoiceMode::KanToEng => new_kan_to_eng(entry, pos, dictionary),
        MultiChoiceMode::EngToKan => new_eng_to_kan(entry, pos, dictionary),
    }
}

fn new_eng_to_hir(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Question> {
    let (reading, sense) = reading_sense_pair(entry, pos)?;

    let mut choices = std::array::from_fn(|_| "".to_string());
    choices[0] = reading.text.clone();

    dictionary
        .entries
        .iter()
        .filter_map(|e| {
            if e.id == entry.id {
                return None;
            }
            reading_sense_pair(e, pos).map(|(reading, _)| reading.text.clone())
        })
        .choose_multiple_fill(&mut rng(), &mut choices[1..]);

    choices.shuffle(&mut rng());

    let answer = choices.iter().position(|o| reading.text == *o).unwrap();

    Some(
        Question::new(
            sense.gloss[0].content.clone(),
            choices,
            answer,
            entry.levels(),
        )
        .unwrap(),
    )
}

fn new_hir_to_eng(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Question> {
    let (reading, sense) = reading_sense_pair(entry, pos)?;

    let mut choices = std::array::from_fn(|_| "".to_string());
    choices[0] = sense.gloss[0].content.clone();

    dictionary
        .entries
        .iter()
        .filter_map(|e| {
            if e.id == entry.id {
                return None;
            }
            reading_sense_pair(e, pos).map(|(_, sense)| sense.gloss[0].content.clone())
        })
        .choose_multiple_fill(&mut rng(), &mut choices[1..]);

    choices.shuffle(&mut rng());

    let answer = choices
        .iter()
        .position(|o| sense.gloss[0].content == *o)
        .unwrap();

    Some(Question::new(reading.text.clone(), choices, answer, entry.levels()).unwrap())
}

fn new_hir_to_kan(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Question> {
    let (kanji, reading) = kanji_reading_pair(entry, pos)?;

    let mut choices = std::array::from_fn(|_| "".to_string());
    choices[0] = kanji.text.clone();
    dictionary
        .entries
        .iter()
        .filter_map(|e| {
            if e.id == entry.id {
                return None;
            }
            kanji_reading_pair(e, pos).map(|(kanji, _)| kanji.text.clone())
        })
        .choose_multiple_fill(&mut rng(), &mut choices[1..]);

    choices.shuffle(&mut rng());

    let answer = choices.iter().position(|o| kanji.text == *o).unwrap();

    Some(Question::new(reading.text.clone(), choices, answer, entry.levels()).unwrap())
}

fn new_kan_to_hir(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Question> {
    let (kanji, reading) = kanji_reading_pair(entry, pos)?;

    let mut choices = std::array::from_fn(|_| "".to_string());
    choices[0] = reading.text.clone();

    dictionary
        .entries
        .iter()
        .filter_map(|e| {
            if e.id == entry.id {
                return None;
            }
            kanji_reading_pair(e, pos).map(|(_, reading)| reading.text.clone())
        })
        .choose_multiple_fill(&mut rng(), &mut choices[1..]);

    choices.shuffle(&mut rng());

    let answer = choices.iter().position(|o| reading.text == *o).unwrap();

    Some(Question::new(kanji.text.clone(), choices, answer, entry.levels()).unwrap())
}

fn new_kan_to_eng(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Question> {
    let (kanji, sense) = kanji_sense_pair(entry, pos)?;

    let mut choices = std::array::from_fn(|_| "".to_string());
    choices[0] = sense.gloss[0].content.clone();

    dictionary
        .entries
        .iter()
        .filter_map(|e| {
            if e.id == entry.id {
                return None;
            }
            kanji_sense_pair(e, pos).map(|(_, sense)| sense.gloss[0].content.clone())
        })
        .choose_multiple_fill(&mut rng(), &mut choices[1..]);

    choices.shuffle(&mut rng());

    let answer = choices
        .iter()
        .position(|o| sense.gloss[0].content == *o)
        .unwrap();

    Some(Question::new(kanji.text.clone(), choices, answer, entry.levels()).unwrap())
}

fn new_eng_to_kan(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Question> {
    let (kanji, sense) = kanji_sense_pair(entry, pos)?;

    let mut choices = std::array::from_fn(|_| "".to_string());
    choices[0] = kanji.text.clone();

    dictionary
        .entries
        .iter()
        .filter_map(|e| {
            if e.id == entry.id {
                return None;
            }
            kanji_sense_pair(e, pos).map(|(kanji, _)| kanji.text.clone())
        })
        .choose_multiple_fill(&mut rng(), &mut choices[1..]);

    choices.shuffle(&mut rng());

    let answer = choices.iter().position(|o| kanji.text == *o).unwrap();

    Some(
        Question::new(
            sense.gloss[0].content.clone(),
            choices,
            answer,
            entry.levels(),
        )
        .unwrap(),
    )
}

/// Conventiently extracts a [`Reading`] and correlated [`Sense`] from a [`DictEntry`] where
/// the sense has the `pos` tag and is guaranteed to have at least one gloss.
///
/// Returns [`None`] if no possible extraction.
fn reading_sense_pair(entry: &DictEntry, pos: Pos) -> Option<(&Reading, &Sense)> {
    let sense = entry
        .senses
        .iter()
        .find(|s| s.pos.contains(&pos) && !s.gloss.is_empty())?;

    let reading = entry
        .readings
        .iter()
        .find(|r| sense.relevant_reading.is_empty() || sense.relevant_reading.contains(&r.text))?;

    Some((reading, sense))
}

/// Conveniently extracts a [`Kanji`] and correlated [`Reading`] from a [`DictEntry`] where
/// the reading has the `pos` tag.
///
/// Returns [`None`] if no possible extraction.
fn kanji_reading_pair(entry: &DictEntry, pos: Pos) -> Option<(&Kanji, &Reading)> {
    let sense = entry.senses.iter().find(|s| s.pos.contains(&pos))?;

    let kanji = entry.kanjis.first()?;

    let reading = entry.readings.iter().find(|r| {
        (r.relevant_to.is_empty() || r.relevant_to.contains(&kanji.text))
            && (sense.relevant_reading.is_empty() || sense.relevant_reading.contains(&r.text))
    })?;

    Some((kanji, reading))
}

/// Conventiently extracts a [`Kanji`] and correlated [`Sense`] from a [`DictEntry`] where
/// the sense has the `pos` tag and is guaranteed to have at least one gloss.
///
/// Returns [`None`] if no possible extraction.
fn kanji_sense_pair(entry: &DictEntry, pos: Pos) -> Option<(&Kanji, &Sense)> {
    let sense = entry
        .senses
        .iter()
        .find(|s| s.pos.contains(&pos) && !s.gloss.is_empty())?;

    let kanji = entry.kanjis.first()?;

    Some((kanji, sense))
}
