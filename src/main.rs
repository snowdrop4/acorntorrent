use acorntorrent::config::CClientConfig;
use acorntorrent::state::{CState, CTorrent};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

enum StateMessage {
    // Torrents
    AddTorrent { torrent: CTorrent },
    RemoveTorrent { info_hash: Vec<u8> }, // Delete entry, keep files on disk
    PurgeTorrent { info_hash: Vec<u8> },  // Delete entry, delete files on disk

    // Downloading
    PieceCompleted { info_hash: Vec<u8>, piece: u64 },

    // Other
    Shutdown,
}

fn state_actor(mut state: CState, recv: Receiver<StateMessage>, state_path: &std::path::Path) {
    loop {
        let msg = recv.recv();
        let msg = msg.unwrap_or(StateMessage::Shutdown);

        match msg {
            StateMessage::AddTorrent { torrent } => {}
            StateMessage::RemoveTorrent { info_hash } => {}
            StateMessage::PurgeTorrent { info_hash } => {}
            StateMessage::PieceCompleted { info_hash, piece } => {}
            StateMessage::Shutdown => {
                let _ = state.save(state_path);
                break;
            }
        }
    }
}

struct StateHandle {
    send: Sender<StateMessage>,
}

impl StateHandle {
    fn add_torrent(&self, torrent: CTorrent) {
        let result = self.send.send(StateMessage::AddTorrent { torrent });
    }

    fn remove_torrent(&self, info_hash: Vec<u8>) {
        let result = self.send.send(StateMessage::RemoveTorrent { info_hash });
    }

    fn purge_torrent(&self, info_hash: Vec<u8>) {
        let result = self.send.send(StateMessage::PurgeTorrent { info_hash });
    }

    fn piece_completed(&self, info_hash: Vec<u8>, piece: u64) {
        let result = self
            .send
            .send(StateMessage::PieceCompleted { info_hash, piece });
    }

    fn shutdown(&self) {
        let result = self.send.send(StateMessage::Shutdown);
    }
}

fn main() {
    let config_path = PathBuf::from("config.toml");
    let config = CClientConfig::from_file(&config_path).expect("failed to read config.toml");

    let cstate = CState::load(&config.state_path).unwrap_or_else(|_| CState::new());

    let (send, recv) = channel::<StateMessage>();
    let state_handle = StateHandle { send };

    let state_path = config.state_path.clone();
    thread::spawn(move || state_actor(cstate, recv, &state_path));
}
