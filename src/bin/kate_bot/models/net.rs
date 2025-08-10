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
    pub manager: Arc<Manager>,
}

pub type KateContext<'a> = poise::Context<'a, KateData, KateError>;
pub type KateError = Box<dyn std::error::Error + Send + Sync>;
pub type KateResult = Result<(), KateError>;

#[derive(Debug)]
pub enum GameMessage {
    /// A component interaction.
    Event(ComponentInteraction),
    /// Indicates game should close.
    Close,
    /// Took too long to receive any message at all.
    Timeout,
}

pub enum RoutingResult<T> {
    Continue,
    Exit(T),
}

type RouterMatcher<Context, Request, RouteExitT> = fn(&Route<Context, Request, RouteExitT>, &Context, &Request) -> bool;

pub struct Router<Context, Request, Provider: self::Provider<Request>, RouteExitT, const N: usize> {
    ctx: Context,
    matcher: Option<RouterMatcher<Context, Request, RouteExitT>>,
    validator: Option<fn(&Context, &Request) -> bool>,
    provider: Provider,
    routes: [Route<Context, Request, RouteExitT>; N],
}

impl<Context, Request, Provider: self::Provider<Request>, RouteExitT, const N: usize>
    Router<Context, Request, Provider, RouteExitT, N>
{
    pub fn new(ctx: Context, provider: Provider, routes: [Route<Context, Request, RouteExitT>; N]) -> Self {
        Self {
            ctx,
            matcher: None,
            validator: None,
            provider,
            routes,
        }
    }

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

    pub fn validator(mut self, validator: fn(&Context, &Request) -> bool) -> Self {
        self.validator = Some(validator);
        self
    }

    fn validate(&self, event: &Request) -> bool {
        self.validator
            .map(|validate| validate(&self.ctx, event))
            .unwrap_or(true)
    }

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

pub struct Route<Context, Request, ExitT> {
    pub path: String,
    handle: for<'a> fn(&'a mut Context, Request) -> BoxFuture<'a, RoutingResult<ExitT>>,
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

    pub fn matcher(mut self, matcher: fn(&Self, &Context, &Request) -> bool) -> Self {
        self.matcher = Some(matcher);
        self
    }
}

pub trait Provider<T> {
    async fn next(&mut self) -> Option<T>;
}

pub struct ComponentInteractionProvider {
    stream: Pin<Box<dyn Stream<Item = ComponentInteraction> + Send>>,
}

impl ComponentInteractionProvider {
    pub fn new(ctx: &KateContext<'_>, target_ids: &[impl Into<String> + Clone]) -> Self {
        const MAX_TIMEOUT: Duration = Duration::from_secs(60);

        let target_ids: Vec<String> = target_ids.iter().cloned().map(Into::into).collect();

        let stream = ComponentInteractionCollector::new(ctx)
            .author_id(ctx.author().id)
            .channel_id(ctx.channel_id())
            .timeout(MAX_TIMEOUT)
            .filter(move |ev| target_ids.contains(&ev.data.custom_id))
            .stream();

        Self {
            stream: Box::pin(stream),
        }
    }
}

impl Provider<ComponentInteraction> for ComponentInteractionProvider {
    async fn next(&mut self) -> Option<ComponentInteraction> {
        self.stream.next().await
    }
}
