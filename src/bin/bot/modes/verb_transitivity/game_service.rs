use std::sync::Arc;

use kate_bot::dictionary::NLevel;
use poise::serenity_prelude::UserId;
use rand::{
    Rng, rng,
    seq::{IndexedRandom, SliceRandom},
};

use crate::{
    models::{
        question::{self},
        scoreboard::Scoreboard,
    },
    modes::verb_transitivity::tverbs::TVerbPair,
};

type Question = question::Question<2>;

pub struct Service {
    tverbs: Vec<Arc<TVerbPair>>,
    levels: Vec<NLevel>,
    question: Option<Question>,
    scoreboard: Scoreboard,
}

impl Service {
    pub fn new(tverbs: &[Arc<TVerbPair>], levels: Vec<NLevel>) -> Self {
        let mut tverbs: Vec<Arc<TVerbPair>> = tverbs
            .iter()
            .filter(|tverb| {
                tverb
                    .intrans
                    .1
                    .levels()
                    .iter()
                    .all(|lvl| levels.contains(lvl))
                    && tverb
                        .trans
                        .1
                        .levels()
                        .iter()
                        .all(|lvl| levels.contains(lvl))
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

        let correct = choice == question.answer_idx();
        // Game is 50/50, so mark all choices guessed after the first try.
        question.guess(question.answer_idx());
        if correct {
            self.scoreboard.add_win(user);
        } else {
            self.scoreboard.add_loss(user);
        }

        correct
    }
}

fn make_question(pair: &TVerbPair) -> Question {
    if rng().random_bool(0.5) {
        make_intrans(pair)
    } else {
        make_trans(pair)
    }
}

fn make_intrans(pair: &TVerbPair) -> Question {
    let intrans_wrd = &pair.intrans.0;
    let trans_wrd = &pair.trans.0;

    let prompt = "A が ___".to_string();

    let choices = [intrans_wrd.clone(), trans_wrd.clone()];
    let answer = 0;

    let difficulty = pair.intrans.1.levels();

    Question::new(prompt, choices, answer, difficulty).unwrap()
}

fn make_trans(pair: &TVerbPair) -> Question {
    let trans_wrd = &pair.trans.0;
    let intrans_wrd = &pair.intrans.0;

    let prompt = format!(
        "{} {} {} を ___",
        random_person(),
        ["が", "は"].choose(&mut rng()).unwrap(),
        random_object()
    );

    let choices = [intrans_wrd.clone(), trans_wrd.clone()];
    let answer = 1;

    let difficulty = pair.trans.1.levels();

    Question::new(prompt, choices, answer, difficulty).unwrap()
}

fn random_person() -> &'static str {
    const PEOPLE: [&str; 1] = ["Bob"];
    PEOPLE.choose(&mut rng()).unwrap()
}

fn random_object() -> &'static str {
    const OBJECTS: [&str; 1] = ["apple"];
    OBJECTS.choose(&mut rng()).unwrap()
}
