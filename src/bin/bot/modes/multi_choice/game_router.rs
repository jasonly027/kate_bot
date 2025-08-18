use poise::serenity_prelude::{
    ComponentInteraction, ComponentInteractionDataKind as EventKind, CreateEmbed,
    CreateInteractionResponse, CreateMessage, EditMessage, UserId,
};
use tokio::sync::mpsc::Receiver;
use tracing::{error, instrument};

use crate::{
    message::{choice_buttons, prompt_embed, prompt_image, scoreboard_embed},
    models::{
        emote::{self, Insult},
        net::{GameContext, GameMessage, Provider},
        question::Question,
    },
    modes::multi_choice::{MultiChoiceMode, game_service::Service as GameService},
    util::{Logging, Retry, RetryResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoundHandleResult {
    ContinueRound,
    NextRound,
    NetError,
}

/// Manages the ongoing game.
#[instrument(level = "warn", skip(ctx, service, receiver), fields(lobby_id = ctx.lobby_id))]
pub async fn handler(
    mut ctx: GameContext,
    mut service: GameService,
    mut receiver: Receiver<GameMessage>,
) {
    let mut retry = Retry::new();

    'game: loop {
        // Get the round's question or end the game if there are no more left.
        service.next_round();
        let Some(question) = service.question() else {
            ctx.send_text("There are no more words left in the pool...")
                .await
                .on_err_warn("Send pool exhausted failed")
                .ok();
            break;
        };

        // Send the round prompt with retries.
        let msg = prompt_msg(&ctx.game_id, question, service.round(), service.mode());
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
            RetryResult::Fail => continue 'game,
            RetryResult::Terminal => {
                ctx.send_text("Stopping game due to network error...")
                    .await
                    .on_err_warn("Send network error failed")
                    .ok();
                return;
            }
        }

        // Handle round interaction
        'round: loop {
            match receiver.next().await {
                Some(GameMessage::Event(event)) => {
                    match handle_event(&mut ctx, &mut service, event).await {
                        RoundHandleResult::ContinueRound => {}
                        RoundHandleResult::NextRound => break 'round,
                        RoundHandleResult::NetError => return,
                    }
                }

                Some(GameMessage::Close) => break 'game,

                Some(GameMessage::Timeout) => {
                    ctx.send_text("Stopping game due to inactivity...")
                        .await
                        .on_err_warn("Send inactivity failed")
                        .ok();
                    break 'game;
                }

                None => unreachable!("Receiver never returns None"),
            }
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
) -> RoundHandleResult {
    let Some(choice) = parse_event(&event, service) else {
        return RoundHandleResult::ContinueRound;
    };

    event
        .create_response(&ctx.manager.http, CreateInteractionResponse::Acknowledge)
        .await
        .on_err_warn_send_failed()
        .ok();

    let correct = service.select_choice(event.user.id, choice);

    // If correct, show answer embed and move to next round.
    if correct {
        event
            .message
            .edit(
                &ctx.manager.http,
                correct_edit(&ctx.game_id, &event.user.name, service),
            )
            .await
            .on_err_warn("Update with correct answer failed")
            .ok();

        RoundHandleResult::NextRound
    // If incorrect, show insult embed and continue current round.
    } else {
        if event
            .message
            .edit(
                &ctx.manager.http,
                incorrect_edit(&ctx.game_id, event.user.id, choice, service),
            )
            .await
            .on_err_warn("Update with insult failed")
            .is_err()
        {
            ctx.send_message(
                CreateMessage::new().content("Aborting game because of network error..."),
            )
            .await
            .on_err_warn("Send abort message failed")
            .ok();
            return RoundHandleResult::NetError;
        }

        RoundHandleResult::ContinueRound
    }
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

fn prompt_msg<const N: usize>(
    game_id: &str,
    question: &Question<N>,
    round: u32,
    mode: MultiChoiceMode,
) -> CreateMessage {
    CreateMessage::new()
        .embed(prompt_embed(round, mode.into()))
        .components(choice_buttons(game_id, round, question))
}

fn correct_edit(game_id: &str, name: &str, service: &GameService) -> EditMessage {
    let round = service.round();
    let question = service.question().unwrap();
    EditMessage::new()
        .add_embed(prompt_embed(round, service.mode().into()))
        .add_embed(answer_embed(name, question))
        .components(choice_buttons(game_id, round, question))
}

fn answer_embed<const N: usize>(name: &str, question: &Question<N>) -> CreateEmbed {
    const THUMBNAIL: &str = r"https://raw.githubusercontent.com/jasonly027/kate_bot/dedaa826e9bbc942cf035ba8eeac15479e8d9416/assets/correct.png";
    let header = format!("{} {:?}", question.answer(), question.difficulty());
    let body = format!(
        "[**Definition ・ 意味**](https://jisho.org/search/{})\n{} {}",
        urlencoding::encode(question.answer()),
        name,
        emote::emote.wow
    );

    CreateEmbed::new()
        .title("Answer · 正解")
        .thumbnail(THUMBNAIL)
        .field(header, body, false)
}

fn incorrect_edit(
    game_id: &str,
    user_id: UserId,
    choice: usize,
    service: &GameService,
) -> EditMessage {
    let question = service.question().unwrap();
    let choice = &question.choices()[choice];
    EditMessage::new()
        .add_embed(prompt_embed(service.round(), service.mode().into()))
        .add_embed(insult_embed(user_id, choice))
        .components(choice_buttons(game_id, service.round(), question))
}

fn insult_embed(user_id: UserId, choice: &str) -> CreateEmbed {
    let Insult {
        message,
        thumbnail_url,
    } = emote::random_insult();

    CreateEmbed::new()
        .title("Incorrect · 間違った")
        .field(
            "\u{200B}",
            format!("{message} <@{user_id}> ({choice})\n\u{200B}"),
            false,
        )
        .thumbnail(*thumbnail_url)
}
