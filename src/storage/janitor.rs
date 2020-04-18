use crate::bittorrent::Peer;
use crate::state::State;
use crate::storage;

use std::time::Duration;

use actix::prelude::*;
use actix_web::web;
use mysql::*;

#[derive(Clone)]
pub struct Janitor {
    reap_interval: Duration,
    peer_timeout: Duration,
    flush_interval: Duration,
    state: web::Data<State>,
    pool: Pool,
}

impl Janitor {
    pub fn new(state: web::Data<State>, pool: Pool) -> Janitor {
        Janitor {
            reap_interval: Duration::new(state.config.bt.reap_interval, 0),
            peer_timeout: Duration::new(state.config.bt.peer_timeout, 0),
            flush_interval: Duration::new(state.config.bt.flush_interval, 0),
            state,
            pool,
        }
    }

    // Had to clone self to avoid wacky lifetime error
    fn clear_peers(&mut self, ctx: &mut Context<Self>) {
        let self2 = self.clone();
        ctx.spawn(actix::fut::wrap_future(async move {
            info!("Clearing away stale peers...");

            let mut seeds_cleared = 0;
            let mut leeches_cleared = 0;

            let info_hashes: Vec<String> = self2
                .state
                .peer_store
                .records
                .read()
                .await
                .iter()
                .map(|(info_hash, _)| info_hash.clone())
                .collect();

            for info_hash in info_hashes {
                if let Some(swarm) = self2
                    .state
                    .peer_store
                    .records
                    .write()
                    .await
                    .get_mut(&info_hash)
                {
                    let seeds_1 = swarm.seeders.len();
                    let leeches_1 = swarm.leechers.len();

                    swarm.seeders.retain(|peer| match peer {
                        Peer::V4(p) => p.last_announced.elapsed() < self2.peer_timeout,
                        Peer::V6(p) => p.last_announced.elapsed() < self2.peer_timeout,
                    });
                    swarm.leechers.retain(|peer| match peer {
                        Peer::V4(p) => p.last_announced.elapsed() < self2.peer_timeout,
                        Peer::V6(p) => p.last_announced.elapsed() < self2.peer_timeout,
                    });

                    seeds_cleared += seeds_1 - swarm.seeders.len();
                    leeches_cleared += leeches_1 - swarm.leechers.len();
                }
            }

            // Make sure that stats are up-to-date
            // TODO: Getting E0495 all over this thing
            /*self.state
                .stats
                .write()
                .await
                .cleared_peers(seeds_cleared as u32, leeches_cleared as u32);
            */

            info!(
                "Cleared {} seeders and {} leechers.",
                seeds_cleared, leeches_cleared
            );
        }));
    }

    // Had to clone self to avoid wacky lifetime error
    fn flush(&mut self, ctx: &mut Context<Self>) {
        let self2 = self.clone();
        ctx.spawn(actix::fut::wrap_future(async move {
            info!("Flushing torrents to database...");

            let torrents: Vec<storage::Torrent> = self2
                .state
                .torrent_store
                .torrents
                .read()
                .await
                .iter()
                .map(|(_, torrent)| torrent.clone())
                .collect();

            let num_torrents = torrents.len();

            let _result = storage::mysql::flush_torrents(self2.pool, torrents);

            info!("Flushed {} torrents.", num_torrents);
        }));
    }
}

impl Actor for Janitor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        info!("Janitor is now on duty...");

        // This will go through all of the swarms and remove
        // any peers that have not announced in a defined time
        ctx.run_interval(self.reap_interval, Self::clear_peers);

        // This will flush all torrent data to the database
        // to ensure that stats are up-to-date
        ctx.run_interval(self.flush_interval, Self::flush);
    }
}
