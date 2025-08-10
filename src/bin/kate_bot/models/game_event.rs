use std::ops::{Deref, DerefMut};

use poise::serenity_prelude::ComponentInteraction;
use tracing::debug;

pub struct GameEvent {
    pub inner: ComponentInteraction,
    game_id: String,
}

impl GameEvent {
    fn new(interaction: ComponentInteraction) -> Option<Self> {
        let fields: Vec<_> = interaction.data.custom_id.split(",").collect();
        let [game_id] = fields.as_slice() else {
            debug!(
                ?fields,
                "Invalid number of fields in interaction's custom_id"
            );
            return None;
        };

        let game_id = game_id.to_string();

        Some(Self {
            inner: interaction,
            game_id,
        })
    }
}

impl Deref for GameEvent {
    type Target = ComponentInteraction;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for GameEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
