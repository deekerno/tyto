use crate::bittorrent::Peer;
use crate::storage;

use std::time::Duration;

use actix::prelude::*;
use actix::utils::IntervalFunc;
use actix_web::web;

pub struct Reaper {
    interval: Duration,
    peer_timeout: Duration,
    state: web::Data<storage::Stores>,
}

impl Reaper {
    pub fn new(
        interval: Duration,
        peer_timeout: Duration,
        state: web::Data<storage::Stores>,
    ) -> Reaper {
        Reaper {
            interval,
            peer_timeout,
            state,
        }
    }

    pub fn reap_peers(&mut self, _context: &mut Context<Self>) {
        info!("Reaping peers...");

        let mut seeds_reaped = 0;
        let mut leeches_reaped = 0;

        let info_hashes: Vec<String> = self
            .state
            .peer_store
            .records
            .read()
            .iter()
            .map(|(info_hash, _)| info_hash.clone())
            .collect();

        for info_hash in info_hashes {
            if let Some(swarm) = self.state.peer_store.records.write().get_mut(&info_hash) {
                let seeds_1 = swarm.seeders.len();
                let leeches_1 = swarm.leechers.len();

                swarm.seeders.retain(|peer| match peer {
                    Peer::V4(p) => p.last_announced.elapsed() < self.peer_timeout,
                    Peer::V6(p) => p.last_announced.elapsed() < self.peer_timeout,
                });
                swarm.leechers.retain(|peer| match peer {
                    Peer::V4(p) => p.last_announced.elapsed() < self.peer_timeout,
                    Peer::V6(p) => p.last_announced.elapsed() < self.peer_timeout,
                });

                seeds_reaped += seeds_1 - swarm.seeders.len();
                leeches_reaped += leeches_1 - swarm.leechers.len();
            }
        }

        info!(
            "Reaped {} seeders and {} leechers.",
            seeds_reaped, leeches_reaped
        );
    }
}

impl Actor for Reaper {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        // This will go through all of the swarms and remove
        // any peers that have not announced in a defined time
        IntervalFunc::new(self.interval, Self::reap_peers)
            .finish()
            .spawn(ctx);
    }
}
