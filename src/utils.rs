use anyhow::Result;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn get_all_album_paths(music_dir: &str) -> Result<Vec<std::path::PathBuf>> {
    let music_dir = shellexpand::tilde(music_dir);
    let artists_path = Path::new(music_dir.as_ref()).join("Artists");
    let mut album_paths = Vec::new();

    if !artists_path.exists() {
        return Ok(album_paths);
    }

    for artist_entry in fs::read_dir(&artists_path)?.filter_map(|e| e.ok()) {
        let artist_path = artist_entry.path();
        if artist_path.is_dir() {
            for album_entry in fs::read_dir(&artist_path)?.filter_map(|e| e.ok()) {
                let album_path = album_entry.path();
                if album_path.is_dir() {
                    album_paths.push(album_path);
                }
            }
        }
    }
    Ok(album_paths)
}

pub fn get_all_track_paths(music_dir: &str) -> Result<Vec<std::path::PathBuf>> {
    let music_dir = shellexpand::tilde(music_dir);
    let artists_path = Path::new(music_dir.as_ref()).join("Artists");
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
                        if track_path.is_file() {
                            let ext = track_path.extension().and_then(|s| s.to_str());
                            if matches!(ext, Some("mp3") | Some("flac") | Some("m4a") | Some("ogg"))
                            {
                                track_paths.push(track_path);
                            }
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
