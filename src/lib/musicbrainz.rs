use anyhow::{Context, Result};
use musicbrainz_rs::{entity::release::Release, prelude::*, MusicBrainzClient};
use std::path::Path;
use std::sync::mpsc;
use tracing::warn;

/// Create and configure a MusicBrainz client with the standard user agent
pub fn create_musicbrainz_client() -> Result<MusicBrainzClient> {
    let mut client = MusicBrainzClient::default();
    client
        .set_user_agent("mfutil/0.1.1 (https://github.com/anoraktrend/music-folder-utils)")
        .context("Failed to set user agent")?;
    Ok(client)
}

/// Look up release information from MusicBrainz
pub async fn lookup_musicbrainz_release(
    artist: &str,
    album: &str,
    tx: &mpsc::Sender<String>,
) -> Result<Option<(String, String, String)>> {
    tx.send(format!(
        "Looking up MusicBrainz release: {} - {}",
        artist, album
    ))
    .context("Failed to send MusicBrainz lookup message")?;

    let client = create_musicbrainz_client()?;

    // Search for releases by artist and album
    let query = musicbrainz_rs::entity::release::ReleaseSearchQuery::query_builder()
        .release(album)
        .and()
        .artist(artist)
        .build();

    match Release::search(query).execute_with_client(&client).await {
        Ok(search_result) => {
            if let Some(release) = search_result.entities.into_iter().next() {
                let artist_credit = release
                    .artist_credit
                    .as_ref()
                    .map(|credits| {
                        credits
                            .iter()
                            .map(|c| c.name.clone())
                            .collect::<Vec<_>>()
                            .join(" & ")
                    })
                    .unwrap_or_else(|| artist.to_string());

                tx.send(format!(
                    "Found MusicBrainz release: {} - {} ({})",
                    artist_credit, release.title, release.id
                ))
                .context("Failed to send release found message")?;

                Ok(Some((artist_credit, release.title, release.id)))
            } else {
                tx.send(format!(
                    "No MusicBrainz release found for {} - {}",
                    artist, album
                ))
                .context("Failed to send no release message")?;
                Ok(None)
            }
        }
        Err(e) => {
            warn!("MusicBrainz search failed: {:?}", e);
            Err(anyhow::anyhow!("MusicBrainz search failed: {:?}", e))
        }
    }
}

/// Enhanced metadata extraction with MusicBrainz lookup
pub async fn extract_and_enhance_metadata(
    file_path: &Path,
    tx: &mpsc::Sender<String>,
) -> Result<(String, String, Option<String>)> {
    // First try to extract from file metadata
    let (artist, album) = super::metadata::extract_artist_album_from_file(file_path)?;

    // If we have basic metadata, try to enhance it with MusicBrainz
    if artist != "Unknown Artist" && album != "Unknown Album" {
        match lookup_musicbrainz_release(&artist, &album, tx).await {
            Ok(Some((enhanced_artist, enhanced_album, release_id))) => {
                tx.send(format!(
                    "Enhanced metadata for {}: '{}' -> '{}' / '{}' -> '{}'",
                    file_path.display(),
                    &artist,
                    &enhanced_artist,
                    &album,
                    &enhanced_album
                ))
                .context("Failed to send enhancement message")?;
                return Ok((enhanced_artist, enhanced_album, Some(release_id)));
            }
            Ok(None) => {
                // No enhancement available, use original metadata
                tx.send(format!(
                    "No MusicBrainz match found for {} - {} (using original metadata)",
                    &artist, &album
                ))
                .context("Failed to send no match message")?;
            }
            Err(e) => {
                warn!(
                    "MusicBrainz lookup failed for {} - {}: {:?}",
                    &artist, &album, e
                );
            }
        }
    }

    Ok((artist.to_string(), album.to_string(), None))
}
