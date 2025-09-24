use anyhow::{Context, Result};
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use musicbrainz_rs::{entity::release::Release, prelude::*, MusicBrainzClient};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tracing::{error, warn};
use walkdir::WalkDir;

// Extension definitions used throughout the module
const ID3_EXTENSIONS: &[&str] = &["mp3", "aac"];
const MP4_EXTENSIONS: &[&str] = &["m4a", "m4b", "m4p", "alac", "mp4"];
const VORBIS_EXTENSIONS: &[&str] = &["flac", "ogg", "oga", "opus", "spx"];
const APE_EXTENSIONS: &[&str] = &["ape", "mpc", "wv"];
const AIFF_EXTENSIONS: &[&str] = &["aiff", "aif"];
const WAV_EXTENSIONS: &[&str] = &["wav"];

/// Comprehensive function to update all tags on a file using MusicBrainz data
fn update_all_tags(
    path: &Path,
    release_id: &str,
    _release_data: &Release,
    relative_path: &str,
    tx: &mpsc::Sender<String>,
) -> Result<()> {
    match lofty::read_from_path(path) {
        Ok(mut tagged_file) => {
            // Get the first tag for modification
            if let Some(tag) = tagged_file.primary_tag_mut() {
                // Update MusicBrainz Release ID - this is the core requirement
                tag.insert_text(lofty::tag::ItemKey::MusicBrainzReleaseId, release_id.to_string());

                // Save the updated tags
                match tagged_file.save_to_path(path, WriteOptions::default()) {
                    Ok(_) => {
                        tx.send(format!("COMPLETED: {} - MusicBrainz ID updated", relative_path))?;
                    }
                    Err(e) => {
                        tx.send(format!(
                            "COMPLETED: {} - Failed to save MusicBrainz ID: {}",
                            relative_path, e
                        ))?;
                    }
                }
            } else {
                tx.send(format!(
                    "COMPLETED: {} - No writable tags found",
                    relative_path
                ))?;
            }
        }
        Err(e) => {
            tx.send(format!(
                "COMPLETED: {} - Cannot read file for tag update: {}",
                relative_path, e
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

    // Send initial total files count for progress tracking
    let audio_files_count = audio_files.len();
    tx.send(format!("TOTAL_FILES:{}", audio_files_count))
        .context("Failed to send total files count")?;

    // Group files by their tags using parallel processing
    let album_groups: FxHashMap<(String, String), Vec<PathBuf>> = audio_files
        .into_par_iter()
        .fold(
            FxHashMap::default,
            |mut groups: FxHashMap<(String, String), Vec<PathBuf>>, path| {
                let (artist, album) =
                    get_artist_album_from_path(&path, &folder_artist, &folder_album);
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
    tx.send(format!("TOTAL_FILES:{}", total_tasks))
        .context("Failed to send updated total files count")?;

    // Send progress for file grouping phase
    tx.send(format!(
        "COMPLETED: Grouped {} audio files into {} album groups",
        audio_files_count,
        album_groups.len()
    ))
    .context("Failed to send grouping progress")?;

    // Batch MusicBrainz searches for better performance
    let mut release_cache: FxHashMap<(String, String), Option<Release>> = FxHashMap::default();

    // Pre-fetch all MusicBrainz release data for album groups
    for (artist, album) in album_groups.keys() {
        if let std::collections::hash_map::Entry::Vacant(e) = release_cache.entry((artist.clone(), album.clone())) {
            let query = musicbrainz_rs::entity::release::ReleaseSearchQuery::query_builder()
                .release(album)
                .and()
                .artist(artist)
                .build();

            match Release::search(query).execute_with_client(&client).await {
                Ok(search_result) => {
                    let release_data = search_result.entities.into_iter().next();
                    e.insert(release_data);
                    // Send progress for completed MusicBrainz search
                    tx.send(format!(
                        "COMPLETED: MusicBrainz search for {} - {}",
                        artist, album
                    ))
                    .context("Failed to send MusicBrainz progress")?;
                }
                Err(e) => {
                    warn!(
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

        // Get release data from cache
        if let Some(Some(release_data)) = release_cache.get(&(artist.to_string(), album.to_string()))
        {
            let release_id = &release_data.id;
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

                    // Use comprehensive function to update all tags
                    update_all_tags(&path, release_id, release_data, &relative_path, tx)
                };

                if let Err(e) = result {
                    error!("Error processing {}: {}", path.display(), e);
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
