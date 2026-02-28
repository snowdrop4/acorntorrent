#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use reqwest::Client;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path_regex},
    };

    use crate::{config, metainfo, torrent, tracker, tracker::BTrackerResponse};

    async fn setup_mock_tracker() -> MockServer {
        let mock_server = MockServer::start().await;
        let mock_tracker_response = b"d8:intervali1800e5:peersld2:ip9:127.0.0.17:peer id20:ABCDEFGHIJ01234567894:porti6881eed2:ip9:127.0.0.27:peer id20:BCDEFGHIJK12345678904:porti6882eee";

        Mock::given(method("GET"))
            .and(path_regex(r"^/announce"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(mock_tracker_response))
            .mount(&mock_server)
            .await;

        mock_server
    }

    #[tokio::test]
    async fn test_announce() -> Result<(), String> {
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
            let mock_server = setup_mock_tracker().await;
            let local_tracker_url = format!("{}/announce", mock_server.uri());

            let cl = Client::new();
            let ns = config::CNetworkSettings {
                ip: None,
                port: 6000,
            };

            let mut mi = metainfo::BMetainfo::from_path(torrent_file.as_path()).unwrap();
            // Override the tracker URL to use our local mock server
            mi.announce = local_tracker_url;

            let bt = torrent::BTorrent::new(mi).unwrap();
            let tr = tracker::announce_to_tracker(&cl, &bt, None, &ns).await;

            assert!(
                tr.is_ok(),
                "Tracker announce should succeed with local tracker"
            );

            let tr = tr.unwrap().bytes().await.unwrap();

            let tr = BTrackerResponse::from_bytes(&tr);

            println!("Response: {:#?}", tr);
        }

        Ok(())
    }
}
