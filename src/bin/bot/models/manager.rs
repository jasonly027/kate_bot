//! This module contains [`Manager`] which manages game sessions.

use std::sync::Arc;

use dashmap::{DashMap, Entry};
use poise::serenity_prelude::{ComponentInteraction, Http};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::{
    models::{dictionary::Dictionary, net::GameMessage},
    util::{GameId, LobbyId},
};

/// Manages game sessions.
#[derive(Debug)]
pub struct Manager {
    /// Handle to serenity client.
    pub http: Arc<Http>,
    /// Reference to a dictionary.
    pub dictionary: Arc<Dictionary>,
    /// The key should be [`LobbyId`], the value should be ([`GameId`], sender to that game).
    /// By using LobbyId, a guild or direct message channel can only have
    /// one active game at a time. GameId is necessary to further differentiate
    /// stray messages coming from the guild or direct message channel.
    sessions: Arc<DashMap<u64, (String, Sender<GameMessage>)>>,
}

impl Manager {
    /// Dictionary is initialized loaded.
    pub fn new(http: Arc<Http>) -> Self {
        Manager {
            http,
            dictionary: Arc::new(Dictionary::new()),
            sessions: Arc::new(DashMap::new()),
        }
    }

    /// Tries to create a new lobby with `lobby_id` and `game_id`. Returns
    /// the receiver for communicating with the lobby. Returns none if there's
    /// already a lobby with `lobby_id`.
    pub fn create_lobby(&self, lobby_id: u64, game_id: String) -> Option<Receiver<GameMessage>> {
        let Entry::Vacant(entry) = self.sessions.entry(lobby_id) else {
            return None;
        };

        const BUFFER_SIZE: usize = 10;
        let (sender, receiver) = mpsc::channel(BUFFER_SIZE);
        entry.insert((game_id, sender));

        Some(receiver)
    }

    /// Removes the lobby at `lobby_id`. Returns true if a lobby was removed.
    /// Return false if the lobby never existed.
    pub fn remove_lobby(&self, lobby_id: u64) -> bool {
        let Entry::Occupied(entry) = self.sessions.entry(lobby_id) else {
            return false;
        };
        entry.remove();
        true
    }

    /// Sends `event` to the lobby with matching [`LobbyId`] and [`GameId`] if it exists.
    pub async fn send(&self, event: ComponentInteraction) {
        if let Some(session) = self.sessions.get(&event.lobby_id()) {
            let (game_id, tx) = (&session.0, &session.1);
            if event.game_id() == game_id {
                tx.send(GameMessage::Event(event)).await.ok();
            }
        }
    }
}
