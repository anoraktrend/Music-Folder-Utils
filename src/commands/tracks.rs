use anyhow::{Result};
use std::path::Path;
use std::fs;
use std::os::unix::fs::symlink;

pub fn process_single_track_symlink(track_path: &Path, music_dir: &str) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir);
    let tracks_path = Path::new(music_dir.as_ref()).join("Tracks");

    if !tracks_path.exists() {
        fs::create_dir(&tracks_path)?;
    }

    let link_name = tracks_path.join(track_path.file_name().unwrap());
    if !link_name.exists() {
        symlink(track_path, &link_name)?;
    }
    Ok(())}

pub fn create_track_symlinks(_music_dir: &str) -> Result<()> {
    // This function is now handled by run_tui and process_single_track_symlink
    Ok(())
}
