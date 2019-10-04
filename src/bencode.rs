use crate::bittorrent::{AnnounceResponse, ScrapeFile, ScrapeResponse};
use bendy::encoding::{Error, SingleItemEncoder, ToBencode};

impl ToBencode for ScrapeFile {
    const MAX_DEPTH: usize = 1;

    // bendy's emit methods return a result, which isn't immediately clear
    fn encode(&self, encoder: SingleItemEncoder) -> Result<(), Error> {
        encoder.emit_dict(|mut e| {
            e.emit_pair(b"complete", &self.complete)?;
            e.emit_pair(b"downloaded", &self.downloaded)?;
            e.emit_pair(b"incomplete", &self.incomplete)?;

            if let Some(name) = &self.name {
                e.emit_pair(b"name", name)?;
            }

            Ok(())
        })?;

        Ok(())
    }
}

impl ToBencode for AnnounceResponse {
    const MAX_DEPTH: usize = 5;

    fn encode(&self, encoder: SingleItemEncoder) -> Result<(), Error> {
        match &self.failure_reason {
            Some(reason) => {
                encoder.emit_dict(|mut e| {
                    e.emit_pair(b"failure_reason", reason)?;

                    Ok(())
                })?;
            }

            None => {
                encoder.emit_dict(|mut e| {
                    e.emit_pair(b"complete", &self.complete)?;
                    e.emit_pair(b"incomplete", &self.incomplete)?;
                    e.emit_pair(b"interval", &self.interval)?;

                    if let Some(min_interval) = &self.min_interval {
                        e.emit_pair(b"min_interval", min_interval)?;
                    }

                    e.emit_pair(b"peers", &self.peersv4_as_compact())?;
                    e.emit_pair(b"peers6", &self.peersv6_as_compact())?;
                    e.emit_pair(b"tracker_id", &self.tracker_id)?;

                    Ok(())
                })?;
            }
        }

        Ok(())
    }
}

impl ToBencode for ScrapeResponse {
    const MAX_DEPTH: usize = 5;

    fn encode(&self, encoder: SingleItemEncoder) -> Result<(), Error> {
        encoder.emit_dict(|mut e| {
            e.emit_pair(b"files", &self.files)?;

            Ok(())
        })?;

        Ok(())
    }
}

pub fn encode_announce_response(response: AnnounceResponse) -> Vec<u8> {
    response.to_bencode().ok().unwrap()
}

pub fn encode_scrape_response(response: ScrapeResponse) -> Vec<u8> {
    response.to_bencode().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bittorrent::{AnnounceResponse, Peer, Peerv4, Peerv6, ScrapeResponse};
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn announce_response_encoding() {
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

        let mut peers = Vec::new();
        peers.push(peerv4_1);
        peers.push(peerv4_2);

        let peerv6_1 = Peer::V6(Peerv6 {
            peer_id: "ABCDEFGHIJKLMNOPABCD".to_string(),
            ip: Ipv6Addr::new(
                0x2001, 0x0db8, 0x85a3, 0x0000, 0x0000, 0x8a2e, 0x0370, 0x7334,
            ),
            port: 6681,
        });
        let peerv6_2 = Peer::V6(Peerv6 {
            peer_id: "ABCDEFGHIJKLMNOPZZZZ".to_string(),
            ip: Ipv6Addr::new(
                0xfe80, 0x0000, 0x0000, 0x0000, 0x0202, 0xb3ff, 0xfe1e, 0x8329,
            ),
            port: 6699,
        });

        let mut peers6 = Vec::new();
        peers6.push(peerv6_1);
        peers6.push(peerv6_2);

        let response = AnnounceResponse::new(60, 100, 23, peers, peers6).unwrap();

        let encoded = encode_announce_response(response);

        assert_eq!(encoded.as_slice(), &b"d8:completei100e10:incompletei23e8:intervali60e5:peersli127ei0ei0ei1ei26ei237ei255ei255ei255ei255ei26ei238ee6:peers6li32ei1ei13ei184ei133ei163ei0ei0ei0ei0ei138ei46ei3ei112ei115ei52ei26ei25ei254ei128ei0ei0ei0ei0ei0ei0ei2ei2ei179ei255ei254ei30ei131ei41ei26ei43ee10:tracker_id0:e"[..]);
    }

    #[test]
    fn announce_failure_encoding() {
        let failure_reason = "ouch".to_string();
        let failure = AnnounceResponse::failure(failure_reason);

        let encoded = encode_announce_response(failure);

        assert_eq!(encoded.as_slice(), b"d14:failure_reason4:ouche");
    }

    #[test]
    fn scrape_response_encoding() {
        let file1 = ScrapeFile {
            complete: 1,
            downloaded: 2,
            incomplete: 3,
            name: Some("test".to_string()),
        };

        let file2 = ScrapeFile {
            complete: 4000,
            downloaded: 5678,
            incomplete: 785,
            name: Some("Reflections".to_string()),
        };

        let mut scrape_response = ScrapeResponse::new().unwrap();
        scrape_response.add_file("ABCDEFGHIJKLMNOPQRST".to_string(), file1);
        scrape_response.add_file("TSRQPONMLKJIHGFEDCBA".to_string(), file2);

        let encoded = encode_scrape_response(scrape_response);

        assert_eq!(encoded.as_slice(), &b"d5:filesd20:ABCDEFGHIJKLMNOPQRSTd8:completei1e10:downloadedi2e10:incompletei3e4:name4:teste20:TSRQPONMLKJIHGFEDCBAd8:completei4000e10:downloadedi5678e10:incompletei785e4:name11:Reflectionseee"[..]);
    }
}
