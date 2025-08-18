use poise::CreateReply;
use tracing::instrument;

use crate::{
    message::{single_button, string_dropdown},
    models::net::{ComponentInteractionRouter, ContextBinder, KateContext, KateResult},
    modes::{
        ModeChoice,
        multi_choice::setup::router::{set_filters, set_levels, submit},
    },
    util::Logging,
};

type Context<'a, 'b> = ContextBinder<'a, KateContext<'b>, service::Service>;

/// Sets up creating a new multi_choice game.
#[instrument(name = "multi_choice", level = "warn", skip(ctx, mode), fields(invocation_id = ctx.id()))]
pub async fn handler(mut ctx: KateContext<'_>, mode: ModeChoice) -> KateResult {
    let mut service = service::Service::new(ctx.data().manager.clone(), mode.try_into().unwrap());
    let mut ctx = Context {
        ctx: &mut ctx,
        service: &mut service,
    };

    let mut router = ComponentInteractionRouter::new(ctx.id().to_string())
        .component(
            string_dropdown(&ctx.service.levels, "Select NLevel Pool(s)"),
            |ctx, ev| Box::pin(set_levels(ctx, ev)),
        )
        .component(
            string_dropdown(&ctx.service.filters, "Select parts-of-speech filters"),
            |ctx, ev| Box::pin(set_filters(ctx, ev)),
        )
        .component(single_button("Create Game"), |ctx, ev| {
            Box::pin(submit(ctx, ev))
        });

    let reply_handle = ctx
        .send(
            CreateReply::default()
                .components(router.take_components())
                .ephemeral(true),
        )
        .await
        .on_err_warn_send_failed()?;
    router.listen(&mut ctx).await;
    reply_handle.delete(*ctx).await.ok();

    Ok(())
}

mod router {
    use poise::serenity_prelude::{
        ComponentInteraction, ComponentInteractionDataKind as EventKind, CreateInteractionResponse,
        CreateInteractionResponseMessage, EditInteractionResponse,
    };
    use tracing::{error, instrument};

    use crate::{
        modes::multi_choice::setup::Context,
        util::{Logging, ParseUnwrapAll},
    };

    #[instrument(level = "warn", skip(ctx, event))]
    pub async fn set_levels(ctx: &mut Context<'_, '_>, event: ComponentInteraction) -> bool {
        let EventKind::StringSelect { values } = &event.data.kind else {
            error!(?event.data.kind, "Expected EventKind::StringSelect");
            return true;
        };

        ctx.service.levels = values.parse_unwrap_all();
        event
            .create_response(**ctx, CreateInteractionResponse::Acknowledge)
            .await
            .on_err_warn_send_failed()
            .ok();
        true
    }

    #[instrument(level = "warn", skip(ctx, event))]
    pub async fn set_filters(ctx: &mut Context<'_, '_>, event: ComponentInteraction) -> bool {
        let EventKind::StringSelect { values } = &event.data.kind else {
            error!(?event.data.kind, "Expected EventKind::StringSelect");
            return true;
        };

        ctx.service.filters = values.parse_unwrap_all();
        event
            .create_response(**ctx, CreateInteractionResponse::Acknowledge)
            .await
            .on_err_warn_send_failed()
            .ok();
        true
    }

    #[instrument(level = "warn", skip(ctx, event))]
    pub async fn submit(ctx: &mut Context<'_, '_>, event: ComponentInteraction) -> bool {
        let EventKind::Button = &event.data.kind else {
            error!(?event.data.kind, "Expected EventKind::Button");
            return true;
        };

        let msg = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("Creating game...")
                .ephemeral(true),
        );
        event
            .create_response(**ctx, msg)
            .await
            .on_err_warn_send_failed()
            .ok();

        let success = ctx.service.submit(ctx);
        if success {
            event
                .delete_response(**ctx)
                .await
                .on_err_warn("Delete creating game message failed")
                .ok();
            false
        } else {
            event
                .edit_response(
                    **ctx,
                    EditInteractionResponse::new()
                        .content("Active game in progress. Please stop it."),
                )
                .await
                .on_err_warn("Edit creating game message failed")
                .ok();
            true
        }
    }
}

mod service {
    use std::sync::Arc;

    use kate_bot::dictionary::NLevel;
    use strum::IntoEnumIterator;

    use crate::{
        models::{
            dictionary::PosFilter,
            manager::Manager,
            net::{GameContext, KateContext},
        },
        modes::multi_choice::{
            MultiChoiceMode, game_router,
            game_service::{self, GameSettings},
        },
    };

    /// Stateful service for setting up a multi_choice game.
    pub struct Service {
        manager: Arc<Manager>,
        /// Desired levels when creating dictionary subset
        pub levels: Vec<NLevel>,
        /// Desired filters when creating dictionary subset
        pub filters: Vec<PosFilter>,
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

        /// Attemps to create a game with `lobby_id`. The game is spawned as a separate
        /// async task and this returns immediately.
        ///
        /// Return true if game creation was successful. Returns false if there's already
        /// an existing game.
        pub fn submit(&self, ctx: &KateContext<'_>) -> bool {
            let Some(receiver) = self.manager.create_lobby(ctx) else {
                return false;
            };

            tokio::spawn({
                let ctx = GameContext::new(ctx);
                let settings = GameSettings {
                    mode: self.mode,
                    levels: self.levels.clone(),
                    filters: self.filters.clone(),
                };
                let service = game_service::Service::new(self.manager.dictionary.clone(), settings);

                async move {
                    game_router::handler(ctx, service, receiver).await;
                }
            });

            true
        }
    }
}
