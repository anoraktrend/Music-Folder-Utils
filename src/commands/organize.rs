use anyhow::Result;
use lofty::{self, file::TaggedFileExt, tag::ItemKey};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Organize music files into proper artist/album structure
pub fn organize_music_library(music_dir: &str, dry_run: bool, quiet: bool) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir).to_string();
    let music_path = Path::new(&music_dir);
    let artists_path = music_path.join("Artists");

    if !music_path.exists() {
        if dry_run {
            if !quiet {
                println!("Would create music directory: {}", music_path.display());
            }
        } else {
            fs::create_dir_all(music_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create music directory '{}': {}",
                    music_path.display(),
                    e
                )
            })?;
        }
    }

    if !artists_path.exists() {
        if dry_run {
            if !quiet {
                println!("Would create Artists directory: {}", artists_path.display());
            }
        } else {
            fs::create_dir(&artists_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create Artists directory '{}': {}",
                    artists_path.display(),
                    e
                )
            })?;
        }
    }

    if !quiet {
        println!("🔍 Scanning music directory: {}", music_path.display());
    }

    // Find all audio files in the music directory
    let mut files_to_move = Vec::new();
    let mut unknown_files = Vec::new();

    for entry in WalkDir::new(music_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            // Check if it's an audio file
            let audio_extensions = ["mp3", "flac", "m4a", "ogg", "aac", "wma", "wav", "aiff"];
            if audio_extensions.contains(&ext.as_str()) {
                files_to_move.push(path.to_path_buf());
            } else {
                unknown_files.push(path.to_path_buf());
            }
        }
    }

    if !quiet {
        println!("✅ Found {} audio files to organize", files_to_move.len());
    }
    if !quiet && !unknown_files.is_empty() {
        println!(
            "ℹ️  Found {} non-audio files (will be left in place)",
            unknown_files.len()
        );
    }

    // Group files by artist and album
    let mut file_groups: HashMap<(String, String), Vec<PathBuf>> = HashMap::new();
    let mut total_files = 0;

    for file_path in files_to_move {
        let (artist, album) = extract_artist_album_from_file(&file_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to extract metadata from '{}': {}",
                file_path.display(),
                e
            )
        })?;

        // Create a clean filename for the group key
        let clean_artist = sanitize_filename(&artist);
        let clean_album = sanitize_filename(&album);

        file_groups
            .entry((clean_artist.clone(), clean_album.clone()))
            .or_default()
            .push(file_path.clone());

        total_files += 1;

        if dry_run && !quiet {
            println!(
                "Would organize: {} -> {} / {}",
                file_path.display(),
                clean_artist,
                clean_album
            );
        }
    }

    if !quiet && dry_run {
        println!(
            "📊 Found {} unique artist/album combinations",
            file_groups.len()
        );
    }

    // Store counts before moving the collections
    let total_groups = file_groups.len();

    // Create directory structure and move files
    for ((artist, album), files) in file_groups {
        let artist_path = artists_path.join(&artist);
        let album_path = artist_path.join(&album);

        if dry_run {
            if !quiet {
                println!("📁 Would create directory: {}", album_path.display());
                for file in files {
                    println!(
                        "  📄 Would move: {} -> {}",
                        file.display(),
                        album_path.display()
                    );
                }
            }
        } else {
            // Create directories
            fs::create_dir_all(&album_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create album directory '{}': {}",
                    album_path.display(),
                    e
                )
            })?;

            // Move files
            for file_path in files {
                let file_name = file_path.file_name().ok_or_else(|| {
                    anyhow::anyhow!("File '{}' has no filename", file_path.display())
                })?;
                let dest_path = album_path.join(file_name);

                if file_path != dest_path {
                    fs::rename(&file_path, &dest_path).map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to move '{}' to '{}': {}",
                            file_path.display(),
                            dest_path.display(),
                            e
                        )
                    })?;
                    if !quiet {
                        println!(
                            "✅ Moved: {} -> {}",
                            file_path.display(),
                            dest_path.display()
                        );
                    }
                }
            }
        }
    }

    if dry_run && !quiet {
        println!("\n🎭 This was a dry run. No files were actually moved.");
        println!("💡 Run without --dry-run to perform the actual organization.");
    } else if !quiet {
        println!("\n🎉 Music library organization completed successfully!");
        println!(
            "   📁 Organized {} files into {} artist/album combinations",
            total_files, total_groups
        );
    }

    Ok(())
}

/// Extract artist and album information from a music file
fn extract_artist_album_from_file(file_path: &Path) -> Result<(String, String)> {
    match lofty::read_from_path(file_path) {
        Ok(tagged_file) => {
            let tags = tagged_file.tags();
            if let Some(tag) = tags.first() {
                // Try multiple artist fields in order of preference
                let artist = tag
                    .get_string(&ItemKey::AlbumArtist)
                    .or_else(|| tag.get_string(&ItemKey::TrackArtist))
                    .unwrap_or_else(|| {
                        // Try to extract from filename if no artist metadata
                        file_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown Artist")
                            .split(" - ")
                            .next()
                            .unwrap_or("Unknown Artist")
                    })
                    .to_string();

                // Try multiple album fields in order of preference
                let album = tag
                    .get_string(&ItemKey::AlbumTitle)
                    .unwrap_or_else(|| {
                        // Try to extract from parent directory name
                        file_path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown Album")
                    })
                    .to_string();

                Ok((artist, album))
            } else {
                // Fallback to path-based extraction
                extract_from_path(file_path)
            }
        }
        Err(_) => {
            // Fallback to path-based extraction
            extract_from_path(file_path)
        }
    }
}

/// Extract artist and album from file path when tags are not available
fn extract_from_path(file_path: &Path) -> Result<(String, String)> {
    let parent = file_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("File '{}' has no parent directory", file_path.display()))?;

    // Try to extract album from parent directory name
    let album = parent
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| {
            // Clean up common album directory naming patterns
            let cleaned = name
                .replace(['_', '-'], " ")
                .split_whitespace()
                .filter(|word| {
                    // Filter out common non-album words
                    let lower = word.to_lowercase();
                    !matches!(
                        lower.as_str(),
                        "album" | "music" | "songs" | "tracks" | "collection"
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");

            if cleaned.trim().is_empty() {
                "Unknown Album".to_string()
            } else {
                cleaned.trim().to_string()
            }
        })
        .unwrap_or_else(|| "Unknown Album".to_string());

    let grandparent = parent
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Album directory '{}' has no parent", parent.display()))?;

    // Try to extract artist from grandparent directory name
    let artist = grandparent
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| {
            // Clean up common artist directory naming patterns
            let cleaned = name
                .replace(['_', '-'], " ")
                .split_whitespace()
                .filter(|word| {
                    // Filter out common non-artist words
                    let lower = word.to_lowercase();
                    !matches!(
                        lower.as_str(),
                        "artist" | "band" | "group" | "music" | "collection"
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");

            if cleaned.trim().is_empty() {
                "Various Artists".to_string()
            } else {
                cleaned.trim().to_string()
            }
        })
        .unwrap_or_else(|| "Various Artists".to_string());

    Ok((artist, album))
}

/// Sanitize filename to be safe for filesystem
fn sanitize_filename(name: &str) -> String {
    // Replace problematic characters with safe alternatives
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Create artist directory structure from existing files
pub fn create_artist_directories(music_dir: &str, dry_run: bool, quiet: bool) -> Result<()> {
    let music_dir = shellexpand::tilde(music_dir).to_string();
    let music_path = Path::new(&music_dir);
    let artists_path = music_path.join("Artists");

    if !artists_path.exists() {
        if dry_run {
            if !quiet {
                println!(
                    "📁 Would create Artists directory: {}",
                    artists_path.display()
                );
            }
        } else {
            fs::create_dir(&artists_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create Artists directory '{}': {}",
                    artists_path.display(),
                    e
                )
            })?;
        }
    }

    if !quiet {
        println!("🔍 Scanning for existing album directories...");
    }

    // Find all directories that could be albums
    let mut album_dirs = Vec::new();

    for entry in WalkDir::new(music_path)
        .max_depth(3) // Only go 3 levels deep to avoid symlinks and other directories
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_dir() && path != music_path && path != artists_path {
            // Check if this directory contains audio files
            let has_audio = WalkDir::new(path)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .any(|e| {
                    if e.path().is_file() {
                        let ext = e
                            .path()
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_lowercase())
                            .unwrap_or_default();
                        matches!(
                            ext.as_str(),
                            "mp3" | "flac" | "m4a" | "ogg" | "aac" | "wma" | "wav" | "aiff"
                        )
                    } else {
                        false
                    }
                });

            if has_audio {
                album_dirs.push(path.to_path_buf());
            }
        }
    }

    if !quiet && dry_run {
        println!("📊 Found {} potential album directories", album_dirs.len());
    }

    // Group albums by artist (parent directory name)
    let mut artist_groups: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for album_path in album_dirs {
        let parent = album_path.parent().unwrap_or(&album_path);
        let artist = parent
            .file_name()
            .and_then(|n| n.to_str())
            .map(|name| {
                // Clean up the artist name
                let cleaned = name
                    .replace(['_', '-'], " ")
                    .split_whitespace()
                    .filter(|word| {
                        // Filter out common non-artist words
                        let lower = word.to_lowercase();
                        !matches!(
                            lower.as_str(),
                            "artist" | "band" | "group" | "music" | "collection"
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                if cleaned.trim().is_empty() {
                    "Various Artists".to_string()
                } else {
                    cleaned.trim().to_string()
                }
            })
            .unwrap_or_else(|| "Various Artists".to_string());

        artist_groups
            .entry(artist.clone())
            .or_default()
            .push(album_path.clone());
    }

    if !quiet && dry_run {
        println!("👥 Found {} unique artists", artist_groups.len());
    }

    // Create artist directories and move albums
    for (artist, albums) in artist_groups {
        let artist_path = artists_path.join(&artist);

        if dry_run {
            if !quiet {
                println!(
                    "📁 Would create artist directory: {}",
                    artist_path.display()
                );
                for album in albums {
                    println!(
                        "  📂 Would move album: {} -> {}",
                        album.display(),
                        artist_path.display()
                    );
                }
            }
        } else {
            fs::create_dir_all(&artist_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create artist directory '{}': {}",
                    artist_path.display(),
                    e
                )
            })?;

            for album_path in albums {
                let album_name = album_path.file_name().ok_or_else(|| {
                    anyhow::anyhow!("Album directory '{}' has no name", album_path.display())
                })?;
                let new_album_path = artist_path.join(album_name);

                if album_path != new_album_path {
                    // Move the entire album directory
                    if new_album_path.exists() {
                        if !quiet {
                            println!(
                                "⚠️  Album already exists at destination, skipping: {}",
                                album_path.display()
                            );
                        }
                        continue;
                    }

                    // Check if source still exists before moving
                    if !album_path.exists() {
                        if !quiet {
                            println!(
                                "❌ Source album no longer exists, skipping: {}",
                                album_path.display()
                            );
                        }
                        continue;
                    }

                    fs::rename(&album_path, &new_album_path).map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to move album '{}' to '{}': {}",
                            album_path.display(),
                            new_album_path.display(),
                            e
                        )
                    })?;
                    if !quiet {
                        println!(
                            "✅ Moved album: {} -> {}",
                            album_path.display(),
                            new_album_path.display()
                        );
                    }
                }
            }
        }
    }

    if !quiet && dry_run {
        println!("\n🎭 This was a dry run. No directories were actually moved.");
        println!("💡 Run without --dry-run to perform the actual organization.");
    } else if !quiet {
        println!("\n🎉 Artist directory creation completed successfully!");
    }

    Ok(())
}
