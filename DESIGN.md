# DESIGN.md — Minutes Design System

> Design tokens, patterns, and conventions for the Minutes desktop app (Tauri).

## Color Tokens

```css
/* Backgrounds */
--bg:          #1c1c1e    /* App background */
--bg-elevated: #2c2c2e    /* Cards, panels, overlays */
--bg-hover:    #38383a    /* Hover state */

/* Borders */
--border:      #38383a    /* Default border */
/* Subtle:     rgba(255,255,255,0.1)  — form controls */
/* Divider:    rgba(255,255,255,0.06) — section separators */

/* Text */
--text:           #f5f5f7    /* Primary text */
--text-secondary: #86868b    /* Labels, metadata */
--text-tertiary:  #636366    /* Placeholders, disabled */

/* Accent */
--accent:       #0a84ff    /* Links, active states, primary buttons */
--accent-hover: #409cff    /* Accent hover */

/* Semantic */
--red:     #ff453a                  /* Recording, errors, destructive */
--red-bg:  rgba(255, 69, 58, 0.12) /* Red background tint */
--green:   #30d158                  /* Success, running, complete */
--purple:  #bf5af2                  /* AI responses, ghost context */
```

**Dark mode only.** No light mode. Matches macOS system dark appearance.

## Typography

```css
--font-sans:    -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif
--font-display: ui-serif, 'Iowan Old Style', 'Palatino Linotype', 'Book Antiqua', Georgia, serif
--font-mono:    'SF Mono', 'Menlo', monospace
```

| Use | Font | Size |
|-----|------|------|
| Body text | `--font-sans` | 13px |
| Headings | `--font-display` | 20px (h2), 16px (h3) |
| Code, paths, timestamps | `--font-mono` | 11-12px |
| Meeting titles | `--font-sans` | 14px, weight 600 |
| Metadata (date, duration) | `--font-sans` | 12px, `--text-secondary` |

**Display serif for headings** is a deliberate choice — it distinguishes Minutes from developer tools and signals "this is about human conversations."

## Spacing & Layout

| Token | Value | Use |
|-------|-------|-----|
| Section padding | 16px | Inside cards, panels |
| Gap (items) | 8px | Between list items, inline elements |
| Gap (sections) | 16px | Between sections, cards |
| Page margin | 16px | Window edge to content |

## Radius

| Context | Value |
|---------|-------|
| Default (`--radius`) | 10px |
| Cards, panels | 12px |
| Buttons | 8px |
| Pills, badges | 999px |
| Inline code | 4px |
| Small controls | 6px |
| Large containers (about, settings) | 14-20px |

**Consistent rounding.** 10px default. Rounder = more prominent. Never sharp corners.

## Component Patterns

### Pills / Badges
```
Background: rgba(accent, 0.12)
Text: accent color
Radius: 999px (fully round)
Padding: 2px 8px
Font-size: 11px
```
Used for: status indicators, type labels (Meeting, Memo, Dictation), counts.

### Buttons
```
Primary:   bg: --accent, text: white, radius: 8px
Secondary: bg: rgba(255,255,255,0.06), text: --text, border: rgba(255,255,255,0.1)
Icon-only: 28x28px circle, hover: --bg-hover
```

### Form Controls
```
Background: rgba(255,255,255,0.06)
Border: 1px solid rgba(255,255,255,0.1)
Radius: 8px
Padding: 6px 8px
Font-size: 13px
```

### Meeting List Items
```
Padding: 12px 16px
Border-bottom: 1px solid --border
Hover: --bg-hover
Title: 14px, weight 600, --text
Metadata: 12px, --text-secondary
Action items badge: pill style, --accent or --red for overdue
```

### Overlays / Panels
```
Background: --bg-elevated
Border: 1px solid --border
Backdrop-filter: blur(12px)
Radius: 12-14px
Box-shadow: 0 8px 32px rgba(0,0,0,0.4)
```

## Animation

| Property | Duration | Easing |
|----------|----------|--------|
| Hover transitions | 0.15s | ease |
| Panel open/close | 0.2s | ease-out |
| Fade in | 0.15s | ease |
| Recording pulse | 1s | ease-in-out (infinite) |

**Subtle and fast.** No bounces, no delays. Utility-grade motion.

## Recording States

```
IDLE         → tray icon: default
RECORDING    → tray icon: red dot, pulsing indicator, timer
PROCESSING   → spinner with stage label
COMPLETE     → green check, file path
ERROR        → red indicator, error message, preserved capture path
```

Every state must be visible in every surface (tray, CLI, MCP).

## Iconography

- **No custom icon set.** Use system emoji for status indicators.
- Recording: `●` (red)
- Success: `✓` (green)
- Error: `✗` (red)
- Voice memo: `📱`
- Ghost context: `👻`
- Action item: `☐` / `☑`

## Overlay Exceptions

The dictation overlay (floating HUD) uses its own color tokens that intentionally
diverge from the main app. A transparent, always-on-top floating surface needs
different treatment than in-app panels.

```css
/* Dictation overlay — tauri/src/dictation-overlay.html */
--panel: rgba(28, 28, 34, 0.96);     /* warmer than --bg-elevated */
--panel-edge: rgba(255, 255, 255, 0.08);
--accent: #79a9ff;                    /* softer blue than --accent */
--text-tertiary: rgba(238, 236, 231, 0.32);
```

## Accessibility

- All interactive elements are keyboard-accessible
- Minimum contrast: WCAG AA (4.5:1 for text, 3:1 for large text)
- Focus indicators: 2px solid `--accent` outline
- No information conveyed by color alone — always paired with text or icon

## File Conventions

| Output | Location | Naming |
|--------|----------|--------|
| Meeting transcripts | `~/meetings/` | `YYYY-MM-DD-{slug}.md` |
| Voice memos | `~/meetings/memos/` | `YYYY-MM-DD-{slug}.md` |
| Dictation files | `~/meetings/memos/` | `YYYY-MM-DD-{first-words}.md` |
| Prep briefs | `~/.minutes/preps/` | `YYYY-MM-DD-{person}.prep.md` |
| State/logs | `~/.minutes/` | Various |

All output files: `0600` permissions (owner read/write only).
