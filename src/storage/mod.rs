use std::sync::Arc;

use hashbrown::{HashMap, HashSet};
use parking_lot::RwLock;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::bittorrent::Peer;

struct PeerList(Vec<Peer>);

// Wasn't a huge fan of this, but couldn't do it using FromIterator
impl PeerList {
    fn new() -> PeerList {
        PeerList(Vec::new())
    }

    fn add_from_swarm(&mut self, elems: &HashSet<Peer>) {
        for peer in elems.clone().into_iter() {
            self.0.push(peer);
        }
    }

    fn give_random(&mut self, numwant: u32) -> Vec<Peer> {
        let mut rng = &mut rand::thread_rng();
        self.0
            .choose_multiple(&mut rng, numwant as usize)
            .cloned()
            .collect()
    }
}

pub trait PeerStorage {
    fn put_seeder(&self, info_hash: String, peer: Peer);
    fn remove_seeder(&self, info_hash: String, peer: Peer);
    fn put_leecher(&self, info_hash: String, peer: Peer);
    fn remove_leecher(&self, info_hash: String, peer: Peer);
    fn promote_leecher(&self, info_hash: String, peer: Peer);
    fn get_peers(&self, info_hash: String, numwant: u32) -> Vec<Peer>;
}

pub trait TorrentStorage {
    // This should retrieve all the torrents from whatever backing storage
    // is being used to store torrent details, e.g. SQL.
    fn get_torrents(&self);

    // This should flush all torrent details that are held in memory to the
    // backing storage in use for production.
    fn flush_torrents(&self);
}

#[derive(Serialize, Deserialize)]
pub struct Torrent {
    pub info_hash: String,
    pub complete: u32,   // Number of seeders
    pub downloaded: u32, // Amount of Event::Complete as been received
    pub incomplete: u32, // Number of leechers
    pub balance: u32,    // Total traffic for this torrent
}

impl Torrent {
    pub fn new(
        info_hash: String,
        complete: u32,
        downloaded: u32,
        incomplete: u32,
        balance: u32,
    ) -> Torrent {
        Torrent {
            info_hash,
            complete,
            downloaded,
            incomplete,
            balance,
        }
    }
}

type TorrentRecords = HashMap<String, Torrent>;

pub struct TorrentStore {
    pub torrents: Arc<RwLock<TorrentRecords>>,
}

impl TorrentStore {
    pub fn new() -> Result<TorrentStore, &'static str> {
        Ok(TorrentStore {
            torrents: Arc::new(RwLock::new(TorrentRecords::new())),
        })
    }
}

impl TorrentStorage for TorrentStore {
    fn get_torrents(&self) {}
    fn flush_torrents(&self) {}
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

type PeerRecords = HashMap<String, Swarm>;

// Sharable between threads, multiple readers, one writer
pub struct PeerStore {
    pub records: Arc<RwLock<PeerRecords>>,
}

impl PeerStore {
    pub fn new() -> Result<PeerStore, &'static str> {
        Ok(PeerStore {
            records: Arc::new(RwLock::new(PeerRecords::new())),
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

    // Returns a randomized vector of peers to be returned to client
    fn get_peers(&self, info_hash: String, numwant: u32) -> Vec<Peer> {
        let mut peer_list = PeerList::new();

        let store = self.records.read();
        if let Some(sw) = store.get(&info_hash) {
            peer_list.add_from_swarm(&sw.seeders);
            peer_list.add_from_swarm(&sw.leechers);
        }

        // Randomized bunch of seeders and leechers
        peer_list.give_random(numwant)
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
