use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use reqwest::Client;
use bendy::{
    decoding::{FromBencode, Decoder, Object, Error as DecodingError, ResultExt},
    encoding::AsString,
};
use failure::err_msg;

use crate::torrent::BTorrent;
use crate::config::NetworkSettings;


#[derive(PartialEq)]
pub enum BAnnounceEvent {
    Started,
    Completed,
    Stopped,
}


pub async fn announce(
    client: &Client,
    torrent: &BTorrent,
    event: Option<BAnnounceEvent>,
    network_settings: &NetworkSettings)
-> Result<reqwest::Response, reqwest::Error> {
    // `reqwest` (and the `serde_urlencoded` library it relies on) doesn't accept
    // raw bytes as input to be url encoded, so we need to work around this by manually
    // url encoding our info hash and peer id, and then manually adding them
    // to the url used for the `RequestBuilder`.
    let url = format!("{}?info_hash={}peer_id={}",
        torrent.metainfo.announce,
        torrent.encoded_info_hash,
        torrent.encoded_peer_id,
    );

    let mut request = client.get(&url);

    request = request.query(&[
            ("info_hash",  &torrent.encoded_info_hash),
            ("port",       &network_settings.port.to_string()),
            ("uploaded",   &torrent.uploaded.to_string()),
            ("downloaded", &torrent.downloaded.to_string()),
            ("left",       &torrent.left.to_string()),
        ]);

    // Optional key.
    if let Some(ip) = &network_settings.ip {
        request = request.query(&["ip", ip]);
    }

    // The `event` key is only necessary if the announce is not for one of the
    // regular announces performed while a torrent is active.
    if let Some(event) = event {
        let val = match event {
            BAnnounceEvent::Started   => "started",
            BAnnounceEvent::Completed => "completed",
            BAnnounceEvent::Stopped   => "stopped",
        };
        request = request.query(&["event", val]);
    }

    request.send().await
}

#[allow(dead_code)]
pub struct BTrackerResponse {
    peers: Vec<BPeer>,
    interval: u64, // suggested minimum announce interval, in seconds
    complete: Option<u64>,
    incomplete: Option<u64>,
}

impl BTrackerResponse {
    pub fn from_bytes(bytes: &[u8]) -> Result<BTrackerResponse, String> {
        let mut decoder = Decoder::new(&bytes);

        // Read in and then parse the tracker response dictionary
        let tracker_response = decoder.next_object()
            .map_err(|x| x.to_string())?
            .ok_or_else(|| String::from("Tracker sent empty response."))?;
        let tracker_response = BTrackerResponse::decode_bencode_object(tracker_response)
            .map_err(|x| x.to_string());

        // Ensure we've hit EOF
        if decoder.next_object().map_err(|x| x.to_string())?.is_some() {
            return Err(String::from("Erroneous data at the end of the tracker response."))
        }

        tracker_response
    }

    // pub async fn from_response(response: reqwest::Response) -> Result<BTrackerResponse, String> {
    // 	let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    // 	BTrackerResponse::from_bytes(&bytes)
    // }
}

impl FromBencode for BTrackerResponse {
    fn decode_bencode_object(object: Object) -> Result<Self, DecodingError> {
        let mut peers      = None;
        let mut peers6     = None;
        let mut interval   = None;
        let mut complete   = None;
        let mut incomplete = None;

        let mut dict = object.try_into_dictionary()?;
        while let Some(keyval) = dict.next_pair()? {
            match keyval {
                (b"peers", val) => {
                    match val {
                        Object::List(_) => {
                            peers = Vec::decode_bencode_object(val)
                                .context("peers")
                                .map(Some)?;
                        }
                        Object::Bytes(_) => {
                            // `AsString` is a wrapper allowing us to decode/encode a Vec<u8>.
                            // It contains only one field -- the Vec<u8>. Unwrap it.
                            let peers_bytestring = AsString::decode_bencode_object(val)
                                .context("peers")
                                .map(|b| b.0)?;

                            peers = parse_compact_ipv4_peer_list(&peers_bytestring)
                                .map(Some)?;
                        }
                        _ => {
                            return Err(DecodingError::malformed_content(
                                err_msg("peers key must be either a dictionary or a list")
                            ));
                        }
                    }
                }
                (b"peers6", val) => {
                    // `AsString` is a wrapper allowing us to decode/encode a Vec<u8>.
                    // It contains only one field -- the Vec<u8>. Unwrap it.
                    let peers_bytestring = AsString::decode_bencode_object(val)
                        .context("peers6")
                        .map(|b| b.0)?;

                    peers6 = parse_compact_ipv6_peer_list(&peers_bytestring)
                        .map(Some)?;
                }
                (b"interval", val) => {
                    interval = u64::decode_bencode_object(val)
                        .context("interval")
                        .map(Some)?;
                }
                (b"complete", val) => {
                    complete = u64::decode_bencode_object(val)
                        .context("complete")
                        .map(Some)?;
                }
                (b"incomplete", val) => {
                    incomplete = u64::decode_bencode_object(val)
                        .context("incomplete")
                        .map(Some)?;
                }
                (key, _) => {
                    return Err(DecodingError::unexpected_field(String::from_utf8_lossy(key)));
                }
            }
        }

        let mut peers    =    peers.ok_or_else(|| DecodingError::missing_field("peers"   ))?;
        let     interval = interval.ok_or_else(|| DecodingError::missing_field("interval"))?;

        // Merge the Ipv6 peer list with the Ipv4 peer list.
        // For our purposes, they can be both in the same vector for simplicity.
        if let Some(mut peers6) = peers6 {
            peers.append(&mut peers6);
        }

        Ok(BTrackerResponse {
            peers,
            interval,
            complete,
            incomplete,
        })
    }
}

#[allow(dead_code)]
struct BPeer {
    ip: IpAddr,
    peer_id: String,
    port: u16,
}

impl FromBencode for BPeer {
    fn decode_bencode_object(object: Object) -> Result<Self, DecodingError> {
        let mut ip      = None;
        let mut peer_id = None;
        let mut port    = None;

        let mut dict = object.try_into_dictionary()?;
        while let Some(keyval) = dict.next_pair()? {
            match keyval {
                (b"ip", val) => {
                    let ip_string = String::decode_bencode_object(val)
                        .context("ip")?;

                    // Bloated peer list ip could either be Ipv4 or Ipv6
                    let ip_obj: IpAddr = ip_string.parse()
                        .map_err(|_| DecodingError::malformed_content(
                            err_msg("invalid ip address")
                        ))?;

                    ip = Some(ip_obj);
                }
                (b"peer id", val) => {
                    peer_id = String::decode_bencode_object(val)
                        .context("peer id")
                        .map(Some)?;
                }
                (b"port", val) => {
                    port = u16::decode_bencode_object(val)
                        .context("port")
                        .map(Some)?;
                }
                (key, _) => {
                    return Err(DecodingError::unexpected_field(String::from_utf8_lossy(key)));
                }
            }
        }

        let ip      =      ip.ok_or_else(|| DecodingError::missing_field("ip"     ))?;
        let peer_id = peer_id.ok_or_else(|| DecodingError::missing_field("peer_id"))?;
        let port    =    port.ok_or_else(|| DecodingError::missing_field("port"   ))?;

        Ok(BPeer {
            ip,
            peer_id,
            port,
        })
    }
}


fn parse_compact_ipv4_peer_list(bytes: &[u8]) -> Result<Vec<BPeer>, DecodingError> {
    let mut peers = Vec::new();

    if bytes.len() % 6 != 0 {
        return Err(DecodingError::malformed_content(
            err_msg("incomplete compact ipv4 peers list (length is not divisible by 6)")
        ));
    }

    for i in bytes.chunks(6) {
        // Give the slices compile-time sizes.
        let ip   = <[u8; 4]>::try_from(&i[0..4]).unwrap();
        let port = <[u8; 2]>::try_from(&i[4..6]).unwrap();

        let ip   = IpAddr::V4(Ipv4Addr::from(ip.map(u8::from_be)));
        let port = u16::from_be_bytes(port);

        peers.push(BPeer {
            ip,
            peer_id: String::from(""),
            port,
        });
    }

    Ok(peers)
}

fn parse_compact_ipv6_peer_list(bytes: &[u8]) -> Result<Vec<BPeer>, DecodingError> {
    let mut peers = Vec::new();

    if bytes.len() % 18 != 0 {
        return Err(DecodingError::malformed_content(
            err_msg("incomplete compact ipv4 peers list (length is not divisible by 18)")
        ));
    }

    for i in bytes.chunks(18) {
        // Give the slices compile-time sizes.
        let ip   = <[u8; 16]>::try_from( &i[0..16]).unwrap();
        let port = <[u8;  2]>::try_from(&i[16..18]).unwrap();

        let ip   = IpAddr::V6(Ipv6Addr::from(ip.map(u8::from_be)));
        let port = u16::from_be_bytes(port);

        peers.push(BPeer {
            ip,
            peer_id: String::from(""),
            port,
        });
    }

    Ok(peers)
}
