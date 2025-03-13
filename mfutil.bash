#!/bin/bash
# Music Folder Utils - Main setup script
# Copyright (C) 2024 Lucy

# Print colored status messages
print_status() {
    echo -e "\033[1;34m=>\033[0m $1"
}

print_error() {
    echo -e "\033[1;31mError:\033[0m $1" >&2
}

# Check if music directory is provided
if [ $# -ne 1 ]; then
    echo "Usage: $0 <music_directory>"
    echo "Example: $0 ~/Music"
    exit 1
fi

MUSIC_DIR="$(realpath "$1")"
SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"

# Validate music directory
if [ ! -d "$MUSIC_DIR" ]; then
    print_error "Directory does not exist: $MUSIC_DIR"
    exit 1
fi

# Check dependencies
print_status "Checking dependencies..."
if ! command -v ffmpeg &> /dev/null; then
    print_error "ffmpeg is required but not installed."
    echo "Please install ffmpeg first:"
    echo "  Ubuntu/Debian: sudo apt install ffmpeg"
    echo "  Fedora: sudo dnf install ffmpeg"
    echo "  Arch: sudo pacman -S ffmpeg"
    exit 1
fi

# Extract album art
print_status "Step 1: Extracting album art..."
bash "$SCRIPT_DIR/extart.bash" "$MUSIC_DIR"
if [ $? -ne 0 ]; then
    print_error "Album art extraction failed"
    exit 1
fi

# Detect File Managers and set icons
print_status "Step 2: Detecting File Managers..."

# Get desktop environment, fallback to window manager
DE="${XDG_CURRENT_DESKTOP:-$DESKTOP_SESSION}"
DE="${DE:-$(basename "${GDMSESSION:-unknown}")}"
DE="${DE^^}" # Convert to uppercase for consistent matching

# Detect file manager type
if [[ "$DE" =~ (GNOME|PANTHEON|UNITY|BUDGIE|CINNAMON|POP|ZORIN|UBUNTU|REGOLITH|XFCE|DEEPIN|MATE|COSMIC) ]]; then
    print_status "GTK environment detected ($DE)"
    if ! command -v gio &> /dev/null; then
        print_error "gio is required but not installed"
        exit 1
    fi
    bash "$SCRIPT_DIR/seticon.bash" "$MUSIC_DIR"
elif [[ "$DE" =~ (KDE|LXQT|PLASMA|NEON) ]]; then
    print_status "KDE environment detected ($DE)"
    print_status "Creating .directory files"
    bash "$SCRIPT_DIR/dir.bash" "$MUSIC_DIR"
else
    print_status "Unknown desktop environment ($DE), checking for specific file managers..."
    if command -v gio &> /dev/null; then
        print_status "GTK file manager detected (gio present)"
        bash "$SCRIPT_DIR/seticon.bash" "$MUSIC_DIR"
    else
        print_status "Defaulting to .directory files"
        bash "$SCRIPT_DIR/dir.bash" "$MUSIC_DIR"
    fi
fi

print_status "Setup complete! Your music folders should now display album art as icons."
echo "Note: You may need to refresh your file manager to see the changes."
