use std::sync::Arc;

use kate_bot::dictionary::NLevel;
use poise::serenity_prelude::ChannelId;
use strum::IntoEnumIterator;

use crate::{
    models::{dictionary::PosFilter, manager::Manager},
    modes::{
        ModeChoice,
        multi_choice::{
            MultiChoiceMode, game_router,
            game_service::{self, GameSettings},
        },
    },
};

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
            #[allow(unreachable_patterns)]
            _ => Err("Invalid variant for MultiChoiceMode"),
        }
    }
}

/// Stateful service for setting up a multi_choice game.
pub struct Service {
    manager: Arc<Manager>,
    /// Desired levels when creating dictionary subset
    levels: Vec<NLevel>,
    /// Desired filters when creating dictionary subset
    filters: Vec<PosFilter>,
    /// Desired sub game mode
    mode: MultiChoiceMode,
}

impl Service {
    pub fn new(manager: Arc<Manager>, mode: MultiChoiceMode) -> Self {
        // When adding new ids, remember to append it to Self::ids().
        Self {
            manager,
            levels: NLevel::iter().collect(),
            filters: PosFilter::iter().collect(),
            mode,
        }
    }

    pub fn levels(&self) -> &[NLevel] {
        &self.levels
    }

    pub fn set_levels(&mut self, levels: Vec<NLevel>) {
        self.levels = levels;
    }

    pub fn filters(&self) -> &[PosFilter] {
        &self.filters
    }

    pub fn set_filters(&mut self, filters: Vec<PosFilter>) {
        self.filters = filters;
    }

    /// Attemps to create a game with `lobby_id`. The game is spawned as a separate
    /// async task and this returns immediately.
    /// 
    /// Return true if game creation was successful. Returns false if there's already
    /// an existing game.
    pub fn submit(&mut self, lobby_id: u64, channel_id: ChannelId, game_id: u64) -> bool {
        let Some(receiver) = self.manager.create_lobby(lobby_id, game_id.to_string()) else {
            return false;
        };

        tokio::spawn({
            let ctx = game_router::GameContext {
                game_id: game_id.to_string(),
                lobby_id,
                channel_id,
                manager: self.manager.clone(),
            };

            let dictionary = self.manager.dictionary.clone();
            let settings = GameSettings {
                mode: self.mode,
                levels: self.levels.clone(),
                filters: self.filters.clone(),
            };
            let service = game_service::Service::new(dictionary, settings);

            async move {
                game_router::handler(ctx, service, receiver).await;
            }
        });

        true
    }
}
