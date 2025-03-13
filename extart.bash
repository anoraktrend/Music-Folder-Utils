#!/bin/bash
# Music Folder Utils - Extract album art from music files
# Copyright (C) 2024 Lucy
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

# Top-level directory containing music files and subdirectories
MUSIC_DIR="$1"

# Check if MUSIC_DIR is provided
if [ -z "$MUSIC_DIR" ]; then
    echo "Usage: $0 <music_directory>"
    exit 1
fi

# Iterate through all directories under the top-level directory
find "$MUSIC_DIR" -type d | while read -r dir; do
    # Check if folder.jpg already exists in the directory
    output_file="$dir/folder.jpg"
    if [ -f "$output_file" ]; then
        echo "Album art already exists in $dir (skipping)"
        continue
    fi

    # Find the first music file in the current directory
    file=$(find "$dir" -maxdepth 1 -type f \( -iname "*.mp3" -o -iname "*.flac" -o -iname "*.m4a" \) | head -n 1)

    # Check if a music file was found
    if [ -z "$file" ]; then
        echo "No music files found in $dir (skipping)"
        continue
    fi

    # Extract album art to folder.jpg in the same directory
    if ffmpeg -i "$file" -an -vcodec copy -vframes 1 -f image2 "$output_file" 2>/dev/null; then
        echo "Album art extracted to $output_file"
    else
        echo "Failed to extract album art from $file"
    fi
done
