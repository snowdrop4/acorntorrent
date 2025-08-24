use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::collections::BTreeMap;
use std::str;

use reqwest::Client;
use acornbencode::parser::parse_bencode;
use acornbencode::common::BencodeValue;

use crate::torrent::BTorrent;
use crate::config::NetworkSettings;
use crate::util::get_utf8_value;


#[derive(PartialEq, Debug)]
pub enum BAnnounceEvent {
    Started,
    Completed,
    Stopped,
}

pub async fn announce_to_tracker(
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
#[derive(Debug)]
pub struct BTrackerResponse {
    peers: Vec<BPeer>,
    interval: isize, // suggested minimum announce interval, in seconds
    complete: Option<isize>, // number of complete peers
    incomplete: Option<isize>, // number of incomplete peers
}

impl BTrackerResponse {
    pub fn from_bytes(bytes: &[u8]) -> Result<BTrackerResponse, String> {
        let (remaining, value) = match parse_bencode(bytes) {
            Ok((rem, val)) => (rem, val),
            Err(e) => return Err(format!("Failed to parse bencode: {:?}", e)),
        };

        // Ensure we've hit EOF (no remaining data)
        if !remaining.is_empty() {
            return Err("Erroneous data at the end of the tracker response".to_string());
        }

        // Extract tracker response from the parsed bencode value
        BTrackerResponse::from_bencode_value(&value)
    }

    fn from_bencode_value(value: &BencodeValue) -> Result<BTrackerResponse, String> {
        match value {
            BencodeValue::Dictionary(dict) => {
                let interval = match dict.get(b"interval".as_ref()) {
                    Some(BencodeValue::Integer(val)) => *val,
                    None => return Err("missing field 'interval'".to_string()),
                    _ => return Err("field 'interval' must be an integer".to_string()),
                };

                let complete = match dict.get(b"complete".as_ref()) {
                    Some(BencodeValue::Integer(val)) => Some(*val),
                    None => None,
                    _ => return Err("field 'complete' must be an integer".to_string()),
                };

                let incomplete = match dict.get(b"incomplete".as_ref()) {
                    Some(BencodeValue::Integer(val)) => Some(*val),
                    None => None,
                    _ => return Err("field 'incomplete' must be an integer".to_string()),
                };

                let peers = match dict.get(b"peers".as_ref()) {
                    // Parse dictionary format peers
                    Some(BencodeValue::List(list)) => {
                        let mut peers_vec = Vec::new();
                        for peer in list {
                            match peer {
                                BencodeValue::Dictionary(peer_dict) => {
                                    let peer = BPeer::from_bencode_dict(peer_dict)?;
                                    peers_vec.push(peer);
                                }
                                _ => return Err("field 'peers' must be a list of dictionaries".to_string()),
                            }
                        }
                        peers_vec
                    },
                    // Parse compact format peers
                    Some(BencodeValue::ByteString(bytes)) => {
                        parse_compact_ipv4_peer_list(*bytes)?
                    },
                    // Otherwise, throw an error
                    None => return Err("missing field 'peers'".to_string()),
                    _ => return Err("field 'peers' must be a list or byte string".to_string()),
                };

                // Handle optional IPv6 peers
                let mut all_peers = peers;
                if let Some(BencodeValue::ByteString(bytes)) = dict.get(b"peers6".as_ref()) {
                    let mut ipv6_peers = parse_compact_ipv6_peer_list(bytes)?;
                    all_peers.append(&mut ipv6_peers);
                }

                Ok(BTrackerResponse {
                    peers: all_peers,
                    interval,
                    complete,
                    incomplete,
                })
            }
            _ => Err("Tracker response must be a dictionary".to_string()),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct BPeer {
    ip: IpAddr,
    peer_id: String,
    port: u16,
}

impl BPeer {
    fn from_bencode_dict(dict: &BTreeMap<&[u8], BencodeValue>) -> Result<Self, String> {
        let ip_string = match dict.get(b"ip".as_ref()) {
            Some(BencodeValue::ByteString(s)) => s,
            None => return Err("missing field 'ip'".to_string()),
            _ => return Err("field 'ip' must be a string".to_string()),
        };

        // Parse IP address
        let ip: IpAddr = str::from_utf8(ip_string).expect("Invalid UTF-8").parse()
            .map_err(|_| "Invalid IP address".to_string())?;

        let peer_id = get_utf8_value(dict, b"peer id")?;

        let port = match dict.get(b"port".as_ref()) {
            Some(BencodeValue::Integer(val)) => *val as u16,
            None => return Err("missing field 'port'".to_string()),
            _ => return Err("field 'port' must be an integer".to_string()),
        };

        Ok(BPeer {
            ip,
            peer_id,
            port,
        })
    }
}

fn parse_compact_ipv4_peer_list(bytes: &[u8]) -> Result<Vec<BPeer>, String> {
    let mut peers = Vec::new();

    if bytes.len() % 6 != 0 {
        return Err("Incomplete compact IPv4 peers list (length is not divisible by 6)".to_string());
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

fn parse_compact_ipv6_peer_list(bytes: &[u8]) -> Result<Vec<BPeer>, String> {
    let mut peers = Vec::new();

    if bytes.len() % 18 != 0 {
        return Err("Incomplete compact IPv6 peers list (length is not divisible by 18)".to_string());
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
