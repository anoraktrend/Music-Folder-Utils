use anyhow::Result;
use lofty::{self, file::TaggedFileExt, tag::ItemKey};
use rustc_hash::FxHashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;

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
        let (artist, album) = extract_artist_album_from_file(&file_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to extract metadata from '{}': {}",
                file_path.display(),
                e
            )
        })?;

        // Create a clean filename for the group key
        let clean_artist = sanitize_filename(&artist);
        let clean_album = sanitize_filename(&album);

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

/// Extract artist and album information from a music file
fn extract_artist_album_from_file(file_path: &Path) -> Result<(String, String)> {
    match lofty::read_from_path(file_path) {
        Ok(tagged_file) => {
            let tags = tagged_file.tags();
            if let Some(tag) = tags.first() {
                // Try multiple artist fields in order of preference
                let artist = tag
                    .get_string(&ItemKey::AlbumArtist)
                    .or_else(|| tag.get_string(&ItemKey::TrackArtist))
                    .unwrap_or_else(|| {
                        // Try to extract from filename if no artist metadata
                        file_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown Artist")
                            .split(" - ")
                            .next()
                            .unwrap_or("Unknown Artist")
                    })
                    .to_string();

                // Try multiple album fields in order of preference
                let album = tag
                    .get_string(&ItemKey::AlbumTitle)
                    .unwrap_or_else(|| {
                        // Try to extract from parent directory name
                        file_path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown Album")
                    })
                    .to_string();

                Ok((artist, album))
            } else {
                // Fallback to path-based extraction
                extract_from_path(file_path)
            }
        }
        Err(_) => {
            // Fallback to path-based extraction
            extract_from_path(file_path)
        }
    }
}

/// Extract artist and album from file path when tags are not available
fn extract_from_path(file_path: &Path) -> Result<(String, String)> {
    let parent = file_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("File '{}' has no parent directory", file_path.display()))?;

    // Try to extract album from parent directory name
    let album = parent
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| {
            // Clean up common album directory naming patterns
            let cleaned = name
                .replace(['_', '-'], " ")
                .split_whitespace()
                .filter(|word| {
                    // Filter out common non-album words
                    let lower = word.to_lowercase();
                    !matches!(
                        lower.as_str(),
                        "album" | "music" | "songs" | "tracks" | "collection"
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");

            if cleaned.trim().is_empty() {
                "Unknown Album".to_string()
            } else {
                cleaned.trim().to_string()
            }
        })
        .unwrap_or_else(|| "Unknown Album".to_string());

    let grandparent = parent
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Album directory '{}' has no parent", parent.display()))?;

    // Try to extract artist from grandparent directory name
    let artist = grandparent
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| {
            // Clean up common artist directory naming patterns
            let cleaned = name
                .replace(['_', '-'], " ")
                .split_whitespace()
                .filter(|word| {
                    // Filter out common non-artist words
                    let lower = word.to_lowercase();
                    !matches!(
                        lower.as_str(),
                        "artist" | "band" | "group" | "music" | "collection"
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");

            if cleaned.trim().is_empty() {
                "Various Artists".to_string()
            } else {
                cleaned.trim().to_string()
            }
        })
        .unwrap_or_else(|| "Various Artists".to_string());

    Ok((artist, album))
}

/// Sanitize filename to be safe for filesystem
fn sanitize_filename(name: &str) -> String {
    // Replace problematic characters with safe alternatives
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

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
        return Ok(());
    }

    if !quiet {
        info!("📁 Found {} misplaced files to reorganize", files_to_move.len());
    }

    // Group files by their correct artist/album based on metadata
    let mut file_groups: FxHashMap<(String, String), Vec<PathBuf>> = FxHashMap::default();

    for file_path in files_to_move {
        total_processed += 1;

        // Extract artist and album information from the file
        let (artist, album) = extract_artist_album_from_file(&file_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to extract metadata from '{}': {}",
                file_path.display(),
                e
            )
        })?;

        // Create clean names for directory creation
        let clean_artist = sanitize_filename(&artist);
        let clean_album = sanitize_filename(&album);

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
            total_processed, total_groups
        );
    }

    Ok(())
}

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
        if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            let audio_extensions = ["mp3", "flac", "m4a", "ogg", "aac", "wma", "wav", "aiff"];
            if audio_extensions.contains(&ext.as_str()) {
                // Check if file has proper metadata before including it
                match extract_artist_album_from_file(path) {
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
        let clean_artist = sanitize_filename(&artist);
        let clean_album = sanitize_filename(&album);

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use std::io::Write;

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
        assert_eq!(sanitize_filename("normal_name"), "normal_name");
        assert_eq!(sanitize_filename("file with spaces"), "file with spaces");
        assert_eq!(sanitize_filename("file/with\\bad:chars*"), "file_with_bad_chars_");

        Ok(())
    }

    #[test]
    fn test_sanitize_filename_edge_cases() -> Result<()> {
        // Test edge cases
        assert_eq!(sanitize_filename(""), "");
        assert_eq!(sanitize_filename("   "), "");
        assert_eq!(sanitize_filename("file\x00with\x01control\x02chars"), "file_with_control_chars");

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

        let (artist, album) = extract_from_path(&file_path)?;

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

        let (artist, album) = extract_from_path(&file_path)?;

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
        let result = extract_from_path(&file_path);

        // The function should handle this case gracefully
        assert!(result.is_ok());

        Ok(())
    }
}
