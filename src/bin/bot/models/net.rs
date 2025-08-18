//! This module contains communication models for communicating with lobbies and handling events.

use std::{
    borrow::{Borrow, BorrowMut},
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Duration,
};

use poise::{
    BoxFuture,
    serenity_prelude::{
        ChannelId, ComponentInteraction, ComponentInteractionCollector, CreateActionRow,
        CreateAttachment, CreateMessage, Error as SerenityError, Message, futures::StreamExt,
    },
};
use tokio::{sync::mpsc::Receiver, time::timeout};

use crate::{models::manager::Manager, util::LobbyId};

#[derive(Debug)]
pub struct KateData {
    // Reference to the global manager.
    pub manager: Arc<Manager>,
}

pub type KateContext<'a> = poise::Context<'a, KateData, KateError>;
pub type KateError = Box<dyn std::error::Error + Send + Sync>;
pub type KateResult = Result<(), KateError>;

/// A message to a lobby.
#[derive(Debug)]
pub enum GameMessage {
    /// An event.
    Event(ComponentInteraction),
    /// Indicates game should close.
    Close,
    /// Took too long to receive a message. Time for timeout is
    /// determined by the listener.
    Timeout,
}

/// The handler to dispatch a ComponentInteraction event to.
pub type ComponentInteractionHandler<Context> =
    for<'a> fn(&'a mut Context, ComponentInteraction) -> BoxFuture<'a, bool>;

/// Registers components and their handlers and routes them
/// appropriately on [`Self::listen`]
#[derive(Debug, Default)]
pub struct ComponentInteractionRouter<'a, Context>
where
    Context: BorrowMut<KateContext<'a>>,
{
    id: String,
    ids: Vec<String>,
    components: Vec<CreateActionRow>,
    handlers: Vec<ComponentInteractionHandler<Context>>,
    _marker: PhantomData<&'a ()>,
}

impl<'a, Context> ComponentInteractionRouter<'a, Context>
where
    Context: BorrowMut<KateContext<'a>>,
{
    /// Creates a new router. `id` is used as the root
    /// for each registered component's id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ids: Default::default(),
            components: Default::default(),
            handlers: Default::default(),
            _marker: Default::default(),
        }
    }

    /// Registers a new component and its handler.
    pub fn component(
        mut self,
        component: impl FnOnce(String) -> CreateActionRow,
        handler: ComponentInteractionHandler<Context>,
    ) -> Self {
        self.ids
            .push(format!("{}-{}", self.id, self.components.len()));
        self.components
            .push(component(self.ids.last().expect("we just pushed").clone()));
        self.handlers.push(handler);

        self
    }

    /// Returns all registered components so they can be used in
    /// a message. This takes from the internal component store leaving
    /// it empty, so an immediate subsequent call would result
    /// in an empty Vec.
    pub fn take_components(&mut self) -> Vec<CreateActionRow> {
        mem::take(&mut self.components)
    }

    /// Blocks and listens for component interactions to route.
    /// If a handler returns false, listening is stopped prematurely.
    pub async fn listen(&mut self, ctx: &mut Context) {
        let kctx: &mut KateContext<'_> = ctx.borrow_mut();

        const MAX_TIMEOUT: Duration = Duration::from_secs(60);
        let mut collector = ComponentInteractionCollector::new(&kctx)
            .author_id(kctx.author().id)
            .channel_id(kctx.channel_id())
            .timeout(MAX_TIMEOUT)
            .stream();

        while let Some(ev) = collector.next().await {
            let Some(handler) = self
                .ids
                .iter()
                .position(|id| *id == ev.data.custom_id)
                .map(|id| self.handlers[id])
            else {
                continue;
            };

            if !handler(ctx, ev).await {
                return;
            }
        }
    }
}

pub trait Provider<T> {
    /// Gets the next event. Returns None if there are no more events.
    async fn next(&mut self) -> Option<T>;
}

impl<T> Provider<GameMessage> for T
where
    T: BorrowMut<Receiver<GameMessage>>,
{
    /// # Warning
    /// This **always** return Some. When the sender side has been closed
    /// GameMessage::Close is repeatedly returned
    async fn next(&mut self) -> Option<GameMessage> {
        const MAX_TIMEOUT: Duration = Duration::from_secs(120);

        let Ok(msg) = timeout(MAX_TIMEOUT, self.borrow_mut().recv()).await else {
            return Some(GameMessage::Timeout);
        };
        Some(msg.unwrap_or(GameMessage::Close))
    }
}

/// Wrapper for storing an underlying context and service.
pub struct ContextBinder<'a, Context, Service> {
    pub ctx: &'a mut Context,
    pub service: &'a mut Service,
}

impl<Context, Service> Deref for ContextBinder<'_, Context, Service> {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl<Context, Service> DerefMut for ContextBinder<'_, Context, Service> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ctx
    }
}

impl<Context, Service> Borrow<Context> for ContextBinder<'_, Context, Service> {
    fn borrow(&self) -> &Context {
        self.ctx
    }
}

impl<Context, Service> BorrowMut<Context> for ContextBinder<'_, Context, Service> {
    fn borrow_mut(&mut self) -> &mut Context {
        self.ctx
    }
}

/// Context during a game. Automatically deletes the lobby
/// from the manager when dropped.
#[derive(Debug)]
pub struct GameContext {
    /// Uniquely identifies this game instance.
    pub game_id: String,
    /// The lobby this game is being played in.
    pub lobby_id: u64,
    /// Identifies the channel to send messages to.
    pub channel_id: ChannelId,
    /// A handle to the manager.
    pub manager: Arc<Manager>,
}

impl GameContext {
    pub fn new(ctx: &KateContext<'_>) -> Self {
        Self {
            game_id: ctx.id().to_string(),
            lobby_id: ctx.lobby_id(),
            channel_id: ctx.channel_id(),
            manager: ctx.data().manager.clone(),
        }
    }

    /// Sends a text message to the game's channel. Equivalent to using [`Self::send_message`]
    /// with a [`CreateMessage`] with content set to `message`.
    pub async fn send_text(&self, message: impl Into<String>) -> Result<Message, SerenityError> {
        self.send_message(CreateMessage::new().content(message))
            .await
    }

    /// Sends a message to the game's channel.
    pub async fn send_message(&self, message: CreateMessage) -> Result<Message, SerenityError> {
        self.channel_id
            .send_message(&self.manager.http, message)
            .await
    }

    /// Sends a message and files to the game's channel.
    pub async fn send_files(
        &self,
        message: CreateMessage,
        files: Vec<CreateAttachment>,
    ) -> Result<Message, SerenityError> {
        self.channel_id
            .send_files(&self.manager.http, files, message)
            .await
    }
}

impl Drop for GameContext {
    fn drop(&mut self) {
        self.manager.remove_lobby(self.lobby_id);
    }
}
