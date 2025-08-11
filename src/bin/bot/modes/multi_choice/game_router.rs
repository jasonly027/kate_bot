use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Duration,
};

use poise::serenity_prelude::{
    ChannelId, CreateActionRow, CreateAttachment, CreateButton, CreateEmbed,
    CreateInteractionResponse, CreateMessage, EditMessage, Error as SerenityError, Message, UserId,
};
use tokio::{sync::mpsc::Receiver, time::timeout};
use tracing::{debug, error, instrument, warn};

use crate::{
    models::{
        emote::{self, Insult},
        manager::Manager,
        net::{GameMessage, Provider, Route, Router, RoutingResult},
        question::Question,
    },
    modes::multi_choice::{MultiChoiceMode, game_service::Service as GameService},
    util::{self, GameId, Logging},
};

enum RoundExitReason {
    NextRound,
    CloseGame,
    NetError,
}

#[derive(Debug)]
pub struct GameContext {
    pub game_id: String,
    pub lobby_id: u64,
    pub channel_id: ChannelId,
    pub manager: Arc<Manager>,
}

impl GameContext {
    async fn send_message(&self, message: CreateMessage) -> Result<Message, SerenityError> {
        self.channel_id
            .send_message(&self.manager.http, message)
            .await
    }

    async fn send_files(
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

struct RoundContext<'a> {
    ctx: &'a mut GameContext,
    service: &'a mut GameService,
}

impl Deref for RoundContext<'_> {
    type Target = GameContext;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl DerefMut for RoundContext<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ctx
    }
}

impl<T> Provider<GameMessage> for T
where
    T: DerefMut<Target = Receiver<GameMessage>>,
{
    /// # Warning
    /// This **always** return Some(_) and **never** None. When the sender side has been closed
    /// or times out, Some(GameMessage::Timeout) is returned.
    async fn next(&mut self) -> Option<GameMessage> {
        const MAX_TIMEOUT: Duration = Duration::from_secs(120);

        let Ok(msg) = timeout(MAX_TIMEOUT, self.recv()).await else {
            return Some(GameMessage::Timeout);
        };
        Some(msg.unwrap_or(GameMessage::Close))
    }
}

/// Manages the ongoing game.
#[instrument(level = "warn", skip(ctx, service, receiver), fields(lobby_id = ctx.lobby_id))]
pub async fn handler(
    mut ctx: GameContext,
    mut service: GameService,
    mut receiver: Receiver<GameMessage>,
) {
    const MAX_RETRIES: i32 = 3;
    let mut retries = 0;

    while service.next_round() {
        let question = service.question().unwrap();
        let msg = create_initial_msg(&ctx.game_id, question, service.round(), service.mode());
        let files = create_files(question.prompt());

        // The game is aborted if sending the round prompt fails
        // three times consecutively.
        if let Err(error) = ctx.send_files(msg, files).await {
            warn!(%error, "Send round prompt failed");
            if retries >= MAX_RETRIES {
                ctx.send_message(
                    CreateMessage::new().content("Stopping game due to network error..."),
                )
                .await
                .on_err_warn_send_failed()
                .ok();
                return;
            }
            retries += 1;

            continue;
        } else {
            retries = 0;
        }

        // Listen and handle interactions on the game's choice buttons.
        let exit = router(&mut ctx, &mut service, &mut receiver).listen().await;

        match exit {
            Some(RoundExitReason::NextRound) => {}
            Some(RoundExitReason::CloseGame) => return,
            Some(RoundExitReason::NetError) => return,
            None => return,
        }
    }

    ctx.send_message(CreateMessage::new().content("There are no more words left in the pool..."))
        .await
        .inspect_err(|error| warn!(%error, "Send pool exhausted failed"))
        .ok();
}

fn router<'a>(
    ctx: &'a mut GameContext,
    service: &'a mut GameService,
    receiver: &'a mut Receiver<GameMessage>,
) -> Router<RoundContext<'a>, GameMessage, &'a mut Receiver<GameMessage>, RoundExitReason, 3> {
    let ctx = RoundContext { ctx, service };
    let routes = [
        Route::new("event", |ctx, event| Box::pin(gm_event(ctx, event)))
            .matcher(|_, _, event| matches!(event, GameMessage::Event(_))),
        Route::new("close", |ctx, event| Box::pin(gm_close(ctx, event)))
            .matcher(|_, _, event| matches!(event, GameMessage::Close)),
        Route::new("timeout", |ctx, event| Box::pin(gm_timeout(ctx, event)))
            .matcher(|_, _, event| matches!(event, GameMessage::Timeout)),
    ];

    Router::new(ctx, receiver, routes).validator(|ctx, event| {
        // Filter out stray messages not intended for this game.
        if let GameMessage::Event(event) = event {
            event.game_id() == ctx.game_id
        } else {
            true
        }
    })
}

#[instrument(level = "warn", skip(ctx, event))]
async fn gm_event(
    ctx: &mut RoundContext<'_>,
    event: GameMessage,
) -> RoutingResult<RoundExitReason> {
    let GameMessage::Event(mut event) = event else {
        error!("Parse event data failed");
        return RoutingResult::Continue;
    };
    let Some(choice) = parse_event(&event.data.custom_id, ctx) else {
        debug!(event.data.custom_id, "Parse event.data.custom_id failed");
        return RoutingResult::Continue;
    };

    event
        .create_response(&ctx.manager.http, CreateInteractionResponse::Acknowledge)
        .await
        .on_err_warn_send_failed()
        .ok();

    let correct = ctx.service.select_choice(choice);

    // If the choice was correct show answer embed and move to next round.
    // If the choice was incorrect show insult embed and continue current round.
    if correct {
        event
            .message
            .edit(
                &ctx.manager.http,
                create_correct_edit(&ctx.game_id, &event.user.name, ctx.service),
            )
            .await
            .on_err_warn("Update with correct answer failed")
            .ok();

        RoutingResult::Exit(RoundExitReason::NextRound)
    } else {
        if event
            .message
            .edit(
                &ctx.manager.http,
                create_incorrect_edit(&ctx.game_id, event.user.id, choice, ctx.service),
            )
            .await
            .on_err_warn("Update with insult failed")
            .is_err()
        {
            return RoutingResult::Exit(RoundExitReason::NetError);
        }

        RoutingResult::Continue
    }
}

#[instrument(level = "warn", skip(_ctx, _event))]
async fn gm_close(
    _ctx: &mut RoundContext<'_>,
    _event: GameMessage,
) -> RoutingResult<RoundExitReason> {
    RoutingResult::Exit(RoundExitReason::CloseGame)
}

#[instrument(level = "warn", skip(ctx, _event))]
async fn gm_timeout(
    ctx: &mut RoundContext<'_>,
    _event: GameMessage,
) -> RoutingResult<RoundExitReason> {
    ctx.send_message(CreateMessage::new().content("Stopping game due to inactivity..."))
        .await
        .on_err_warn_send_failed()
        .ok();

    RoutingResult::Exit(RoundExitReason::CloseGame)
}

fn parse_event(payload: &str, ctx: &RoundContext) -> Option<usize> {
    let fields: Vec<&str> = payload.split(",").collect();

    let round: u32 = fields.get(1)?.parse().ok()?;
    let choice: usize = fields.get(2)?.parse().ok()?;

    if round != ctx.service.round() || choice >= ctx.service.question().unwrap().choices().len() {
        return None;
    }

    Some(choice)
}

fn create_initial_msg<const N: usize>(
    game_id: &str,
    question: &Question<N>,
    round: u32,
    mode: MultiChoiceMode,
) -> CreateMessage {
    CreateMessage::new()
        .embed(create_prompt_embed(round, mode))
        .components(create_choice_btns(game_id, round, question))
}

fn create_prompt_embed(round: u32, mode: MultiChoiceMode) -> CreateEmbed {
    CreateEmbed::new()
        .title(format!("Question {round}"))
        .field(mode.to_string(), "", false)
        .attachment("prompt.png")
}

fn create_choice_btns<const N: usize>(
    game_id: &str,
    round: u32,
    question: &Question<N>,
) -> Vec<CreateActionRow> {
    let buttons = question
        .choices()
        .iter()
        .zip(question.guessed())
        .enumerate()
        .map(|(idx, (label, &guessed))| {
            let id = format!("{game_id},{round},{idx}");
            CreateButton::new(id).label(label).disabled(guessed)
        })
        .collect();
    vec![CreateActionRow::Buttons(buttons)]
}

fn create_files(text: &str) -> Vec<CreateAttachment> {
    vec![CreateAttachment::bytes(
        util::text_to_image(text),
        "prompt.png",
    )]
}

fn create_correct_edit(game_id: &str, name: &str, service: &GameService) -> EditMessage {
    let round = service.round();
    let question = service.question().unwrap();
    EditMessage::new()
        .add_embed(create_prompt_embed(round, service.mode()))
        .add_embed(create_answer_embed(name, question))
        .components(create_choice_btns(game_id, round, question))
}

fn create_answer_embed<const N: usize>(name: &str, question: &Question<N>) -> CreateEmbed {
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

fn create_incorrect_edit(
    game_id: &str,
    user_id: UserId,
    choice: usize,
    service: &GameService,
) -> EditMessage {
    let question = service.question().unwrap();
    let choice = &question.choices()[choice];
    EditMessage::new()
        .add_embed(create_prompt_embed(service.round(), service.mode()))
        .add_embed(create_insult_embed(user_id, choice))
        .components(create_choice_btns(game_id, service.round(), question))
}

fn create_insult_embed(user_id: UserId, choice: &str) -> CreateEmbed {
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
