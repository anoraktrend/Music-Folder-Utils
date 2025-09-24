use anyhow::Result;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

pub fn process_single_track_symlink(track_path: &Path, music_dir: &str) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir);
    let tracks_path = Path::new(music_dir.as_ref()).join("Tracks");

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
