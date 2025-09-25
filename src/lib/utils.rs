/// Sanitize filename to be safe for filesystem
pub fn sanitize_filename(name: &str) -> String {
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

use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// Extension definitions used throughout the module
const ID3_EXTENSIONS: &[&str] = &["mp3", "aac"];
const MP4_EXTENSIONS: &[&str] = &["m4a", "m4b", "m4p", "alac", "mp4"];
const VORBIS_EXTENSIONS: &[&str] = &["flac", "ogg", "oga", "opus", "spx"];
const APE_EXTENSIONS: &[&str] = &["ape", "mpc", "wv"];
const AIFF_EXTENSIONS: &[&str] = &["aiff", "aif"];
const WAV_EXTENSIONS: &[&str] = &["wav"];

/// Get all album paths from the music directory
pub fn get_all_album_paths(music_dir: &str) -> Result<Vec<PathBuf>> {
    let music_path = Path::new(music_dir);
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
        for entry in WalkDir::new(&album_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.path().is_file() {
                let ext = entry
                    .path()
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();

                let all_extensions: Vec<_> = ID3_EXTENSIONS
                    .iter()
                    .chain(MP4_EXTENSIONS.iter())
                    .chain(VORBIS_EXTENSIONS.iter())
                    .chain(APE_EXTENSIONS.iter())
                    .chain(AIFF_EXTENSIONS.iter())
                    .chain(WAV_EXTENSIONS.iter())
                    .collect();

                if all_extensions.iter().any(|&&e| e == ext) {
                    track_paths.push(entry.path().to_path_buf());
                }
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