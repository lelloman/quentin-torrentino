//! Torrent client abstraction.
//!
//! This module provides a `TorrentClient` trait for managing torrents across
//! various backends (qBittorrent, librqbit, etc.).

mod librqbit;
mod qbittorrent;
mod types;

pub use librqbit::LibrqbitClient;
pub use qbittorrent::QBittorrentClient;
pub use types::*;
