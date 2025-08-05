use dotenvy::dotenv;
use poise::{
    Framework,
    serenity_prelude::{self as serenity},
};

use crate::config::{Data, Error};

mod command;
mod config;
mod dictionary;
mod emote;
mod game;
mod image;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenv().ok();

    println!(
        "Running in {} mode",
        config::environment().to_string().to_uppercase()
    );

    let token = config::discord_token();
    let intents = serenity::GatewayIntents::non_privileged();
    let framework: Framework<Data, Error> = poise::Framework::builder()
        .options(config::framework_options())
        .setup(config::framework_setup)
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}
