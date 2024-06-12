use crate::error::report_error;
use anyhow::{Context, Result};
use dashmap::DashMap;
use koe_db::redis;
use koe_speech::{speech::initialize_speakers, voicevox::VoicevoxClient};
use log::info;
use sentry::integrations::anyhow::capture_anyhow;
use serenity::{model::gateway::GatewayIntents, Client};
use songbird::SerenityInit;
use tokio::time::Duration;

mod app_state;
mod command;
mod component_interaction;
mod error;
mod event_handler;
mod message;
mod regex;
mod voice_state;

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = sentry::init(());

    run().await.map_err(|err| {
        capture_anyhow(&err);
        err
    })
}

async fn run() -> Result<()> {
    ecs_logger::init();

    let config = koe_config::load().await?;
    info!("Config loaded");

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(config.discord.bot_token, intents)
        .event_handler(event_handler::Handler)
        .application_id(config.discord.client_id)
        .register_songbird()
        .await
        .context("Failed to build serenity client")?;

    app_state::initialize(
        &client,
        app_state::AppState {
            redis_client: redis::Client::open(config.redis.url)?,
            voicevox_client: VoicevoxClient::new(config.voicevox.api_base),
            connected_guild_states: DashMap::new(),
            speak_user_name: config.discord.speak_user_name,
            speak_length_limit: config.discord.speak_length_limit,
        },
    )
    .await;

    {
        let d = client.data.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            info!("Initializing speakers...");

            let data = d.read().await;
            let state = data.get::<app_state::AppState>().unwrap();

            if let Err(err) = initialize_speakers(&state.voicevox_client).await {
                report_error(err);
            }
        });
    }

    info!("Starting client...");
    client.start().await.context("Client error occurred")?;

    Ok(())
}
