pub mod config;
pub mod metainfo;
pub mod torrent;
pub mod tracker;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
