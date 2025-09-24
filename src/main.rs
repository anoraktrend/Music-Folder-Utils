use clap::Parser;
use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use magick_rust::magick_wand_genesis;
use dotenvy::dotenv;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

mod commands;
mod utils;
mod tui;

// Helper function to run TUI for album operations
fn run_album_tui<F>(
    title: &str,
    music_dir: &str,
    operation: F,
    running_token: Arc<AtomicBool>,
) -> Result<()>
where
    F: Fn(&std::path::Path) -> Result<()> + Send + Sync + 'static,
{
    let album_paths = utils::get_all_album_paths(music_dir)?;
    let total_albums = album_paths.len();
    tui::run_tui(title, total_albums, move |tx, cancel_token| {
        for album_path in album_paths.iter() {
            if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }
            operation(album_path)?;
            tx.send(album_path.display().to_string())?;
        }
        Ok(())
    }, running_token)
}

// Helper function to run TUI for track operations
fn run_track_tui<F>(
    title: &str,
    music_dir: &str,
    operation: F,
    running_token: Arc<AtomicBool>,
) -> Result<()>
where
    F: Fn(&std::path::Path) -> Result<()> + Send + Sync + 'static,
{
    let track_paths = utils::get_all_track_paths(music_dir)?;
    let total_tracks = track_paths.len();
    tui::run_tui(title, total_tracks, move |tx, cancel_token| {
        for track_path in track_paths.iter() {
            if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }
            operation(track_path)?;
            tx.send(track_path.display().to_string())?;
        }
        Ok(())
    }, running_token)
}

// Helper function to run TUI for folder operations
fn run_folder_tui<F>(
    title: &str,
    music_dir: &str,
    operation: F,
    running_token: Arc<AtomicBool>,
) -> Result<()>
where
    F: Fn(&std::path::Path) -> Result<()> + Send + Sync + 'static,
{
    let folder_paths = utils::get_all_folder_paths(music_dir)?;
    let total_folders = folder_paths.len();
    tui::run_tui(title, total_folders, move |tx, cancel_token| {
        for folder_path in folder_paths.iter() {
            if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }
            operation(folder_path)?;
            tx.send(folder_path.display().to_string())?;
        }
        Ok(())
    }, running_token)
}

// Helper function for the All command steps
fn run_all_sync_tags(music_dir: &str, rt: &tokio::runtime::Runtime, running_token: Arc<AtomicBool>) -> Result<()> {
    let album_paths = utils::get_all_album_paths(music_dir)?;
    let total_albums = album_paths.len();
    let rt_handle = rt.handle().clone();
    tui::run_tui("Syncing Tags with MusicBrainz", total_albums, move |tx, cancel_token| {
        for album_path in album_paths.iter() {
            if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }
            rt_handle.block_on(commands::sync::process_single_album_sync_tags(album_path, tx.clone()))?;
        }
        Ok(())
    }, running_token)
}

fn run_all_artist_art(music_dir: &str, rt: &tokio::runtime::Runtime) -> Result<()> {
    commands::art::extract_artist_art(music_dir)?;
    rt.block_on(commands::art::fetch_placeholders(music_dir))
}

fn run_all_album_art(music_dir: &str, running_token: Arc<AtomicBool>) -> Result<()> {
    run_album_tui(
        "Extracting Album Art",
        music_dir,
        commands::art::process_single_album_art,
        running_token,
    )
}

fn run_all_folder_icons(music_dir: &str, running_token: Arc<AtomicBool>) -> Result<()> {
    run_folder_tui(
        "Setting Folder Icons",
        music_dir,
        commands::art::set_folder_icons_callback,
        running_token,
    )
}

fn run_all_album_symlinks(music_dir: &str, running_token: Arc<AtomicBool>) -> Result<()> {
    let music_dir_owned = music_dir.to_string();
    run_album_tui(
        "Creating Album Symlinks",
        music_dir,
        move |album_path| commands::albums::process_single_album_symlink(album_path, &music_dir_owned),
        running_token,
    )
}

fn run_all_track_symlinks(music_dir: &str, running_token: Arc<AtomicBool>) -> Result<()> {
    let music_dir_owned = music_dir.to_string();
    run_track_tui(
        "Creating Track Symlinks",
        music_dir,
        move |track_path| commands::tracks::process_single_track_symlink(track_path, &music_dir_owned),
        running_token,
    )
}

fn run_all_organize(music_dir: &str, running_token: Arc<AtomicBool>) -> Result<()> {
    let music_dir_owned = music_dir.to_string();
    let total_steps = 6;
    tui::run_tui("Organizing Music Library", total_steps, move |tx, cancel_token| {
        if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }

        // Step 1: Sync tags
        let rt = tokio::runtime::Runtime::new()?;
        run_all_sync_tags(&music_dir_owned, &rt, cancel_token.clone())?;
        tx.send("COMPLETED: Synced tags with MusicBrainz".to_string())?;

        if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }

        // Step 2: Reorganize misplaced files
        commands::organize::reorganize_misplaced_files(&music_dir_owned, false, true)?;
        tx.send("COMPLETED: Reorganized misplaced files".to_string())?;

        if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }

        // Step 3: Import files
        commands::organize::import_and_organize_files(&music_dir_owned, &music_dir_owned, false, true)?;
        tx.send("COMPLETED: Imported external files".to_string())?;

        if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }

        // Step 4: Organize files by metadata
        commands::organize::organize_music_library(&music_dir_owned, false, true)?;
        tx.send("COMPLETED: Organized files by metadata".to_string())?;

        if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }

        // Step 5: Create album symlinks
        run_all_album_symlinks(&music_dir_owned, cancel_token.clone())?;
        tx.send("COMPLETED: Created album symlinks".to_string())?;

        if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }

        // Step 6: Create track symlinks
        run_all_track_symlinks(&music_dir_owned, cancel_token)?;
        tx.send("COMPLETED: Created track symlinks".to_string())?;

        Ok(())
    }, running_token)
}

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
    /// Reorganize misplaced files to their proper artist/album structure
    Reorganize {
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
    },
    /// Import files from an external directory and organize them
    Import {
        /// Import source directory
        import_path: String,
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
        /// Show what would be done without actually importing
        #[arg(long)]
        dry_run: bool,
    },
    /// Run all tasks (art, icons, albums, tracks)
    All {
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
        /// Comma-separated list of subcommands to skip when running `all` (examples: sync,art,albums,tracks,organize,reorganize,import)
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
            let music_dir = music_dir.clone();
            // Handle artist images first
            commands::art::extract_artist_art(&music_dir)
                .context(format!("Failed to extract artist art for music directory: {}", music_dir))?;
            rt.handle().block_on(commands::art::fetch_placeholders(&music_dir))
                .context(format!("Failed to fetch placeholders for music directory: {}", music_dir))?;

            // Set folder icons
            run_folder_tui(
                "Setting Folder Icons",
                &music_dir,
                commands::art::set_folder_icons_callback,
                running_token.clone(),
            )
            .context(format!("Failed to set folder icons for music directory: {}", music_dir))?;

            // Extract album art
            run_album_tui(
                "Extracting Album Art",
                &music_dir,
                commands::art::process_single_album_art,
                running_token.clone(),
            )
            .context(format!("Failed to extract album art for music directory: {}", music_dir))?;
        }
        Commands::Albums { music_dir } => {
            let music_dir_owned = music_dir.to_string();
            run_album_tui(
                "Creating Album Symlinks",
                &music_dir,
                move |album_path| commands::albums::process_single_album_symlink(album_path, &music_dir_owned),
                running_token,
            )
            .context(format!("Failed to create album symlinks for music directory: {}", music_dir))?;
        }
        Commands::Tracks { music_dir } => {
            let music_dir_owned = music_dir.to_string();
            run_track_tui(
                "Creating Track Symlinks",
                &music_dir,
                move |track_path| commands::tracks::process_single_track_symlink(track_path, &music_dir_owned),
                running_token,
            )
            .context(format!("Failed to create track symlinks for music directory: {}", music_dir))?;
        }
        Commands::Sync { music_dir } => {
            let album_paths = utils::get_all_album_paths(&music_dir)?;
            let total_albums = album_paths.len();
            let rt_handle = rt.handle().clone();
            tui::run_tui("Syncing Tags with MusicBrainz", total_albums, move |tx, cancel_token| {
                for album_path in album_paths.iter() {
                    if !cancel_token.load(Ordering::SeqCst) { return Ok(()); }
                    rt_handle.block_on(commands::sync::process_single_album_sync_tags(album_path, tx.clone()))?;
                }
                Ok(())
            }, running_token)
            .context(format!("Failed to sync tags with MusicBrainz for music directory: {}", music_dir))?;
        }
        Commands::Reorganize { music_dir } => {
            commands::organize::reorganize_misplaced_files(&music_dir, false, false)
                .context(format!("Failed to reorganize misplaced files in music directory: {}", music_dir))?;
        }
        Commands::Import { import_path, music_dir, dry_run } => {
            commands::organize::import_and_organize_files(&import_path, &music_dir, dry_run, false)
                .context(format!("Failed to import files from {} to music directory: {}", import_path, music_dir))?;
        }
        Commands::All { music_dir, skip } => {
            use std::collections::HashSet;
            let skip_set: HashSet<String> = skip.into_iter().map(|s| s.to_lowercase()).collect();

            // 1. Sync Tags with MusicBrainz (first step)
            if !skip_set.contains("sync") {
                run_all_sync_tags(&music_dir, &rt, running_token.clone())?;
            }

            // 2. Handle artist images
            if !skip_set.contains("art") {
                run_all_artist_art(&music_dir, &rt)?;
            }

            // 3. Setting Folder Icons
            if !skip_set.contains("icons") && !skip_set.contains("art") {
                run_all_folder_icons(&music_dir, running_token.clone())?;
            }

            // 4. Extracting Album Art
            if !skip_set.contains("art") {
                run_all_album_art(&music_dir, running_token.clone())?;
            }

            // 5. Creating Album Symlinks
            if !skip_set.contains("albums") {
                run_all_album_symlinks(&music_dir, running_token.clone())?;
            }

            // 6. Creating Track Symlinks
            if !skip_set.contains("tracks") {
                run_all_track_symlinks(&music_dir, running_token.clone())?;
            }

            // 7. Organizing Music Library
            if !skip_set.contains("organize") {
                run_all_organize(&music_dir, running_token)?;
            }
        }
    }

    Ok(())
}