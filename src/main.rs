pub mod bencode;
pub mod bittorrent;
pub mod storage;
pub mod util;

use actix_web::{web, App, HttpResponse};
use clap::{App as ClapApp, Arg};
use env_logger;

use bittorrent::{AnnounceRequest, ScrapeRequest};

#[macro_use]
extern crate log;

fn parse_announce() {
    //
}

fn parse_scrape() {
    //
}

fn main() {
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

    let app = App::new()
        .route("/announce", web::get().to(parse_announce))
        .route("/scrape", web::get().to(parse_scrape));
}
