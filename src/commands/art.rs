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
use urlencoding;

fn pexels_api_key() -> Result<String> {
    env::var("PEXELS_API_KEY")
        .map_err(|_| anyhow::anyhow!("Missing PEXELS_API_KEY environment variable"))
}

fn audiodb_api_key() -> Result<String> {
    env::var("AUDIODB_API_KEY")
        .map_err(|_| anyhow::anyhow!("Missing AUDIODB_API_KEY environment variable"))
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
                        let key = match audiodb_api_key() { Ok(k) => k, Err(_) => String::new() };
                        let audiodb_url = if key.is_empty() {
                            String::new()
                        } else {
                            format!("https://www.theaudiodb.com/api/v1/json/{}/search.php?s={}", key, urlencoding::encode(&artist_name))
                        };
                        if audiodb_url.is_empty() {
                            eprintln!("AUDIODB_API_KEY not set, skipping AudioDB artist fetch for {}", artist_name);
                            return Ok(false) as Result<bool, anyhow::Error>;
                        }
                        let audiodb_response = client.get(&audiodb_url).send().await?;

                        if audiodb_response.status().is_success() {
                            let audiodb_json: serde_json::Value = audiodb_response.json().await?;
                            if let Some(artists) = audiodb_json["artists"].as_array() {
                                if let Some(artist) = artists.first() {
                                    if let Some(image_url) = artist["strArtistThumb"].as_str() {
                                        let image_response = reqwest::get(image_url).await?;
                                        let image_content = image_response.bytes().await?;
                                        fs::write(&output_file, &image_content)?;
                                        eprintln!("Artist image fetched from AudioDB for: {} (album artist)", artist_name);
                                        return Ok(true) as Result<bool, anyhow::Error>; // Indicate success
                                    }
                                }
                            }
                        } else {
                            eprintln!("Error searching AudioDB for artist {}: {}", artist_name, audiodb_response.status());
                        }
                        Ok(false) as Result<bool, anyhow::Error> // Indicate failure
                    })?;

                    if !audiodb_fetch_successful {
                        // If AudioDB failed, check for existing folder.jpg
                        let folder_jpg_path = artist_path.join("folder.jpg");
                        if folder_jpg_path.exists() {
                            fs::copy(&folder_jpg_path, &output_file)?;
                            eprintln!(
                                "Copied {} to {}",
                                folder_jpg_path.display(),
                                output_file.display()
                            );
                        }
                    }
                } else {
                    eprintln!(
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
                            eprintln!("Album art extracted to {}", output_file.display());
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
        eprintln!("Fetching placeholder for {}: {}", name, path.display());

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
        if let Err(_) = key {
            eprintln!(
                "PEXELS_API_KEY not set, skipping placeholder fetch for {}",
                name
            );
            return Ok(());
        }
        let response = client
            .get(&url)
            .header("Authorization", key.unwrap())
            .send()
            .await?;

        if response.status().is_success() {
            let search_result = response.json::<PexelsSearchResponse>().await?;
            if let Some(photo) = search_result.photos.first() {
                let image_url = &photo.src.large;
                let image_response = reqwest::get(image_url).await?;
                let image_content = image_response.bytes().await?;
                fs::write(&placeholder_path, &image_content)?;
                eprintln!(
                    "Placeholder fetched for {}: {} (searched by album artist)",
                    name,
                    path.display()
                );
            } else {
                eprintln!(
                    "No image found for {}: {} (searched by album artist)",
                    name,
                    path.display()
                );
            }
        } else {
            eprintln!(
                "Error searching Pexels for {}: {}: {}",
                name,
                path.display(),
                response.status()
            );
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    Ok(())
}

pub async fn fetch_placeholders(music_dir: &str) -> Result<()> {
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
    eprintln!("Image cropped: {}", image_path.display());

    Ok(())
}
