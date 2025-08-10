use std::sync::Arc;

use jplearnbot::dictionary::NLevel;
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

pub struct Service {
    manager: Arc<Manager>,
    levels: Vec<NLevel>,
    filters: Vec<PosFilter>,
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
