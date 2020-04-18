use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::statistics::GlobalStatistics;
use crate::storage::{PeerStore, TorrentStore};

#[derive(Clone)]
pub struct State {
    pub config: Config,
    pub peer_store: PeerStore,
    pub stats: Arc<RwLock<GlobalStatistics>>,
    pub torrent_store: TorrentStore,
}

impl State {
    pub fn new(config: Config, torrent_store: TorrentStore) -> State {
        State {
            config,
            peer_store: PeerStore::new(),
            stats: Arc::new(RwLock::new(GlobalStatistics::new())),
            torrent_store,
        }
    }
}
