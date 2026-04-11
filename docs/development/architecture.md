# Architecture

ThinkUtils is a **Tauri v2** app with a Rust backend and vanilla JavaScript frontend (no framework).

## Overview

```mermaid
graph TB
    subgraph Frontend["Frontend (src/)"]
        direction LR
        HTML["HTML + CSS"]
        JS["Vanilla JavaScript"]
    end

    subgraph IPC["Tauri IPC Bridge"]
        Invoke["invoke('command', args)"]
        Emit["emit_to('main', event, payload)"]
    end

    subgraph Backend["Backend (src-tauri/src/)"]
        direction LR
        Rust["Rust Modules"]
        API["ApiResponse&lt;T&gt;"]
    end

    subgraph System["Linux System"]
        direction LR
        Sysfs["/sys/**"]
        Proc["/proc/acpi/ibm/fan"]
        Sensors["lm-sensors"]
        Pkexec["pkexec"]
    end

    Frontend -->|"invoke()"| IPC
    IPC -->|"emit_to()"| Frontend
    IPC --> Backend
    Backend --> System
```

## Backend (Rust)

All Tauri commands return `ApiResponse<T> { success, data, error }` for consistent error handling. System operations requiring root use **pkexec** (PolicyKit) — the app itself runs unprivileged.

### Modules

```mermaid
graph LR
    subgraph Core
        lib.rs["lib.rs<br/><small>Command registration</small>"]
        permissions.rs["permissions.rs<br/><small>One-time setup</small>"]
        settings.rs["settings.rs<br/><small>Persistent storage</small>"]
    end

    subgraph Hardware
        fan_control.rs["fan_control.rs<br/><small>Manual fan speed</small>"]
        fan_curve.rs["fan_curve.rs<br/><small>Auto fan curve</small>"]
        battery.rs["battery.rs<br/><small>Battery info</small>"]
        performance.rs["performance.rs<br/><small>CPU governor</small>"]
    end

    subgraph Monitoring
        monitor.rs["monitor.rs<br/><small>System stats</small>"]
        system_info.rs["system_info.rs<br/><small>Hardware info</small>"]
        security.rs["security.rs<br/><small>ClamAV</small>"]
    end

    subgraph Services
        sync.rs["sync.rs<br/><small>Google Drive</small>"]
        auth.rs["auth.rs<br/><small>OAuth helpers</small>"]
        mcp.rs["mcp.rs<br/><small>MCP server</small>"]
    end
```

### Communication Patterns

```mermaid
sequenceDiagram
    participant F as Frontend (JS)
    participant T as Tauri IPC
    participant B as Backend (Rust)
    participant S as System

    F->>T: invoke('set_fan_level', {level: 5})
    T->>B: Call Rust handler
    B->>S: Write to /proc/acpi/ibm/fan
    S-->>B: OK
    B-->>T: ApiResponse { success: true }
    T-->>F: Promise resolves

    Note over B,F: Backend can also push events
    B->>T: emit_to("main", "fan-update", data)
    T->>F: Event listener fires
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
    A[App Launch] --> B{Helper binary exists?}
    B -->|Yes| C[Fan control works immediately]
    B -->|No| D[Show Setup Permissions dialog]
    D --> E[User clicks Setup]
    E --> F["pkexec runs setup script (one password prompt)"]
    F --> G[Install fan helper binary]
    F --> H[Install polkit rule]
    F --> I[Set sysfs permissions]
    G --> C
    H --> C
    I --> J[CPU/battery controls work]
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
