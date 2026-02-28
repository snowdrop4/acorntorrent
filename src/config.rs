use serde::Deserialize;
use std::path::{Path, PathBuf};

pub struct CNetworkSettings {
    pub ip: Option<String>,
    pub port: u64,
}

#[derive(Deserialize)]
pub struct CClientConfig {
    /// Path to store incomplete torrent data
    pub incomplete_path: PathBuf,

    /// Path to store complete torrent data (may be the same as `incomplete_path`)
    pub complete_path: PathBuf,

    /// Path to store `.torrent`s.
    pub torrent_path: PathBuf,

    /// Path to store state data
    pub state_path: PathBuf,
}

impl CClientConfig {
    pub fn from_file(path: &Path) -> Result<CClientConfig, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: CClientConfig = toml::from_str(&contents)?;
        Ok(config)
    }
}
