# Design System: AraView

> Note: the token names and values in this document came from the original web UI (`App.css`). The GPUI app maps them onto the gpui-kit theme (`app`); the intent, scale, and rules still apply, while CSS-specific mechanics (Tailwind utilities, `@theme`) do not. The `app` theme is the runtime source of truth.

## Overview

**Creative North Star: "The Reading Room"**

A calm, paper-neutral room built for looking, not for clicking. The viewer chrome is a thin frame around the image: quiet surfaces, small precise labels, one ink color doing all the structural work. Nothing glows, nothing gradients, nothing performs. Density is low in the chrome and zero in the reading area, where the picture owns every pixel and the interface gets out of the way (auto-hide during reading).

**Key Characteristics:**

- Paper-neutral surfaces in rest; ink only for structure and primary action.
- Small semibold labels and tabular readouts instead of decorative icon walls.
- Flat by default; shadow appears only on floating layers (dialogs, popovers, error cards).
- Motion is limited to state feedback (press, fade, slide).

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
**Body Font:** GPUI `.SystemUIFont` (on Windows, the system UI family such as Segoe UI). Pretendard is not bundled. Reason in one line: the only typeface in the product must read Korean and Latin equally well at small UI sizes while staying quiet behind images.
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

Single-window app shell: a thin top toolbar (40px tall, 32px buttons with 4px of breathing room between them and above and below them, grouped controls separated by vertical rules), a content well that owns all remaining space, and optional side/bottom layers (EXIF panel, thumbnail strip, nav bar). The toolbar keeps its left and right menus and native caption buttons visible at the 500px minimum window width; labels collapse first, then fit/zoom below 690px, view modes below 840px, and single-image transforms below 1010px. Animation controls live in the bottom overlay, not the toolbar. Button groups do not shrink into the native minimize/maximize/close area. The reading well never scrolls the page itself except in webtoon mode, where vertical scroll is the content. Spacing rhythm is 8px in chrome, 16px in dialogs and panels. Scrollbars are always visible (`ScrollbarMode::Always`, OS 자동 숨김 무시).

### Layer Stack

Lowest to highest: reading content with in-canvas overlays (`z-10`), floating callouts (`z-20`), the thumbnail grid overlay (`z-40`), popovers, tooltips, dialogs (`z-50`), toasts (`z-100`). A dialog scrim covers the whole window at 20% black and never closes the dialog on click; the Windows Snap hit-test overlay is OS-owned and sits above the maximize button outside this scale. While a dialog is open the caption controls do not respond (SPEC §1).

## Elevation & Depth

Flat by default. Depth is conveyed by tonal layering (paper over muted wash) and hairline rules, not shadows.

### Shadow Vocabulary

- **Floating layer** (gpui-kit theme shadow): dialogs, popovers, error cards. Only elements that hover above the reading well may cast it.
- **Field rest** (gpui-kit theme hairline): text inputs at rest.

### Named Rules

- **The Flat-By-Default Rule.** Surfaces are flat at rest. Shadows appear only on floating layers, never on inline chrome.

## Shapes

Softly squared geometry: 10px radius on buttons and toolbar groups, 8px on inputs and small cards, 6px on the smallest controls. Borders are 1px rules, never pill shapes; icon buttons are squares, never circles. The reading well itself is square-cornered so images meet a clean edge.

## Motion

Motion confirms state; it never decorates. GPUI has no CSS transitions. Scroll areas use `SmoothScroll` (`app/src/smooth.rs`: wheel input accumulates to a target offset, frames ease toward it with `TIME_CONSTANT 0.08s`, `SETTLE 0.5px`, `MAX_STEP 0.05s`; native scrolling off via `overflow_hidden`, wheel handled outside).

- **Entrances and exits**: dialogs use `DialogEntrance::Fade`, toasts and grid overlay fade in place.
- **On-screen movement**: virtual lists (grid, license list, dock) share a scroll handle with the content (`SmoothScroll::from_handle`), jump with `scroll_to_item` after `cancel()`; webtoon uses the same easing in `viewport.rs`.
- **Indeterminate progress** is the gpui-kit spinner/progress component; when OS reduced-motion is on it stays static (Windows animation effects).

### Named Rules

- **The Feedback-Only Rule.** If removing the motion removes no information, remove the motion.
- **The Same-Change Gating Rule.** Every motion ships with its OS reduced-motion fallback in the same change, not after.

## App Icon

The app icon is an obangsaek 2x2 grid: four sticky-note tiles in red, ochre, teal, and ink, with the bottom-right tile's outer corner cut away and folded back over the tile (the page-turn). 청・적・황・흑 carry the identity, and 백 (white) is the transparent gap itself.

- **Construction:** 1024 canvas, tile 440, gap 24, tile radius 68, fold 180 measured along the cut diagonal. Tiles at (60,60), (524,60), (60,524); the bottom-right tile is a cut-corner path whose cut corner stays transparent, and the solid fold triangle is mirrored across the diagonal onto the tile. In tile-edge units: radius 15.5%, fold 40.9%.
- **Symmetry:** 60 margin on every side, four 440 tiles, a 24 gap centred on 512, and one 68 radius shared by every corner. The ink tile's corners are circular arcs (`A`), matching the `rx` corners of the other tiles; the fold is the only asymmetry and keeps the main-diagonal mirror.
- **Light (default):** `#D9382B` / `#E5A81C` / `#2E8B7A` / ink `#171717`, fold `#525252`. **Dark:** the first three tiles stay; the bottom-right tile is paper `#F5F5F5` with an ink `#171717` fold, so it survives dark taskbars.
- **Why two variants:** Win32 `.ico` cannot switch with the Windows theme, so each variant is tuned for its own background. The light variant is the bundled default.
- **Small sizes:** verified at every size the app ships in `icon.ico` (16, 24, 32, 48, 64, 256) and at 1024px from `icon.png`, on light and dark backgrounds. The 24-unit gap is 0.4px at 16px, so the tiles fuse there, show a faint seam at 32px, and separate cleanly from 64px up.
- **Sources and regeneration:** `app/resources/icon.ico` is the only embedded icon (`app/resources/app.rc`); `app/resources/icon.png` is the tracked 1024px copy. `site/icon-light.svg` and `site/icon-dark.svg` are website logos with a different grid, not the app icon source. Procedure: `docs/playbooks.md` "Regenerate the app icon".

## Components

### Buttons

Toolbar buttons with a quiet, tactile press (1px downward shift on active, except menu triggers).

- **Shape:** gently squared (10px radius).
- **Primary:** Room Ink fill with paper text, 32px height, 10px horizontal padding.
- **Outline:** paper fill, hairline rule border, used for window-level actions (open, info, settings).
- **Ghost:** borderless, used for in-canvas adjustments (fit, zoom, rotate, flip); hover shows muted wash.
- **Icon:** 32px square ghost/outline for canvas transforms; every icon button carries an accessible label.
- **Icon family:** viewer UI icons are Phosphor (regular weight; fill for the active pin and play/pause), the set the Tauri app used, bundled as SVG in `app/resources/phosphor/` and referenced through the `Ph` enum in `app/src/assets.rs`. Icons that only gpui-kit components draw internally stay on its default Lucide set.
- **Window controls:** minimize, maximize/restore, and close are native-style caption buttons pinned to the top-right corner: 34px wide, full header height (edge to edge, no gaps, square corners, no border), gpui-kit `IconName::WindowMinimize`/`WindowRestore`/`WindowMaximize`/`WindowClose`. Hover shows the muted wash; close hovers to alarm fill with paper glyph. Caption buttons are not focusable app controls; keyboard users close with the OS shortcut (`Alt+F4`). On Windows 11, hovering the maximize/restore button shows the OS Snap Layouts flyout (the maximize button is registered as the window's maximize control area, so Windows handles the hit test natively).
- **Hover / Focus:** muted wash hover; visible focus ring on all variants; disabled at 50% opacity.

### Inputs / Fields

- **Style:** transparent or paper fill, 1px rule stroke, 8px radius, 36px height.
- **Focus:** ring in Focus Ring color.
- **Error / Disabled:** alarm-tinted border with alarm text; disabled at reduced opacity.
- **gpui-kit in use:** settings `Select`/`SearchableVec` (dropdowns), `Kbd` (shortcut badges), `Spinner` (update check), `Alert` (licenses/settings notices), `Form`/`Field` (rename), `TextView::markdown` (update notes), `AlertDialog` (confirmations). Lists use `uniform_list`/`VirtualList`, not the old `List` component.

### Navigation

- **Toolbar:** grouped button clusters with 6-8px gaps and vertical rule separators; the layout responds to window width by collapsing labels first, then lower-priority groups, so it never collides with the caption buttons.
- **Thumbnail strip:** edge-docked (top/bottom/left/right, movable), collapsible as a whole; a gpui-kit `VirtualList` sharing a `SmoothScroll` handle. The strip keeps every image in the folder and loads thumbnails progressively. There is no prev/next button, slider, or `⋯` menu; index jumps use keyboard, grid, or strip selection.
- **Settings dialog:** tabbed (general, view, list, performance, shortcuts, extensions), field-group rhythm, 16px panel padding.

### Overlays

- **Thumbnail grid:** full reading-well overlay (`z-40`), viewport-virtualized, filename filter, `Enter` to jump, `Esc`/`G` to close. Viewer shortcuts are suspended while it is open.
- **Command palette (`Ctrl+K`):** grouped commands (`file → navigate → view → display → system`), token AND matching, English aliases searchable in Korean UI.
- **Peek overlay:** hidden-menu-bar mode hides the header together with app-drawn caption controls. Hovering anywhere across the top 24 logical px reveals both over the reading well; they stay visible within the top 48 logical px and close beyond that band or on window focus loss, never on `Esc` (`Esc` is reserved for closing the image). Returning home or disabling hiding clears the peek state.
- **Auto-hide overlays:** while UI auto-hide is enabled, the header, thumbnail dock (at any edge), and status bar float over the image when revealed. The bottom chrome opens within the bottom 24 logical px across the window and stays visible within the bottom 150 logical px (exclusive boundaries). Revealing or hiding them never resizes or moves the reading well or changes image scale, pan, or webtoon reading position. Bottom and vertical docks stop above the status bar. With auto-hide disabled, the dock and status bar keep their normal flow layout.
- **Dialogs:** the scrim covers the whole window at 20% black (`DIALOG_DIM_HEX`, `dialog.rs::centered`); background click does not close (palette and settings handle `BackdropClick` press/release themselves, the rest close only via `Esc`/buttons). While a dialog is open the caption buttons do not respond (`occlude()`, SPEC §1); no invisible shield may cover them beyond the dialog host.
- **Toasts:** top-right stack (`z-100`), popover surface with floating shadow, destructive icon for failures. A toast never carries a failed load alone; the error card owns that.

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
