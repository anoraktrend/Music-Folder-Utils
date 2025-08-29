#!/bin/bash

# Define the target directory
TARGET_DIR=~/Music/Artists

# Check if the target directory exists
if [ ! -d "$TARGET_DIR" ]; then
    echo "Error: Directory '$TARGET_DIR' not found."
    exit 1
fi

# Create a directory to store the symlinks, if it doesn't exist
SYMLINK_DEST_DIR=~/Music/
mkdir -p "$SYMLINK_DEST_DIR"

# Find all subdirectories within subdirectories of TARGET_DIR
find "$TARGET_DIR" -mindepth 2 -type d | while read -r subfolder_path; do
    # Get the parent directory name
    parent_dir_name=$(basename "$(dirname "$subfolder_path")")

    # Get the subfolder name
    subfolder_name=$(basename "$subfolder_path")

    # Construct the desired link name
    link_name="${parent_dir_name} - ${subfolder_name}"

    # Create the symbolic link
    ln -s "$subfolder_path" "$SYMLINK_DEST_DIR/$link_name"
    echo "Created symlink: $SYMLINK_DEST_DIR/$link_name -> $subfolder_path"
done
