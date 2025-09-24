use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

pub fn process_single_album_symlink(album_path: &Path, music_dir: &str) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir);
    let music_path = Path::new(music_dir.as_ref());
    let artists_path = music_path.join("Artists");

    // Validate that the album path is within the expected Artists directory structure
    let album_path = PathBuf::from(album_path);

    // Check if album_path is within music_dir/Artists/
    if !album_path.starts_with(&artists_path) {
        return Err(anyhow::anyhow!(
            "Album path '{}' is not within the expected Artists directory '{}'",
            album_path.display(),
            artists_path.display()
        ));
    }

    // Ensure the album path has a valid parent (artist directory)
    let artist_path = album_path.parent()
        .ok_or_else(|| anyhow::anyhow!("Album path '{}' has no parent directory", album_path.display()))?;

    // Ensure the artist directory is directly under Artists
    if artist_path.parent() != Some(&artists_path) {
        return Err(anyhow::anyhow!(
            "Album path '{}' is not in the expected structure (should be Artists/Artist/Album)",
            album_path.display()
        ));
    }

    // Validate that both artist and album paths exist and are directories
    if !artist_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Artist path '{}' is not a directory",
            artist_path.display()
        ));
    }

    if !album_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Album path '{}' is not a directory",
            album_path.display()
        ));
    }

    // Ensure the Albums directory exists
    let albums_path = music_path.join("Albums");
    if !albums_path.exists() {
        fs::create_dir(&albums_path)?;
    }

    // Get artist and album names safely
    let artist_name = artist_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid artist directory name in path '{}'", artist_path.display()))?;

    let album_name = album_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid album directory name in path '{}'", album_path.display()))?;

    let link_name = albums_path.join(format!("{} - {}", artist_name, album_name));

    // Check for existing symlink
    if link_name.exists() {
        if link_name.is_symlink() {
            let current_target = fs::read_link(&link_name)?;
            if current_target == album_path {
                // Already correctly linked, skip
                return Ok(());
            }
        }
        // Remove existing file/symlink and create new one
        fs::remove_file(&link_name)?;
    }

    // Create the symlink
    symlink(&album_path, &link_name)
        .with_context(|| format!("Failed to create symlink from '{}' to '{}'", link_name.display(), album_path.display()))?;

    Ok(())
}
