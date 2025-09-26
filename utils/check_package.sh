#!/bin/bash

# Check if package name is provided
if [ $# -eq 0 ]; then
    echo "Usage: $0 <package-name>"
    echo "Example: $0 zaino-state"
    exit 1
fi

package_NAME="$1"

# Run all cargo commands for the specified package
set -e  # Exit on first error

echo "Running checks for package: $package_NAME"

cargo check -p "$package_NAME" && \
cargo check --all-features -p "$package_NAME" && \
cargo check --tests -p "$package_NAME" && \
cargo check --tests --all-features -p "$package_NAME" && \
cargo fmt -p "$package_NAME" && \
cargo clippy -p "$package_NAME" #&& \
cargo nextest run -p "$package_NAME"
