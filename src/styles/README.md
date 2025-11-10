# ThinkUtils CSS Architecture

This directory contains the modular CSS structure for ThinkUtils, organized for better maintainability and scalability.

## File Structure

```
src/styles/
├── common.css       # Common styles, variables, layout, navigation
├── home.css         # Home page specific styles
├── fan.css          # Fan control page styles
├── battery.css      # Battery management page styles
├── performance.css  # Performance/CPU page styles
├── monitor.css      # System monitor page styles
├── sync.css         # Settings sync page styles
├── system.css       # System info page styles
├── security.css     # Security page styles
├── dialogs.css      # Dialog/modal styles
└── README.md        # This file
```

## Main Entry Point

The main `src/styles.css` file imports all modular CSS files:

```css
@import './styles/common.css';
@import './styles/home.css';
@import './styles/fan.css';
/* ... etc */
```

## File Descriptions

### common.css
Contains:
- CSS variables (colors, spacing, shadows)
- Reset styles
- Titlebar styles
- Sidebar navigation
- Page header
- Common buttons and toggles
- Scrollbar styling
- Responsive breakpoints

### Page-Specific Files
Each page has its own CSS file containing only the styles relevant to that page:

- **home.css**: Home dashboard, quick actions, system overview cards
- **fan.css**: Fan control interface, sliders, mode selection, fan curves
- **battery.css**: Battery info cards, threshold controls, battery tips
- **performance.css**: CPU info, governor controls, turbo settings
- **monitor.css**: System monitoring, CPU cores, disk/network stats, process table
- **sync.css**: Google sync interface, user info, sync status
- **system.css**: System information cards and details
- **security.css**: Security scanning interface, threat detection
- **dialogs.css**: Modal dialogs, about dialog, permission dialogs

## CSS Variables

All CSS variables are defined in `common.css` under the `:root` selector:

```css
:root {
  /* Colors */
  --red-primary: #e4002b;
  --bg-primary: #1a1a1a;
  --text-primary: #fff;
  /* ... etc */
}
```

## Adding New Styles

### For a New Page
1. Create a new CSS file in `src/styles/` (e.g., `newpage.css`)
2. Add page-specific styles using existing CSS variables
3. Import it in `src/styles.css`:
   ```css
   @import './styles/newpage.css';
   ```

### For Common Components
Add styles to `common.css` if they're used across multiple pages.

### For Dialogs/Modals
Add styles to `dialogs.css`.

## Best Practices

1. **Use CSS Variables**: Always use CSS variables for colors, spacing, etc.
2. **Scope Styles**: Keep page-specific styles in their respective files
3. **Avoid Duplication**: If a style is used in multiple pages, move it to `common.css`
4. **Naming Convention**: Use descriptive class names (e.g., `.fan-control-card`, `.battery-threshold-slider`)
5. **Responsive Design**: Include responsive breakpoints in the same file as the component
6. **Comments**: Add section comments for better organization

## Responsive Breakpoints

Standard breakpoints used across the app:
- Desktop: > 1200px
- Tablet: 768px - 1200px
- Mobile: < 768px
- Small Mobile: < 480px

## Browser Support

The CSS is designed to work with modern browsers and includes:
- Webkit prefixes for Safari/Chrome
- Mozilla prefixes for Firefox
- Standard CSS properties

## Performance Considerations

- CSS is split into modules for better caching
- Only necessary styles are loaded per page
- CSS variables reduce redundancy
- Transitions and animations use GPU-accelerated properties
