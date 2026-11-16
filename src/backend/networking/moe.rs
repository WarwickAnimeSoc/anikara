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

pub async fn server_auth(config: &KaraConfig, client: reqwest::Client) {
    let url = &config.karaokemugen.moe_api;
    dotenv::dotenv().ok();
    let username = env::var("USERNAME")
        .map_err(|_| "USERNAME is not set")
        .unwrap();
    let password = env::var("PASSWORD")
        .map_err(|_| "PASSWORD is not set")
        .unwrap();
    let request_body = LoginInfo { username, password };

    let res = client
        .post(url.to_owned() + "/auth/login")
        .json(&request_body)
        .send()
        .await;

    match res {
        Ok(msg) => {
            println!("Received response: {:?}", msg)
        }
        Err(e) => {
            println!("Error: {:?}", e)
        }
    }
}
