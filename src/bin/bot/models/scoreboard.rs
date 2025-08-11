//! This module contains [`Scoreboard`] for keeping track of
//! participating players's winrates across rounds during a game.

use std::slice;

use poise::serenity_prelude::UserId;

use crate::util::IndexMap;

/// Keeps track of participating players's winrates across rounds
/// during a game. It is assumed that a round ends and a new one starts
/// when a win is added to any player.
#[derive(Clone, Debug, Default)]
pub struct Scoreboard {
    /// Total number of rounds played
    rounds: u32,
    players: IndexMap<UserId, ScoreEntry>,
}

impl Scoreboard {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn get(&self, user: UserId) -> Option<&ScoreEntry> {
        self.players.get(&user)
    }

    /// Gets total number of rounds played.
    pub fn rounds(&self) -> u32 {
        self.rounds
    }

    pub fn iter(&self) -> slice::Iter<'_, (UserId, ScoreEntry)> {
        self.players.iter()
    }

    /// Records a win for the given user. The total rounds counter is incremented
    /// due to the assumption that a win means the current round is over.
    pub fn add_win(&mut self, user: UserId) {
        self.rounds += 1;

        self.players
            .get_or_insert_with(user, ScoreEntry::default)
            .add_win();
    }

    /// Records a loss for the given user.
    pub fn add_loss(&mut self, user: UserId) {
        self.players
            .get_or_insert_with(user, ScoreEntry::default)
            .add_loss();
    }
}

impl IntoIterator for Scoreboard {
    type Item = (UserId, ScoreEntry);

    type IntoIter = <IndexMap<UserId, ScoreEntry> as std::iter::IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.players.into_iter()
    }
}

/// Keeps track of wins and losses across rounds for one player.
#[derive(Debug, Clone, Default)]
pub struct ScoreEntry {
    wins: u32,
    losses: u32,
}

impl ScoreEntry {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn wins(&self) -> u32 {
        self.wins
    }

    pub fn losses(&self) -> u32 {
        self.losses
    }

    /// Gets total attempts, i.e., the sum of wins and losses.
    pub fn attempts(&self) -> u32 {
        self.wins + self.losses
    }

    /// Gets the win rate, i.e., wins divided by total attempts.
    /// 0 is returned if there have been no attempts.
    pub fn win_rate(&self) -> f64 {
        if self.attempts() == 0 {
            return 0.0;
        }
        self.wins as f64 / self.attempts() as f64
    }

    /// Adds a win to the score.
    pub fn add_win(&mut self) {
        self.wins += 1;
    }

    /// Adds a loss to the score.
    pub fn add_loss(&mut self) {
        self.losses += 1;
    }
}
