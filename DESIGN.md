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

# Design System: AraView

## Overview

**Creative North Star: "The Reading Room"**

A calm, paper-neutral room built for looking, not for clicking. The viewer chrome is a thin frame around the image: quiet surfaces, small precise labels, one ink color doing all the structural work. Nothing glows, nothing gradients, nothing performs. Density is low in the chrome and zero in the reading area, where the picture owns every pixel and the interface gets out of the way (auto-hide during reading).

**Key Characteristics:**

- Paper-neutral surfaces in rest; ink only for structure and primary action.
- Small semibold labels and tabular readouts instead of decorative icon walls.
- Flat by default; shadow appears only on floating layers (dialogs, popovers, error cards).
- Motion is limited to state feedback (press, fade, slide) and the slideshow progress line.

## Colors

One ink, one paper, warm-gray washes between. No accent hue exists; emphasis comes from ink weight and placement.

### Primary

- **Room Ink** (oklch(0.205 0 0)): primary buttons, active structure, text on paper. Used sparingly; its rarity is the point.

### Neutral

- **Paper** (oklch(1 0 0)): app background, chrome surfaces.
- **Paper Ink** (oklch(0.145 0 0)): body text on paper.
- **Muted Wash** (oklch(0.97 0 0)): hover fills, secondary surfaces, accent fills.
- **Muted Ink** (oklch(0.556 0 0)): secondary text, EXIF section heads. Never on colored fills.
- **Rule** (oklch(0.922 0 0)): borders, dividers, input strokes.
- **Focus Ring** (oklch(0.708 0 0)): visible focus outlines on every control.

### Named Rules

- **The One Ink Rule.** A second hue appears only for alarm states. If a screen needs more color than ink plus wash, the layout is wrong, not the palette.
- **The Alarm Rule.** Alarm red (oklch(0.577 0.245 27.325)) is reserved for destructive actions and load failures, always at low fill (10-20%) with ink-weight text. One exception: the close caption button fills with alarm red on hover, following the Windows titlebar convention.

## Typography

**Display Font:** none. This product has no marketing surfaces; there is no display role.
**Body Font:** Pretendard Variable (with Pretendard, system fallbacks). Reason in one line: the only typeface in the product must read Korean and Latin equally well at small UI sizes while staying quiet behind images.
**Label/Mono Font:** same family; numerals use tabular figures (`tnum`) for the zoom readout and sliders.

**Character:** Restrained grotesk, small sizes, medium weights. Labels earn their place by being scannable, never by being loud.

### Hierarchy

- **Title** (500, 1rem, 1.5): dialog titles only.
- **Body** (400, 0.875rem, 1.5): settings copy, empty/error messages, panel text.
- **Label** (600, 0.75rem, 1.33, 0.05em tracking, uppercase for section heads): EXIF groups, toolbar button labels beside icons.
- **Readout** (500, 0.875rem, tabular numerals): zoom percentage, page position, slideshow interval.

### Named Rules

- **The Small Type Rule.** Chrome text never exceeds 0.875rem except dialog titles; the image is always the largest thing on screen.
- **The Uppercase Rule.** Uppercase plus wide tracking is allowed only for short section heads (EXIF groups), never for sentences or buttons.

## Layout

Single-window app shell: a thin top toolbar (8px padding, grouped controls separated by vertical rules), a content well that owns all remaining space, and optional side/bottom layers (EXIF panel, thumbnail strip, nav bar). The toolbar sheds controls in priority order instead of overlapping the caption buttons: text labels below 1440px, the rotate/flip cluster below 840px, the always-on-top pin below 768px, and the zoom readout below 640px. Even at the 600px minimum window width the remaining controls fit. The reading well never scrolls the page itself except in webtoon mode, where vertical scroll is the content. Spacing rhythm is 8px in chrome, 16px in dialogs and panels.

## Elevation & Depth

Flat by default. Depth is conveyed by tonal layering (paper over muted wash) and hairline rules, not shadows.

### Shadow Vocabulary

- **Floating layer** (Tailwind v4 default `shadow-lg`): dialogs, popovers, error cards, floating nav bar. Only elements that hover above the reading well may cast it.
- **Field rest** (Tailwind v4 default `shadow-xs`): text inputs at rest.

### Named Rules

- **The Flat-By-Default Rule.** Surfaces are flat at rest. Shadows appear only on floating layers, never on inline chrome.

## Shapes

Softly squared geometry: 10px radius on buttons and toolbar groups, 8px on inputs and small cards, 6px on the smallest controls. Borders are 1px rules, never pill shapes; icon buttons are squares, never circles. The reading well itself is square-cornered so images meet a clean edge.

## App Icon

The app icon is an obangsaek 2x2 grid: four sticky-note tiles in red, ochre, teal, and ink, with a folded corner on the bottom-right tile (the page-turn). The name stays Korean-first after the 아 monogram: 청・적・황・흑 carry the identity, and 백 (white) is the transparent gap itself.

- **Construction:** 1024 canvas, tile 408, gap 64, tile radius 48, fold 150. Tiles at (72,72), (544,72), (72,544); bottom-right tile is a cut-corner path with a solid fold triangle.
- **Light (default):** `#D9382B` / `#E5A81C` / `#2E8B7A` / ink `#171717`, fold `#525252`.
- **Dark:** same first three tiles; bottom-right tile is paper `#F5F5F5` with an ink `#171717` fold, so it survives dark taskbars where the ink tile would merge.
- **Why two variants:** Win32 `.ico` cannot switch with the Windows theme, so each variant is tuned for its own background. The light variant is the bundled default (matches the paper-first chrome and the default light taskbar).
- **Small sizes:** verified at 16, 32, 64, 128, and 1024 on light and dark backgrounds; the grid stays readable to 16px. The fold disappears at 16px, which is acceptable.
- **Sources of truth:** `src-tauri/icons/icon.svg` (bundled set, light variant), `src-tauri/icons/candidates/icon-obang-light.svg`, `src-tauri/icons/candidates/icon-obang-dark.svg`. Export a 1024px PNG from the SVG, then regenerate the full set with `npm run tauri icon <1024.png>`. Full reference sets live in `design/icon-output/obang-light/` and `design/icon-output/obang-dark/`.
- **Switching to dark:** `npx tauri icon src-tauri/icons/candidates/icon-obang-dark.svg` (writes directly to `src-tauri/icons/`). Switch back the same way with the light SVG. Windows caches icons aggressively; log out/in or rebuild the icon cache if the old mark persists.

## Components

### Buttons

Toolbar buttons with a quiet, tactile press (1px downward shift on active, except menu triggers).

- **Shape:** gently squared (10px radius).
- **Primary:** Room Ink fill with paper text, 32px height, 10px horizontal padding.
- **Outline:** paper fill, hairline rule border, used for window-level actions (open, info, settings).
- **Ghost:** borderless, used for in-canvas adjustments (fit, zoom, rotate, flip); hover shows muted wash.
- **Icon:** 32px square ghost/outline for canvas transforms; every icon button carries an accessible label.
- **Window controls:** minimize, maximize/restore, and close are native-style caption buttons pinned to the top-right corner: 48px wide, full titlebar height (edge to edge, no gaps, square corners, no border), glyphs Minus/Square/Copy/X. Hover shows the muted wash; close hovers to alarm fill with paper glyph; focus uses a 2px inset ring so it is never clipped at the window edge.
- **Hover / Focus:** muted wash hover; visible focus ring on all variants; disabled at 50% opacity.

### Inputs / Fields

- **Style:** transparent or paper fill, 1px rule stroke, 8px radius, 36px height.
- **Focus:** ring in Focus Ring color.
- **Error / Disabled:** alarm-tinted border with alarm text; disabled at reduced opacity.

### Navigation

- **Toolbar:** grouped button clusters with 6-8px gaps and vertical rule separators; the layout responds to window width by collapsing labels first, then lower-priority groups, so it never collides with the caption buttons.
- **Thumbnail strip / slider:** bottom-dwelling, floating, dismissible; page position as tabular readout.
- **Settings dialog:** tabbed (general, extensions), field-group rhythm, 16px panel padding.

### Viewer Layers (signature)

- **Reading well:** square, borderless, background follows the viewer-background setting; empty state names a real next action (open a file or folder).
- **Slideshow progress:** a single 1px linear progress line; linear easing only.
- **Error card:** floating, hairline alarm-tinted border, alarm-ink message, retry or open action. Never a bare toast for a failed load.

## Do's and Don'ts

### Do:

- **Do** let the image be the largest, most saturated thing on every screen.
- **Do** give every icon button a real accessible label and a keyboard path.
- **Do** use tabular numerals for zoom, page, and interval readouts.
- **Do** keep destructive actions at low-fill alarm with ink-weight text.

### Don't:

- **Don't** add gradients, glows, glassmorphism, or background grids to the chrome.
- **Don't** use pill shapes or circular icon buttons.
- **Don't** use uppercase wide-tracked labels outside short section heads.
- **Don't** invent statistics, testimonials, or brand claims anywhere in the product.
- **Don't** introduce icon or font families outside this file without recording the reason here first.
