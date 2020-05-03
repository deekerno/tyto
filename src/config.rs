use std::fs::File;
use std::io::Read;

use serde::Deserialize;
use toml;

use crate::errors::InternalError;

#[derive(Default, Deserialize, Clone)]
pub struct Config {
    pub network: Network,
    pub storage: Storage,
    pub bt: BitTorrent,
    pub client_approval: ClientApproval,
}

#[derive(Deserialize, Clone)]
pub struct Network {
    pub binding: String,
}

#[derive(Deserialize, Clone)]
pub struct Storage {
    pub backend: String,
    pub path: String,
    pub password: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct BitTorrent {
    pub announce_rate: u64,
    pub peer_timeout: u64,
    pub reap_interval: u64,
    pub flush_interval: u64,
}

#[derive(Deserialize, Clone)]
pub struct ClientApproval {
    pub enabled: bool,
    pub blacklist_style: bool,
    pub versioned: bool,
    pub client_list: Vec<String>,
}

impl Default for Network {
    fn default() -> Self {
        Network {
            binding: "0.0.0.0:8585".to_string(),
        }
    }
}

impl Default for Storage {
    fn default() -> Self {
        Storage {
            backend: "memory".to_string(),
            path: "".to_string(),
            password: None,
        }
    }
}

impl Default for BitTorrent {
    fn default() -> Self {
        BitTorrent {
            announce_rate: 1800,
            peer_timeout: 7200,
            reap_interval: 1800,
            flush_interval: 900,
        }
    }
}

impl Default for ClientApproval {
    fn default() -> ClientApproval {
        ClientApproval {
            enabled: false,
            blacklist_style: false,
            versioned: false,
            client_list: Vec::new(),
        }
    }
}

impl Config {
    pub fn load_config(path: String) -> Config {
        let mut config_toml = String::new();

        let mut file = match File::open(&path) {
            Ok(file) => file,
            _ => {
                error!("{}", InternalError::ConfigFileOpen.text());
                return Config::default();
            }
        };

        if file.read_to_string(&mut config_toml).is_err() {
            error!("{}", InternalError::ConfigFileRead.text());
            return Config::default();
        };

        let config: Config = match toml::from_str(&config_toml) {
            Ok(t) => t,
            _ => {
                error!("{}", InternalError::ConfigParse.text());
                Config::default()
            }
        };

        info!("Binding to address: {}", &config.network.binding);
        info!(
            "Utilizing {} storage backend located at {}",
            &config.storage.backend, &config.storage.path
        );
        info!("Announce interval: {} secs", &config.bt.announce_rate);
        info!(
            "Clearing peers older than {} secs at {}-sec interval",
            &config.bt.peer_timeout, &config.bt.reap_interval
        );
        info!(
            "Flushing torrents to disk every {} secs",
            &config.bt.flush_interval
        );
        info!("Client list: {:?}", &config.client_approval.client_list);

        config
    }
}
