use anyhow::{Context, Result};
use std::path::Path;
use std::sync::mpsc;
use reqwest;
use serde_json;
use urlencoding;

/// Fetch cover art from MusicBrainz Cover Art Archive
pub async fn fetch_musicbrainz_cover_art(release_id: &str, tx: &mpsc::Sender<String>) -> Result<Option<Vec<u8>>> {
    tx.send(format!("Fetching cover art from MusicBrainz for release: {}", release_id))
        .context("Failed to send cover art fetch message")?;

    let cover_art_url = format!("https://coverartarchive.org/release/{}/front", release_id);
    let client = reqwest::Client::new();

    match client.get(&cover_art_url)
        .header("User-Agent", "mfutil/0.1.1 (https://github.com/anoraktrend/music-folder-utils)")
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.bytes().await {
                    Ok(image_data) => {
                        tx.send("Successfully fetched cover art from MusicBrainz".to_string())
                            .context("Failed to send cover art success message")?;
                        Ok(Some(image_data.to_vec()))
                    }
                    Err(e) => {
                        tx.send(format!("Failed to read cover art data: {}", e))
                            .context("Failed to send cover art data error")?;
                        Ok(None)
                    }
                }
            } else {
                tx.send(format!("Cover art not available from MusicBrainz (status: {})", response.status()))
                    .context("Failed to send cover art unavailable message")?;
                Ok(None)
            }
        }
        Err(e) => {
            tx.send(format!("Failed to fetch cover art from MusicBrainz: {}", e))
                .context("Failed to send cover art fetch error")?;
            Ok(None)
        }
    }
}

/// Fetch cover art from AudioDB as fallback
pub async fn fetch_audiodb_cover_art(artist: &str, album: &str, tx: &mpsc::Sender<String>) -> Result<Option<Vec<u8>>> {
    tx.send(format!("Trying AudioDB for cover art: {} - {}", artist, album))
        .context("Failed to send AudioDB cover art message")?;

    let encoded_artist = urlencoding::encode(artist);
    let encoded_album = urlencoding::encode(album);
    let audiodb_url = format!(
        "https://www.theaudiodb.com/api/v1/json/2/searchalbum.php?s={}&a={}",
        encoded_artist, encoded_album
    );

    let client = reqwest::Client::new();

    match client.get(&audiodb_url)
        .header("User-Agent", "mfutil/0.1.1 (https://github.com/anoraktrend/music-folder-utils)")
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(json_data) => {
                        if let Some(albums) = json_data.get("album") {
                            if let Some(albums_array) = albums.as_array() {
                                if let Some(first_album) = albums_array.first() {
                                    if let Some(thumbnail_url) = first_album.get("strAlbumThumb") {
                                        if let Some(url_str) = thumbnail_url.as_str() {
                                            if !url_str.is_empty() && url_str != "null" {
                                                match client.get(url_str)
                                                    .header("User-Agent", "mfutil/0.1.1 (https://github.com/anoraktrend/music-folder-utils)")
                                                    .send()
                                                    .await
                                                {
                                                    Ok(image_response) => {
                                                        if image_response.status().is_success() {
                                                            match image_response.bytes().await {
                                                                Ok(image_data) => {
                                                                    tx.send("Successfully fetched cover art from AudioDB".to_string())
                                                                        .context("Failed to send AudioDB success message")?;
                                                                    Ok(Some(image_data.to_vec()))
                                                                }
                                                                Err(e) => {
                                                                    tx.send(format!("Failed to download AudioDB cover art: {}", e))
                                                                        .context("Failed to send AudioDB download error")?;
                                                                    Ok(None)
                                                                }
                                                            }
                                                        } else {
                                                            tx.send("AudioDB cover art download failed".to_string())
                                                                .context("Failed to send AudioDB download failed")?;
                                                            Ok(None)
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tx.send(format!("Failed to fetch from AudioDB URL: {}", e))
                                                            .context("Failed to send AudioDB URL error")?;
                                                        Ok(None)
                                                    }
                                                }
                                            } else {
                                                tx.send("No cover art URL found in AudioDB response".to_string())
                                                    .context("Failed to send no AudioDB URL message")?;
                                                Ok(None)
                                            }
                                        } else {
                                            tx.send("No cover art URL found in AudioDB response".to_string())
                                                .context("Failed to send no AudioDB URL message")?;
                                            Ok(None)
                                        }
                                    } else {
                                        tx.send("No cover art found in AudioDB response".to_string())
                                            .context("Failed to send no AudioDB cover art")?;
                                        Ok(None)
                                    }
                                } else {
                                    tx.send("No albums found in AudioDB response".to_string())
                                        .context("Failed to send no AudioDB albums")?;
                                    Ok(None)
                                }
                            } else {
                                tx.send("Invalid AudioDB response format".to_string())
                                    .context("Failed to send invalid AudioDB format")?;
                                Ok(None)
                            }
                        } else {
                            tx.send("No album data in AudioDB response".to_string())
                                .context("Failed to send no AudioDB album data")?;
                            Ok(None)
                        }
                    }
                    Err(e) => {
                        tx.send(format!("Failed to parse AudioDB response: {}", e))
                            .context("Failed to send AudioDB parse error")?;
                        Ok(None)
                    }
                }
            } else {
                tx.send(format!("AudioDB request failed (status: {})", response.status()))
                    .context("Failed to send AudioDB request failed")?;
                Ok(None)
            }
        }
        Err(e) => {
            tx.send(format!("Failed to fetch from AudioDB: {}", e))
                .context("Failed to send AudioDB fetch error")?;
            Ok(None)
        }
    }
}

/// Save cover art to album directory
pub async fn save_cover_art_to_album(
    album_path: &Path,
    release_id: &str,
    artist: &str,
    album: &str,
    tx: &mpsc::Sender<String>,
) -> Result<()> {
    // Try MusicBrainz first
    if let Ok(Some(cover_art)) = fetch_musicbrainz_cover_art(release_id, tx).await {
        let cover_art_path = album_path.join("cover.jpg");
        if let Err(e) = std::fs::write(&cover_art_path, &cover_art) {
            tracing::warn!("Failed to save MusicBrainz cover art to {:?}: {}", cover_art_path, e);
            // Try AudioDB as fallback
            if let Ok(Some(audiodb_cover_art)) = fetch_audiodb_cover_art(artist, album, tx).await {
                if let Err(e) = std::fs::write(&cover_art_path, &audiodb_cover_art) {
                    tracing::warn!("Failed to save AudioDB cover art to {:?}: {}", cover_art_path, e);
                } else {
                    tx.send(format!("Saved AudioDB cover art to: {}", cover_art_path.display()))
                        .context("Failed to send AudioDB cover art save message")?;
                }
            }
        } else {
            tx.send(format!("Saved MusicBrainz cover art to: {}", cover_art_path.display()))
                .context("Failed to send MusicBrainz cover art save message")?;
        }
    } else {
        // Try AudioDB as fallback
        if let Ok(Some(cover_art)) = fetch_audiodb_cover_art(artist, album, tx).await {
            let cover_art_path = album_path.join("cover.jpg");
            if let Err(e) = std::fs::write(&cover_art_path, &cover_art) {
                tracing::warn!("Failed to save AudioDB cover art to {:?}: {}", cover_art_path, e);
            } else {
                tx.send(format!("Saved AudioDB cover art to: {}", cover_art_path.display()))
                    .context("Failed to send AudioDB cover art save message")?;
            }
        } else {
            tx.send("No cover art found from any source".to_string())
                .context("Failed to send no cover art message")?;
        }
    }

    Ok(())
}