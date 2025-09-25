use anyhow::Result;
use rustc_hash::FxHashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;
use mfutil::utils;
use mfutil::metadata;

/// Reorganize files that are not in their correct artist/album structure
/// This function finds files that are misplaced and moves them to their proper locations
pub fn reorganize_misplaced_files(music_dir: &str, dry_run: bool, quiet: bool) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir).to_string();
    let music_path = Path::new(&music_dir);
    let artists_path = music_path.join("Artists");

    if !artists_path.exists() || !artists_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Artists directory '{}' does not exist. Run organize first to create the directory structure.",
            artists_path.display()
        ));
    }

    if !quiet {
        info!("🔍 Scanning for misplaced files to reorganize...");
    }

    let mut files_to_move = Vec::new();
    let mut total_processed = 0;

    // Walk through the music directory and find audio files
    for entry in WalkDir::new(music_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip the Artists directory and its contents - these are already organized
        if path.starts_with(&artists_path) {
            continue;
        }

        // Only process audio files
        if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            let audio_extensions = ["mp3", "flac", "m4a", "ogg", "aac", "wma", "wav", "aiff"];
            if audio_extensions.contains(&ext.as_str()) {
                files_to_move.push(path.to_path_buf());
            }
        }
    }

    if files_to_move.is_empty() {
        if !quiet {
            info!("✅ No misplaced files found. All files are already properly organized.");
        }
        return Ok(())
    }

    if !quiet {
        info!("📁 Found {} misplaced files to reorganize", files_to_move.len());
    }

    // Group files by their correct artist/album based on metadata
    let mut file_groups: FxHashMap<(String, String), Vec<PathBuf>> = FxHashMap::default();

    for file_path in files_to_move {
        total_processed += 1;

        // Extract artist and album information from the file
        let (artist, album) = metadata::extract_artist_album_from_file(&file_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to extract metadata from '{}': {}",
                file_path.display(),
                e
            )
        })?;

        // Create clean names for directory creation
        let clean_artist = utils::sanitize_filename(&artist);
        let clean_album = utils::sanitize_filename(&album);

        file_groups
            .entry((clean_artist.clone(), clean_album.clone()))
            .or_default()
            .push(file_path.clone());

        if dry_run && !quiet {
            info!(
                "Would reorganize: {} -> {} / {}",
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
            total_processed
        );
    }

    // Move files to their correct locations
    let total_groups = file_groups.len();

    for ((artist, album), files) in file_groups {
        let artist_path = artists_path.join(&artist);
        let album_path = artist_path.join(&album);

        if dry_run {
            if !quiet {
                info!("📁 Would create directory: {}", album_path.display());
                for file in &files {
                    info!(
                        "  📄 Would move: {} -> {}",
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

            // Move each file
            for file_path in files {
                let file_name = file_path.file_name().ok_or_else(|| {
                    anyhow::anyhow!("File '{}' has no filename", file_path.display())
                })?;
                let dest_path = album_path.join(file_name);

                // Only move if the destination doesn't already exist
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

                // Move the file
                fs::rename(&file_path, &dest_path).map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to move '{}' to '{}': {}",
                        file_path.display(),
                        dest_path.display(),
                        e
                    )
                })?;

                if !quiet {
                    info!(
                        "✅ Reorganized: {} -> {}",
                        file_path.display(),
                        dest_path.display()
                    );
                }
            }
        }
    }

    if dry_run && !quiet {
        info!("\n🎭 This was a dry run. No files were actually moved.");
        info!("💡 Run without --dry-run to perform the actual reorganization.");
    } else if !quiet {
        info!("\n🎉 File reorganization completed successfully!");
        info!(
            "   📁 Reorganized {} files into {} artist/album combinations",
            total_processed,
            total_groups
        );
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
    fn test_reorganize_misplaced_files_no_artists_dir() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");

        // Test that it fails when Artists directory doesn't exist
        let result = reorganize_misplaced_files(music_root.to_str().unwrap(), false, true);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Artists directory"));

        Ok(())
    }

    #[test]
    fn test_reorganize_misplaced_files_with_no_misplaced_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");

        // Create proper structure with no misplaced files
        fs::create_dir_all(&artists_dir)?;
        let artist_dir = artists_dir.join("TestArtist");
        fs::create_dir(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        fs::File::create(album_dir.join("track.mp3"))?.write_all(b"audio")?;

        // Test that it succeeds with no misplaced files
        let result = reorganize_misplaced_files(music_root.to_str().unwrap(), false, true);

        assert!(result.is_ok());

        Ok(())
    }

    #[test]
    fn test_reorganize_misplaced_files_dry_run() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");

        // Create proper structure
        fs::create_dir_all(&artists_dir)?;

        // Create a misplaced file
        let misplaced_file = music_root.join("misplaced.mp3");
        fs::File::create(&misplaced_file)?.write_all(b"audio")?;

        // Test dry run - should not actually move files
        let result = reorganize_misplaced_files(music_root.to_str().unwrap(), true, true);

        assert!(result.is_ok());
        assert!(misplaced_file.exists()); // File should still be in original location

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
        let artist_dir = temp_dir.path().join("Test Artist");
        let album_dir = artist_dir.join("Test Album");
        let file_path = album_dir.join("track.mp3");

        // Create the directory structure
        fs::create_dir_all(&album_dir)?;

        let (artist, album) = metadata::extract_from_path(&file_path)?;

        // The function filters out common words like "artist" and "album"
        assert_eq!(artist, "Test"); // "Test Artist" -> "Test" (removes "artist")
        assert_eq!(album, "Test"); // "Test Album" -> "Test" (removes "album")

        Ok(())
    }

    #[test]
    fn test_extract_from_path_with_problematic_names() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let artist_dir = temp_dir.path().join("Metallica");
        let album_dir = artist_dir.join("Master of Puppets");
        let file_path = album_dir.join("track.mp3");

        // Create the directory structure
        fs::create_dir_all(&album_dir)?;

        let (artist, album) = metadata::extract_from_path(&file_path)?;

        // These names should not be filtered out
        assert_eq!(artist, "Metallica");
        assert_eq!(album, "Master of Puppets");

        Ok(())
    }

    #[test]
    fn test_extract_from_path_missing_parent() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("track.mp3");

        // File with no parent directory - this should actually work
        let result = metadata::extract_from_path(&file_path);

        // The function should handle this case gracefully
        assert!(result.is_ok());

        Ok(())
    }
}