# ThinkUtils CSS Architecture

## Structure Overview

```
src/
├── styles.css (Main Entry - imports all modules)
└── styles/
    ├── common.css          # Shared: Variables, Layout, Navigation, Buttons
    ├── home.css            # Home page dashboard
    ├── fan.css             # Fan control interface
    ├── battery.css         # Battery management
    ├── performance.css     # CPU/Performance settings
    ├── monitor.css         # System monitoring
    ├── sync.css            # Settings sync
    ├── system.css          # System information
    ├── security.css        # Security features
    ├── dialogs.css         # Modal dialogs
    └── README.md           # Documentation
```

## Import Chain

```
index.html
    └── styles.css
        ├── common.css (loaded first - contains variables)
        ├── home.css
        ├── fan.css
        ├── battery.css
        ├── performance.css
        ├── monitor.css
        ├── sync.css
        ├── system.css
        ├── security.css
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

## CSS Variables Hierarchy

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
5. **Layout components** (common.css)
6. **Page-specific styles** (individual page CSS)
7. **Responsive overrides** (in each file)

## File Size Breakdown

| File | Approximate Lines | Purpose |
|------|------------------|---------|
| common.css | ~600 | Shared styles |
| home.css | ~400 | Home page |
| fan.css | ~500 | Fan control |
| battery.css | ~300 | Battery |
| performance.css | ~200 | Performance |
| monitor.css | ~400 | Monitoring |
| sync.css | ~250 | Sync |
| system.css | ~150 | System info |
| security.css | ~350 | Security |
| dialogs.css | ~250 | Dialogs |
| **Total** | **~3,400** | **All styles** |

## Benefits

### Before (Single File)
- ❌ 3940 lines in one file
- ❌ Hard to find specific styles
- ❌ Merge conflicts
- ❌ Slow to load and parse

### After (Modular)
- ✅ Organized by page/component
- ✅ Easy to locate styles
- ✅ Parallel development
- ✅ Better caching
- ✅ Maintainable

## Development Workflow

### Adding a New Page
1. Create `src/styles/newpage.css`
2. Add page-specific styles
3. Import in `src/styles.css`:
   ```css
   @import './styles/newpage.css';
   ```

### Modifying Existing Styles
1. Identify the page/component
2. Open corresponding CSS file
3. Make changes
4. Test in browser

### Adding Shared Components
1. Add to `common.css`
2. Use CSS variables for consistency
3. Document in comments

## Best Practices

1. **Use CSS Variables**: Always reference variables for colors, spacing
2. **Scope Styles**: Keep page-specific styles in their files
3. **Avoid Duplication**: Move repeated styles to common.css
4. **Naming Convention**: Use descriptive, hierarchical class names
5. **Comments**: Add section headers for organization
6. **Responsive**: Include breakpoints in the same file as the component

## Testing

After making changes:
1. Check all affected pages
2. Test responsive breakpoints
3. Verify no style conflicts
4. Check browser console for errors

## Performance

- CSS modules enable better browser caching
- Only changed files need to be re-downloaded
- Smaller individual files parse faster
- @import is processed at build time
