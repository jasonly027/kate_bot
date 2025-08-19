use std::sync::{Arc, OnceLock};

use kate_bot::dictionary::NLevel;
use poise::serenity_prelude::UserId;
use rand::{
    Rng, rng,
    seq::{IndexedRandom, SliceRandom},
};

use crate::{
    models::{
        dictionary::Dictionary,
        question::{self},
        scoreboard::Scoreboard,
    },
    modes::verb_transitivity::tverbs::{TVerbPooledPair, tverb_pooled_pairs},
};

type Question = question::Question<2>;

static TVERBS: OnceLock<Vec<Arc<TVerbPooledPair>>> = OnceLock::new();

fn tverbs(dictionary: &Dictionary) -> &'static [Arc<TVerbPooledPair>] {
    TVERBS.get_or_init(|| {
        tverb_pooled_pairs(dictionary)
            .into_iter()
            .map(Arc::new)
            .collect()
    })
}

enum VerbType {
    Intrans,
    Trans,
}

pub struct Service {
    tverbs: Vec<Arc<TVerbPooledPair>>,
    levels: Vec<NLevel>,
    question: Option<Question>,
    scoreboard: Scoreboard,
}

impl Service {
    pub fn new(dictionary: &Dictionary, levels: Vec<NLevel>) -> Self {
        let mut tverbs: Vec<Arc<TVerbPooledPair>> = tverbs(dictionary)
            .iter()
            .filter(|tverb| {
                tverb
                    .intrans_entry()
                    .levels()
                    .iter()
                    .any(|lvl| levels.contains(lvl))
                    || tverb
                        .trans_entry()
                        .levels()
                        .iter()
                        .any(|lvl| levels.contains(lvl))
            })
            .cloned()
            .collect();
        tverbs.shuffle(&mut rng());

        Self {
            tverbs,
            levels,
            question: None,
            scoreboard: Scoreboard::new(),
        }
    }

    pub fn next_round(&mut self) {
        self.question = self.tverbs.pop().map(|p| make_question(&p));
        if self.question.is_some() {
            self.scoreboard.next_round();
        }
    }

    #[allow(dead_code)]
    pub fn levels(&self) -> &[NLevel] {
        &self.levels
    }

    pub fn question(&self) -> Option<&Question> {
        self.question.as_ref()
    }

    pub fn scoreboard(&self) -> &Scoreboard {
        &self.scoreboard
    }

    pub fn round(&self) -> u32 {
        self.scoreboard.rounds()
    }

    pub fn select_choice(&mut self, user: UserId, choice: usize) -> bool {
        let Some(question) = self.question.as_mut() else {
            return false;
        };

        // Game is 50/50, so mark all choices guessed after the first try.
        question.guess(question.answer_idx());

        let correct = choice == question.answer_idx();
        self.scoreboard.record(user, correct);
        correct
    }
}

fn make_question(pair: &TVerbPooledPair) -> Question {
    if rng().random_bool(0.5) {
        make_intrans(pair)
    } else {
        make_trans(pair)
    }
}

fn make_intrans(pair: &TVerbPooledPair) -> Question {
    let prompt = format!(
        "{} が ___",
        pair.intrans_nouns().choose(&mut rng()).unwrap()
    );
    let (choices, answer) = make_choices(pair, VerbType::Intrans);
    let difficulty = pair.intrans_entry().levels();

    Question::new(prompt, choices, answer, difficulty).unwrap()
}

fn make_trans(pair: &TVerbPooledPair) -> Question {
    let prompt = format!(
        "{} {} {} を ___",
        pair.trans_subjects().choose(&mut rng()).unwrap(),
        ["が", "は"].choose(&mut rng()).unwrap(),
        pair.trans_nouns().choose(&mut rng()).unwrap(),
    );
    let (choices, answer) = make_choices(pair, VerbType::Trans);
    let difficulty = pair.trans_entry().levels();

    Question::new(prompt, choices, answer, difficulty).unwrap()
}

fn make_choices(pair: &TVerbPooledPair, verbt: VerbType) -> ([String; 2], usize) {
    let intrans = match pair.intrans_entry().readings.first() {
        Some(hir) => format!("{} ({})", pair.intrans_kanji(), hir.text),
        None => pair.intrans_kanji().to_string(),
    };
    let trans = match pair.trans_entry().readings.first() {
        Some(hir) => format!("{} ({})", pair.trans_kanji(), hir.text),
        None => pair.trans_kanji().to_string(),
    };

    let answer = match verbt {
        VerbType::Intrans => intrans.clone(),
        VerbType::Trans => trans.clone(),
    };

    let mut choices = [intrans, trans];
    choices.shuffle(&mut rng());
    let answer_idx = choices.iter().position(|c| *c == answer).expect("answer is a clone");

    (choices, answer_idx)
}
