# Coding standards - AraView

Conventions the code does not confess on its own. Read the sections your change touches. Behavior is specified in `SPEC.md`, the visual system in `DESIGN.md`.

## Rust

- `unsafe_op_in_unsafe_fn` is denied and `undocumented_unsafe_blocks` warns: every `unsafe` block carries a `// SAFETY:` comment.
- Shared logic (decode, archives, cache, thumbnails, file operations) lives in `crates/araview-core`; the app in `src-gpui` stays presentation and input. New file or image logic goes in `araview-core/src/ops/` as a sync function.
- Never block the UI thread: run file, decode, and network work in `cx.background_spawn(...)` and apply the result with `update`/`update_in`.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass; fix warnings instead of allowing them.

## GPUI app (`src-gpui`)

- UI text goes through `t("key")` / `t_with(...)` (`src/i18n.rs`); the strings live in `src-gpui/locales/{ko,en}.json`. Add a key to both languages.
- Viewer shortcuts are resolved in the root key handler from the settings map (`src/keys.rs`), not through GPUI's keymap. A new action id goes in `run_action` (`src/app/menu.rs`) so keys, the context menu, and the command palette share one dispatcher.
- Scroll areas that should ease use `SmoothScroll` (`src/smooth.rs`). It turns native scrolling off (`overflow_y_hidden`) and handles the wheel itself, so do not add a second wheel handler to the same element.
- User-visible results go through `window.toast(...)` (`src/toast.rs`), not `push_notification`, so they stay visible when a dialog is open.
- Header and title-bar buttons live in groups that call `.occlude()`; a control outside such a group is treated as part of the window drag area and does not receive clicks.
- Settings keys are camelCase in `settings.json` and match `SPEC.md` §13 (`npm run docs:check` compares them).

## Dialogs

Dialogs and sheets use gpui-kit's `open_dialog`. Dialogs paint above the notification layer; keep dialog bodies at a fixed height (or a `max_h`) so scroll areas inside them have a bounded size.

## Design non-negotiables

- Every data view ships empty, loading, and error states.
- Every control is keyboard reachable and operable with a visible focus indicator.
- UI copy punctuates with comma, period, colon, or parentheses; the em dash (U+2014) stays out.
- Final logos, avatars, and statistics need explicit instructions; until then use honest placeholders.
