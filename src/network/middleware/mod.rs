use std::task::{Context, Poll};

use actix_service::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{Error, HttpResponse};
use futures::future::{ok, Either, Ready};
use hashbrown::HashSet;
use url::form_urlencoded;

use crate::bencode;
use crate::bittorrent::AnnounceResponse;

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
            match k.as_str() {
                "peer_id" => peer_string = value,
                _ => {}
            }
        }

        if peer_string.is_empty() {
            let failure = AnnounceResponse::failure("Unsupported Client".to_string());
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
            let client_check = match self.versioned {
                true => &peer_string[1..7],
                false => &peer_string[1..3],
            };

            if self.blacklist_style {
                // Check that client isn't part of blacklist.
                // If so, reject with same error as above.
                // If not, let the request pass through.
                if self.list.contains(&client_check.to_string()) {
                    let failure = AnnounceResponse::failure("Unsupported Client".to_string());
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
                    let failure = AnnounceResponse::failure("Unsupported Client".to_string());
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
