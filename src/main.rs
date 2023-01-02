use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::{fmt, str::FromStr};

use axum::extract::{ConnectInfo, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::Query, routing::get, Router};

use bendy::encoding::{Error, SingleItemEncoder, ToBencode};
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

impl ToBencode for Peer {
    const MAX_DEPTH: usize = 1;

    fn encode(&self, encoder: SingleItemEncoder) -> Result<(), Error> {
        encoder.emit_dict(|mut e| {
            e.emit_pair(b"ip", self.ip.to_string())?;
            e.emit_pair(b"peer_id", self.id.clone())?;
            e.emit_pair(b"port", self.port)?;
            Ok(())
        })?;

        Ok(())
    }
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

    async fn get_stats_for_scrapes(&self, info_hashes: Vec<InfoHash>) -> Vec<(usize, usize)> {
        let read_locked_store = self.0.read().await;
        let stats = info_hashes
            .iter()
            .map(|info_hash| {
                if let Some(swarm) = read_locked_store.get(info_hash) {
                    return (swarm.seeders.len(), swarm.leechers.len());
                } else {
                    return (0, 0);
                }
            })
            .collect();
        stats
    }

    async fn get_global_scrape_stats(&self) -> HashMap<InfoHash, (usize, usize)> {
        let read_locked_store = self.0.read().await;
        let mut new_map = HashMap::new();

        for (info_hash, swarm) in read_locked_store.iter() {
            new_map.insert(
                info_hash.clone(),
                (swarm.seeders.len(), swarm.leechers.len()),
            );
        }

        new_map
    }
}

#[derive(Deserialize)]
struct Torrent {
    info_hash: InfoHash,
    downloaded: usize,
}

#[derive(Clone)]
struct TorrentStore(Arc<RwLock<HashMap<InfoHash, Torrent>>>);

impl TorrentStore {
    fn new() -> TorrentStore {
        TorrentStore(Arc::new(RwLock::new(HashMap::new())))
    }

    async fn add_torrent(&mut self, info_hash: InfoHash) {
        let mut write_locked_store = self.0.write().await;
        write_locked_store.insert(
            info_hash.clone(),
            Torrent {
                info_hash,
                downloaded: 0,
            },
        );
    }

    async fn increment_downloaded(&mut self, info_hash: InfoHash) {
        let mut write_locked_store = self.0.write().await;
        if let Some(torrent) = write_locked_store.get_mut(&info_hash) {
            torrent.downloaded += 1;
        }
    }

    async fn get_stats_for_scrapes(&self, info_hashes: Vec<InfoHash>) -> Vec<usize> {
        let read_locked_store = self.0.read().await;
        let stats = info_hashes
            .iter()
            .map(|info_hash| {
                if let Some(torrent) = read_locked_store.get(info_hash) {
                    return torrent.downloaded;
                } else {
                    return 0;
                }
            })
            .collect();
        stats
    }

    async fn get_global_scrape_stats(&self) -> HashMap<InfoHash, usize> {
        let read_locked_store = self.0.read().await;
        let mut new_map = HashMap::new();

        for (info_hash, torrent) in read_locked_store.iter() {
            new_map.insert(info_hash.clone(), torrent.downloaded);
        }

        new_map
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

#[derive(Debug)]
struct ScrapeRequest {
    info_hashes: Option<Vec<InfoHash>>,
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
        complete: usize,
        incomplete: usize,
        peers: Vec<Peer>,
        peers6: Vec<Peer>,
        tracker_id: String,
        warning_message: Option<String>,
        min_interval: Option<u64>,
    },
}

impl ToBencode for AnnounceResponse {
    const MAX_DEPTH: usize = 5;
    fn encode(&self, encoder: SingleItemEncoder) -> Result<(), Error> {
        match self {
            Self::Failure { failure_reason } => {
                encoder.emit_dict(|mut e| {
                    e.emit_pair(b"failure_reason", failure_reason)?;
                    Ok(())
                })?;
            }
            Self::Success {
                interval,
                complete,
                incomplete,
                tracker_id,
                peers,
                peers6,
                warning_message,
                min_interval,
            } => {
                encoder.emit_dict(|mut e| {
                    e.emit_pair(b"complete", complete)?;
                    e.emit_pair(b"incomplete", incomplete)?;
                    e.emit_pair(b"interval", interval)?;

                    if let Some(min_interval) = min_interval {
                        e.emit_pair(b"min_interval", min_interval)?;
                    }

                    e.emit_pair(b"peers", peers)?;
                    e.emit_pair(b"peers6", peers6)?;

                    e.emit_pair(b"tracker_id", tracker_id)?;


                    if let Some(warning_message) = warning_message {
                        e.emit_pair(b"warning_message", warning_message)?;
                    }

                    Ok(())
                })?;
            }
        }

        Ok(())
    }
}

async fn handle_announce(
    announce: Query<AnnounceRequest>,
    State(mut swarm_store): State<SwarmStore>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let announce: AnnounceRequest = announce.0;

    let info_hash = announce.info_hash;

    if let Some(event) = announce.event {
        let ip = if let Some(client_ip) = announce.ip {
            client_ip
        } else {
            addr.ip()
        };

        let peer = Peer {
            id: announce.peer_id,
            ip,
            port: announce.port,
        };

        let is_download_complete = announce.left == 0;

        match event {
            Event::Started => {
                swarm_store
                    .add_peer(info_hash.clone(), peer, is_download_complete)
                    .await;
            }
            Event::Stopped => {
                swarm_store.remove_peer(info_hash.clone(), peer).await;
            }
            Event::Completed => {
                swarm_store.promote_peer(info_hash.clone(), peer).await;
            }
        }
    }

    let numwant = if let Some(n) = announce.numwant {
        n
    } else {
        30 // 30 peers is generally a good amount
    };

    let (peers, peers6) = swarm_store.get_peers(info_hash.clone(), numwant).await;
    let (complete, incomplete) = swarm_store.get_announce_stats(info_hash).await;

    let response = AnnounceResponse::Success {
        interval: 1800, // 30-minute announce interval
        complete,
        incomplete,
        peers,
        peers6,
        tracker_id: String::from("test"),
        min_interval: None,
        warning_message: None,
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain")],
        response.to_bencode().unwrap(),
    )
}

async fn handle_scrape(scrape: Query<Vec<(String, String)>>) {
    let info_hashes: Option<Vec<InfoHash>> = if scrape.0.is_empty() {
        None
    } else {
        let raw_info_hashes: Vec<&(String, String)> = scrape
            .0
            .iter()
            .filter(|(key, _)| key.to_lowercase() == "info_hash")
            .collect();
        if raw_info_hashes.is_empty() {
            None
        } else {
            let decoded_info_hashes = raw_info_hashes
                .into_iter()
                .map(|(_, raw_val)| urlencoding::decode_binary(raw_val.as_bytes()).into_owned())
                .filter(|buf| buf.len() == 20)
                .collect();
            Some(decoded_info_hashes)
        }
    };

    let scrape = ScrapeRequest { info_hashes };
}

#[tokio::main]
async fn main() {
    let swarm_store: SwarmStore = SwarmStore::new();

    let app = Router::new()
        .route("/announce", get(handle_announce))
        .route("/scrape", get(handle_scrape))
        .route("/", get(|| async { "Hello, World!" }))
        .with_state(swarm_store);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
