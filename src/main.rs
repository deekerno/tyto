pub mod bencode;
pub mod bittorrent;
pub mod config;
pub mod network;
pub mod storage;
pub mod util;

use actix_rt;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use clap::{App as ClapApp, Arg};
use config::Config;
use mysql;
use pretty_env_logger;

#[macro_use]
extern crate log;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "INFO");
    }
    pretty_env_logger::init_timed();

    let matches = ClapApp::new("tyto")
        .version("0.1")
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

    let config_path = matches.value_of("config");
    let config = match config_path {
        Some(path) => Config::load_config(path.to_string()),
        None => Config::load_config("config.toml".to_string()),
    };
    let binding = config.network.binding.clone();

    // This will soon be abstracted out into a general loading function
    let pool = mysql::Pool::new(&config.storage.path).unwrap();
    let torrents = storage::mysql::get_torrents(pool).unwrap();
    let stores = web::Data::new(storage::Stores::new(torrents.clone()));
    info!("Number of torrents loaded: {}", torrents.len());

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(network::middleware::ClientApproval::new(
                config.client_approval.blacklist_style,
                config.client_approval.versioned,
                config.client_approval.client_list.clone(),
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
    .run()
    .await
}
