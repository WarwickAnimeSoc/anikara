use crate::{
    Context, KaraConfig, MoeConfig,
    backend::networking::moe::EmitForErrors::{UnexpectedOutput, Unreachable},
};
use futures_util::FutureExt;
use rust_socketio::{
    Payload,
    asynchronous::{Client, ClientBuilder},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::fmt::Display;
use std::time::Duration;
use tokio::sync::oneshot;

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
pub struct SongInfo {
    pub kid: String,
    pub songname: String,
    pub year: u16,
    pub duration: u16,
    pub titles: TitleInfo,
    pub singers: Vec<SingerInfo>,
    pub series: Vec<SeriesInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TitleInfo {
    pub eng: Option<String>,
    pub jpn: Option<String>,
    pub chn: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SingerInfo {
    pub name: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct SeriesInfo {
    pub external_database_ids: ExternalInfo,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExternalInfo {
    anilist: u32,
    myanimelist: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SongResponse {
    pub content: Vec<SongInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClientPlay {
    plaid: String,
    requester: String,
    kids: Vec<String>,
    pos: i16,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayForm {
    body: ClientPlay,
    authorization: String,
    onlineAuthorization: String,
}

trait EmitFor {
    async fn emit_for<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        data: impl Serialize,
    ) -> Result<T, EmitForErrors>;
}

#[derive(Debug)]
/// Potentially returned errors for the `EmitFor` trait.
pub enum EmitForErrors {
    /// The SocketIO server is currently unreachable,
    /// check your server condition and location.
    Unreachable(rust_socketio::Error),
    /// The connection timeout, maybe something is taking too long to process?
    Timeout(oneshot::error::RecvError),
    /// The returned data cannot be parsed into the desired struct.
    /// It is time to reread the documentation and update your code.
    UnexpectedOutput(),
}

impl Display for EmitForErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(err) => write!(f, "SocketIOServerUnreachable({})", err),
            Self::Timeout(err) => write!(f, "SocketIOConnectionTimeout({})", err),
            Self::UnexpectedOutput() => write!(f, "SocketIOUnexpectedOutput()"),
        }
    }
}

impl std::error::Error for EmitForErrors {}

/// Separated to avoid generic templating explosion bloat.
/// Do not use directly, use `socket.emit_for` instead from `EmitFor` trait.
async fn _inner_emit_for_json(
    socket: &Client,
    endpoint: &str,
    value: Value,
) -> Result<Value, EmitForErrors> {
    let (tx, rx) = oneshot::channel::<Payload>();
    let mut tx = Some(tx);

    // lambda should only ever run once.
    // 2 second timeout should be enough for a localhost connection.
    socket
        .emit_with_ack(endpoint, value, Duration::from_secs(2), move |msg, _| {
            let tx = tx.take();
            async move {
                if let Some(tx) = tx {
                    let _ = tx.send(msg);
                }
            }
            .boxed()
        })
        .await
        .map_err(|e| EmitForErrors::Unreachable(e))?;

    let result = rx.await.map_err(|e| EmitForErrors::Timeout(e))?;

    println!("{:#?}", result);

    match result {
        Payload::Text(values) => {
            if values.len() > 1 {
                eprintln!("SocketIO response unexpectedly return multiple json")
            }
            values
                .into_iter()
                .next()
                .ok_or_else(|| EmitForErrors::UnexpectedOutput())
        }
        // TODO: make the above prettier
        Payload::Binary(_) => Err(UnexpectedOutput()), // There is no parser for this anyways.
        Payload::String(string) => {
            Ok(serde_json::to_value(string).map_err(|_| EmitForErrors::UnexpectedOutput())?)
        }
    }
}

impl EmitFor for Client {
    /// Use the SocketIO connection to connect to the specified KaraokeMugen endpoint.
    /// The provided `data` struct will be converted to json and sent.
    /// Await for the json response, which will be deserialized and returned as the target struct type.
    async fn emit_for<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        data: impl Serialize,
    ) -> Result<T, EmitForErrors> {
        let result =
            _inner_emit_for_json(self, endpoint, serde_json::to_value(data).unwrap()).await?;
        serde_json::from_value::<T>(result).map_err(|_| EmitForErrors::UnexpectedOutput())
    }
}

pub async fn auth(config: &KaraConfig) -> Result<[String; 2], EmitForErrors> {
    let url = &config.karaokemugen.moe_server;
    let socket = ClientBuilder::new(url)
        .namespace("/")
        .on("error", |err, _| {
            async move { eprintln!("Error: {:#?}", err) }.boxed()
        })
        .connect()
        .await
        .expect("Connection Failed");

    let auth = GuestLogin {
        username: "martinmimi".into(),
    };

    let auth_form = LoginForm {
        body: auth,
        authorization: String::new(),
        onlineAuthorization: String::new(),
    };

    let payload = json!(auth_form);
    let login_response: Vec<LoginResponse> = socket.emit_for("loginGuest", payload).await?;
    println!("Login response: {:?}", login_response);

    socket.disconnect().await.expect("Disconnect failed");
    println!("Server Disconnected");

    // TODO: make this prettier
    let data = login_response.into_iter().next().unwrap().data;
    Ok([data.token, data.username])
}

pub async fn server_search(
    filter: &str,
    config: &KaraConfig,
    client: &reqwest::Client,
) -> Result<SongResponse, Box<dyn std::error::Error>> {
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

    Ok(data)
}

pub async fn client_play(
    config: &KaraConfig,
    ctx: &Context,
    kid: String,
    plaid: String,
    pos: i16,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = ClientPlay {
        plaid,
        requester: ctx.username.to_owned(),
        kids: vec![kid],
        pos,
    };
    let token = &ctx.auth_token;
    println!("{:?}", token);
    let url = &config.karaokemugen.moe_server;

    let payload = json!(PlayForm {
        body: body,
        authorization: token.to_owned(),
        onlineAuthorization: String::new(),
    });

    let socket = ClientBuilder::new(url)
        .namespace("/")
        .on("error", |err, _| {
            async move { eprintln!("Error: {:#?}", err) }.boxed()
        })
        .connect()
        .await
        .expect("Connection Failed");

    let play_response: Vec<Value> = socket.emit_for("addKaraToPlaylist", payload).await?;

    println!("{:#?}", play_response);

    Ok(())
}
