//! This module contains game modes.

pub mod multi_choice;

/// All game modes
#[derive(Debug, poise::ChoiceParameter, Clone, Copy)]
pub enum ModeChoice {
    #[name = "English ▶ ひらがな"]
    EngToHir,
    #[name = "ひらがな ▶ English"]
    HirToEng,
    #[name = "ひらがな ▶ 漢字"]
    HirToKan,
    #[name = "漢字 ▶ ひらがな"]
    KanToHir,
    #[name = "漢字 ▶ English"]
    KanToEng,
    #[name = "English ▶ 漢字"]
    EngToKan,
}
