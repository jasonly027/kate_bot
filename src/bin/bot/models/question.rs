//! This module contains [`Question`] which models N options multiple choice questions.

use kate_bot::dictionary::NLevel;
use thiserror::Error;

/// Question construction errors
#[derive(Debug, Error)]
pub enum QuestionError {
    #[error("Choices array must not be empty")]
    EmptyChoices,

    #[error("Answer index must be in range of choices array")]
    AnswerOutOfRange,
}

/// Models a N option multiple choice quesiton.
#[derive(Debug)]
pub struct Question<const N: usize> {
    /// The question being asked.
    prompt: String,
    /// The possible answers to `prompt`.
    choices: [String; N],
    /// Which choices were already guessed.
    guessed: [bool; N],
    /// The index of the correct answer in `choices`.
    answer_idx: usize,
    /// The difficulty category(ies) of this question.
    difficulty: Vec<NLevel>,
}

impl<const N: usize> Question<N> {
    pub fn new(
        prompt: String,
        choices: [String; N],
        answer: usize,
        difficulty: Vec<NLevel>,
    ) -> Result<Self, QuestionError> {
        if choices.is_empty() {
            return Err(QuestionError::EmptyChoices);
        }
        if answer >= choices.len() {
            return Err(QuestionError::AnswerOutOfRange);
        }

        Ok(Self {
            prompt,
            choices,
            guessed: [false; N],
            answer_idx: answer,
            difficulty,
        })
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn choices(&self) -> &[String; N] {
        &self.choices
    }

    pub fn guessed(&self) -> &[bool; N] {
        &self.guessed
    }

    pub fn answer(&self) -> &str {
        &self.choices[self.answer_idx]
    }

    pub fn difficulty(&self) -> &Vec<NLevel> {
        &self.difficulty
    }

    #[allow(dead_code)]
    pub fn answer_idx(&self) -> usize {
        self.answer_idx
    }

    /// Inquire if one of the choices is the answer. Always returns false
    /// on a `choice` index that is out of range of the valid choices range.
    pub fn guess(&mut self, choice: usize) -> bool {
        let correct = choice == self.answer_idx;

        if correct {
            self.guessed.iter_mut().for_each(|g| *g = true);
        } else if let Some(g) = self.guessed.get_mut(choice) {
            *g = true;
        }

        correct
    }
}
