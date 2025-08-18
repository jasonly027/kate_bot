use std::sync::{Arc, OnceLock};

use poise::CreateReply;
use tracing::instrument;

use crate::{
    message::{ids, single_button, string_dropdown},
    models::net::{KateContext, KateResult},
    modes::{
        ModeChoice,
        verb_transitivity::tverbs::{TVerbPair, tverb_pairs},
    },
    util::Logging,
};

static TVERBS: OnceLock<Vec<Arc<TVerbPair>>> = OnceLock::new();

fn tverbs(ctx: &KateContext<'_>) -> &'static[Arc<TVerbPair>] {
    TVERBS.get_or_init(|| {
        tverb_pairs(&ctx.data().manager.dictionary)
            .into_iter()
            .map(Arc::new)
            .collect()
    })
}

// Sets up creating a verb transitivity game.
#[instrument(level = "warn", skip(ctx, _mode), fields(invocation_id = ctx.id()))]
pub async fn handler(mut ctx: KateContext<'_>, _mode: ModeChoice) -> KateResult {
    let mut service = service::Service::new(ctx.data().manager.clone(), tverbs(&ctx));
    let [levels_id, submit_id] = ids(ctx.id(), ["lvls", "sbmt"]);

    let first_reply = CreateReply::default()
        .components(vec![
            string_dropdown(&levels_id, &service.levels, "Select NLevel Pool(s)"),
            single_button(&submit_id, "Create Game"),
        ])
        .ephemeral(true);

    let reply_handle = ctx.send(first_reply).await.on_err_warn_send_failed()?;

    router::router(&mut ctx, &mut service, &levels_id, &submit_id)
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
            matcher,
        },
        modes::verb_transitivity::setup::service::Service as SetupService,
        util::{Logging, ParseUnwrapAll},
    };

    type SetupContext<'a, 'b, 'c> = ContextBinder<'a, KateContext<'b>, SetupService<'c>>;

    pub fn router<'a, 'kctx, 'serv>(
        ctx: &'a mut KateContext<'kctx>,
        service: &'a mut SetupService<'serv>,
        levels_path: &'a str,
        submit_path: &'a str,
    ) -> Router<
        SetupContext<'a, 'kctx, 'serv>,
        ComponentInteraction,
        ComponentInteractionProvider,
        (),
        2,
    > {
        let provider = ComponentInteractionProvider::new(ctx, &[levels_path, submit_path]);
        let ctx = SetupContext { ctx, service };
        let routes = [
            Route::new(levels_path, |ctx, event| Box::pin(set_levels(ctx, event))),
            Route::new(submit_path, |ctx, event| Box::pin(submit(ctx, event))),
        ];

        Router::new(ctx, provider, routes).matcher(matcher::full_route_path)
    }

    #[instrument(level = "warn", skip(ctx, event))]
    async fn set_levels(
        ctx: &mut SetupContext<'_, '_, '_>,
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
    async fn submit(
        ctx: &mut SetupContext<'_, '_, '_>,
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
            manager::Manager,
            net::{GameContext, KateContext},
        },
        modes::verb_transitivity::{game_router, game_service, tverbs::TVerbPair},
    };

    pub struct Service<'a> {
        manager: Arc<Manager>,
        tverbs: &'a [Arc<TVerbPair>],
        pub levels: Vec<NLevel>,
    }

    impl<'a> Service<'a> {
        pub fn new(manager: Arc<Manager>, tverbs: &'a [Arc<TVerbPair>]) -> Self {
            Self {
                manager,
                tverbs,
                levels: NLevel::iter().collect(),
            }
        }

        pub fn submit(&self, ctx: &KateContext<'_>) -> bool {
            let Some(receiver) = self.manager.create_lobby(ctx) else {
                return false;
            };

            tokio::spawn({
                let ctx = GameContext::new(ctx);
                let service = game_service::Service::new(self.tverbs, self.levels.clone());

                async move {
                    game_router::handler(ctx, service, receiver).await;
                }
            });

            true
        }
    }
}
