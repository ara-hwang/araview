# AraView icon candidates (QuiverAI, Arrow 2)

Generated with the hosted QuiverAI MCP server (`https://app.quiver.ai/mcp`, model `arrow-2`).
Every candidate is a drop-in replacement shape for the existing icon flow: 1024 canvas,
transparent background, flat fills only, no gradients or shadows.

**Status:** `quiver-obangsaek-grid` is promoted to the bundled app icon (see `DESIGN.md` App Icon).
The other eight stay as reference directions.

Each folder holds `icon.svg` (the source of truth) and `icon-1024.png` (rasterized preview).
`preview-light.png` and `preview-dark.png` show every candidate at 256, 128, 64, 32 and 16 px
on a white and a dark background.

| Candidate                | Direction        | What it is                                                                                  | Quiver creation                        |
| ------------------------ | ---------------- | ------------------------------------------------------------------------------------------- | -------------------------------------- |
| `quiver-ara-tile`        | amonogram        | Korean-first 아 monogram in a softly squared tile, with a single obangsaek accent.          | `01a10c14-a5f6-7f94-b1e6-a85693a8d211` |
| `quiver-ara-mark`        | amonogram-ink    | Korean ara monogram in ink with no tile: the name as a pure mark.                           | `01a10c1b-cee2-770e-8a96-7c9ee6cd7371` |
| `quiver-obangsaek-grid`  | obangsaek-grid-2 | Obangsaek tile grid, second pass: heavier color block, smaller gaps, page-turn fold.        | `01a10c17-14e8-7e74-a0f3-36604b5d339b` |
| `quiver-obangsaek-frame` | obangsaek-frame  | Obangsaek picture inside an ink ring: the image itself carries the Korean palette.          | `01a10c1c-6ff9-78f1-8b3f-8b0d6b322e62` |
| `quiver-fold-tile`       | fold-tile        | One large tile with a big page-turn fold: the identity gesture, stripped to a single shape. | `01a10c18-c1d3-7122-aa44-94e635b4ecc7` |
| `quiver-ink-frame`       | ink-frame-2      | Monochrome ink frame drawn as a ring, with a minimal image inside.                          | `01a10c17-fa37-7bd4-a236-f15499218cbd` |
| `quiver-page-stack`      | page-stack       | Two stacked pages with a fold: multiple images in one viewer.                               | `01a10c1e-7b93-740f-9795-067484527bd6` |
| `quiver-viewer-window`   | viewer-window    | Two overlapping tiles: a viewer window holding one picture.                                 | `01a10c1d-8a80-79a1-8cf4-7e9b421a0c1b` |
| `quiver-crop-frame`      | crop-frame       | Crop brackets around one color dot: the viewing gesture, drawn as machinery.                | `01a10c20-2833-77ff-a58b-d1f0554c626d` |

## Reading the previews

- Candidates whose mark is mostly ink (monochrome ring, bare monogram, ink page) disappear on a
  dark taskbar the same way `icon-obangsaek-grid-dark.svg` solves it today: they need a paper-on-ink
  twin before they can ship as the bundled `.ico`.
- The 16px row is the honest test. The folded tile and the obangsaek grid keep their shape there;
  the monograms and the framed marks read as a silhouette with the detail gone.

## quiver-obangsaek-grid vs the previous mark (obang)

This candidate is the bundled icon now: `src-tauri/icons/icon.svg` holds it, and the obang mark it
replaced is kept in `design/icon-output/obang-light/icon.svg`. Same family, measured differences,
both on the 1024 artboard:

| Measure                     | Bundled                                                  | quiver-obangsaek-grid                  | At 16px            | At 32px            |
| --------------------------- | -------------------------------------------------------- | -------------------------------------- | ------------------ | ------------------ |
| Tile edge                   | 408                                                      | 440                                    | 6.4 px -> 6.9 px   | 12.8 px -> 13.8 px |
| Gutter between tiles        | 64                                                       | 24                                     | 1.0 px -> 0.4 px   | 2.0 px -> 0.8 px   |
| Outer margin                | 72                                                       | 60                                     | 1.1 px -> 0.9 px   | 2.3 px -> 1.9 px   |
| Mark bounding box           | 880                                                      | 904                                    | 13.8 px -> 14.1 px | 27.5 px -> 28.3 px |
| Tile corner radius          | 48 (11.8% of the edge)                                   | 68 (15.5%)                             | 0.75 px -> 1.06 px | 1.5 px -> 2.1 px   |
| Fold leg (diagonal cut)     | 150 (36.8% of the edge)                                  | 180 (40.9%)                            | 2.3 px -> 2.8 px   | 4.7 px -> 5.6 px   |
| Fold flap position          | fills the cut corner, tip pointing out                   | mirrored inward, tip lying on the tile | 32px and up        | 32px and up        |
| Palette and tile assignment | #D9382B / #E5A81C / #2E8B7A / ink #171717 / flap #525252 | identical                              | same               | same               |

The fold is a different construction, not a different size. The bundled icon cuts the ink tile's
bottom-right corner and fills the cut with the gray flap, so the tile keeps a full square footprint
with a sharp outer corner. The candidate cuts the same corner, leaves it empty, and lays the gray
flap inside the tile: the fold a sheet of paper actually makes. See `compare/compare-fold.png`.

At 16px the tighter gutter (0.4px) closes up and the four tiles read as one four-colour block,
where the bundled icon still shows four separate tiles across a 1px gutter. From 32px up the
candidate is the heavier and softer mark. See `compare/compare-small.png`, `compare/compare-light.png`
and `compare/compare-dark.png`.

Both variants need the same dark twin. The bundled icon solves the dark taskbar by turning the ink
tile into paper (#F5F5F5) with an ink flap; the candidate uses the same five colours and the same
tile assignment, so the same substitution applies without any geometry change.

## Not shipped here

Six further generations from the same session were rejected during review: two monochrome frames
that came back as solid ink blobs, one detailed frame superseded by `quiver-ink-frame`, one panel
grid whose white panels vanish on both backgrounds, one panel grid that repeats
`quiver-obangsaek-grid`, and the looser first pass of the grid.

## Promoting one candidate

Done for `quiver-obangsaek-grid`; the steps stay here for the next one.

1. Copy the chosen `icon.svg` over `src-tauri/icons/candidates/icon-<variant>.svg` and derive the
   dark twin from it (paper tile, ink flap).
2. Rasterize at 1024 and run `npm run tauri icon <svg-or-1024.png>` (writes the full set into
   `src-tauri/icons/`; delete the `ios/` and `android/` folders it emits).
3. Record the choice, the construction numbers, and the variant color values in `DESIGN.md`
   (App Icon section), then keep the reference set in `design/icon-output/<variant>/`.
