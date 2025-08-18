//! This module contains communication models for communicating with lobbies and handling events.

use std::{
    borrow::BorrowMut,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use poise::{
    BoxFuture,
    serenity_prelude::{
        ChannelId, ComponentInteraction, ComponentInteractionCollector, CreateAttachment,
        CreateMessage, Error as SerenityError, Message,
        futures::{Stream, StreamExt},
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

/// Describes to a router what action to take next.
pub enum RoutingResult<T> {
    /// Continue listening from the provider and passing to routes.
    Continue,
    /// Stop listening. This also contains a value from the route that
    /// requested an exit.
    Exit(T),
}

type RouterMatcher<Context, Request, RouteExitT> =
    fn(&Route<Context, Request, RouteExitT>, &Context, &Request) -> bool;

/// Listens for events from provider, optionally validates events, and passes it to a route using
/// matching rules.
pub struct Router<Context, Request, Provider: self::Provider<Request>, RouteExitT, const N: usize> {
    /// Context data to be passed to each route.
    ctx: Context,
    /// Router level matcher for determining if event should be passed to a route.
    /// If a route defines its own matcher, that's used instead.
    matcher: Option<RouterMatcher<Context, Request, RouteExitT>>,
    /// Validate an event before trying to match.
    validator: Option<fn(&Context, &Request) -> bool>,
    /// Source of events.
    provider: Provider,
    /// Eligible routes to pass events to.
    routes: [Route<Context, Request, RouteExitT>; N],
}

impl<Context, Request, Provider: self::Provider<Request>, RouteExitT, const N: usize>
    Router<Context, Request, Provider, RouteExitT, N>
{
    pub fn new(
        ctx: Context,
        provider: Provider,
        routes: [Route<Context, Request, RouteExitT>; N],
    ) -> Self {
        Self {
            ctx,
            matcher: None,
            validator: None,
            provider,
            routes,
        }
    }

    /// Sets the router-level matcher.
    /// Note: A route's own matcher takes precedence in usage if it exists.
    pub fn matcher(
        mut self,
        matcher: fn(&Route<Context, Request, RouteExitT>, &Context, &Request) -> bool,
    ) -> Self {
        self.matcher = Some(matcher);
        self
    }

    fn matches(&self, route: &Route<Context, Request, RouteExitT>, event: &Request) -> bool {
        if let Some(matcher) = route.matcher {
            matcher(route, &self.ctx, event)
        } else if let Some(matcher) = self.matcher {
            matcher(route, &self.ctx, event)
        } else {
            false
        }
    }

    // Sets the validator.
    #[allow(dead_code)]
    pub fn validator(mut self, validator: fn(&Context, &Request) -> bool) -> Self {
        self.validator = Some(validator);
        self
    }

    fn validate(&self, event: &Request) -> bool {
        self.validator
            .map(|validate| validate(&self.ctx, event))
            .unwrap_or(true)
    }

    /// Listen for events from the provider and try to match it to a router.
    /// If an event could be matched to multiple routers, the event will be
    /// passed to the matchable route that was defined first.
    ///
    /// Returns Some if a router requested ending the listen.
    /// Return None if the provider ended the listen by passing None.
    pub async fn listen(&mut self) -> Option<RouteExitT> {
        while let Some(event) = self.provider.next().await {
            if !self.validate(&event) {
                continue;
            }

            let Some(route) = self.routes.iter().find(|route| self.matches(route, &event)) else {
                continue;
            };

            match (route.handle)(&mut self.ctx, event).await {
                RoutingResult::Continue => {}
                RoutingResult::Exit(val) => return Some(val),
            }
        }
        None
    }
}

/// A route that an event can be passed to.
pub struct Route<Context, Request, ExitT> {
    /// Identifier for the route. This could be an empty string and there would be no
    /// problems, but a unique tag could be used for match rules if desired.
    /// Uniqueness is not automatically checked by [`Router`].
    pub path: String,
    /// The handler that is called with context and event given by the router.
    handle: for<'a> fn(&'a mut Context, Request) -> BoxFuture<'a, RoutingResult<ExitT>>,
    /// Route level matcher for determining if event should be passed to this route.
    /// If defined, it will be used instead of a router level matcher.
    matcher: Option<fn(&Self, &Context, &Request) -> bool>,
}

impl<Context, Request, ExitT> Route<Context, Request, ExitT> {
    pub fn new(
        path: impl Into<String>,
        handle: for<'a> fn(&'a mut Context, Request) -> BoxFuture<'a, RoutingResult<ExitT>>,
    ) -> Self {
        Self {
            path: path.into(),
            handle,
            matcher: None,
        }
    }

    /// Sets the matcher. This matcher will take precedence in usage over a
    /// router level matcher.
    #[allow(dead_code)]
    pub fn matcher(mut self, matcher: fn(&Self, &Context, &Request) -> bool) -> Self {
        self.matcher = Some(matcher);
        self
    }
}

pub trait Provider<T> {
    /// Gets the next event. Returns None if there are no more events.
    async fn next(&mut self) -> Option<T>;
}

/// Wraps [`ComponentInteractionCollector`]
pub struct ComponentInteractionProvider {
    stream: Pin<Box<dyn Stream<Item = ComponentInteraction> + Send>>,
}

impl ComponentInteractionProvider {
    pub fn new(ctx: &KateContext<'_>, target_ids: &[impl Into<String> + Clone]) -> Self {
        const MAX_TIMEOUT: Duration = Duration::from_secs(60);

        let target_ids: Vec<String> = target_ids.iter().cloned().map(Into::into).collect();

        let collector = ComponentInteractionCollector::new(ctx)
            .author_id(ctx.author().id)
            .channel_id(ctx.channel_id())
            .timeout(MAX_TIMEOUT)
            .filter(move |ev| target_ids.contains(&ev.data.custom_id))
            .stream();

        Self {
            stream: Box::pin(collector),
        }
    }
}

impl Provider<ComponentInteraction> for ComponentInteractionProvider {
    async fn next(&mut self) -> Option<ComponentInteraction> {
        self.stream.next().await
    }
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
        self.send_message(CreateMessage::new().content(message)).await
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

/// This module contains helper matchers for [`Router`] and [`Route`]
pub mod matcher {
    use poise::serenity_prelude::ComponentInteraction;

    use crate::models::net::Route;

    /// Matches a ComponentInteraction's `data.custom_id` with `route.path`.
    pub fn full_route_path<Context, ExitT>(
        route: &Route<Context, ComponentInteraction, ExitT>,
        _ctx: &Context,
        event: &ComponentInteraction,
    ) -> bool {
        event.data.custom_id == route.path
    }
}
