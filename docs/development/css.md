# ThinkUtils CSS Architecture

## Structure Overview

```
src/
├── styles.css (Main entry point — imports all modules)
└── styles/
    ├── common.css          # Variables, reset, layout, navigation, buttons
    ├── layouts.css         # Reusable layout components (cards, headers)
    ├── home.css            # Home page dashboard
    ├── fan.css             # Fan control interface
    ├── battery.css         # Battery management
    ├── performance.css     # CPU/Performance settings
    ├── monitor.css         # System monitoring
    ├── sync.css            # Settings sync
    ├── system.css          # System information
    ├── security.css        # Security features
    ├── mcp.css             # MCP server management
    ├── dialogs.css         # Modal dialogs
    └── README.md           # Quick reference
```

## Import Chain

```
index.html
    └── styles.css
        ├── common.css      (loaded first — contains variables)
        ├── layouts.css     (reusable layout components)
        ├── home.css
        ├── fan.css
        ├── battery.css
        ├── performance.css
        ├── monitor.css
        ├── sync.css
        ├── system.css
        ├── security.css
        ├── mcp.css
        └── dialogs.css
```

## Component Mapping

### Common Components (common.css)
```
┌─────────────────────────────────────┐
│ Titlebar                            │
├─────────────────────────────────────┤
│ ┌──────┐ ┌────────────────────────┐│
│ │      │ │                        ││
│ │ Side │ │   Page Header          ││
│ │ bar  │ │                        ││
│ │      │ ├────────────────────────┤│
│ │      │ │                        ││
│ │      │ │   Content View         ││
│ │      │ │   (page-specific CSS)  ││
│ │      │ │                        ││
│ └──────┘ └────────────────────────┘│
└─────────────────────────────────────┘
```

### Page-Specific Layouts

#### Home Page (home.css)
```
┌─────────────────────────────────────┐
│ System Overview (Compact Cards)     │
├─────────────────────────────────────┤
│ Quick Actions Grid                  │
├─────────────────────────────────────┤
│ Quick Settings                      │
├─────────────────────────────────────┤
│ System Info                         │
└─────────────────────────────────────┘
```

#### Fan Control (fan.css)
```
┌──────────┬──────────────────────────┐
│          │ Fan Mode Selection       │
│  Status  ├──────────────────────────┤
│ Sidebar  │ Manual Speed Slider      │
│          ├──────────────────────────┤
│          │ Fan Curve Editor         │
└──────────┴──────────────────────────┘
```

#### Battery (battery.css)
```
┌─────────────────────────────────────┐
│ Battery Info Cards (Grid)           │
├──────────────────┬──────────────────┤
│ Threshold        │ Battery Tips     │
│ Controls         │                  │
└──────────────────┴──────────────────┘
```

#### Monitor (monitor.css)
```
┌──────────────────┬──────────────────┐
│ CPU Usage        │ Memory Usage     │
├──────────────────┴──────────────────┤
│ CPU Cores Grid                      │
├─────────────────────────────────────┤
│ Disk Usage List                     │
├─────────────────────────────────────┤
│ Network Stats                       │
├─────────────────────────────────────┤
│ Process Table                       │
└─────────────────────────────────────┘
```

## CSS Variables

```css
:root {
  /* Theme Colors */
  --red-primary
  --red-hover
  --red-light
  --red-glow

  /* Backgrounds */
  --bg-primary
  --bg-secondary
  --bg-tertiary
  --bg-elevated
  --bg-card

  /* Text */
  --text-primary
  --text-secondary
  --text-tertiary

  /* Borders & Effects */
  --border-color
  --border-light
  --shadow-sm
  --shadow-md
  --shadow-lg
}
```

## Responsive Breakpoints

```
Desktop (> 1200px)
    ↓
Tablet (768px - 1200px)
    ↓
Mobile (< 768px)
    ↓
Small Mobile (< 480px)
```

## Style Cascade

1. **Browser defaults**
2. **CSS Reset** (common.css)
3. **CSS Variables** (common.css)
4. **Base styles** (common.css)
5. **Layout components** (layouts.css)
6. **Page-specific styles** (individual page CSS)
7. **Responsive overrides** (in each file)

## File Size Breakdown

| File | Lines | Purpose |
|------|-------|---------|
| common.css | ~620 | Shared styles, variables, navigation |
| layouts.css | ~690 | Reusable layout components |
| fan.css | ~715 | Fan control |
| security.css | ~790 | Security features |
| battery.css | ~500 | Battery management |
| home.css | ~470 | Home dashboard |
| monitor.css | ~340 | System monitoring |
| dialogs.css | ~290 | Modal dialogs |
| mcp.css | ~190 | MCP server management |
| sync.css | ~85 | Settings sync |
| performance.css | ~70 | CPU/Performance |
| system.css | ~60 | System information |
| **Total** | **~4,820** | **All styles** |

## Development Workflow

### Adding a New Page
1. Create `src/styles/newpage.css`
2. Add page-specific styles
3. Import in `src/styles.css`:
   ```css
   @import url('./styles/newpage.css');
   ```

### Modifying Existing Styles
1. Identify the page/component
2. Open corresponding CSS file
3. Make changes
4. Test in browser

### Adding Shared Components
1. Add to `common.css` or `layouts.css`
2. Use CSS variables for consistency

## Best Practices

1. **Use CSS Variables**: Always reference variables for colors, spacing
2. **Scope Styles**: Keep page-specific styles in their files
3. **Avoid Duplication**: Move repeated styles to common.css or layouts.css
4. **Naming Convention**: Use descriptive, hierarchical class names (e.g., `.fan-control-card`)
5. **Responsive**: Include breakpoints in the same file as the component
