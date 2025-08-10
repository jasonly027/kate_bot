//! This module contains communication models for communicating with lobbies and handling events.

use std::{pin::Pin, sync::Arc, time::Duration};

use poise::{
    BoxFuture,
    serenity_prelude::{
        ComponentInteraction, ComponentInteractionCollector,
        futures::{Stream, StreamExt},
    },
};

use crate::models::manager::Manager;

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
