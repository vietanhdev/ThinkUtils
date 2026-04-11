# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
# Development
npm run tauri dev              # Run with hot reload + Rust debug build

# Build
npm run tauri build            # Production build (.deb, .rpm, .AppImage)
cargo check                    # Quick Rust compile check (from src-tauri/)
cargo test                     # Run Rust unit tests (from src-tauri/)

# Linting & Formatting
npm run lint                   # Run all linters (ESLint, Stylelint, HTMLHint)
npm run lint:fix               # Auto-fix linting issues
npm run format                 # Format with Prettier
npm run validate               # Lint + format check (run before committing)
```

Pre-commit hooks (Husky + lint-staged) run linters automatically on staged files.

## Version Bumping

Version must be updated in all 4 files before release:
- `package.json` — `"version": "X.Y.Z"`
- `package-lock.json` — `"version": "X.Y.Z"` (2 occurrences at top)
- `src-tauri/Cargo.toml` — `version = "X.Y.Z"`
- `src-tauri/tauri.conf.json` — `"version": "X.Y.Z"`

After committing, tag with `git tag vX.Y.Z` and push the tag — the GitHub Actions workflow builds and publishes release artifacts automatically.

## Architecture

**Tauri v2 app** with Rust backend and vanilla JavaScript frontend (no framework).

### Backend (`src-tauri/src/`)

All Tauri commands return `ApiResponse<T> { success, data, error }` for consistent error handling. System operations requiring root use **pkexec** (PolicyKit) — the app itself runs unprivileged.

Key modules:
- **fan_control.rs** — Manual fan speed via `/proc/acpi/ibm/fan`, permission checks, polkit rule installation
- **fan_curve.rs** — Temperature-based auto fan control with a background task (runs every 2s). Uses `FanCurveState` (Arc<Mutex>) shared state. Falls back to pkexec only when polkit rule is installed (to avoid dialog spam from background task)
- **permissions.rs** — Broader permission setup for sysfs files (CPU governor, battery thresholds). Separate from fan permissions
- **battery.rs** — Reads `/sys/class/power_supply/BAT0|BAT1/`
- **performance.rs** — CPU governor, turbo boost, power profiles via sysfs
- **monitor.rs** — System stats (CPU, memory, disk, network, processes)
- **sync.rs** — Google OAuth2 + Drive-based settings backup/restore
- **security.rs** — ClamAV integration
- **settings.rs** — Persistent storage via tauri-plugin-store (JSON)

### Frontend (`src/js/`)

- **app.js** — Initialization entry point, sets up all views and permissions
- **state.js** — Simple centralized state object (current mode, intervals, locks)
- **dom.js** — Cached DOM element references (avoids repeated getElementById)
- **navigation.js** — View routing via sidebar `data-feature` attributes
- **settingsManager.js** — Load/save/apply settings coordination
- **fanCurve.js** — Canvas-based interactive curve editor with draggable points
- **views/** — One JS file per feature (fan.js, battery.js, performance.js, etc.)

### Communication Pattern

Frontend calls backend via `window.__TAURI__.core.invoke('command', {args})`. Backend pushes async updates via `app.emit_to("main", "event-name", payload)` — used by the fan curve background task to send temperature/level updates and permission errors.

### Permission Model

Two separate permission systems:
1. **Fan permissions** (`fan_control.rs`): Checks direct write to `/proc/acpi/ibm/fan` OR existence of polkit rule at `/etc/polkit-1/rules.d/50-thinkutils.rules`. "Grant Permissions" button installs the polkit rule (one-time).
2. **System permissions** (`permissions.rs`): chmod/chown on sysfs files for CPU/battery control. Runs at startup, persists within boot session.

## Rules

- Never add Claude as co-author in git commits.
