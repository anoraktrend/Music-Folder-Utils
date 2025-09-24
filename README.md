# Music Folder Utils (mfutil)

Set album art as folder icons and create album/track symlink collections for Linux desktops (GNOME/KDE).

![Nautilus with Album Art Icons](media/Screenshot_Nautilus.jpg) ![Dolphin with Album Art Icons](media/Screenshot_Dolphin.jpg)

Note: the project is implemented in Rust. Older references to a C program and `make` are outdated — use `cargo` to build and run.

## Quick start

Build (debug):

```bash
cargo build
```

Build (release):

```bash
cargo build --release
```

Run (example):

```bash
cargo run --release -- all ~/Music
# or, run the built binary
./target/release/mfutil art ~/Music
```

Most commands default to `~/Music` if no path is supplied.

## CLI subcommands

The binary exposes these subcommands (see `src/main.rs`):

- `art [music_dir]` — extract album/artist art and set folder icons
- `albums [music_dir]` — create symlinks for albums under `Albums/`
- `tracks [music_dir]` — create symlinks for tracks under `Tracks/`
- `sync [music_dir]` — query MusicBrainz and update tags
- `all [music_dir]` — run sync, fetch artist art, set icons, extract album art, create album and track symlinks

Example:

```bash
cargo run --release -- sync ~/Music
```

## Project layout & important files

- `src/main.rs` — CLI parsing and orchestration using a small TUI helper
- `src/tui.rs` — `run_tui(title, total, closure, running_token)` progress helper (uses `mpsc::Sender<String>` to receive progress messages)
- `src/utils.rs` — filesystem helpers (expects a `~/Music/Artists` layout)
- `src/commands/` — per-feature modules: `art.rs`, `albums.rs`, `tracks.rs`, `sync.rs`

## System dependencies

The Rust crates wrap native libraries. On Debian/Ubuntu you will typically need:

```bash
sudo apt update
sudo apt install build-essential pkg-config libavformat-dev libavcodec-dev libavutil-dev libmagickwand-dev libglib2.0-dev libgirepository1.0-dev git
```

Also ensure `ffmpeg` is installed on the system (runtime) for extracting attached pictures.

## Configuration: API keys

Two external APIs require keys if you want placeholder images or artist images fetched automatically:

- `PEXELS_API_KEY` — used to fetch placeholder images from Pexels for Artists/Albums/Tracks.
- `AUDIODB_API_KEY` — used to fetch artist thumbnails from TheAudioDB.

Set them in your shell before running the program, for example:

```bash
export PEXELS_API_KEY="your_pexels_api_key_here"
export AUDIODB_API_KEY="your_audiodb_api_key_here"
```

If the variables are not set the program will skip those network calls and continue with local fallbacks.

## Notes & conventions

- Music directory layout: expects a `Artists/` directory with per-artist folders and their albums. The tool will create `Albums/` and `Tracks/` siblings for symlinks.
- Folder icons: the program writes `.folder.jpg` files inside directories and a `.directory` file with `Icon=./.folder.jpg` so GNOME/KDE will display the custom icon.
- Progress protocol: TUI workers send human-readable strings down an `mpsc::Sender<String>`; keep that contract if you change `tui.rs`.
- Async vs sync: network functions in commands are async (reqwest + tokio). Many call sites use a `tokio::runtime::Runtime` and `block_on`. If you change APIs, maintain compatibility or update all callers.

## Security / configuration

- `src/commands/art.rs` currently contains hard-coded API keys (`PEXELS_API_KEY`, `AUDIODB_API_KEY`) — these are secrets. Replace them with environment variables before using in production. If you change the mechanism, update all call sites that expect those constants.

## Debugging

Enable backtraces:

```bash
RUST_BACKTRACE=1 cargo run -- <subcommand>
```

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.
