#![allow(dead_code)]

pub mod config;

mod metainfo_test;
pub mod metainfo;

pub mod torrent;

mod tracker_test;
pub mod tracker;

mod util;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
