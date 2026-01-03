#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::metainfo::BMetainfo;
    use crate::util::formatting::{
        format_datetime_to_localtime, fuzzy_format_bytes_to_si, parse_size_to_bytes,
    };

    #[derive(Debug)]
    struct TorrentInfo {
        name: Option<String>,
        hash_v1: Option<String>,
        created_by: Option<String>,
        created_on: Option<String>,
        comment: Option<String>,
        piece_count: Option<String>,
        piece_size: Option<String>,
        total_size: Option<String>, // the total size of the metainfo file (not the total size of the pieces)
        privacy: Option<String>,
    }

    impl TorrentInfo {
        fn new() -> Self {
            TorrentInfo {
                name: None,
                hash_v1: None,
                created_by: None,
                created_on: None,
                comment: None,
                piece_count: None,
                piece_size: None,
                total_size: None,
                privacy: None,
            }
        }
    }

    fn parse_transmission_show_info(input: &str) -> TorrentInfo {
        let mut info = TorrentInfo::new();

        for line in input.lines() {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Check for section headers
            if trimmed == "GENERAL" {
                continue;
            }

            // Parse key-value pairs
            if let Some(colon_idx) = trimmed.find(':') {
                let key = trimmed[..colon_idx].trim();
                let value = trimmed[colon_idx + 1..].trim();

                match key {
                    "Name" => info.name = Some(value.to_string()),
                    "Hash v1" => info.hash_v1 = Some(value.to_string()),
                    "Created by" => {
                        if value == "Unknown" {
                            info.created_by = None;
                        } else {
                            info.created_by = Some(value.to_string())
                        }
                    }
                    "Created on" => {
                        if value == "Unknown" {
                            info.created_on = None;
                        } else {
                            info.created_on = Some(value.to_string())
                        }
                    }
                    "Comment" => info.comment = Some(value.to_string()),
                    "Piece Count" => info.piece_count = Some(value.to_string()),
                    "Piece Size" => info.piece_size = Some(value.to_string()),
                    "Total Size" => info.total_size = Some(value.to_string()),
                    "Privacy" => info.privacy = Some(value.to_string()),
                    _ => {} // Ignore unknown fields
                }
            }
        }

        info
    }

    fn parse_transmission_show_from_command(torrent_file: &str) -> TorrentInfo {
        let output = Command::new("transmission-show")
            .arg("--info")
            .arg(torrent_file)
            .output()
            .unwrap();

        if !output.status.success() {
            panic!("Failed to execute transmission-show");
        }

        let stdout = String::from_utf8(output.stdout).unwrap();
        parse_transmission_show_info(&stdout)
    }

    #[test]
    fn test_torrent_corpus_read() {
        let path = Path::new("test_torrents/");
        let mut err = false;

        for entry in path.read_dir().expect("read_dir call failed") {
            let entry = entry.expect("Failed to read dir entry");

            if let Err(e) = BMetainfo::from_path(&entry.path()) {
                println!("{:?}, {:?}", entry.path(), e);
                err = true;
            }
        }

        assert!(!err);
    }

    #[test]
    fn test_torrent_corpus_transmission_show_info() {
        let test_dir = PathBuf::from("test_torrents");
        let mut torrent_files: Vec<_> = fs::read_dir(&test_dir)
            .expect("Failed to read test_torrents directory")
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()? == "torrent" {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        torrent_files.sort();

        for torrent_file in torrent_files {
            let path_str = torrent_file.to_str().unwrap();

            let expected = parse_transmission_show_from_command(path_str);
            let actual = BMetainfo::from_path(&torrent_file).unwrap();

            println!("----------");
            println!("Path: {}", &path_str);
            println!("----------");

            println!(
                "Hash v1:        {:?}, {:?}",
                hex::encode(actual.info.compute_hash().unwrap()),
                expected.hash_v1
            );
            assert_eq!(
                hex::encode(actual.info.compute_hash().unwrap()),
                expected.hash_v1.unwrap()
            );

            println!(
                "Created By:     {:?}, {:?}",
                actual.created_by, expected.created_by
            );
            assert_eq!(actual.created_by, expected.created_by);

            println!(
                "Created On:     {:?}, {:?}",
                actual.created_on, expected.created_on
            );
            if actual.created_on.is_some() || expected.created_on.is_some() {
                assert_eq!(actual.created_on.is_some(), expected.created_on.is_some());
                assert_eq!(
                    format_datetime_to_localtime(actual.created_on.unwrap() as i64),
                    expected.created_on.unwrap()
                );
            }

            println!(
                "Comment:        {:?}, {:?}",
                actual.comment, expected.comment
            );
            assert_eq!(actual.comment, expected.comment);

            println!(
                "Piece Count:    {:?}, {:?}",
                actual.info.total_piece_count(),
                expected.piece_count
            );
            assert_eq!(
                actual.info.total_piece_count().to_string(),
                expected.piece_count.unwrap()
            );

            let expected_piece_size_bytes =
                parse_size_to_bytes(&expected.piece_size.unwrap()).unwrap();
            println!(
                "Piece Size:     {:?}, {:?}",
                actual.info.piece_size as u64, expected_piece_size_bytes
            );
            assert_eq!(actual.info.piece_size as u64, expected_piece_size_bytes);

            let expected_total_size_str = expected.total_size.as_ref().unwrap();
            let actual_total_size_bytes = actual.info.metainfo_total_size_bytes() as u64;
            let actual_total_size_formatted = fuzzy_format_bytes_to_si(actual_total_size_bytes);
            println!(
                "Total Size:     {} bytes -> {:?}, {:?}",
                actual_total_size_bytes, actual_total_size_formatted, expected_total_size_str
            );
            assert_eq!(actual_total_size_formatted, *expected_total_size_str);

            println!(
                "Private:        {:?}, {:?}",
                actual.info.private, expected.privacy
            );
            if actual.info.private.is_some() || expected.privacy.is_some() {
                let actual_is_private = actual.info.private.unwrap_or(false);
                let expected_is_private = expected.privacy.unwrap() != "Public torrent";
                assert_eq!(actual_is_private, expected_is_private);
            }
        }
    }
}
