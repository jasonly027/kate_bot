use poise::serenity_prelude::{
    ComponentInteraction, ComponentInteractionDataKind as EventKind, CreateEmbed,
    CreateInteractionResponse, CreateMessage, EditMessage,
};
use tokio::sync::mpsc::Receiver;
use tracing::{error, instrument};

use crate::{
    message::{choice_buttons, prompt_embed, prompt_image, scoreboard_embed},
    models::{
        emote,
        net::{GameContext, GameMessage, Provider},
        question::Question,
    },
    modes::{ModeChoice, verb_transitivity::game_service::Service as GameService},
    util::{Logging, Retry, RetryResult},
};

#[instrument(level = "warn", skip(ctx, service, receiver), fields(lobby_id = ctx.lobby_id))]
pub async fn handler(
    mut ctx: GameContext,
    mut service: GameService,
    mut receiver: Receiver<GameMessage>,
) {
    let mut retry = Retry::new();

    loop {
        // Get the round's question or end the game if there are no more left.
        service.next_round();
        let Some(question) = service.question() else {
            ctx.send_text("All unique words expended.")
                .await
                .on_err_warn("Send pool exhausted failed")
                .ok();
            break;
        };

        // Send the round prompt with retries.
        let msg = prompt_msg(&ctx.game_id, question, service.round(), ModeChoice::VerbT);
        let file = prompt_image(question.prompt());
        match retry
            .try_async(async || {
                ctx.send_files(msg, file)
                    .await
                    .on_err_warn("Send round prompt failed")
                    .is_ok()
            })
            .await
        {
            RetryResult::Success => {}
            RetryResult::Fail => continue,
            RetryResult::Terminal => {
                ctx.send_text("Stopping game due to network error...")
                    .await
                    .on_err_warn("Send network error failed")
                    .ok();
                return;
            }
        }

        // Handle round interaction
        match receiver.next().await {
            Some(GameMessage::Event(event)) => handle_event(&mut ctx, &mut service, event).await,
            Some(GameMessage::Close) => break,
            Some(GameMessage::Timeout) => {
                ctx.send_text("Stopping game due to inactivity...")
                    .await
                    .on_err_warn("Send inactivity failed")
                    .ok();
                break;
            }
            None => unreachable!("Receiver never returns none"),
        }
    }

    // Show scoreboard
    if service.scoreboard().has_players() {
        ctx.send_message(CreateMessage::new().add_embed(scoreboard_embed(service.scoreboard())))
            .await
            .on_err_warn("Send scoreboard failed")
            .ok();
    }
}

#[instrument(level = "warn", skip(ctx, service, event))]
async fn handle_event(
    ctx: &mut GameContext,
    service: &mut GameService,
    mut event: ComponentInteraction,
) {
    let Some(choice) = parse_event(&event, service) else {
        return;
    };

    let correct = service.select_choice(event.user.id, choice);

    event
        .create_response(&ctx.manager.http, CreateInteractionResponse::Acknowledge)
        .await
        .on_err_warn_send_failed()
        .ok();
    event
        .message
        .edit(
            &ctx.manager.http,
            prompt_edit(&ctx.game_id, service, &event.user.name, correct),
        )
        .await
        .on_err_warn("Send prompt edit failed")
        .ok();
}

fn parse_event(event: &ComponentInteraction, service: &GameService) -> Option<usize> {
    let EventKind::Button = &event.data.kind else {
        error!(actual = ?event.data.kind, "Expected EventKind::Button");
        return None;
    };

    let fields: Vec<&str> = event.data.custom_id.split(",").collect();

    let round: u32 = fields
        .get(1)?
        .parse()
        .on_err_warn("Parse round failed")
        .ok()?;
    let choice: usize = fields
        .get(2)?
        .parse()
        .on_err_warn("Parse choice failed")
        .ok()?;

    if round != service.round() || choice >= service.question().unwrap().choices().len() {
        return None;
    }

    Some(choice)
}

fn prompt_msg(
    game_id: &str,
    question: &Question<2>,
    round: u32,
    mode: ModeChoice,
) -> CreateMessage {
    CreateMessage::new()
        .embed(prompt_embed(round, mode))
        .components(choice_buttons(game_id, round, question))
}

fn prompt_edit(game_id: &str, service: &GameService, name: &str, correct: bool) -> EditMessage {
    let round = service.round();
    let question = service.question().unwrap();
    EditMessage::new()
        .add_embed(prompt_embed(round, ModeChoice::VerbT))
        .add_embed(answer_embed(name, question, correct))
        .components(choice_buttons(game_id, round, question))
}

fn answer_embed<const N: usize>(name: &str, question: &Question<N>, correct: bool) -> CreateEmbed {
    let thumbnail = if correct {
        emote::THUMBNAIL.correct
    } else {
        emote::random_insult().thumbnail_url
    };
    let header = format!("{} {:?}", question.answer(), question.difficulty());
    let body = format!(
        "[**Definition ・ 意味**](https://jisho.org/search/{})\n{} {}",
        urlencoding::encode(question.answer()),
        name,
        emote::emote.wow
    );

    CreateEmbed::new()
        .title("Answer · 正解")
        .thumbnail(thumbnail)
        .field(header, body, false)
}
