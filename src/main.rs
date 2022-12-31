use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::net::IpAddr;
use std::sync::Arc;
use std::{fmt, str::FromStr};

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::Query, routing::get, Router};

use rand::seq::SliceRandom;
use serde::{de, Deserialize, Deserializer};
use tokio::sync::RwLock;

type InfoHash = Vec<u8>;
type PeerId = Vec<u8>;

#[derive(Hash, Eq, PartialEq, Clone)]
struct Peer {
    id: PeerId,
    ip: IpAddr,
    port: u16,
}

#[derive(Clone)]
struct Swarm {
    seeders: HashSet<Peer>,
    leechers: HashSet<Peer>,
}

impl Swarm {
    fn new() -> Swarm {
        Swarm {
            seeders: HashSet::new(),
            leechers: HashSet::new(),
        }
    }

    fn add_seeder(&mut self, peer: Peer) {
        self.seeders.insert(peer);
    }

    fn add_leecher(&mut self, peer: Peer) {
        self.leechers.insert(peer);
    }

    fn remove_seeder(&mut self, peer: Peer) {
        self.seeders.remove(&peer);
    }

    fn remove_leecher(&mut self, peer: Peer) {
        self.leechers.remove(&peer);
    }

    fn promote_leecher(&mut self, peer: Peer) {
        match self.leechers.take(&peer) {
            Some(leecher) => self.seeders.insert(leecher),
            None => self.seeders.insert(peer),
        };
    }
}

#[derive(Clone)]
struct SwarmStore(Arc<RwLock<HashMap<InfoHash, Swarm>>>);

impl SwarmStore {
    fn new() -> SwarmStore {
        SwarmStore(Arc::new(RwLock::new(HashMap::new())))
    }

    async fn add_peer(&mut self, info_hash: InfoHash, peer: Peer, is_download_complete: bool) {
        let mut write_locked_store = self.0.write().await;
        match write_locked_store.get_mut(&info_hash) {
            Some(swarm) => {
                if is_download_complete {
                    swarm.add_seeder(peer);
                } else {
                    swarm.add_leecher(peer);
                }
            }
            None => {
                let mut swarm = Swarm::new();
                if is_download_complete {
                    swarm.add_seeder(peer);
                } else {
                    swarm.add_leecher(peer);
                }
                write_locked_store.insert(info_hash, swarm);
            }
        }
    }

    async fn remove_peer(&mut self, info_hash: InfoHash, peer: Peer) {
        let mut write_locked_store = self.0.write().await;
        if let Some(swarm) = write_locked_store.get_mut(&info_hash) {
            swarm.remove_seeder(peer.clone());
            swarm.remove_leecher(peer);
        }
    }

    async fn promote_peer(&mut self, info_hash: InfoHash, peer: Peer) {
        let mut write_locked_store = self.0.write().await;
        if let Some(swarm) = write_locked_store.get_mut(&info_hash) {
            swarm.promote_leecher(peer)
        }
    }

    async fn get_peers(&self, info_hash: InfoHash, numwant: usize) -> (Vec<Peer>, Vec<Peer>) {
        let (mut peers, mut peers6): (Vec<Peer>, Vec<Peer>) = (Vec::new(), Vec::new());
        let read_locked_store = self.0.read().await;
        if let Some(swarm) = read_locked_store.get(&info_hash) {
            for peer in swarm.seeders.clone().into_iter() {
                if peer.ip.is_ipv4() {
                    peers.push(peer);
                } else {
                    peers6.push(peer);
                }
            }

            for peer in swarm.leechers.clone().into_iter() {
                if peer.ip.is_ipv4() {
                    peers.push(peer);
                } else {
                    peers6.push(peer);
                }
            }

            if (swarm.seeders.len() + swarm.leechers.len()) > numwant {
                let mut rng = rand::thread_rng();
                peers.shuffle(&mut rng);
                peers6.shuffle(&mut rng);
                peers.truncate(numwant);
                peers6.truncate(numwant);
            }
        }

        (peers, peers6)
    }

    async fn get_announce_stats(&self, info_hash: InfoHash) -> (usize, usize) {
        let read_locked_store = self.0.read().await;
        if let Some(swarm) = read_locked_store.get(&info_hash) {
            return (swarm.seeders.len(), swarm.leechers.len());
        } else {
            return (0, 0);
        }
    }
}

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
    numwant: Option<usize>,
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
