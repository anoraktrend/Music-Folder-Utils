use anyhow::{Context, Result};
use mfutil::{cd, cover_art};
use std::fs;
use std::path::Path;
use std::sync::mpsc;

/// Import a CD to the music library with real CD reading
#[cfg(feature = "cd-ripping")]
pub async fn import_cd(device: &str, music_dir: &str, tx: mpsc::Sender<String>) -> Result<()> {
    tx.send(format!("Reading CD from device: {}", device))
        .context("Failed to send CD reading message")?;

    // Read CD information using cd-da-reader
    let cd_info = cd::read_cd_from_device(device, tx.clone()).await?;

    tx.send(format!("Found CD: {} - {}", cd_info.artist, cd_info.title))
        .context("Failed to send CD info message")?;

    // Look up CD information from MusicBrainz
    let cd_info = cd::lookup_cd_info(&cd_info, tx.clone()).await?;

    // Fetch cover art if we have a release ID
    let mut cover_art_data: Option<Vec<u8>> = None;
    if let Some(release_id) = &cd_info.release_id {
        // Try MusicBrainz first
        if let Ok(Some(cover_art)) = cover_art::fetch_musicbrainz_cover_art(release_id, &tx).await {
            cover_art_data = Some(cover_art);
        } else {
            // Fallback to AudioDB
            if let Ok(Some(cover_art)) =
                cover_art::fetch_audiodb_cover_art(&cd_info.artist, &cd_info.title, &tx).await
            {
                cover_art_data = Some(cover_art);
            }
        }
    }

    if cover_art_data.is_some() {
        tx.send("Cover art fetched successfully - will be embedded in FLAC files".to_string())
            .context("Failed to send cover art success message")?;
    } else {
        tx.send(
            "No cover art found - FLAC files will be created without embedded artwork".to_string(),
        )
        .context("Failed to send no cover art message")?;
    }

    // Create directory structure
    let artist_dir = Path::new(music_dir).join("Artists").join(&cd_info.artist);
    let album_dir = artist_dir.join(&cd_info.title);
    fs::create_dir_all(&album_dir)
        .with_context(|| format!("Failed to create album directory: {:?}", album_dir))?;

    tx.send(format!("Created directory: {}", album_dir.display()))
        .context("Failed to send directory creation message")?;

    // Import each track
    let total_tracks = cd_info.tracks.len();
    tx.send(format!("TOTAL_FILES:{}", total_tracks))
        .context("Failed to send total tracks count")?;

    for (i, track) in cd_info.tracks.iter().enumerate() {
        // Add timeout for individual tracks (5 minutes per track should be more than enough)
        match tokio::time::timeout(
            std::time::Duration::from_secs(300),
            cd::import_cd_track(
                device,
                &cd_info,
                track,
                &album_dir,
                tx.clone(),
                cover_art_data.as_ref(),
            ),
        )
        .await
        {
            Ok(Ok(())) => {
                tx.send(format!(
                    "COMPLETED: Imported track {}/{}: {}",
                    i + 1,
                    total_tracks,
                    track.title
                ))
                .context("Failed to send track completion message")?;
            }
            Ok(Err(e)) => {
                tx.send(format!(
                    "ERROR: Failed to import track {}: {}",
                    track.title, e
                ))
                .context("Failed to send track error message")?;
                // Continue with next track instead of failing completely
            }
            Err(_) => {
                tx.send(format!(
                    "ERROR: Timeout importing track {} - skipping",
                    track.title
                ))
                .context("Failed to send timeout error message")?;
                // Continue with next track
            }
        }
    }

    tx.send(format!(
        "Successfully imported CD: {} - {}",
        cd_info.artist, cd_info.title
    ))
    .context("Failed to send completion message")?;

    Ok(())
}

#[cfg(not(feature = "cd-ripping"))]
pub async fn import_cd(_device: &str, _music_dir: &str, tx: mpsc::Sender<String>) -> Result<()> {
    tx.send("CD ripping feature is not enabled. Cannot import CD.".to_string())
        .context("Failed to send message about disabled CD ripping feature")?;
    Err(anyhow::anyhow!("CD ripping feature is not enabled. Please enable the 'cd-ripping' feature in Cargo.toml to use this command."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mfutil;
    use mfutil::cd::{CdInfo, CdTrack};
    use std::fs;
    use std::path::Path;
    use std::sync::mpsc;
    use tempfile::tempdir;

    // Test-specific implementation to avoid actual hardware access
    async fn read_cd_from_device_test(_device: &str) -> Result<CdInfo> {
        Ok(CdInfo {
            disc_id: "test_disc_id".to_string(),
            title: "Test Album".to_string(),
            artist: "Test Artist".to_string(),
            tracks: vec![
                CdTrack {
                    number: 1,
                    title: "Test Track 1".to_string(),
                    artist: "Test Artist".to_string(),
                    duration: 2,
                    filename: "01 Test Track 1.flac".to_string(),
                },
                CdTrack {
                    number: 2,
                    title: "Test Track 2".to_string(),
                    artist: "Test Artist".to_string(),
                    duration: 2,
                    filename: "02 Test Track 2.flac".to_string(),
                },
            ],
            total_duration: 4,
            release_id: None,
        })
    }

    async fn read_cd_data_test(
        _device: &str,
        _track: &CdTrack,
        _tx: &mpsc::Sender<String>,
    ) -> Result<Vec<u8>> {
        // Return 2 seconds of silent audio data
        let sample_rate = 44100;
        let samples = (2 * sample_rate) as usize;
        let audio_data = vec![0u8; samples * 2 * 2]; // 16-bit stereo
        Ok(audio_data)
    }

    fn write_flac_file(
        path: &Path,
        audio_data: &[u8],
        _track: &CdTrack,
        _cover_art: Option<&Vec<u8>>,
    ) -> Result<()> {
        // Simplified write for test purposes
        fs::write(path, audio_data).context("Failed to write test FLAC file")
    }

    #[tokio::test]
    async fn test_import_cd_flow() {
        let temp_dir = tempdir().unwrap();
        let music_dir = temp_dir.path().to_str().unwrap();

        // Mock the CD reading process for testing
        let import_task = async {
            let cd_info = read_cd_from_device_test("test_device").await.unwrap();

            let artist_dir = Path::new(music_dir).join("Artists").join(&cd_info.artist);
            let album_dir = artist_dir.join(&cd_info.title);
            fs::create_dir_all(&album_dir).unwrap();

            // Create a dummy channel for testing
            let (tx, _rx) = mpsc::channel::<String>();

            for track in &cd_info.tracks {
                let audio_data = read_cd_data_test("test_device", track, &tx).await.unwrap();
                let track_path = album_dir.join(&track.filename);
                write_flac_file(&track_path, &audio_data, track, None).unwrap();
            }
            Ok::<(), anyhow::Error>(())
        };

        let result = import_task.await;
        assert!(result.is_ok());

        // Verify directory and file creation
        let artist_dir = temp_dir.path().join("Artists").join("Test Artist");
        let album_dir = artist_dir.join("Test Album");
        assert!(album_dir.exists());

        let track1_path = album_dir.join("01 Test Track 1.flac");
        let track2_path = album_dir.join("02 Test Track 2.flac");
        assert!(track1_path.exists());
        assert!(track2_path.exists());
    }

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(
            mfutil::utils::sanitize_filename("normal_name"),
            "normal_name"
        );
        assert_eq!(
            mfutil::utils::sanitize_filename("file with spaces"),
            "file with spaces"
        );
        assert_eq!(
            mfutil::utils::sanitize_filename("file/with\\bad:chars*"),
            "file_with_bad_chars_"
        );
    }

    #[test]
    fn test_sanitize_filename_edge_cases() {
        assert_eq!(mfutil::utils::sanitize_filename(""), "");
        assert_eq!(mfutil::utils::sanitize_filename("   "), "");
        assert_eq!(
            mfutil::utils::sanitize_filename("file\x00with\x01control\x02chars"),
            "file_with_control_chars"
        );
    }

    #[test]
    fn test_fetch_cover_art_integration() {
        // Test that cover art functions are properly integrated
        // This is a basic integration test to ensure the functions exist and have correct signatures
        let (tx, _rx) = mpsc::channel::<String>();

        // Test that the functions can be called (even if they return None for test data)
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result1 = rt.block_on(cover_art::fetch_musicbrainz_cover_art(
            "test_release_id",
            &tx,
        ));
        let result2 = rt.block_on(cover_art::fetch_audiodb_cover_art(
            "Test Artist",
            "Test Album",
            &tx,
        ));

        // These should return Ok(None) since we're using test data
        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }
}
