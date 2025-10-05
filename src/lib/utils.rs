use crate::audio;
use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use shellexpand;

pub fn get_default_music_dir() -> String {
    std::env::var("XDG_MUSIC_DIR").unwrap_or_else(|_| "~/Music".to_string())
}

/// Sanitize filename to be safe for filesystem
pub fn sanitize_filename(name: &str) -> String {
    // Replace problematic characters with safe alternatives
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\'' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Get all album paths from the music directory
pub fn get_all_album_paths(music_dir: &str) -> Result<Vec<PathBuf>> {
    let expanded_music_dir = shellexpand::tilde(music_dir).into_owned();
    let music_path = Path::new(&expanded_music_dir);
    let artists_path = music_path.join("Artists");

    if !artists_path.exists() {
        return Ok(Vec::new());
    }

    let mut album_paths = Vec::new();

    for artist_entry in WalkDir::new(&artists_path)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if artist_entry.path().is_dir() {
            for album_entry in WalkDir::new(artist_entry.path())
                .min_depth(1)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if album_entry.path().is_dir() {
                    album_paths.push(album_entry.path().to_path_buf());
                }
            }
        }
    }

    Ok(album_paths)
}

/// Get all track paths from the music directory
pub fn get_all_track_paths(music_dir: &str) -> Result<Vec<PathBuf>> {
    let mut track_paths = Vec::new();

    let album_paths = get_all_album_paths(music_dir)?;

    for album_path in album_paths {
        for entry in WalkDir::new(&album_path).into_iter().filter_map(|e| e.ok()) {
            if entry.path().is_file() && audio::is_audio_file(entry.path()) {
                track_paths.push(entry.path().to_path_buf());
            }
        }
    }

    Ok(track_paths)
}

/// Get all folder paths from the music directory
pub fn get_all_folder_paths(music_dir: &str) -> Result<Vec<PathBuf>> {
    let mut folder_paths = Vec::new();

    let album_paths = get_all_album_paths(music_dir)?;

    for album_path in album_paths {
        folder_paths.push(album_path);
    }

    Ok(folder_paths)
}

/// Scan a directory for audio files and return statistics
pub struct FileScanResult {
    pub audio_files: Vec<PathBuf>,
    pub files_scanned: usize,
    pub files_skipped: usize,
}

pub fn scan_directory_for_audio_files(dir_path: &Path) -> Result<FileScanResult> {
    let mut audio_files = Vec::new();
    let mut files_scanned = 0;
    let mut files_skipped = 0;

    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        if !entry.path().is_file() {
            continue;
        }

        files_scanned += 1;

        if audio::is_audio_file(entry.path()) {
            audio_files.push(entry.path().to_path_buf());
        } else {
            files_skipped += 1;
        }
    }

    Ok(FileScanResult {
        audio_files,
        files_scanned,
        files_skipped,
    })
}
