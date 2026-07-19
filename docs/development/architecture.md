# Architecture

ThinkUtils is a **Tauri v2** app with a Rust backend and vanilla JavaScript frontend (no framework).

## Overview

```mermaid
graph LR
    Frontend["🖥️ Frontend\nVanilla JS + HTML + CSS"] -->|"invoke(cmd, args)"| Tauri["⚡ Tauri IPC"]
    Tauri -->|"emit(event, payload)"| Frontend
    Tauri --> Backend["⚙️ Rust Backend\nApiResponse&lt;T&gt;"]
    Backend --> Sysfs["/sys/**"]
    Backend --> Proc["/proc/acpi/ibm/fan"]
    Backend --> Sensors["lm-sensors"]
    Backend --> Pkexec["pkexec"]
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

### Communication Pattern

```mermaid
sequenceDiagram
    participant F as Frontend
    participant T as Tauri IPC
    participant B as Rust Backend
    participant S as Linux System

    F->>T: invoke('set_fan_level', {level: 5})
    T->>B: Call handler
    B->>S: Write to /proc/acpi/ibm/fan
    S-->>B: OK
    B-->>T: ApiResponse { success: true }
    T-->>F: Promise resolves

    Note over B,F: Backend can push events too
    B-)F: emit("fan-update", data)
```

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

```mermaid
flowchart TD
    A["App Launch"] --> B{"Fan helper\ninstalled?"}
    B -- Yes --> C["✅ Fan control ready"]
    B -- No --> D["Show Setup dialog"]
    D --> E["User clicks Setup"]
    E --> F["pkexec — one password prompt"]
    F --> G["Install fan helper"]
    F --> H["Install polkit rule"]
    F --> I["Set sysfs permissions"]
    G & H --> J["✅ Persists across reboots"]
    I --> K["⚠️ Resets on reboot"]
```

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
