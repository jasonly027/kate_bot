use crate::{config::Context, Error};

/// Stops the active game, if any.
#[poise::command(
    slash_command,
    user_cooldown = 3,
    name_localized("ja", "止まる"),
    description_localized("ja", "ゲームを止まる")
)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let session_id = ctx.guild_id().map(|g| g.get()).unwrap_or(ctx.author().id.get());
    let stopped = ctx.data().manager.stop(session_id).await;

    if stopped {
        ctx.say("Stopping game...").await?;
    } else {
        ctx.say("There is no active game to stop.").await?;
    }

    Ok(())
}
