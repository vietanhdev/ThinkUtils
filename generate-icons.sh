#!/bin/bash

# Icon Generation Script
# Converts src/assets/logo.svg to all required icon formats for Tauri app

set -e

SOURCE_SVG="src/assets/logo.svg"
ICON_DIR="src-tauri/icons"

# Check if ImageMagick is installed
if ! command -v convert &> /dev/null; then
    echo "Error: ImageMagick is not installed."
    echo "Install it with: sudo apt-get install imagemagick (Ubuntu/Debian)"
    echo "or: brew install imagemagick (macOS)"
    exit 1
fi

# Check if Inkscape is installed (better SVG rendering)
USE_INKSCAPE=false
if command -v inkscape &> /dev/null; then
    USE_INKSCAPE=true
    echo "Using Inkscape for SVG conversion (better quality)"
else
    echo "Using ImageMagick for SVG conversion"
    echo "For better quality, install Inkscape: sudo apt-get install inkscape"
fi

# Function to convert SVG to PNG
convert_svg_to_png() {
    local size=$1
    local output=$2
    
    if [ "$USE_INKSCAPE" = true ]; then
        inkscape "$SOURCE_SVG" --export-filename="$output" --export-width=$size --export-height=$size
    else
        convert -background none -resize ${size}x${size} "$SOURCE_SVG" "$output"
    fi
    
    echo "Generated: $output (${size}x${size})"
}

echo "Starting icon generation from $SOURCE_SVG..."
echo "Output directory: $ICON_DIR"
echo ""

# Create icon directory if it doesn't exist
mkdir -p "$ICON_DIR"

# Generate PNG icons for different sizes
echo "Generating PNG icons..."
convert_svg_to_png 32 "$ICON_DIR/32x32.png"
convert_svg_to_png 128 "$ICON_DIR/128x128.png"
convert_svg_to_png 256 "$ICON_DIR/128x128@2x.png"
convert_svg_to_png 512 "$ICON_DIR/icon.png"

# Generate Windows Store logos
echo ""
echo "Generating Windows Store logos..."
convert_svg_to_png 30 "$ICON_DIR/Square30x30Logo.png"
convert_svg_to_png 44 "$ICON_DIR/Square44x44Logo.png"
convert_svg_to_png 50 "$ICON_DIR/Square50x50Logo.png"
convert_svg_to_png 71 "$ICON_DIR/Square71x71Logo.png"
convert_svg_to_png 89 "$ICON_DIR/Square89x89Logo.png"
convert_svg_to_png 107 "$ICON_DIR/Square107x107Logo.png"
convert_svg_to_png 142 "$ICON_DIR/Square142x142Logo.png"
convert_svg_to_png 150 "$ICON_DIR/Square150x150Logo.png"
convert_svg_to_png 284 "$ICON_DIR/Square284x284Logo.png"
convert_svg_to_png 310 "$ICON_DIR/Square310x310Logo.png"
convert_svg_to_png 50 "$ICON_DIR/StoreLogo.png"

# Generate tray icons
echo ""
echo "Generating tray icons..."
convert_svg_to_png 32 "$ICON_DIR/tray-icon.png"
convert_svg_to_png 64 "$ICON_DIR/tray-icon@2x.png"

# Linux tray icon must be exactly 32x32 RGBA (4096 bytes = 32*32*4)
if [ "$USE_INKSCAPE" = true ]; then
    inkscape "$SOURCE_SVG" --export-filename="$ICON_DIR/tray-icon-linux.png" --export-width=32 --export-height=32
else
    convert -background none -resize 32x32! "$SOURCE_SVG" PNG32:"$ICON_DIR/tray-icon-linux.png"
fi
# Ensure proper RGBA format using Python/PIL
if command -v python3 &> /dev/null; then
    python3 << 'PYEOF'
from PIL import Image
img = Image.open('src-tauri/icons/tray-icon-linux.png')
if img.mode != 'RGBA':
    img = img.convert('RGBA')
img.save('src-tauri/icons/tray-icon-linux.png', 'PNG')
PYEOF
fi
echo "Generated: $ICON_DIR/tray-icon-linux.png (32x32 RGBA)"

# Generate .ico file (Windows)
echo ""
echo "Generating Windows .ico file..."
if [ "$USE_INKSCAPE" = true ]; then
    # Generate temporary PNGs for ico creation
    inkscape "$SOURCE_SVG" --export-filename="/tmp/icon-16.png" --export-width=16 --export-height=16
    inkscape "$SOURCE_SVG" --export-filename="/tmp/icon-32.png" --export-width=32 --export-height=32
    inkscape "$SOURCE_SVG" --export-filename="/tmp/icon-48.png" --export-width=48 --export-height=48
    inkscape "$SOURCE_SVG" --export-filename="/tmp/icon-64.png" --export-width=64 --export-height=64
    inkscape "$SOURCE_SVG" --export-filename="/tmp/icon-128.png" --export-width=128 --export-height=128
    inkscape "$SOURCE_SVG" --export-filename="/tmp/icon-256.png" --export-width=256 --export-height=256
    
    convert /tmp/icon-{16,32,48,64,128,256}.png "$ICON_DIR/icon.ico"
    rm /tmp/icon-*.png
else
    convert "$SOURCE_SVG" -background none -define icon:auto-resize=256,128,64,48,32,16 "$ICON_DIR/icon.ico"
fi
echo "Generated: $ICON_DIR/icon.ico"

# Generate tray .ico file
echo "Generating tray .ico file..."
if [ "$USE_INKSCAPE" = true ]; then
    inkscape "$SOURCE_SVG" --export-filename="/tmp/tray-16.png" --export-width=16 --export-height=16
    inkscape "$SOURCE_SVG" --export-filename="/tmp/tray-32.png" --export-width=32 --export-height=32
    convert /tmp/tray-{16,32}.png "$ICON_DIR/tray-icon.ico"
    rm /tmp/tray-*.png
else
    convert "$SOURCE_SVG" -background none -define icon:auto-resize=32,16 "$ICON_DIR/tray-icon.ico"
fi
echo "Generated: $ICON_DIR/tray-icon.ico"

# Generate .icns file (macOS)
echo ""
echo "Generating macOS .icns file..."
if command -v png2icns &> /dev/null; then
    # Use png2icns if available
    convert_svg_to_png 1024 "/tmp/icon-1024.png"
    png2icns "$ICON_DIR/icon.icns" /tmp/icon-1024.png
    rm /tmp/icon-1024.png
    echo "Generated: $ICON_DIR/icon.icns"
elif command -v iconutil &> /dev/null; then
    # Use iconutil on macOS
    ICONSET_DIR="/tmp/icon.iconset"
    mkdir -p "$ICONSET_DIR"
    
    convert_svg_to_png 16 "$ICONSET_DIR/icon_16x16.png"
    convert_svg_to_png 32 "$ICONSET_DIR/icon_16x16@2x.png"
    convert_svg_to_png 32 "$ICONSET_DIR/icon_32x32.png"
    convert_svg_to_png 64 "$ICONSET_DIR/icon_32x32@2x.png"
    convert_svg_to_png 128 "$ICONSET_DIR/icon_128x128.png"
    convert_svg_to_png 256 "$ICONSET_DIR/icon_128x128@2x.png"
    convert_svg_to_png 256 "$ICONSET_DIR/icon_256x256.png"
    convert_svg_to_png 512 "$ICONSET_DIR/icon_256x256@2x.png"
    convert_svg_to_png 512 "$ICONSET_DIR/icon_512x512.png"
    convert_svg_to_png 1024 "$ICONSET_DIR/icon_512x512@2x.png"
    
    iconutil -c icns "$ICONSET_DIR" -o "$ICON_DIR/icon.icns"
    rm -rf "$ICONSET_DIR"
    echo "Generated: $ICON_DIR/icon.icns"
else
    echo "Warning: Neither png2icns nor iconutil found. Skipping .icns generation."
    echo "Install libicns-utils (Linux) or use macOS for .icns generation."
fi

echo ""
echo "✓ Icon generation complete!"
echo "All icons have been generated from $SOURCE_SVG"
