use anyhow::{Context, Result};
use std::path::Path;
use serde::Deserialize;
use reqwest;

#[derive(Deserialize, Debug)]
pub struct ArtistCredit {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct Release {
    pub id: String,
    pub title: String,
    #[serde(rename = "artist-credit")]
    pub artist_credit: Vec<ArtistCredit>,
}

#[derive(Deserialize, Debug)]
pub struct ReleaseSearchResult {
    pub releases: Vec<Release>,
}

pub async fn process_single_album_sync_tags(album_path: &Path) -> Result<()> {
    let artist_path = album_path.parent().context("Album path has no parent")?;
    let artist_name = artist_path.file_name().unwrap().to_str().unwrap();
    let album_name = album_path.file_name().unwrap().to_str().unwrap();
    println!("Searching for artist: {}, album: {}", artist_name, album_name);

    let client = reqwest::Client::new();
    let query = format!("artist:\"{}\" AND release:\"{}\"", artist_name, album_name);
    let url = format!("https://musicbrainz.org/ws/2/release?query={}&fmt=json", query);

    let response = client.get(&url)
        .header("User-Agent", "mfutil/0.1.0 (https://github.com/your-username/music-folder-utils)")
        .send()
        .await?;

    if response.status().is_success() {
        let search_result = response.json::<ReleaseSearchResult>().await?;
        if let Some(release) = search_result.releases.first() {
            println!("Found release: {} - {}", release.artist_credit.first().map_or("Unknown Artist", |a| &a.name), release.title);
            println!("MusicBrainz ID: {}", release.id);
        } else {
            println!("No release found for {} - {}", artist_name, album_name);
        }
    } else {
        println!("Error searching for release: {}", response.status());
    }
    std::thread::sleep(std::time::Duration::from_secs(1));
    Ok(())
}

pub async fn sync_tags(_music_dir: &str) -> Result<()> {
    // This function is now handled by run_tui and process_single_album_sync_tags
    Ok(())
}
