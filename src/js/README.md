# JavaScript Module Structure

This directory contains the refactored JavaScript code organized into logical modules for better maintainability.

## File Structure

```
src/js/
├── app.js              # Main entry point - initializes the application
├── dom.js              # DOM element references
├── state.js            # Application state management
├── utils.js            # Utility functions (showStatus, etc.)
├── navigation.js       # View switching and navigation
├── titlebar.js         # Window controls (minimize, maximize, close)
├── about.js            # About dialog functionality
└── views/              # Feature-specific view modules
    ├── home.js         # Home dashboard view
    ├── fan.js          # Fan control view
    ├── battery.js      # Battery management view
    ├── performance.js  # Performance tuning view
    ├── monitor.js      # System monitor view
    ├── sync.js         # Cloud sync view
    └── system.js       # System information view
```

## Module Responsibilities

### Core Modules

- **app.js**: Application initialization and lifecycle management
- **dom.js**: Centralized DOM element references to avoid repeated queries
- **state.js**: Global application state (fan mode, intervals, current view)
- **utils.js**: Shared utility functions like status notifications
- **navigation.js**: Handles view switching and menu navigation
- **titlebar.js**: Custom window controls for the frameless window
- **about.js**: About dialog display and interaction

### View Modules

Each view module is self-contained and handles:
- Data fetching from Tauri backend
- UI updates and rendering
- Event handlers specific to that view
- View-specific state management

## Import/Export Pattern

All modules use ES6 imports/exports:

```javascript
// Exporting
export function myFunction() { ... }
export const myVariable = ...;

// Importing
import { myFunction } from './module.js';
```

## Tauri Integration

Tauri invoke calls are accessed via:
```javascript
const { invoke } = window.__TAURI__.core;
```

## Benefits of This Structure

1. **Maintainability**: Each file has a single, clear responsibility
2. **Reusability**: Shared utilities can be imported where needed
3. **Testability**: Individual modules can be tested in isolation
4. **Scalability**: New features can be added as new view modules
5. **Debugging**: Easier to locate and fix issues in specific modules
6. **Collaboration**: Multiple developers can work on different modules

## Migration Notes

The original `main.js` has been backed up as `main.js.backup`. The new modular structure maintains all original functionality while improving code organization.
