use percent_encoding;
use rand::Rng;

use crate::metainfo::BMetainfo;

#[derive(Debug)]
pub struct BTorrent {
    pub metainfo: BMetainfo,

    pub info_hash: Vec<u8>,
    pub encoded_info_hash: String,

    pub peer_id: Vec<u8>,
    pub encoded_peer_id: String,

    pub uploaded: u64,
    pub downloaded: u64,
    pub left: u64,
}

impl BTorrent {
    pub fn new(metainfo: BMetainfo) -> Result<BTorrent, String> {
        let info_hash = metainfo.info.compute_hash().map_err(|e| e.to_string())?;
        let encoded_info_hash =
            percent_encoding::percent_encode(&info_hash, percent_encoding::NON_ALPHANUMERIC)
                .to_string();

        let peer_id = rand::thread_rng().gen::<[u8; 20]>().to_vec();
        let encoded_peer_id =
            percent_encoding::percent_encode(&peer_id, percent_encoding::NON_ALPHANUMERIC)
                .to_string();

        Ok(BTorrent {
            metainfo,

            info_hash,
            encoded_info_hash,

            peer_id,
            encoded_peer_id,

            uploaded: 0,
            downloaded: 0,
            left: 0,
        })
    }
}
