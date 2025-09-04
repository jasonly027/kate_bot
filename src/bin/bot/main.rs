use dotenvy::dotenv;
use poise::{
    Framework,
    serenity_prelude::{self as serenity},
};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::models::net::{KateData, KateError};

mod commands;
mod config;
mod message;
mod models;
mod modes;
mod util;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
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
