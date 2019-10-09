use actix_web::{web, HttpRequest, HttpResponse};

use crate::bencode;
use crate::bittorrent::{AnnounceRequest, AnnounceResponse, Peer, ScrapeRequest};
use crate::storage::{PeerStorage, PeerStore};

pub fn parse_announce(data: web::Data<PeerStore>, req: HttpRequest) -> HttpResponse {
    let announce_request = AnnounceRequest::new(req.query_string(), req.connection_info().remote());

    match announce_request {
        Ok(parsed_req) => {

            let peer_list = data.get_peers(parsed_req.info_hash, parsed_req.numwant.unwrap());
            let mut peers = Vec::new();
            let mut peers6 = Vec::new();

            for peer in peer_list {
                match peer {
                    Peer::V4(p) => peers.push(p),
                    Peer::V6(p) => peers6.push(p)
                }
            };

            // Dummy values
            let response = AnnounceResponse::new(30, 23, 1, peers, peers6);
            let bencoded = bencode::encode_announce_response(response.unwrap());
            HttpResponse::Ok().content_type("text/plain").body(bencoded)
        }

        // If the request is not parse-able, short-circuit and respond with failure
        Err(failure) => {
            let bencoded = bencode::encode_announce_response(failure);
            HttpResponse::Ok().content_type("text/plain").body(bencoded)
        }
    }
}

pub fn parse_scrape(data: web::Data<PeerStore>, req: HttpRequest) {
    let scrape_request = ScrapeRequest::new(req.query_string());
}
