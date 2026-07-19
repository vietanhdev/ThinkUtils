# ThinkUtils CSS

Modular CSS architecture. See [docs/development/css.md](../../docs/development/css.md) for full documentation.

## File Structure

```
styles/
├── common.css       # Variables, reset, layout, navigation, buttons
├── layouts.css      # Reusable layout components (cards, headers)
├── home.css         # Home page
├── fan.css          # Fan control
├── battery.css      # Battery management
├── performance.css  # CPU/Performance
├── monitor.css      # System monitor
├── sync.css         # Settings sync
├── system.css       # System info
├── security.css     # Security
├── mcp.css          # MCP server
└── dialogs.css      # Modal dialogs
```

All files are imported via `src/styles.css`. To add a new page, create a CSS file here and add an `@import` in `styles.css`.
