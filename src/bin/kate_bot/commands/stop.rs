use crate::{models::net::{KateContext, KateResult}, util::LobbyId};

/// Stops the active game, if any.
#[poise::command(
    slash_command,
    user_cooldown = 3,
    name_localized("ja", "止まる"),
    description_localized("ja", "ゲームを止まる")
)]
pub async fn stop(ctx: KateContext<'_>) -> KateResult {
    let stopped = ctx.data().manager.remove_lobby(ctx.lobby_id());

    if stopped {
        ctx.say("Stopping game...").await?;
    } else {
        ctx.say("There is no active game to stop.").await?;
    }

    Ok(())
}
