pub mod backend;

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
    pub auth_token: String,
}

impl Context {
    fn new() -> Context {
        Context {
            auth_token: String::new(),
        }
    }
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
    let mut ctx: Context = Context::new();
    let config = load_config().unwrap();

    println!("{:?}", config);
    let auth_res = backend::networking::moe::auth(&config).await;
    println!("{:#?}", auth_res);
    match auth_res {
        Ok(token) => {
            ctx.auth_token = token;
        }
        Err(e) => eprintln!("Error: {:?}", e),
    }
    let search_res =
        backend::networking::moe::server_search("Cosmic Princess Kaguya", &config, &client)
            .await
            .unwrap()
            .content;

    println!("{:#?}", search_res)
}
