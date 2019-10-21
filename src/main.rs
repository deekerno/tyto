pub mod bencode;
pub mod bittorrent;
pub mod network;
pub mod storage;
pub mod util;

use std::io;

use actix_web::{guard, middleware, web, App, HttpResponse, HttpServer};
use clap::{App as ClapApp, Arg};
use pretty_env_logger;

#[macro_use]
extern crate log;

fn main() -> io::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "actix_web=DEBUG");
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

    info!("Loading configuration...");

    HttpServer::new(move || {
        // Creates a data object to be shared between actor threads
        let data = web::Data::new(storage::PeerStore::new().unwrap());

        App::new()
            .wrap(middleware::Logger::default())
            .register_data(data.clone())
            .service(
                web::resource("announce")
                    .guard(guard::Header("content-type", "text/plain"))
                    .route(web::get().to(network::parse_announce)),
            )
            .service(
                web::resource("scrape")
                    .guard(guard::Header("content-type", "text/plain"))
                    .route(web::get().to(network::parse_scrape)),
            )
            .default_service(web::route().to(HttpResponse::MethodNotAllowed))
    })
    .bind("127.0.0.1:8585")?
    .run()
}
