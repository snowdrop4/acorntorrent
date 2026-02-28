use std::collections::BTreeMap;

use crate::torrent::BTorrent;

pub enum CTorrentState {
    // Incomplete states
    Paused,      // Not downloading
    Downloading, // Not paused

    // Complete states
    Stopped, // Not seeding
    Seeding, // Not stopped

    // Verifying states
    VerifyingActive,   // Previous state was downloading or seeding
    VerifyingInactive, // Previous state was paused or stopped

    // Invalid states
    FileNotFound, // We tried to resume an existing torrent, but the downloaded files are missing
}

pub struct CTorrent {
    /// Path to the .torrent file
    torrent_path: String,

    /// Directory where the downloaded files are stored
    download_directory: String,

    btorrent: BTorrent,
}

pub struct CState {
    ctorrents: BTreeMap<Vec<u8>, CTorrent>,
}

impl CState {
    pub fn new() -> CState {
        CState {
            ctorrents: BTreeMap::new(),
        }
    }

    pub fn load(state_path: &std::path::Path) -> Result<CState, Box<dyn std::error::Error>> {
        // TODO: deserialize state from disk
        Ok(CState::new())
    }

    pub fn save(&self, state_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: serialize state to disk
        Ok(())
    }
}
