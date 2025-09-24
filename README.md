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

## ⚡ Fast Builds with Just

For much faster builds, use **Just** - a modern command runner that provides significant performance improvements:

### Install Just
```bash
cargo install just
```

**Or from your distribution's package manager:**
```bash
# Arch Linux
sudo pacman -S just

# Debian/Ubuntu
sudo apt install just

# Fedora
sudo dnf install just

# macOS (Homebrew)
brew install just

# Alpine Linux
sudo apk add just
```

### Fast Build Commands
```bash
just dev           # ⚡⚡⚡ Fastest development builds
just build         # ⚡⚡ Optimized release builds
just build-fast    # ⚡⚡⚡ Fast release builds (no LTO)
just check         # ⚡⚡⚡ Check without building
just test          # ⚡⚡ Run tests
```

### Installation Commands
```bash
just install-local     # Install for current user (~/.local/bin)
just install-system    # Install system-wide (/usr/local/bin) - requires sudo
just install-custom /path/to/dir  # Install to custom location
```

### Performance Benefits
- **Development builds**: 2-5x faster with incremental compilation
- **Release builds**: 1.5-3x faster with optimized settings
- **Fast builds**: 3-10x faster than standard release
- **Rebuilds**: 10-50x faster with build caching (sccache)
- **Linking**: 2-4x faster with lld linker

### Setup Build Tools (Optional)
```bash
just install-sccache    # Install build cache for massive rebuild speedups
just install-lld        # Install faster linker
just setup              # Install all build dependencies
```

### Build Performance Comparison
| Command | Description | Speed | Use Case |
|---------|-------------|-------|----------|
| `cargo build` | Standard dev build | 🐌 | Basic development |
| `just dev` | Optimized dev build | ⚡⚡⚡ | Fast development |
| `cargo build --release` | Standard release | 🐌 | Production |
| `just build` | Optimized release | ⚡⚡ | Best optimization |
| `just build-fast` | Fast release | ⚡⚡⚡ | Development/CI |

**Example workflow:**
```bash
just check          # Quick error check
just dev            # Fast rebuild during development
just install-local  # Install for current user
./target/debug/mfutil organize ./testdata
```

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

Also ensure `ffmpeg` is installed on the system (runtime) for tagging uncommon files and extracting attached pictures.

### Optional: Performance Tools

For significantly faster builds, install these optional tools:

#### sccache (Build Caching)
```bash
# Install sccache for 10-50x faster rebuilds
cargo install sccache

# Or from system packages
sudo apt install sccache    # Ubuntu/Debian
sudo pacman -S sccache      # Arch Linux and derivatives
sudo dnf install sccache    # Fedora and similar, package name may vary
```

#### lld (Faster Linker)
```bash
# Install lld for 2-4x faster linking
sudo apt install lld        # Ubuntu/Debian
sudo pacman -S lld          # Arch Linux and derivatives
sudo dnf install lld        # Fedora and similar, package name may vary
brew install llvm           # macOS
```

**With these tools installed:**
- `just dev` builds become 2-5x faster
- `just build-fast` provides 3-10x faster releases
- Rebuilds become 10-50x faster with caching

## Configuration: API keys

Two external APIs require keys if you want placeholder images or artist images fetched automatically:

- `PEXELS_API_KEY` — used to fetch placeholder images from Pexels for Artists/Albums/Tracks.
- `AUDIODB_API_KEY` — used to fetch artist thumbnails from TheAudioDB.

Set them in your shell before running the program, for example:

```bash
export PEXELS_API_KEY="your_pexels_api_key_here"
export AUDIODB_API_KEY="your_audiodb_api_key_here"
```

You can also set them via a .env file, the .env.example is available for reference.

if you are simply too lazy to get a pexels api key of your own, mine is below

```bash
PEXELS_API_KEY="563492ad6f91700001000001aacfd87a60cb4f369cb54d595b2f4142"
```

You should remove the quotes when pasting into your .env file.

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
