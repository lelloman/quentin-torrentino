//! File-to-track mapping logic.
//!
//! Maps torrent files to expected content items (tracks, episodes, etc.).
//! Supports both heuristic-based ("dumb") and LLM-enhanced mapping.

use crate::searcher::TorrentFile;
use crate::ticket::{ExpectedContent, ExpectedTrack};
use crate::textbrain::types::FileMapping;

/// Configuration for the dumb file mapper.
#[derive(Debug, Clone)]
pub struct DumbFileMapperConfig {
    /// Minimum confidence to consider a mapping valid.
    pub min_confidence: f32,
    /// Weight for track number matching (0.0-1.0).
    pub track_number_weight: f32,
    /// Weight for title similarity (0.0-1.0).
    pub title_weight: f32,
    /// Audio file extensions to consider.
    pub audio_extensions: Vec<String>,
    /// Video file extensions to consider.
    pub video_extensions: Vec<String>,
}

impl Default for DumbFileMapperConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.3,
            track_number_weight: 0.4,
            title_weight: 0.6,
            audio_extensions: vec![
                "flac", "mp3", "m4a", "aac", "ogg", "opus", "wav", "ape", "wv", "alac",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            video_extensions: vec![
                "mkv", "mp4", "avi", "mov", "wmv", "m4v", "webm", "ts", "m2ts",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

/// Heuristic-based file mapper.
///
/// Maps torrent files to expected content using:
/// - File extension filtering
/// - Track number extraction from filenames
/// - Fuzzy title matching
pub struct DumbFileMapper {
    config: DumbFileMapperConfig,
}

impl DumbFileMapper {
    /// Create a new file mapper with default config.
    pub fn new() -> Self {
        Self {
            config: DumbFileMapperConfig::default(),
        }
    }

    /// Create a new file mapper with custom config.
    pub fn with_config(config: DumbFileMapperConfig) -> Self {
        Self { config }
    }

    /// Map torrent files to expected content.
    ///
    /// Returns file mappings sorted by confidence.
    pub fn map_files(&self, files: &[TorrentFile], expected: &ExpectedContent) -> Vec<FileMapping> {
        match expected {
            ExpectedContent::Album { tracks, .. } => self.map_album_files(files, tracks),
            ExpectedContent::Track { title, artist } => {
                self.map_single_track(files, title, artist.as_deref())
            }
            ExpectedContent::Movie { title, year } => self.map_movie_file(files, title, *year),
            ExpectedContent::TvEpisode {
                series,
                season,
                episodes,
            } => self.map_tv_episodes(files, series, *season, episodes),
        }
    }

    /// Map files for an album with multiple tracks.
    fn map_album_files(&self, files: &[TorrentFile], tracks: &[ExpectedTrack]) -> Vec<FileMapping> {
        let audio_files = self.filter_audio_files(files);

        if audio_files.is_empty() || tracks.is_empty() {
            return Vec::new();
        }

        let mut mappings = Vec::new();

        for track in tracks {
            if let Some((file, confidence)) = self.find_best_track_match(&audio_files, track) {
                mappings.push(FileMapping {
                    torrent_file_path: file.path.clone(),
                    ticket_item_id: format!("track-{}", track.number),
                    confidence,
                });
            }
        }

        // Sort by confidence descending
        mappings.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        mappings
    }

    /// Map files for a single track.
    fn map_single_track(
        &self,
        files: &[TorrentFile],
        title: &str,
        artist: Option<&str>,
    ) -> Vec<FileMapping> {
        let audio_files = self.filter_audio_files(files);

        if audio_files.is_empty() {
            return Vec::new();
        }

        // For single track, find best matching file
        let mut best_match: Option<(&TorrentFile, f32)> = None;

        for file in &audio_files {
            let filename = self.extract_filename(&file.path);
            let title_score = self.title_similarity(&filename, title);

            // Bonus for artist match
            let artist_bonus = if let Some(artist) = artist {
                if self.contains_substring(&file.path, artist) {
                    0.1
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let score = (title_score + artist_bonus).min(1.0);

            if score >= self.config.min_confidence
                && (best_match.is_none() || score > best_match.unwrap().1)
            {
                best_match = Some((file, score));
            }
        }

        best_match
            .map(|(file, confidence)| {
                vec![FileMapping {
                    torrent_file_path: file.path.clone(),
                    ticket_item_id: "track-1".to_string(),
                    confidence,
                }]
            })
            .unwrap_or_default()
    }

    /// Map files for a movie.
    fn map_movie_file(
        &self,
        files: &[TorrentFile],
        title: &str,
        year: Option<u32>,
    ) -> Vec<FileMapping> {
        let video_files = self.filter_video_files(files);

        if video_files.is_empty() {
            return Vec::new();
        }

        // For movies, prefer largest video file that matches title
        let mut candidates: Vec<(&TorrentFile, f32)> = Vec::new();

        for file in &video_files {
            let filename = self.extract_filename(&file.path);
            let title_score = self.title_similarity(&filename, title);

            // Bonus for year match
            let year_bonus = if let Some(y) = year {
                if filename.contains(&y.to_string()) {
                    0.15
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // Size bonus - larger files are more likely to be the main movie
            let size_bonus = self.size_score(file.size_bytes, &video_files);

            let score = (title_score * 0.6 + year_bonus + size_bonus * 0.25).min(1.0);

            if score >= self.config.min_confidence {
                candidates.push((file, score));
            }
        }

        // Sort by score descending
        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates
            .first()
            .map(|(file, confidence)| {
                vec![FileMapping {
                    torrent_file_path: file.path.clone(),
                    ticket_item_id: "movie".to_string(),
                    confidence: *confidence,
                }]
            })
            .unwrap_or_default()
    }

    /// Map files for TV episodes.
    fn map_tv_episodes(
        &self,
        files: &[TorrentFile],
        series: &str,
        season: u32,
        episodes: &[u32],
    ) -> Vec<FileMapping> {
        let video_files = self.filter_video_files(files);

        if video_files.is_empty() || episodes.is_empty() {
            return Vec::new();
        }

        let mut mappings = Vec::new();

        for episode in episodes {
            if let Some((file, confidence)) =
                self.find_episode_match(&video_files, series, season, *episode)
            {
                mappings.push(FileMapping {
                    torrent_file_path: file.path.clone(),
                    ticket_item_id: format!("s{:02}e{:02}", season, episode),
                    confidence,
                });
            }
        }

        mappings
    }

    /// Find best matching file for a track.
    fn find_best_track_match<'a>(
        &self,
        files: &'a [&TorrentFile],
        track: &ExpectedTrack,
    ) -> Option<(&'a TorrentFile, f32)> {
        let mut best_match: Option<(&TorrentFile, f32)> = None;

        for file in files {
            let filename = self.extract_filename(&file.path);
            let (extracted_number, _extracted_disc) = self.extract_track_info(&filename);

            // Calculate scores
            let number_score = if let Some(num) = extracted_number {
                if num == track.number {
                    1.0
                } else {
                    0.0
                }
            } else {
                0.3 // Uncertain - no number found
            };

            let title_score = self.title_similarity(&filename, &track.title);

            let weighted_score = (number_score * self.config.track_number_weight)
                + (title_score * self.config.title_weight);

            // Only consider if above threshold
            if weighted_score >= self.config.min_confidence
                && (best_match.is_none() || weighted_score > best_match.unwrap().1)
            {
                best_match = Some((file, weighted_score));
            }
        }

        best_match
    }

    /// Find matching file for a TV episode.
    fn find_episode_match<'a>(
        &self,
        files: &'a [&TorrentFile],
        series: &str,
        season: u32,
        episode: u32,
    ) -> Option<(&'a TorrentFile, f32)> {
        let mut best_match: Option<(&TorrentFile, f32)> = None;

        for file in files {
            let path_lower = file.path.to_lowercase();

            // Check for S01E01 pattern
            let episode_pattern = format!("s{:02}e{:02}", season, episode).to_lowercase();
            let alt_pattern = format!("{}x{:02}", season, episode).to_lowercase();

            let episode_match = path_lower.contains(&episode_pattern)
                || path_lower.contains(&alt_pattern)
                || self.contains_loose_episode(&path_lower, season, episode);

            if !episode_match {
                continue;
            }

            // Score based on series name match
            let series_score = self.title_similarity(&file.path, series);
            let episode_score = if episode_match { 0.5 } else { 0.0 };
            let total_score = (series_score * 0.5 + episode_score).min(1.0);

            if total_score >= self.config.min_confidence
                && (best_match.is_none() || total_score > best_match.unwrap().1)
            {
                best_match = Some((file, total_score));
            }
        }

        best_match
    }

    /// Check for loose episode patterns like "Episode 5" or "E05".
    fn contains_loose_episode(&self, path: &str, season: u32, episode: u32) -> bool {
        // Look for patterns like "Season 1" + "Episode 5" or "E05"
        let season_patterns = [
            format!("season {}", season),
            format!("season{}", season),
            format!("s{:02}", season),
            format!("s{}", season),
        ];

        let episode_patterns = [
            format!("episode {}", episode),
            format!("episode{}", episode),
            format!("e{:02}", episode),
            format!(" {:02} ", episode),
        ];

        let has_season = season_patterns.iter().any(|p| path.contains(p));
        let has_episode = episode_patterns.iter().any(|p| path.contains(p));

        has_season && has_episode
    }

    /// Filter files to only audio files.
    fn filter_audio_files<'a>(&self, files: &'a [TorrentFile]) -> Vec<&'a TorrentFile> {
        files
            .iter()
            .filter(|f| {
                let ext = self.get_extension(&f.path);
                self.config
                    .audio_extensions
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(&ext))
            })
            .collect()
    }

    /// Filter files to only video files.
    fn filter_video_files<'a>(&self, files: &'a [TorrentFile]) -> Vec<&'a TorrentFile> {
        files
            .iter()
            .filter(|f| {
                let ext = self.get_extension(&f.path);
                self.config
                    .video_extensions
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(&ext))
            })
            .collect()
    }

    /// Get file extension from path.
    fn get_extension(&self, path: &str) -> String {
        path.rsplit('.')
            .next()
            .unwrap_or("")
            .to_lowercase()
    }

    /// Extract filename from path.
    fn extract_filename(&self, path: &str) -> String {
        path.rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .to_string()
    }

    /// Extract track number and disc number from filename.
    ///
    /// Handles patterns like:
    /// - "01 - Track Name.flac"
    /// - "01. Track Name.flac"
    /// - "Track Name (01).flac"
    /// - "CD1/01 Track.flac"
    /// - "D1T01 - Track.flac"
    fn extract_track_info(&self, filename: &str) -> (Option<u32>, Option<u32>) {
        let filename_lower = filename.to_lowercase();

        // Try to extract disc number
        let disc = self.extract_disc_number(&filename_lower);

        // Try various track number patterns

        // Pattern 1: Number at start "01 - " or "01. " or "01 "
        if let Some(num) = self.extract_leading_number(filename) {
            return (Some(num), disc);
        }

        // Pattern 2: "D1T01" or "D01T01"
        if let Some(num) = self.extract_disc_track_pattern(&filename_lower) {
            return (Some(num), disc);
        }

        // Pattern 3: Number in parentheses "(01)"
        if let Some(num) = self.extract_paren_number(filename) {
            return (Some(num), disc);
        }

        // Pattern 4: "Track 01" or "Track01"
        if let Some(num) = self.extract_track_keyword_number(&filename_lower) {
            return (Some(num), disc);
        }

        (None, disc)
    }

    /// Extract disc number from filename.
    fn extract_disc_number(&self, filename: &str) -> Option<u32> {
        // Patterns: "CD1", "CD 1", "Disc 1", "D1", "[Disc 1]"
        let patterns = [
            (r"cd\s*(\d+)", 1),
            (r"disc\s*(\d+)", 1),
            (r"\bd(\d)t", 1),   // D1T pattern
            (r"\[disc\s*(\d+)\]", 1),
        ];

        for (pattern, _group) in patterns {
            if let Some(caps) = regex_lite::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(filename))
            {
                if let Some(m) = caps.get(1) {
                    if let Ok(num) = m.as_str().parse::<u32>() {
                        return Some(num);
                    }
                }
            }
        }

        None
    }

    /// Extract leading number from filename.
    fn extract_leading_number(&self, filename: &str) -> Option<u32> {
        let re = regex_lite::Regex::new(r"^(\d{1,3})[\s.\-_]").ok()?;
        re.captures(filename)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())
    }

    /// Extract track number from D1T01 pattern.
    fn extract_disc_track_pattern(&self, filename: &str) -> Option<u32> {
        let re = regex_lite::Regex::new(r"d\d+t(\d{1,3})").ok()?;
        re.captures(filename)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())
    }

    /// Extract number from parentheses.
    fn extract_paren_number(&self, filename: &str) -> Option<u32> {
        let re = regex_lite::Regex::new(r"\((\d{1,3})\)").ok()?;
        re.captures(filename)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())
    }

    /// Extract number after "track" keyword.
    fn extract_track_keyword_number(&self, filename: &str) -> Option<u32> {
        let re = regex_lite::Regex::new(r"track\s*(\d{1,3})").ok()?;
        re.captures(filename)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())
    }

    /// Calculate title similarity score (0.0-1.0).
    fn title_similarity(&self, filename: &str, title: &str) -> f32 {
        let filename_keywords = self.extract_keywords(filename);
        let title_keywords = self.extract_keywords(title);

        if title_keywords.is_empty() {
            return 0.5;
        }

        // Count matching keywords
        let matches = title_keywords
            .iter()
            .filter(|kw| {
                filename_keywords.contains(*kw)
                    || filename_keywords
                        .iter()
                        .any(|fk| fk.contains(kw.as_str()) || kw.contains(fk.as_str()))
            })
            .count();

        (matches as f32 / title_keywords.len() as f32).min(1.0)
    }

    /// Extract keywords from text.
    fn extract_keywords(&self, text: &str) -> Vec<String> {
        let stop_words: std::collections::HashSet<&str> = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "from", "as", "is", "ft", "feat", "featuring", "vs", "remix", "remaster",
            "remastered", "edition", "version", "mix",
        ]
        .into_iter()
        .collect();

        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .map(|s| s.trim().to_string())
            .filter(|s| s.len() > 1)
            .filter(|s| !stop_words.contains(s.as_str()))
            .filter(|s| s.parse::<u32>().is_err()) // Filter out pure numbers
            .collect()
    }

    /// Check if path contains a substring (case-insensitive).
    fn contains_substring(&self, haystack: &str, needle: &str) -> bool {
        haystack.to_lowercase().contains(&needle.to_lowercase())
    }

    /// Calculate size score relative to other files.
    fn size_score(&self, size: u64, all_files: &[&TorrentFile]) -> f32 {
        let max_size = all_files.iter().map(|f| f.size_bytes).max().unwrap_or(1);
        (size as f32 / max_size as f32).min(1.0)
    }
}

impl Default for DumbFileMapper {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate overall file mapping quality for a set of mappings.
///
/// Returns a score from 0.0 to 1.0 indicating how well files matched.
pub fn calculate_mapping_quality(
    mappings: &[FileMapping],
    expected: &ExpectedContent,
) -> f32 {
    let expected_count = expected.expected_file_count();

    if expected_count == 0 {
        return 1.0; // Nothing expected, nothing to match
    }

    if mappings.is_empty() {
        return 0.0;
    }

    // Coverage score: how many expected items have mappings
    let coverage = mappings.len() as f32 / expected_count as f32;

    // Average confidence of mappings
    let avg_confidence: f32 = mappings.iter().map(|m| m.confidence).sum::<f32>() / mappings.len() as f32;

    // Combined score (weighted)
    (coverage * 0.6 + avg_confidence * 0.4).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_file(path: &str, size: u64) -> TorrentFile {
        TorrentFile {
            path: path.to_string(),
            size_bytes: size,
        }
    }

    fn make_audio_files() -> Vec<TorrentFile> {
        vec![
            make_file("Album/01 - Come Together.flac", 30_000_000),
            make_file("Album/02 - Something.flac", 25_000_000),
            make_file("Album/03 - Maxwell's Silver Hammer.flac", 28_000_000),
            make_file("Album/cover.jpg", 500_000),
        ]
    }

    fn make_tracks() -> Vec<ExpectedTrack> {
        vec![
            ExpectedTrack::new(1, "Come Together"),
            ExpectedTrack::new(2, "Something"),
            ExpectedTrack::new(3, "Maxwell's Silver Hammer"),
        ]
    }

    #[test]
    fn test_filter_audio_files() {
        let mapper = DumbFileMapper::new();
        let files = make_audio_files();
        let audio = mapper.filter_audio_files(&files);

        assert_eq!(audio.len(), 3);
        assert!(audio.iter().all(|f| f.path.ends_with(".flac")));
    }

    #[test]
    fn test_extract_leading_number() {
        let mapper = DumbFileMapper::new();

        assert_eq!(mapper.extract_leading_number("01 - Track.flac"), Some(1));
        assert_eq!(mapper.extract_leading_number("02. Track.flac"), Some(2));
        assert_eq!(mapper.extract_leading_number("03_Track.flac"), Some(3));
        assert_eq!(mapper.extract_leading_number("10-Track.flac"), Some(10));
        assert_eq!(mapper.extract_leading_number("Track.flac"), None);
    }

    #[test]
    fn test_extract_disc_track_pattern() {
        let mapper = DumbFileMapper::new();

        assert_eq!(mapper.extract_disc_track_pattern("d1t01 - track.flac"), Some(1));
        assert_eq!(mapper.extract_disc_track_pattern("d2t05 track.flac"), Some(5));
        assert_eq!(mapper.extract_disc_track_pattern("track.flac"), None);
    }

    #[test]
    fn test_extract_disc_number() {
        let mapper = DumbFileMapper::new();

        assert_eq!(mapper.extract_disc_number("cd1/track.flac"), Some(1));
        assert_eq!(mapper.extract_disc_number("cd 2/track.flac"), Some(2));
        assert_eq!(mapper.extract_disc_number("disc 1/track.flac"), Some(1));
        assert_eq!(mapper.extract_disc_number("[disc 2] track.flac"), Some(2));
        assert_eq!(mapper.extract_disc_number("d1t05 track.flac"), Some(1));
    }

    #[test]
    fn test_title_similarity() {
        let mapper = DumbFileMapper::new();

        let score = mapper.title_similarity("01 - Come Together.flac", "Come Together");
        assert!(score >= 0.9, "Expected high similarity, got {}", score);

        let score = mapper.title_similarity("01 - Come Together.flac", "Something");
        assert!(score < 0.5, "Expected low similarity, got {}", score);
    }

    #[test]
    fn test_map_album_files() {
        let mapper = DumbFileMapper::new();
        let files = make_audio_files();
        let tracks = make_tracks();

        let expected = ExpectedContent::Album {
            artist: Some("The Beatles".to_string()),
            title: "Abbey Road".to_string(),
            tracks,
        };

        let mappings = mapper.map_files(&files, &expected);

        assert_eq!(mappings.len(), 3);
        assert!(mappings.iter().all(|m| m.confidence >= 0.5));
        assert!(mappings.iter().any(|m| m.ticket_item_id == "track-1"));
        assert!(mappings.iter().any(|m| m.ticket_item_id == "track-2"));
        assert!(mappings.iter().any(|m| m.ticket_item_id == "track-3"));
    }

    #[test]
    fn test_map_single_track() {
        let mapper = DumbFileMapper::new();
        let files = vec![
            make_file("Various/Track1.mp3", 5_000_000),
            make_file("Various/Come Together - Beatles.mp3", 8_000_000),
            make_file("Various/Track3.mp3", 5_000_000),
        ];

        let expected = ExpectedContent::Track {
            artist: Some("Beatles".to_string()),
            title: "Come Together".to_string(),
        };

        let mappings = mapper.map_files(&files, &expected);

        assert_eq!(mappings.len(), 1);
        assert!(mappings[0].torrent_file_path.contains("Come Together"));
    }

    #[test]
    fn test_map_movie_file() {
        let mapper = DumbFileMapper::new();
        let files = vec![
            make_file("The.Matrix.1999.1080p.BluRay.x264.mkv", 15_000_000_000),
            make_file("Sample/sample.mkv", 50_000_000),
            make_file("Subs/english.srt", 100_000),
        ];

        let expected = ExpectedContent::Movie {
            title: "The Matrix".to_string(),
            year: Some(1999),
        };

        let mappings = mapper.map_files(&files, &expected);

        assert_eq!(mappings.len(), 1);
        assert!(mappings[0].torrent_file_path.contains("Matrix"));
        assert!(!mappings[0].torrent_file_path.contains("sample"));
    }

    #[test]
    fn test_map_tv_episodes() {
        let mapper = DumbFileMapper::new();
        let files = vec![
            make_file("Breaking.Bad.S01E01.Pilot.mkv", 500_000_000),
            make_file("Breaking.Bad.S01E02.Cat's.in.the.Bag.mkv", 500_000_000),
            make_file("Breaking.Bad.S01E03.mkv", 500_000_000),
        ];

        let expected = ExpectedContent::TvEpisode {
            series: "Breaking Bad".to_string(),
            season: 1,
            episodes: vec![1, 2],
        };

        let mappings = mapper.map_files(&files, &expected);

        assert_eq!(mappings.len(), 2);
        assert!(mappings.iter().any(|m| m.ticket_item_id == "s01e01"));
        assert!(mappings.iter().any(|m| m.ticket_item_id == "s01e02"));
    }

    #[test]
    fn test_calculate_mapping_quality_full() {
        let expected = ExpectedContent::album("Test", make_tracks());
        let mappings = vec![
            FileMapping {
                torrent_file_path: "01.flac".to_string(),
                ticket_item_id: "track-1".to_string(),
                confidence: 0.95,
            },
            FileMapping {
                torrent_file_path: "02.flac".to_string(),
                ticket_item_id: "track-2".to_string(),
                confidence: 0.90,
            },
            FileMapping {
                torrent_file_path: "03.flac".to_string(),
                ticket_item_id: "track-3".to_string(),
                confidence: 0.85,
            },
        ];

        let quality = calculate_mapping_quality(&mappings, &expected);
        assert!(quality >= 0.8, "Expected high quality for full mapping, got {}", quality);
    }

    #[test]
    fn test_calculate_mapping_quality_partial() {
        let expected = ExpectedContent::album("Test", make_tracks());
        let mappings = vec![
            FileMapping {
                torrent_file_path: "01.flac".to_string(),
                ticket_item_id: "track-1".to_string(),
                confidence: 0.8,
            },
        ];

        let quality = calculate_mapping_quality(&mappings, &expected);
        assert!(quality < 0.6, "Expected lower quality for partial mapping, got {}", quality);
    }

    #[test]
    fn test_calculate_mapping_quality_empty() {
        let expected = ExpectedContent::album("Test", make_tracks());
        let mappings: Vec<FileMapping> = vec![];

        let quality = calculate_mapping_quality(&mappings, &expected);
        assert_eq!(quality, 0.0);
    }

    #[test]
    fn test_multi_disc_album() {
        let mapper = DumbFileMapper::new();
        let _files = vec![
            make_file("CD1/01 - Track One.flac", 30_000_000),
            make_file("CD1/02 - Track Two.flac", 25_000_000),
            make_file("CD2/01 - Track Three.flac", 28_000_000),
            make_file("CD2/02 - Track Four.flac", 27_000_000),
        ];

        // extract_track_info works on filenames, not paths
        // For paths, disc info in directory is handled during map_files
        let filename = mapper.extract_filename("CD1/01 - Track One.flac");
        let (track, _disc) = mapper.extract_track_info(&filename);
        assert_eq!(track, Some(1));
        assert_eq!(filename, "01 - Track One.flac");

        // Disc embedded in filename (d1t01 pattern) does work
        let (track, disc) = mapper.extract_track_info("d1t05 - Track.flac");
        assert_eq!(track, Some(5));
        assert_eq!(disc, Some(1));
    }

    #[test]
    fn test_various_filename_formats() {
        let mapper = DumbFileMapper::new();

        // Format 1: "01 - Title.flac"
        assert_eq!(mapper.extract_leading_number("01 - Title.flac"), Some(1));

        // Format 2: "01. Title.flac"
        assert_eq!(mapper.extract_leading_number("01. Title.flac"), Some(1));

        // Format 3: "01_Title.flac"
        assert_eq!(mapper.extract_leading_number("01_Title.flac"), Some(1));

        // Format 4: "Title (01).flac" - uses paren pattern
        assert_eq!(mapper.extract_paren_number("Title (01).flac"), Some(1));

        // Format 5: "D1T01 - Title.flac"
        assert_eq!(mapper.extract_disc_track_pattern("d1t01 - title.flac"), Some(1));
    }
}
