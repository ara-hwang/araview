# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary: Windows 10/11 (x64) desktop users viewing local images. Core job is fast photo/image inspection: open a file, move through a folder, zoom and fit. Secondary job is comic reading (CBZ archives, multi-page spreads, vertical webtoon scroll), including right-to-left reading. Interface languages: Korean and English. Users operate keyboard-first (arrow/page navigation, slideshow, fullscreen) with mouse/touchpad for zoom and pan.

## Product Purpose

A lightweight offline image and comic viewer for Windows. It exists so opening any local image or comic is instant and stays out of the way: no library import, no account, no network. Success means a double-clicked file renders immediately, folder/archive navigation never stalls, and reading modes match the material (single page, spreads, webtoon).

## Positioning

A neighboring viewer cannot truthfully copy this combination: genuinely offline Windows viewer with first-class comic reading (CBZ archives plus single, left-to-right, right-to-left, and webtoon view modes) and a path-based rendering pipeline that keeps large images and archives responsive, including HEIC/HEIF via JPEG sidecar transcode.

## Operating Context

Local-filesystem workflows only: file picker, drag-and-drop of files or folders, and OS file-association launch (Windows passes the path as a CLI argument; backend emits `open-file`). Frameless app window with a custom titlebar/toolbar; window state persists across launches. Viewer chrome auto-hides during reading. Slideshow and fullscreen are presentation contexts. Settings (viewer preferences, recent files) persist locally via Tauri Store. Supported inputs: png, jpg, jpeg, gif, bmp, webp, svg, ico, tiff, tif, avif, heic, heif, psd (read-only preview), cbz, cb7, cbr, rar, zip, 7z, cbt. The only network use is an explicit manual update check (Settings or command palette); there is no background polling.

## Capabilities and Constraints

Confirmed capabilities: zoom, fit-to-width/height/screen, pan, rotate, flip; previous/next navigation, slider jump, thumbnail strip, optional loop navigation; multi-page view modes (single, left-to-right, right-to-left, webtoon); EXIF panel; slideshow; fullscreen; copy image to clipboard as PNG; recent files; Korean/English UI.

Constraints: Windows 10/11 x64 only. Offline by design. File associations can only open the Windows per-extension default-app picker; silent UserChoice registry writes are not possible. HEIC/HEIF renders through a JPEG sidecar under the process temp dir. Backend responses carry file metadata only (`file_path`, `mime_type`, `file_name`, `file_size`); no base64 payloads. Undecided: any cloud or sharing features (out of scope unless explicitly requested).

## Brand Commitments

Name: AraView (Korean: 아라뷰). The technical identifiers stay lowercase (`araview`): npm package, Rust crate, and bundle identifier `com.araview.viewer`. App icon confirmed: obangsaek 2x2 grid with light/dark variants (see `DESIGN.md` App Icon). No other logo, color, voice, or visual assets confirmed; future work must not invent brand claims, testimonials, statistics, or compliance statements.

## Evidence on Hand

No marketing content, testimonials, benchmarks, or press. Real content is the user's own local files; empty states must reflect that (open a file or folder) rather than fabricated samples. Absences future work must not fabricate: user counts, performance claims, reviews.

## Product Principles

1. The image is the interface; chrome defers to content.
2. Keyboard-complete: every viewing and reading action is reachable without a pointer.
3. Offline and instant: no network, no library, no waiting.
4. Reading mode follows the material: single, spread, or webtoon per content, not per habit.
5. Honest states: empty, loading, and error views describe the real filesystem situation.

## Accessibility & Inclusion

Keyboard operability is a product requirement: all viewer controls, navigation, dialogs, and the custom titlebar must be reachable and operable by keyboard with visible focus. Text must meet WCAG AA contrast in both light and dark viewer themes. No additional product-specific accessibility standard confirmed.
