# Music Folder Utils

A collection of bash scripts for managing music folder icons and album art in Linux.

![Nautilus with Album Art Icons](media/Screenshot_Nautilus.jpg) ![Dolphin with Album Art Icons](media/Screenshot_Dolphin.jpg)

Easily set album art as folder icons in your music collection. Works with both GNOME and KDE file managers.

## Quick Start

The easiest way to set up album art icons is to use the main setup script:

```bash
bash mfutil.bash /path/to/music/directory
```

The script will:
1. Check for required dependencies
2. Extract album art from your music files
3. Automatically detect your desktop environment
4. Set up folder icons using the appropriate method

## Scripts

### setup-music-icons.bash
Main setup script that automatically runs all necessary operations. This script:
- Detects your desktop environment
- Runs the extraction and icon setting process automatically
- Chooses the appropriate icon setting method for your system

Usage:
```bash
bash mfutil.bash /path/to/music/directory
```

### extart.bash
Extracts album art from music files (MP3, FLAC, M4A) and saves it as `folder.jpg` in each directory. This script:
- Recursively processes a music directory
- Extracts the first image found in audio files using ffmpeg
- Skips directories that already have a folder.jpg

Usage:
```bash
bash extart.bash /path/to/music/directory
```

### dir.bash
Creates `.directory` files in each subdirectory to set folder icons. This script:
- Recursively creates .directory files for KDE/compatible file managers
- Configures each directory to use folder.jpg as its icon

Usage:
```bash
bash dir.bash /path/to/music/directory
```

### seticon.bash
Sets custom folder icons using GIO (GNOME/GTK file managers). This script:
- Recursively processes directories
- Sets folder.jpg as the custom icon for each folder
- Works with GNOME-based file managers

Usage:
```bash
bash seticon.bash /path/to/music/directory
```

## Requirements
- `ffmpeg` (for extart.bash)
- `gio` (for seticon.bash)
- Bash shell

## Workflow
1. First run `extart.bash` to extract album art
2. Then run either:
   - `dir.bash` for KDE-based systems
   - `seticon.bash` for GNOME-based systems

This will set up album art as folder icons throughout your music collection.

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.
