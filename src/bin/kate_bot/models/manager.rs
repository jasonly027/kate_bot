use std::sync::Arc;

use dashmap::{DashMap, Entry};
use poise::serenity_prelude::{ComponentInteraction, Http};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::{
    models::{dictionary::Dictionary, net::GameMessage},
    util::{GameId, LobbyId},
};

/// Manages all game sessions.
#[derive(Debug)]
pub struct Manager {
    /// Handle to serenity client.
    pub http: Arc<Http>,
    /// Dictionary for getting randomized samples and entries.
    pub dictionary: Arc<Dictionary>,
    /// Stores transmitters to game sessions. A Server/DM may only have
    /// one active game session.
    sessions: Arc<DashMap<u64, (String, Sender<GameMessage>)>>,
}

impl Manager {
    pub fn new(http: Arc<Http>) -> Self {
        Manager {
            http,
            dictionary: Arc::new(Dictionary::new()),
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn create_lobby(&self, lobby_id: u64, game_id: String) -> Option<Receiver<GameMessage>> {
        let Entry::Vacant(entry) = self.sessions.entry(lobby_id) else {
            return None;
        };

        const BUFFER_SIZE: usize = 10;
        let (sender, receiver) = mpsc::channel(BUFFER_SIZE);
        entry.insert((game_id, sender));

        Some(receiver)
    }

    pub fn remove_lobby(&self, id: u64) -> bool {
        let Entry::Occupied(entry) = self.sessions.entry(id) else {
            return false;
        };
        entry.remove();
        true
    }

    /// Sends `interaction` to the game session compatible with the interaction's custom_id.
    /// Does nothing if no matching game sesssion.
    pub async fn send(&self, event: ComponentInteraction) {
        if let Some(session) = self.sessions.get(&event.lobby_id()) {
            let (game_id, tx) = (&session.0, &session.1);
            if event.game_id() == game_id {
                tx.send(GameMessage::Event(event)).await.ok();
            }
        }
    }
}
