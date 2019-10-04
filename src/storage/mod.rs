use std::sync::Arc;

use hashbrown::{HashMap, HashSet};
use parking_lot::RwLock;

use crate::bittorrent::{Peer, Peerv4, Peerv6};

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
    pub fn new() -> Result<PeerStore, &'static str> {
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
        if let Some(sw) = store.get_mut(&info_hash) {
            sw.remove_seeder(peer);
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
        if let Some(sw) = store.get_mut(&info_hash) {
            sw.remove_leecher(peer);
        }
    }

    fn promote_leecher(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write();
        if let Some(sw) = store.get_mut(&info_hash) {
            sw.promote_leecher(peer);
        }
    }
}

#[cfg(test)]
mod tests {

    use std::net::{Ipv4Addr, Ipv6Addr};

    use crate::bittorrent::{Peer, Peerv4, Peerv6};

    use super::*;

    #[test]
    fn memory_peer_storage_put_seeder_new_swarm() {
        let store = PeerStore::new().unwrap();
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        store.put_seeder(info_hash.clone(), peer.clone());
        assert_eq!(
            store
                .records
                .read()
                .get(&info_hash)
                .unwrap()
                .seeders
                .contains(&peer),
            true
        );
    }

    #[test]
    fn memory_peer_storage_put_seeder_prior_swarm() {
        let store = PeerStore::new().unwrap();
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer1 = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        store.put_seeder(info_hash.clone(), peer1);

        let peer2 = Peer::V4(Peerv4 {
            peer_id: "TSRQPONMLKJIHGFEDCBA".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6881,
        });

        store.put_seeder(info_hash.clone(), peer2.clone());
        assert_eq!(
            store
                .records
                .read()
                .get(&info_hash)
                .unwrap()
                .seeders
                .contains(&peer2),
            true
        );
    }

    #[test]
    fn memory_peer_storage_put_leecher_new_swarm() {
        let store = PeerStore::new().unwrap();
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        store.put_leecher(info_hash.clone(), peer.clone());
        assert_eq!(
            store
                .records
                .read()
                .get(&info_hash)
                .unwrap()
                .leechers
                .contains(&peer),
            true
        );
    }

    #[test]
    fn memory_peer_storage_put_leecher_prior_swarm() {
        let store = PeerStore::new().unwrap();
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer1 = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        store.put_seeder(info_hash.clone(), peer1);

        let peer2 = Peer::V4(Peerv4 {
            peer_id: "TSRQPONMLKJIHGFEDCBA".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6881,
        });

        store.put_leecher(info_hash.clone(), peer2.clone());
        assert_eq!(
            store
                .records
                .read()
                .get(&info_hash)
                .unwrap()
                .leechers
                .contains(&peer2),
            true
        );
    }

    #[test]
    fn memory_peer_storage_remove_seeder() {
        let store = PeerStore::new().unwrap();
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        store.put_seeder(info_hash.clone(), peer.clone());

        store.remove_seeder(info_hash.clone(), peer.clone());
        assert_eq!(
            store
                .records
                .read()
                .get(&info_hash)
                .unwrap()
                .seeders
                .contains(&peer),
            false
        );
    }

    #[test]
    fn memory_peer_storage_remove_leecher() {
        let store = PeerStore::new().unwrap();
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        store.put_leecher(info_hash.clone(), peer.clone());

        store.remove_leecher(info_hash.clone(), peer.clone());
        assert_eq!(
            store
                .records
                .read()
                .get(&info_hash)
                .unwrap()
                .leechers
                .contains(&peer),
            false
        );
    }

    #[test]
    fn memory_peer_storage_promote_leecher() {
        let store = PeerStore::new().unwrap();
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        store.put_leecher(info_hash.clone(), peer.clone());
        store.promote_leecher(info_hash.clone(), peer.clone());

        assert_eq!(
            store
                .records
                .read()
                .get(&info_hash)
                .unwrap()
                .seeders
                .contains(&peer),
            true
        );
    }
}
