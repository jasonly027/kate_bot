use jplearnbot::dictionary::NLevel;
use thiserror::Error;

#[derive(Debug)]
pub struct Question<const N: usize> {
    prompt: String,
    choices: [String; N],
    guessed: [bool; N],
    answer_idx: usize,
    difficulty: Vec<NLevel>,
}

#[derive(Debug, Error)]
pub enum QuestionError {
    #[error("Choices array must not be empty")]
    EmptyChoices,

    #[error("Answer index must be in range of choices array")]
    AnswerOutOfRange,
}

impl<const N: usize> Question<N> {
    pub fn new(prompt: String, choices: [String; N], answer: usize, difficulty: Vec<NLevel>) -> Result<Self, QuestionError> {
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
            difficulty
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
