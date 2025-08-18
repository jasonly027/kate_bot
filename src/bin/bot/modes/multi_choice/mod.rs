//! This module contains the multiple choice game mode
//! of translating Kanji/Reading to English and vice-versa.

use strum_macros::Display;

mod game_router;
mod game_service;
mod setup;

pub use setup::handler;

use crate::modes::ModeChoice;

/// Submodes under the multiple choice game mode.
#[derive(Debug, Clone, Copy, Display)]
pub enum MultiChoiceMode {
    EngToHir,
    HirToEng,
    HirToKan,
    KanToHir,
    KanToEng,
    EngToKan,
}

impl From<MultiChoiceMode> for ModeChoice {
    fn from(val: MultiChoiceMode) -> Self {
        match val {
            MultiChoiceMode::EngToHir => ModeChoice::EngToHir,
            MultiChoiceMode::HirToEng => ModeChoice::HirToEng,
            MultiChoiceMode::HirToKan => ModeChoice::HirToKan,
            MultiChoiceMode::KanToHir => ModeChoice::KanToHir,
            MultiChoiceMode::KanToEng => ModeChoice::KanToEng,
            MultiChoiceMode::EngToKan => ModeChoice::EngToKan,
        }
    }
}

impl TryInto<MultiChoiceMode> for ModeChoice {
    type Error = &'static str;

    fn try_into(self) -> Result<MultiChoiceMode, Self::Error> {
        match self {
            ModeChoice::EngToHir => Ok(MultiChoiceMode::EngToHir),
            ModeChoice::HirToEng => Ok(MultiChoiceMode::HirToEng),
            ModeChoice::HirToKan => Ok(MultiChoiceMode::HirToKan),
            ModeChoice::KanToHir => Ok(MultiChoiceMode::KanToHir),
            ModeChoice::KanToEng => Ok(MultiChoiceMode::KanToEng),
            ModeChoice::EngToKan => Ok(MultiChoiceMode::EngToKan),
            _ => Err("Invalid variant for MultiChoiceMode"),
        }
    }
}
