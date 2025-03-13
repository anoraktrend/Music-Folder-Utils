#!/bin/bash
# Music Folder Utils - Set folder icons using .directory files
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

# Check if a directory is provided as an argument
if [ -z "$1" ]; then
    echo "Usage: $0 <directory>"
    exit 1
fi

# Get the target directory
target_dir="$1"

# Find all directories and create a .directory file in each
find "$target_dir" -type d | while read -r dir; do
    # Define the path for the .directory file
    directory_file="$dir/.directory"
    
    # Write the contents to the .directory file
    echo -e "[Desktop Entry]\nIcon=./folder.jpg" > "$directory_file"
    
    echo "Created $directory_file"
done

echo "Finished creating .directory files in all subdirectories of $target_dir."
