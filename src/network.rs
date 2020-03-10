use actix_web::{web, HttpRequest, HttpResponse, Responder};

use crate::bencode;
use crate::bittorrent::{AnnounceRequest, AnnounceResponse, Peer, ScrapeRequest, ScrapeResponse};
use crate::storage::Stores;
use crate::util::Event;

// This will eventually be read from the configuration YAML.
const INTERVAL: u32 = 3600;

pub async fn parse_announce(data: web::Data<Stores>, req: HttpRequest) -> impl Responder {
    let announce_request = AnnounceRequest::new(req.query_string(), req.connection_info().remote());

    match announce_request {
        Ok(parsed_req) => {
            match parsed_req.event {
                Event::Started => {
                    data.peer_store
                        .put_leecher(parsed_req.info_hash.clone(), parsed_req.peer);
                    data.torrent_store.new_leech(parsed_req.info_hash.clone());

                    let peer_list = data
                        .peer_store
                        .get_peers(parsed_req.info_hash.clone(), parsed_req.numwant.unwrap());
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

                    let (complete, incomplete) =
                        data.torrent_store.get_announce_stats(parsed_req.info_hash);

                    let response =
                        AnnounceResponse::new(INTERVAL, complete, incomplete, peers, peers6);
                    let bencoded = bencode::encode_announce_response(response.unwrap());
                    HttpResponse::Ok().content_type("text/plain").body(bencoded)
                }
                Event::Stopped => {
                    // TODO: Need to make sure that peer is decremented from whichever swarm it
                    // came from
                    data.peer_store
                        .remove_seeder(parsed_req.info_hash.clone(), parsed_req.peer.clone());
                    data.peer_store
                        .remove_leecher(parsed_req.info_hash.clone(), parsed_req.peer);

                    let peer_list = data
                        .peer_store
                        .get_peers(parsed_req.info_hash.clone(), parsed_req.numwant.unwrap());
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

                    let (complete, incomplete) =
                        data.torrent_store.get_announce_stats(parsed_req.info_hash);

                    let response =
                        AnnounceResponse::new(INTERVAL, complete, incomplete, peers, peers6);
                    let bencoded = bencode::encode_announce_response(response.unwrap());
                    HttpResponse::Ok().content_type("text/plain").body(bencoded)
                }
                Event::Completed => {
                    data.peer_store
                        .promote_leecher(parsed_req.info_hash.clone(), parsed_req.peer);
                    data.torrent_store.new_seed(parsed_req.info_hash.clone());

                    let peer_list = data
                        .peer_store
                        .get_peers(parsed_req.info_hash.clone(), parsed_req.numwant.unwrap());
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

                    let (complete, incomplete) =
                        data.torrent_store.get_announce_stats(parsed_req.info_hash);

                    let response =
                        AnnounceResponse::new(INTERVAL, complete, incomplete, peers, peers6);
                    let bencoded = bencode::encode_announce_response(response.unwrap());
                    HttpResponse::Ok().content_type("text/plain").body(bencoded)
                }
                Event::None => {
                    // This is just a way to ensure that a leecher is added if
                    // the client doesn't send an event
                    data.peer_store
                        .put_leecher(parsed_req.info_hash.clone(), parsed_req.peer);

                    let peer_list = data
                        .peer_store
                        .get_peers(parsed_req.info_hash.clone(), parsed_req.numwant.unwrap());
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

                    let (complete, incomplete) =
                        data.torrent_store.get_announce_stats(parsed_req.info_hash);

                    let response =
                        AnnounceResponse::new(INTERVAL, complete, incomplete, peers, peers6);
                    let bencoded = bencode::encode_announce_response(response.unwrap());
                    HttpResponse::Ok().content_type("text/plain").body(bencoded)
                }
            }
        }

        // If the request is not parse-able, short-circuit and respond with failure
        Err(failure) => {
            let bencoded = bencode::encode_announce_response(failure);
            HttpResponse::Ok().content_type("text/plain").body(bencoded)
        }
    }
}

pub async fn parse_scrape(data: web::Data<Stores>, req: HttpRequest) -> impl Responder {
    let scrape_request = ScrapeRequest::new(req.query_string());
    match scrape_request {
        Ok(parsed_req) => {
            let scrape_files = data.torrent_store.get_scrapes(parsed_req.info_hashes);
            let mut scrape_response = ScrapeResponse::new().unwrap();

            for file in scrape_files {
                scrape_response.add_file(file.info_hash.clone(), file);
            }

            let bencoded = bencode::encode_scrape_response(scrape_response);
            HttpResponse::Ok().content_type("text/plain").body(bencoded)
        }

        Err(failure) => {
            let bencoded = bencode::encode_scrape_response(failure);
            HttpResponse::Ok().content_type("text/plain").body(bencoded)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use actix_service::Service;
    use actix_web::{http::StatusCode, test, web, App, HttpResponse};

    use crate::bittorrent::{Peerv4, Peerv6};
    use crate::storage::{PeerStore, Stores, Torrent, TorrentMemoryStore};
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[actix_rt::test]
    async fn index_get_not_allowed() {
        let stores = web::Data::new(Stores::new("test".to_string()));
        let mut app = test::init_service(
            App::new()
                .service(
                    web::scope("announce")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_announce)),
                )
                .service(
                    web::scope("scrape")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_scrape)),
                )
                .service(
                    web::scope("/").route("", web::get().to(|| HttpResponse::MethodNotAllowed())),
                ),
        )
        .await;

        let req = test::TestRequest::with_uri("/").to_request();
        let resp = app.call(req).await.unwrap();

        assert!(resp.status().is_client_error());
    }

    #[actix_rt::test]
    async fn announce_get_malformed() {
        let stores = web::Data::new(Stores::new("test".to_string()));
        let mut app = test::init_service(
            App::new()
                .service(
                    web::scope("announce")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_announce)),
                )
                .service(
                    web::scope("scrape")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_scrape)),
                )
                .service(
                    web::scope("/").route("", web::get().to(|| HttpResponse::MethodNotAllowed())),
                ),
        )
        .await;

        let proper_resp = "d14:failure_reason17:Malformed requeste".as_bytes();
        let req = test::TestRequest::with_uri("/announce?bad_stuff=123").to_request();
        let resp = test::read_response(&mut app, req).await;

        assert_eq!(resp, proper_resp);
    }

    #[actix_rt::test]
    async fn scrape_get_malformed() {
        let stores = web::Data::new(Stores::new("test".to_string()));
        let mut app = test::init_service(
            App::new()
                .service(
                    web::scope("announce")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_announce)),
                )
                .service(
                    web::scope("scrape")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_scrape)),
                )
                .service(
                    web::scope("/").route("", web::get().to(|| HttpResponse::MethodNotAllowed())),
                ),
        )
        .await;

        let proper_resp = "d14:failure_reason24:Malformed scrape requeste".as_bytes();
        let req = test::TestRequest::with_uri("/scrape?bad_stuff=123").to_request();
        let resp = test::read_response(&mut app, req).await;

        assert_eq!(resp, proper_resp);
    }

    #[actix_rt::test]
    async fn scrape_get_success() {
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

        let mut app = test::init_service(
            App::new()
                .service(
                    web::scope("announce")
                        .app_data(data.clone())
                        .route("", web::get().to(parse_announce)),
                )
                .service(
                    web::scope("scrape")
                        .app_data(data.clone())
                        .route("", web::get().to(parse_scrape)),
                )
                .service(
                    web::scope("/").route("", web::get().to(|| HttpResponse::MethodNotAllowed())),
                ),
        )
        .await;

        let uri = "/scrape?info_hash=A1B2C3D4E5F6G7H8I9J0\
                   &info_hash=B2C3D4E5F6G7H8I9J0K1";

        let proper_resp = "d5:filesd20:A1B2C3D4E5F6G7H8I9J0d8:completei10e10:downloadedi34e10:incompletei7ee20:B2C3D4E5F6G7H8I9J0K1d8:completei25e10:downloadedi57e10:incompletei19eeee".as_bytes();
        let req = test::TestRequest::with_uri(uri).to_request();
        let resp = test::read_response(&mut app, req).await;

        assert_eq!(resp, proper_resp);
    }
}
