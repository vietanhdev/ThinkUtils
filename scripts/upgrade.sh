#!/bin/bash
set -e

echo "=== ThinkUtils Upgrade ==="
echo ""

# Build
echo "Building..."
npm run tauri build

# Find the .deb
DEB=$(ls -t src-tauri/target/release/bundle/deb/*.deb 2>/dev/null | head -1)

if [ -z "$DEB" ]; then
    echo "Error: No .deb package found"
    exit 1
fi

VERSION=$(basename "$DEB" | grep -oP '\d+\.\d+\.\d+')
echo "Installing ThinkUtils v${VERSION}..."
echo ""

sudo dpkg -i "$DEB"

echo ""
echo "✓ ThinkUtils v${VERSION} installed successfully!"
echo "  Restart the app to use the new version."
