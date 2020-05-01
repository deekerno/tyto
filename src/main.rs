pub mod bencode;
pub mod bittorrent;
pub mod config;
pub mod errors;
pub mod network;
pub mod state;
pub mod statistics;
pub mod storage;
pub mod util;

use actix::prelude::*;
use actix_rt;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use clap::{App as ClapApp, Arg};
use config::Config;
use mysql;
use pretty_env_logger;
use state::State;
use storage::janitor::Janitor;

#[macro_use]
extern crate log;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "INFO");
    }
    pretty_env_logger::init_timed();

    let matches = ClapApp::new("tyto")
        .version("0.5.5")
        .author("Alexander Decurnou. <ad@alx.xyz>")
        .about("A BitTorrent tracker that aims to be distributed, fast, and fault-tolerant.")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("configuration")
                .value_name("CONFIG_FILE")
                .help("Start the tracker using this configuration")
                .takes_value(true),
        )
        .get_matches();

    // Parse arguments and attempt to parse configuration file
    let config_path = matches.value_of("config");
    let config = match config_path {
        Some(path) => Config::load_config(path.to_string()),
        None => Config::load_config("config.toml".to_string()),
    };

    // Copy and cloning up here to avoid errors for moved values
    let binding = config.network.binding.clone();

    // TODO: abstract into a general loading function
    // TODO: add support to pass mysql password
    // Collect torrents from desired storage
    // backend and instantiate data stores.
    let pool = mysql::Pool::new(&config.storage.path).unwrap();
    let torrents = storage::mysql::get_torrents(pool.clone()).unwrap();
    info!("Number of torrents loaded: {}", torrents.len());

    let torrent_records = storage::TorrentStore::new(torrents);
    let state = web::Data::new(State::new(config.clone(), torrent_records));
    let janitor_state_clone = state.clone();

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            // Log all requests to stdout
            //.wrap(middleware::Logger::default())
            // If enabled, filter requests
            // by client ID and reject or accept
            .wrap(middleware::Condition::new(
                config.client_approval.enabled,
                network::middleware::ClientApproval::new(
                    config.client_approval.blacklist_style,
                    config.client_approval.versioned,
                    config.client_approval.client_list.clone(),
                ),
            ))
            .service(web::scope("announce").route("", web::get().to(network::parse_announce)))
            .service(web::scope("scrape").route("", web::get().to(network::parse_scrape)))
            .service(web::scope("stats").route("", web::get().to(network::get_stats)))
            .service(web::scope("/").route("", web::get().to(|| HttpResponse::MethodNotAllowed())))
    })
    .bind(binding)?
    .run();

    // Start janitor in its own thread
    Janitor::create(|_ctx: &mut Context<Janitor>| Janitor::new(janitor_state_clone, pool));

    // Start server
    server.await
}
