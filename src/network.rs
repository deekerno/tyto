use actix_web::{web, Error, HttpRequest, HttpResponse};
use futures::{future::ok as fut_ok, Future};

use crate::bencode;
use crate::bittorrent::{AnnounceRequest, AnnounceResponse, Peer, ScrapeResponse, ScrapeRequest};
use crate::storage::{PeerStorage, PeerStore, TorrentMemoryStore, TorrentStorage};

// This will eventually be read from the configuration YAML.
const INTERVAL: u32 = 60;

pub fn parse_announce(data: web::Data<PeerStore>, req: HttpRequest) -> impl Future<Item = HttpResponse, Error = Error> 
{
    let announce_request = AnnounceRequest::new(req.query_string(), req.connection_info().remote());

    match announce_request {
        Ok(parsed_req) => {
            let peer_list = data.get_peers(parsed_req.info_hash, parsed_req.numwant.unwrap());
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

            // Dummy values, the actuals will come from the torrent storage
            let response = AnnounceResponse::new(INTERVAL, 100, 23, peers, peers6);
            let bencoded = bencode::encode_announce_response(response.unwrap());
            fut_ok(HttpResponse::Ok().content_type("text/plain").body(bencoded))
        }

        // If the request is not parse-able, short-circuit and respond with failure
        Err(failure) => {
            let bencoded = bencode::encode_announce_response(failure);
            fut_ok(HttpResponse::Ok().content_type("text/plain").body(bencoded))
        }
    }
}

pub fn parse_scrape(data: web::Data<TorrentMemoryStore>, req: HttpRequest) -> impl Future<Item = HttpResponse, Error = Error> 
{
    let scrape_request = ScrapeRequest::new(req.query_string());
    match scrape_request {
        Ok(parsed_req) => {
            let scrape_files = data.get_scrapes(parsed_req.info_hashes);
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
    use crate::storage::{PeerStorage, PeerStore, Torrent, TorrentStorage, TorrentMemoryStore};
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn index_get_not_allowed() {
        let data = web::Data::new(PeerStore::new().unwrap());
        let mut app = test::init_service(
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
        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::block_on(app.call(req)).unwrap();

        assert!(resp.status().is_client_error());
    }

    #[test]
    fn announce_get_malformed() {
        let peer_store = PeerStore::new().unwrap();
        let data = web::Data::new(peer_store);

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
    fn announce_get_success() {
        let peer_store = PeerStore::new().unwrap();

        let info_hash = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let peerv4_1 = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
        });

        let peerv4_2 = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::BROADCAST,
            port: 6894,
        });

        peer_store.put_seeder(info_hash.clone(), peerv4_2.clone());
        peer_store.put_seeder(info_hash.clone(), peerv4_1.clone());

        let peerv6_1 = Peer::V6(Peerv6 {
            peer_id: "ABCDEFGHIJKLMNOPABCD".to_string(),
            ip: Ipv6Addr::new(
                0x2001, 0x0db8, 0x85a3, 0x0000, 0x0000, 0x8a2e, 0x0370, 0x7334,
            ),
            port: 6681,
        });

        peer_store.put_seeder(info_hash.clone(), peerv6_1.clone());

        let peerv6_2 = Peer::V6(Peerv6 {
            peer_id: "ABCDEFGHIJKLMNOPZZZZ".to_string(),
            ip: Ipv6Addr::new(
                0xfe80, 0x0000, 0x0000, 0x0000, 0x0202, 0xb3ff, 0xfe1e, 0x8329,
            ),
            port: 6699,
        });

        peer_store.put_seeder(info_hash.clone(), peerv6_2.clone());

        let data = web::Data::new(peer_store);
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

        let uri = "/announce?info_hash=A1B2C3D4E5F6G7H8I9J0\
                   &peer_id=ABCDEFGHIJKLMNOPQRST&port=6881&uploaded=0&downloaded=0\
                   &left=727955456&event=started&numwant=100&no_peer_id=1&compact=1";

        let proper_resp = HttpResponse::Ok().content_type("text/plain").body("d8:completei100e10:incompletei23e8:intervali60e5:peersli127ei0ei0ei1ei26ei237ei255ei255ei255ei255ei26ei238ee6:peers6li32ei1ei13ei184ei133ei163ei0ei0ei0ei0ei138ei46ei3ei112ei115ei52ei26ei25ei254ei128ei0ei0ei0ei0ei0ei0ei2ei2ei179ei255ei254ei30ei131ei41ei26ei43ee10:tracker_id0:e".as_bytes());
        let req = test::TestRequest::get().uri(uri).to_http_request();
        let resp = test::block_on(parse_announce(data, req)).unwrap();

        assert_eq!(
            resp.body().as_ref().unwrap(),
            proper_resp.body().as_ref().unwrap()
        );
    }

    #[test]
    fn scrape_get_malformed() {
        let peer_storage = PeerStore::new().unwrap();
        let torrent_storage = TorrentMemoryStore::new("".to_string()).unwrap();
        let peer_data = web::Data::new(peer_storage);
        let torrent_data = web::Data::new(torrent_storage);

        let app = test::init_service(
            App::new()
                .service(
                    web::scope("/announce")
                        .data(peer_data.clone())
                        .guard(guard::Header("content-type", "text/plain"))
                        .route("/", web::get().to_async(parse_announce)),
                )
                .service(
                    web::scope("/scrape")
                        .data(torrent_data.clone())
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
        let resp = test::block_on(parse_scrape(torrent_data, req)).unwrap();

        assert_eq!(
            resp.body().as_ref().unwrap(),
            proper_resp.body().as_ref().unwrap()
        );
    }

    #[test]
    fn scrape_get_success() {
        let torrent_storage = TorrentMemoryStore::new("".to_string()).unwrap();

        let info_hash1 = "A1B2C3D4E5F6G7H8I9J0".to_string();
        let torrent1 = Torrent::new(info_hash1, 10, 34, 7, 10000000);

        let info_hash2 = "B2C3D4E5F6G7H8I9J0K1".to_string();
        let torrent2 = Torrent::new(info_hash2, 25, 57, 19, 20000000);

        {
            let mut store = torrent_storage.torrents.write();
            store.insert(torrent1.info_hash.clone(), torrent1);
            store.insert(torrent2.info_hash.clone(), torrent2);
        }

        let peer_storage = PeerStore::new().unwrap();
        let peer_data = web::Data::new(peer_storage);
        let torrent_data = web::Data::new(torrent_storage);

        let app = test::init_service(
            App::new()
                .service(
                    web::scope("/announce")
                        .data(peer_data.clone())
                        .guard(guard::Header("content-type", "text/plain"))
                        .route("/", web::get().to_async(parse_announce)),
                )
                .service(
                    web::scope("/scrape")
                        .data(torrent_data.clone())
                        .guard(guard::Header("content-type", "text/plain"))
                        .route("/", web::get().to_async(parse_scrape)),
                )
                .default_service(web::route().to(HttpResponse::MethodNotAllowed)),
        );

        let uri = "/scrape?info_hash=A1B2C3D4E5F6G7H8I9J0\
                   &info_hash=B2C3D4E5F6G7H8I9J0K1";

        let proper_resp = HttpResponse::Ok().content_type("text/plain").body("d5:filesd20:A1B2C3D4E5F6G7H8I9J0d8:completei10e10:downloadedi34e10:incompletei7ee20:B2C3D4E5F6G7H8I9J0K1d8:completei25e10:downloadedi57e10:incompletei19eeee".as_bytes());
        let req = test::TestRequest::get().uri(uri).to_http_request();
        let resp = test::block_on(parse_scrape(torrent_data, req)).unwrap();

        assert_eq!(
            resp.body().as_ref().unwrap(),
            proper_resp.body().as_ref().unwrap()
        );
    }
}
