use clap::Parser;
use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use magick_rust::magick_wand_genesis;

mod commands;
mod utils;
mod tui;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Extract album art
    Art {
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
    },
    /// Create album symlinks
    Albums {
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
    },
    /// Create track symlinks
    Tracks {
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
    },
    /// Sync tags with MusicBrainz
    Sync {
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
    },
    /// Run all tasks (art, icons, albums, tracks)
    All {
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
    },
}

fn main() -> Result<()> {
    ffmpeg::init().context("Failed to initialize ffmpeg")?;
    magick_wand_genesis();
    let cli = Cli::parse();

    let rt = tokio::runtime::Runtime::new()?;

    match &cli.command {
        Commands::Art { music_dir } => {
            // Handle artist images first
            commands::art::extract_artist_art(music_dir)?;
            rt.block_on(commands::art::fetch_placeholders(music_dir))?;
            let folder_paths = utils::get_all_folder_paths(music_dir)?;
            let total_folders = folder_paths.len();
            tui::run_tui("Setting Folder Icons", total_folders, |index| {
                commands::art::set_folder_icons_callback(&folder_paths[index])
            })?;

            // Then handle album art with TUI
            let album_paths = utils::get_all_album_paths(music_dir)?;
            let total_albums = album_paths.len();
            tui::run_tui("Extracting Album Art", total_albums, |index| {
                commands::art::process_single_album_art(&album_paths[index])
            })?;
        }
        Commands::Albums { music_dir } => {
            let album_paths = utils::get_all_album_paths(music_dir)?;
            let total_albums = album_paths.len();
            tui::run_tui("Creating Album Symlinks", total_albums, |index| {
                commands::albums::process_single_album_symlink(&album_paths[index], music_dir)
            })?;
        }
        Commands::Tracks { music_dir } => {
            let track_paths = utils::get_all_track_paths(music_dir)?;
            let total_tracks = track_paths.len();
            tui::run_tui("Creating Track Symlinks", total_tracks, |index| {
                commands::tracks::process_single_track_symlink(&track_paths[index], music_dir)
            })?;
        }
        Commands::Sync { music_dir } => {
            let album_paths = utils::get_all_album_paths(music_dir)?;
            let total_albums = album_paths.len();
            tui::run_tui("Syncing Tags with MusicBrainz", total_albums, |index| {
                rt.block_on(commands::sync::process_single_album_sync_tags(&album_paths[index]))
            })?;
        }
        Commands::All { music_dir } => {
            commands::art::extract_artist_art(music_dir)?;
            rt.block_on(commands::art::fetch_placeholders(music_dir))?;
            let folder_paths = utils::get_all_folder_paths(music_dir)?;
            let total_folders = folder_paths.len();
            tui::run_tui("Setting Folder Icons", total_folders, |index| {
                commands::art::set_folder_icons_callback(&folder_paths[index])
            })?;
            let album_paths = utils::get_all_album_paths(music_dir)?;
            let total_albums = album_paths.len();
            tui::run_tui("Extracting Album Art", total_albums, |index| {
                commands::art::process_single_album_art(&album_paths[index])
            })?;
            // The original create_album_symlinks and create_track_symlinks were placeholders
            // and are now handled by the TUI calls above. I'll replicate the TUI calls here.
            let album_paths_for_symlinks = utils::get_all_album_paths(music_dir)?;
            let total_albums_for_symlinks = album_paths_for_symlinks.len();
            tui::run_tui("Creating Album Symlinks", total_albums_for_symlinks, |index| {
                commands::albums::process_single_album_symlink(&album_paths_for_symlinks[index], music_dir)
            })?;

            let track_paths = utils::get_all_track_paths(music_dir)?;
            let total_tracks = track_paths.len();
            tui::run_tui("Creating Track Symlinks", total_tracks, |index| {
                commands::tracks::process_single_track_symlink(&track_paths[index], music_dir)
            })?;

            let album_paths_for_sync = utils::get_all_album_paths(music_dir)?;
            let total_albums_for_sync = album_paths_for_sync.len();
            tui::run_tui("Syncing Tags with MusicBrainz", total_albums_for_sync, |index| {
                rt.block_on(commands::sync::process_single_album_sync_tags(&album_paths_for_sync[index]))
            })?;
        }
    }

    Ok(())
}