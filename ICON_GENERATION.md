# Icon Generation

This project uses a script to generate all application icons from a single source SVG file for consistency.

## Source File

All icons are generated from: `src/assets/logo.svg`

## Generated Icons

The script generates the following icon formats:

### Application Icons
- `icon.png` (512x512) - Main application icon
- `32x32.png` - Small icon
- `128x128.png` - Medium icon
- `128x128@2x.png` (256x256) - Retina medium icon
- `icon.ico` - Windows icon (multi-resolution)
- `icon.icns` - macOS icon (multi-resolution)

### Tray Icons
- `tray-icon.png` (32x32)
- `tray-icon@2x.png` (64x64)
- `tray-icon-linux.png` (32x32)
- `tray-icon.ico` - Windows tray icon

### Windows Store Logos
- Square30x30Logo.png through Square310x310Logo.png
- StoreLogo.png

## Usage

### Prerequisites

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get install imagemagick inkscape libicns-utils
```

**macOS:**
```bash
brew install imagemagick inkscape
```

### Generate Icons

Run the script to regenerate all icons:

```bash
./generate-icons.sh
```

The script will:
1. Check for required dependencies
2. Convert the source SVG to all required formats
3. Output all icons to `src-tauri/icons/`

### Notes

- **Inkscape** is recommended for better quality SVG rendering (optional)
- **ImageMagick** is required (fallback if Inkscape not available)
- **libicns-utils** (Linux) or **iconutil** (macOS) needed for .icns generation

## Modifying the Logo

To update all icons:

1. Edit `src/assets/logo.svg`
2. Run `./generate-icons.sh`
3. All icons will be regenerated automatically

This ensures consistency across all platforms and icon sizes.
