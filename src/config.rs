use std::fs::File;
use std::io::Read;

use serde::Deserialize;
use toml;

#[derive(Default, Deserialize)]
pub struct Config {
    pub network: Network,
    pub storage: Storage,
    pub bt: BitTorrent,
}

#[derive(Deserialize)]
pub struct Network {
    pub binding: String,
}

#[derive(Deserialize)]
pub struct Storage {
    pub backend: String,
    pub path: String,
    pub password: Option<String>,
}

#[derive(Deserialize)]
pub struct BitTorrent {
    announce_rate: String,
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
            announce_rate: "1600".to_string(),
        }
    }
}

impl Config {
    pub fn load_config(path: String) -> Config {
        let mut config_toml = String::new();

        let mut file = match File::open(&path) {
            Ok(file) => file,
            _ => {
                error!("Could not find config file; loading default config...");
                return Config::default();
            }
        };

        if file.read_to_string(&mut config_toml).is_err() {
            error!("Could not read config file; loading default config...");
            return Config::default();
        };

        let config: Config = match toml::from_str(&config_toml) {
            Ok(t) => t,
            _ => {
                error!("Could not parse config file; loading default config...");
                return Config::default();
            }
        };

        info!("Binding to address: {}", &config.network.binding);
        info!(
            "Utilizing {} storage backend located at {}",
            &config.storage.backend, &config.storage.path
        );
        info!("Announce interval: {} seconds", &config.bt.announce_rate);

        config
    }
}
