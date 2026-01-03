//! Torrent file parser - extracts file listings from .torrent files.
//!
//! Uses librqbit-core to parse bencoded .torrent data and extract
//! the file listing (paths and sizes) without needing to download anything.

use librqbit_core::torrent_metainfo::{torrent_from_bytes, TorrentMetaV1Owned};
use thiserror::Error;

use super::TorrentFile;

/// Errors that can occur when parsing torrent files.
#[derive(Debug, Error)]
pub enum TorrentParseError {
    #[error("Failed to parse torrent: {0}")]
    ParseError(String),

    #[error("Invalid UTF-8 in file path: {0}")]
    InvalidPath(String),

    #[error("Empty torrent (no files)")]
    EmptyTorrent,
}

/// Parse a .torrent file and extract the file listing.
///
/// Supports both single-file and multi-file torrents.
///
/// # Arguments
/// * `bytes` - Raw bytes of the .torrent file
///
/// # Returns
/// A vector of `TorrentFile` with path and size for each file.
pub fn parse_torrent_files(bytes: &[u8]) -> Result<Vec<TorrentFile>, TorrentParseError> {
    let torrent: TorrentMetaV1Owned =
        torrent_from_bytes(bytes).map_err(|e| TorrentParseError::ParseError(e.to_string()))?;

    let info = &torrent.info;

    // Get the root name (folder name for multi-file, file name for single-file)
    let root_name = info
        .name
        .as_ref()
        .map(|b| bytes_to_string(b.as_ref()))
        .transpose()?
        .unwrap_or_else(|| "unknown".to_string());

    // Check if it's a multi-file torrent
    if let Some(ref files) = info.files {
        // Multi-file torrent
        let mut result = Vec::with_capacity(files.len());

        for file in files {
            // Build the full path: root_name/path/components
            let mut path_parts = vec![root_name.clone()];
            for part in &file.path {
                path_parts.push(bytes_to_string(part.as_ref())?);
            }
            let full_path = path_parts.join("/");

            result.push(TorrentFile {
                path: full_path,
                size_bytes: file.length,
            });
        }

        if result.is_empty() {
            return Err(TorrentParseError::EmptyTorrent);
        }

        Ok(result)
    } else if let Some(length) = info.length {
        // Single-file torrent
        Ok(vec![TorrentFile {
            path: root_name,
            size_bytes: length,
        }])
    } else {
        Err(TorrentParseError::EmptyTorrent)
    }
}

/// Extract the info_hash from a .torrent file.
///
/// Returns the lowercase hex string of the info_hash.
pub fn parse_torrent_info_hash(bytes: &[u8]) -> Result<String, TorrentParseError> {
    let torrent: TorrentMetaV1Owned =
        torrent_from_bytes(bytes).map_err(|e| TorrentParseError::ParseError(e.to_string()))?;

    Ok(torrent.info_hash.as_string())
}

/// Convert bytes to a UTF-8 string, handling common encoding issues.
fn bytes_to_string(bytes: &[u8]) -> Result<String, TorrentParseError> {
    // Try UTF-8 first
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok(s.to_string());
    }

    // Fall back to lossy conversion (replaces invalid chars with ?)
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_invalid_torrent() {
        let result = parse_torrent_files(b"not a valid torrent");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_data() {
        let result = parse_torrent_files(b"");
        assert!(result.is_err());
    }

    #[test]
    fn test_bytes_to_string_valid_utf8() {
        let result = bytes_to_string(b"hello world").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_bytes_to_string_invalid_utf8() {
        // Invalid UTF-8 sequence should be handled with lossy conversion
        let invalid = vec![0xff, 0xfe, 0x68, 0x65, 0x6c, 0x6c, 0x6f]; // some invalid bytes followed by "hello"
        let result = bytes_to_string(&invalid).unwrap();
        // Should not panic, result contains replacement characters
        assert!(result.contains("hello"));
    }

    // Integration tests with real .torrent files would go here.
    // These require actual torrent files which aren't included in the repo.
    // To test manually:
    // 1. Download any .torrent file
    // 2. Read it as bytes
    // 3. Call parse_torrent_files() and verify the output
}
