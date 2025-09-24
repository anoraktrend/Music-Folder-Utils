use anyhow::{Context, Result};
use clap::Parser;
use dotenvy::dotenv;
use ffmpeg_next as ffmpeg;
use magick_rust::magick_wand_genesis;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

mod commands;
mod tui;
mod utils;

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
    /// Organize music library into artist/album structure
    Organize {
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
        /// Show what would be done without making changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Run all tasks (art, icons, albums, tracks)
    All {
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
        /// Comma-separated list of subcommands to skip when running `all` (examples: sync,art,albums,tracks,organize)
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
            rt.handle()
                .block_on(commands::art::fetch_placeholders(&music_dir))?;

            let folder_paths = utils::get_all_folder_paths(&music_dir)?;
            let total_folders = folder_paths.len();
            let _music_dir_clone_1 = music_dir.clone();
            tui::run_tui(
                "Setting Folder Icons",
                total_folders,
                move |tx, running_token_closure| {
                    for folder_path in folder_paths.iter().take(total_folders) {
                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        } // Check for cancellation
                        commands::art::set_folder_icons_callback(folder_path)?;
                        tx.send(folder_path.display().to_string())
                            .context("Failed to send progress update")?;
                    }
                    Ok(())
                },
                running_token.clone(),
            )?;

            // Then handle album art with TUI
            let album_paths = utils::get_all_album_paths(&music_dir)?;
            let total_albums = album_paths.len();
            tui::run_tui(
                "Extracting Album Art",
                total_albums,
                move |tx, running_token_closure| {
                    for album_path in album_paths.iter().take(total_albums) {
                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        } // Add cancellation check
                        commands::art::process_single_album_art(album_path)?;
                        tx.send(album_path.display().to_string())
                            .context("Failed to send progress update")?;
                    }
                    Ok(())
                },
                running_token.clone(),
            )?;
        }
        Commands::Albums { music_dir } => {
            let music_dir = music_dir.clone(); // Clone music_dir here
            let album_paths = utils::get_all_album_paths(&music_dir)?;
            let total_albums = album_paths.len();
            tui::run_tui(
                "Creating Album Symlinks",
                total_albums,
                move |tx, running_token_closure| {
                    for album_path in album_paths.iter().take(total_albums) {
                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        } // Add cancellation check
                        commands::albums::process_single_album_symlink(album_path, &music_dir)?;
                        tx.send(album_path.display().to_string())
                            .context("Failed to send progress update")?;
                    }
                    Ok(())
                },
                running_token.clone(),
            )?;
        }
        Commands::Tracks { music_dir } => {
            let music_dir = music_dir.clone(); // Clone music_dir here
            let track_paths = utils::get_all_track_paths(&music_dir)?;
            let total_tracks = track_paths.len();
            tui::run_tui(
                "Creating Track Symlinks",
                total_tracks,
                move |tx, running_token_closure| {
                    for track_path in track_paths.iter().take(total_tracks) {
                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        } // Add cancellation check
                        commands::tracks::process_single_track_symlink(track_path, &music_dir)?;
                        tx.send(track_path.display().to_string())
                            .context("Failed to send progress update")?;
                    }
                    Ok(())
                },
                running_token.clone(),
            )?;
        }
        Commands::Sync { music_dir } => {
            let music_dir = music_dir.clone(); // Clone music_dir here
            let album_paths = utils::get_all_album_paths(&music_dir)?;
            let total_albums = album_paths.len();
            let rt_handle = rt.handle().clone(); // Clone the handle once
            tui::run_tui(
                "Syncing Tags with MusicBrainz",
                total_albums,
                move |tx, running_token_closure| {
                    for album_path in &album_paths {
                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        } // Add cancellation check
                        rt_handle.block_on(commands::sync::process_single_album_sync_tags(
                            album_path,
                            tx.clone(),
                        ))?;
                    }
                    Ok(())
                },
                running_token.clone(),
            )?;
        }
        Commands::Organize { music_dir, dry_run } => {
            let music_dir = music_dir.clone();
            let total_steps = 4; // Four main steps: analyze, organize, album symlinks, track symlinks
            let running_token_for_organize = running_token.clone(); // Clone for the organize command

            tui::run_tui(
                "Organizing Music Library",
                total_steps,
                move |tx, running_token_closure| {
                    if !running_token_closure.load(Ordering::SeqCst) {
                        return Ok(());
                    }

                    // Step 1: Create artist directories
                    commands::organize::create_artist_directories(&music_dir, dry_run, true)?; // true = quiet mode for TUI
                    tx.send(format!(
                        "COMPLETED: Created artist directories ({})",
                        if dry_run { "dry run" } else { "completed" }
                    ))
                    .context("Failed to send progress update")?;

                    if !running_token_closure.load(Ordering::SeqCst) {
                        return Ok(());
                    }

                    // Step 2: Organize files by metadata
                    commands::organize::organize_music_library(&music_dir, dry_run, true)?; // true = quiet mode for TUI
                    tx.send(format!(
                        "COMPLETED: Organized files by metadata ({})",
                        if dry_run { "dry run" } else { "completed" }
                    ))
                    .context("Failed to send progress update")?;

                    if !running_token_closure.load(Ordering::SeqCst) {
                        return Ok(());
                    }

                    // Step 3: Create album symlinks
                    if !dry_run {
                        let album_paths = utils::get_all_album_paths(&music_dir)?;
                        let total_albums = album_paths.len();
                        let music_dir_clone = music_dir.clone();
                        let running_token_for_albums = running_token_closure.clone();
                        tui::run_tui(
                            "Creating Album Symlinks",
                            total_albums,
                            move |tx, _running_token_closure| {
                                for (index, album_path) in album_paths.iter().enumerate() {
                                    if !running_token_for_albums.load(Ordering::SeqCst) {
                                        return Ok(());
                                    }
                                    commands::albums::process_single_album_symlink(
                                        album_path,
                                        &music_dir_clone,
                                    )?;
                                    tx.send((index + 1).to_string())
                                        .context("Failed to send progress update")?;
                                }
                                Ok(())
                            },
                            running_token_closure.clone(),
                        )?;
                    } else {
                        tx.send("COMPLETED: Skipped album symlinks (dry run)".to_string())
                            .context("Failed to send progress update")?;
                    }

                    if !running_token_closure.load(Ordering::SeqCst) {
                        return Ok(());
                    }

                    // Step 4: Create track symlinks
                    if !dry_run {
                        let track_paths = utils::get_all_track_paths(&music_dir)?;
                        let total_tracks = track_paths.len();
                        let music_dir_clone = music_dir.clone();
                        let running_token_for_tracks = running_token_closure.clone();
                        tui::run_tui(
                            "Creating Track Symlinks",
                            total_tracks,
                            move |tx, _running_token_closure| {
                                for (index, track_path) in track_paths.iter().enumerate() {
                                    if !running_token_for_tracks.load(Ordering::SeqCst) {
                                        return Ok(());
                                    }
                                    commands::tracks::process_single_track_symlink(
                                        track_path,
                                        &music_dir_clone,
                                    )?;
                                    tx.send((index + 1).to_string())
                                        .context("Failed to send progress update")?;
                                }
                                Ok(())
                            },
                            running_token_closure.clone(),
                        )?;
                    } else {
                        tx.send("COMPLETED: Skipped track symlinks (dry run)".to_string())
                            .context("Failed to send progress update")?;
                    }

                    Ok(())
                },
                running_token_for_organize.clone(),
            )?;
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
                tui::run_tui(
                    "Syncing Tags with MusicBrainz",
                    total_albums_for_sync,
                    move |tx, running_token_closure| {
                        for (index, album_path) in album_paths_for_sync.iter().enumerate() {
                            if !running_token_closure.load(Ordering::SeqCst) {
                                return Ok(());
                            } // Add cancellation check
                            rt_handle_sync.block_on(
                                commands::sync::process_single_album_sync_tags(
                                    album_path,
                                    tx.clone(),
                                ),
                            )?;
                        }
                        Ok(())
                    },
                    running_token.clone(),
                )?;
            }

            // 2. Handle artist images
            if !skip_set.contains("art") {
                let music_dir_clone_for_art = music_dir_clone.clone();
                tui::run_tui(
                    "Extracting Artist Art",
                    1,
                    move |tx, running_token_closure| {
                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        }
                        commands::art::extract_artist_art(&music_dir_clone_for_art)?;
                        tx.send("COMPLETED: Extracted artist images".to_string())
                            .context("Failed to send progress update")?;
                        Ok(())
                    },
                    running_token.clone(),
                )?;

                let music_dir_clone_for_placeholders = music_dir_clone.clone();
                tui::run_tui(
                    "Fetching Placeholders",
                    1,
                    move |tx, running_token_closure| {
                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        }
                        rt.handle().block_on(commands::art::fetch_placeholders(
                            &music_dir_clone_for_placeholders,
                        ))?;
                        tx.send("COMPLETED: Fetched placeholder images".to_string())
                            .context("Failed to send progress update")?;
                        Ok(())
                    },
                    running_token.clone(),
                )?;
            }

            // 3. Setting Folder Icons
            if !skip_set.contains("icons") && !skip_set.contains("art") {
                let folder_paths = utils::get_all_folder_paths(&music_dir_clone)?;
                let total_folders = folder_paths.len();
                tui::run_tui(
                    "Setting Folder Icons",
                    total_folders,
                    move |tx, running_token_closure| {
                        for (index, folder_path) in folder_paths.iter().enumerate() {
                            if !running_token_closure.load(Ordering::SeqCst) {
                                return Ok(());
                            } // Check for cancellation
                            commands::art::set_folder_icons_callback(folder_path)?;
                            tx.send((index + 1).to_string())
                                .context("Failed to send progress update")?;
                        }
                        Ok(())
                    },
                    running_token.clone(),
                )?;
            }

            // 4. Extracting Album Art
            if !skip_set.contains("art") {
                let album_paths_art = utils::get_all_album_paths(&music_dir_clone)?;
                let total_albums_art = album_paths_art.len();
                tui::run_tui(
                    "Extracting Album Art",
                    total_albums_art,
                    move |tx, running_token_closure| {
                        for (index, album_path) in album_paths_art.iter().enumerate() {
                            if !running_token_closure.load(Ordering::SeqCst) {
                                return Ok(());
                            } // Add cancellation check
                            commands::art::process_single_album_art(album_path)?;
                            tx.send((index + 1).to_string())
                                .context("Failed to send progress update")?;
                        }
                        Ok(())
                    },
                    running_token.clone(),
                )?;
            }

            // 5. Creating Album Symlinks
            if !skip_set.contains("albums") {
                let album_paths_for_symlinks = utils::get_all_album_paths(&music_dir_clone)?;
                let total_albums_for_symlinks = album_paths_for_symlinks.len();
                let music_dir_clone_for_albums = music_dir_clone.clone(); // Clone for this closure
                tui::run_tui(
                    "Creating Album Symlinks",
                    total_albums_for_symlinks,
                    move |tx, running_token_closure| {
                        for (index, album_path) in album_paths_for_symlinks.iter().enumerate() {
                            if !running_token_closure.load(Ordering::SeqCst) {
                                return Ok(());
                            } // Add cancellation check
                            commands::albums::process_single_album_symlink(
                                album_path,
                                &music_dir_clone_for_albums,
                            )?;
                            tx.send((index + 1).to_string())
                                .context("Failed to send progress update")?;
                        }
                        Ok(())
                    },
                    running_token.clone(),
                )?;
            }

            // 6. Creating Track Symlinks
            if !skip_set.contains("tracks") {
                let track_paths = utils::get_all_track_paths(&music_dir_clone)?;
                let total_tracks = track_paths.len();
                let music_dir_clone_for_tracks = music_dir_clone.clone(); // Clone for this closure
                tui::run_tui(
                    "Creating Track Symlinks",
                    total_tracks,
                    move |tx, running_token_closure| {
                        for (index, track_path) in track_paths.iter().enumerate() {
                            if !running_token_closure.load(Ordering::SeqCst) {
                                return Ok(());
                            } // Add cancellation check
                            commands::tracks::process_single_track_symlink(
                                track_path,
                                &music_dir_clone_for_tracks,
                            )?;
                            tx.send((index + 1).to_string())
                                .context("Failed to send progress update")?;
                        }
                        Ok(())
                    },
                    running_token.clone(),
                )?;
            }

            // 7. Organizing Music Library
            if !skip_set.contains("organize") {
                let total_steps = 4;
                let music_dir_clone_for_organize = music_dir_clone.clone();
                let running_token_for_organize = running_token.clone(); // Clone for the organize command
                tui::run_tui(
                    "Organizing Music Library",
                    total_steps,
                    move |tx, running_token_closure| {
                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        }
                        commands::organize::create_artist_directories(
                            &music_dir_clone_for_organize,
                            false,
                            true,
                        )?; // false = not dry run, true = quiet mode for TUI
                        tx.send("COMPLETED: Created artist directories (completed)".to_string())
                            .context("Failed to send progress update")?;

                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        }
                        commands::organize::organize_music_library(
                            &music_dir_clone_for_organize,
                            false,
                            true,
                        )?; // false = not dry run, true = quiet mode for TUI
                        tx.send("COMPLETED: Organized files by metadata (completed)".to_string())
                            .context("Failed to send progress update")?;

                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        }
                        let album_paths =
                            utils::get_all_album_paths(&music_dir_clone_for_organize)?;
                        let total_albums = album_paths.len();
                        let music_dir_clone_for_albums = music_dir_clone_for_organize.clone();
                        let running_token_for_albums = running_token_closure.clone();
                        tui::run_tui(
                            "Creating Album Symlinks",
                            total_albums,
                            move |tx, _running_token_closure| {
                                for (index, album_path) in album_paths.iter().enumerate() {
                                    if !running_token_for_albums.load(Ordering::SeqCst) {
                                        return Ok(());
                                    }
                                    commands::albums::process_single_album_symlink(
                                        album_path,
                                        &music_dir_clone_for_albums,
                                    )?;
                                    tx.send((index + 1).to_string())
                                        .context("Failed to send progress update")?;
                                }
                                Ok(())
                            },
                            running_token_closure.clone(),
                        )?;
                        tx.send("COMPLETED: Created album symlinks (completed)".to_string())
                            .context("Failed to send progress update")?;

                        if !running_token_closure.load(Ordering::SeqCst) {
                            return Ok(());
                        }
                        let track_paths =
                            utils::get_all_track_paths(&music_dir_clone_for_organize)?;
                        let total_tracks = track_paths.len();
                        let music_dir_clone_for_tracks = music_dir_clone_for_organize.clone();
                        let running_token_for_tracks = running_token_closure.clone();
                        tui::run_tui(
                            "Creating Track Symlinks",
                            total_tracks,
                            move |tx, _running_token_closure| {
                                for (index, track_path) in track_paths.iter().enumerate() {
                                    if !running_token_for_tracks.load(Ordering::SeqCst) {
                                        return Ok(());
                                    }
                                    commands::tracks::process_single_track_symlink(
                                        track_path,
                                        &music_dir_clone_for_tracks,
                                    )?;
                                    tx.send((index + 1).to_string())
                                        .context("Failed to send progress update")?;
                                }
                                Ok(())
                            },
                            running_token_closure.clone(),
                        )?;
                        tx.send("COMPLETED: Created track symlinks (completed)".to_string())
                            .context("Failed to send progress update")?;

                        Ok(())
                    },
                    running_token_for_organize.clone(),
                )?;
            }
        }
    }

    Ok(())
}
