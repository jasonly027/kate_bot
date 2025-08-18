use poise::CreateReply;
use tracing::instrument;

use crate::{
    message::{ids, single_button, string_dropdown},
    models::net::{KateContext, KateResult},
    modes::ModeChoice,
    util::Logging,
};

/// Sets up creating a new multi_choice game.
#[instrument(level = "warn", skip(ctx, mode), fields(invocation_id = ctx.id()))]
pub async fn handler(mut ctx: KateContext<'_>, mode: ModeChoice) -> KateResult {
    let mut service = service::Service::new(ctx.data().manager.clone(), mode.try_into().unwrap());

    let [levels_id, filters_id, submit_id] = ids(ctx.id(), ["lvls", "fltrs", "sbmt"]);
    let first_reply = CreateReply::default()
        .components(vec![
            string_dropdown(&levels_id, &service.levels, "Select NLevel Pool(s)"),
            string_dropdown(
                &filters_id,
                &service.filters,
                "Select parts-of-speech filters",
            ),
            single_button(&submit_id, "Create Game"),
        ])
        .ephemeral(true);

    let reply_handle = ctx.send(first_reply).await.on_err_warn_send_failed()?;

    // Listen and handle interactions on the game setup buttons.
    router::router(&mut ctx, &mut service, &levels_id, &filters_id, &submit_id)
        .listen()
        .await;

    reply_handle.delete(ctx).await.ok();

    Ok(())
}

mod router {
    use poise::serenity_prelude::{
        ComponentInteraction, ComponentInteractionDataKind as EventKind, CreateInteractionResponse,
        CreateInteractionResponseMessage, EditInteractionResponse,
    };
    use tracing::{error, instrument};

    use crate::{
        models::net::{
            ComponentInteractionProvider, ContextBinder, KateContext, Route, Router, RoutingResult,
        },
        modes::multi_choice::setup::service::Service as SetupService,
        util::{Logging, ParseUnwrapAll},
    };

    type SetupContext<'a, 'b> = ContextBinder<'a, KateContext<'b>, SetupService>;

    pub fn router<'a, 'b>(
        ctx: &'a mut KateContext<'b>,
        service: &'a mut SetupService,
        levels_path: &'a str,
        filters_path: &'a str,
        submit_path: &'a str,
    ) -> Router<SetupContext<'a, 'b>, ComponentInteraction, ComponentInteractionProvider, (), 3>
    {
        let provider =
            ComponentInteractionProvider::new(ctx, &[levels_path, filters_path, submit_path]);
        let ctx = SetupContext { ctx, service };
        let routes = [
            Route::new(levels_path, |ctx, event| Box::pin(set_levels(ctx, event))),
            Route::new(filters_path, |ctx, event| Box::pin(set_filters(ctx, event))),
            Route::new(submit_path, |ctx, event| Box::pin(submit(ctx, event))),
        ];

        Router::new(ctx, provider, routes)
            .matcher(|route, _ctx, event| event.data.custom_id == route.path)
    }

    #[instrument(level = "warn", skip(ctx, event))]
    async fn set_levels(
        ctx: &mut SetupContext<'_, '_>,
        event: ComponentInteraction,
    ) -> RoutingResult<()> {
        let EventKind::StringSelect { values } = &event.data.kind else {
            error!(?event.data.kind, "Expected EventKind::StringSelect");
            return RoutingResult::Continue;
        };

        ctx.service.levels = values.parse_unwrap_all();
        event
            .create_response(**ctx, CreateInteractionResponse::Acknowledge)
            .await
            .on_err_warn_send_failed()
            .ok();

        RoutingResult::Continue
    }

    #[instrument(level = "warn", skip(ctx, event))]
    async fn set_filters(
        ctx: &mut SetupContext<'_, '_>,
        event: ComponentInteraction,
    ) -> RoutingResult<()> {
        let EventKind::StringSelect { values } = &event.data.kind else {
            error!(?event.data.kind, "Expected EventKind::StringSelect");
            return RoutingResult::Continue;
        };

        ctx.service.filters = values.parse_unwrap_all();
        event
            .create_response(**ctx, CreateInteractionResponse::Acknowledge)
            .await
            .on_err_warn_send_failed()
            .ok();

        RoutingResult::Continue
    }

    #[instrument(level = "warn", skip(ctx, event))]
    async fn submit(
        ctx: &mut SetupContext<'_, '_>,
        event: ComponentInteraction,
    ) -> RoutingResult<()> {
        let EventKind::Button = &event.data.kind else {
            error!(?event.data.kind, "Expected EventKind::Button");
            return RoutingResult::Continue;
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

        // On successful game creation, tell router to stop listening.
        if success {
            event
                .delete_response(**ctx)
                .await
                .on_err_warn("Delete creating game message failed")
                .ok();

            RoutingResult::Exit(())
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

            RoutingResult::Continue
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
}
