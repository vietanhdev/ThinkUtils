# Icon Generation

All application icons are generated from a single source file: `src/assets/logo.svg`

## Prerequisites

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get install imagemagick inkscape libicns-utils
```

**macOS:**
```bash
brew install imagemagick inkscape
```

## Usage

```bash
./generate-icons.sh
```

This generates all icons into `src-tauri/icons/`:

| Icon | Size | Purpose |
|------|------|---------|
| icon.png | 512x512 | Main application icon |
| 32x32.png | 32x32 | Small icon |
| 128x128.png | 128x128 | Medium icon |
| 128x128@2x.png | 256x256 | Retina medium icon |
| icon.ico | multi | Windows icon |
| icon.icns | multi | macOS icon |
| tray-icon.png | 32x32 | System tray |
| tray-icon@2x.png | 64x64 | Retina system tray |
| tray-icon-linux.png | 32x32 | Linux tray |
| tray-icon.ico | multi | Windows tray |
| Square*Logo.png | various | Windows Store logos |

## Updating the Logo

1. Edit `src/assets/logo.svg`
2. Run `./generate-icons.sh`
3. All icons regenerate automatically
