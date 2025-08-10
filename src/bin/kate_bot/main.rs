use dotenvy::dotenv;
use poise::{
    Framework,
    serenity_prelude::{self as serenity},
};
use tracing::info;

use crate::models::net::{KateData, KateError};

mod config;
mod models;
mod modes;
mod commands;
mod util;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenv().ok();

    info!("Running in {} mode", config::environment());

    let token = config::discord_token();
    let intents = serenity::GatewayIntents::non_privileged();
    let framework: Framework<KateData, KateError> = poise::Framework::builder()
        .options(config::framework_options())
        .setup(config::framework_setup)
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}
