# Music Folder Utils

A collection of bash scripts for managing music folder icons and album art in Linux.

![Nautilus with Album Art Icons](media/Screenshot_Nautilus.png)
![Dolphin with Album Art Icons](media/Screenshot_Dolphin.png)

Easily set album art as folder icons in your music collection. Works with both GNOME and KDE file managers.

## Scripts

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
edit the script to set the path to your music directory
```bash
bash seticon.bash
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
