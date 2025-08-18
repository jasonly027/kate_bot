//! This module provides ease of use components for creating messages.

use std::{cmp, fmt::Display, process};

use poise::{
    ChoiceParameter,
    serenity_prelude::{
        CreateActionRow, CreateAttachment, CreateButton, CreateEmbed, CreateEmbedFooter,
        CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, UserId,
    },
};
use tracing::error;

use crate::{
    models::{
        question::Question,
        scoreboard::{ScoreEntry, Scoreboard},
    },
    modes::ModeChoice,
    util,
};

pub fn single_button<T: Into<String>>(
    label: impl Into<String>,
) -> impl FnOnce(T) -> CreateActionRow {
    |id| _single_button(id, label)
}

fn _single_button(id: impl Into<String>, label: impl Into<String>) -> CreateActionRow {
    let btn = CreateButton::new(id).label(label);
    CreateActionRow::Buttons(vec![btn])
}

pub fn string_dropdown<T: Display, I: Into<String>>(
    options: &[T],
    placeholder: &str,
) -> impl Fn(I) -> CreateActionRow {
    |id| _string_dropdown(id, options, placeholder)
}

fn _string_dropdown<T: Display>(
    id: impl Into<String>,
    options: &[T],
    placeholder: &str,
) -> CreateActionRow {
    let options: Vec<CreateSelectMenuOption> = options
        .iter()
        .map(|opt| {
            let opt = opt.to_string();
            CreateSelectMenuOption::new(&opt, &opt).default_selection(true)
        })
        .collect();
    let len = options.len().try_into().unwrap_or_else(|error| {
        error!(
            len = options.len(),
            %error,
            "Too many options were supplied (max 25)"
        );
        process::exit(2);
    });

    let menu = CreateSelectMenu::new(id, CreateSelectMenuKind::String { options })
        .placeholder(placeholder)
        .min_values(1)
        .max_values(len);

    CreateActionRow::SelectMenu(menu)
}

pub fn scoreboard_embed(scoreboard: &Scoreboard) -> CreateEmbed {
    const TOP_N: usize = 10;
    const EPSILON: f64 = 1e-6;

    let mut scores: Vec<(UserId, ScoreEntry)> = scoreboard.iter().cloned().collect();
    scores.sort_unstable_by_key(|(user, score)| {
        let wr_scaled = (score.win_rate() / EPSILON).round() as i64;
        cmp::Reverse((wr_scaled, score.attempts(), *user))
    });

    let mut stats: Vec<String> = scores
        .iter()
        .take(TOP_N)
        .map(|(user, score)| {
            format!(
                "<@{user}> **{:.1}%** ({} - {})",
                score.win_rate() * 100.0,
                score.wins(),
                score.losses()
            )
        })
        .collect();

    // Prepend medals to top three stat holders
    stats
        .iter_mut()
        .zip(["🥇 ", "🥈 ", "🥉 ", "\n"])
        .for_each(|(details, medal)| {
            *details = format!("{medal}{details}");
        });

    CreateEmbed::new()
        .title("Scoreboard")
        .description(stats.join("\n"))
        .footer(CreateEmbedFooter::new(format!(
            "Rounds Played: {}",
            scoreboard.rounds()
        )))
}

pub fn prompt_embed(round: u32, mode: ModeChoice) -> CreateEmbed {
    CreateEmbed::new()
        .title(format!("Question {round}"))
        .field(mode.name(), "", false)
        .attachment("prompt.png")
}

pub fn prompt_image(text: &str) -> Vec<CreateAttachment> {
    vec![CreateAttachment::bytes(
        util::text_to_image(text),
        "prompt.png",
    )]
}

pub fn choice_buttons<const N: usize>(
    game_id: &str,
    round: u32,
    question: &Question<N>,
) -> Vec<CreateActionRow> {
    let buttons = question
        .choices()
        .iter()
        .zip(question.guessed())
        .enumerate()
        .map(|(idx, (label, &guessed))| {
            let id = format!("{game_id},{round},{idx}");
            CreateButton::new(id).label(label).disabled(guessed)
        })
        .collect();

    vec![CreateActionRow::Buttons(buttons)]
}
