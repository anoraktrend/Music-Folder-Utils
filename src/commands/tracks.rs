use anyhow::Result;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

pub fn process_single_track_symlink(track_path: &Path, music_dir: &str) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir);
    let tracks_path = Path::new(music_dir.as_ref()).join("Tracks");

    // Check if the source file exists
    if !track_path.exists() {
        return Err(anyhow::anyhow!("Track file '{}' does not exist", track_path.display()));
    }

    if !tracks_path.exists() {
        fs::create_dir(&tracks_path)?;
    }

    let link_name = tracks_path.join(track_path.file_name().unwrap());

    if link_name.exists() {
        // Check if it's already a symlink to the correct target
        if link_name.is_symlink() {
            let current_target = fs::read_link(&link_name)?;
            if current_target == track_path {
                // Already correctly linked, skip
                return Ok(());
            }
        }
        // Remove existing file/symlink and create new one
        fs::remove_file(&link_name)?;
    }

    symlink(track_path, &link_name)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::symlink;

    #[test]
    fn test_process_single_track_symlink_valid_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let tracks_dir = music_root.join("Tracks");

        fs::create_dir_all(&tracks_dir)?;

        // Create a test track file
        let artist_dir = music_root.join("Artists").join("TestArtist");
        fs::create_dir_all(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        let track_file = album_dir.join("test_track.mp3");
        fs::File::create(&track_file)?.write_all(b"test audio content")?;

        // Test the function
        let result = process_single_track_symlink(&track_file, music_root.to_str().unwrap());

        assert!(result.is_ok());

        // Check that symlink was created
        let expected_link = tracks_dir.join("test_track.mp3");
        assert!(expected_link.exists());
        assert!(expected_link.is_symlink());

        // Verify the symlink points to the correct target
        let link_target = fs::read_link(&expected_link)?;
        assert_eq!(link_target, track_file);

        Ok(())
    }

    #[test]
    fn test_process_single_track_symlink_missing_tracks_dir() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");

        // Don't create Tracks directory
        let artist_dir = music_root.join("Artists").join("TestArtist");
        fs::create_dir_all(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        let track_file = album_dir.join("test_track.mp3");
        fs::File::create(&track_file)?.write_all(b"test audio content")?;

        // Test the function - should create Tracks directory
        let result = process_single_track_symlink(&track_file, music_root.to_str().unwrap());

        assert!(result.is_ok());

        // Check that Tracks directory was created
        let tracks_dir = music_root.join("Tracks");
        assert!(tracks_dir.exists());
        assert!(tracks_dir.is_dir());

        // Check that symlink was created
        let expected_link = tracks_dir.join("test_track.mp3");
        assert!(expected_link.exists());

        Ok(())
    }

    #[test]
    fn test_process_single_track_symlink_already_exists_correct() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let tracks_dir = music_root.join("Tracks");

        fs::create_dir_all(&tracks_dir)?;

        // Create a test track file
        let artist_dir = music_root.join("Artists").join("TestArtist");
        fs::create_dir_all(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        let track_file = album_dir.join("test_track.mp3");
        fs::File::create(&track_file)?.write_all(b"test audio content")?;

        // Create the symlink manually first
        let link_path = tracks_dir.join("test_track.mp3");
        symlink(&track_file, &link_path)?;

        // Test the function - should succeed without recreating
        let result = process_single_track_symlink(&track_file, music_root.to_str().unwrap());

        assert!(result.is_ok());

        Ok(())
    }

    #[test]
    fn test_process_single_track_symlink_already_exists_wrong_target() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let tracks_dir = music_root.join("Tracks");

        fs::create_dir_all(&tracks_dir)?;

        // Create test track files
        let artist_dir = music_root.join("Artists").join("TestArtist");
        fs::create_dir_all(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        let correct_track = album_dir.join("correct_track.mp3");
        fs::File::create(&correct_track)?.write_all(b"correct content")?;

        let wrong_track = album_dir.join("wrong_track.mp3");
        fs::File::create(&wrong_track)?.write_all(b"wrong content")?;

        // Create the symlink pointing to wrong target
        let link_path = tracks_dir.join("correct_track.mp3");
        symlink(&wrong_track, &link_path)?;

        // Test the function - should recreate the symlink
        let result = process_single_track_symlink(&correct_track, music_root.to_str().unwrap());

        assert!(result.is_ok());

        // Verify the symlink now points to the correct target
        let link_target = fs::read_link(&link_path)?;
        assert_eq!(link_target, correct_track);

        Ok(())
    }

    #[test]
    fn test_process_single_track_symlink_nonexistent_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let music_root = temp_dir.path().join("Music");
        let tracks_dir = music_root.join("Tracks");

        fs::create_dir_all(&tracks_dir)?;

        // Create path to nonexistent file
        let nonexistent_file = temp_dir.path().join("nonexistent.mp3");

        // Test the function - should fail
        let result = process_single_track_symlink(&nonexistent_file, music_root.to_str().unwrap());

        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_process_single_track_symlink_with_tilde_expansion() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let home_dir = temp_dir.path().join("home").join("user");
        fs::create_dir_all(&home_dir)?;

        // Set the HOME environment variable for this test
        std::env::set_var("HOME", home_dir.to_str().unwrap());

        let music_root = home_dir.join("Music");
        let tracks_dir = music_root.join("Tracks");
        fs::create_dir_all(&tracks_dir)?;

        // Create a test track file
        let artist_dir = music_root.join("Artists").join("TestArtist");
        fs::create_dir_all(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        let track_file = album_dir.join("test_track.mp3");
        fs::File::create(&track_file)?.write_all(b"test audio content")?;

        // Test with tilde path
        let tilde_path = "~/Music";
        let result = process_single_track_symlink(&track_file, tilde_path);

        assert!(result.is_ok());

        // Check that symlink was created
        let expected_link = tracks_dir.join("test_track.mp3");
        assert!(expected_link.exists());

        Ok(())
    }
}
