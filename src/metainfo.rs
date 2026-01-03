use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str;

use acornbencode::common::BencodeValue;
use acornbencode::parser::parse_bencode;
use ring::digest;

use crate::util::bencoding::{get_optional_utf8_value, get_utf8_value};

type DecodingError = String;
type EncodingError = String;

// Extract the raw bytes of the info dictionary from a torrent file.
// This is used for calculating the infohash. If we hash the raw bytes,
// then our hash reflects all additional/optional/unrecognised keys inside
// the dictionary, that our parser would otherwise ignore.
//
// Technically the spec is a bit ambigious about the correct behaviour
// regarding additional/optional keys.
//
// It just says:
//
// > The 20 byte sha1 hash of the bencoded form of the info value from the
// > metainfo file. This value will almost certainly have to be escaped.
// >
// > Note that this is a substring of the metainfo file. The info-hash must be
// the hash of the encoded form as found in the .torrent file, which is identical
// to bdecoding the metainfo file, extracting the info dictionary and encoding
// it if and only if the bdecoder fully validated the input (e.g. key ordering,
// absence of leading zeros). Conversely that means clients must either reject
// invalid metainfo files or extract the substring directly. They must not
// perform a decode-encode roundtrip on invalid data.

fn extract_raw_info_bytes(bytes: &[u8]) -> Result<Vec<u8>, DecodingError> {
    // Find "4:info" in the bencode data
    let info_key = b"4:info";
    let pos = bytes
        .windows(info_key.len())
        .position(|window| window == info_key)
        .ok_or("Could not find '4:info' in torrent file")?;

    // The info dictionary VALUE starts after "4:info"
    let info_start = pos + info_key.len();

    // Parse the info dictionary value using the bencode parser
    // The parser will correctly handle all bencode structures and return the remaining bytes
    let (remaining, _) = parse_bencode(&bytes[info_start..])
        .map_err(|e| format!("Failed to parse info dictionary: {:?}", e))?;

    // The info dict length is: total bytes from info_start minus remaining bytes
    let info_len = bytes[info_start..].len() - remaining.len();
    let info_end = info_start + info_len;

    Ok(bytes[info_start..info_end].to_vec())
}

#[derive(Debug)]
pub struct BMetainfo {
    // If `announce_list` is present, it overrides `announce`:
    pub announce: String,
    pub announce_list: Option<Vec<Vec<String>>>, // https://www.bittorrent.org/beps/bep_0012.html

    // Free-form comment.
    pub comment: Option<String>,

    // The torrent client/library/tool that created the torrent.
    pub created_by: Option<String>,

    // Seconds since epoch.
    pub created_on: Option<isize>,

    // Encoding used for the filenames in `info`. Assumed to be UTF-8 if not present.
    // If present and not set to 'UTF-8', parsing will raise an error.
    pub encoding: Option<String>,

    pub info: BInfo,
}

impl BMetainfo {
    pub fn from_bytes(bytes: &[u8]) -> Result<BMetainfo, DecodingError> {
        let (remaining, value) = match parse_bencode(bytes) {
            Ok((rem, val)) => (rem, val),
            Err(e) => return Err(format!("Failed to parse bencode: {:?}", e)),
        };

        // Ensure we've hit EOF (no remaining data)
        if !remaining.is_empty() {
            return Err("Erroneous data at the end of the metainfo file".to_string());
        }

        // Extract raw info bytes from the original bytes
        let raw_info_bytes = extract_raw_info_bytes(bytes)?;

        // Extract metainfo from the parsed bencode value
        BMetainfo::from_bencode_value(&value, raw_info_bytes)
    }

    pub fn from_path(path: &Path) -> Result<BMetainfo, DecodingError> {
        let mut f = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
        let mut b = Vec::new();
        f.read_to_end(&mut b)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        BMetainfo::from_bytes(&b)
    }

    fn from_bencode_value(
        value: &BencodeValue,
        raw_info_bytes: Vec<u8>,
    ) -> Result<BMetainfo, DecodingError> {
        let dict = match value {
            BencodeValue::Dictionary(dict) => dict,
            _ => return Err("Metainfo must be a dictionary".to_string()),
        };

        let announce = get_utf8_value(dict, b"announce")?;
        let announce_list = BMetainfo::from_bencode_value_announce_list(dict)?;
        let comment = get_optional_utf8_value(dict, b"comment")?;
        let created_by = get_optional_utf8_value(dict, b"created by")?;

        let created_on = match dict.get(b"creation date".as_ref()) {
            Some(BencodeValue::Integer(val)) => Some(*val),
            None => None,
            _ => return Err("field 'creation date' must be an integer".to_string()),
        };

        let encoding = match get_optional_utf8_value(dict, b"encoding") {
            Ok(Some(e)) => {
                if e.to_lowercase() != "utf-8" {
                    return Err(format!(
                        "only UTF-8 encoding is supported; encountered encoding '{}' instead",
                        e
                    ));
                }
                Some(e)
            }
            _ => None,
        };

        let info = match dict.get(b"info".as_ref()) {
            Some(BencodeValue::Dictionary(info_dict)) => {
                BInfo::from_bencode_dict(info_dict, raw_info_bytes)?
            }
            None => return Err("missing field 'info'".to_string()),
            _ => return Err("field 'info' must be a dictionary".to_string()),
        };

        Ok(BMetainfo {
            announce,
            announce_list,
            comment,
            created_by,
            created_on,
            encoding,
            info,
        })
    }

    fn from_bencode_value_announce_list(
        dict: &BTreeMap<&[u8], BencodeValue>,
    ) -> Result<Option<Vec<Vec<String>>>, DecodingError> {
        let raw_announce_list = match dict.get(b"announce-list".as_ref()) {
            Some(BencodeValue::List(list)) => list,
            None => return Ok(None),
            _ => return Err("announce-list must be a list".to_string()),
        };

        // Each tier contains multiple announce URLs
        let mut announce_tiers = Vec::new();

        for raw_announce_list_inner in raw_announce_list {
            let mut announce_tier = Vec::new();

            let raw_announce_tier_trackers = match raw_announce_list_inner {
                BencodeValue::List(raw_announce_tier_trackers) => raw_announce_tier_trackers,
                _ => return Err("Invalid item in announce-list".to_string()),
            };

            for tracker in raw_announce_tier_trackers {
                match tracker {
                    BencodeValue::ByteString(s) => announce_tier.push(
                        str::from_utf8(s)
                            .expect("Invalid UTF-8 in announce-list")
                            .to_string(),
                    ),
                    _ => return Err("Invalid type in announce-list".to_string()),
                }
            }

            announce_tiers.push(announce_tier);
        }

        Ok(Some(announce_tiers))
    }
}

#[derive(Debug)]
pub struct BInfo {
    //                              These are mutually exclusive of one another:
    pub files: Option<Vec<BFile>>, // Multi-file torrents
    pub length: Option<isize>,     // Single-file torrents

    // The suggested title for the torrent.
    // If the torrent is a single-file torrent, this is also the suggested filename.
    pub name: String,

    // The length (in bytes) of each piece.
    pub piece_size: isize,

    // 20-byte hashes of every single piece, concated together.
    pub pieces: Vec<u8>,

    // Whether DHT should be disabled or not.
    pub private: Option<bool>,

    // The tracker the torrent came from.
    //
    // This is used by private trackers to stop their peer lists being leaked
    // if the same torrent is uploaded to multiple trackers, since each tracker
    // will force a different infohash by setting `source`, even if the rest of
    // the torrent is identical.
    pub source: Option<String>,

    // The raw bytes of the info dictionary from the original torrent file.
    //
    // This is used when computing the info hash, as our parser ignores
    // additional/optional/unrecognised keys in the info dictionary.
    //
    // It also means that we don't need to go through a decode-encode roundtip,
    // which may cause the hash to be different than intended.
    raw_info_bytes: Vec<u8>,
}

impl BInfo {
    // -------------------------------------------------------------------------
    // Convenience properties
    // -------------------------------------------------------------------------

    /// THe total number of all pieces.
    pub fn total_piece_count(&self) -> isize {
        self.pieces.len() as isize / 20
    }

    /// The total size of all pieces.
    pub fn total_piece_size_bytes(&self) -> isize {
        self.piece_size * self.total_piece_count()
    }

    /// The size of the entire metainfo file.
    pub fn metainfo_total_size_bytes(&self) -> isize {
        if let Some(files) = &self.files {
            files.iter().map(|f| f.length).sum()
        } else {
            self.length.unwrap_or(0)
        }
    }

    // -------------------------------------------------------------------------
    // Hashing
    // -------------------------------------------------------------------------

    pub fn compute_hash(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(
            digest::digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, &self.raw_info_bytes)
                .as_ref()
                .to_vec(),
        )
    }

    // -------------------------------------------------------------------------
    // Parsing
    // -------------------------------------------------------------------------

    pub fn from_bencode_dict(
        dict: &BTreeMap<&[u8], BencodeValue>,
        raw_info_bytes: Vec<u8>,
    ) -> Result<Self, DecodingError> {
        let files = match dict.get(b"files".as_ref()) {
            Some(BencodeValue::List(list)) => {
                let mut files_vec = Vec::new();
                for item in list {
                    match item {
                        BencodeValue::Dictionary(file_dict) => {
                            let file = BFile::from_bencode_dict(file_dict)?;
                            files_vec.push(file);
                        }
                        _ => return Err("field 'files' must be list of dictionaries".to_string()),
                    }
                }
                Some(files_vec)
            }
            None => None,
            _ => return Err("field 'files' must be a list".to_string()),
        };

        let length = match dict.get(b"length".as_ref()) {
            Some(BencodeValue::Integer(val)) => Some(*val),
            None => None,
            _ => return Err("field 'length' must be an integer".to_string()),
        };

        let name = get_utf8_value(dict, b"name".as_ref())?;

        let piece_size = match dict.get(b"piece length".as_ref()) {
            Some(BencodeValue::Integer(val)) => *val,
            None => return Err("missing field 'piece length'".to_string()),
            _ => return Err("field 'piece length' must be an integer".to_string()),
        };

        let pieces = match dict.get(b"pieces".as_ref()) {
            Some(BencodeValue::ByteString(val)) => val.to_vec(),
            None => return Err("missing field 'pieces'".to_string()),
            _ => return Err("field 'pieces' must be a byte string".to_string()),
        };

        let private = match dict.get(b"private".as_ref()) {
            Some(BencodeValue::Integer(val)) => Some(*val != 0),
            None => None,
            _ => return Err("field 'private' must be an integer".to_string()),
        };

        let source = get_optional_utf8_value(dict, b"source".as_ref())?;

        if length.is_some() == files.is_some() {
            return Err(
                "Metainfo files must contain the field 'length' or 'files' (not both or none)"
                    .to_string(),
            );
        }

        Ok(BInfo {
            files,
            length,
            name,
            piece_size,
            pieces,
            private,
            source,
            raw_info_bytes,
        })
    }
}

#[derive(Debug)]
pub struct BFile {
    length: isize,
    path: Vec<String>,
}

impl BFile {
    pub fn from_bencode_dict(dict: &BTreeMap<&[u8], BencodeValue>) -> Result<Self, DecodingError> {
        let length = match dict.get(b"length".as_ref()) {
            Some(BencodeValue::Integer(val)) => *val,
            None => return Err("missing field 'length'".to_string()),
            _ => return Err("field 'length' must be an integer".to_string()),
        };

        let path = match dict.get(b"path".as_ref()) {
            Some(BencodeValue::List(list)) => {
                let mut path_vec = Vec::new();
                for item in list {
                    match item {
                        BencodeValue::ByteString(s) => path_vec.push(
                            str::from_utf8(s)
                                .expect("Invalid UTF-8 in path")
                                .to_string(),
                        ),
                        _ => return Err("field 'path' must be a list of strings".to_string()),
                    }
                }
                path_vec
            }
            None => return Err("missing field 'path'".to_string()),
            _ => return Err("field 'path' must be a list".to_string()),
        };

        Ok(BFile { length, path })
    }
}
