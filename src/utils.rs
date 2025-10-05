use anyhow::Result;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn get_default_music_dir() -> String {
    std::env::var("XDG_MUSIC_DIR").unwrap_or_else(|_| shellexpand::tilde("~/Music").into_owned())
}

// Supported audio file extensions
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "ogg"];

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| AUDIO_EXTENSIONS.contains(&ext))
}

fn contains_audio_files(path: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if entry_path.is_file() && is_audio_file(&entry_path) {
                return true;
            }
        }
    }
    false
}

pub fn get_all_album_paths(music_dir: &str) -> Result<Vec<std::path::PathBuf>> {
    let music_dir = shellexpand::tilde(music_dir).to_string();
    let artists_path = Path::new(&music_dir).join("Artists");
    let mut album_paths = Vec::new();

    if !artists_path.exists() {
        return Ok(album_paths);
    }

    for artist_entry in fs::read_dir(&artists_path)?.filter_map(|e| e.ok()) {
        let artist_path = artist_entry.path();
        if artist_path.is_dir() {
            for album_entry in fs::read_dir(&artist_path)?.filter_map(|e| e.ok()) {
                let album_path = album_entry.path();
                if album_path.is_dir() && contains_audio_files(&album_path) {
                    album_paths.push(album_path);
                }
            }
        }
    }
    Ok(album_paths)
}

pub fn get_all_track_paths(music_dir: &str) -> Result<Vec<std::path::PathBuf>> {
    let music_dir = shellexpand::tilde(music_dir).to_string();
    let artists_path = Path::new(&music_dir).join("Artists");
    let mut track_paths = Vec::new();

    if !artists_path.exists() {
        return Ok(track_paths);
    }

    for artist_entry in fs::read_dir(&artists_path)?.filter_map(|e| e.ok()) {
        let artist_path = artist_entry.path();
        if artist_path.is_dir() {
            for album_entry in fs::read_dir(&artist_path)?.filter_map(|e| e.ok()) {
                let album_path = album_entry.path();
                if album_path.is_dir() {
                    for track_entry in fs::read_dir(&album_path)?.filter_map(|e| e.ok()) {
                        let track_path = track_entry.path();
                        if track_path.is_file() && is_audio_file(&track_path) {
                            track_paths.push(track_path);
                        }
                    }
                }
            }
        }
    }
    Ok(track_paths)
}

pub fn get_all_folder_paths(music_dir: &str) -> Result<Vec<std::path::PathBuf>> {
    let music_dir = shellexpand::tilde(music_dir);
    let mut folder_paths = Vec::new();

    for entry in WalkDir::new(music_dir.as_ref())
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            folder_paths.push(entry.path().to_path_buf());
        }
    }
    Ok(folder_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_get_all_album_paths() -> Result<()> {
        let tmp_dir = tempdir()?;
        let music_root = tmp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");

        fs::create_dir_all(&artists_dir)?;

        // Create a dummy music library structure
        let artist1_dir = artists_dir.join("Artist1");
        fs::create_dir(&artist1_dir)?;
        let album1_1_dir = artist1_dir.join("Album1_1");
        fs::create_dir(&album1_1_dir)?;
        fs::File::create(album1_1_dir.join("track1.mp3"))?.write_all(b"test")?;
        let album1_2_dir = artist1_dir.join("Album1_2");
        fs::create_dir(&album1_2_dir)?;
        fs::File::create(album1_2_dir.join("track2.flac"))?.write_all(b"test")?;

        let artist2_dir = artists_dir.join("Artist2");
        fs::create_dir(&artist2_dir)?;
        let album2_1_dir = artist2_dir.join("Album2_1");
        fs::create_dir(&album2_1_dir)?;
        fs::File::create(album2_1_dir.join("track3.m4a"))?.write_all(b"test")?;

        // Add a non-album directory to ensure it's ignored
        let singles_dir = artist1_dir.join("Singles");
        fs::create_dir(&singles_dir)?;
        fs::File::create(singles_dir.join("info.txt"))?.write_all(b"test")?;

        let expected_paths = vec![
            album1_1_dir.clone(),
            album1_2_dir.clone(),
            album2_1_dir.clone(),
        ];

        let mut actual_paths = get_all_album_paths(music_root.to_str().unwrap())?;
        actual_paths.sort(); // Sort to ensure consistent order for comparison

        assert_eq!(actual_paths, expected_paths);

        Ok(())
    }

    #[test]
    fn test_get_all_track_paths() -> Result<()> {
        let tmp_dir = tempdir()?;
        let music_root = tmp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");

        fs::create_dir_all(&artists_dir)?;

        // Create a dummy music library structure
        let artist1_dir = artists_dir.join("Artist1");
        fs::create_dir(&artist1_dir)?;
        let album1_1_dir = artist1_dir.join("Album1_1");
        fs::create_dir(&album1_1_dir)?;
        fs::File::create(album1_1_dir.join("track1.mp3"))?.write_all(b"test")?;
        fs::File::create(album1_1_dir.join("track2.flac"))?.write_all(b"test")?;
        fs::File::create(album1_1_dir.join("info.txt"))?.write_all(b"test")?; // Non-audio file

        let artist2_dir = artists_dir.join("Artist2");
        fs::create_dir(&artist2_dir)?;
        let album2_1_dir = artist2_dir.join("Album2_1");
        fs::create_dir(&album2_1_dir)?;
        fs::File::create(album2_1_dir.join("track3.m4a"))?.write_all(b"test")?;
        fs::File::create(album2_1_dir.join("track4.ogg"))?.write_all(b"test")?;

        let expected_paths = vec![
            album1_1_dir.join("track1.mp3"),
            album1_1_dir.join("track2.flac"),
            album2_1_dir.join("track3.m4a"),
            album2_1_dir.join("track4.ogg"),
        ];

        let mut actual_paths = get_all_track_paths(music_root.to_str().unwrap())?;
        actual_paths.sort(); // Sort to ensure consistent order for comparison

        assert_eq!(actual_paths, expected_paths);

        Ok(())
    }

    #[test]
    fn test_get_all_folder_paths() -> Result<()> {
        let tmp_dir = tempdir()?;
        let music_root = tmp_dir.path().join("Music");
        let artists_dir = music_root.join("Artists");

        fs::create_dir_all(&artists_dir)?;

        // Create a nested directory structure
        let artist1_dir = artists_dir.join("Artist1");
        fs::create_dir(&artist1_dir)?;
        let album1_1_dir = artist1_dir.join("Album1_1");
        fs::create_dir(&album1_1_dir)?;
        let album1_2_dir = artist1_dir.join("Album1_2");
        fs::create_dir(&album1_2_dir)?;

        let artist2_dir = artists_dir.join("Artist2");
        fs::create_dir(&artist2_dir)?;
        let album2_1_dir = artist2_dir.join("Album2_1");
        fs::create_dir(&album2_1_dir)?;

        // Add some files
        fs::File::create(album1_1_dir.join("track1.mp3"))?.write_all(b"test")?;
        fs::File::create(album2_1_dir.join("track2.flac"))?.write_all(b"test")?;

        let actual_paths = get_all_folder_paths(music_root.to_str().unwrap())?;
        let mut expected_paths = vec![
            music_root.clone(),
            artists_dir.clone(),
            artist1_dir.clone(),
            album1_1_dir.clone(),
            album1_2_dir.clone(),
            artist2_dir.clone(),
            album2_1_dir.clone(),
        ];
        expected_paths.sort();

        let mut sorted_actual_paths = actual_paths.clone();
        sorted_actual_paths.sort();

        assert_eq!(sorted_actual_paths, expected_paths);

        Ok(())
    }

    #[test]
    fn test_contains_audio_files() -> Result<()> {
        let tmp_dir = tempdir()?;
        let test_dir = tmp_dir.path();

        // Test directory with audio files
        let audio_dir = test_dir.join("audio");
        fs::create_dir(&audio_dir)?;
        fs::File::create(audio_dir.join("track1.mp3"))?.write_all(b"test")?;
        fs::File::create(audio_dir.join("track2.flac"))?.write_all(b"test")?;

        assert!(contains_audio_files(&audio_dir));

        // Test directory without audio files
        let no_audio_dir = test_dir.join("no_audio");
        fs::create_dir(&no_audio_dir)?;
        fs::File::create(no_audio_dir.join("info.txt"))?.write_all(b"test")?;
        fs::File::create(no_audio_dir.join("cover.jpg"))?.write_all(b"test")?;

        assert!(!contains_audio_files(&no_audio_dir));

        // Test empty directory
        let empty_dir = test_dir.join("empty");
        fs::create_dir(&empty_dir)?;

        assert!(!contains_audio_files(&empty_dir));

        Ok(())
    }

    #[test]
    fn test_get_all_album_paths_with_no_artists_dir() -> Result<()> {
        let tmp_dir = tempdir()?;
        let music_root = tmp_dir.path().join("Music");

        // Don't create Artists directory
        let album_paths = get_all_album_paths(music_root.to_str().unwrap())?;

        assert_eq!(album_paths.len(), 0);

        Ok(())
    }

    #[test]
    fn test_get_all_album_paths_with_tilde_expansion() -> Result<()> {
        let tmp_dir = tempdir()?;
        let home_dir = tmp_dir.path().join("home").join("user");
        fs::create_dir_all(&home_dir)?;

        // Set the HOME environment variable for this test
        std::env::set_var("HOME", home_dir.to_str().unwrap());

        let music_root = home_dir.join("Music");
        let artists_dir = music_root.join("Artists");
        fs::create_dir_all(&artists_dir)?;

        let artist_dir = artists_dir.join("TestArtist");
        fs::create_dir(&artist_dir)?;
        let album_dir = artist_dir.join("TestAlbum");
        fs::create_dir(&album_dir)?;
        fs::File::create(album_dir.join("track.mp3"))?.write_all(b"test")?;

        // Test with tilde path
        let tilde_path = "~/Music";
        let album_paths = get_all_album_paths(tilde_path)?;

        assert_eq!(album_paths.len(), 1);
        assert_eq!(album_paths[0], album_dir);

        Ok(())
    }
}
