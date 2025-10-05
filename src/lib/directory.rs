use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Directory operations and file organization utilities
/// Common patterns for creating directories and organizing files
/// Please update this when adding or changing directory operations
/// Create an album directory structure (Artist/Album)
/// Returns the created album path
pub fn create_album_directory(artists_path: &Path, artist: &str, album: &str) -> Result<PathBuf> {
    let artist_path = artists_path.join(artist);
    let album_path = artist_path.join(album);

    fs::create_dir_all(&album_path).with_context(|| {
        format!(
            "Failed to create album directory '{}'",
            album_path.display()
        )
    })?;

    Ok(album_path)
}

/// Create an album directory structure with dry-run support
pub fn create_album_directory_with_dry_run(
    artists_path: &Path,
    artist: &str,
    album: &str,
    dry_run: bool,
    quiet: bool,
) -> Result<PathBuf> {
    let artist_path = artists_path.join(artist);
    let album_path = artist_path.join(album);

    if dry_run {
        if !quiet {
            info!("Would create directory: {}", album_path.display());
        }
    } else {
        fs::create_dir_all(&album_path).with_context(|| {
            format!(
                "Failed to create album directory '{}'",
                album_path.display()
            )
        })?;
    }

    Ok(album_path)
}

/// Move a file to an album directory
/// Handles filename conflicts and provides detailed error messages
pub fn move_file_to_album(
    file_path: &Path,
    album_path: &Path,
    dry_run: bool,
    quiet: bool,
) -> Result<()> {
    let file_name = file_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("File '{}' has no filename", file_path.display()))?;

    let dest_path = album_path.join(file_name);

    // Skip if source and destination are the same
    if file_path == dest_path {
        return Ok(());
    }

    if dry_run {
        if !quiet {
            info!(
                "  Would move: {} -> {}",
                file_path.display(),
                album_path.display()
            );
        }
    } else {
        // Check if destination already exists
        if dest_path.exists() {
            if !quiet {
                info!(
                    "Warning: File already exists at destination, skipping: {} -> {}",
                    file_path.display(),
                    dest_path.display()
                );
            }
            return Ok(());
        }

        // Move the file
        fs::rename(file_path, &dest_path).with_context(|| {
            format!(
                "Failed to move '{}' to '{}': {}",
                file_path.display(),
                dest_path.display(),
                std::io::Error::last_os_error()
            )
        })?;

        if !quiet {
            info!("Moved: {} -> {}", file_path.display(), dest_path.display());
        }
    }

    Ok(())
}

/// Copy a file to an album directory
/// Handles filename conflicts and provides detailed error messages
pub fn copy_file_to_album(
    file_path: &Path,
    album_path: &Path,
    dry_run: bool,
    quiet: bool,
) -> Result<()> {
    let file_name = file_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("File '{}' has no filename", file_path.display()))?;

    let dest_path = album_path.join(file_name);

    // Skip if source and destination are the same
    if file_path == dest_path {
        return Ok(());
    }

    if dry_run {
        if !quiet {
            info!(
                "  Would copy: {} -> {}",
                file_path.display(),
                album_path.display()
            );
        }
    } else {
        // Check if destination already exists
        if dest_path.exists() {
            if !quiet {
                info!(
                    "Warning: File already exists at destination, skipping: {} -> {}",
                    file_path.display(),
                    dest_path.display()
                );
            }
            return Ok(());
        }

        // Copy the file
        fs::copy(file_path, &dest_path).with_context(|| {
            format!(
                "Failed to copy '{}' to '{}': {}",
                file_path.display(),
                dest_path.display(),
                std::io::Error::last_os_error()
            )
        })?;

        if !quiet {
            info!("Copied: {} -> {}", file_path.display(), dest_path.display());
        }
    }

    Ok(())
}

/// Organize files by artist and album into the proper directory structure
/// This is a comprehensive function that handles both moving and copying files
pub struct FileOrganizationResult {
    pub files_processed: usize,
    pub directories_created: usize,
    pub files_skipped: usize,
}

pub fn organize_files_by_metadata(
    files: &[(PathBuf, String, String)], // (file_path, artist, album)
    music_dir: &Path,
    dry_run: bool,
    quiet: bool,
) -> Result<FileOrganizationResult> {
    let artists_path = music_dir.join("Artists");
    let mut files_processed = 0;
    let mut directories_created = 0;
    let mut files_skipped = 0;

    // Group files by artist and album
    let mut file_groups: std::collections::HashMap<(String, String), Vec<PathBuf>> =
        std::collections::HashMap::new();

    for (file_path, artist, album) in files {
        file_groups
            .entry((artist.clone(), album.clone()))
            .or_default()
            .push(file_path.clone());
    }

    // Process each group
    for ((artist, album), files) in file_groups {
        let album_path =
            create_album_directory_with_dry_run(&artists_path, &artist, &album, dry_run, quiet)?;

        if dry_run {
            directories_created += 0;
        } else if !album_path.exists() {
            directories_created += 1;
        }

        for file_path in files {
            match move_file_to_album(&file_path, &album_path, dry_run, quiet) {
                Ok(_) => files_processed += 1,
                Err(_) => files_skipped += 1,
            }
        }
    }

    Ok(FileOrganizationResult {
        files_processed,
        directories_created,
        files_skipped,
    })
}

/// Copy files by artist and album into the proper directory structure
/// Similar to organize_files_by_metadata but copies instead of moves
pub fn copy_files_by_metadata(
    files: &[(PathBuf, String, String)], // (file_path, artist, album)
    music_dir: &Path,
    dry_run: bool,
    quiet: bool,
) -> Result<FileOrganizationResult> {
    let artists_path = music_dir.join("Artists");
    let mut files_processed = 0;
    let mut directories_created = 0;
    let mut files_skipped = 0;

    // Group files by artist and album
    let mut file_groups: std::collections::HashMap<(String, String), Vec<PathBuf>> =
        std::collections::HashMap::new();

    for (file_path, artist, album) in files {
        file_groups
            .entry((artist.clone(), album.clone()))
            .or_default()
            .push(file_path.clone());
    }

    // Process each group
    for ((artist, album), files) in file_groups {
        let album_path =
            create_album_directory_with_dry_run(&artists_path, &artist, &album, dry_run, quiet)?;

        if dry_run {
            directories_created += 1;
        } else if !album_path.exists() {
            directories_created += 0;
        }

        for file_path in files {
            match copy_file_to_album(&file_path, &album_path, dry_run, quiet) {
                Ok(_) => files_processed += 1,
                Err(_) => files_skipped += 1,
            }
        }
    }

    Ok(FileOrganizationResult {
        files_processed,
        directories_created,
        files_skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_album_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let artists_path = temp_dir.path().join("Artists");
        let artist = "Test Artist";
        let album = "Test Album";

        let album_path = create_album_directory(&artists_path, artist, album)?;

        assert!(album_path.exists());
        assert!(album_path.is_dir());
        assert_eq!(album_path, artists_path.join(artist).join(album));

        Ok(())
    }

    #[test]
    fn test_create_album_directory_with_dry_run() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let artists_path = temp_dir.path().join("Artists");
        let artist = "Test Artist";
        let album = "Test Album";

        // Test dry run - should not create directories
        let album_path = create_album_directory_with_dry_run(
            &artists_path,
            artist,
            album,
            true, // dry_run
            true, // quiet
        )?;

        assert!(!album_path.exists()); // Should not exist in dry run

        // Test actual creation
        let album_path = create_album_directory_with_dry_run(
            &artists_path,
            artist,
            album,
            false, // not dry_run
            true,  // quiet
        )?;

        assert!(album_path.exists());
        assert!(album_path.is_dir());

        Ok(())
    }

    #[test]
    fn test_move_file_to_album_dry_run() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_file = temp_dir.path().join("source.mp3");
        let album_path = temp_dir.path().join("album");

        // Create source file
        fs::write(&source_file, b"test audio content")?;

        // Test dry run - should not move file
        move_file_to_album(&source_file, &album_path, true, true)?;

        assert!(source_file.exists());
        assert!(!album_path.exists());

        Ok(())
    }

    #[test]
    fn test_copy_file_to_album_dry_run() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_file = temp_dir.path().join("source.mp3");
        let album_path = temp_dir.path().join("album");

        // Create source file
        fs::write(&source_file, b"test audio content")?;

        // Test dry run - should not copy file
        copy_file_to_album(&source_file, &album_path, true, true)?;

        assert!(source_file.exists());
        assert!(!album_path.exists());

        Ok(())
    }
}
