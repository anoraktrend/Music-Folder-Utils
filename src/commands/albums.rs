use anyhow::{Context, Result};
use std::path::Path;
use std::fs;
use std::os::unix::fs::symlink;

pub fn process_single_album_symlink(album_path: &Path, music_dir: &str) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir);
    let albums_path = Path::new(music_dir.as_ref()).join("Albums");

    if !albums_path.exists() {
        fs::create_dir(&albums_path)?;
    }

    let artist_path = album_path.parent().context("Album path has no parent")?;
    let artist_name = artist_path.file_name().unwrap().to_str().unwrap();
    let album_name = album_path.file_name().unwrap().to_str().unwrap();
    let link_name = albums_path.join(format!("{} - {}", artist_name, album_name));
    if !link_name.exists() {
        symlink(album_path, &link_name)?;
    }
    Ok(())
}


