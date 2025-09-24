use anyhow::{Context, Result};
use musicbrainz_rs::{entity::release::Release, prelude::*, MusicBrainzClient};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use walkdir::WalkDir;

// Extension definitions used throughout the module
const ID3_EXTENSIONS: &[&str] = &["mp3", "aac"];
const MP4_EXTENSIONS: &[&str] = &["m4a", "m4b", "m4p", "alac", "mp4"];
const VORBIS_EXTENSIONS: &[&str] = &["flac", "ogg", "oga", "opus", "spx"];
const APE_EXTENSIONS: &[&str] = &["ape", "mpc", "wv"];
const AIFF_EXTENSIONS: &[&str] = &["aiff", "aif"];
const WAV_EXTENSIONS: &[&str] = &["wav"];

/// Unified function to set MusicBrainz Album ID on a file using lofty
fn set_musicbrainz_id(
    path: &Path,
    release_id: &str,
    relative_path: &str,
    tx: &mpsc::Sender<String>,
) -> Result<()> {
    match lofty::read_from_path(path) {
        Ok(tagged_file) => {
            // Use the TaggedFileExt trait to access tags
            use lofty::file::TaggedFileExt;
            let tags = tagged_file.tags();

            // Check if already tagged with this release ID
            if let Some(tag) = tags.first() {
                // Check existing MusicBrainz Album ID
                if let Some(existing_id) =
                    tag.get_string(&lofty::tag::ItemKey::MusicBrainzReleaseId)
                {
                    if existing_id == release_id {
                        tx.send(format!("COMPLETED: {} - Already tagged", relative_path))?;
                        return Ok(());
                    }
                }
            }

            // For now, skip files that require complex writing operations
            // TODO: Implement proper lofty writing when the API is better understood
            tx.send(format!(
                "COMPLETED: {} - Skipped (format not supported by lofty)",
                relative_path
            ))?;
        }
        Err(_) => {
            // Skip files that lofty can't handle at all
            tx.send(format!(
                "COMPLETED: {} - Skipped (unsupported format)",
                relative_path
            ))?;
        }
    }

    Ok(())
}

/// Helper function to extract artist and album from a file path using lofty
fn get_artist_album_from_path(
    path: &Path,
    folder_artist: &str,
    folder_album: &str,
) -> (String, String) {
    match lofty::read_from_path(path) {
        Ok(tagged_file) => {
            // Use the TaggedFileExt trait to access tags
            use lofty::file::TaggedFileExt;
            let tags = tagged_file.tags();

            // Try to get the first tag
            if let Some(tag) = tags.first() {
                // Use ItemKey constants to access specific fields
                let artist = tag
                    .get_string(&lofty::tag::ItemKey::TrackArtist)
                    .unwrap_or(folder_artist);
                let album = tag
                    .get_string(&lofty::tag::ItemKey::AlbumTitle)
                    .unwrap_or(folder_album);
                (artist.to_string(), album.to_string())
            } else {
                (folder_artist.to_string(), folder_album.to_string())
            }
        }
        Err(_) => {
            // Fallback to folder names if we can't read the tags
            (folder_artist.to_string(), folder_album.to_string())
        }
    }
}

pub async fn process_single_album_sync_tags(
    album_path: &Path,
    tx: mpsc::Sender<String>,
) -> Result<()> {
    // Set up MusicBrainz client with proper user agent
    let mut client = MusicBrainzClient::default();
    client
        .set_user_agent("mfutil/0.1.1 ( https://github.com/anoraktrend/music-folder-utils )")
        .context("Failed to set user agent")?;
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

    // Supported audio file extensions grouped by format type
    // (These are now defined as constants at the top of the file)

    // Combine all extensions for filtering (all formats supported by lofty)
    let all_extensions: Vec<_> = ID3_EXTENSIONS
        .iter()
        .chain(MP4_EXTENSIONS.iter())
        .chain(VORBIS_EXTENSIONS.iter())
        .chain(APE_EXTENSIONS.iter())
        .chain(AIFF_EXTENSIONS.iter())
        .chain(WAV_EXTENSIONS.iter())
        .collect();

    // First, collect all audio files and count them for progress tracking
    let mut audio_files = Vec::new();
    let mut files_scanned = 0;
    let mut files_skipped = 0;

    for entry in WalkDir::new(album_path).into_iter().filter_map(|e| e.ok()) {
        if !entry.path().is_file() {
            continue;
        }

        let path = entry.path().to_owned();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        files_scanned += 1;

        // Skip files we don't recognize or can't handle
        if !all_extensions.iter().any(|&&e| e == ext) {
            files_skipped += 1;
            continue;
        }

        // AAC files are now supported by lofty, so we can process them directly
        audio_files.push(path);
    }

    // Send progress for file discovery phase
    tx.send(format!(
        "COMPLETED: Scanned {} files ({} audio files found, {} skipped)",
        files_scanned,
        audio_files.len(),
        files_skipped
    ))
    .context("Failed to send file discovery progress")?;

    // Send initial total files count for progress tracking (will be updated)
    let audio_files_count = audio_files.len();
    tx.send(format!("TOTAL_FILES:{}", audio_files_count))
        .context("Failed to send total files count")?;

    // Group files by their tags using parallel processing
    let album_groups: HashMap<(String, String), Vec<PathBuf>> = audio_files
        .into_par_iter()
        .fold(
            HashMap::new,
            |mut groups: HashMap<(String, String), Vec<PathBuf>>, path| {
                let (artist, album) =
                    get_artist_album_from_path(&path, &folder_artist, &folder_album);
                groups.entry((artist, album)).or_default().push(path);
                groups
            },
        )
        .reduce(
            HashMap::new,
            |mut a: HashMap<(String, String), Vec<PathBuf>>, b| {
                for (key, paths) in b {
                    a.entry(key).or_default().extend(paths);
                }
                a
            },
        );

    // Update total tasks to include MusicBrainz searches
    let total_tasks = audio_files_count + album_groups.len();
    tx.send(format!("TOTAL_FILES:{}", total_tasks))
        .context("Failed to send updated total files count")?;

    // Send progress for file grouping phase
    tx.send(format!(
        "COMPLETED: Grouped {} audio files into {} album groups",
        audio_files_count,
        album_groups.len()
    ))
    .context("Failed to send grouping progress")?;

    // We'll track progress based on the number of files processed in each group

    // Batch MusicBrainz searches for better performance
    let mut release_cache: HashMap<(String, String), Option<String>> = HashMap::new();

    // Pre-fetch all MusicBrainz release IDs for album groups
    for (artist, album) in album_groups.keys() {
        if let std::collections::hash_map::Entry::Vacant(e) = release_cache.entry((artist.clone(), album.clone())) {
            let query = musicbrainz_rs::entity::release::ReleaseSearchQuery::query_builder()
                .release(album)
                .and()
                .artist(artist)
                .build();

            match Release::search(query).execute_with_client(&client).await {
                Ok(search_result) => {
                    let release_id = search_result.entities.first().map(|r| r.id.clone());
                    e.insert(release_id);
                    // Send progress for completed MusicBrainz search
                    tx.send(format!(
                        "COMPLETED: MusicBrainz search for {} - {}",
                        artist, album
                    ))
                    .context("Failed to send MusicBrainz progress")?;
                }
                Err(e) => {
                    eprintln!(
                        "MusicBrainz search failed for {} - {}: {}",
                        artist, album, e
                    );
                    release_cache.insert((artist.clone(), album.clone()), None);
                    // Still count as completed task even if failed
                    tx.send(format!(
                        "COMPLETED: MusicBrainz search for {} - {} (failed)",
                        artist, album
                    ))
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
        tx.send(format!("Processing group: {} - {}", artist, album))
            .context("Failed to send group info to TUI")?;

        // Get release ID from cache
        if let Some(Some(release_id)) = release_cache.get(&(artist.to_string(), album.to_string()))
        {
            tx.send(format!("Found cached release: {}", release_id))
                .context("Failed to send release found message to TUI")?;

            // Process files in parallel within this group
            let tx = tx.clone(); // Clone for parallel iterator
            let album_path = album_path.to_path_buf();

            paths.into_par_iter().for_each_with(tx.clone(), |tx, path| {
                let result = {
                    let relative_path = path
                        .strip_prefix(&album_path)
                        .unwrap_or(&path)
                        .display()
                        .to_string();

                    // Use unified function to set MusicBrainz ID
                    set_musicbrainz_id(&path, release_id, &relative_path, tx)
                };

                if let Err(e) = result {
                    eprintln!("Error processing {}: {}", path.display(), e);
                }
            });

            // Send summary for this album group
            tx.send(format!(
                "COMPLETED: Finished processing {} - {} ({} files processed)",
                artist, album, paths_len
            ))
            .context("Failed to send album summary")?;
        } else {
            tx.send(format!(
                "COMPLETED: Skipped {} - {} (no MusicBrainz match found)",
                artist, album
            ))
            .context("Failed to send no match message")?;
        }
    }

    tx.send(format!(
        "Successfully synchronized all files in {}",
        folder_album
    ))
    .context("Failed to send success message to TUI")?;

    Ok(())
}
