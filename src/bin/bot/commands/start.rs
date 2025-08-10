use crate::{
    models::net::{KateContext, KateResult},
    modes::{self, ModeChoice},
};

/// Starts a new game.
#[poise::command(
    slash_command,
    user_cooldown = 3,
    guild_cooldown = 3,
    name_localized("ja", "スタート"),
    description_localized("ja", "ゲームを始める")
)]
pub async fn start(
    ctx: KateContext<'_>,
    #[name_localized("ja", "モード")]
    #[description = "Pick a game mode"]
    #[description_localized("ja", "ゲームのモードを選んでください")]
    mode: ModeChoice,
) -> KateResult {
    match mode {
        ModeChoice::EngToHir
        | ModeChoice::HirToEng
        | ModeChoice::HirToKan
        | ModeChoice::KanToHir
        | ModeChoice::KanToEng
        | ModeChoice::EngToKan => {
            modes::multi_choice::handler(ctx, mode.try_into().unwrap()).await?
        }
    }

    Ok(())
}
