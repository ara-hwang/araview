<!-- Title: Conventional Commits prefix (feat, fix, perf, refactor, docs, ci, chore) plus a short summary. -->

## Background

<!-- Why is this change needed? Describe the problem and its cause. Link related issues (e.g. Closes #123). -->

## Changes

<!-- What changed, grouped by area. Mention user-visible behavior first. -->

-

## Verification

<!-- Check what you ran and report the result. Leave unchecked items with a reason. -->

- [ ] `npx tsc --noEmit`, `npm run lint`, `npm run format:check`
- [ ] `npm test`
- [ ] `cargo fmt --check` and `cargo test` (Rust changes only)
- [ ] `npm run docs:check` (extensions, IPC commands, settings keys, plugins, versions)
- [ ] Runtime check in the dev build (`com.araview.viewer.dev`): what was opened, clicked, and observed

## Docs

<!-- Delete this section if nothing applies. -->

- [ ] `SPEC.md` updated for behavior changes (see §22 for which sections)
- [ ] `README.md`, `docs/`, `AGENTS.md` updated where relevant

## Notes for reviewers

<!-- Anything not verified, known limitations, judgment calls, follow-ups. Delete if empty. -->
