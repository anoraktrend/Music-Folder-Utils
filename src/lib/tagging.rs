use anyhow::Result;
use std::path::Path;
use std::sync::mpsc;
use musicbrainz_rs::entity::release::Release;

/// Update MusicBrainz release ID on a music file
pub fn update_musicbrainz_release_id(
    file_path: &Path,
    release_id: &str,
    tx: &mpsc::Sender<String>,
) -> Result<()> {
    // Use the library function to set enhanced metadata
    match super::metadata::set_enhanced_metadata(file_path, "", "", release_id) {
        Ok(_) => {
            tx.send(format!("COMPLETED: {} - MusicBrainz ID updated", file_path.display()))?;
        }
        Err(e) => {
            tx.send(format!(
                "COMPLETED: {} - Failed to save MusicBrainz ID: {}",
                file_path.display(), e
            ))?;
        }
    }

    Ok(())
}

/// Extract artist and album from a file path with fallback to folder names
pub fn extract_artist_album_from_path_with_fallback(
    file_path: &Path,
    folder_artist: &str,
    folder_album: &str,
) -> (String, String) {
    match super::metadata::extract_artist_album_from_file(file_path) {
        Ok((artist, album)) => (artist, album),
        Err(_) => (folder_artist.to_string(), folder_album.to_string()),
    }
}

/// Process a single music file with MusicBrainz data
pub fn process_music_file_with_musicbrainz(
    file_path: &Path,
    release_id: &str,
    _relative_path: &str,
    tx: &mpsc::Sender<String>,
) -> Result<()> {
    // Create a minimal Release instance for compatibility
    let _dummy_release = Release {
        id: "".to_string(),
        title: "".to_string(),
        artist_credit: None,
        release_group: None,
        date: None,
        country: None,
        label_info: None,
        disambiguation: None,
        packaging: None,
        status: None,
        barcode: None,
        asin: None,
        annotation: None,
        quality: None,
        status_id: None,
        packaging_id: None,
        relations: None,
        media: None,
        tags: None,
        aliases: None,
        genres: None,
        text_representation: None,
        cover_art_archive: None,
        release_events: None,
    };

    update_musicbrainz_release_id(file_path, release_id, tx)
}