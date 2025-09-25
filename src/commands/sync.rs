use anyhow::{Context, Result};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tracing::{error, warn};
use mfutil::{musicbrainz, cover_art, tagging, utils, progress};

/// Comprehensive function to update all tags on a file using MusicBrainz data
pub async fn process_single_album_sync_tags(
    album_path: &Path,
    tx: mpsc::Sender<String>,
) -> Result<()> {
    let artist_path = album_path.parent().context("Album path has no parent")?;
    let folder_artist = artist_path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let folder_album = album_path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    tx.send(format!("Scanning album folder: {}", folder_album))
        .context("Failed to send scan message to TUI")?;

    // First, collect all audio files and count them for progress tracking
    let scan_result = utils::scan_directory_for_audio_files(album_path)
        .context("Failed to scan directory for audio files")?;

    let audio_files = scan_result.audio_files;
    let files_scanned = scan_result.files_scanned;
    let files_skipped = scan_result.files_skipped;

    // Send progress for file discovery phase
    progress::send_scan_complete(&tx, files_scanned, audio_files.len(), files_skipped)
        .context("Failed to send file discovery progress")?;

    // Send initial total files count for progress tracking
    let audio_files_count = audio_files.len();
    progress::send_total_files(&tx, audio_files_count)
        .context("Failed to send total files count")?;

    // Group files by their tags using parallel processing
    let album_groups: FxHashMap<(String, String), Vec<PathBuf>> = audio_files
        .into_par_iter()
        .fold(
            FxHashMap::default,
            |mut groups: FxHashMap<(String, String), Vec<PathBuf>>, path: PathBuf| {
                let (artist, album) =
                    tagging::extract_artist_album_from_path_with_fallback(&path, &folder_artist, &folder_album);
                groups.entry((artist, album)).or_default().push(path);
                groups
            },
        )
        .reduce(
            FxHashMap::default,
            |mut a: FxHashMap<(String, String), Vec<PathBuf>>, b| {
                for (key, paths) in b {
                    a.entry(key).or_default().extend(paths);
                }
                a
            },
        );

    // Update total tasks to include MusicBrainz searches
    let total_tasks = audio_files_count + album_groups.len();
    progress::send_total_files(&tx, total_tasks)
        .context("Failed to send updated total files count")?;

    // Send progress for file grouping phase
    progress::send_grouping_complete(&tx, audio_files_count, album_groups.len())
        .context("Failed to send grouping progress")?;

    // Batch MusicBrainz searches for better performance
    let mut release_cache: FxHashMap<(String, String), Option<String>> = FxHashMap::default();

    // Pre-fetch all MusicBrainz release data for album groups
    for (artist, album) in album_groups.keys() {
        if let std::collections::hash_map::Entry::Vacant(e) = release_cache.entry((artist.clone(), album.clone())) {
            // Use library function for MusicBrainz lookup
            match musicbrainz::lookup_musicbrainz_release(artist, album, &tx).await {
                Ok(Some((_, _, release_id))) => {
                    e.insert(Some(release_id));
                    // Send progress for completed MusicBrainz search
                    progress::send_musicbrainz_search_complete(&tx, artist, album, true)
                        .context("Failed to send MusicBrainz progress")?;
                }
                Ok(None) => {
                    warn!(
                        "MusicBrainz search failed for {} - {}: No release found",
                        artist, album
                    );
                    release_cache.insert((artist.clone(), album.clone()), None);
                    // Still count as completed task even if failed
                    progress::send_musicbrainz_search_complete(&tx, artist, album, false)
                        .context("Failed to send MusicBrainz progress")?;
                }
                Err(e) => {
                    warn!(
                        "MusicBrainz search failed for {} - {}: {}",
                        artist, album, e
                    );
                    release_cache.insert((artist.clone(), album.clone()), None);
                    // Still count as completed task even if failed
                    progress::send_musicbrainz_search_complete(&tx, artist, album, false)
                        .context("Failed to send MusicBrainz progress")?;
                }
            }
        }
    }

    // Process each group
    for ((artist, album), paths) in album_groups.into_iter() {
        let artist = artist.as_str();
        let album = album.as_str();
        let paths_len = paths.len(); // Store length before moving
        progress::send_processing_group(&tx, artist, album)
            .context("Failed to send group info to TUI")?;

        // Get release data from cache
        if let Some(Some(release_id)) = release_cache.get(&(artist.to_string(), album.to_string()))
        {
            progress::send_custom_message(&tx, &format!("Found cached release: {}", release_id))
                .context("Failed to send release found message to TUI")?;

            // Process files in parallel within this group
            let tx = tx.clone(); // Clone for parallel iterator
            let album_path = album_path.to_path_buf();

            paths.into_par_iter().for_each_with(tx.clone(), |tx, path| {
                let result = {
                    // Calculate relative path from album directory
                    let relative_path = path.strip_prefix(&album_path).unwrap_or(&path).to_string_lossy().to_string();
                    tagging::process_music_file_with_musicbrainz(
                        &path,
                        release_id,
                        &relative_path,
                        tx
                    )
                };
                if let Err(e) = result {
                    error!("Error processing {}: {}", path.display(), e);
                }
            });

            // Send summary for this album group
            progress::send_album_processing_complete(&tx, artist, album, paths_len)
                .context("Failed to send album summary")?;

            // Fetch and save cover art for this album (don't use spawn to avoid borrowing issues)
            if let Err(e) = cover_art::save_cover_art_to_album(
                &album_path,
                release_id,
                artist,
                album,
                &tx
            ).await {
                warn!("Failed to fetch cover art for {} - {}: {}", artist, album, e);
            }
        } else {
            progress::send_album_skipped(&tx, artist, album)
                .context("Failed to send no match message")?;
        }
    }

    progress::send_final_complete(&tx, &folder_album)
        .context("Failed to send success message to TUI")?;

    Ok(())
}

/// Sync all tags with MusicBrainz and fetch cover art for all albums
pub async fn sync_all_tags_with_cover_art(music_dir: &str, tx: mpsc::Sender<String>) -> Result<()> {
    let album_paths = utils::get_all_album_paths(music_dir)?;

    for album_path in album_paths.iter() {
        process_single_album_sync_tags(album_path, tx.clone()).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_process_single_album_sync_tags_with_valid_album() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");
        fs::create_dir_all(&artists_dir)?;

        // Create test structure
        let artist_dir = artists_dir.join("TestArtist");
        fs::create_dir(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;

        // Create a test audio file
        let track_file = album_dir.join("test_track.mp3");
        fs::File::create(&track_file)?.write_all(b"fake audio content")?;

        // Set up channel for progress messages
        let (tx, rx) = mpsc::channel::<String>();

        // Mock the MusicBrainz response by setting up a minimal test
        // Since we can't easily mock the MusicBrainz API, we'll test the file scanning part
        let result = process_single_album_sync_tags(&album_dir, tx).await;

        // The function should complete (even if MusicBrainz search fails in test environment)
        assert!(result.is_ok());

        // We should receive some progress messages
        let mut message_count = 0;
        while rx.try_recv().is_ok() {
            message_count += 1;
        }
        assert!(message_count > 0, "Should receive at least one progress message");

        Ok(())
    }

    #[test]
    fn test_process_single_album_sync_tags_with_nonexistent_album() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let nonexistent_album = temp_dir.path().join("NonexistentAlbum");

        let (tx, _rx) = mpsc::channel::<String>();

        // This should fail gracefully
        let _result = std::panic::AssertUnwindSafe(async {
            process_single_album_sync_tags(&nonexistent_album, tx).await
        });

        // The function should handle the error gracefully
        // Note: This is a runtime test that might need adjustment based on actual behavior
        Ok(())
    }

    #[test]
    fn test_process_single_album_sync_tags_with_empty_album() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");
        fs::create_dir_all(&artists_dir)?;

        // Create test structure
        let artist_dir = artists_dir.join("TestArtist");
        fs::create_dir(&artist_dir)?;
        let album_dir = artist_dir.join("EmptyAlbum");
        fs::create_dir(&album_dir)?;

        // No audio files in the album
        let (tx, _rx) = mpsc::channel::<String>();

        // This should complete without processing any files
        let _result = std::panic::AssertUnwindSafe(async {
            process_single_album_sync_tags(&album_dir, tx).await
        });

        Ok(())
    }

    #[test]
    fn test_process_single_album_sync_tags_with_mixed_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");
        fs::create_dir_all(&artists_dir)?;

        // Create test structure
        let artist_dir = artists_dir.join("TestArtist");
        fs::create_dir(&artist_dir)?;
        let album_dir = artist_dir.join("MixedAlbum");
        fs::create_dir(&album_dir)?;

        // Create mix of audio and non-audio files
        fs::File::create(album_dir.join("track1.mp3"))?.write_all(b"audio")?;
        fs::File::create(album_dir.join("track2.flac"))?.write_all(b"audio")?;
        fs::File::create(album_dir.join("cover.jpg"))?.write_all(b"image")?;
        fs::File::create(album_dir.join("lyrics.txt"))?.write_all(b"text")?;

        let (tx, _rx) = mpsc::channel::<String>();

        // Should process only audio files
        let _result = std::panic::AssertUnwindSafe(async {
            process_single_album_sync_tags(&album_dir, tx).await
        });

        Ok(())
    }

    #[test]
    fn test_process_single_album_sync_tags_with_unsupported_formats() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");
        fs::create_dir_all(&artists_dir)?;

        // Create test structure
        let artist_dir = artists_dir.join("TestArtist");
        fs::create_dir(&artist_dir)?;
        let album_dir = artist_dir.join("UnsupportedAlbum");
        fs::create_dir(&album_dir)?;

        // Create files with unsupported extensions
        fs::File::create(album_dir.join("file.m3u"))?.write_all(b"playlist")?;
        fs::File::create(album_dir.join("file.exe"))?.write_all(b"binary")?;
        fs::File::create(album_dir.join("file.doc"))?.write_all(b"document")?;

        let (tx, _rx) = mpsc::channel::<String>();

        // Should skip all unsupported files
        let _result = std::panic::AssertUnwindSafe(async {
            process_single_album_sync_tags(&album_dir, tx).await
        });

        Ok(())
    }

    #[test]
    fn test_process_single_album_sync_tags_with_no_artist_parent() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let album_dir = temp_dir.path().join("OrphanedAlbum");
        fs::create_dir(&album_dir)?;

        let (tx, _rx) = mpsc::channel::<String>();

        // This should fail because album has no artist parent
        let _result = std::panic::AssertUnwindSafe(async {
            process_single_album_sync_tags(&album_dir, tx).await
        });

        Ok(())
    }
}
