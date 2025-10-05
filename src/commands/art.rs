use anyhow::Result;
use ffmpeg_next as ffmpeg;
use ffmpeg_next::format::stream::Disposition;
use gio::prelude::*;
use lofty::{self, file::TaggedFileExt, tag::ItemKey};
use magick_rust::MagickWand;
use reqwest;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;
use tracing::{error, info, warn};
use urlencoding;

/// Validates that required API keys are present before making network requests
pub fn validate_api_keys() -> Result<()> {
    let pexels_key = env::var("PEXELS_API_KEY");
    let audiodb_key = env::var("AUDIODB_API_KEY");

    if pexels_key.is_err() {
        warn!("PEXELS_API_KEY not set - placeholder image fetching will be disabled");
    }

    if audiodb_key.is_err() {
        warn!("AUDIODB_API_KEY not set - artist image fetching will be disabled");
    }

    // Don't fail completely if keys are missing - just warn and continue with local fallbacks
    Ok(())
}

fn pexels_api_key() -> Option<String> {
    env::var("PEXELS_API_KEY").ok()
}

fn audiodb_api_key() -> Option<String> {
    env::var("AUDIODB_API_KEY").ok()
}

fn extract_album_artist_from_directory(artist_path: &Path) -> Result<Option<String>> {
    // For AlbumArtist metadata, look in the first directory's first music file
    // This is more efficient and accurate than searching all files

    // Get the first subdirectory (first album)
    if let Some(first_album_dir) = fs::read_dir(artist_path)?
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_dir())
    {
        let album_path = first_album_dir.path();

        // Find the first music file in that album directory
        if let Some(music_file) = fs::read_dir(&album_path)?.filter_map(|e| e.ok()).find(|e| {
            let path = e.path();
            if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();

                // Check if it's an audio file
                matches!(
                    ext.as_str(),
                    "mp3" | "flac" | "m4a" | "ogg" | "aac" | "wma" | "wav" | "aiff"
                )
            } else {
                false
            }
        }) {
            // Read the metadata from this single file
            if let Ok(tagged_file) = lofty::read_from_path(music_file.path()) {
                let tags = tagged_file.tags();
                if let Some(tag) = tags.first() {
                    // Try to get album artist first, fall back to track artist
                    if let Some(album_artist) = tag.get_string(&ItemKey::AlbumArtist) {
                        return Ok(Some(album_artist.to_string()));
                    } else if let Some(track_artist) = tag.get_string(&ItemKey::TrackArtist) {
                        return Ok(Some(track_artist.to_string()));
                    }
                }
            }
        }
    }

    // If no album artist found, return None
    Ok(None)
}

#[derive(Deserialize, Debug)]
struct PexelsPhotoSrc {
    large: String,
}

#[derive(Deserialize, Debug)]
struct PexelsPhoto {
    src: PexelsPhotoSrc,
}

#[derive(Deserialize, Debug)]
struct PexelsSearchResponse {
    photos: Vec<PexelsPhoto>,
}

pub fn extract_artist_art(music_dir: &str) -> Result<()> {
    // Validate API keys before starting
    validate_api_keys()?;

    let music_dir = shellexpand::tilde(music_dir);
    let artists_path = Path::new(music_dir.as_ref()).join("Artists");

    for artist_entry in fs::read_dir(&artists_path)?.filter_map(|e| e.ok()) {
        let artist_path = artist_entry.path();
        if artist_path.is_dir() {
            let output_file = artist_path.join(".folder.jpg");
            if !output_file.exists() {
                // Extract album artist from music files in this directory
                let album_artist = extract_album_artist_from_directory(&artist_path)?;

                if let Some(artist_name) = album_artist {
                    let rt = tokio::runtime::Runtime::new()?; // Need a runtime for async call
                    let audiodb_fetch_successful = rt.block_on(async {
                        let client = reqwest::Client::new();
                        let key = audiodb_api_key();
                        if key.is_none() {
                            warn!("AUDIODB_API_KEY not set, skipping AudioDB artist fetch for {}", artist_name);
                            return Ok::<bool, anyhow::Error>(false);
                        }
                        let audiodb_url = format!("https://www.theaudiodb.com/api/v1/json/{}/search.php?s={}", key.unwrap(), urlencoding::encode(&artist_name));

                        match client.get(&audiodb_url).send().await {
                            Ok(response) => {
                                if response.status().is_success() {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(audiodb_json) => {
                                            if let Some(artists) = audiodb_json["artists"].as_array() {
                                                if let Some(artist) = artists.first() {
                                                    if let Some(image_url) = artist["strArtistThumb"].as_str() {
                                                        match reqwest::get(image_url).await {
                                                            Ok(image_response) => {
                                                                match image_response.bytes().await {
                                                                    Ok(image_content) => {
                                                                        if fs::write(&output_file, &image_content).is_ok() {
                                                                            info!("Artist image fetched from AudioDB for: {} (album artist)", artist_name);
                                                                            return Ok(true);
                                                                        }
                                                                    }
                                                                    Err(e) => error!("Failed to read image bytes: {}", e),
                                                                }
                                                            }
                                                            Err(e) => error!("Failed to fetch image: {}", e),
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => error!("Failed to parse AudioDB JSON: {}", e),
                                    }
                                } else {
                                    error!("Error searching AudioDB for artist {}: {}", artist_name, response.status());
                                }
                            }
                            Err(e) => error!("Failed to send AudioDB request: {}", e),
                        }
                        Ok(false)
                    })?;

                    if !audiodb_fetch_successful {
                        // If AudioDB failed, check for existing folder.jpg
                        let folder_jpg_path = artist_path.join("folder.jpg");
                        if folder_jpg_path.exists() {
                            fs::copy(&folder_jpg_path, &output_file)?;
                            info!(
                                "Copied {} to {}",
                                folder_jpg_path.display(),
                                output_file.display()
                            );
                        }
                    }
                } else {
                    warn!(
                        "No album artist metadata found in directory: {}",
                        artist_path.display()
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn process_single_album_art(current_dir: &Path) -> Result<()> {
    let output_file = current_dir.join(".folder.jpg");
    if output_file.exists() {
        return Ok(());
    }

    let music_file = fs::read_dir(current_dir)?.filter_map(|e| e.ok()).find(|e| {
        let path = e.path();
        if path.is_file() {
            let ext = path.extension().and_then(|s| s.to_str());
            matches!(ext, Some("mp3") | Some("flac") | Some("m4a"))
        } else {
            false
        }
    });

    if let Some(music_file) = music_file {
        if let Ok(mut ictx) = ffmpeg::format::input(&music_file.path()) {
            let stream_index = ictx
                .streams()
                .find(|s| s.disposition().contains(Disposition::ATTACHED_PIC))
                .map(|s| s.index());

            if let Some(index) = stream_index {
                for (s, packet) in ictx.packets() {
                    if s.index() == index {
                        if let Some(data) = packet.data() {
                            fs::write(&output_file, data)?;
                            info!("Album art extracted to {}", output_file.display());
                        }
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn set_folder_icons_callback(current_dir: &Path) -> Result<()> {
    let icon_path = current_dir.join(".folder.jpg");
    if icon_path.exists() {
        let file = gio::File::for_path(current_dir);
        let icon_uri = format!("file://{}", icon_path.display());
        file.set_attribute_string(
            "metadata::custom-icon",
            &icon_uri,
            gio::FileQueryInfoFlags::NONE,
            None::<&gio::Cancellable>,
        )?;

        let directory_file = current_dir.join(".directory");
        fs::write(directory_file, "[Desktop Entry]\nIcon=./.folder.jpg")?;
    }
    Ok(())
}

async fn fetch_and_save_placeholder(path: &Path, name: &str, category: &str) -> Result<()> {
    let placeholder_path = path.join(".folder.jpg");
    if !placeholder_path.exists() {
        info!("Fetching placeholder for {}: {}", name, path.display());

        // Try to extract album artist from music files first
        let search_name = if let Ok(album_artist) = extract_album_artist_from_directory(path) {
            album_artist.unwrap_or_else(|| name.to_string())
        } else {
            name.to_string()
        };

        let client = reqwest::Client::new();
        let query = format!("{} {}", category, search_name);
        let url = format!(
            "https://api.pexels.com/v1/search?query={}&per_page=1",
            urlencoding::encode(&query)
        );
        let key = pexels_api_key();
        if key.is_none() {
            warn!(
                "PEXELS_API_KEY not set, skipping placeholder fetch for {}",
                name
            );
            return Ok(());
        }

        match client
            .get(&url)
            .header("Authorization", key.unwrap())
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<PexelsSearchResponse>().await {
                        Ok(search_result) => {
                            if let Some(photo) = search_result.photos.first() {
                                let image_url = &photo.src.large;
                                match reqwest::get(image_url).await {
                                    Ok(image_response) => match image_response.bytes().await {
                                        Ok(image_content) => {
                                            if fs::write(&placeholder_path, &image_content).is_ok()
                                            {
                                                info!(
                                                        "Placeholder fetched for {}: {} (searched by album artist)",
                                                        name,
                                                        path.display()
                                                    );
                                            }
                                        }
                                        Err(e) => error!("Failed to read image bytes: {}", e),
                                    },
                                    Err(e) => error!("Failed to fetch image: {}", e),
                                }
                            } else {
                                warn!(
                                    "No image found for {}: {} (searched by album artist)",
                                    name,
                                    path.display()
                                );
                            }
                        }
                        Err(e) => error!("Failed to parse Pexels JSON: {}", e),
                    }
                } else {
                    error!(
                        "Error searching Pexels for {}: {}: {}",
                        name,
                        path.display(),
                        response.status()
                    );
                }
            }
            Err(e) => error!("Failed to send Pexels request: {}", e),
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    Ok(())
}

pub async fn fetch_placeholders(music_dir: &str) -> Result<()> {
    // Validate API keys before starting
    validate_api_keys()?;

    let music_dir = shellexpand::tilde(music_dir);
    let artists_path = Path::new(music_dir.as_ref()).join("Artists");
    let albums_path = Path::new(music_dir.as_ref()).join("Albums");
    let tracks_path = Path::new(music_dir.as_ref()).join("Tracks");

    // Fetch for root Artists, Albums, Tracks directories
    fetch_and_save_placeholder(&artists_path, "Artists", "Music Artists").await?;
    crop_image_to_square(&artists_path.join(".folder.jpg"))?;

    fetch_and_save_placeholder(&albums_path, "Albums", "Music Albums").await?;
    crop_image_to_square(&albums_path.join(".folder.jpg"))?;

    fetch_and_save_placeholder(&tracks_path, "Tracks", "Music Tracks").await?;
    crop_image_to_square(&tracks_path.join(".folder.jpg"))?;

    Ok(())
}

pub fn crop_image_to_square(image_path: &Path) -> Result<()> {
    if !image_path.exists() {
        return Ok(()); // No image to crop
    }

    let image_content = fs::read(image_path)?;
    let mut wand = MagickWand::new();
    wand.read_image_blob(&image_content)?;

    let width = wand.get_image_width();
    let height = wand.get_image_height();
    let size = std::cmp::min(width, height);
    let x = (width - size) / 2;
    let y = (height - size) / 2;

    wand.crop_image(size, size, x as isize, y as isize)?;
    wand.set_image_format("jpeg")?;

    fs::write(image_path, &wand.write_image_blob("jpeg")?)?;
    info!("Image cropped: {}", image_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_validate_api_keys_missing_keys() {
        // Save original values
        let original_pexels = env::var("PEXELS_API_KEY");
        let original_audiodb = env::var("AUDIODB_API_KEY");

        // Remove API keys to simulate missing state
        env::remove_var("PEXELS_API_KEY");
        env::remove_var("AUDIODB_API_KEY");

        // This should not fail, just warn
        let result = validate_api_keys();
        assert!(result.is_ok());

        // Restore original values
        if let Ok(pexels) = original_pexels {
            env::set_var("PEXELS_API_KEY", pexels);
        }
        if let Ok(audiodb) = original_audiodb {
            env::set_var("AUDIODB_API_KEY", audiodb);
        }
    }

    #[test]
    fn test_validate_api_keys_with_keys() {
        // Save original values
        let original_pexels = env::var("PEXELS_API_KEY");
        let original_audiodb = env::var("AUDIODB_API_KEY");

        // Set test keys
        env::set_var("PEXELS_API_KEY", "test_pexels_key");
        env::set_var("AUDIODB_API_KEY", "test_audiodb_key");

        // This should succeed without warnings
        let result = validate_api_keys();
        assert!(result.is_ok());

        // Restore original values
        if let Ok(pexels) = original_pexels {
            env::set_var("PEXELS_API_KEY", pexels);
        } else {
            env::remove_var("PEXELS_API_KEY");
        }
        if let Ok(audiodb) = original_audiodb {
            env::set_var("AUDIODB_API_KEY", audiodb);
        } else {
            env::remove_var("AUDIODB_API_KEY");
        }
    }

    #[test]
    fn test_pexels_api_key_function() {
        // Save original value
        let original = env::var("PEXELS_API_KEY");

        // Test with key set
        env::set_var("PEXELS_API_KEY", "test_key");
        assert_eq!(pexels_api_key(), Some("test_key".to_string()));

        // Test without key
        env::remove_var("PEXELS_API_KEY");
        assert_eq!(pexels_api_key(), None);

        // Restore original value
        if let Ok(key) = original {
            env::set_var("PEXELS_API_KEY", key);
        }
    }

    #[test]
    fn test_audiodb_api_key_function() {
        // Save original value
        let original = env::var("AUDIODB_API_KEY");

        // Test with key set
        env::set_var("AUDIODB_API_KEY", "test_key");
        assert_eq!(audiodb_api_key(), Some("test_key".to_string()));

        // Test without key
        env::remove_var("AUDIODB_API_KEY");
        assert_eq!(audiodb_api_key(), None);

        // Restore original value
        if let Ok(key) = original {
            env::set_var("AUDIODB_API_KEY", key);
        }
    }

    #[test]
    fn test_crop_image_to_square_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let missing_file = temp_dir.path().join("missing.jpg");

        // Should not fail when file doesn't exist
        let result = crop_image_to_square(&missing_file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_album_artist_from_directory_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let empty_dir = temp_dir.path();

        // Should return None for empty directory
        let result = extract_album_artist_from_directory(empty_dir);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }
}
