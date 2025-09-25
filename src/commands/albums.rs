use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn test_process_single_album_symlink_valid_structure() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");
        let albums_dir = music_root.join("Albums");

        fs::create_dir_all(&artists_dir)?;
        fs::create_dir_all(&albums_dir)?;

        // Create proper structure: Music/Artists/Artist/Album
        let artist_dir = artists_dir.join("TestArtist");
        fs::create_dir(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        fs::File::create(album_dir.join("track1.mp3"))?.write_all(b"test")?;

        // Test the function
        let result = process_single_album_symlink(&album_dir, music_root.to_str().unwrap());

        assert!(result.is_ok());

        // Check that symlink was created
        let expected_link = albums_dir.join("TestArtist - TestAlbum");
        assert!(expected_link.exists());
        assert!(expected_link.is_symlink());

        // Verify the symlink points to the correct target
        let link_target = fs::read_link(&expected_link)?;
        assert_eq!(link_target, album_dir);

        Ok(())
    }

    #[test]
    fn test_process_single_album_symlink_invalid_path() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");

        fs::create_dir_all(&artists_dir)?;

        // Create album path outside of expected structure
        let invalid_album = temp_dir.path().join("InvalidAlbum");
        fs::create_dir(&invalid_album)?;

        // Test the function - should fail
        let result = process_single_album_symlink(&invalid_album, music_root.to_str().unwrap());

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not within the expected Artists directory"));

        Ok(())
    }

    #[test]
    fn test_process_single_album_symlink_already_exists_correct() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");
        let albums_dir = music_root.join("Albums");

        fs::create_dir_all(&artists_dir)?;
        fs::create_dir_all(&albums_dir)?;

        // Create proper structure
        let artist_dir = artists_dir.join("TestArtist");
        fs::create_dir(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        fs::File::create(album_dir.join("track1.mp3"))?.write_all(b"test")?;

        // Create the symlink manually first
        let link_path = albums_dir.join("TestArtist - TestAlbum");
        symlink(&album_dir, &link_path)?;

        // Test the function - should succeed without recreating
        let result = process_single_album_symlink(&album_dir, music_root.to_str().unwrap());

        assert!(result.is_ok());

        Ok(())
    }

    #[test]
    fn test_process_single_album_symlink_already_exists_wrong_target() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");
        let albums_dir = music_root.join("Albums");

        fs::create_dir_all(&artists_dir)?;
        fs::create_dir_all(&albums_dir)?;

        // Create proper structure
        let artist_dir = artists_dir.join("TestArtist");
        fs::create_dir(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        fs::File::create(album_dir.join("track1.mp3"))?.write_all(b"test")?;

        // Create a different album to link to
        let wrong_album = artist_dir.join("WrongAlbum");
        fs::create_dir(&wrong_album)?;

        // Create the symlink pointing to wrong target
        let link_path = albums_dir.join("TestArtist - TestAlbum");
        symlink(&wrong_album, &link_path)?;

        // Test the function - should recreate the symlink
        let result = process_single_album_symlink(&album_dir, music_root.to_str().unwrap());

        assert!(result.is_ok());

        // Verify the symlink now points to the correct target
        let link_target = fs::read_link(&link_path)?;
        assert_eq!(link_target, album_dir);

        Ok(())
    }

    #[test]
    fn test_process_single_album_symlink_missing_artists_dir() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");

        // Don't create Artists directory
        let artist_dir = music_root.join("Artists").join("TestArtist");
        fs::create_dir_all(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        fs::File::create(album_dir.join("track1.mp3"))?.write_all(b"test")?;

        // Test the function - should create Albums directory
        let result = process_single_album_symlink(&album_dir, music_root.to_str().unwrap());

        assert!(result.is_ok());

        // Check that Albums directory was created
        let albums_dir = music_root.join("Albums");
        assert!(albums_dir.exists());
        assert!(albums_dir.is_dir());

        Ok(())
    }

    #[test]
    fn test_process_single_album_symlink_invalid_unicode_names() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");

        fs::create_dir_all(&artists_dir)?;

        // Create directories with invalid unicode names (using filesystem-safe names)
        let artist_dir = artists_dir.join("Test_Artist");
        fs::create_dir(&artist_dir)?;
        let album_dir = artist_dir.join("Test_Album");
        fs::create_dir(&album_dir)?;
        fs::File::create(album_dir.join("track1.mp3"))?.write_all(b"test")?;

        // Test the function
        let result = process_single_album_symlink(&album_dir, music_root.to_str().unwrap());

        assert!(result.is_ok());

        // Check that symlink was created with proper names
        let albums_dir = music_root.join("Albums");
        let expected_link = albums_dir.join("Test_Artist - Test_Album");
        assert!(expected_link.exists());

        Ok(())
    }
}
