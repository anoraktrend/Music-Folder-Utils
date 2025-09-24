use anyhow::{Context, Result};
use std::path::Path;
use std::sync::mpsc;
use audiotags::{Tag, Album};
use walkdir::WalkDir;
use musicbrainz_rs::{
    entity::release::Release,
    prelude::*,
    MusicBrainzClient,
};
use std::collections::HashMap;

pub async fn process_single_album_sync_tags(album_path: &Path, tx: mpsc::Sender<String>) -> Result<()> {
    let artist_path = album_path.parent().context("Album path has no parent")?;
    let artist_name = artist_path.file_name().unwrap().to_str().unwrap();
    let album_name = album_path.file_name().unwrap().to_str().unwrap();

    tx.send(format!("Syncing: {} - {}", artist_name, album_name))
        .context("Failed to send album name to TUI")?;

    // Set up MusicBrainz client with proper user agent
    let mut client = MusicBrainzClient::default();
    client.set_user_agent("mfutil/0.1.1 ( https://github.com/anoraktrend/music-folder-utils )")
        .context("Failed to set user agent")?;

    // Build query using the musicbrainz_rs builder
    let query = musicbrainz_rs::entity::release::ReleaseSearchQuery::query_builder()
        .release(album_name)
        .and()
        .artist(artist_name)
        .build();

    // Log the query for debugging
    tx.send(format!("Searching with query: {}", &query))
        .context("Failed to send query debug message to TUI")?;

    // Search for the release using the configured client
    let search_result = Release::search(query)
        .execute_with_client(&client)
        .await
        .context(format!("Failed to search MusicBrainz for {} - {}", artist_name, album_name))?;

    if let Some(first) = search_result.entities.first() {
        tx.send(format!("Found release: {}", first.title))
            .context("Failed to send release found message to TUI")?;

        // Fetch full release details including recordings
        let release = Release::fetch()
            .id(&first.id)
            .with_artists()
            .with_recordings()
            .with_media()
            .execute()
            .await
            .context("Failed to fetch release details")?;

        // Extract artist name from credits
        let artist_name = release
            .artist_credit
            .as_ref()
            .and_then(|ac| ac.first().map(|a| a.name.clone()))
            .unwrap_or_else(|| "Unknown Artist".to_string());

        // Build track mapping
        let mut track_info = HashMap::new();
        if let Some(media) = release.media {
            for medium in media {
                if let Some(tracks) = medium.tracks {
                    for track in tracks {
                        if let Some(recording) = track.recording {
                            track_info.insert(track.position, (track.number, recording.title));
                        }
                    }
                }
            }
        }

        tx.send(format!("Found {} tracks", track_info.len()))
            .context("Failed to send track count message to TUI")?;

        // Update audio files
        for entry in WalkDir::new(album_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| {
                matches!(ext.to_str().unwrap_or("").to_lowercase().as_str(), 
                    "mp3" | "flac" | "ogg" | "m4a" | "wav")
            }) {
                let mut tag = Tag::new().read_from_path(path)?;
                let file_name = path.file_name().unwrap().to_string_lossy();

                tag.set_artist(&artist_name);
                tag.set_album(Album::with_title(&release.title));

                // Try to match track by number
                if let Some(track_num) = tag.track_number() {
                    if let Some((number, title)) = track_info.get(&(track_num as u32)) {
                        tag.set_title(title);
                        tx.send(format!("Matched track {} - {}", number, title))
                            .context("Failed to send track match message to TUI")?;
                    } else {
                        tx.send(format!("No match found for track {} ({})", track_num, file_name))
                            .context("Failed to send no match message to TUI")?;
                    }
                }

                tag.write_to_path(path.to_str().unwrap())?;
            }
        }

        tx.send(format!("Successfully synchronized {} - {}", artist_name, release.title))
            .context("Failed to send success message to TUI")?;
    } else {
        tx.send(format!("No release found for {} - {}", artist_name, album_name))
            .context("Failed to send no release message to TUI")?;
    }

    // Rate limiting is handled automatically by musicbrainz_rs with the rate_limit feature
    Ok(())
}
