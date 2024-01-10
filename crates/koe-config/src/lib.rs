use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub discord: DiscordConfig,
    pub voicevox: VoicevoxConfig,
    pub redis: RedisConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordConfig {
    pub client_id: u64,
    pub bot_token: String,
    #[serde(default = "default_speak_user_name")]
    pub speak_user_name: bool,
    #[serde(default = "default_speak_length_limit", deserialize_with = "deserialize_speak_length_limit")]
    pub speak_length_limit: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VoicevoxConfig {
    pub api_base: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

fn default_speak_user_name() -> bool {
    true
}

fn default_speak_length_limit() -> usize {
    60
}

fn deserialize_speak_length_limit<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let mut value = usize::deserialize(deserializer)?;
    if value < 1 {
        value = 1;
    }
    Ok(value)
}

pub async fn load() -> Result<Config> {
    let config_path = std::env::var("KOE_CONFIG").unwrap_or_else(|_| "/etc/koe.yaml".to_string());

    let yaml = tokio::fs::read_to_string(&config_path)
        .await
        .with_context(|| format!("Failed to load config file from {}", config_path))?;

    let config = serde_yaml::from_str::<Config>(&yaml).context("Failed to parse config file")?;

    Ok(config)
}
