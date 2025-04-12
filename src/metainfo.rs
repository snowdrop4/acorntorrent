use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::collections::BTreeMap;
use std::str;

use ring::digest;
use acornbencode::parser::parse_bencode;
use acornbencode::common::BencodeValue;
use acornbencode::encoder;

use crate::util::{get_optional_utf8_value, get_utf8_value};

type DecodingError = String;
type EncodingError = String;


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

        // Extract metainfo from the parsed bencode value
        BMetainfo::from_bencode_value(&value)
    }

    pub fn from_path(path: &Path) -> Result<BMetainfo, DecodingError> {
        let mut f = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
        let mut b = Vec::new();
        f.read_to_end(&mut b).map_err(|e| format!("Failed to read file: {}", e))?;

        BMetainfo::from_bytes(&b)
    }

    fn from_bencode_value_anounce_list(
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
                    BencodeValue::ByteString(s) => announce_tier.push(str::from_utf8(s).expect("Invalid UTF-8 in announce-list").to_string()),
                    _ => return Err("Invalid type in announce-list".to_string()),
                }
            }

            announce_tiers.push(announce_tier);
        }

        Ok(Some(announce_tiers))
    }

    fn from_bencode_value(value: &BencodeValue) -> Result<BMetainfo, DecodingError> {
        let dict = match value {
            BencodeValue::Dictionary(dict) => dict,
            _ => return Err("Metainfo must be a dictionary".to_string()),
        };

        let announce = get_utf8_value(dict, b"announce")?;
        let announce_list = BMetainfo::from_bencode_value_anounce_list(dict)?;
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
                    return Err(format!("only UTF-8 encoding is supported; encountered encoding '{}' instead", e));
                }
                Some(e)
            }
            _ => None,
        };

        let info = match dict.get(b"info".as_ref()) {
            Some(BencodeValue::Dictionary(info_dict)) => BInfo::from_bencode_dict(info_dict)?,
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
}


#[derive(Debug)]
pub struct BInfo {
    // These are mutually exclusive of one another:
    pub files:  Option<Vec<BFile>>, // Multi-file torrents
    pub length: Option<isize>,      // Single-file torrents

    // Suggested title for the torrent.
    // If the torrent is a single-file torrent, this is also the suggested filename.
    pub name: String,

    // Length in bytes of each piece.
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
        } else if let Some(length) = self.length {
            length
        } else {
            0
        }
    }

    // -------------------------------------------------------------------------
    // Hashing
    // -------------------------------------------------------------------------

    pub fn compute_hash(&self) -> Result<Vec<u8>, EncodingError> {
        // Create a BencodeValue dictionary representing this BInfo
        let mut info_dict = BTreeMap::new();

        // Add files or length (mutually exclusive)
        if let Some(files) = &self.files {
            let mut file_list = Vec::new();
            for file in files {
                let mut file_dict = BTreeMap::new();
                file_dict.insert("length".as_bytes(), BencodeValue::Integer(file.length));

                let path_list: Vec<BencodeValue> = file.path.iter()
                    .map(|s| BencodeValue::ByteString(s.as_bytes()))
                    .collect();

                file_dict.insert("path".as_bytes(), BencodeValue::List(path_list));
                file_list.push(BencodeValue::Dictionary(file_dict));
            }
            info_dict.insert("files".as_bytes(), BencodeValue::List(file_list));
        } else if let Some(length) = self.length {
            info_dict.insert("length".as_bytes(), BencodeValue::Integer(length));
        }

        // Add the rest of the fields
        info_dict.insert("name".as_bytes(), BencodeValue::ByteString(&self.name.as_bytes()));
        info_dict.insert("piece length".as_bytes(), BencodeValue::Integer(self.piece_size));
        info_dict.insert("pieces".as_bytes(), BencodeValue::ByteString(&self.pieces));

        if let Some(private) = self.private {
            info_dict.insert("private".as_bytes(), BencodeValue::Integer(if private { 1 } else { 0 }));
        }

        if let Some(source) = &self.source {
            info_dict.insert("source".as_bytes(), BencodeValue::ByteString(source.as_bytes()));
        }

        // Convert to a BencodeValue and encode
        let info_value = BencodeValue::Dictionary(info_dict);
        let encoded = encoder::encode_to_bytes(&info_value)
            .map_err(|e| format!("Failed to encode info: {}", e))?;

        // Calculate the SHA1 hash
        Ok(digest::digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, &encoded).as_ref().to_vec())
    }

    // -------------------------------------------------------------------------
    // Parsing
    // -------------------------------------------------------------------------

    pub fn from_bencode_dict(dict: &BTreeMap<&[u8], BencodeValue>) -> Result<Self, DecodingError> {
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
            return Err("Metainfo files must contain the field 'length' or 'files' (not both or none)".to_string());
        }

        Ok(BInfo {
            files,
            length,
            name,
            piece_size,
            pieces,
            private,
            source,
        })
    }
}


#[derive(Debug)]
pub struct BFile {
    length: isize,
    path: Vec<String>
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
                        BencodeValue::ByteString(s) => path_vec.push(str::from_utf8(s).expect("Invalid UTF-8 in path").to_string()),
                        _ => return Err("field 'path' must be a list of strings".to_string()),
                    }
                }
                path_vec
            }
            None => return Err("missing field 'path'".to_string()),
            _ => return Err("field 'path' must be a list".to_string()),
        };

        Ok(BFile {
            length,
            path,
        })
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torrent_corpus_read() {
        let path = Path::new("test_torrents/");
        let mut err = false;

        for entry in path.read_dir().expect("read_dir call failed") {
            if let Ok(entry) = entry {
                if let Err(e) = BMetainfo::from_path(&entry.path()) {
                    println!("{:?}, {:?}", entry.path(), e);
                    err = true;
                }
            }
        }

        assert!(!err);
    }
}
