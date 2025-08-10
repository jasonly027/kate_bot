use crate::models::net::{KateContext, KateResult};
use strum_macros::Display;
use tracing::instrument;

mod game_router;
mod game_service;
mod setup_router;
mod setup_service;

#[derive(Debug, Clone, Copy, Display)]
pub enum MultiChoiceMode {
    #[strum(serialize = "English ▶ ひらがな")]
    EngToHir,
    #[strum(serialize = "ひらがな ▶ English")]
    HirToEng,
    #[strum(serialize = "ひらがな ▶ 漢字")]
    HirToKan,
    #[strum(serialize = "漢字 ▶ ひらがな")]
    KanToHir,
    #[strum(serialize = "漢字 ▶ English")]
    KanToEng,
    #[strum(serialize = "English ▶ 漢字")]
    EngToKan,
}

#[instrument(level = "warn", skip(ctx, mode), fields(invocation_id = ctx.id()))]
pub async fn router(ctx: KateContext<'_>, mode: MultiChoiceMode) -> KateResult {
    let service = setup_service::Service::new(ctx.data().manager.clone(), mode);
    setup_router::handler(ctx, service).await
}
