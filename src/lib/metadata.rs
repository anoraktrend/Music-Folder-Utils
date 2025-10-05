use anyhow::Result;
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::ItemKey;
use std::path::Path;
use tracing::warn;

/// Extract artist and album information from a music file
pub fn extract_artist_album_from_file(file_path: &Path) -> Result<(String, String)> {
    match lofty::read_from_path(file_path) {
        Ok(tagged_file) => {
            let tags = tagged_file.tags();
            if let Some(tag) = tags.first() {
                // Try multiple artist fields in order of preference
                let artist = tag
                    .get_string(&ItemKey::AlbumArtist)
                    .or_else(|| tag.get_string(&ItemKey::TrackArtist))
                    .unwrap_or_else(|| {
                        // Try to extract from filename if no artist metadata
                        file_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown Artist")
                            .split(" - ")
                            .next()
                            .unwrap_or("Unknown Artist")
                    })
                    .to_string();

                // Try multiple album fields in order of preference
                let album = tag
                    .get_string(&ItemKey::AlbumTitle)
                    .unwrap_or_else(|| {
                        // Try to extract from parent directory name
                        file_path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown Album")
                    })
                    .to_string();

                Ok((artist, album))
            } else {
                // Fallback to path-based extraction
                extract_from_path(file_path)
            }
        }
        Err(_) => {
            // Fallback to path-based extraction
            extract_from_path(file_path)
        }
    }
}

/// Extract artist and album from file path when tags are not available
pub fn extract_from_path(file_path: &Path) -> Result<(String, String)> {
    let parent = file_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("File '{}' has no parent directory", file_path.display()))?;

    // Try to extract album from parent directory name
    let album = parent
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| {
            // Clean up common album directory naming patterns
            let cleaned = name
                .replace(['_', '-'], " ")
                .split_whitespace()
                .filter(|word| {
                    // Filter out common non-album words
                    let lower = word.to_lowercase();
                    !matches!(
                        lower.as_str(),
                        "album" | "music" | "songs" | "tracks" | "collection"
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");

            if cleaned.trim().is_empty() {
                "Unknown Album".to_string()
            } else {
                cleaned.trim().to_string()
            }
        })
        .unwrap_or_else(|| "Unknown Album".to_string());

    let grandparent = parent
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Album directory '{}' has no parent", parent.display()))?;

    // Try to extract artist from grandparent directory name
    let artist = grandparent
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| {
            // Clean up common artist directory naming patterns
            let cleaned = name
                .replace(['_', '-'], " ")
                .split_whitespace()
                .filter(|word| {
                    // Filter out common non-artist words
                    let lower = word.to_lowercase();
                    !matches!(
                        lower.as_str(),
                        "artist" | "band" | "group" | "music" | "collection"
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");

            if cleaned.trim().is_empty() {
                "Various Artists".to_string()
            } else {
                cleaned.trim().to_string()
            }
        })
        .unwrap_or_else(|| "Various Artists".to_string());

    Ok((artist, album))
}

/// Set enhanced metadata with MusicBrainz release ID
pub fn set_enhanced_metadata(
    file_path: &Path,
    artist: &str,
    album: &str,
    release_id: &str,
) -> Result<()> {
    match lofty::read_from_path(file_path) {
        Ok(mut tagged_file) => {
            if let Some(tag) = tagged_file.primary_tag_mut() {
                // Set standard metadata
                tag.insert_text(ItemKey::TrackArtist, artist.to_string());
                tag.insert_text(ItemKey::AlbumArtist, artist.to_string());
                tag.insert_text(ItemKey::AlbumTitle, album.to_string());

                // Add MusicBrainz release ID
                tag.insert_text(ItemKey::MusicBrainzReleaseId, release_id.to_string());

                // Try to save the enhanced metadata
                if let Err(e) = tagged_file.save_to_path(file_path, WriteOptions::default()) {
                    warn!(
                        "Failed to save enhanced metadata for {}: {}",
                        file_path.display(),
                        e
                    );
                }
            }
        }
        Err(e) => {
            warn!(
                "Could not read file for metadata enhancement: {} ({})",
                file_path.display(),
                e
            );
        }
    }

    Ok(())
}
