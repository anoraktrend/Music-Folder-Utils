use anyhow::{Context, Result};
use lofty::{self, config::WriteOptions, file::{TaggedFileExt, AudioFile}, tag::ItemKey};
use rustc_hash::FxHashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use walkdir::WalkDir;
use musicbrainz_rs::{entity::release::Release, prelude::*, MusicBrainzClient};
use std::sync::mpsc;
use serde_json;
use reqwest;
use urlencoding;
use mfutil::{audio, utils, metadata, self};
/// Type alias for file grouping by artist, album, and release ID
type FileGroupsByMetadata = FxHashMap<(String, String, Option<String>), Vec<(PathBuf, Option<String>)>>;

/// Import files from an external directory into the music library
/// This function copies files from the specified import path and organizes them
pub fn import_and_organize_files(import_path: &str, music_dir: &str, dry_run: bool, quiet: bool) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir).to_string();
    let music_path = Path::new(&music_dir);
    let artists_path = music_path.join("Artists");
    let import_path = Path::new(import_path);

    // Validate import path exists
    if !import_path.exists() {
        return Err(anyhow::anyhow!("Import path '{}' does not exist", import_path.display()));
    }

    if !import_path.is_dir() {
        return Err(anyhow::anyhow!("Import path '{}' is not a directory", import_path.display()));
    }

    // Ensure Artists directory exists
    if !artists_path.exists() {
        if dry_run {
            if !quiet {
                info!("Would create Artists directory: {}", artists_path.display());
            }
        } else {
            fs::create_dir_all(&artists_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create Artists directory '{}': {}",
                    artists_path.display(),
                    e
                )
            })?;
        }
    }

    if !quiet {
        info!("🔍 Scanning import directory: {}", import_path.display());
    }

    let mut files_to_import = Vec::new();
    let mut files_excluded = 0;

    // Find all audio files in the import directory
    for entry in WalkDir::new(import_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only process audio files
        if path.is_file() && audio::is_audio_file(path) {
            // Check if file has proper metadata before including it
            match metadata::extract_artist_album_from_file(path) {
                    Ok((artist, album)) => {
                        // Only include files with meaningful metadata
                        if !artist.is_empty() &&
                           !album.is_empty() &&
                           artist != "Unknown Artist" &&
                           album != "Unknown Album" {
                            files_to_import.push((path.to_path_buf(), artist, album));
                        } else {
                            files_excluded += 1;
                            if !quiet {
                                info!(
                                    "⏭️  Excluding file without proper metadata: {} (Artist: '{}', Album: '{}')",
                                    path.display(), artist, album
                                );
                            }
                        }
                    }
                    Err(e) => {
                        files_excluded += 1;
                        if !quiet {
                            info!(
                                "⏭️  Excluding file with unreadable metadata: {} ({})",
                                path.display(), e
                            );
                        }
                    }
                }
            }
        }

    if files_to_import.is_empty() {
        if !quiet {
            if files_excluded > 0 {
                info!("✅ No files with proper metadata found. {} files excluded due to insufficient metadata.", files_excluded);
            } else {
                info!("✅ No audio files found in import directory. Nothing to import.");
            }
        }
        return Ok(());
    }

    if !quiet {
        info!("📁 Found {} files with proper metadata to import ({} excluded)", files_to_import.len(), files_excluded);
    }

    // Group files by their correct artist/album based on metadata
    let mut file_groups: FxHashMap<(String, String), Vec<PathBuf>> = FxHashMap::default();
    let import_count = files_to_import.len();

    for (file_path, artist, album) in files_to_import {
        // Create clean names for directory creation
        let clean_artist = utils::sanitize_filename(&artist);
        let clean_album = utils::sanitize_filename(&album);

        file_groups
            .entry((clean_artist.clone(), clean_album.clone()))
            .or_default()
            .push(file_path.clone());

        if dry_run && !quiet {
            info!(
                "Would import: {} -> {} / {}",
                file_path.display(),
                clean_artist,
                clean_album
            );
        }
    }

    if !quiet && dry_run {
        info!(
            "📊 Found {} unique artist/album combinations for {} files",
            file_groups.len(),
            import_count
        );
    }

    // Import files to their correct locations
    let total_groups = file_groups.len();

    for ((artist, album), files) in file_groups {
        let artist_path = artists_path.join(&artist);
        let album_path = artist_path.join(&album);

        if dry_run {
            if !quiet {
                info!("📁 Would create directory: {}", album_path.display());
                for file in &files {
                    info!(
                        "  📄 Would copy: {} -> {}",
                        file.display(),
                        album_path.display()
                    );
                }
            }
        } else {
            // Create directories if they don't exist
            fs::create_dir_all(&album_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create album directory '{}': {}",
                    album_path.display(),
                    e
                )
            })?;

            // Copy each file
            for file_path in files {
                let file_name = file_path.file_name().ok_or_else(|| {
                    anyhow::anyhow!("File '{}' has no filename", file_path.display())
                })?;
                let dest_path = album_path.join(file_name);

                // Only copy if the destination doesn't already exist
                if dest_path.exists() {
                    if !quiet {
                        info!(
                            "⚠️  File already exists at destination, skipping: {} -> {}",
                            file_path.display(),
                            dest_path.display()
                        );
                    }
                    continue;
                }

                // Copy the file
                fs::copy(&file_path, &dest_path).map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to copy '{}' to '{}': {}",
                        file_path.display(),
                        dest_path.display(),
                        e
                    )
                })?;

                if !quiet {
                    info!(
                        "✅ Imported: {} -> {}",
                        file_path.display(),
                        dest_path.display()
                    );
                }
            }
        }
    }

    if dry_run && !quiet {
        info!("\n🎭 This was a dry run. No files were actually imported.");
        info!("💡 Run without --dry-run to perform the actual import.");
    } else if !quiet {
        info!("\n🎉 File import completed successfully!");
        info!(
            "   📁 Imported {} files into {} artist/album combinations ({} files excluded)",
            import_count, total_groups, files_excluded
        );
    }

    Ok(())
}

/// Enhanced import with MusicBrainz integration and cover art fetching
pub async fn import_and_organize_files_with_musicbrainz(
    import_path: &str,
    music_dir: &str,
    dry_run: bool,
    quiet: bool,
    tx: mpsc::Sender<String>,
) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir).to_string();
    let music_path = Path::new(&music_dir);
    let artists_path = music_path.join("Artists");
    let import_path = Path::new(import_path);

    // Validate import path exists
    if !import_path.exists() {
        return Err(anyhow::anyhow!("Import path '{}' does not exist", import_path.display()));
    }

    if !import_path.is_dir() {
        return Err(anyhow::anyhow!("Import path '{}' is not a directory", import_path.display()));
    }

    // Ensure Artists directory exists
    if !artists_path.exists() {
        if dry_run {
            if !quiet {
                info!("Would create Artists directory: {}", artists_path.display());
            }
        } else {
            fs::create_dir_all(&artists_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create Artists directory '{}': {}",
                    artists_path.display(),
                    e
                )
            })?;
        }
    }

    tx.send("Scanning import directory for audio files...".to_string())
        .context("Failed to send scan message")?;

    let mut files_to_import = Vec::new();
    let mut files_excluded = 0;

    // Find all audio files in the import directory
    for entry in WalkDir::new(import_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only process audio files
        if path.is_file() && audio::is_audio_file(path) {
            // Enhanced metadata extraction with MusicBrainz lookup
            match extract_and_enhance_metadata(path, &tx).await {
                    Ok((artist, album, release_id)) => {
                        // Only include files with meaningful metadata
                        if !artist.is_empty() &&
                           !album.is_empty() &&
                           artist != "Unknown Artist" &&
                           album != "Unknown Album" {
                            files_to_import.push((path.to_path_buf(), artist, album, release_id));
                        } else {
                            files_excluded += 1;
                            tx.send(format!("Excluding file without proper metadata: {} (Artist: '{}', Album: '{}')",
                                           path.display(), artist, album))
                                .context("Failed to send exclusion message")?;
                        }
                    }
                    Err(e) => {
                        files_excluded += 1;
                        tx.send(format!("Excluding file with unreadable metadata: {} ({})",
                                       path.display(), e))
                            .context("Failed to send metadata error message")?;
                    }
                }
            }
        }


    if files_to_import.is_empty() {
        tx.send(format!("No files with proper metadata found. {} files excluded due to insufficient metadata.", files_excluded))
            .context("Failed to send completion message")?;
        return Ok(());
    }

    tx.send(format!("TOTAL_FILES:{}", files_to_import.len()))
        .context("Failed to send total files count")?;

    // Group files by their correct artist/album based on enhanced metadata
    let mut file_groups: FileGroupsByMetadata = FxHashMap::default();
    let import_count = files_to_import.len();

    for (file_path, artist, album, release_id) in files_to_import {
        // Create clean names for directory creation
        let clean_artist = utils::sanitize_filename(&artist);
        let clean_album = utils::sanitize_filename(&album);

        file_groups
            .entry((clean_artist.clone(), clean_album.clone(), release_id.clone()))
            .or_default()
            .push((file_path.clone(), release_id.clone()));

        if dry_run && !quiet {
            tx.send(format!("Would import: {} -> {} / {} (Release ID: {:?})",
                           file_path.display(), clean_artist, clean_album, release_id))
                .context("Failed to send dry run message")?;
        }
    }

    tx.send(format!("Found {} unique artist/album combinations for {} files", file_groups.len(), import_count))
        .context("Failed to send group count message")?;

    // Import files to their correct locations with cover art fetching
    let total_groups = file_groups.len();

    for ((artist, album, release_id), files) in file_groups {
        let artist_path = artists_path.join(&artist);
        let album_path = artist_path.join(&album);

        // Fetch cover art for this release if we have a release ID
        let mut cover_art_data: Option<Vec<u8>> = None;
        if let Some(ref id) = release_id {
            // Try MusicBrainz first
            if let Ok(Some(cover_art)) = fetch_musicbrainz_cover_art(id, &tx).await {
                cover_art_data = Some(cover_art);
            } else {
                // Fallback to AudioDB
                if let Ok(Some(cover_art)) = fetch_audiodb_cover_art(&artist, &album, &tx).await {
                    cover_art_data = Some(cover_art);
                }
            }
        }

        if dry_run {
            tx.send(format!("Would create directory: {}", album_path.display()))
                .context("Failed to send dry run directory message")?;
            for (file, _) in &files {
                tx.send(format!("  Would copy: {} -> {}", file.display(), album_path.display()))
                    .context("Failed to send dry run file message")?;
            }
        } else {
            // Create directories if they don't exist
            fs::create_dir_all(&album_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create album directory '{}': {}",
                    album_path.display(),
                    e
                )
            })?;

            // Copy each file
            for (file_path, _) in files {
                let file_name = file_path.file_name().ok_or_else(|| {
                    anyhow::anyhow!("File '{}' has no filename", file_path.display())
                })?;
                let dest_path = album_path.join(file_name);

                // Only copy if the destination doesn't already exist
                if dest_path.exists() {
                    tx.send(format!("File already exists at destination, skipping: {} -> {}",
                                   file_path.display(), dest_path.display()))
                        .context("Failed to send skip message")?;
                    continue;
                }

                // Copy the file
                fs::copy(&file_path, &dest_path).map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to copy '{}' to '{}': {}",
                        file_path.display(),
                        dest_path.display(),
                        e
                    )
                })?;

                // Set enhanced metadata with MusicBrainz release ID
                if let Some(ref release_id) = release_id {
                    set_enhanced_metadata(&dest_path, &artist, &album, release_id)
                        .with_context(|| format!("Failed to set metadata for: {:?}", dest_path))?;
                }

                tx.send(format!("COMPLETED: Imported {} -> {}", file_path.display(), dest_path.display()))
                    .context("Failed to send completion message")?;
            }

            // Save cover art if we fetched it
            if let Some(cover_art) = cover_art_data {
                let cover_art_path = album_path.join("cover.jpg");
                if let Err(e) = std::fs::write(&cover_art_path, &cover_art) {
                    warn!("Failed to save cover art to {:?}: {}", cover_art_path, e);
                } else {
                    tx.send(format!("Saved cover art to: {}", cover_art_path.display()))
                        .context("Failed to send cover art save message")?;
                }
            }
        }
    }

    tx.send(format!("Successfully imported {} files into {} artist/album combinations",
                   import_count, total_groups))
        .context("Failed to send final completion message")?;

    Ok(())
}



/// Enhanced metadata extraction with MusicBrainz lookup
async fn extract_and_enhance_metadata(file_path: &Path, tx: &mpsc::Sender<String>) -> Result<(String, String, Option<String>)> {
    // First try to extract from file metadata
    let (mut artist, mut album) = metadata::extract_artist_album_from_file(file_path)?;

    // If we have basic metadata, try to enhance it with MusicBrainz
    if artist != "Unknown Artist" && album != "Unknown Album" {
        match lookup_musicbrainz_release(&artist, &album, tx).await {
            Ok(Some((enhanced_artist, enhanced_album, release_id))) => {
                tx.send(format!("Enhanced metadata for {}: '{}' -> '{}' / '{}' -> '{}'",
                               file_path.display(), artist, enhanced_artist, album, enhanced_album))
                    .context("Failed to send enhancement message")?;
                artist = enhanced_artist;
                album = enhanced_album;
                return Ok((artist, album, Some(release_id)));
            }
            Ok(None) => {
                // No enhancement available, use original metadata
                tx.send(format!("No MusicBrainz match found for {} - {} (using original metadata)",
                               artist, album))
                    .context("Failed to send no match message")?;
            }
            Err(e) => {
                warn!("MusicBrainz lookup failed for {} - {}: {}", artist, album, e);
            }
        }
    }

    Ok((artist.to_string(), album.to_string(), None))
}

/// Look up release information from MusicBrainz
async fn lookup_musicbrainz_release(artist: &str, album: &str, tx: &mpsc::Sender<String>) -> Result<Option<(String, String, String)>> {
    tx.send(format!("Looking up MusicBrainz release: {} - {}", artist, album))
        .context("Failed to send MusicBrainz lookup message")?;

    let mut client = MusicBrainzClient::default();
    client.set_user_agent("mfutil/0.1.1 (https://github.com/anoraktrend/music-folder-utils)")
        .context("Failed to set user agent")?;

    // Search for releases by artist and album
    let query = musicbrainz_rs::entity::release::ReleaseSearchQuery::query_builder()
        .release(album)
        .and()
        .artist(artist)
        .build();

    match Release::search(query).execute_with_client(&client).await {
        Ok(search_result) => {
            if let Some(release) = search_result.entities.into_iter().next() {
                let artist_credit = release.artist_credit.as_ref() 
                    .map(|credits| credits.iter().map(|c| c.name.clone()).collect::<Vec<_>>().join(" & "))
                    .unwrap_or_else(|| artist.to_string());

                tx.send(format!("Found MusicBrainz release: {} - {} ({})",
                               artist_credit, release.title, release.id))
                    .context("Failed to send release found message")?;

                Ok(Some((artist_credit, release.title, release.id)))
            } else {
                tx.send(format!("No MusicBrainz release found for {} - {}", artist, album))
                    .context("Failed to send no release message")?;
                Ok(None)
            }
        }
        Err(e) => {
            warn!("MusicBrainz search failed: {}", e);
            Err(anyhow::anyhow!("MusicBrainz search failed: {}", e))
        }
    }
}

/// Fetch cover art from MusicBrainz Cover Art Archive
async fn fetch_musicbrainz_cover_art(release_id: &str, tx: &mpsc::Sender<String>) -> Result<Option<Vec<u8>>> {
    tx.send(format!("Fetching cover art from MusicBrainz for release: {}", release_id))
        .context("Failed to send cover art fetch message")?;

    let cover_art_url = format!("https://coverartarchive.org/release/{}/front", release_id);
    let client = reqwest::Client::new();

    match client.get(&cover_art_url)
        .header("User-Agent", "mfutil/0.1.1 (https://github.com/anoraktrend/music-folder-utils)")
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.bytes().await {
                    Ok(image_data) => {
                        tx.send("Successfully fetched cover art from MusicBrainz".to_string())
                            .context("Failed to send cover art success message")?;
                        Ok(Some(image_data.to_vec()))
                    }
                    Err(e) => {
                        tx.send(format!("Failed to read cover art data: {}", e))
                            .context("Failed to send cover art data error")?;
                        Ok(None)
                    }
                }
            } else {
                tx.send(format!("Cover art not available from MusicBrainz (status: {})", response.status()))
                    .context("Failed to send cover art unavailable message")?;
                Ok(None)
            }
        }
        Err(e) => {
            tx.send(format!("Failed to fetch cover art from MusicBrainz: {}", e))
                .context("Failed to send cover art fetch error")?;
            Ok(None)
        }
    }
}

/// Fetch cover art from AudioDB as fallback
async fn fetch_audiodb_cover_art(artist: &str, album: &str, tx: &mpsc::Sender<String>) -> Result<Option<Vec<u8>>> {
    tx.send(format!("Trying AudioDB for cover art: {} - {}", artist, album))
        .context("Failed to send AudioDB cover art message")?;

    let encoded_artist = urlencoding::encode(artist);
    let encoded_album = urlencoding::encode(album);
    let audiodb_url = format!(
        "https://www.theaudiodb.com/api/v1/json/2/searchalbum.php?s={}&a={}",
        encoded_artist, encoded_album
    );

    let client = reqwest::Client::new();

    match client.get(&audiodb_url)
        .header("User-Agent", "mfutil/0.1.1 (https://github.com/anoraktrend/music-folder-utils)")
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(json_data) => {
                        if let Some(albums) = json_data.get("album") {
                            if let Some(albums_array) = albums.as_array() {
                                if let Some(first_album) = albums_array.first() {
                                    if let Some(thumbnail_url) = first_album.get("strAlbumThumb") {
                                        if let Some(url_str) = thumbnail_url.as_str() {
                                            if !url_str.is_empty() && url_str != "null" {
                                                match client.get(url_str)
                                                    .header("User-Agent", "mfutil/0.1.1 (https://github.com/anoraktrend/music-folder-utils)")
                                                    .send()
                                                    .await
                                                {
                                                    Ok(image_response) => {
                                                        if image_response.status().is_success() {
                                                            match image_response.bytes().await {
                                                                Ok(image_data) => {
                                                                    tx.send("Successfully fetched cover art from AudioDB".to_string())
                                                                        .context("Failed to send AudioDB success message")?;
                                                                    Ok(Some(image_data.to_vec()))
                                                                }
                                                                Err(e) => {
                                                                    tx.send(format!("Failed to download AudioDB cover art: {}", e))
                                                                        .context("Failed to send AudioDB download error")?;
                                                                    Ok(None)
                                                                }
                                                            }
                                                        } else {
                                                            tx.send("AudioDB cover art download failed".to_string())
                                                                .context("Failed to send AudioDB download failed")?;
                                                            Ok(None)
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tx.send(format!("Failed to fetch from AudioDB URL: {}", e))
                                                            .context("Failed to send AudioDB URL error")?;
                                                        Ok(None)
                                                    }
                                                }
                                            } else {
                                                tx.send("No cover art URL found in AudioDB response".to_string())
                                                    .context("Failed to send no AudioDB URL message")?;
                                                Ok(None)
                                            }
                                        } else {
                                            tx.send("No cover art URL found in AudioDB response".to_string())
                                                .context("Failed to send no AudioDB URL message")?;
                                            Ok(None)
                                        }
                                    } else {
                                        tx.send("No cover art found in AudioDB response".to_string())
                                            .context("Failed to send no AudioDB cover art")?;
                                        Ok(None)
                                    }
                                } else {
                                    tx.send("No albums found in AudioDB response".to_string())
                                        .context("Failed to send no AudioDB albums")?;
                                    Ok(None)
                                }
                            } else {
                                tx.send("Invalid AudioDB response format".to_string())
                                    .context("Failed to send invalid AudioDB format")?;
                                Ok(None)
                            }
                        } else {
                            tx.send("No album data in AudioDB response".to_string())
                                .context("Failed to send no AudioDB album data")?;
                            Ok(None)
                        }
                    }
                    Err(e) => {
                        tx.send(format!("Failed to parse AudioDB response: {}", e))
                            .context("Failed to send AudioDB parse error")?;
                        Ok(None)
                    }
                }
            } else {
                tx.send(format!("AudioDB request failed (status: {})", response.status()))
                    .context("Failed to send AudioDB request failed")?;
                Ok(None)
            }
        }
        Err(e) => {
            tx.send(format!("Failed to fetch from AudioDB: {}", e))
                .context("Failed to send AudioDB fetch error")?;
            Ok(None)
        }
    }
}

/// Set enhanced metadata with MusicBrainz release ID
fn set_enhanced_metadata(file_path: &Path, artist: &str, album: &str, release_id: &str) -> Result<()> {
    match lofty::read_from_path(file_path) {
        Ok(mut tagged_file) => {
            if let Some(tag) = tagged_file.primary_tag_mut() {
                // Set standard metadata
                tag.insert_text(ItemKey::TrackArtist, artist.to_string());
                tag.insert_text(ItemKey::AlbumArtist, artist.to_string());
                tag.insert_text(ItemKey::AlbumTitle, album.to_string());

                // Add MusicBrainz release ID
                tag.insert_text(ItemKey::MusicBrainzReleaseId, release_id.to_string());

                // Try to save the enhanced metadata
                if let Err(e) = tagged_file.save_to_path(file_path, WriteOptions::default()) {
                    warn!("Failed to save enhanced metadata for {}: {}", file_path.display(), e);
                }
            }
        }
        Err(e) => {
            warn!("Could not read file for metadata enhancement: {} ({})", file_path.display(), e);
        }
    }

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_import_and_organize_files_nonexistent_import_path() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let nonexistent_import = temp_dir.path().join("NonexistentImport");

        // Test that it fails with nonexistent import path
        let result = import_and_organize_files(nonexistent_import.to_str().unwrap(), music_root.to_str().unwrap(), false, true);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));

        Ok(())
    }

    #[test]
    fn test_import_and_organize_files_import_path_not_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let import_file = temp_dir.path().join("import.txt");

        // Create a file instead of directory
        fs::File::create(&import_file)?.write_all(b"not a directory")?;

        // Test that it fails when import path is not a directory
        let result = import_and_organize_files(import_file.to_str().unwrap(), music_root.to_str().unwrap(), false, true);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("is not a directory"));

        Ok(())
    }

    #[test]
    fn test_import_and_organize_files_empty_import_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let empty_import = temp_dir.path().join("EmptyImport");

        // Create empty import directory
        fs::create_dir(&empty_import)?;

        // Test that it succeeds with empty import directory
        let result = import_and_organize_files(empty_import.to_str().unwrap(), music_root.to_str().unwrap(), false, true);

        assert!(result.is_ok());

        Ok(())
    }

    #[test]
    fn test_import_and_organize_files_dry_run() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let import_dir = temp_dir.path().join("Import");

        // Create import directory with audio file
        fs::create_dir(&import_dir)?;
        fs::File::create(import_dir.join("test.mp3"))?.write_all(b"audio")?;

        // Test dry run - should not actually import files
        let result = import_and_organize_files(import_dir.to_str().unwrap(), music_root.to_str().unwrap(), true, true);

        assert!(result.is_ok());

        // Check that no files were actually moved
        let artists_dir = music_root.join("Artists");
        assert!(!artists_dir.exists()); // Should not create directories in dry run

        Ok(())
    }

    #[test]
    fn test_sanitize_filename_basic() -> Result<()> {
        // Test basic sanitization
        assert_eq!(utils::sanitize_filename("normal_name"), "normal_name");
        assert_eq!(utils::sanitize_filename("file with spaces"), "file with spaces");
        assert_eq!(utils::sanitize_filename("file/with\\bad:chars*"), "file_with_bad_chars_");

        Ok(())
    }

    #[test]
    fn test_sanitize_filename_edge_cases() -> Result<()> {
        // Test edge cases
        assert_eq!(utils::sanitize_filename(""), "");
        assert_eq!(utils::sanitize_filename("   "), "");
        assert_eq!(utils::sanitize_filename("file\x00with\x01control\x02chars"), "file_with_control_chars");

        Ok(())
    }

    #[test]
    fn test_extract_from_path_valid_structure() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");
        let artist_dir = artists_dir.join("TestArtist");
        let album_dir = artist_dir.join("TestAlbum");
        let file_path = album_dir.join("track.mp3");

        // Create the directory structure
        fs::create_dir_all(&album_dir)?;

        // Test extraction from path
        let (artist, album) = metadata::extract_from_path(&file_path)?;

        assert_eq!(artist, "TestArtist");
        assert_eq!(album, "TestAlbum");

        Ok(())
    }
}
