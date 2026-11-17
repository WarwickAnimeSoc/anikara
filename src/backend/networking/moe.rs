use crate::{Context, KaraConfig};
use dotenv;
use futures_util::{FutureExt, future::BoxFuture};
use rust_socketio::{
    Payload,
    asynchronous::{Client, ClientBuilder},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::{str::FromStr, sync::OnceLock, time::Duration};

#[derive(Debug, Serialize)]
struct LoginInfo {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct GuestLogin {
    username: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize)]
struct LoginForm {
    body: GuestLogin,
    authorization: String,
    onlineAuthorization: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
struct LoginResponseBody {
    role: String,
    token: String,
    username: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
struct LoginResponse {
    data: LoginResponseBody,
    err: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct SongInfo {
    kid: String,
    songname: String,
    year: u16,
    duration: u16,
    titles: TitleInfo,
    singers: Vec<SingerInfo>,
    series: Vec<SeriesInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TitleInfo {
    eng: Option<String>,
    jpn: Option<String>,
    chn: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SingerInfo {
    name: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
struct SeriesInfo {
    external_database_ids: ExternalInfo,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExternalInfo {
    anilist: u32,
    myanimelist: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct SongResponse {
    content: Vec<SongInfo>,
}

pub async fn auth(config: &KaraConfig, ctx: OnceLock<Context>) {
    let ack_callback = |message: Payload, _: Client| {
        match message {
            Payload::Text(mut values) => {
                if let Some(first_value) = values.pop() {
                    match serde_json::from_value::<LoginResponse>(first_value) {
                        Ok(msg) => println!("Parsed Struct: {:?}", msg),
                        Err(e) => println!("Error: {:?}", e),
                    }
                }
            }
            Payload::Binary(bytes) => {
                println!("Received binary payload: {:?}", bytes);
            }

            #[allow(deprecated)]
            Payload::String(str_data) => {
                if let Ok(msg) = serde_json::from_str::<LoginResponse>(&str_data) {
                    println!("Parsed legacy string: {:?}", msg);
                }
            }
        }
        async move {
            println!("Yehaa! My ack got acked?");
        }
        .boxed()
    };

    let url = &config.karaokemugen.moe_server;
    let auth = GuestLogin {
        username: String::from_str("martinmimi").unwrap(),
    };

    let auth_form = LoginForm {
        body: auth,
        authorization: String::new(),
        onlineAuthorization: String::new(),
    };

    println!("Authing at: {}", url);
    let socket = ClientBuilder::new(url)
        .namespace("/")
        .on("error", |err, _| {
            async move { eprintln!("Error: {:#?}", err) }.boxed()
        })
        .connect()
        .await
        .expect("Connection Failed");

    let payload = json!(auth_form);

    socket
        .emit_with_ack("loginGuest", payload, Duration::from_secs(2), ack_callback)
        .await
        .expect("Server unreachable");

    tokio::time::sleep(Duration::from_secs(10)).await;

    socket.disconnect().await.expect("Disconnect failed");
    println!("Server Disconnected")
}

pub async fn server_search(
    filter: &str,
    config: &KaraConfig,
    client: &reqwest::Client,
    ctx: &OnceLock<Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = &config.karaokemugen.moe_api;
    let query_params = [("filter", filter), ("order", "played")];
    let size_params = [("size", 3)];
    let res = client
        .get(url.to_owned() + "/karas/search")
        .query(&query_params)
        .query(&size_params)
        .send()
        .await?;

    let data: SongResponse = res.json().await?;
    println!("Received: {:#?}", data);
    Ok(())
}
