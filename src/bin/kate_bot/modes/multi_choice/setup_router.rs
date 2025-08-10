use std::{
    ops::{Deref, DerefMut},
    process,
};

use jplearnbot::dictionary::NLevel;
use poise::{
    CreateReply,
    serenity_prelude::{
        ComponentInteraction, ComponentInteractionDataKind as EventKind, CreateActionRow,
        CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse,
    },
};
use tracing::{error, instrument};

use crate::{
    models::{
        dictionary::PosFilter,
        net::{
            ComponentInteractionProvider, KateContext, KateResult, Route, Router, RoutingResult,
        },
    },
    modes::multi_choice::setup_service::Service as SetupService,
    util::{LobbyId, Logging, ParseUnwrapAll},
};

struct SetupContext<'a, 'b> {
    ctx: &'a mut KateContext<'b>,
    service: SetupService,
}

impl<'b> Deref for SetupContext<'_, 'b> {
    type Target = KateContext<'b>;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl DerefMut for SetupContext<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ctx
    }
}

pub async fn handler(mut ctx: KateContext<'_>, service: SetupService) -> KateResult {
    let id = ctx.id();
    let levels_id = format!("{id}-lvls");
    let filters_id = format!("{id}-fltrs");
    let submit_id = format!("{id}-sbmt");

    let first_reply = CreateReply::default()
        .components(vec![
            create_levels_dropdown(&levels_id, service.levels()),
            create_filters_dropdown(&filters_id, service.filters()),
            create_submit_button(&submit_id),
        ])
        .ephemeral(true);

    let reply_handle = ctx.send(first_reply).await.on_err_warn_send_failed()?;

    router(&mut ctx, service, &levels_id, &filters_id, &submit_id)
        .listen()
        .await;

    reply_handle.delete(ctx).await.ok();

    Ok(())
}

fn router<'a, 'b>(
    ctx: &'a mut KateContext<'b>,
    service: SetupService,
    levels_path: &'a str,
    filters_path: &'a str,
    submit_path: &'a str,
) -> Router<SetupContext<'a, 'b>, ComponentInteraction, ComponentInteractionProvider, (), 3> {
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
async fn set_levels(ctx: &mut SetupContext<'_, '_>, event: ComponentInteraction) -> RoutingResult<()> {
    let EventKind::StringSelect { values } = &event.data.kind else {
        error!("Parse event data failed");
        return RoutingResult::Continue;
    };

    ctx.service.set_levels(values.parse_unwrap_all());
    event
        .create_response(**ctx, CreateInteractionResponse::Acknowledge)
        .await
        .on_err_warn_send_failed()
        .ok();

    RoutingResult::Continue
}

#[instrument(level = "warn", skip(ctx, event))]
async fn set_filters(ctx: &mut SetupContext<'_, '_>, event: ComponentInteraction) -> RoutingResult<()> {
    let EventKind::StringSelect { values } = &event.data.kind else {
        error!(?event.data.kind, "Parse event data failed");
        return RoutingResult::Continue;
    };

    ctx.service.set_filters(values.parse_unwrap_all());
    event
        .create_response(**ctx, CreateInteractionResponse::Acknowledge)
        .await
        .on_err_warn_send_failed()
        .ok();

    RoutingResult::Continue
}

#[instrument(level = "warn", skip(ctx, event))]
async fn submit(ctx: &mut SetupContext<'_, '_>, event: ComponentInteraction) -> RoutingResult<()> {
    let EventKind::Button = &event.data.kind else {
        error!("Parse event data failed");
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

    let success = ctx
        .service
        .submit(ctx.lobby_id(), ctx.channel_id(), ctx.id());

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
                EditInteractionResponse::new().content("Active game in progress. Please stop it."),
            )
            .await
            .on_err_warn("Edit creating game message failed")
            .ok();

        RoutingResult::Continue
    }
}

fn create_levels_dropdown(id: &str, levels: &[NLevel]) -> CreateActionRow {
    let options: Vec<_> = levels
        .iter()
        .map(|level| {
            let level = level.to_string();
            CreateSelectMenuOption::new(&level, &level).default_selection(true)
        })
        .collect();
    let len = options.len();

    let menu = CreateSelectMenu::new(id, CreateSelectMenuKind::String { options })
        .placeholder("Select NLevel Pool(s)")
        .min_values(1)
        .max_values(len.try_into().unwrap_or_else(|err| {
            error!(len, error = %err, "Max select value of Levels Dropdown is too high");
            process::exit(2);
        }));

    CreateActionRow::SelectMenu(menu)
}

fn create_filters_dropdown(id: &str, filters: &[PosFilter]) -> CreateActionRow {
    let options: Vec<_> = filters
        .iter()
        .map(|pos| {
            let pos = pos.to_string();
            CreateSelectMenuOption::new(&pos, &pos).default_selection(true)
        })
        .collect();
    let len = options.len();

    let menu = CreateSelectMenu::new(id, CreateSelectMenuKind::String { options })
        .placeholder("Select parts-of-speech filters")
        .min_values(1)
        .max_values(len.try_into().unwrap_or_else(|err| {
            error!(len, error = %err, "Max select value of Pos Dropdown is too high");
            process::exit(2)
        }));

    CreateActionRow::SelectMenu(menu)
}

fn create_submit_button(id: &str) -> CreateActionRow {
    let button = CreateButton::new(id).label("Create Game");
    CreateActionRow::Buttons(vec![button])
}
