use clap::Parser;
use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use magick_rust::magick_wand_genesis;
use dotenvy::dotenv;
use std::sync::{mpsc, Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::path::{Path, PathBuf};

mod commands;
mod utils;
mod tui;

// Generic helper to run an operation with a TUI
fn run_with_tui<I, T, F>(title: &'static str, items: I, operation: F) -> Result<()>
where
    I: IntoIterator<Item = T> + Send + 'static,
    T: Send + 'static,
    F: Fn(T) -> Result<String> + Send + 'static,
{
    let items: Vec<T> = items.into_iter().collect();
    let total_items = items.len();
    let cancel_token = Arc::new(AtomicBool::new(true));
    let (tx, rx) = mpsc::channel();

    let thread_cancel_token = cancel_token.clone();
    let handle = thread::spawn(move || -> Result<()> {
        tx.send(format!("TOTAL_FILES:{}", total_items))?;
        tx.send(title.to_string())?;
        for item in items {
            if !thread_cancel_token.load(Ordering::SeqCst) {
                break;
            }
            let msg = operation(item)?;
            tx.send(format!("COMPLETED: {}", msg))?;
        }
        Ok(())
    });

    tui::run_tui(rx, cancel_token).map_err(anyhow::Error::from)?;

    handle.join().unwrap()?;

    Ok(())
}


// Helper function to run TUI for album operations
fn run_album_tui<F>(title: &'static str, music_dir: &str, operation: F) -> Result<()>
where
    F: Fn(&Path) -> Result<()> + Send + Sync + 'static,
{
    let album_paths = utils::get_all_album_paths(music_dir)?;
    let op = Arc::new(operation);
    run_with_tui(title, album_paths, move |path: PathBuf| {
        let op = op.clone();
        op(&path)?;
        Ok(path.display().to_string())
    })
}

// Helper function to run TUI for track operations
fn run_track_tui<F>(title: &'static str, music_dir: &str, operation: F) -> Result<()>
where
    F: Fn(&Path) -> Result<()> + Send + Sync + 'static,
{
    let track_paths = utils::get_all_track_paths(music_dir)?;
    let op = Arc::new(operation);
    run_with_tui(title, track_paths, move |path: PathBuf| {
        let op = op.clone();
        op(&path)?;
        Ok(path.display().to_string())
    })
}

// Helper function to run TUI for folder operations
fn run_folder_tui<F>(title: &'static str, music_dir: &str, operation: F) -> Result<()>
where
    F: Fn(&Path) -> Result<()> + Send + Sync + 'static,
{
    let folder_paths = utils::get_all_folder_paths(music_dir)?;
    let op = Arc::new(operation);
    run_with_tui(title, folder_paths, move |path: PathBuf| {
        let op = op.clone();
        op(&path)?;
        Ok(path.display().to_string())
    })
}

// Helper function for the All command steps
fn run_all_sync_tags(music_dir: &str, rt: &tokio::runtime::Runtime) -> Result<()> {
    let album_paths = utils::get_all_album_paths(music_dir)?;
    let total_albums = album_paths.len();
    let cancel_token = Arc::new(AtomicBool::new(true));
    let (tx, rx) = mpsc::channel();

    let thread_cancel_token = cancel_token.clone();
    let rt_handle = rt.handle().clone();
    let _music_dir_clone = music_dir.to_string();
    let handle = thread::spawn(move || -> Result<()> {
        tx.send(format!("TOTAL_FILES:{}", total_albums))?;
        tx.send("Syncing Tags with MusicBrainz".to_string())?;
        for album_path in album_paths {
            if !thread_cancel_token.load(Ordering::SeqCst) {
                break;
            }
            rt_handle.block_on(commands::sync::process_single_album_sync_tags(&album_path, tx.clone()))?;
        }
        Ok(())
    });

    tui::run_tui(rx, cancel_token).map_err(anyhow::Error::from)?;

    handle.join().unwrap()?;

    Ok(())
}

fn run_all_artist_art(music_dir: &str, rt: &tokio::runtime::Runtime) -> Result<()> {
    commands::art::extract_artist_art(music_dir)?;
    rt.block_on(commands::art::fetch_placeholders(music_dir))
}

fn run_all_album_art(music_dir: &str) -> Result<()> {
    run_album_tui(
        "Extracting Album Art",
        music_dir,
        commands::art::process_single_album_art,
    )
}

fn run_all_folder_icons(music_dir: &str) -> Result<()> {
    run_folder_tui(
        "Setting Folder Icons",
        music_dir,
        commands::art::set_folder_icons_callback,
    )
}

fn run_all_album_symlinks(music_dir: &str) -> Result<()> {
    let music_dir_owned = music_dir.to_string();
    run_album_tui(
        "Creating Album Symlinks",
        music_dir,
        move |album_path| commands::albums::process_single_album_symlink(album_path, &music_dir_owned),
    )
}

fn run_all_track_symlinks(music_dir: &str) -> Result<()> {
    let music_dir_owned = music_dir.to_string();
    run_track_tui(
        "Creating Track Symlinks",
        music_dir,
        move |track_path| commands::tracks::process_single_track_symlink(track_path, &music_dir_owned),
    )
}

fn run_all_organize(music_dir: &str) -> Result<()> {
    let music_dir_owned = music_dir.to_string();
    let cancel_token = Arc::new(AtomicBool::new(true));
    let (tx, rx) = mpsc::channel();

    let thread_cancel_token = cancel_token.clone();
    let handle = thread::spawn(move || -> Result<()> {
        tx.send("TOTAL_FILES:6".to_string())?;
        tx.send("Organizing Music Library".to_string())?;

        if !thread_cancel_token.load(Ordering::SeqCst) { return Ok(()); }
        let rt = tokio::runtime::Runtime::new()?;
        run_all_sync_tags(&music_dir_owned, &rt)?;
        tx.send("COMPLETED: Synced tags with MusicBrainz".to_string())?;

        if !thread_cancel_token.load(Ordering::SeqCst) { return Ok(()); }
        commands::reorganize::reorganize_misplaced_files(&music_dir_owned, false, true)?;
        tx.send("COMPLETED: Reorganized misplaced files".to_string())?;

        if !thread_cancel_token.load(Ordering::SeqCst) { return Ok(()); }
        commands::import::import_and_organize_files(&music_dir_owned, &music_dir_owned, false, true)?;
        tx.send("COMPLETED: Imported external files".to_string())?;

        if !thread_cancel_token.load(Ordering::SeqCst) { return Ok(()); }
        commands::organize::organize_music_library(&music_dir_owned, false, true)?;
        tx.send("COMPLETED: Organized files by metadata".to_string())?;

        if !thread_cancel_token.load(Ordering::SeqCst) { return Ok(()); }
        run_all_album_symlinks(&music_dir_owned)?;
        tx.send("COMPLETED: Created album symlinks".to_string())?;

        if !thread_cancel_token.load(Ordering::SeqCst) { return Ok(()); }
        run_all_track_symlinks(&music_dir_owned)?;
        tx.send("COMPLETED: Created track symlinks".to_string())?;

        Ok(())
    });

    tui::run_tui(rx, cancel_token).map_err(anyhow::Error::from)?;

    handle.join().unwrap()?;

    Ok(())
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
    /// Sync music tags with MusicBrainz and fetch cover art
    SyncWithArt {
        /// Music directory to sync
        #[arg(default_value = "~/Music")]
        music_dir: String,
    },
    /// Reorganize misplaced files to their proper artist/album structure
    Reorganize {
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
    },
    /// Import music files from an external directory into the music library
    Import {
        /// Path to the directory containing files to import
        import_path: String,
        /// Music directory to import into
        #[arg(default_value = "~/Music")]
        music_dir: String,
        /// Perform a dry run without actually importing files
        #[arg(long)]
        dry_run: bool,
    },
    /// Import music files with MusicBrainz integration and cover art fetching
    ImportEnhanced {
        /// Path to the directory containing files to import
        import_path: String,
        /// Music directory to import into
        #[arg(default_value = "~/Music")]
        music_dir: String,
        /// Perform a dry run without actually importing files
        #[arg(long)]
        dry_run: bool,
    },
    /// Import music from a CD
    Cd {
        /// CD device path (e.g., /dev/cdrom)
        device: String,
        /// Music directory
        #[arg(default_value = "~/Music")]
        music_dir: String,
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
    let command_to_execute = cli.command.clone();
    match command_to_execute {
        Commands::Art { music_dir } => {
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
            )
            .context(format!("Failed to set folder icons for music directory: {}", music_dir))?;

            // Extract album art
            run_album_tui(
                "Extracting Album Art",
                &music_dir,
                commands::art::process_single_album_art,
            )
            .context(format!("Failed to extract album art for music directory: {}", music_dir))?;
        }
        Commands::Albums { music_dir } => {
            let music_dir_owned = music_dir.to_string();
            run_album_tui(
                "Creating Album Symlinks",
                &music_dir,
                move |album_path| commands::albums::process_single_album_symlink(album_path, &music_dir_owned),
            )
            .context(format!("Failed to create album symlinks for music directory: {}", music_dir))?;
        }
        Commands::Tracks { music_dir } => {
            let music_dir_owned = music_dir.to_string();
            run_track_tui(
                "Creating Track Symlinks",
                &music_dir,
                move |track_path| commands::tracks::process_single_track_symlink(track_path, &music_dir_owned),
            )
            .context(format!("Failed to create track symlinks for music directory: {}", music_dir))?;
        }
        Commands::SyncWithArt { music_dir } => {
            run_all_sync_tags(&music_dir, &rt)?;
        }
        Commands::Reorganize { music_dir } => {
            commands::reorganize::reorganize_misplaced_files(&music_dir, false, false)
                .context(format!("Failed to reorganize misplaced files in music directory: {}", music_dir))?;
        }
        Commands::Import { import_path, music_dir, dry_run } => {
            commands::import::import_and_organize_files(&import_path, &music_dir, dry_run, false)
                .context(format!("Failed to import files from {} to music directory: {}", import_path, music_dir))?;
        }
        Commands::ImportEnhanced { import_path, music_dir, dry_run } => {
            let cancel_token = Arc::new(AtomicBool::new(true));
            let (tx, rx) = mpsc::channel();
            let rt_handle = rt.handle().clone();
            let _thread_cancel_token = cancel_token.clone();
            let import_path_clone = import_path.clone();
            let music_dir_clone = music_dir.clone();
            let handle = thread::spawn(move || -> Result<()> {
                rt_handle.block_on(commands::import::import_and_organize_files_with_musicbrainz(
                    &import_path_clone, &music_dir_clone, dry_run, false, tx
                ))
            });
            tui::run_tui(rx, cancel_token).map_err(anyhow::Error::from)?;
            handle.join().unwrap()?;
        }
        Commands::Cd { device, music_dir } => {
            let cancel_token = Arc::new(AtomicBool::new(true));
            let (tx, rx) = mpsc::channel();
            let rt_handle = rt.handle().clone();
            let _thread_cancel_token = cancel_token.clone();
            let device_clone = device.clone();
            let music_dir_clone = music_dir.clone();
            let handle = thread::spawn(move || -> Result<()> {
                rt_handle.block_on(commands::cd::import_cd(&device_clone, &music_dir_clone, tx))
            });
            tui::run_tui(rx, cancel_token).map_err(anyhow::Error::from)?;
            handle.join().unwrap()?;
        }
        Commands::All { music_dir, skip } => {
            use std::collections::HashSet;
            let skip_set: HashSet<String> = skip.into_iter().map(|s| s.to_lowercase()).collect();

            // 1. Sync Tags with MusicBrainz (first step)
            if !skip_set.contains("sync") {
                run_all_sync_tags(&music_dir, &rt)?;
            }

            // 2. Handle artist images
            if !skip_set.contains("art") {
                run_all_artist_art(&music_dir, &rt)?;
            }

            // 3. Setting Folder Icons
            if !skip_set.contains("icons") && !skip_set.contains("art") {
                run_all_folder_icons(&music_dir)?;
            }

            // 4. Extracting Album Art
            if !skip_set.contains("art") {
                run_all_album_art(&music_dir)?;
            }

            // 5. Creating Album Symlinks
            if !skip_set.contains("albums") {
                run_all_album_symlinks(&music_dir)?;
            }

            // 6. Creating Track Symlinks
            if !skip_set.contains("tracks") {
                run_all_track_symlinks(&music_dir)?;
            }

            // 7. Organizing Music Library
            if !skip_set.contains("organize") {
                run_all_organize(&music_dir)?;
            }
        }
    }

    Ok(())
}