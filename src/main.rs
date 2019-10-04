pub mod bencode;
pub mod bittorrent;
pub mod network;
pub mod storage;
pub mod util;

use std::io;
use std::sync::Arc;

use actix_web::{guard, web, App, HttpRequest, HttpResponse, HttpServer};
use clap::{App as ClapApp, Arg};
use env_logger;

use bittorrent::{AnnounceRequest, ScrapeRequest};
use network::{parse_announce, parse_scrape};

#[macro_use]
extern crate log;

fn main() -> io::Result<()> {
    env_logger::init();

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

    info!("Loading configuration...");

    // Creates a data object to be shared between actor threads
    let data = Arc::new(storage::PeerStore::new());

    HttpServer::new(move || {
        App::new()
            .data(data.clone())
            .service(
                web::resource("/announce")
                    .guard(guard::Header("content-type", "text/plain"))
                    .route(web::get().to(parse_announce)),
            )
            .service(
                web::resource("/scrape")
                    .guard(guard::Header("content-type", "text/plain"))
                    .route(web::get().to(parse_scrape)),
            )
            .default_service(web::route().to(HttpResponse::MethodNotAllowed))
    })
    .bind("127.0.0.1:8585")?
    .run()
}
