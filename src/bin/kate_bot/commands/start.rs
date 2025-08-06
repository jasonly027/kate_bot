use std::time::Duration;

use crate::{
    config::KateContext, modes::{self, multi_choice::PosFilter, ModeChoice}, KateError
};
use jplearnbot::dictionary::NLevel;
use poise::serenity_prelude::{
    ComponentInteractionCollector, ComponentInteractionDataKind, CreateActionRow, CreateButton,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu,
    CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse, futures::StreamExt,
};
use strum::IntoEnumIterator;

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
) -> Result<(), KateError> {
    // match mode {
    //     ModeChoice::EngToHir
    //     | ModeChoice::HirToEng
    //     | ModeChoice::HirToKan
    //     | ModeChoice::KanToHir
    //     | ModeChoice::KanToEng
    //     | ModeChoice::EngToKan => modes::multi_choice::handle(ctx).await?,
    // }

    let mut menu = FiltersMenu::new(&ctx, ctx.id(), mode);

    ctx.send(
        poise::CreateReply::default()
            .components(menu.create_components())
            .ephemeral(true),
    )
    .await?;

    menu.handle_interactions().await?;

    Ok(())
}

/// Manages the components of the create game form.
struct FiltersMenu<'a> {
    ctx: &'a KateContext<'a>,
    /// Identifier for the NLevel filter menu.
    nlvls_id: String,
    /// Currently selected NLevels. Initially all of them.
    levels: Vec<NLevel>,

    /// Identifier for the parts of speech filter menu.
    pos_id: String,
    /// Currently selected parts of speech. Initially all of them.
    pos: Vec<PosFilter>,

    /// Identifier for the submit button.
    submit_id: String,

    /// Mode of game to create.
    mode: ModeChoice,
}

impl<'a> FiltersMenu<'a> {
    fn new(ctx: &'a KateContext<'_>, invocation_id: u64, mode: ModeChoice) -> Self {
        let id = invocation_id.to_string();
        FiltersMenu {
            ctx,
            nlvls_id: format!("{}-nlvls", id),
            levels: NLevel::iter().collect(),

            pos_id: format!("{}-pos", id),
            pos: PosFilter::iter().collect(),

            submit_id: format!("{}-submit", id),

            mode,
        }
    }

    /// Create all of the components of this menu.
    fn create_components(&self) -> Vec<CreateActionRow> {
        vec![self.levels_menu(), self.pos_menu(), self.submit_button()]
    }

    /// Creates a new menu for selecting NLevels. Used by [`Self::create_components`].
    fn levels_menu(&self) -> CreateActionRow {
        let levels = self
            .levels
            .iter()
            .map(|lvl| {
                CreateSelectMenuOption::new(lvl.to_string(), lvl.to_string())
                    .default_selection(true)
            })
            .collect::<Vec<_>>();
        let levels_len = levels.len();

        let menu = CreateSelectMenu::new(
            &self.nlvls_id,
            CreateSelectMenuKind::String { options: levels },
        )
        .placeholder("Select NLevel Pool(s)")
        .min_values(1)
        .max_values(levels_len.try_into().expect("Too many options were added"));

        CreateActionRow::SelectMenu(menu)
    }

    /// Creates a new menu for selecting parts of speech. Used by [`Self::create_components`].
    fn pos_menu(&self) -> CreateActionRow {
        let pos = self
            .pos
            .iter()
            .map(|p| {
                CreateSelectMenuOption::new(p.to_string(), p.to_string()).default_selection(true)
            })
            .collect::<Vec<_>>();
        let pos_len = pos.len();

        let menu =
            CreateSelectMenu::new(&self.pos_id, CreateSelectMenuKind::String { options: pos })
                .placeholder("Select parts-of-speech filters")
                .min_values(1)
                .max_values(pos_len.try_into().expect("Too many options were added"));

        CreateActionRow::SelectMenu(menu)
    }

    /// Creates a new submit button. Used by [`Self::create_components`].
    fn submit_button(&self) -> CreateActionRow {
        let button = CreateButton::new(&self.submit_id).label("Create Game");
        CreateActionRow::Buttons(vec![button])
    }

    /// Listens for form interactions. Starts a game on submission. Does nothing
    /// on subsequent submissions.
    async fn handle_interactions(&mut self) -> Result<(), KateError> {
        let mut submitted = false;

        let mut collector = ComponentInteractionCollector::new(self.ctx)
            .author_id(self.ctx.author().id)
            .channel_id(self.ctx.channel_id())
            .timeout(Duration::from_secs(60))
            .filter({
                // Only listen for this form's components.
                let ids = [
                    self.nlvls_id.clone(),
                    self.pos_id.clone(),
                    self.submit_id.clone(),
                ];
                move |ci| ids.contains(&ci.data.custom_id)
            })
            .stream();

        while let Some(ci) = collector.next().await {
            let id = &ci.data.custom_id;

            match &ci.data.kind {
                // Update filters
                ComponentInteractionDataKind::StringSelect { values } => {
                    if id != &self.nlvls_id && id != &self.pos_id {
                        continue;
                    }

                    if id == &self.nlvls_id {
                        self.levels = values.iter().map(|v| v.parse().unwrap()).collect();
                    } else if id == &self.pos_id {
                        self.pos = values.iter().map(|v| v.parse().unwrap()).collect();
                    }
                    ci.create_response(self.ctx, CreateInteractionResponse::Acknowledge)
                        .await?;
                }
                // Handle submit
                ComponentInteractionDataKind::Button => {
                    if id != &self.submit_id {
                        continue;
                    }

                    // Ignore subsequent submissions
                    if submitted {
                        ci.create_response(
                            self.ctx,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Game has already been created")
                                    .ephemeral(true),
                            ),
                        )
                        .await?;
                        continue;
                    }
                    submitted = true;

                    ci.create_response(
                        self.ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("Creating game...")
                                .ephemeral(true),
                        ),
                    )
                    .await?;

                    // Try start game
                    if self
                        .ctx
                        .data()
                        .manager
                        .start_game(self.ctx, self.mode, self.levels.clone(), self.pos.clone())
                        .is_err()
                    {
                        ci.edit_response(
                            self.ctx,
                            EditInteractionResponse::new()
                                .content("Active game in progress. Please stop it."),
                        )
                        .await?;
                    } else {
                        ci.delete_response(self.ctx).await?;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}
