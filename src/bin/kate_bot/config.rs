use std::{fs, process, sync::Arc};

use poise::{
    BoxFuture, Framework, FrameworkOptions,
    serenity_prelude::{self as serenity, Context as SerenityContext, GuildId, Ready},
};
use strum_macros::{Display, EnumString};

use crate::{command, game};

pub struct Data {
    pub manager: Arc<game::Manager>,
}
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(EnumString, Display)]
pub enum Env {
    #[strum(serialize = "dev")]
    Dev,
    #[strum(serialize = "prod")]
    Prod,
}

/// Prints missing `var` environment variable to stderr and terminates
/// with an exit code of 1.
fn report_missing_and_exit(var: &str) -> ! {
    eprintln!("Missing `{var}` environment variable.");
    process::exit(1)
}

pub fn environment() -> Env {
    let Ok(env) = std::env::var("ENV") else {
        report_missing_and_exit("ENV")
    };
    env.parse()
        .expect("Invalid value for ENV environment variable.")
}

/// Gets the Discord bot token from DISCORD_TOKEN or DISCORD_TOKEN_FILE env.
///
/// # Process Termination
/// Terminates the program if either environment variable is missing.
pub fn discord_token() -> String {
    match environment() {
        Env::Dev => std::env::var("DISCORD_TOKEN")
            .unwrap_or_else(|_| report_missing_and_exit("DISCORD_TOKEN")),
        Env::Prod => std::env::var("DISCORD_TOKEN_FILE")
            .ok()
            .and_then(|path| fs::read_to_string(path).ok())
            .unwrap_or_else(|| report_missing_and_exit("DISCORD_TOKEN_FILE")),
    }
}

/// Gets the Discord development GuildId from `DISCORD_DEV_GUILD_ID` env. Returns
/// None if not set.
pub fn discord_dev_guild_id() -> u64 {
    let Ok(id) = std::env::var("DISCORD_DEV_GUILD_ID") else {
        report_missing_and_exit("DISCORD_DEV_GUILD_ID")
    };
    id.parse()
        .expect("Invalid value for DISCORD_DEV_GUILD_ID environment variable.")
}

pub fn framework_options() -> FrameworkOptions<Data, Error> {
    poise::FrameworkOptions {
        commands: vec![command::start(), command::stop(), command::info()],
        event_handler: |_ctx, event, framework, _data| {
            Box::pin(async move {
                if let serenity::FullEvent::InteractionCreate { interaction } = event {
                    if let Some(interaction) = interaction.clone().into_message_component() {
                        framework.user_data.manager.send(interaction).await;
                    };
                };

                Ok(())
            })
        },
        on_error: |error| {
            Box::pin(async move {
                eprintln!("[{}] {error}", chrono::Utc::now());
            })
        },
        ..Default::default()
    }
}

pub fn framework_setup<'a>(
    ctx: &'a SerenityContext,
    _ready: &'a Ready,
    framework: &'a Framework<Data, Error>,
) -> BoxFuture<'a, Result<Data, Error>> {
    Box::pin(async move {
        match environment() {
            Env::Dev => {
                println!("Registering commands to DEV Guild");
                poise::builtins::register_in_guild(
                    ctx,
                    &framework.options().commands,
                    GuildId::new(discord_dev_guild_id()),
                )
                .await?;
            }
            Env::Prod => {
                println!("Registering commands globally");
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
            }
        }

        Ok(Data {
            manager: game::Manager::new(ctx.http.clone()).into(),
        })
    })
}
