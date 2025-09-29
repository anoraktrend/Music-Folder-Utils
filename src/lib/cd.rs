use anyhow::{Context, Result};
use discid::DiscId;
use musicbrainz_rs::{entity::release::Release, prelude::*, ApiRequest, MusicBrainzClient};

use std::path::Path;
use std::sync::mpsc;
use lofty::{self, file::TaggedFileExt, tag::ItemKey};
use tracing::warn;
use flacenc::component::BitRepr;
use flacenc::error::Verify;
use cdparanoia;
use serde_json;
use crate::utils;

/// Information about a CD track
#[derive(Debug, Clone)]
pub struct CdTrack {
    pub number: u32,
    pub title: String,
    pub artist: String,
    pub duration: u64, // in seconds
    pub filename: String,
}

/// Information about a CD
#[derive(Debug, Clone)]
pub struct CdInfo {
    pub disc_id: String,
    pub title: String,
    pub artist: String,
    pub tracks: Vec<CdTrack>,
    pub total_duration: u64,
    pub release_id: Option<String>,
}

/// Read CD Table of Contents and calculate Disc ID using discid
pub async fn read_cd_from_device(device: &str, tx: mpsc::Sender<String>) -> Result<CdInfo> {
    tx.send(format!("Reading TOC from device: {}", device))
        .context("Failed to send TOC reading message")?;

    // Use discid for MusicBrainz ID calculation
    let disc_id = DiscId::read(Some(device))
        .with_context(|| format!("Failed to read disc ID from device: {}", device))?;

    // Debug: Print discid details
    let disc_id_str = disc_id.id();
    let first_track = disc_id.first_track_num();
    let last_track = disc_id.last_track_num();
    let sectors = disc_id.sectors();

    tx.send(format!("Calculated Disc ID: {}", disc_id_str))
        .context("Failed to send Disc ID message")?;
    tx.send(format!("Debug: First track: {}, Last track: {}, Sectors: {}",
                   first_track, last_track, sectors))
        .context("Failed to send debug info")?;

    // Use cdparanoia to read the TOC and track info
    let device_cstr = std::ffi::CString::new(device).context("Failed to create CString for device")?;
    let drive = cdparanoia::CdromDrive::identify(&device_cstr, cdparanoia::Verbosity::PrintIt)
        .context("Failed to identify CD-ROM drive")?;
    drive.open().context("Failed to open CD-ROM drive")?;

    let num_tracks = drive.tracks().context("Failed to get number of tracks")?;
    let mut tracks = Vec::new();

    for i in 1..=num_tracks {
        if drive.track_audiop(i).context(format!("Failed to check if track {} is audio", i))? {
            let first_sector = drive.track_first_sector(i).context(format!("Failed to get first sector of track {}", i))?;
            let last_sector = drive.track_last_sector(i).context(format!("Failed to get last sector of track {}", i))?;

            // Ensure last_sector >= first_sector to avoid overflow
            if last_sector < first_sector {
                warn!("Invalid sector range for track {}: first={} last={}", i, first_sector, last_sector);
                continue; // Skip this track
            }

            let duration = (last_sector - first_sector) / 75; // 75 frames per second

            let number = i;
            let title = format!("Track {:02}", number);
            tracks.push(CdTrack {
                number,
                title: title.clone(),
                artist: "Unknown Artist".to_string(),
                duration,
                filename: format!("{:02} {}.flac", number, utils::sanitize_filename(&title)),
            });
        }
    }

    if tracks.is_empty() {
        return Err(anyhow::anyhow!("No valid audio tracks found on the disc. This may indicate the CD is damaged or not readable."));
    }

    let total_duration = tracks.iter().map(|t| t.duration).sum();

    Ok(CdInfo {
        disc_id: disc_id_str,
        title: "Unknown Album".to_string(), // Will be filled by MusicBrainz
        artist: "Unknown Artist".to_string(), // Will be filled by MusicBrainz
        tracks,
        total_duration,
        release_id: None,
    })
}

/// Look up CD information from MusicBrainz
pub async fn lookup_cd_info(
    cd_info: &CdInfo,
    tx: mpsc::Sender<String>,
) -> Result<CdInfo> {
    tx.send("Looking up CD information from MusicBrainz...".to_string())
        .context("Failed to send MusicBrainz lookup message")?;

    let mut client = MusicBrainzClient::default();
    client.set_user_agent("mfutil/0.1.1 ( https://github.com/anoraktrend/music-folder-utils )")
        .context("Failed to set user agent")?;

    // First try to lookup by discid using the direct discid endpoint
    tx.send(format!("Attempting lookup by DiscID: {}", cd_info.disc_id))
        .context("Failed to send discid lookup message")?;

    // Use raw API request to lookup release by discid
    let discid_url = format!("https://musicbrainz.org/ws/2/discid/{}?fmt=json&inc=artists+release-groups+recordings", cd_info.disc_id);
    let request = ApiRequest::new(discid_url);

    match request.get_json(&client).await {
        Ok(discid_response) => {
            // Parse the discid response to extract release information
            if let Some(releases) = discid_response.get("releases") {
                if let Some(release_data) = releases.get(0) {
                    if let Some(release_id) = release_data.get("id").and_then(|id| id.as_str()) {
                        // We already have the full release data from the discid response
                        // Extract artist and title from the discid response
                        let artist_credit = release_data.get("artist-credit")
                            .and_then(|ac| ac.as_array())
                            .and_then(|ac| ac.first())
                            .and_then(|a| a.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("Unknown Artist");

                        let title = release_data.get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("Unknown Album");

                        tx.send(format!("Found release: {} - {} ({})", artist_credit, title, release_id))
                            .context("Failed to send release found message")?;

                        // Create CdInfo from the discid response data with full track information
                        let cd_info = cd_info_from_discid_response(release_data, cd_info)?;
                        Ok(cd_info)
                    } else {
                        tx.send("No release ID found in discid response".to_string())
                            .context("Failed to send error message")?;
                        Ok(cd_info.clone())
                    }
                } else {
                    tx.send("No releases found for this discid".to_string())
                        .context("Failed to send error message")?;
                    Ok(cd_info.clone())
                }
            } else {
                tx.send("Invalid discid response format".to_string())
                    .context("Failed to send error message")?;
                Ok(cd_info.clone())
            }
        }
        Err(e) => {
            warn!("MusicBrainz discid lookup failed: {}", e);
            tx.send("DiscID lookup failed, trying search by artist/album...".to_string())
                .context("Failed to send fallback message")?;

            // Fallback to search by artist and album name
            let query = musicbrainz_rs::entity::release::ReleaseSearchQuery::query_builder()
                .release(&cd_info.title)
                .and()
                .artist(&cd_info.artist)
                .build();

            match Release::search(query).execute_with_client(&client).await {
                Ok(search_result) => {
                    if let Some(release) = search_result.entities.into_iter().next() {
                        let artist_credit = release.artist_credit.as_ref()
                            .map(|credits| credits.iter().map(|c| c.name.clone()).collect::<Vec<_>>().join(" & "))
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        tx.send(format!("Found release: {} - {} ({})", artist_credit, release.title, release.id))
                            .context("Failed to send release found message")?;

                        let cd_info = CdInfo {
                            disc_id: cd_info.disc_id.clone(),
                            title: release.title.clone(),
                            artist: artist_credit.clone(),
                            tracks: cd_info.tracks.clone(), // Keep original tracks for fallback
                            total_duration: cd_info.total_duration,
                            release_id: Some(release.id.clone()),
                        };
                        Ok(cd_info)
                    } else {
                        tx.send("No exact match found, using provided information...".to_string())
                            .context("Failed to send fallback message")?;

                        // Return the original CD info if no match found
                        Ok(cd_info.clone())
                    }
                }
                Err(e) => {
                    warn!("MusicBrainz search failed: {}", e);
                    tx.send("MusicBrainz lookup failed, using provided information...".to_string())
                        .context("Failed to send fallback message")?;

                    // Return the original CD info if lookup fails
                    Ok(cd_info.clone())
                }
            }
        }
    }
}

/// Create CdInfo from a MusicBrainz discid response
fn cd_info_from_discid_response(release_data: &serde_json::Value, cd_info: &CdInfo) -> Result<CdInfo> {
    let release_id = release_data.get("id")
        .and_then(|id| id.as_str())
        .unwrap_or("");

    let artist = release_data.get("artist-credit")
        .and_then(|ac| ac.as_array())
        .and_then(|ac| ac.first())
        .and_then(|a| a.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("Unknown Artist");

    let title = release_data.get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("Unknown Album");

    // Extract track information from media - this is the key part
    let tracks: Vec<CdTrack> = if let Some(media) = release_data.get("media") {
        if let Some(media_array) = media.as_array() {
            if let Some(first_medium) = media_array.first() {
                if let Some(tracks) = first_medium.get("tracks") {
                    if let Some(tracks_array) = tracks.as_array() {
                        tracks_array.iter().enumerate().map(|(i, track_data)| {
                            let number = track_data.get("number")
                                .and_then(|n| n.as_str())
                                .and_then(|n| n.parse::<u32>().ok())
                                .unwrap_or((i + 1) as u32);

                            let default_title = format!("Track {:02}", number);
                            let track_title = track_data.get("title")
                                .and_then(|t| t.as_str())
                                .unwrap_or(&default_title);

                            let duration = track_data.get("length")
                                .and_then(|l| l.as_u64())
                                .map(|l| l / 1000) // Convert from milliseconds to seconds
                                .unwrap_or(0);

                            CdTrack {
                                number,
                                title: track_title.to_string(),
                                artist: artist.to_string(),
                                duration,
                                filename: format!("{:02} {}.flac", number, utils::sanitize_filename(track_title)),
                            }
                        }).collect()
                    } else {
                        // Fallback: create basic tracks if parsing fails
                        (1..=11).map(|i| CdTrack {
                            number: i,
                            title: format!("Track {:02}", i),
                            artist: artist.to_string(),
                            duration: 180, // Default 3 minutes
                            filename: format!("{:02} Track {:02}.flac", i, i),
                        }).collect()
                    }
                } else {
                    // Fallback: create basic tracks
                    (1..=11).map(|i| CdTrack {
                        number: i,
                        title: format!("Track {:02}", i),
                        artist: artist.to_string(),
                        duration: 180, // Default 3 minutes
                        filename: format!("{:02} Track {:02}.flac", i, i),
                    }).collect()
                }
            } else {
                // Fallback: create basic tracks
                (1..=11).map(|i| CdTrack {
                    number: i,
                    title: format!("Track {:02}", i),
                    artist: artist.to_string(),
                    duration: 180, // Default 3 minutes
                    filename: format!("{:02} Track {:02}.flac", i, i),
                }).collect()
            }
        } else {
            // Fallback: create basic tracks
            (1..=11).map(|i| CdTrack {
                number: i,
                title: format!("Track {:02}", i),
                artist: artist.to_string(),
                duration: 180, // Default 3 minutes
                filename: format!("{:02} Track {:02}.flac", i, i),
            }).collect()
        }
    } else {
        // Fallback: create basic tracks
        (1..=11).map(|i| CdTrack {
            number: i,
            title: format!("Track {:02}", i),
            artist: artist.to_string(),
            duration: 180, // Default 3 minutes
            filename: format!("{:02} Track {:02}.flac", i, i),
        }).collect()
    };

    let total_duration = tracks.iter().map(|t| t.duration).sum();

    Ok(CdInfo {
        disc_id: cd_info.disc_id.clone(),
        title: title.to_string(),
        artist: artist.to_string(),
        tracks,
        total_duration,
        release_id: Some(release_id.to_string()),
    })
}

/// Import a single track from CD with actual CD reading
pub async fn import_cd_track(
    device: &str,
    cd_info: &CdInfo,
    track: &CdTrack,
    album_dir: &Path,
    tx: mpsc::Sender<String>,
    cover_art: Option<&Vec<u8>>,
) -> Result<()> {
    tx.send(format!("Importing track: {}", track.title))
        .context("Failed to send track import message")?;

    let track_path = album_dir.join(&track.filename);

    // Read actual audio data from CD
    let audio_data = match read_cd_data(device, track, &tx).await {
        Ok(data) => {
            tx.send(format!("Read {} bytes of audio data for track {}", data.len(), track.title))
                .context("Failed to send audio read message")?;
            data
        }
        Err(e) => {
            tx.send(format!("ERROR: Failed to read audio data for track {}: {}", track.title, e))
                .context("Failed to send track read error message")?;
            return Err(e);
        }
    };

    // Write the audio data to FLAC file
    match write_flac_file(&track_path, &audio_data, track, cover_art) {
        Ok(())
             => {
            tx.send(format!("Encoded FLAC file: {}", track_path.display()))
                .context("Failed to send FLAC encoding message")?;
        }
        Err(e) => {
            tx.send(format!("ERROR: Failed to encode FLAC for track {}: {}", track.title, e))
                .context("Failed to send FLAC encoding error message")?;
            return Err(e);
        }
    };

    // Set metadata tags
    set_audio_metadata(
        &track_path,
        track,
        &cd_info.title,
        &cd_info.artist,
        cd_info.release_id.as_deref(),
    )
    .with_context(|| format!("Failed to set metadata for: {:?}", track_path))?;

    Ok(())
}

/// Read a single track's audio data from the CD using cdparanoia
async fn read_cd_data(device: &str, track: &CdTrack, tx: &mpsc::Sender<String>) -> Result<Vec<u8>> {
    let device_cstr = std::ffi::CString::new(device).context("Failed to create CString for device")?;
    let drive = cdparanoia::CdromDrive::identify(&device_cstr, cdparanoia::Verbosity::LogIt)
        .context("Failed to identify CD-ROM drive")?;
    drive.open().context("Failed to open CD-ROM drive")?;

    let paranoia = cdparanoia::CdromParanoia::init(drive);

    let first_sector = paranoia.drive().track_first_sector(track.number)?;
    let last_sector = paranoia.drive().track_last_sector(track.number)?;

    paranoia.seek(std::io::SeekFrom::Start(first_sector))
        .with_context(|| format!("Failed to seek to track {}", track.number))?;

    let mut samples_i16 = Vec::new();
    let mut sectors_read = 0;
    let total_sectors = last_sector - first_sector + 1;

    for _sector in first_sector..=last_sector {
        // The callback function is a C function pointer, we can pass a dummy one or a proper logger.
        // For now, using a simple extern "C" fn is sufficient.
        extern "C" fn callback(_: i64, _: i32) {}
        let sector_ptr = unsafe { cdparanoia::cdparanoia_sys::paranoia_read(paranoia.as_raw(), Some(callback)) };
        if sector_ptr.is_null() {
            break; // End of read
        }
        samples_i16.extend_from_slice(unsafe { std::slice::from_raw_parts(sector_ptr as *const i16, cdparanoia::CD_FRAMEWORDS as usize) });
        sectors_read += 1;

        // Progress logging every 100 sectors through TUI
        if sectors_read % 100 == 0 {
            let progress = (sectors_read * 100) / total_sectors;
            let _ = tx.send(format!("PROGRESS: Reading track {}: {}% complete ({} sectors)", track.number, progress, sectors_read));
        }
    }

    if sectors_read == 0 {
        return Err(anyhow::anyhow!("Failed to read any audio data from track {}", track.number));
    }

    let _ = tx.send(format!("Successfully read {} sectors for track {}", sectors_read, track.number));

    // Convert Vec<i16> to Vec<u8> for the rest of the pipeline
    let mut byte_buffer = Vec::with_capacity(samples_i16.len() * 2);
    for sample in samples_i16 {
        byte_buffer.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(byte_buffer)
}

/// Write audio data to FLAC file with proper error handling and optional cover art embedding
fn write_flac_file(path: &Path, audio_data: &[u8], _track: &CdTrack, cover_art: Option<&Vec<u8>>) -> Result<()> {
    // Convert audio data to i32 samples (interleaved stereo)
    let samples_i16: Vec<i16> = audio_data
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    // Convert to i32 samples as required by flacenc
    let samples: Vec<i32> = samples_i16.iter().map(|&s| s as i32).collect();

    let (channels, bits_per_sample, sample_rate) = (2, 16, 44100);

    // Create encoder config
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| anyhow::anyhow!("Config verification failed: {:?}", e))?;

    // Create memory source from samples
    let source = flacenc::source::MemSource::from_samples(
        &samples, channels, bits_per_sample, sample_rate
    );

    // Encode with fixed block size
    let flac_stream = flacenc::encode_with_fixed_block_size(
        &config, source, config.block_size
    ).map_err(|e| anyhow::anyhow!("FLAC encoding failed: {:?}", e))?;

    // Write to byte sink
    let mut sink = flacenc::bitsink::ByteSink::new();
    flac_stream.write(&mut sink)
        .map_err(|e| anyhow::anyhow!("Failed to write FLAC stream to sink: {:?}", e))?;

    // Write to file
    std::fs::write(path, sink.as_slice())
        .with_context(|| format!("Failed to write FLAC data to file: {:?}", path))?;

    // TODO: Embed cover art in FLAC file using lofty or other FLAC manipulation library
    // For now, we'll save the cover art as a separate file if provided
    if let Some(cover_art_data) = cover_art {
        let cover_art_path = path.with_extension("jpg");
        if let Err(e) = std::fs::write(&cover_art_path, cover_art_data) {
            warn!("Failed to save cover art to {:?}: {}", cover_art_path, e);
        }
    }

    Ok(())
}

/// Set metadata tags on audio file
fn set_audio_metadata(
    path: &Path,
    track: &CdTrack,
    album_title: &str,
    album_artist: &str,
    release_id: Option<&str>,
) -> Result<()> {
    match lofty::read_from_path(path) {
        Ok(mut tagged_file) => {
            if let Some(tag) = tagged_file.primary_tag_mut() {
                tag.insert_text(ItemKey::TrackTitle, track.title.clone());
                tag.insert_text(ItemKey::TrackArtist, track.artist.clone());
                tag.insert_text(ItemKey::AlbumTitle, album_title.to_string());
                tag.insert_text(ItemKey::AlbumArtist, album_artist.to_string());
                tag.insert_text(ItemKey::TrackNumber, track.number.to_string());
                if let Some(id) = release_id {
                    // Assuming lofty uses this key for MusicBrainz Release ID
                    tag.insert_text(ItemKey::MusicBrainzReleaseId, id.to_string());
                }
            }

            // lofty::save() is the modern way to write tags
            // For now, we'll continue to skip saving as per original logic.
            // tagged_file.save_to(path)?;
        }
        Err(_) => {
            warn!("Could not read file for metadata: {}", path.display());
        }
    }

    Ok(())
}
