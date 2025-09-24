## Purpose

Quick, actionable orientation for code-generating AI agents working on this repository (music-folder-utils / mfutil). Focus on patterns, build/run flows, conventions, and important integration points so an agent can make safe, correct edits quickly.

## Big-picture architecture

- Binary name / package: `mfutil` (Cargo package defined in `Cargo.toml`). Entry point: `src/main.rs`.
- Command dispatch: `src/main.rs` parses CLI subcommands (Art, Albums, Tracks, Sync, All) and delegates to modules under `src/commands/`.
- Command modules: `src/commands/{art,albums,tracks,sync}.rs` implement the per-item work. Each exposes small functions used by `main.rs` (e.g. `process_single_album_art`, `process_single_album_symlink`, `process_single_track_symlink`, `process_single_album_sync_tags`).
- Utilities: `src/utils.rs` contains filesystem traversal helpers (get_all_album_paths, get_all_track_paths, get_all_folder_paths).
- UI: `src/tui.rs` implements a minimal TUI/progress helper `run_tui(title, total, closure, running_token)` which takes an mpsc channel-based closure to perform work and send progress strings back.
- Data flow: `main.rs` obtains a list of paths from `utils::get_*` then calls command module routines for each path inside a closure passed into `tui::run_tui`. Network calls are async (reqwest + tokio) — many command functions create or reuse a tokio runtime and use `block_on` when invoked from synchronous code.

## Files and examples to reference

- `src/main.rs` — how subcommands are wired into TUI loops and how the `running_token` is cloned and passed to cancel work.
- `src/tui.rs` — `run_tui` contract: argument types and the channel/string protocol used to report progress.
- `src/utils.rs` — path discovery; important: expects a `~/Music/Artists` layout and expands `~` via `shellexpand::tilde`.
- `src/commands/art.rs` — image extraction, placeholder fetching (Pexels), and setting folder icons via `gio::File` and `.directory` files. Key constants: `PEXELS_API_KEY` and `AUDIODB_API_KEY` (hard-coded here).
- `src/commands/sync.rs` — MusicBrainz interaction and tag updates using `audiotags`.

## Project-specific conventions and patterns

- Music directory layout (required by code): top-level `Artists` directory containing per-artist folders, inside which albums reside. The program also creates `Albums/` and `Tracks/` directories at the same level as `Artists` when creating symlinks.
- Icon convention: album/artist folder icons are written as `.folder.jpg` inside the directory and `src/commands/art.rs::set_folder_icons_callback` also writes a `.directory` desktop entry with `Icon=./.folder.jpg`.
- File discovery: audio file extensions are matched by extension checks (mp3, flac, m4a, ogg, wav in places). Use the same extension set when adding logic.
- Progress reporting: workers send human-readable strings down an `mpsc::Sender<String>`; `tui::run_tui` expects that string protocol. Don't change it without updating callers.
- Async vs sync: many network functions are async, but callers often call them via `tokio::runtime::Runtime::block_on`. If you add async APIs, either expose a synchronous wrapper or update callers consistently.
- Platform: code uses `std::os::unix::fs::symlink` and `gio` — project targets Unix-like Linux desktops (GNOME/KDE). Avoid Windows-specific changes unless adding platform gating.

## External integrations / system dependencies

- Requires system libraries for: ffmpeg (libav*), ImageMagick (MagickWand), and GLib/GIO. The Rust crates in `Cargo.toml` (ffmpeg-next, magick_rust, gio, glib) rely on these native libs.
- Network APIs used:
  - MusicBrainz WS2 (search releases) — `src/commands/sync.rs`.
  - Pexels search API — `src/commands/art.rs` (constant `PEXELS_API_KEY` present in codebase).
  - TheAudioDB (artist images) — `src/commands/art.rs` (constant `AUDIODB_API_KEY`).

Example Debian/Ubuntu install suggestions (document-only; adjust to target distro):

```bash
sudo apt update
sudo apt install build-essential pkg-config libavformat-dev libavcodec-dev libavutil-dev libmagickwand-dev libglib2.0-dev libgirepository1.0-dev git
```

## Build / run / debug

- Build (debug): `cargo build`
- Build (release): `cargo build --release`
- Run (debug): `cargo run -- <SUBCOMMAND> [music_dir]` (example: `cargo run --release -- all ~/Music`)
- Run binary directly: `./target/debug/mfutil all ~/Music` or `./target/release/mfutil art ~/Music`
- Debugging: use `RUST_BACKTRACE=1` for backtraces. The code prints progress and informational messages to stdout.

Note: README.md is outdated (mentions a C program and `make`) — prefer `cargo` commands shown above.

## Safety notes and sensitive data

- There are hard-coded API keys/constants in `src/commands/art.rs` (`PEXELS_API_KEY`, `AUDIODB_API_KEY`). Treat these as secrets that should be rotated or moved to environment variables before production use. If you change how API keys are provided, update all call sites.

## Good-first edits and low-risk change patterns

- Replace hard-coded API keys with environment variable reads (look in `src/commands/art.rs`, keep the same request flow).
- Expand supported audio extensions in `src/utils.rs` and `src/commands/*` by updating the match lists.
- Add a small unit test around `utils::get_all_album_paths` using a temporary directory to ensure path discovery behavior.

## When to ask for clarification

- If a change touches the TUI progress protocol (the `mpsc::Sender<String>` message format) — ask which consumer expectations to preserve.
- If changing how `music_dir` is interpreted (tilde expansion, relative vs absolute), confirm desired UX for non-~ paths.

If anything above is unclear or you want me to expand a section (e.g., a precise apt/yum/arch package list for system libs), tell me which distro(s) you target and I'll update the file.
