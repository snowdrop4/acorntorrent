#![allow(dead_code)]

pub mod config;

mod formatting;

mod metainfo_test;
pub mod metainfo;

pub mod torrent;

pub mod tracker;
mod tracker_test;

mod util;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
