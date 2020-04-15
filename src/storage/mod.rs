pub mod mysql;
pub mod reaper;

use std::sync::Arc;

use hashbrown::{HashMap, HashSet};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::bittorrent::Peer;
use crate::bittorrent::ScrapeFile;

#[derive(Debug, Clone)]
struct PeerList(Vec<Peer>);

// Wasn't a huge fan of this, but couldn't do it using FromIterator
impl PeerList {
    fn new() -> PeerList {
        PeerList(Vec::new())
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

pub type TorrentRecords = HashMap<String, Torrent>;

// TorrentStore needs to be wrapped in a RwLock or other exclusion
// primitive in order to prevent data races. This is further wrapped
// in an atomic reference counter in order to make it thread-safe.
#[derive(Debug, Clone)]
pub struct TorrentStore {
    pub torrents: Arc<RwLock<TorrentRecords>>,
}

impl TorrentStore {
    pub fn new(torrent_records: TorrentRecords) -> Result<TorrentStore, ()> {
        Ok(TorrentStore {
            torrents: Arc::new(RwLock::new(torrent_records)),
        })
    }

    pub fn default() -> Result<TorrentStore, ()> {
        Ok(TorrentStore {
            torrents: Arc::new(RwLock::new(TorrentRecords::new())),
        })
    }

    pub async fn get_scrapes(&self, info_hashes: Vec<String>) -> Vec<ScrapeFile> {
        let torrents = self.torrents.read().await;
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
    pub async fn get_announce_stats(&self, info_hash: String) -> (u32, u32) {
        let torrents = self.torrents.read().await;
        let mut complete: u32 = 0;
        let mut incomplete: u32 = 0;

        if let Some(t) = torrents.get(&info_hash) {
            complete = t.complete;
            incomplete = t.incomplete;
        }

        (complete, incomplete)
    }

    pub async fn new_seed(&self, info_hash: String) {
        let mut torrents = self.torrents.write().await;
        if let Some(t) = torrents.get_mut(&info_hash) {
            t.complete += 1;
            t.incomplete = t.incomplete.saturating_sub(1);
        }
    }

    pub async fn new_leech(&self, info_hash: String) {
        let mut torrents = self.torrents.write().await;
        if let Some(t) = torrents.get_mut(&info_hash) {
            t.incomplete += 1;
        }
    }

    /*pub fn undo_snatch(&self, info_hash: String) {
        let mut torrents = self.torrents.write();
        if let Some(t) = torrents.get_mut(&info_hash) {
            t.incomplete = t.incomplete.saturating_sub(1);
        }
    }*/
}

#[derive(Debug, Clone)]
pub struct Swarm {
    pub seeders: HashSet<Peer>,
    pub leechers: HashSet<Peer>,
}

// Swarm actually holds the peers for each torrent. The structure
// is essentially a wrapper around HashSet with a tiny bit of logic.
// The more complex logic around peer retrieval takes place in PeerStore.
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

    // The update methods ensure that peers that
    // continue to announce have accurate announce times
    // in order to prevent errant peer reaping
    fn update_seeder(&mut self, peer: Peer) {
        if self.seeders.contains(&peer) {
            self.seeders.replace(peer);
        }
    }

    fn update_leecher(&mut self, peer: Peer) {
        if self.leechers.contains(&peer) {
            self.leechers.replace(peer);
        }
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

// PeerStore needs to be wrapped in a RwLock or other exclusion
// primitive in order to prevent data races. This is further wrapped
// in an atomic reference counter in order to make it thread-safe.
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

    pub async fn put_seeder(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write().await;
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

    pub async fn remove_seeder(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write().await;
        if let Some(sw) = store.get_mut(&info_hash) {
            sw.remove_seeder(peer);
        }
    }

    pub async fn put_leecher(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write().await;
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

    pub async fn remove_leecher(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write().await;
        if let Some(sw) = store.get_mut(&info_hash) {
            sw.remove_leecher(peer);
        }
    }

    pub async fn promote_leecher(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write().await;
        if let Some(sw) = store.get_mut(&info_hash) {
            sw.promote_leecher(peer);
        }
    }

    pub async fn update_peer(&self, info_hash: String, peer: Peer) {
        let mut store = self.records.write().await;
        if let Some(sw) = store.get_mut(&info_hash) {
            sw.update_seeder(peer.clone());
            sw.update_leecher(peer);
        }
    }

    // Returns a randomized vector of peers to be returned to client
    pub async fn get_peers(&self, info_hash: String, numwant: u32) -> Vec<Peer> {
        let mut peer_list = PeerList::new();

        let store = self.records.read().await;
        if let Some(sw) = store.get(&info_hash) {
            let mut seeds: Vec<Peer> = sw.seeders.iter().map(|p| p.clone()).collect();
            let mut leeches: Vec<Peer> = sw.leechers.iter().map(|p| p.clone()).collect();
            peer_list.0.append(&mut seeds);
            peer_list.0.append(&mut leeches);
        }

        // Randomized bunch of seeders and leechers
        peer_list.give_random(numwant)
    }
}

#[derive(Debug, Clone)]
pub struct Stores {
    pub peer_store: PeerStore,
    pub torrent_store: TorrentStore,
}

impl Stores {
    pub fn new(torrent_records: TorrentRecords) -> Stores {
        Stores {
            peer_store: PeerStore::new().unwrap(),
            torrent_store: TorrentStore::new(torrent_records).unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {

    use std::net::Ipv4Addr;
    use std::time::Instant;

    use crate::bittorrent::{Peer, Peerv4};

    use super::*;

    #[tokio::test]
    async fn memory_peer_storage_put_seeder_new_swarm() {
        let records = TorrentRecords::new();
        let stores = Stores::new(records);
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
            last_announced: Instant::now(),
        });

        stores
            .peer_store
            .put_seeder(info_hash.clone(), peer.clone())
            .await;
        assert_eq!(
            stores
                .peer_store
                .records
                .read()
                .await
                .get(&info_hash)
                .unwrap()
                .seeders
                .contains(&peer),
            true
        );
    }

    #[tokio::test]
    async fn memory_peer_storage_put_seeder_prior_swarm() {
        let records = TorrentRecords::new();
        let stores = Stores::new(records);
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer1 = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
            last_announced: Instant::now(),
        });

        stores.peer_store.put_seeder(info_hash.clone(), peer1).await;

        let peer2 = Peer::V4(Peerv4 {
            peer_id: "TSRQPONMLKJIHGFEDCBA".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6881,
            last_announced: Instant::now(),
        });

        stores
            .peer_store
            .put_seeder(info_hash.clone(), peer2.clone())
            .await;
        assert_eq!(
            stores
                .peer_store
                .records
                .read()
                .await
                .get(&info_hash)
                .unwrap()
                .seeders
                .contains(&peer2),
            true
        );
    }

    #[tokio::test]
    async fn memory_peer_storage_put_leecher_new_swarm() {
        let records = TorrentRecords::new();
        let stores = Stores::new(records);
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
            last_announced: Instant::now(),
        });

        stores
            .peer_store
            .put_leecher(info_hash.clone(), peer.clone())
            .await;
        assert_eq!(
            stores
                .peer_store
                .records
                .read()
                .await
                .get(&info_hash)
                .unwrap()
                .leechers
                .contains(&peer),
            true
        );
    }

    #[tokio::test]
    async fn memory_peer_storage_put_leecher_prior_swarm() {
        let records = TorrentRecords::new();
        let stores = Stores::new(records);
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer1 = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
            last_announced: Instant::now(),
        });

        stores.peer_store.put_seeder(info_hash.clone(), peer1).await;

        let peer2 = Peer::V4(Peerv4 {
            peer_id: "TSRQPONMLKJIHGFEDCBA".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6881,
            last_announced: Instant::now(),
        });

        stores
            .peer_store
            .put_leecher(info_hash.clone(), peer2.clone())
            .await;
        assert_eq!(
            stores
                .peer_store
                .records
                .read()
                .await
                .get(&info_hash)
                .unwrap()
                .leechers
                .contains(&peer2),
            true
        );
    }

    #[tokio::test]
    async fn memory_peer_storage_remove_seeder() {
        let records = TorrentRecords::new();
        let stores = Stores::new(records);
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
            last_announced: Instant::now(),
        });

        stores
            .peer_store
            .put_seeder(info_hash.clone(), peer.clone())
            .await;

        stores
            .peer_store
            .remove_seeder(info_hash.clone(), peer.clone())
            .await;
        assert_eq!(
            stores
                .peer_store
                .records
                .read()
                .await
                .get(&info_hash)
                .unwrap()
                .seeders
                .contains(&peer),
            false
        );
    }

    #[tokio::test]
    async fn memory_peer_storage_remove_leecher() {
        let records = TorrentRecords::new();
        let stores = Stores::new(records);
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
            last_announced: Instant::now(),
        });

        stores
            .peer_store
            .put_leecher(info_hash.clone(), peer.clone())
            .await;

        stores
            .peer_store
            .remove_leecher(info_hash.clone(), peer.clone())
            .await;
        assert_eq!(
            stores
                .peer_store
                .records
                .read()
                .await
                .get(&info_hash)
                .unwrap()
                .leechers
                .contains(&peer),
            false
        );
    }

    #[tokio::test]
    async fn memory_peer_storage_promote_leecher() {
        let records = TorrentRecords::new();
        let stores = Stores::new(records);
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
            last_announced: Instant::now(),
        });

        stores
            .peer_store
            .put_leecher(info_hash.clone(), peer.clone())
            .await;
        stores
            .peer_store
            .promote_leecher(info_hash.clone(), peer.clone())
            .await;

        assert_eq!(
            stores
                .peer_store
                .records
                .read()
                .await
                .get(&info_hash)
                .unwrap()
                .seeders
                .contains(&peer),
            true
        );
    }

    #[tokio::test]
    async fn memory_peer_storage_update_peer() {
        let records = TorrentRecords::new();
        let stores = Stores::new(records);
        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
            last_announced: Instant::now(),
        });

        stores
            .peer_store
            .put_leecher(info_hash.clone(), peer.clone())
            .await;

        let peer2 = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
            last_announced: Instant::now(),
        });

        stores
            .peer_store
            .update_peer(info_hash.clone(), peer2.clone())
            .await;

        assert_eq!(
            stores
                .peer_store
                .records
                .read()
                .await
                .get(&info_hash)
                .unwrap()
                .leechers
                .contains(&peer2),
            true
        );
    }
}
