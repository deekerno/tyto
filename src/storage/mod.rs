use std::sync::Arc;

use hashbrown::{HashMap, HashSet};
use parking_lot::*;

use crate::bittorrent::{Peer, Peerv4, Peerv6};

// todo: Need to do a generic Peer so that both peer types are passable
pub trait PeerStorage {
    fn put_seeder(&self, info_hash: String, peer: Peer);
    fn remove_seeder(&self, info_hash: String, peer: Peer);
    fn put_leecher(&self, info_hash: String, peer: Peer);
    fn remove_leecher(&self, info_hash: String, peer: Peer);
    fn promote_leecher(&self, info_hash: String, peer: Peer);
}

pub trait TorrentStorage {
    fn get_torrents(&self);
    fn flush_torrents(&self);
    fn add_torrents(&self);
}

// Should these be byte strings instead of just peer types?
// Or should Hash be implemented for the peer types?
pub struct Swarm {
    pub seeders: HashSet<Peer>,
    pub leechers: HashSet<Peer>,
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
        let result = self.seeders.remove(&peer);
    }

    fn remove_leecher(&mut self, peer: Peer) {
        let result = self.leechers.remove(&peer);
    }

    fn promote_leecher(&mut self, peer: Peer) {
        if let Some(leecher) = self.leechers.take(&peer) {
            self.seeders.insert(leecher);
        }
    }
}

type Records = HashMap<String, Swarm>;

// Sharable between threads, multiple readers, one writer
pub struct PeerStore {
    pub records: Arc<RwLock<Records>>,
}

impl PeerStore {
    fn new() -> Result<PeerStore, &'static str> {
        Ok(PeerStore {
            records: Arc::new(RwLock::new(Records::new())),
        })
    }
}

impl PeerStorage for PeerStore {
    fn put_seeder(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write();
        match store.get_mut(&info_hash) {
            Some(sw) => {
                sw.add_seeder(peer);
            }
            None => {
                let mut sw = Swarm::new();
                sw.add_seeder(peer);
                store.insert(info_hash, sw);
            }
        }
    }

    fn remove_seeder(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write();
        match store.get_mut(&info_hash) {
            Some(sw) => {
                sw.remove_seeder(peer);
            }
            None => {}
        }
    }

    fn put_leecher(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write();
        match store.get_mut(&info_hash) {
            Some(sw) => {
                sw.add_leecher(peer);
            }
            None => {
                let mut sw = Swarm::new();
                sw.add_leecher(peer);
                store.insert(info_hash, sw);
            }
        }
    }

    fn remove_leecher(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write();
        match store.get_mut(&info_hash) {
            Some(sw) => {
                sw.remove_leecher(peer);
            }
            None => {}
        }
    }
    fn promote_leecher(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write();
        match store.get_mut(&info_hash) {
            Some(sw) => {
                sw.promote_leecher(peer);
            }
            None => {}
        }
    }
}
