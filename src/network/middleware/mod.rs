use std::task::{Context, Poll};

use actix_service::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{Error, HttpResponse};
use futures::future::{ok, Either, Ready};
use hashbrown::HashSet;
use url::form_urlencoded;

use crate::bencode;
use crate::bittorrent::AnnounceResponse;
use crate::errors::ClientError;

pub struct ClientApproval {
    blacklist_style: bool,
    versioned: bool,
    list: HashSet<String>,
}

impl ClientApproval {
    pub fn new(blacklist_style: bool, versioned: bool, client_list: Vec<String>) -> Self {
        ClientApproval {
            blacklist_style,
            versioned,
            list: client_list.into_iter().collect(),
        }
    }
}

impl<S, B> Transform<S> for ClientApproval
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = ClientApprovalMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(ClientApprovalMiddleware {
            service,
            blacklist_style: self.blacklist_style,
            versioned: self.versioned,
            list: self.list.clone(),
        })
    }
}
pub struct ClientApprovalMiddleware<S> {
    service: S,
    blacklist_style: bool,
    versioned: bool,
    list: HashSet<String>,
}

impl<S, B> Service for ClientApprovalMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Either<S::Future, Ready<Result<Self::Response, Self::Error>>>;

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let request_kv_pairs = form_urlencoded::parse(req.query_string().as_bytes()).into_owned();
        let mut peer_string: String = "".to_string();

        for (k, value) in request_kv_pairs {
            if let "peer_id" = k.as_str() {
                peer_string = value
            }
        }

        // If a client's peer string is empty, this is a Bad Thing
        if peer_string.is_empty() {
            let failure = AnnounceResponse::failure(ClientError::UnapprovedClient.text());
            let bencoded = bencode::encode_announce_response(failure);
            Either::Right(ok(req.into_response(
                HttpResponse::Ok()
                    .content_type("text/plain")
                    .body(bencoded)
                    .into_body(),
            )))
        } else {
            // Most clients do Azureus-style encoding which
            // looks like '-AZ1234-' followed by a random string
            let client_check = if self.versioned {
                &peer_string[1..7]
            } else {
                &peer_string[1..3]
            };

            if self.blacklist_style {
                // Check that client isn't part of blacklist.
                // If so, reject with same error as above.
                // If not, let the request pass through.
                if self.list.contains(&client_check.to_string()) {
                    let failure = AnnounceResponse::failure(ClientError::UnapprovedClient.text());
                    let bencoded = bencode::encode_announce_response(failure);
                    Either::Right(ok(req.into_response(
                        HttpResponse::Ok()
                            .content_type("text/plain")
                            .body(bencoded)
                            .into_body(),
                    )))
                } else {
                    Either::Left(self.service.call(req))
                }
            } else {
                // Check that client is part of whitelist.
                // If so, let the request pass through.
                // If not, reject with same error as above.
                if self.list.contains(&client_check.to_string()) {
                    Either::Left(self.service.call(req))
                } else {
                    let failure = AnnounceResponse::failure(ClientError::UnapprovedClient.text());
                    let bencoded = bencode::encode_announce_response(failure);
                    Either::Right(ok(req.into_response(
                        HttpResponse::Ok()
                            .content_type("text/plain")
                            .body(bencoded)
                            .into_body(),
                    )))
                }
            }
        }
    }
}

pub struct TorrentApproval {
    prohibited_list: HashSet<String>,
}

impl TorrentApproval {
    pub fn new(prohibited_list: Vec<String>) -> Self {
        TorrentApproval {
            prohibited_list: prohibited_list.into_iter().collect(),
        }
    }
}

impl<S, B> Transform<S> for TorrentApproval
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = TorrentApprovalMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(TorrentApprovalMiddleware {
            service,
            prohibited_list: self.prohibited_list.clone(),
        })
    }
}
pub struct TorrentApprovalMiddleware<S> {
    service: S,
    prohibited_list: HashSet<String>,
}

impl<S, B> Service for TorrentApprovalMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Either<S::Future, Ready<Result<Self::Response, Self::Error>>>;

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let request_kv_pairs = form_urlencoded::parse(req.query_string().as_bytes()).into_owned();
        let mut info_hash: String = "".to_string();

        for (k, value) in request_kv_pairs {
            if let "info_hash" = k.as_str() {
                info_hash = value
            }
        }

        // If a client's peer string is empty, this is a Bad Thing
        if self.prohibited_list.contains(&info_hash) {
            let failure = AnnounceResponse::failure(ClientError::UnapprovedTorrent.text());
            let bencoded = bencode::encode_announce_response(failure);
            Either::Right(ok(req.into_response(
                HttpResponse::Ok()
                    .content_type("text/plain")
                    .body(bencoded)
                    .into_body(),
            )))
        } else {
            Either::Left(self.service.call(req))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use actix_web::{test, web, App};

    use crate::config::Config;
    use crate::network::parse_announce;
    use crate::state::State;
    use crate::storage::{TorrentRecords, TorrentStore};

    #[actix_rt::test]
    async fn client_blacklist_non_versioned() {
        let config = Config::default();
        let torrent_store = TorrentStore::new(TorrentRecords::new());
        let stores = web::Data::new(State::new(config, torrent_store));
        let blacklist_style = true;
        let versioned = false;
        let client_list = vec![
            "DE".to_string(),
            "LT".to_string(),
            "qB".to_string(),
            "TR".to_string(),
            "UT".to_string(),
        ];

        let mut app = test::init_service(
            App::new()
                .wrap(ClientApproval::new(blacklist_style, versioned, client_list))
                .service(
                    web::scope("announce")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_announce)),
                ),
        )
        .await;

        let proper_resp = "d14:failure_reason17:Unapproved cliente".as_bytes();
        let req = test::TestRequest::with_uri("/announce?info_hash=2fa90c59c8072c5a4c54c1f1307dacaeb4c82f0f&peer_id=-DE9824-143964258012&port=6881&uploaded=9000&downloaded=1000&left=727955456&numwant=30&no_peer_id=1&compact=1").to_request();
        let resp = test::read_response(&mut app, req).await;

        assert_eq!(resp, proper_resp);
    }

    #[actix_rt::test]
    async fn client_blacklist_versioned() {
        let config = Config::default();
        let torrent_store = TorrentStore::new(TorrentRecords::new());
        let stores = web::Data::new(State::new(config, torrent_store));
        let blacklist_style = true;
        let versioned = true;
        let client_list = vec![
            "DE9824".to_string(),
            "LT1111".to_string(),
            "qB2222".to_string(),
            "TR3333".to_string(),
            "UT4444".to_string(),
        ];

        let mut app = test::init_service(
            App::new()
                .wrap(ClientApproval::new(blacklist_style, versioned, client_list))
                .service(
                    web::scope("announce")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_announce)),
                ),
        )
        .await;

        let proper_resp = "d14:failure_reason17:Unapproved cliente".as_bytes();
        let req = test::TestRequest::with_uri("/announce?info_hash=2fa90c59c8072c5a4c54c1f1307dacaeb4c82f0f&peer_id=-DE9824-143964258012&port=6881&uploaded=9000&downloaded=1000&left=727955456&numwant=30&no_peer_id=1&compact=1").to_request();
        let resp = test::read_response(&mut app, req).await;

        assert_eq!(resp, proper_resp);
    }

    #[actix_rt::test]
    async fn client_whitelist_non_versioned() {
        let config = Config::default();
        let torrent_store = TorrentStore::new(TorrentRecords::new());
        let stores = web::Data::new(State::new(config, torrent_store));
        let blacklist_style = false;
        let versioned = false;
        let client_list = vec![
            "DE".to_string(),
            "LT".to_string(),
            "qB".to_string(),
            "TR".to_string(),
            "UT".to_string(),
        ];

        let mut app = test::init_service(
            App::new()
                .wrap(ClientApproval::new(blacklist_style, versioned, client_list))
                .service(
                    web::scope("announce")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_announce)),
                ),
        )
        .await;

        let proper_resp = "d14:failure_reason17:Unapproved cliente".as_bytes();
        let req = test::TestRequest::with_uri("/announce?info_hash=2fa90c59c8072c5a4c54c1f1307dacaeb4c82f0f&peer_id=-AZ9824-143964258012&port=6881&uploaded=9000&downloaded=1000&left=727955456&numwant=30&no_peer_id=1&compact=1").to_request();
        let resp = test::read_response(&mut app, req).await;

        assert_eq!(resp, proper_resp);
    }

    #[actix_rt::test]
    async fn client_whitelist_versioned() {
        let config = Config::default();
        let torrent_store = TorrentStore::new(TorrentRecords::new());
        let stores = web::Data::new(State::new(config, torrent_store));
        let blacklist_style = false;
        let versioned = true;
        let client_list = vec![
            "DE1111".to_string(),
            "LT2222".to_string(),
            "qB3333".to_string(),
            "TR4444".to_string(),
            "UT5555".to_string(),
        ];

        let mut app = test::init_service(
            App::new()
                .wrap(ClientApproval::new(blacklist_style, versioned, client_list))
                .service(
                    web::scope("announce")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_announce)),
                ),
        )
        .await;

        let proper_resp = "d14:failure_reason17:Unapproved cliente".as_bytes();
        let req = test::TestRequest::with_uri("/announce?info_hash=2fa90c59c8072c5a4c54c1f1307dacaeb4c82f0f&peer_id=-DE0000-143964258012&port=6881&uploaded=9000&downloaded=1000&left=727955456&numwant=30&no_peer_id=1&compact=1").to_request();
        let resp = test::read_response(&mut app, req).await;

        assert_eq!(resp, proper_resp);
    }

    #[actix_rt::test]
    async fn torrent_blacklist() {
        let config = Config::default();
        let torrent_store = TorrentStore::new(TorrentRecords::new());
        let stores = web::Data::new(State::new(config, torrent_store));
        let prohibited_list = vec![
            "2fa90c59c8072c5a4c54c1f1307dacaeb4c82f0f".to_string(),
            "3bbc36a0bcae854bd40c4deec639d4afadf65deb".to_string(),
            "8a541fa2db56003884b0acf9c059f6652d5f611c".to_string(),
        ];

        let mut app = test::init_service(
            App::new()
                .wrap(TorrentApproval::new(prohibited_list))
                .service(
                    web::scope("announce")
                        .app_data(stores.clone())
                        .route("", web::get().to(parse_announce)),
                ),
        )
        .await;

        let proper_resp = "d14:failure_reason18:Unapproved torrente".as_bytes();
        let req = test::TestRequest::with_uri("/announce?info_hash=2fa90c59c8072c5a4c54c1f1307dacaeb4c82f0f&peer_id=-DE0000-143964258012&port=6881&uploaded=9000&downloaded=1000&left=727955456&numwant=30&no_peer_id=1&compact=1").to_request();
        let resp = test::read_response(&mut app, req).await;

        assert_eq!(resp, proper_resp);
    }
}
