use std::sync::Arc;

use actix_web::{web, HttpRequest, HttpResponse};

use crate::bencode;
use crate::bittorrent::{AnnounceRequest, ScrapeRequest};
use crate::storage::PeerStore;

pub fn parse_announce(req: HttpRequest) -> HttpResponse {
    let announce_request = AnnounceRequest::new(req.query_string(), req.connection_info().remote());

    match announce_request {
        // If the request is not parse-able, short-circuit and respond with failure
        Ok(req) => HttpResponse::Ok()
            .content_type("text/plain")
            .body("I'm working here!"),
        Err(failure) => {
            let bencoded = bencode::encode_announce_response(failure);
            HttpResponse::Ok().content_type("text/plain").body(bencoded)
        }
    }
}

pub fn parse_scrape(data: web::Data<Arc<PeerStore>>, req: HttpRequest) {
    let scrape_request = ScrapeRequest::new(req.query_string());
}
