#![allow(dead_code)]

use std::path::PathBuf;

use acorntorrent::{
    config::NetworkSettings,
    metainfo::BMetainfo,
    torrent::BTorrent,
    tracker::{announce_to_tracker, BAnnounceEvent, BTrackerResponse},
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "acorntorrent")]
#[command(about = "A BitTorrent client", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Announce to tracker
    Announce {
        /// Path to the .torrent file
        #[arg(short, long)]
        torrent: PathBuf,

        /// Port to announce (default: 6881)
        #[arg(short, long, default_value = "6881")]
        port: u16,

        /// Event type (started, completed, stopped)
        #[arg(short, long)]
        event: Option<String>,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Announce {
            torrent,
            port,
            event,
            verbose,
        } => {
            // Load the torrent file
            if verbose {
                println!("Loading torrent file: {}", torrent.display());
            }
            let metainfo = BMetainfo::from_path(&torrent)
                .map_err(|e| format!("Failed to load torrent: {}", e))?;

            // Create torrent instance
            let mut btorrent =
                BTorrent::new(metainfo).map_err(|e| format!("Failed to create torrent: {}", e))?;

            // For testing purposes, set the file size from metainfo
            // In a real implementation, this would be calculated from actual downloaded files
            btorrent.left = btorrent.metainfo.info.metainfo_total_size_bytes() as u64;

            if verbose {
                println!("Tracker URL: {}", btorrent.metainfo.announce);
                println!("Info hash: {}", hex::encode(&btorrent.info_hash));
                println!("Peer ID: {}", hex::encode(&btorrent.peer_id));
            }

            // Parse event
            let announce_event = event.as_ref().map(|e| match e.as_str() {
                "started" => BAnnounceEvent::Started,
                "completed" => BAnnounceEvent::Completed,
                "stopped" => BAnnounceEvent::Stopped,
                _ => {
                    eprintln!("Warning: Unknown event '{}', using 'started'", e);
                    BAnnounceEvent::Started
                }
            });

            // Create network settings
            let network_settings = NetworkSettings {
                port: port.into(),
                ip: None,
            };

            // Announce to tracker
            if verbose {
                println!("Announcing to tracker...");
            }

            let client = reqwest::Client::new();
            let response =
                announce_to_tracker(&client, &btorrent, announce_event, &network_settings).await?;

            if verbose {
                println!("Response status: {}", response.status());
            }

            // Parse the response
            let response_bytes = response.bytes().await?;

            match BTrackerResponse::from_bytes(&response_bytes) {
                Ok(tracker_response) => {
                    println!("✓ Successfully announced to tracker");
                    if verbose {
                        println!("Tracker response: {:?}", tracker_response);
                    }
                    Ok(())
                }
                Err(e) => {
                    eprintln!("✗ Failed to parse tracker response: {}", e);
                    eprintln!(
                        "Raw response: {:?}",
                        String::from_utf8_lossy(&response_bytes)
                    );
                    std::process::exit(1);
                }
            }
        }
    }
}
