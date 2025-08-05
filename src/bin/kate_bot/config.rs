use std::{
    env, fs, process,
    sync::{Arc, LazyLock},
};

use poise::{
    BoxFuture, Framework, FrameworkOptions,
    serenity_prelude::{self as serenity, Context as SerenityContext, GuildId, Ready},
};
use strum_macros::{Display, EnumString};
use tracing::{error, info, warn};

use crate::{command, game};

pub struct Data {
    pub manager: Arc<game::Manager>,
}
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(EnumString, Display, Clone, Copy)]
pub enum Env {
    #[strum(serialize = "dev")]
    Dev,
    #[strum(serialize = "prod")]
    Prod,
}

/// Gets the environment from ENV env.
///
/// # Terminates
/// Terminates if variable is not set.
pub fn environment() -> Env {
    static ENV: LazyLock<Env> = LazyLock::new(|| {
        let Ok(env) = env::var("ENV") else {
            error!("ENV environment variable not set");
            process::exit(1);
        };

        env.parse().unwrap_or_else(|_| {
            error!(
                msg = "Invalid value for ENV environment variable",
                value = env
            );
            process::exit(1);
        })
    });

    *ENV
}

/// Gets the Discord bot token from DISCORD_TOKEN or DISCORD_TOKEN_FILE depending on [`Env`].
///
/// # Termination
/// Terminates if neither variable is set or the file at DISCORD_TOKEN_FILE couldn't be read.
pub fn discord_token() -> &'static str {
    static TOKEN: LazyLock<String> = LazyLock::new(|| match environment() {
        Env::Dev => env::var("DISCORD_TOKEN").unwrap_or_else(|_| {
            error!("DISCORD_TOKEN environment variable not set");
            process::exit(1);
        }),
        Env::Prod => {
            let file = env::var("DISCORD_TOKEN_FILE").unwrap_or_else(|_| {
                error!("DISCORD_TOKEN_FILE environment variable not set");
                process::exit(1);
            });

            fs::read_to_string(&file)
                .unwrap_or_else(|err| {
                    error!(message = "Failed to read from DISCORD_TOKEN_FILE", error = %err);
                    process::exit(1);
                })
                .trim()
                .to_string()
        }
    });

    &TOKEN
}

/// Gets the Discord development GuildId from DISCORD_DEV_GUILD_ID env.
///
/// # Terminates
/// Terminates if variable is not set.
pub fn discord_dev_guild_id() -> u64 {
    static ID: LazyLock<u64> = LazyLock::new(|| {
        let Ok(id) = env::var("DISCORD_DEV_GUILD_ID") else {
            error!("DISCORD_DEV_GUILD_ID environment variable not set");
            process::exit(1);
        };

        id.parse().unwrap_or_else(|_| {
            error!(
                msg = "Invalid value for DISCORD_DEV_GUILD_ID environment variable",
                value = id
            );
            process::exit(1);
        })
    });

    *ID
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
                warn!(%error);
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
                info!("Registering commands to development guild");
                poise::builtins::register_in_guild(
                    ctx,
                    &framework.options().commands,
                    GuildId::new(discord_dev_guild_id()),
                )
                .await?;
            }
            Env::Prod => {
                info!("Registering commands globally");
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
            }
        }

        Ok(Data {
            manager: Arc::new(game::Manager::new(ctx.http.clone())),
        })
    })
}
