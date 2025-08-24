#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;
    use rstest::rstest;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path_regex};

    use reqwest::Client;
    use tokio;

    use crate::config;
    use crate::metainfo;
    use crate::torrent;
    use crate::tracker;

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

    #[rstest]
    #[timeout(Duration::from_millis(100))]
    #[tokio::test]
    async fn test_announce(
        #[files("test_torrents/*.torrent")]
        torrent_file: PathBuf
    ) -> Result<(), String> {
        use crate::tracker::BTrackerResponse;

        let mock_server = setup_mock_tracker().await;
        let local_tracker_url = format!("{}/announce", mock_server.uri());

        let cl = Client::new();
        let ns = config::NetworkSettings {
            ip: None,
            port: 6000,
        };

        let mut mi = metainfo::BMetainfo::from_path(torrent_file.as_path()).unwrap();
        // Override the tracker URL to use our local mock server
        mi.announce = local_tracker_url;

        let bt = torrent::BTorrent::new(mi).unwrap();
        let tr = tracker::announce_to_tracker(&cl, &bt, None, &ns).await;

        assert!(tr.is_ok(), "Tracker announce should succeed with local tracker");

        let tr = tr.unwrap().bytes().await.unwrap();

        let tr = BTrackerResponse::from_bytes(&tr);

        println!("Response: {:#?}", tr);

        Ok(())
    }
}
