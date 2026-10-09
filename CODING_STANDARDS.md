# Coding standards - AraView

Conventions the code does not confess on its own. Read the sections your change touches. Behavior is specified in `SPEC.md`, the visual system in `DESIGN.md`.

## TypeScript / React

- Declare shapes with `type`.
- Domain logic lives in hooks in `src/hooks/`; components stay presentational.
- Group Zustand subscriptions with selectors + `useShallow`.
- `src/constants/commands.ts` holds command palette (`Ctrl+K`) commands; Tauri command wrappers live in `src-tauri/src/commands.rs` and their implementations in `src-tauri/crates/araview-core/src/ops/`.

## Component stack (shadcn/ui on `@base-ui/react`)

- `src/components/ui/` primitives are generated: change behavior at the call site.
- Icons are Phosphor (`@phosphor-icons/react`). Newly added shadcn components arrive with `lucide-react` imports: swap them to Phosphor equivalents on arrival.

## Rust / Tauri

- Run blocking work in commands through `run_blocking`.

## Dialogs and Sheets

Every Dialog/Sheet call site passes `modal="trap-focus"`, which keeps the focus trap, `Esc`, outside `aria-hidden`, and backdrop-click dismissal. `modal={true}` makes Base UI render a transparent full-window `InternalBackdrop` that swallows clicks on the titlebar, so minimize/maximize/close would do nothing while a dialog is open.

## Snap Layouts

The Rust `button_id` in `src-tauri/src/lib.rs` and the DOM id of the maximize caption button (`caption-maximize`) stay identical. A native overlay owns the mouse over that button, so its hover wash and tooltip are mirrored in `src/hooks/useSnapLayout.ts` (mechanism: `SPEC.md` §20).

## Browser support

WebView2 is the only frontend runtime. Use Baseline Widely available features without fallbacks; feature-detect Baseline Newly available ones and degrade gracefully. A fallback is custom code of roughly 20 lines or fewer, with no polyfill or compatibility dependency.

## Motion

- Use the `ease-motion-*` utilities from `src/App.css` (`@theme`): `--ease-motion-out` (entrances, exits), `--ease-motion-in-out` (on-screen movement), `--ease-motion-drawer` (sheets, drawers). The built-in `ease-out`/`ease-in-out` stay untouched.
- Durations come from the duration table in `.agents/skills/review-animations/STANDARDS.md`.
- Reduced-motion and hover gating ship in the same change as the motion (`DESIGN.md`, "The Same-Change Gating Rule").

## Design non-negotiables

- Every data view ships empty, loading, and error states.
- Every control is keyboard reachable and operable with a visible focus indicator.
- UI copy punctuates with comma, period, colon, or parentheses; the em dash (U+2014) stays out.
- Final logos, avatars, and statistics need explicit instructions; until then use honest placeholders.
