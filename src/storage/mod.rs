//pub mod mysql;

use std::fs;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;

use bincode::{deserialize_from, serialize_into};
use hashbrown::{HashMap, HashSet};
use parking_lot::RwLock;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::bittorrent::Peer;
use crate::bittorrent::ScrapeFile;

#[derive(Debug, Clone)]
struct PeerList(Vec<Peer>);

// Wasn't a huge fan of this, but couldn't do it using FromIterator
impl PeerList {
    fn new() -> PeerList {
        PeerList(Vec::new())
    }

    fn add_from_swarm(&mut self, elems: &HashSet<Peer>) {
        for peer in elems.iter().cloned() {
            self.0.push(peer);
        }
    }

    fn give_random(&mut self, numwant: u32) -> Vec<Peer> {
        // If the total amount of peers is less than numwant,
        // just return the entire list of peers
        if self.0.len() <= numwant as usize {
            self.0.clone()
        } else {
            // Otherwise, choose a random sampling and send it
            let mut rng = &mut rand::thread_rng();
            self.0
                .choose_multiple(&mut rng, numwant as usize)
                .cloned()
                .collect()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct TorrentMemoryStore {
    pub torrents: Arc<RwLock<TorrentRecords>>,
    pub path: String,
}

impl TorrentMemoryStore {
    pub fn new(path: String) -> Result<TorrentMemoryStore, &'static str> {
        Ok(TorrentMemoryStore {
            torrents: Arc::new(RwLock::new(TorrentRecords::new())),
            path,
        })
    }

    fn get_torrents(&mut self) {
        let mut torrent_flat_file_reader =
            BufReader::new(fs::File::open(&self.path).expect("Could not open database file"));
        let torrents =
            deserialize_from(&mut torrent_flat_file_reader).expect("Could not deserialize");
        self.torrents = Arc::new(RwLock::new(torrents));
    }

    fn flush_torrents(&self) {
        let torrents = self.torrents.read();
        let mut torrent_flat_file_writer =
            BufWriter::new(fs::File::create(&self.path).expect("Could not write to database path"));

        serialize_into(&mut torrent_flat_file_writer, &*torrents)
            .expect("Could not write database to file");
    }

    pub fn get_scrapes(&self, info_hashes: Vec<String>) -> Vec<ScrapeFile> {
        let torrents = self.torrents.read();
        let mut scrapes = Vec::new();

        for info_hash in info_hashes {
            if let Some(t) = torrents.get(&info_hash) {
                scrapes.push(ScrapeFile {
                    info_hash: info_hash.clone(),
                    complete: t.complete,
                    downloaded: t.downloaded,
                    incomplete: t.incomplete,
                    name: None,
                });
            }
        }

        scrapes
    }

    // Announces only require complete and incomplete
    pub fn get_announce_stats(&self, info_hash: String) -> (u32, u32) {
        let torrents = self.torrents.read();
        let mut complete: u32 = 0;
        let mut incomplete: u32 = 0;

        if let Some(t) = torrents.get(&info_hash) {
            complete = t.complete;
            incomplete = t.incomplete;
        }

        (complete, incomplete)
    }

    pub fn new_seed(&self, info_hash: String) {
        let mut torrents = self.torrents.write();
        if let Some(t) = torrents.get_mut(&info_hash) {
            t.complete += 1;
            t.incomplete -= 1;
        }
    }

    pub fn new_leech(&self, info_hash: String) {
        let mut torrents = self.torrents.write();
        if let Some(t) = torrents.get_mut(&info_hash) {
            t.incomplete += 1;
        }
    }

    /*pub fn undo_snatch(&self, info_hash: String) {
        let mut torrents = self.torrents.write();
        if let Some(t) = torrents.get_mut(&info_hash) {
            t.incomplete -= 1;
        }
    }*/
}

// Should these be byte strings instead of just peer types?
// Or should Hash be implemented for the peer types?
#[derive(Debug, Clone)]
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
        let _result = self.seeders.remove(&peer);
    }

    fn remove_leecher(&mut self, peer: Peer) {
        let _result = self.leechers.remove(&peer);
    }

    fn promote_leecher(&mut self, peer: Peer) {
        match self.leechers.take(&peer) {
            Some(leecher) => {
                self.seeders.insert(leecher);
            }
            None => {
                self.seeders.insert(peer);
            }
        };
    }
}

type PeerRecords = HashMap<String, Swarm>;

// Sharable between threads, multiple readers, one writer
#[derive(Debug, Clone)]
pub struct PeerStore {
    pub records: Arc<RwLock<PeerRecords>>,
}

impl PeerStore {
    pub fn new() -> Result<PeerStore, &'static str> {
        Ok(PeerStore {
            records: Arc::new(RwLock::new(PeerRecords::new())),
        })
    }

    pub fn put_seeder(&self, info_hash: String, peer: Peer) {
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

    pub fn remove_seeder(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write();
        if let Some(sw) = store.get_mut(&info_hash) {
            sw.remove_seeder(peer);
        }
    }

    pub fn put_leecher(&self, info_hash: String, peer: Peer) {
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

    pub fn remove_leecher(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write();
        if let Some(sw) = store.get_mut(&info_hash) {
            sw.remove_leecher(peer);
        }
    }

    pub fn promote_leecher(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write();
        if let Some(sw) = store.get_mut(&info_hash) {
            sw.promote_leecher(peer);
        }
    }

    // Returns a randomized vector of peers to be returned to client
    pub fn get_peers(&self, info_hash: String, numwant: u32) -> Vec<Peer> {
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

#[derive(Debug, Clone)]
pub struct Stores {
    pub peer_store: PeerStore,
    pub torrent_store: TorrentMemoryStore,
}

impl Stores {
    pub fn new(torrent_store_path: String) -> Stores {
        Stores {
            peer_store: PeerStore::new().unwrap(),
            torrent_store: TorrentMemoryStore::new(torrent_store_path).unwrap(),
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
        let stores = Stores::new("test".to_string());
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        stores
            .peer_store
            .put_seeder(info_hash.clone(), peer.clone());
        assert_eq!(
            stores
                .peer_store
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
        let stores = Stores::new("test".to_string());
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer1 = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        stores.peer_store.put_seeder(info_hash.clone(), peer1);

        let peer2 = Peer::V4(Peerv4 {
            peer_id: "TSRQPONMLKJIHGFEDCBA".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6881,
        });

        stores
            .peer_store
            .put_seeder(info_hash.clone(), peer2.clone());
        assert_eq!(
            stores
                .peer_store
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
        let stores = Stores::new("test".to_string());
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        stores
            .peer_store
            .put_leecher(info_hash.clone(), peer.clone());
        assert_eq!(
            stores
                .peer_store
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
        let stores = Stores::new("test".to_string());
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer1 = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        stores.peer_store.put_seeder(info_hash.clone(), peer1);

        let peer2 = Peer::V4(Peerv4 {
            peer_id: "TSRQPONMLKJIHGFEDCBA".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6881,
        });

        stores
            .peer_store
            .put_leecher(info_hash.clone(), peer2.clone());
        assert_eq!(
            stores
                .peer_store
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
        let stores = Stores::new("test".to_string());
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        stores
            .peer_store
            .put_seeder(info_hash.clone(), peer.clone());

        stores
            .peer_store
            .remove_seeder(info_hash.clone(), peer.clone());
        assert_eq!(
            stores
                .peer_store
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
        let stores = Stores::new("test".to_string());
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        stores
            .peer_store
            .put_leecher(info_hash.clone(), peer.clone());

        stores
            .peer_store
            .remove_leecher(info_hash.clone(), peer.clone());
        assert_eq!(
            stores
                .peer_store
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
        let stores = Stores::new("test".to_string());
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        stores
            .peer_store
            .put_leecher(info_hash.clone(), peer.clone());
        stores
            .peer_store
            .promote_leecher(info_hash.clone(), peer.clone());

        assert_eq!(
            stores
                .peer_store
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
