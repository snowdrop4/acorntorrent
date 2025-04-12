#![allow(dead_code)]

use std::path::Path;

use reqwest::Client;
use tokio;

use acorntorrent::config;
use acorntorrent::metainfo;
use acorntorrent::torrent;
use acorntorrent::tracker;

fn main() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            println!("Hello world");
        })
}


async fn announce() -> Result<(), String> {
    // let cmi = metainfo::TInfo {
    // 	piece_length: 5,
    // 	pieces: vec![34, 56, 45, 75, 0, 11, 23, 89, 11, 3],
    // 	private: None,
    // 	source: None,
    // 	name: String::from("test"),
    // 	length: Some(5),
    // 	files: None,
    // };

    // let b = cmi.to_bencode();
    // match b {
    // 	Ok(t)  => println!("{:?}", t),
    // 	Err(e) => println!("{:?}", e.to_string()),
    // }

    let cl = Client::new();
    let ns = config::NetworkSettings {
        ip: None,
        port: 6000,
    };

    let mi = metainfo::BMetainfo::from_path(Path::new("test3.torrent")).unwrap();
    let bt = torrent::BTorrent::new(mi).unwrap();
    let tr = tracker::announce(&cl, &bt, None, &ns).await;

    println!("Torrent: {:#?}", tr);

    Ok(())
}
