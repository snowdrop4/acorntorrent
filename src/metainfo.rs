use std::fs::File;
use std::io::Read;
use std::path::Path;

use ring::digest;
use bendy::{
    decoding::{FromBencode, Decoder, Object, Error as DecodingError, ResultExt},
    encoding::{ToBencode, SingleItemEncoder, Error as EncodingError, AsString},
};
use failure::err_msg;


#[derive(Debug)]
pub struct BMetainfo {
    pub announce: String,

    pub announce_list: Option<Vec<Vec<String>>>,

    // Free-form comment.
    pub comment: Option<String>,

    // The torrent client/library/tool that created the torrent.
    pub created_by: Option<String>,

    // Seconds eince epoch.
    pub creation_date: Option<u64>,

    // Encoding used for the filenames in `info`. Assumed to be UTF-8 if not present.
    // If present and not set to 'UTF-8', parsing will raise an error.
    pub encoding: Option<String>,

    pub info: BInfo,
}

impl BMetainfo {
    pub fn from_bytes(bytes: &[u8]) -> Result<BMetainfo, DecodingError> {
        let mut decoder = Decoder::new(&bytes);

        // Read in and then parse the metainfo dictionary
        let metainfo = decoder.next_object()?
            .ok_or_else(|| DecodingError::malformed_content(
                err_msg("encountered EOF before metainfo dictionary")
            ))?;
        let metainfo = BMetainfo::decode_bencode_object(metainfo);

        // Ensure we've hit EOF
        if decoder.next_object()?.is_some() {
            return Err(DecodingError::malformed_content(
                err_msg("erroneous data at the end of the metainfo file")
            ))
        }

        metainfo
    }

    pub fn from_path(path: &Path) -> Result<BMetainfo, DecodingError> {
        let mut f = File::open(path).unwrap();
        let mut b = Vec::new();
        f.read_to_end(&mut b).unwrap();

        BMetainfo::from_bytes(&b)
    }
}

impl FromBencode for BMetainfo {
    fn decode_bencode_object(object: Object) -> Result<Self, DecodingError> {
        let mut announce      = None;
        let mut announce_list = None;
        let mut comment       = None;
        let mut created_by    = None;
        let mut creation_date = None;
        let mut encoding      = None;
        let mut info          = None;

        let mut dict = object.try_into_dictionary()?;
        while let Some(keyval) = dict.next_pair()? {
            match keyval {
                (b"announce", val) => {
                    announce = String::decode_bencode_object(val)
                        .context("announce")
                        .map(Some)?;
                }
                (b"announce-list", val) => {
                    announce_list = Vec::decode_bencode_object(val)
                        .context("announce-list")
                        .map(Some)?;
                }
                (b"comment", val) => {
                    comment = String::decode_bencode_object(val)
                        .context("comment")
                        .map(Some)?;
                }
                (b"created by", val) => {
                    created_by = String::decode_bencode_object(val)
                        .context("created by")
                        .map(Some)?;
                }
                (b"creation date", val) => {
                    creation_date = u64::decode_bencode_object(val)
                        .context("creation date")
                        .map(Some)?;
                }
                (b"encoding", val) => {
                    let e = String::decode_bencode_object(val)
                        .context("encoding")?;

                    if e.to_lowercase() != "utf-8" {
                        return Err(DecodingError::malformed_content(
                            err_msg(format!("only UTF-8 encoding is supported; encountered encoding '{}' instead", e))
                        ))
                    }

                    encoding = Some(e);
                }
                (b"info", val) => {
                    info = BInfo::decode_bencode_object(val)
                        .context("info")
                        .map(Some)?;
                }
                (key, _) => {
                    return Err(DecodingError::unexpected_field(String::from_utf8_lossy(key)));
                }
            }
        }

        let announce = announce.ok_or_else(|| DecodingError::missing_field("announce"))?;
        let info     =     info.ok_or_else(|| DecodingError::missing_field("info"    ))?;

        Ok(BMetainfo {
            announce,
            announce_list,
            comment,
            created_by,
            creation_date,
            encoding,
            info
        })
    }
}


#[derive(Debug)]
pub struct BInfo {
    // These are mutually exclusive of one another:
    pub files:  Option<Vec<BFile>>, // Multi-file torrents
    pub length: Option<u64>,        // Single-file torrents

    // Suggested title for the torrent, and, if the torrent is a single-file torrent, the suggested filename.
    pub name: String,

    // Length in bytes of each piece.
    pub piece_length: u64,

    // 20-byte hashes of every single piece concated together.
    pub pieces: Vec<u8>,

    // Whether DHT should be disabled or not.
    pub private: Option<bool>,

    // The tracker the torrent came from, in order to enforce a unique infohash.
    // This is used by private trackers to stop their peer lists being leaked if the same
    // torrent is uploaded to multiple private trackers, and added to the same client,
    // since each private tracker will force a different infohash by adding their own `source` tag.
    pub source: Option<String>,
}

impl BInfo {
    pub fn compute_hash(&self) -> Result<Vec<u8>, EncodingError> {
        let bencoded = self.to_bencode()?;

        Ok(digest::digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, &bencoded).as_ref().to_vec())
    }
}

impl FromBencode for BInfo {
    fn decode_bencode_object(object: Object) -> Result<Self, DecodingError> {
        let mut files        = None; // Multi-file torrents
        let mut length       = None; // Single-file torrents
        let mut name         = None;
        let mut piece_length = None;
        let mut pieces       = None;
        let mut private      = None;
        let mut source       = None;

        let mut dict = object.try_into_dictionary()?;
        while let Some(keyval) = dict.next_pair()? {
            match keyval {
                (b"files", val) => {
                    files = Vec::decode_bencode_object(val)
                        .context("files")
                        .map(Some)?;
                }
                (b"length", val) => {
                    length = u64::decode_bencode_object(val)
                        .context("length")
                        .map(Some)?;
                }
                (b"name", val) => {
                    name = String::decode_bencode_object(val)
                        .context("name")
                        .map(Some)?;
                }
                (b"piece length", val) => {
                    piece_length = u64::decode_bencode_object(val)
                        .context("piece length")
                        .map(Some)?;
                }
                (b"pieces", val) => {
                    // `AsString` is a wrapper allowing us to decode/encode a Vec<u8>.
                    // It contains only one field -- the Vec<u8>. Unwrap it.
                    pieces = AsString::decode_bencode_object(val)
                        .context("pieces")
                        .map(|b| Some(b.0))?;
                }
                (b"private", val) => {
                    private = u64::decode_bencode_object(val)
                        .context("private")
                        .map(|i| Some(i != 0))?;
                }
                (b"source", val) => {
                    source = String::decode_bencode_object(val)
                        .context("source")
                        .map(Some)?;
                }
                (key, _) => {
                    return Err(DecodingError::unexpected_field(String::from_utf8_lossy(key)));
                }
            }
        }

        let name         =         name.ok_or_else(|| DecodingError::missing_field("name"        ))?;
        let piece_length = piece_length.ok_or_else(|| DecodingError::missing_field("piece_length"))?;
        let pieces       =       pieces.ok_or_else(|| DecodingError::missing_field("pieces"      ))?;

        if length.is_some() == files.is_some() {
            return Err(DecodingError::malformed_content(
                err_msg("metainfo files must contain the key `length` or `files` (not both or none)")
            ))
        }

        Ok(BInfo {
            files,
            length,
            name,
            piece_length,
            pieces,
            private,
            source,
        })
    }
}

impl ToBencode for BInfo {
    const MAX_DEPTH: usize = usize::MAX;

    // Pairs MUST be emitted in alphabetical order, else the encoder will return an error.
    //
    // Keys MUST be alphabetically sorted when calculating the info hash,
    // to ensure one canonical info hash. This is thus guaranteed.
    fn encode(&self, encoder: SingleItemEncoder) -> Result<(), EncodingError> {
        encoder.emit_dict(|mut e| {
            if let Some(files) = &self.files {
                e.emit_pair(b"files", files)?;
            }

            if let Some(length) = &self.length {
                e.emit_pair(b"length", length)?;
            }

            e.emit_pair(b"name", &self.name)?;

            e.emit_pair(b"piece length", &self.piece_length)?;
            e.emit_pair(b"pieces", AsString(&self.pieces))?;

            if let Some(private) = &self.private {
                e.emit_pair(b"private", *private as u64)?;
            }

            if let Some(source) = &self.source {
                e.emit_pair(b"source", source)?;
            }

            Ok(())
        })?;

        Ok(())
    }
}


#[derive(Debug)]
pub struct BFile {
    length: u64,
    path: Vec<String>
}

impl FromBencode for BFile {
    fn decode_bencode_object(object: Object) -> Result<Self, DecodingError> {
        // Struct fields:
        let mut length = None;
        let mut path   = None;

        let mut dict = object.try_into_dictionary()?;
        while let Some(keyval) = dict.next_pair()? {
            match keyval {
                (b"length", val) => {
                    length = u64::decode_bencode_object(val)
                        .context("length")
                        .map(Some)?;
                }
                (b"path", val) => {
                    path = Vec::decode_bencode_object(val)
                        .context("path")
                        .map(Some)?;
                }
                (key, _) => {
                    return Err(DecodingError::unexpected_field(String::from_utf8_lossy(key)));
                }
            }
        }

        let length = length.ok_or_else(|| DecodingError::missing_field("length"))?;
        let path   =   path.ok_or_else(|| DecodingError::missing_field("path"  ))?;

        Ok(BFile {
            length,
            path,
        })
    }
}

impl ToBencode for BFile {
    const MAX_DEPTH: usize = usize::MAX;

    // Pairs MUST be emitted in alphabetical order, else the encoder will return an error.
    //
    // Keys MUST be alphabetically sorted when calculating the info hash,
    // to ensure one canonical info hash. This is thus guaranteed.
    fn encode(&self, encoder: SingleItemEncoder) -> Result<(), EncodingError> {
        encoder.emit_dict(|mut e| {
            e.emit_pair(b"length", &self.length)?;
            e.emit_pair(b"path",   &self.path)
        })?;

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_corpus() {
        let path = Path::new("test_torrents/");
        let mut err = false;

        for entry in path.read_dir().expect("read_dir call failed") {
            if let Ok(entry) = entry {
                if let Err(e) = TMetainfo::from_path(&entry.path()) {
                    println!("{:?}", e);
                    err = true;
                }
            }
        }

        assert!(!err);
    }
}
