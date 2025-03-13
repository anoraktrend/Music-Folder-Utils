#!/bin/bash
# Music Folder Utils - Set folder icons using GIO
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

# Function to set a custom icon for a folder
set_custom_icon() {
    local folder_path="$1"
    local icon_path="$1/folder.jpg"  # Icon path is dynamically set

    if [[ ! -f "$icon_path" ]]; then
        echo "Skipping '$folder_path': No 'folder.jpg' found."
        return 1
    fi

    gio set "$folder_path" metadata::custom-icon "file://$icon_path"
    echo "Custom icon set for '$folder_path'."
}

# Recursive function to traverse directories
process_directories() {
    local base_path="$1"

    for subdir in "$base_path"/*/; do
        if [[ -d "$subdir" ]]; then
            set_custom_icon "$subdir"
            process_directories "$subdir"  # Recursive call for subdirectories
        fi
    done
}

# Example usage
# Replace this path with the root folder you want to start with
ROOT_FOLDER="/home/lucy/media/music"

if [[ ! -d "$ROOT_FOLDER" ]]; then
    echo "Error: Root folder '$ROOT_FOLDER' does not exist."
    exit 1
fi

set_custom_icon "$ROOT_FOLDER"  # Set icon for the root folder if applicable
process_directories "$ROOT_FOLDER"
