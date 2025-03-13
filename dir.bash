#!/bin/bash

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
