#!/usr/bin/env bash

# Build script for compiling spotify-tui to Windows executable
# This script enables cross-compilation from Linux/macOS to Windows

set -e

# Default to GNU target for better Linux cross-compilation compatibility
TARGET="${TARGET:-x86_64-pc-windows-gnu}"
BINARY_NAME="spt.exe"

echo "Building spotify-tui for Windows..."
echo "Target: $TARGET"

# Check if target is installed
if ! rustup target list --installed | grep -q "$TARGET"; then
    echo "Installing Windows target..."
    rustup target add "$TARGET"
fi

# Install cross-compilation dependencies for Windows if needed
if [[ "$TARGET" == *"gnu"* ]]; then
    if ! command -v x86_64-w64-mingw32-gcc &> /dev/null; then
        echo "Warning: mingw-w64 not found. Installing it may be needed:"
        echo "  Ubuntu/Debian: sudo apt-get install mingw-w64"
        echo "  Fedora: sudo dnf install mingw64-gcc"
        echo "  Arch: sudo pacman -S mingw-w64-gcc"
        echo ""
        echo "Attempting to continue anyway..."
    fi
fi

# Build the project
echo "Compiling..."
if ! cargo build --release --target "$TARGET"; then
    echo ""
    echo "✗ Build failed. See error messages above."
    echo ""
    echo "Common solutions:"
    echo "  1. For Linux: Install mingw-w64 (sudo apt-get install mingw-w64)"
    echo "  2. Try setting TARGET=x86_64-pc-windows-msvc for MSVC toolchain"
    echo "  3. See BUILD_WINDOWS.md for detailed troubleshooting"
    exit 1
fi

# Get output path
OUTPUT_PATH="target/$TARGET/release/$BINARY_NAME"

if [ -f "$OUTPUT_PATH" ]; then
    echo ""
    echo "✓ Build successful!"
    echo "Windows executable: $OUTPUT_PATH"
    echo ""
    echo "File size: $(du -h "$OUTPUT_PATH" | cut -f1)"
else
    echo "✗ Build failed - executable not found at $OUTPUT_PATH"
    exit 1
fi
