# CSS Architecture

## Import Chain

```mermaid
graph TD
    index.html --> styles.css
    styles.css --> common.css["common.css<br/><small>Variables, reset, nav, buttons</small>"]
    styles.css --> layouts.css["layouts.css<br/><small>Reusable layout components</small>"]
    styles.css --> home.css
    styles.css --> fan.css
    styles.css --> battery.css
    styles.css --> performance.css
    styles.css --> monitor.css
    styles.css --> sync.css
    styles.css --> system.css
    styles.css --> security.css
    styles.css --> mcp.css
    styles.css --> dialogs.css
```

## App Layout

```mermaid
block-beta
    columns 5
    Titlebar:5
    Sidebar:1 Content:4

    style Titlebar fill:#333,color:#fff
    style Sidebar fill:#1a1a1a,color:#fff
    style Content fill:#242424,color:#fff
```

The app uses a fixed sidebar + scrollable content area. Each page has its own CSS file scoped to that view.

### Page Layouts

```mermaid
graph TD
    subgraph home["Home (home.css)"]
        H1[System Overview Cards]
        H2[Quick Actions Grid]
        H3[Quick Settings]
        H4[System Info]
        H1 --> H2 --> H3 --> H4
    end

    subgraph fan["Fan Control (fan.css)"]
        direction LR
        F1[Status Sidebar]
        F2[Mode Selection + Slider + Curve Editor]
    end

    subgraph battery["Battery (battery.css)"]
        B1[Battery Info Cards Grid]
        B2[Threshold Controls]
        B3[Battery Tips]
        B1 --> B2
        B1 --> B3
    end

    subgraph monitor["Monitor (monitor.css)"]
        M1[CPU + Memory]
        M2[CPU Cores Grid]
        M3[Disk Usage]
        M4[Network Stats]
        M5[Process Table]
        M1 --> M2 --> M3 --> M4 --> M5
    end
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

```mermaid
graph LR
    A["Desktop<br/>> 1200px"] --> B["Tablet<br/>768–1200px"] --> C["Mobile<br/>< 768px"] --> D["Small Mobile<br/>< 480px"]
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
