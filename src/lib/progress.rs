use std::sync::mpsc;

/// Progress reporting utilities for consistent TUI messaging
/// These functions provide standardized progress messages across all commands
/// Please update this when adding or changing progress messages
/// Progress message types for consistent formatting
#[derive(Debug, Clone)]
pub enum ProgressMessage {
    /// File scanning completion
    ScanComplete {
        files_scanned: usize,
        audio_files_found: usize,
        files_skipped: usize,
    },
    /// Total file count update
    TotalFiles {
        count: usize,
    },
    /// File grouping completion
    GroupingComplete {
        audio_files_count: usize,
        album_groups_count: usize,
    },
    /// MusicBrainz search completion
    MusicBrainzSearchComplete {
        artist: String,
        album: String,
        success: bool,
    },
    /// Album processing start
    ProcessingGroup {
        artist: String,
        album: String,
    },
    /// Album processing completion
    AlbumProcessingComplete {
        artist: String,
        album: String,
        files_processed: usize,
    },
    /// Album skipped due to no MusicBrainz match
    AlbumSkipped {
        artist: String,
        album: String,
    },
    /// Final completion message
    FinalComplete {
        folder_name: String,
    },
    /// Custom message
    Custom {
        message: String,
    },
}

impl ProgressMessage {
    /// Format the message for TUI display
    pub fn format(&self) -> String {
        match self {
            ProgressMessage::ScanComplete { files_scanned, audio_files_found, files_skipped } => {
                format!(
                    "COMPLETED: Scanned {} files ({} audio files found, {} skipped)",
                    files_scanned, audio_files_found, files_skipped
                )
            }
            ProgressMessage::TotalFiles { count } => {
                format!("TOTAL_FILES:{}", count)
            }
            ProgressMessage::GroupingComplete { audio_files_count, album_groups_count } => {
                format!(
                    "COMPLETED: Grouped {} audio files into {} album groups",
                    audio_files_count, album_groups_count
                )
            }
            ProgressMessage::MusicBrainzSearchComplete { artist, album, success } => {
                if *success {
                    format!("COMPLETED: MusicBrainz search for {} - {}", artist, album)
                } else {
                    format!("COMPLETED: MusicBrainz search for {} - {} (failed)", artist, album)
                }
            }
            ProgressMessage::ProcessingGroup { artist, album } => {
                format!("Processing group: {} - {}", artist, album)
            }
            ProgressMessage::AlbumProcessingComplete { artist, album, files_processed } => {
                format!(
                    "COMPLETED: Finished processing {} - {} ({} files processed)",
                    artist, album, files_processed
                )
            }
            ProgressMessage::AlbumSkipped { artist, album } => {
                format!("COMPLETED: Skipped {} - {} (no MusicBrainz match found)", artist, album)
            }
            ProgressMessage::FinalComplete { folder_name } => {
                format!("Successfully synchronized all files in {}", folder_name)
            }
            ProgressMessage::Custom { message } => {
                message.clone()
            }
        }
    }
}

/// Send a progress message to the TUI channel
pub fn send_progress_message(
    tx: &mpsc::Sender<String>,
    message: ProgressMessage,
) -> anyhow::Result<()> {
    tx.send(message.format())
        .map_err(|e| anyhow::anyhow!("Failed to send progress message: {}", e))?;
    Ok(())
}

/// Send a progress message to the TUI channel with context for error handling
pub fn send_progress_message_with_context(
    tx: &mpsc::Sender<String>,
    message: ProgressMessage,
    context: &str,
) -> anyhow::Result<()> {
    tx.send(message.format())
        .map_err(|e| anyhow::anyhow!("Failed to send progress message: {} - {}", context, e))?;
    Ok(())
}

/// Convenience functions for common progress messages
pub fn send_scan_complete(
    tx: &mpsc::Sender<String>,
    files_scanned: usize,
    audio_files_found: usize,
    files_skipped: usize,
) -> anyhow::Result<()> {
    send_progress_message(
        tx,
        ProgressMessage::ScanComplete {
            files_scanned,
            audio_files_found,
            files_skipped,
        },
    )
}

pub fn send_total_files(tx: &mpsc::Sender<String>, count: usize) -> anyhow::Result<()> {
    send_progress_message(tx, ProgressMessage::TotalFiles { count })
}

pub fn send_grouping_complete(
    tx: &mpsc::Sender<String>,
    audio_files_count: usize,
    album_groups_count: usize,
) -> anyhow::Result<()> {
    send_progress_message(
        tx,
        ProgressMessage::GroupingComplete {
            audio_files_count,
            album_groups_count,
        },
    )
}

pub fn send_musicbrainz_search_complete(
    tx: &mpsc::Sender<String>,
    artist: &str,
    album: &str,
    success: bool,
) -> anyhow::Result<()> {
    send_progress_message(
        tx,
        ProgressMessage::MusicBrainzSearchComplete {
            artist: artist.to_string(),
            album: album.to_string(),
            success,
        },
    )
}

pub fn send_processing_group(
    tx: &mpsc::Sender<String>,
    artist: &str,
    album: &str,
) -> anyhow::Result<()> {
    send_progress_message(
        tx,
        ProgressMessage::ProcessingGroup {
            artist: artist.to_string(),
            album: album.to_string(),
        },
    )
}

pub fn send_album_processing_complete(
    tx: &mpsc::Sender<String>,
    artist: &str,
    album: &str,
    files_processed: usize,
) -> anyhow::Result<()> {
    send_progress_message(
        tx,
        ProgressMessage::AlbumProcessingComplete {
            artist: artist.to_string(),
            album: album.to_string(),
            files_processed,
        },
    )
}

pub fn send_album_skipped(
    tx: &mpsc::Sender<String>,
    artist: &str,
    album: &str,
) -> anyhow::Result<()> {
    send_progress_message(
        tx,
        ProgressMessage::AlbumSkipped {
            artist: artist.to_string(),
            album: album.to_string(),
        },
    )
}

pub fn send_final_complete(tx: &mpsc::Sender<String>, folder_name: &str) -> anyhow::Result<()> {
    send_progress_message(
        tx,
        ProgressMessage::FinalComplete {
            folder_name: folder_name.to_string(),
        },
    )
}

pub fn send_custom_message(tx: &mpsc::Sender<String>, message: &str) -> anyhow::Result<()> {
    send_progress_message(tx, ProgressMessage::Custom { message: message.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_message_formatting() {
        let scan_msg = ProgressMessage::ScanComplete {
            files_scanned: 100,
            audio_files_found: 80,
            files_skipped: 20,
        };
        assert_eq!(
            scan_msg.format(),
            "COMPLETED: Scanned 100 files (80 audio files found, 20 skipped)"
        );

        let total_msg = ProgressMessage::TotalFiles { count: 50 };
        assert_eq!(total_msg.format(), "TOTAL_FILES:50");

        let grouping_msg = ProgressMessage::GroupingComplete {
            audio_files_count: 80,
            album_groups_count: 5,
        };
        assert_eq!(
            grouping_msg.format(),
            "COMPLETED: Grouped 80 audio files into 5 album groups"
        );

        let musicbrainz_msg = ProgressMessage::MusicBrainzSearchComplete {
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            success: true,
        };
        assert_eq!(
            musicbrainz_msg.format(),
            "COMPLETED: MusicBrainz search for Test Artist - Test Album"
        );

        let processing_msg = ProgressMessage::ProcessingGroup {
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
        };
        assert_eq!(
            processing_msg.format(),
            "Processing group: Test Artist - Test Album"
        );

        let completion_msg = ProgressMessage::AlbumProcessingComplete {
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            files_processed: 10,
        };
        assert_eq!(
            completion_msg.format(),
            "COMPLETED: Finished processing Test Artist - Test Album (10 files processed)"
        );

        let skipped_msg = ProgressMessage::AlbumSkipped {
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
        };
        assert_eq!(
            skipped_msg.format(),
            "COMPLETED: Skipped Test Artist - Test Album (no MusicBrainz match found)"
        );

        let final_msg = ProgressMessage::FinalComplete {
            folder_name: "Test Album".to_string(),
        };
        assert_eq!(
            final_msg.format(),
            "Successfully synchronized all files in Test Album"
        );

        let custom_msg = ProgressMessage::Custom {
            message: "Custom message".to_string(),
        };
        assert_eq!(custom_msg.format(), "Custom message");
    }
}
