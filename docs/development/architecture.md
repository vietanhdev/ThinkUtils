# Architecture

ThinkUtils is a **Tauri v2** app with a Rust backend and vanilla JavaScript frontend (no framework).

## Overview

```
┌─────────────────────────────────────────────┐
│              Frontend (src/)                 │
│  Vanilla JS + HTML + CSS                    │
│  invoke('command', {args})                  │
├───────────────────────��─────────────────────┤
│              Tauri IPC Bridge               │
├─────────────────────────────────────────────┤
│              Backend (src-tauri/src/)        │
│  Rust — system access, hardware control     │
│  All commands return ApiResponse<T>         │
└───────────────────────���─────────────────────┘
```

## Backend (Rust)

All Tauri commands return `ApiResponse<T> { success, data, error }` for consistent error handling. System operations requiring root use **pkexec** (PolicyKit) — the app itself runs unprivileged.

### Modules

| Module | Purpose |
|--------|---------|
| `fan_control.rs` | Manual fan speed via `/proc/acpi/ibm/fan` |
| `fan_curve.rs` | Temperature-based auto fan control (background task, runs every 2s) |
| `battery.rs` | Reads `/sys/class/power_supply/BAT0\|BAT1/` |
| `performance.rs` | CPU governor, turbo boost, power profiles via sysfs |
| `monitor.rs` | System stats (CPU, memory, disk, network, processes) |
| `permissions.rs` | One-time permission setup via pkexec |
| `security.rs` | ClamAV integration |
| `sync.rs` | Google OAuth2 + Drive-based settings backup/restore |
| `settings.rs` | Persistent storage via tauri-plugin-store |
| `system_info.rs` | Hardware information |
| `auth.rs` | OAuth helpers |
| `mcp.rs` | MCP server for AI integration |

### Communication Patterns

- **Frontend → Backend**: `window.__TAURI__.core.invoke('command', {args})`
- **Backend → Frontend**: `app.emit_to("main", "event-name", payload)` — used by the fan curve background task to push temperature/level updates and permission errors

## Frontend (JavaScript)

### Core Modules

| Module | Purpose |
|--------|---------|
| `app.js` | Initialization entry point |
| `state.js` | Centralized state object (current mode, intervals, locks) |
| `dom.js` | Cached DOM element references |
| `navigation.js` | View routing via sidebar `data-feature` attributes |
| `settingsManager.js` | Load/save/apply settings coordination |
| `fanCurve.js` | Canvas-based interactive curve editor with draggable points |
| `templateLoader.js` | HTML template loading |
| `titlebar.js` | Custom window titlebar |

### View Modules (`views/`)

One JS file per feature: `home.js`, `fan.js`, `battery.js`, `performance.js`, `monitor.js`, `system.js`, `security.js`, `sync.js`, `mcp.js`.

## Permission Model

One unified setup (`permissions.rs::setup_permissions()`) handles everything in a single pkexec call:

1. Installs a dedicated fan helper at `/usr/local/bin/thinkutils-fan-control`
2. Installs a polkit rule at `/etc/polkit-1/rules.d/50-thinkutils.rules`
3. Sets sysfs file permissions for CPU/battery control

See [Permissions](/guide/permissions) for user-facing details.

## Development

```bash
npm run tauri dev       # Dev mode with hot reload
npm run tauri build     # Production build
npm run validate        # Lint + format check
cargo test              # Rust tests (from src-tauri/)
```

## Version Bumping

Version must be updated in 4 files before release:
- `package.json`
- `package-lock.json` (2 occurrences at top)
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

After committing, tag with `git tag vX.Y.Z` and push — GitHub Actions builds and publishes release artifacts.
