pub mod bencode;
pub mod bittorrent;
pub mod config;
pub mod network;
pub mod storage;
pub mod util;

use actix::prelude::*;
use actix_rt;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use clap::{App as ClapApp, Arg};
use config::Config;
use mysql;
use pretty_env_logger;
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
        .version("0.2.1")
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
    let reap_interval = config.bt.reap_interval;
    let peer_timeout = config.bt.peer_timeout;
    let flush_interval = config.bt.flush_interval;

    // TODO: abstract into a general loading function
    // TODO: add support to pass mysql password
    // Collect torrents from desired storage
    // backend and instantiate data stores.
    let pool = mysql::Pool::new(&config.storage.path).unwrap();
    let torrents = storage::mysql::get_torrents(pool.clone()).unwrap();
    let stores = web::Data::new(storage::Stores::new(torrents.clone()));
    let janitor_store_clone = stores.clone();
    info!("Number of torrents loaded: {}", torrents.len());

    let server = HttpServer::new(move || {
        App::new()
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
            .service(
                web::scope("announce")
                    .app_data(stores.clone())
                    .route("", web::get().to(network::parse_announce)),
            )
            .service(
                web::scope("scrape")
                    .app_data(stores.clone())
                    .route("", web::get().to(network::parse_scrape)),
            )
            .service(web::scope("/").route("", web::get().to(|| HttpResponse::MethodNotAllowed())))
    })
    .bind(binding)?
    .run();

    // Start janitor in its own thread
    Janitor::create(|_ctx: &mut Context<Janitor>| {
        Janitor::new(
            reap_interval,
            peer_timeout,
            flush_interval,
            janitor_store_clone,
            pool,
        )
    });

    // Start server
    server.await
}
