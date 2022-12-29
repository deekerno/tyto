use std::fmt::Display;
use std::net::IpAddr;
use std::{fmt, str::FromStr};

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::Query, routing::get, Router};

use serde::{de, Deserialize, Deserializer};

#[derive(Deserialize)]
enum Event {
    Started,
    Stopped,
    Completed,
}

#[derive(Debug, PartialEq, Eq)]
struct EventParseError(String);

impl Display for EventParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Could not parse event: {}", self.0)
    }
}

impl FromStr for Event {
    type Err = EventParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Started" | "started" | "STARTED" => Ok(Self::Started),
            "Stopped" | "stopped" | "STOPPED" => Ok(Self::Stopped),
            "Completed" | "completed" | "COMPLETED" => Ok(Self::Completed),
            _ => Err(EventParseError(s.to_string())),
        }
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Completed => write!(f, "completed"),
            Self::Started => write!(f, "started"),
            Self::Stopped => write!(f, "stopped"),
        }
    }
}

#[derive(Deserialize)]
struct AnnounceRequest {
    #[serde(default, deserialize_with = "deserialize_url_encode")]
    info_hash: Vec<u8>,
    #[serde(default, deserialize_with = "deserialize_url_encode")]
    peer_id: Vec<u8>,
    port: u16,
    uploaded: u64,
    downloaded: u64,
    left: u64,
    #[serde(default, deserialize_with = "deserialize_bool")]
    compact: bool,
    #[serde(default, deserialize_with = "deserialize_bool")]
    no_peer_id: bool,
    #[serde(default, deserialize_with = "deserialize_optional_fields")]
    event: Option<Event>,
    #[serde(default, deserialize_with = "deserialize_optional_fields")]
    ip: Option<IpAddr>,
    #[serde(default, deserialize_with = "deserialize_optional_fields")]
    numwant: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_fields")]
    key: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_fields")]
    trackerid: Option<String>,
}

fn deserialize_url_encode<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let buf: &[u8] = de::Deserialize::deserialize(deserializer)?;
    let decoded = urlencoding::decode_binary(buf).into_owned();
    if decoded.len() == 20 {
        return Ok(decoded);
    } else {
        return Err(de::Error::custom(
            "URL-encoded parameters should be 20 bytes in length",
        ));
    }
}

fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    match s {
        "1" | "true" | "TRUE" => Ok(true),
        "0" | "false" | "FALSE" => Ok(false),
        _ => Err(de::Error::unknown_variant(s, &["1", "0", "true", "false"])),
    }
}

fn deserialize_optional_fields<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s).map_err(de::Error::custom).map(Some),
    }
}

enum AnnounceResponse {
    Failure {
        failure_reason: String,
    },
    Success {
        interval: u64,
        complete: u64,
        incomplete: u64,
        warning_message: Option<String>,
        min_interval: Option<u64>,
        peers: Vec<String>,
    },
}

impl Display for AnnounceResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failure { failure_reason } => {
                write!(f, "failure_reason: {}", failure_reason)
            }
            Self::Success {
                interval,
                complete,
                incomplete,
                warning_message,
                min_interval,
                peers,
            } => {
                write!(
                    f,
                    "interval: {}, complete: {}, incomplete: {}",
                    interval, complete, incomplete
                )?;

                if let Some(warning) = warning_message {
                    write!(f, ", warning_message: {}", warning)?;
                };

                if let Some(minimum) = min_interval {
                    write!(f, ", min_interval: {}", minimum)?;
                };

                write!(f, ", peers: ")?;

                for peer in peers.iter() {
                    write!(f, "{}", peer)?;
                }

                write!(f, "")
            }
        }
    }
}

async fn handle_announce(announce: Query<AnnounceRequest>) -> impl IntoResponse {
    let announce: AnnounceRequest = announce.0;
    let response = AnnounceResponse::Failure {
        failure_reason: "test".to_string(),
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain")],
        response.to_string(),
    )
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/announce", get(handle_announce))
        .route("/", get(|| async { "Hello, World!" }));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
