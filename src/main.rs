use std::path::Path;

use reqwest::Client;
use bendy::encoding::{ToBencode, Error};

use acorntorrent::metainfo;
use acorntorrent::torrent;
use acorntorrent::tracker;
use acorntorrent::config;


fn main() -> Result<(), String> {
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
	let ih = metainfo.info.compute_hash()
		.map_err(|e| e.to_string())?;
	let bt = torrent::BTorrent::new(mi, ih).unwrap();
	let tr = tracker::announce(&cl, &bt, None, &ns);
	
	println!("Torrent: {:#?}", tr);
	
	Ok(())
}
