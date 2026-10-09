---
name: araview
description: Quiet offline image and comic viewer for Windows; the picture is the interface.
colors:
  ink: "oklch(0.205 0 0)"
  paper: "oklch(1 0 0)"
  paper-ink: "oklch(0.145 0 0)"
  muted-wash: "oklch(0.97 0 0)"
  muted-ink: "oklch(0.556 0 0)"
  rule: "oklch(0.922 0 0)"
  focus-ring: "oklch(0.708 0 0)"
  alarm: "oklch(0.577 0.245 27.325)"
typography:
  body:
    fontFamily: "Pretendard Variable, Pretendard, -apple-system, Segoe UI, Malgun Gothic, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "Pretendard Variable, Pretendard, -apple-system, Segoe UI, Malgun Gothic, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 600
    lineHeight: 1.33
    letterSpacing: "0.05em"
  readout:
    fontFamily: "Pretendard Variable, Pretendard, -apple-system, Segoe UI, Malgun Gothic, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 500
    fontFeature: "tnum"
rounded:
  sm: "0.375rem"
  md: "0.5rem"
  lg: "0.625rem"
spacing:
  sm: "8px"
  md: "16px"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
    rounded: "{rounded.lg}"
    padding: "0 10px"
    height: "32px"
  button-outline:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.paper-ink}"
    rounded: "{rounded.lg}"
    padding: "0 10px"
    height: "32px"
  button-ghost:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.paper-ink}"
    rounded: "{rounded.lg}"
    padding: "0 10px"
    height: "32px"
  button-icon:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.paper-ink}"
    rounded: "{rounded.lg}"
    size: "32px"
  input-field:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.paper-ink}"
    rounded: "{rounded.md}"
    padding: "4px 12px"
    height: "36px"
---

> Note: the token names and values below came from the original web UI (`App.css`). The GPUI app maps them onto the gpui-kit theme (`src-gpui`); the intent, scale, and rules here still apply, while CSS-specific mechanics (Tailwind utilities, `@theme`) do not.

# Design System: AraView

## Overview

**Creative North Star: "The Reading Room"**

A calm, paper-neutral room built for looking, not for clicking. The viewer chrome is a thin frame around the image: quiet surfaces, small precise labels, one ink color doing all the structural work. Nothing glows, nothing gradients, nothing performs. Density is low in the chrome and zero in the reading area, where the picture owns every pixel and the interface gets out of the way (auto-hide during reading).

**Key Characteristics:**

- Paper-neutral surfaces in rest; ink only for structure and primary action.
- Small semibold labels and tabular readouts instead of decorative icon walls.
- Flat by default; shadow appears only on floating layers (dialogs, popovers, error cards).
- Motion is limited to state feedback (press, fade, slide).

## Colors

One ink, one paper, warm-gray washes between. No accent hue exists; emphasis comes from ink weight and placement. Token values here mirror the original web tokens; the CSS is the runtime truth.

### Primary

- **Room Ink** (oklch(0.205 0 0)): primary buttons, active structure, text on paper. Used sparingly; its rarity is the point.

### Neutral

- **Paper** (oklch(1 0 0)): app background, chrome surfaces.
- **Paper Ink** (oklch(0.145 0 0)): body text on paper.
- **Muted Wash** (oklch(0.97 0 0)): hover fills, secondary surfaces, accent fills.
- **Muted Ink** (oklch(0.556 0 0)): secondary text, EXIF section heads. Never on colored fills.
- **Rule** (oklch(0.922 0 0)): borders, dividers, input strokes.
- **Focus Ring** (oklch(0.708 0 0)): visible focus outlines on every control.

### Dark Theme

Dark mode inverts the room: surfaces go ink, text goes paper (the dark theme).

- **Paper** (oklch(0.145 0 0)): app background, chrome surfaces.
- **Paper Ink** (oklch(0.985 0 0)): body text on paper.
- **Room Ink** becomes the light structure color (oklch(0.922 0 0)) with ink text on it, so primary buttons keep paper-on-ink contrast by flipping.
- **Muted Wash** (oklch(0.269 0 0)), **Muted Ink** (oklch(0.708 0 0)), **Rule** (white at 10%), **Focus Ring** (oklch(0.556 0 0)).
- **Alarm** lightens to (oklch(0.704 0.191 22.216)) so red on dark keeps its weight.

### Named Rules

- **The One Ink Rule.** A second hue appears only for alarm states. If a screen needs more color than ink plus wash, the layout is wrong, not the palette.
- **The Alarm Rule.** Alarm red (oklch(0.577 0.245 27.325)) is reserved for destructive actions and load failures, always at low fill (10-20%) with ink-weight text. One exception: the close caption button fills with alarm on hover, following the Windows titlebar convention.
- **The Contrast Rule.** Body text meets WCAG AA against its surface in both themes. High contrast follows the OS (`prefers-contrast: more`, `forced-colors: active`); there is no in-app toggle. Under forced colors the checkerboard falls back to system Canvas and focus uses the system Highlight outline.

## Typography

**Display Font:** none. This product has no marketing surfaces; there is no display role.
**Body Font:** Pretendard Variable (with Pretendard, system fallbacks). Reason in one line: the only typeface in the product must read Korean and Latin equally well at small UI sizes while staying quiet behind images.
**Label/Mono Font:** same family; numerals use tabular figures (`tnum`) for readouts (zoom, page position, dimensions, file sizes, counts, slider values).

**Character:** Restrained grotesk, small sizes, medium weights. Labels earn their place by being scannable, never by being loud.

### Hierarchy

- **Title** (500, 1rem, 1.5): dialog titles only.
- **Body** (400, 0.875rem, 1.5): settings copy, empty/error messages, panel text.
- **Label** (600, 0.75rem, 1.33, 0.05em tracking, uppercase for section heads): EXIF groups, toolbar button labels beside icons.
- **Readout** (500, 0.875rem, tabular numerals): zoom percentage, page position, dimensions, file sizes, counts, slider values.

### Named Rules

- **The Small Type Rule.** Chrome text never exceeds 0.875rem except dialog titles; the image is always the largest thing on screen.
- **The Uppercase Rule.** Uppercase plus wide tracking is allowed only for short section heads (EXIF groups), never for sentences or buttons.
- **The Full-Name Rule.** Long names truncate to one line with the full string in the `title` tooltip; the layout never wraps a filename to make it fit.

## Layout

Single-window app shell: a thin top toolbar (8px padding, grouped controls separated by vertical rules), a content well that owns all remaining space, and optional side/bottom layers (EXIF panel, thumbnail strip, nav bar). The toolbar sheds controls in priority order instead of overlapping the caption buttons: text labels below 1440px, then the view-mode cluster below 1024px, then the rotate/flip cluster below 840px; wide text labels inside those clusters appear only on very wide windows (1950px and up). Even at the 600px minimum window width the remaining controls fit. The reading well never scrolls the page itself except in webtoon mode, where vertical scroll is the content. Spacing rhythm is 8px in chrome, 16px in dialogs and panels.

### Layer Stack

Lowest to highest: reading content with in-canvas overlays (`z-10`), floating callouts (`z-20`), the thumbnail grid overlay (`z-40`), popovers, tooltips, dialogs, and sheets (`z-50`), toasts (`z-100`). Dialog and sheet scrims start at `--header-height` so the caption buttons stay clickable; the Windows Snap hit-test overlay is OS-owned and sits above the maximize button outside this scale.

## Elevation & Depth

Flat by default. Depth is conveyed by tonal layering (paper over muted wash) and hairline rules, not shadows.

### Shadow Vocabulary

- **Floating layer** (Tailwind v4 default `shadow-lg`): dialogs, popovers, error cards. Only elements that hover above the reading well may cast it.
- **Field rest** (Tailwind v4 default `shadow-xs`): text inputs at rest.

### Named Rules

- **The Flat-By-Default Rule.** Surfaces are flat at rest. Shadows appear only on floating layers, never on inline chrome.

## Shapes

Softly squared geometry: 10px radius on buttons and toolbar groups, 8px on inputs and small cards, 6px on the smallest controls. Borders are 1px rules, never pill shapes; icon buttons are squares, never circles. The reading well itself is square-cornered so images meet a clean edge.

## Motion

Motion confirms state; it never decorates. This section states the intent of the motion curves.

- **Entrances and exits** (`ease-motion-out`): dialogs, popovers, toasts, grid overlay.
- **On-screen movement** (`ease-motion-in-out`): drawer slides, the indeterminate progress sweep.
- **Sheets and drawers** (`ease-motion-drawer`): side sheets and bottom sheets. The built-in `ease-out`/`ease-in-out` stay untouched so existing components keep their feel.
- **Durations:** press 100-160ms, tooltips 125-200ms, dropdowns 150-250ms, modals and drawers 200-500ms (code uses `duration-100/150/200` steps).
- **Indeterminate progress** is a left-to-right sweep on a 1.2s loop; under `prefers-reduced-motion` it becomes a static centered bar.

### Named Rules

- **The Feedback-Only Rule.** If removing the motion removes no information, remove the motion.
- **The Same-Change Gating Rule.** Every motion ships with its `prefers-reduced-motion` fallback and `@media (hover: hover) and (pointer: fine)` gating in the same change, not after.

## App Icon

The app icon is an obangsaek 2x2 grid: four sticky-note tiles in red, ochre, teal, and ink, with the bottom-right tile's outer corner cut away and folded back over the tile (the page-turn). The name stays Korean-first after the 아 monogram: 청・적・황・흑 carry the identity, and 백 (white) is the transparent gap itself.

- **Construction:** 1024 canvas, tile 440, gap 24, tile radius 68, fold 180 measured along the cut diagonal. Tiles at (60,60), (524,60), (60,524); the bottom-right tile is a cut-corner path whose cut corner stays transparent, with the solid fold triangle mirrored across the diagonal so it lies on the tile itself. In tile-edge units: radius 15.5%, fold 40.9%.
- **Symmetry:** the grid is exactly mirror-symmetric left-right, up-down, and on both diagonals: 60 margin on every side, four 440 tiles, a 24 gap centred on 512, and one 68 radius shared by every corner. The ink tile's corners are circular arcs (`A`), so they match the `rx` corners of the other three tiles; quadratic Bézier corners are up to 4px tighter at this scale and break that match. The fold is the only asymmetry, and it keeps the main-diagonal mirror because the cut is a 45 degree isosceles triangle sitting on that diagonal.
- **Light (default):** `#D9382B` / `#E5A81C` / `#2E8B7A` / ink `#171717`, fold `#525252`.
- **Dark:** same first three tiles; bottom-right tile is paper `#F5F5F5` with an ink `#171717` fold, so it survives dark taskbars where the ink tile would merge.
- **Why two variants:** Win32 `.ico` cannot switch with the Windows theme, so each variant is tuned for its own background. The light variant is the bundled default (matches the paper-first chrome and the default light taskbar).
- **Small sizes:** verified at every size the bundled set ships (16, 24, 32, 48, 64, 128, 256, 512, 1024) on light and dark backgrounds. The 24-unit gap is 0.4px at 16px, so the four tiles fuse into one four-colour block there, show only a faint seam at 32px, and separate cleanly from 64px up; the cut corner still reads at 16px, and the gray fold triangle holds as a solid silhouette from 32px up.
- **Sources of truth:** `design/icons/icon.svg` (bundled set, light variant), `design/icons/candidates/icon-obangsaek-grid-light.svg`, `design/icons/candidates/icon-obangsaek-grid-dark.svg`. Regenerate the full set from either SVG (square, transparency) or a 1024px PNG of it with `npx --yes @tauri-apps/cli@2.11.4 icon <source>`; the app only embeds `icon.ico`, so copy that one file to `src-gpui/resources/icon.ico` and the site icons from the same SVG. Full reference sets live in `design/icon-output/quiver-obangsaek-grid-light/` and `design/icon-output/quiver-obangsaek-grid-dark/`. The earlier obang mark is kept as history in `src-tauri/icons/candidates/icon-obang-light.svg`, `icon-obang-dark.svg`, `design/icon-output/obang-light/`, and `design/icon-output/obang-dark/`.
- **Switching to dark:** run the icon command above on `design/icons/candidates/icon-obangsaek-grid-dark.svg` in a scratch folder (`-o`), then copy `icon.ico` to `src-gpui/resources/icon.ico`. Switch back the same way with the light SVG. Windows caches icons aggressively; log out/in or rebuild the icon cache to see the change.

## Components

### Buttons

Toolbar buttons with a quiet, tactile press (1px downward shift on active, except menu triggers).

- **Shape:** gently squared (10px radius).
- **Primary:** Room Ink fill with paper text, 32px height, 10px horizontal padding.
- **Outline:** paper fill, hairline rule border, used for window-level actions (open, info, settings).
- **Ghost:** borderless, used for in-canvas adjustments (fit, zoom, rotate, flip); hover shows muted wash.
- **Icon:** 32px square ghost/outline for canvas transforms; every icon button carries an accessible label.
- **Window controls:** minimize, maximize/restore, and close are native-style caption buttons pinned to the top-right corner: 48px wide, full titlebar height (edge to edge, no gaps, square corners, no border), glyphs Minus/Square/Copy/X. Hover shows the muted wash; close hovers to alarm fill with paper glyph; focus uses a 2px inset ring so it is never clipped at the window edge. On Windows 11, hovering the maximize/restore button shows the OS Snap Layouts flyout (the maximize button is registered as the window's maximize control area, so Windows handles the hit test natively).
- **Hover / Focus:** muted wash hover; visible focus ring on all variants; disabled at 50% opacity.

### Inputs / Fields

- **Style:** transparent or paper fill, 1px rule stroke, 8px radius, 36px height.
- **Focus:** ring in Focus Ring color.
- **Error / Disabled:** alarm-tinted border with alarm text; disabled at reduced opacity.

### Navigation

- **Toolbar:** grouped button clusters with 6-8px gaps and vertical rule separators; the layout responds to window width by collapsing labels first, then lower-priority groups, so it never collides with the caption buttons.
- **Thumbnail strip:** edge-docked (top/bottom/left/right, movable), single row with inline prev/next, collapsible as a whole; the strip keeps every image in the folder and loads thumbnails progressively. Its `⋯` menu (dropdown) carries the same dock options as the View settings. There is no slider control; index jumps use keyboard, grid, or strip selection.
- **Settings dialog:** tabbed (general, view, list, performance, shortcuts, extensions), field-group rhythm, 16px panel padding.

### Overlays

- **Thumbnail grid:** full reading-well overlay (`z-40`), viewport-virtualized, filename filter, `Enter` to jump, `Esc`/`G` to close. Viewer shortcuts are suspended while it is open.
- **Command palette (`Ctrl+K`):** grouped commands (`file → navigate → view → display → system`), token AND matching, English aliases searchable in Korean UI.
- **Peek overlay:** hidden-menu-bar mode reveals the header over the reading well on top-edge hover; it closes on focus loss, never on `Esc` (`Esc` is reserved for closing the image).
- **Dialogs and sheets:** scrims start at `--header-height`, focus is trapped (`modal="trap-focus"`), backdrop click and `Esc` dismiss. No invisible shield may cover the caption buttons.
- **Toasts:** bottom-right stack (`z-100`), popover surface with floating shadow, destructive icon for failures. A toast never carries a failed load alone; the error card owns that.

### Viewer Layers (signature)

- **Reading well:** square, borderless, background follows the viewer-background setting (`theme | black | white | checker`); the checker is fixed 20px tiles (`#c7c7c7` on `#ffffff`) in both themes and falls back to system Canvas under forced colors; empty state names a real next action (open a file or folder).
- **Error card:** floating, hairline alarm-tinted border, alarm-ink message, retry or open action. Never a bare toast for a failed load.

## Do's and Don'ts

### Do:

- **Do** let the image be the largest, most saturated thing on every screen.
- **Do** give every icon button a real accessible label and a keyboard path.
- **Do** use tabular numerals for readouts (zoom, page position, dimensions, file sizes, counts, slider values).
- **Do** keep destructive actions at low-fill alarm with ink-weight text.

### Don't:

- **Don't** add gradients, glows, glassmorphism, or background grids to the chrome.
- **Don't** use pill shapes or circular icon buttons.
- **Don't** use uppercase wide-tracked labels outside short section heads.
- **Don't** invent statistics, testimonials, or brand claims anywhere in the product.
- **Don't** introduce icon or font families outside this file without recording the reason here first.
