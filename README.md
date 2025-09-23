# Music Folder Utils

A C program for managing music folder icons and album art in Linux.

![Nautilus with Album Art Icons](media/Screenshot_Nautilus.jpg) ![Dolphin with Album Art Icons](media/Screenshot_Dolphin.jpg)

Easily set album art as folder icons in your music collection. Works with both GNOME and KDE file managers.

## Quick Start

### Compilation

To compile the program, simply run `make`:

```bash
make
```

### Usage

The easiest way to set up album art icons is to use the `all` command:

```bash
./mfutil all /path/to/music/directory
```

If you don't provide a directory, it will default to `~/Music`.

The program will:
1. Check for required dependencies
2. Extract album art from your music files
3. Set up folder icons using the appropriate methods

## Commands

*   `./mfutil art [music_directory]`: Extracts album art from music files.
*   `./mfutil icons [music_directory]`: Sets folder icons.
*   `./mfutil albums [music_directory]`: Creates symlinks to album directories.
*   `./mfutil tracks [music_directory]`: Creates symlinks to individual tracks.
*   `./mfutil sync [music_directory]`: Syncs music tags with MusicBrainz.
*   `./mfutil all [music_directory]`: Runs `art`, `icons`, `albums`, and `tracks` commands.

## Requirements
- `ffmpeg`
- `picard`
- `gio`
- A C compiler (like `gcc`) and `make`

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.