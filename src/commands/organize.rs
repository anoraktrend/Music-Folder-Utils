use anyhow::Result;
use rustc_hash::FxHashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;
use mfutil::utils;
use mfutil::metadata;

/// Organize music files into proper artist/album structure
pub fn organize_music_library(music_dir: &str, dry_run: bool, quiet: bool) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir).to_string();
    let music_path = Path::new(&music_dir);
    let artists_path = music_path.join("Artists");

    if !music_path.exists() {
        if dry_run {
            if !quiet {
                println!("Would create music directory: {}", music_path.display());
            }
        } else {
            fs::create_dir_all(music_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create music directory '{}': {}",
                    music_path.display(),
                    e
                )
            })?;
        }
    }

    if !artists_path.exists() {
        if dry_run {
            if !quiet {
                println!("Would create Artists directory: {}", artists_path.display());
            }
        } else {
            fs::create_dir(&artists_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create Artists directory '{}': {}",
                    artists_path.display(),
                    e
                )
            })?;
        }
    }

    if !quiet {
        info!("🔍 Scanning music directory: {}", music_path.display());
    }

    // Find all audio files in the music directory
    let mut files_to_move = Vec::new();
    let mut unknown_files = Vec::new();

    for entry in WalkDir::new(music_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            // Check if it's an audio file
            let audio_extensions = ["mp3", "flac", "m4a", "ogg", "aac", "wma", "wav", "aiff"];
            if audio_extensions.contains(&ext.as_str()) {
                files_to_move.push(path.to_path_buf());
            } else {
                unknown_files.push(path.to_path_buf());
            }
        }
    }

    if !quiet {
        info!("✅ Found {} audio files to organize", files_to_move.len());
    }
    if !quiet && !unknown_files.is_empty() {
        info!(
            "ℹ️  Found {} non-audio files (will be left in place)",
            unknown_files.len()
        );
    }

    // Group files by artist and album
    let mut file_groups: FxHashMap<(String, String), Vec<PathBuf>> = FxHashMap::default();
    let mut total_files = 0;

    for file_path in files_to_move {
        let (artist, album) = metadata::extract_artist_album_from_file(&file_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to extract metadata from '{}': {}",
                file_path.display(),
                e
            )
        })?;

        // Create a clean filename for the group key
        let clean_artist = utils::sanitize_filename(&artist);
        let clean_album = utils::sanitize_filename(&album);

        file_groups
            .entry((clean_artist.clone(), clean_album.clone()))
            .or_default()
            .push(file_path.clone());

        total_files += 1;

        if dry_run && !quiet {
            info!(
                "Would organize: {} -> {} / {}",
                file_path.display(),
                clean_artist,
                clean_album
            );
        }
    }

    if !quiet && dry_run {
        info!(
            "📊 Found {} unique artist/album combinations",
            file_groups.len()
        );
    }

    // Store counts before moving the collections
    let total_groups = file_groups.len();

    // Create directory structure and move files
    for ((artist, album), files) in file_groups {
        let artist_path = artists_path.join(&artist);
        let album_path = artist_path.join(&album);

        if dry_run {
            if !quiet {
                info!("📁 Would create directory: {}", album_path.display());
                for file in files {
                    info!(
                        "  📄 Would move: {} -> {}",
                        file.display(),
                        album_path.display()
                    );
                }
            }
        } else {
            // Create directories
            fs::create_dir_all(&album_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create album directory '{}': {}",
                    album_path.display(),
                    e
                )
            })?;

            // Move files
            for file_path in files {
                let file_name = file_path.file_name().ok_or_else(|| {
                    anyhow::anyhow!("File '{}' has no filename", file_path.display())
                })?;
                let dest_path = album_path.join(file_name);

                if file_path != dest_path {
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
                            "✅ Moved: {} -> {}",
                            file_path.display(),
                            dest_path.display()
                        );
                    }
                }
            }
        }
    }

    if dry_run && !quiet {
        info!("\n🎭 This was a dry run. No files were actually moved.");
        info!("💡 Run without --dry-run to perform the actual organization.");
    } else if !quiet {
        info!("\n🎉 Music library organization completed successfully!");
        info!(
            "   📁 Organized {} files into {} artist/album combinations",
            total_files, total_groups
        );
    }

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;


    #[test]
    fn test_organize_music_library_creates_directories() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");

        // Test that it creates the directory structure (without dry_run)
        let result = organize_music_library(music_root.to_str().unwrap(), false, true);

        assert!(result.is_ok());

        // Check that directories were created
        let artists_dir = music_root.join("Artists");
        assert!(artists_dir.exists());
        assert!(artists_dir.is_dir());

        Ok(())
    }

    #[test]
    fn test_organize_music_library_with_existing_structure() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");

        // Create existing structure
        fs::create_dir_all(&music_root)?;
        fs::create_dir(&artists_dir)?;

        // Test that it doesn't fail with existing structure
        let result = organize_music_library(music_root.to_str().unwrap(), false, true);

        assert!(result.is_ok());
        assert!(artists_dir.exists());

        Ok(())
    }

    #[test]
    fn test_sanitize_filename_basic() -> Result<()> {
        // Test basic sanitization
        assert_eq!(utils::sanitize_filename("normal_name"), "normal_name");
        assert_eq!(utils::sanitize_filename("file with spaces"), "file with spaces");
        assert_eq!(utils::sanitize_filename("file/with'\'bad:chars*"), "file_with_bad_chars_");

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