use actix_web::{web, Error, HttpRequest, HttpResponse};
use futures::{future::ok as fut_ok, Future};

use crate::bencode;
use crate::bittorrent::{AnnounceRequest, AnnounceResponse, Peer, ScrapeRequest, ScrapeResponse};
use crate::storage::Stores;
use crate::util::Event;

// This will eventually be read from the configuration YAML.
const INTERVAL: u32 = 60;

pub fn parse_announce(
    data: web::Data<Stores>,
    req: HttpRequest,
) -> impl Future<Item = HttpResponse, Error = Error> {
    let announce_request = AnnounceRequest::new(req.query_string(), req.connection_info().remote());

    match announce_request {
        Ok(parsed_req) => {
            match parsed_req.event {
                Event::Started => {
                    data.peer_store.put_leecher(parsed_req.info_hash.clone(), parsed_req.peer);

                    let peer_list =
                        data.peer_store.get_peers(parsed_req.info_hash.clone(), parsed_req.numwant.unwrap());
                    let mut peers = Vec::new();
                    let mut peers6 = Vec::new();

                    for peer in peer_list {
                        match peer {
                            Peer::V4(p) => peers.push(p),
                            Peer::V6(p) => peers6.push(p),
                        }
                    }

                    peers.sort();
                    peers6.sort();

                    let (complete, incomplete) = data.torrent_store.get_announce_stats(parsed_req.info_hash);

                    let response = AnnounceResponse::new(INTERVAL, complete, incomplete, peers, peers6);
                    let bencoded = bencode::encode_announce_response(response.unwrap());
                    fut_ok(HttpResponse::Ok().content_type("text/plain").body(bencoded))
                }
                Event::Stopped => {
                    data.peer_store.remove_seeder(parsed_req.info_hash.clone(), parsed_req.peer.clone());
                    data.peer_store.remove_leecher(parsed_req.info_hash.clone(), parsed_req.peer);

                    let peer_list =
                        data.peer_store.get_peers(parsed_req.info_hash.clone(), parsed_req.numwant.unwrap());
                    let mut peers = Vec::new();
                    let mut peers6 = Vec::new();

                    for peer in peer_list {
                        match peer {
                            Peer::V4(p) => peers.push(p),
                            Peer::V6(p) => peers6.push(p),
                        }
                    }

                    peers.sort();
                    peers6.sort();

                    let (complete, incomplete) = data.torrent_store.get_announce_stats(parsed_req.info_hash);

                    let response = AnnounceResponse::new(INTERVAL, complete, incomplete, peers, peers6);
                    let bencoded = bencode::encode_announce_response(response.unwrap());
                    fut_ok(HttpResponse::Ok().content_type("text/plain").body(bencoded))
                }
                Event::Completed => {
                    data.peer_store.promote_leecher(parsed_req.info_hash.clone(), parsed_req.peer);

                    let peer_list =
                        data.peer_store.get_peers(parsed_req.info_hash.clone(), parsed_req.numwant.unwrap());
                    let mut peers = Vec::new();
                    let mut peers6 = Vec::new();

                    for peer in peer_list {
                        match peer {
                            Peer::V4(p) => peers.push(p),
                            Peer::V6(p) => peers6.push(p),
                        }
                    }

                    peers.sort();
                    peers6.sort();

                    let (complete, incomplete) = data.torrent_store.get_announce_stats(parsed_req.info_hash);

                    let response = AnnounceResponse::new(INTERVAL, complete, incomplete, peers, peers6);
                    let bencoded = bencode::encode_announce_response(response.unwrap());
                    fut_ok(HttpResponse::Ok().content_type("text/plain").body(bencoded))
                }
                Event::None => {
                    // This is just a way to ensure that a leecher is added if
                    // the client doesn't send an event
                    data.peer_store.put_leecher(parsed_req.info_hash.clone(), parsed_req.peer);

                    let peer_list =
                        data.peer_store.get_peers(parsed_req.info_hash.clone(), parsed_req.numwant.unwrap());
                    let mut peers = Vec::new();
                    let mut peers6 = Vec::new();

                    for peer in peer_list {
                        match peer {
                            Peer::V4(p) => peers.push(p),
                            Peer::V6(p) => peers6.push(p),
                        }
                    }

                    peers.sort();
                    peers6.sort();

                    let (complete, incomplete) = data.torrent_store.get_announce_stats(parsed_req.info_hash);

                    let response = AnnounceResponse::new(INTERVAL, complete, incomplete, peers, peers6);
                    let bencoded = bencode::encode_announce_response(response.unwrap());
                    fut_ok(HttpResponse::Ok().content_type("text/plain").body(bencoded))
                }
            }
        }

        // If the request is not parse-able, short-circuit and respond with failure
        Err(failure) => {
            let bencoded = bencode::encode_announce_response(failure);
            fut_ok(HttpResponse::Ok().content_type("text/plain").body(bencoded))
        }
    }
}

pub fn parse_scrape(
    data: web::Data<Stores>,
    req: HttpRequest,
) -> impl Future<Item = HttpResponse, Error = Error> {
    let scrape_request = ScrapeRequest::new(req.query_string());
    match scrape_request {
        Ok(parsed_req) => {
            let scrape_files = data.torrent_store.get_scrapes(parsed_req.info_hashes);
            let mut scrape_response = ScrapeResponse::new().unwrap();

            for file in scrape_files {
                scrape_response.add_file(file.info_hash.clone(), file);
            }

            let bencoded = bencode::encode_scrape_response(scrape_response);
            fut_ok(HttpResponse::Ok().content_type("text/plain").body(bencoded))
        }

        Err(failure) => {
            let bencoded = bencode::encode_scrape_response(failure);
            fut_ok(HttpResponse::Ok().content_type("text/plain").body(bencoded))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use actix_web::dev::Service;
    use actix_web::{guard, test, web, App, HttpResponse};

    use crate::bittorrent::{Peerv4, Peerv6};
    use crate::storage::{PeerStore, Stores, Torrent, TorrentMemoryStore};
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn index_get_not_allowed() {
        let stores = web::Data::new(Stores::new("test".to_string()));
        let mut app = test::init_service(
            App::new()
                .register_data(stores.clone())
                .service(
                    web::resource("announce")
                        .guard(guard::Header("content-type", "text/plain"))
                        .route(web::get().to_async(parse_announce)),
                )
                .service(
                    web::resource("scrape")
                        .guard(guard::Header("content-type", "text/plain"))
                        .route(web::get().to_async(parse_scrape)),
                )
                .default_service(web::route().to(HttpResponse::MethodNotAllowed)),
        );
        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::block_on(app.call(req)).unwrap();

        assert!(resp.status().is_client_error());
    }

    #[test]
    fn announce_get_malformed() {
        let stores = Stores::new("test".to_string());
        let data = web::Data::new(stores);

        let app = test::init_service(
            App::new()
                .register_data(data.clone())
                .service(
                    web::resource("announce")
                        .guard(guard::Header("content-type", "text/plain"))
                        .route(web::get().to_async(parse_announce)),
                )
                .service(
                    web::resource("scrape")
                        .guard(guard::Header("content-type", "text/plain"))
                        .route(web::get().to_async(parse_scrape)),
                )
                .default_service(web::route().to(HttpResponse::MethodNotAllowed)),
        );

        let proper_resp = HttpResponse::Ok()
            .content_type("text/plain")
            .body("d14:failure_reason17:Malformed requeste".as_bytes());
        let req = test::TestRequest::get()
            .uri("/announce?bad_stuff=123")
            .to_http_request();
        let resp = test::block_on(parse_announce(data, req)).unwrap();

        assert_eq!(
            resp.body().as_ref().unwrap(),
            proper_resp.body().as_ref().unwrap()
        );
    }

    #[test]
    fn scrape_get_malformed() {
        let stores = Stores::new("test".to_string());
        let data = web::Data::new(stores);

        let app = test::init_service(
            App::new()
                .service(
                    web::scope("/announce")
                        .register_data(data.clone())
                        .guard(guard::Header("content-type", "text/plain"))
                        .route("/", web::get().to_async(parse_announce)),
                )
                .service(
                    web::scope("/scrape")
                        .register_data(data.clone())
                        .guard(guard::Header("content-type", "text/plain"))
                        .route("/", web::get().to_async(parse_scrape)),
                )
                .default_service(web::route().to(HttpResponse::MethodNotAllowed)),
        );

        let proper_resp = HttpResponse::Ok()
            .content_type("text/plain")
            .body("d14:failure_reason24:Malformed scrape requeste".as_bytes());
        let req = test::TestRequest::get()
            .uri("/scrape?bad_stuff=123")
            .to_http_request();
        let resp = test::block_on(parse_scrape(data, req)).unwrap();

        assert_eq!(
            resp.body().as_ref().unwrap(),
            proper_resp.body().as_ref().unwrap()
        );
    }

    #[test]
    fn scrape_get_success() {
        let stores = Stores::new("test".to_string());

        let info_hash1 = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let torrent1 = Torrent::new(info_hash1, 10, 34, 7, 10000000);

        let info_hash2 = "B2C3D4E5F6G7H8I9J0K1".to_string();
        let torrent2 = Torrent::new(info_hash2, 25, 57, 19, 20000000);

        {
            let mut store = stores.torrent_store.torrents.write();
            store.insert(torrent1.info_hash.clone(), torrent1);
            store.insert(torrent2.info_hash.clone(), torrent2);
        }

        let data = web::Data::new(stores);

        let app = test::init_service(
            App::new()
                .service(
                    web::scope("/announce")
                        .data(data.clone())
                        .guard(guard::Header("content-type", "text/plain"))
                        .route("/", web::get().to_async(parse_announce)),
                )
                .service(
                    web::scope("/scrape")
                        .data(data.clone())
                        .guard(guard::Header("content-type", "text/plain"))
                        .route("/", web::get().to_async(parse_scrape)),
                )
                .default_service(web::route().to(HttpResponse::MethodNotAllowed)),
        );

        let uri = "/scrape?info_hash=A1B2C3D4E5F6G7H8I9J0\
                   &info_hash=B2C3D4E5F6G7H8I9J0K1";

        let proper_resp = HttpResponse::Ok().content_type("text/plain").body("d5:filesd20:A1B2C3D4E5F6G7H8I9J0d8:completei10e10:downloadedi34e10:incompletei7ee20:B2C3D4E5F6G7H8I9J0K1d8:completei25e10:downloadedi57e10:incompletei19eeee".as_bytes());
        let req = test::TestRequest::get().uri(uri).to_http_request();
        let resp = test::block_on(parse_scrape(data, req)).unwrap();

        assert_eq!(
            resp.body().as_ref().unwrap(),
            proper_resp.body().as_ref().unwrap()
        );
    }
}
