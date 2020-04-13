// This package contains structures specific to the BitTorrent protocol.
// Most of the information is coming from the following link:
// https://wiki.theory.org/index.php/BitTorrentSpecification

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Instant;

use bytes::BufMut;
use percent_encoding;
use regex::Regex;
use url::form_urlencoded;

use crate::util::{string_to_event, Event};

trait Compact {
    fn compact(&self) -> Vec<u8>;
}

// These two peer types could probably be implemented more elegantly
// with a trait, but there's only two types right now, so it's not a lot of work
#[derive(Clone, Eq, Ord, PartialOrd, Debug)]
pub struct Peerv4 {
    pub peer_id: String, // This should be 20 bytes in length
    pub ip: Ipv4Addr,
    pub port: u16,
    pub last_announced: Instant,
}

#[derive(Clone, Eq, Ord, PartialOrd, Debug)]
pub struct Peerv6 {
    pub peer_id: String, // This should be 20 bytes in length
    pub ip: Ipv6Addr,
    pub port: u16,
    pub last_announced: Instant,
}

impl Compact for Peerv4 {
    fn compact(&self) -> Vec<u8> {
        let ip: u32 = self.ip.into();

        let mut full_compact_peer = vec![];
        full_compact_peer.put_slice(&ip.to_be_bytes());
        full_compact_peer.put_slice(&self.port.to_be_bytes());

        full_compact_peer
    }
}

// BEP 07: IPv6 Tracker Extension
impl Compact for Peerv6 {
    fn compact(&self) -> Vec<u8> {
        let ip: u128 = self.ip.into();

        // Had some trouble getting i128 features to work with
        // vectors, so this is a workaround; slowdown should be minimal
        let mut full_compact_peer = vec![];
        full_compact_peer.put_slice(&ip.to_be_bytes());
        full_compact_peer.put_slice(&self.port.to_be_bytes());

        full_compact_peer
    }
}

impl Hash for Peerv4 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.peer_id.hash(state);
        self.ip.hash(state);
        self.port.hash(state);
    }
}

impl Hash for Peerv6 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.peer_id.hash(state);
        self.ip.hash(state);
        self.port.hash(state);
    }
}

impl PartialEq for Peerv4 {
    fn eq(&self, other: &Self) -> bool {
        self.peer_id == other.peer_id && self.ip == other.ip && self.port == other.port
    }
}

impl PartialEq for Peerv6 {
    fn eq(&self, other: &Self) -> bool {
        self.peer_id == other.peer_id && self.ip == other.ip && self.port == other.port
    }
}

#[derive(Clone, PartialOrd, Ord, PartialEq, Eq, Hash, Debug)]
pub enum Peer {
    V4(Peerv4),
    V6(Peerv6),
}

impl Compact for Peer {
    fn compact(&self) -> Vec<u8> {
        match self {
            Peer::V4(p) => p.compact(),
            Peer::V6(p) => p.compact(),
        }
    }
}

#[derive(Debug)]
pub struct AnnounceRequest {
    pub info_hash: String,
    pub peer: Peer,
    pub port: u16,
    pub uploaded: u32,
    pub downloaded: u32,
    pub left: u32,
    pub compact: bool,
    pub no_peer_id: bool,
    pub event: Event,
    pub ip: Option<IpAddr>,
    pub numwant: Option<u32>,
    pub key: Option<String>,
    pub trackerid: Option<String>,
}

impl AnnounceRequest {
    pub fn new(
        url_string: &str,
        req_ip: Option<&str>,
    ) -> Result<AnnounceRequest, AnnounceResponse> {
        let request_kv_pairs = form_urlencoded::parse(url_string.as_bytes()).into_owned();

        let mut info_hash: String = "".to_string();
        let mut peer_string: String = "".to_string();
        let mut port = 0;
        let mut uploaded = 0;
        let mut downloaded = 0;
        let mut left = 0;
        let mut compact = false;
        let mut no_peer_id = false;
        let mut event = Event::None;
        let mut ip = None;
        let mut numwant = None;
        let mut key = None;
        let mut trackerid = None;

        for (k, value) in request_kv_pairs {
            match k.as_str() {
                "info_hash" => {
                    match percent_encoding::percent_decode(value.as_bytes()).decode_utf8() {
                        Ok(s) => info_hash = s.to_string(),
                        _ => {
                            return Err(AnnounceResponse::failure("Malformed request".to_string()))
                        }
                    }
                }
                "peer_id" => peer_string = value,
                "port" => match value.parse::<u16>() {
                    Ok(n) => port = n,
                    _ => return Err(AnnounceResponse::failure("Malformed request".to_string())),
                },
                "uploaded" => match value.parse::<u32>() {
                    Ok(n) => uploaded = n,
                    _ => return Err(AnnounceResponse::failure("Malformed request".to_string())),
                },
                "downloaded" => match value.parse::<u32>() {
                    Ok(n) => downloaded = n,
                    _ => return Err(AnnounceResponse::failure("Malformed request".to_string())),
                },
                "left" => match value.parse::<u32>() {
                    Ok(n) => left = n,
                    _ => return Err(AnnounceResponse::failure("Malformed request".to_string())),
                },
                "compact" => match value.parse::<u32>() {
                    Ok(n) => compact = n != 0,
                    _ => return Err(AnnounceResponse::failure("Malformed request".to_string())),
                },
                "no_peer_id" => match value.parse::<u32>() {
                    Ok(n) => no_peer_id = n != 0,
                    _ => return Err(AnnounceResponse::failure("Malformed request".to_string())),
                },
                "event" => event = string_to_event(value),
                "ip" => match value.parse::<IpAddr>() {
                    Ok(addr) => ip = Some(addr),
                    _ => return Err(AnnounceResponse::failure("Malformed request".to_string())),
                },
                "numwant" => match value.parse::<u32>() {
                    Ok(n) => numwant = Some(n),
                    _ => numwant = Some(50),
                },
                "key" => key = Some(value),
                "trackerid" => trackerid = Some(value),
                _ => {}
            }
        }

        // This should not be the default value
        if info_hash == "" {
            return Err(AnnounceResponse::failure("Malformed request".to_string()));
        }

        // Digusting unwrap sequence, but whatever.
        if ip.is_none() {
            if let Some(addr) = req_ip {
                if addr.starts_with('[') {
                    let re = Regex::new(r"\[(.*)\]").unwrap();
                    let caps = re.captures(addr).unwrap();
                    let ip_string = &caps[0];
                    ip = Some(ip_string.parse().unwrap());
                } else {
                    let ip_string: Vec<&str> = addr.split(':').collect();
                    ip = Some(ip_string[0].parse().unwrap());
                }
            }
        }

        let peer = match ip.unwrap() {
            IpAddr::V4(i) => Peer::V4(Peerv4 {
                peer_id: peer_string,
                ip: i,
                port,
                last_announced: Instant::now(),
            }),
            IpAddr::V6(i) => Peer::V6(Peerv6 {
                peer_id: peer_string,
                ip: i,
                port,
                last_announced: Instant::now(),
            }),
        };

        Ok(AnnounceRequest {
            info_hash,
            peer,
            port,
            uploaded,
            downloaded,
            left,
            compact,
            no_peer_id,
            event,
            ip,
            numwant,
            key,
            trackerid,
        })
    }
}

// Peer types are functionally the same, but due to different
// byte lengths, they should be separated for client compatibility
#[derive(Default, Debug)]
pub struct AnnounceResponse {
    pub failure_reason: Option<String>,
    pub interval: u32,
    pub min_interval: Option<u32>,
    pub tracker_id: String,
    pub complete: u32,
    pub incomplete: u32,
    pub peers: Vec<Peerv4>,
    pub peers6: Vec<Peerv6>,
}

impl AnnounceResponse {
    pub fn new(
        interval: u32,
        complete: u32,
        incomplete: u32,
        peers: Vec<Peerv4>,
        peers6: Vec<Peerv6>,
    ) -> Result<AnnounceResponse, &'static str> {
        Ok(AnnounceResponse {
            failure_reason: None,
            interval,
            min_interval: None,
            tracker_id: "".to_string(),
            complete,
            incomplete,
            peers,
            peers6,
        })
    }

    pub fn failure(reason: String) -> AnnounceResponse {
        AnnounceResponse {
            failure_reason: Some(reason),
            ..Default::default()
        }
    }

    pub fn peersv4_as_compact(&self) -> Vec<u8> {
        let mut compact_peers = Vec::new();
        for peer in &self.peers {
            compact_peers.push(peer.compact());
        }
        compact_peers.concat()
    }

    pub fn peersv6_as_compact(&self) -> Vec<u8> {
        let mut compact_peers = Vec::new();
        for peer in &self.peers6 {
            compact_peers.push(peer.compact());
        }
        compact_peers.concat()
    }
}

#[derive(Debug, Default)]
pub struct ScrapeFile {
    pub info_hash: String,
    pub complete: u32,
    pub downloaded: u32,
    pub incomplete: u32,
    pub name: Option<String>,
}

pub struct ScrapeRequest {
    pub info_hashes: Vec<String>,
}

impl ScrapeRequest {
    pub fn new(url_string: &str) -> Result<ScrapeRequest, ScrapeResponse> {
        let request_kv_pairs = form_urlencoded::parse(url_string.as_bytes()).into_owned();
        let mut info_hashes = Vec::new();

        for (key, value) in request_kv_pairs {
            match key.as_str() {
                "info_hash" => info_hashes.push(value),
                _ => {
                    return Err(ScrapeResponse::failure(
                        "Malformed scrape request".to_string(),
                    ))
                }
            }
        }

        Ok(ScrapeRequest { info_hashes })
    }
}

#[derive(Default, Debug)]
pub struct ScrapeResponse {
    pub failure_reason: Option<String>,
    pub files: HashMap<String, ScrapeFile>,
}

impl ScrapeResponse {
    pub fn new() -> Result<ScrapeResponse, ()> {
        Ok(ScrapeResponse {
            failure_reason: None,
            files: HashMap::new(),
        })
    }

    pub fn failure(reason: String) -> ScrapeResponse {
        ScrapeResponse {
            failure_reason: Some(reason),
            ..Default::default()
        }
    }

    pub fn add_file(&mut self, info_hash: String, scrape_file: ScrapeFile) {
        self.files.insert(info_hash, scrape_file);
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::time::Instant;

    use super::{
        AnnounceRequest, AnnounceResponse, Compact, Peer, Peerv4, Peerv6, ScrapeFile,
        ScrapeRequest, ScrapeResponse,
    };

    use bytes::BufMut;

    #[test]
    fn announce_bad_request_creation() {
        let url_string = "info_hash=%90%28%9F%D3M%FC%1C%F8%F3%16%A2h%AD%D85L%853DX\
             &peer_id=ABCDEFGHIJKLMNOPQRST&port=thisisnotanumber&uploaded=0&downloaded=0\
             &left=727955456&event=started&numwant=100&no_peer_id=1&compact=thisisnotanumber";

        assert!(
            AnnounceRequest::new(url_string, None).is_err(),
            "Incorrect announce request parameter parsing"
        );
    }

    #[test]
    fn announce_failure_return() {
        let failure_reason = "It's not you...no, it's just you".to_string();
        let failure = AnnounceResponse::failure(failure_reason);
        assert_eq!(
            failure.failure_reason,
            Some("It's not you...no, it's just you".to_string())
        );
    }

    #[test]
    fn announce_response_creation() {
        let peerv4_1 = Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6893,
            last_announced: Instant::now(),
        };
        let peerv4_2 = Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::BROADCAST,
            port: 6894,
            last_announced: Instant::now(),
        };

        let mut peers: Vec<Peerv4> = Vec::new();
        peers.push(peerv4_1);
        peers.push(peerv4_2);

        let peerv6_1 = Peerv6 {
            peer_id: "ABCDEFGHIJKLMNOPABCD".to_string(),
            ip: Ipv6Addr::new(
                0x2001, 0x0db8, 0x85a3, 0x0000, 0x0000, 0x8a2e, 0x0370, 0x7334,
            ),
            port: 6681,
            last_announced: Instant::now(),
        };
        let peerv6_2 = Peerv6 {
            peer_id: "ABCDEFGHIJKLMNOPZZZZ".to_string(),
            ip: Ipv6Addr::new(
                0xfe80, 0x0000, 0x0000, 0x0000, 0x0202, 0xb3ff, 0xfe1e, 0x8329,
            ),
            port: 6699,
            last_announced: Instant::now(),
        };

        let mut peers6: Vec<Peerv6> = Vec::new();
        peers6.push(peerv6_1);
        peers6.push(peerv6_2);

        let response = AnnounceResponse::new(60, 100, 23, peers, peers6);

        assert!(response.is_ok(), "Incorrect announce response creation");
    }

    #[test]
    fn peerv4_compact_transform() {
        let peer = Peer::V4(Peerv4 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv4Addr::LOCALHOST,
            port: 6681,
            last_announced: Instant::now(),
        });

        let mut localhost_port_byte_string = vec![];
        localhost_port_byte_string.put_u32(2130706433); // localhost in decimal
        localhost_port_byte_string.put_u16(6681);

        let compact_rep_byte_string = peer.compact();

        assert_eq!(compact_rep_byte_string, localhost_port_byte_string.to_vec());
    }

    #[test]
    fn peerv6_compact_transform() {
        let peer = Peer::V6(Peerv6 {
            peer_id: "ABCDEFGHIJKLMNOPQRST".to_string(),
            ip: Ipv6Addr::new(
                0x2001, 0x0db8, 0x85a3, 0x0000, 0x0000, 0x8a2e, 0x0370, 0x7334,
            ),
            port: 6681,
            last_announced: Instant::now(),
        });

        let mut localhost_port_byte_string = vec![];
        let localhost_decimal = 42540766452641154071740215577757643572 as u128;
        let port = 6681 as u16;
        localhost_port_byte_string.put_slice(&localhost_decimal.to_be_bytes());
        localhost_port_byte_string.put_slice(&port.to_be_bytes());

        let compact_rep_byte_string = peer.compact();

        assert_eq!(compact_rep_byte_string, localhost_port_byte_string.to_vec());
    }

    #[test]
    fn scrape_good_request_creation() {
        let url_string = "info_hash=aaaaaaaaaaaaaaaaaaaa&info_hash=bbbbbbbbbbbbbbbbbbbb&info_hash=cccccccccccccccccccc";

        assert!(
            ScrapeRequest::new(url_string).is_ok(),
            "Scrape request creation failed"
        );
    }

    #[test]
    fn scrape_good_request_multiple_hashes() {
        let url_string = "info_hash=aaaaaaaaaaaaaaaaaaaa&info_hash=bbbbbbbbbbbbbbbbbbbb&info_hash=cccccccccccccccccccc";
        let scrape = ScrapeRequest::new(url_string).unwrap();
        assert_eq!(
            scrape.info_hashes,
            vec![
                "aaaaaaaaaaaaaaaaaaaa",
                "bbbbbbbbbbbbbbbbbbbb",
                "cccccccccccccccccccc"
            ]
        );
    }

    #[test]
    fn scrape_bad_request_creation() {
        let url_string = "info_hash=aaaaaaaaaaaaaaaaaaaa&info_bash=bbbbbbbbbbbbbbbbbbbb&info_slash=cccccccccccccccccccc";

        assert!(
            ScrapeRequest::new(url_string).is_err(),
            "Incorrect scrape request parsing"
        );
    }

    #[test]
    fn scrape_response_add_file() {
        let file = ScrapeFile::default();
        let mut scrape_response = ScrapeResponse::new().unwrap();
        scrape_response.add_file("test".to_string(), file);

        assert_eq!(scrape_response.files.len(), 1);
    }
}
