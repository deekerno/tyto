use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct GlobalStatistics {
    pub start_time: Instant,
    pub total_seeders: u32,
    pub total_leechers: u32,
    pub announce_requests: u32,
    pub succ_announces: u32,
    pub scrapes: u32,
}

impl GlobalStatistics {
    pub fn new() -> GlobalStatistics {
        GlobalStatistics {
            start_time: Instant::now(),
            total_seeders: 0,
            total_leechers: 0,
            announce_requests: 0,
            succ_announces: 0,
            scrapes: 0,
        }
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn succ_announce(&mut self) {
        self.announce_requests += 1;
        self.succ_announces += 1;
    }

    pub fn fail_announce(&mut self) {
        self.announce_requests += 1;
    }

    pub fn num_fails(&self) -> u32 {
        self.announce_requests - self.succ_announces
    }

    pub fn incr_scrapes(&mut self) {
        self.scrapes += 1;
    }

    pub fn add_seed(&mut self) {
        self.total_seeders += 1;
    }

    pub fn add_leech(&mut self) {
        self.total_leechers += 1;
    }

    pub fn sub_seed(&mut self) {
        self.total_seeders -= 1;
    }

    pub fn sub_leech(&mut self) {
        self.total_leechers -= 1;
    }

    pub fn promote_leech(&mut self) {
        self.total_leechers -= 1;
        self.total_seeders += 1;
    }

    pub fn cleared_peers(&mut self, seeders_cleared: u32, leechers_cleared: u32) {
        self.total_seeders -= seeders_cleared;
        self.total_leechers -= leechers_cleared;
    }
}
