use std::sync::{Arc, LazyLock};

use dashmap::DashMap;
use jplearnbot::dictionary::{NLevel, Pos};
use poise::serenity_prelude::{ComponentInteraction, CreateMessage, Http};
use rand::{rng, seq::SliceRandom};
use regex::Regex;
use tokio::sync::mpsc::{self, Sender};
use uuid::Uuid;

use crate::{config::KateContext, models::dictionary::Dictionary, modes::{multi_choice::{GameMessage, InteractionExitReason, Menu, PosFilter, Question, SessionAlreadyCreated}, ModeChoice}};

/// Manages all game sessions.
#[derive(Debug)]
pub struct Manager {
    /// Handle to serenity client.
    http: Arc<Http>,
    /// Dictionary for getting randomized samples and entries.
    dictionary: Arc<Dictionary>,
    /// Stores transmitters to game sessions. A Server/DM may only have
    /// one active game session.
    sessions: Arc<DashMap<u64, Sender<GameMessage>>>,
}

impl Manager {
    pub fn new(http: Arc<Http>) -> Self {
        Manager {
            http,
            dictionary: Dictionary::new().into(),
            sessions: DashMap::new().into(),
        }
    }

    /// Starts a new game session with the selected `mode`, `levels`, and `pos`.
    /// A separate task is created for game interaction handling. A [`Sender`]
    /// to the session is stored in [`Self::sessions`] for the duration of the game.
    /// The sessions exists while there are words in the pool and user interaction
    /// doesn't timeout from inactivity. A session can be stopped prematurely by sending
    /// a [`GameMessage::Close`] through the sender.
    ///
    /// # Errors
    /// Fails if user already has an active game.
    pub fn start_game(
        &self,
        ctx: &KateContext<'_>,
        mode: ModeChoice,
        levels: Vec<NLevel>,
        filters: Vec<PosFilter>,
    ) -> Result<(), SessionAlreadyCreated> {
        let session_id = ctx
            .guild_id()
            .map(|g| g.get())
            .unwrap_or(ctx.author().id.get());

        if self.sessions.contains_key(&session_id) {
            return Err(SessionAlreadyCreated);
        }

        let channel_id = ctx.channel_id();

        let http = Arc::clone(&self.http);
        let sessions = Arc::clone(&self.sessions);
        let dictionary = Arc::clone(&self.dictionary);

        let mut pos = pos_filters_to_pos(filters);

        let (tx, mut rx) = mpsc::channel(10);
        self.sessions.insert(session_id, tx);

        tokio::spawn(async move {
            // Natural expected exit reason, reason may change from interactions or lack thereof.
            let mut exit_reason = InteractionExitReason::PoolExhausted;

            for (round, entry) in dictionary.sample(&levels, &pos).await.iter().enumerate() {
                pos.shuffle(&mut rng());
                let Some(question) = pos
                    .iter()
                    .find_map(|&p| Question::new(entry, mode, p, &dictionary))
                else {
                    continue;
                };

                let menu_id = format!("{session_id},{}", Uuid::new_v4());
                let mut menu = Menu::new(&http, menu_id, question, entry);

                if channel_id
                    .send_files(
                        &http,
                        menu.create_files(),
                        menu.create_message(round + 1, mode),
                    )
                    .await
                    .is_err()
                {
                    exit_reason = InteractionExitReason::NetworkError;
                    break;
                }

                if let Err(reason) = menu.handle_interactions(&mut rx).await {
                    exit_reason = reason;
                    break;
                }
            }

            let message = match exit_reason {
                InteractionExitReason::PoolExhausted => {
                    Some("There are no more words left in the pool")
                }
                InteractionExitReason::Timeout => Some("Stopping game due to inactivity..."),
                InteractionExitReason::NetworkError => {
                    Some("Stopping game due to network error...")
                }
                InteractionExitReason::CloseRequest => None,
            };

            if let Some(message) = message {
                channel_id
                    .send_message(&http, CreateMessage::new().content(message))
                    .await
                    .ok();
            }

            sessions.remove(&session_id);
        });

        Ok(())
    }

    /// Stops `session_id`'s game if it exists.
    ///
    /// Returns true if there was an active game stopped.
    ///
    /// Returns false if there was no game associated with the `session_id`.
    pub async fn stop(&self, session_id: u64) -> bool {
        if let Some(tx) = self.sessions.get(&session_id) {
            tx.send(GameMessage::Close).await.ok();
            return true;
        }

        false
    }

    /// Sends `interaction` to the game session compatible with the interaction's custom_id.
    /// Does nothing if no matching game sesssion.
    pub async fn send(&self, interaction: ComponentInteraction) {
        if let Some(tx) =
            parse_session_id(&interaction.data.custom_id).and_then(|id| self.sessions.get(&id))
        {
            tx.send(GameMessage::Interaction(interaction)).await.ok();
        }
    }
}

/// Converts [`PosFilter`]'s to [`Pos`] using [`PosFilter::as_pos`].
fn pos_filters_to_pos(filters: Vec<PosFilter>) -> Vec<Pos> {
    let mut res = Vec::new();

    for filter in filters {
        res.extend_from_slice(filter.as_pos());
    }

    res
}

/// Extracts game session_id from interaction's custom_id.
fn parse_session_id(interaction_id: &str) -> Option<u64> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+").unwrap());

    let session_id = RE.find(interaction_id)?.as_str().parse().ok()?;

    Some(session_id)
}
