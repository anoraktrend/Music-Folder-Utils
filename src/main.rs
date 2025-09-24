use clap::Parser;
use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use magick_rust::magick_wand_genesis;
use dotenvy::dotenv;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

mod commands;
mod utils;
mod tui;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, clap::Subcommand)]
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
        /// Comma-separated list of subcommands to skip when running `all` (examples: sync,art,albums,tracks)
        #[arg(long, value_delimiter = ',')]
        skip: Vec<String>,
    },
}

fn main() -> Result<()> {
    // Load environment variables from a .env file if present
    dotenv().ok();
    ffmpeg::init().context("Failed to initialize ffmpeg")?;
    magick_wand_genesis();
    let cli = Cli::parse();

    let rt = tokio::runtime::Runtime::new()?;
    let running_token = Arc::new(AtomicBool::new(true)); // Create the running token
    let command_to_execute = cli.command.clone();
    match command_to_execute {
        Commands::Art { music_dir } => {
            let music_dir = music_dir.clone(); // Clone music_dir here
            // Handle artist images first
            commands::art::extract_artist_art(&music_dir)?;
                        rt.handle().block_on(commands::art::fetch_placeholders(&music_dir))?;

            let folder_paths = utils::get_all_folder_paths(&music_dir)?;
            let total_folders = folder_paths.len();
            let _music_dir_clone_1 = music_dir.clone();
                        tui::run_tui("Setting Folder Icons", total_folders, move |tx, running_token_closure| {
                for index in 0..total_folders {
                    if !running_token_closure.load(Ordering::SeqCst) { return Ok(()); } // Check for cancellation
                    commands::art::set_folder_icons_callback(&folder_paths[index])?;
                    tx.send(folder_paths[index].display().to_string()).context("Failed to send progress update")?;
                }
                Ok(())
            }, running_token.clone())?;


            // Then handle album art with TUI
            let album_paths = utils::get_all_album_paths(&music_dir)?;
            let total_albums = album_paths.len();
            tui::run_tui("Extracting Album Art", total_albums, move |tx, running_token_closure| {
                for index in 0..total_albums {
                    if !running_token_closure.load(Ordering::SeqCst) { return Ok(()); } // Add cancellation check
                    commands::art::process_single_album_art(&album_paths[index])?;
                    tx.send(album_paths[index].display().to_string()).context("Failed to send progress update")?;
                }
                Ok(())
            }, running_token.clone())?;
        }
        Commands::Albums { music_dir } => {
            let music_dir = music_dir.clone(); // Clone music_dir here
            let album_paths = utils::get_all_album_paths(&music_dir)?;
            let total_albums = album_paths.len();
            tui::run_tui("Creating Album Symlinks", total_albums, move |tx, running_token_closure| {
                for index in 0..total_albums {
                    if !running_token_closure.load(Ordering::SeqCst) { return Ok(()); } // Add cancellation check
                    commands::albums::process_single_album_symlink(&album_paths[index], &music_dir)?;
                    tx.send(album_paths[index].display().to_string()).context("Failed to send progress update")?;
                }
                Ok(())
            }, running_token.clone())?;
        }
        Commands::Tracks { music_dir } => {
            let music_dir = music_dir.clone(); // Clone music_dir here
            let track_paths = utils::get_all_track_paths(&music_dir)?;
            let total_tracks = track_paths.len();
            tui::run_tui("Creating Track Symlinks", total_tracks, move |tx, running_token_closure| {
                for index in 0..total_tracks {
                    if !running_token_closure.load(Ordering::SeqCst) { return Ok(()); } // Add cancellation check
                    commands::tracks::process_single_track_symlink(&track_paths[index], &music_dir)?;
                    tx.send(track_paths[index].display().to_string()).context("Failed to send progress update")?;
                }
                Ok(())
            }, running_token.clone())?;
        }
        Commands::Sync { music_dir } => {
            let music_dir = music_dir.clone(); // Clone music_dir here
            let album_paths = utils::get_all_album_paths(&music_dir)?;
            let total_albums = album_paths.len();
            let rt_handle = rt.handle().clone(); // Clone the handle once
            tui::run_tui("Syncing Tags with MusicBrainz", total_albums, move |tx, running_token_closure| {
                for index in 0..total_albums {
                    if !running_token_closure.load(Ordering::SeqCst) { return Ok(()); } // Add cancellation check
                    rt_handle.block_on(commands::sync::process_single_album_sync_tags(&album_paths[index], tx.clone()))?;
                }
                Ok(())
            }, running_token.clone())?;
        }
        Commands::All { music_dir, skip } => {
            use std::collections::HashSet;
            let music_dir_clone = music_dir.clone();
            let skip_set: HashSet<String> = skip.into_iter().map(|s| s.to_lowercase()).collect();

            // 1. Sync Tags with MusicBrainz (first step)
            if !skip_set.contains("sync") {
            let album_paths_for_sync = utils::get_all_album_paths(&music_dir_clone)?;
            let total_albums_for_sync = album_paths_for_sync.len();
            let rt_handle_sync = rt.handle().clone(); // Clone the handle for this closure
            tui::run_tui("Syncing Tags with MusicBrainz", total_albums_for_sync, move |tx, running_token_closure| {
                for index in 0..total_albums_for_sync {
                    if !running_token_closure.load(Ordering::SeqCst) { return Ok(()); } // Add cancellation check
                    rt_handle_sync.block_on(commands::sync::process_single_album_sync_tags(&album_paths_for_sync[index], tx.clone()))?;
                }
                Ok(())
            }, running_token.clone())?;
            }

            // 2. Handle artist images
            if !skip_set.contains("art") {
                commands::art::extract_artist_art(&music_dir_clone)?;
                rt.handle().block_on(commands::art::fetch_placeholders(&music_dir_clone))?;
            }

            // 3. Setting Folder Icons
            if !skip_set.contains("icons") && !skip_set.contains("art") {
            let folder_paths = utils::get_all_folder_paths(&music_dir_clone)?;
            let total_folders = folder_paths.len();
            tui::run_tui("Setting Folder Icons", total_folders, move |tx, running_token_closure| {
                for index in 0..total_folders {
                    if !running_token_closure.load(Ordering::SeqCst) { return Ok(()); } // Check for cancellation
                    commands::art::set_folder_icons_callback(&folder_paths[index])?;
                    tx.send((index + 1).to_string()).context("Failed to send progress update")?;
                }
                Ok(())
            }, running_token.clone())?;
            }

            // 4. Extracting Album Art
            if !skip_set.contains("art") {
            let album_paths_art = utils::get_all_album_paths(&music_dir_clone)?;
            let total_albums_art = album_paths_art.len();
            tui::run_tui("Extracting Album Art", total_albums_art, move |tx, running_token_closure| {
                for index in 0..total_albums_art {
                    if !running_token_closure.load(Ordering::SeqCst) { return Ok(()); } // Add cancellation check
                    commands::art::process_single_album_art(&album_paths_art[index])?;
                    tx.send((index + 1).to_string()).context("Failed to send progress update")?;
                }
                Ok(())
            }, running_token.clone())?;
            }

            // 5. Creating Album Symlinks
            if !skip_set.contains("albums") {
            let album_paths_for_symlinks = utils::get_all_album_paths(&music_dir_clone)?;
            let total_albums_for_symlinks = album_paths_for_symlinks.len();
            let music_dir_clone_for_albums = music_dir_clone.clone(); // Clone for this closure
            tui::run_tui("Creating Album Symlinks", total_albums_for_symlinks, move |tx, running_token_closure| {
                for index in 0..total_albums_for_symlinks {
                    if !running_token_closure.load(Ordering::SeqCst) { return Ok(()); } // Add cancellation check
                    commands::albums::process_single_album_symlink(&album_paths_for_symlinks[index], &music_dir_clone_for_albums)?;
                    tx.send((index + 1).to_string()).context("Failed to send progress update")?;
                }
                Ok(())
            }, running_token.clone())?;
            }

            // 6. Creating Track Symlinks
            if !skip_set.contains("tracks") {
            let track_paths = utils::get_all_track_paths(&music_dir_clone)?;
            let total_tracks = track_paths.len();
            let music_dir_clone_for_tracks = music_dir_clone.clone(); // Clone for this closure
            tui::run_tui("Creating Track Symlinks", total_tracks, move |tx, running_token_closure| {
                for index in 0..total_tracks {
                    if !running_token_closure.load(Ordering::SeqCst) { return Ok(()); } // Add cancellation check
                    commands::tracks::process_single_track_symlink(&track_paths[index], &music_dir_clone_for_tracks)?;
                    tx.send((index + 1).to_string()).context("Failed to send progress update")?;
                }
                Ok(())
            }, running_token.clone())?;
            }
        }
    }

    Ok(())
}