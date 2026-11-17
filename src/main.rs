pub mod backend;

use std::sync::OnceLock;

use config::{Config, File};
use serde::Deserialize;
use tokio;

#[derive(Debug, Deserialize)]
pub struct MoeConfig {
    pub moe_server: String,
    pub moe_api: String,
}

#[derive(Debug, Deserialize)]
pub struct USConfig {
    pub us_server: String,
    pub song_dir: String,
}

#[derive(Debug, Deserialize)]
pub struct KaraConfig {
    pub karaokemugen: MoeConfig,
    pub vocaluxe: USConfig,
}

#[derive(Debug, Deserialize)]
pub struct Context {
    auth_token: String,
}

fn load_config() -> Result<KaraConfig, config::ConfigError> {
    let settings = Config::builder()
        .add_source(File::with_name("config/default"))
        .build()?;

    settings.try_deserialize()
}

#[tokio::main]
async fn main() {
    let client = reqwest::Client::new();
    let ctx: OnceLock<Context> = OnceLock::new();
    let config = load_config().unwrap();

    println!("{:?}", config);
    // backend::networking::moe::auth(&config, ctx).await;
    backend::networking::moe::server_search("Cosmic Princess Kaguya", &config, &client, &ctx).await;
}
