<!-- Title: Conventional Commits prefix (feat, fix, perf, refactor, docs, ci, chore) plus a short summary. -->

## Background

<!-- Why is this change needed? Describe the problem and its cause. Link related issues (e.g. Closes #123). -->

## Changes

<!-- What changed, grouped by area. Mention user-visible behavior first. -->

-

## Verification

<!-- Check what you ran and report the result. Leave unchecked items with a reason. -->

- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `npm run docs:check` (versions, supported extensions, settings keys, leftover plans)
- [ ] Runtime check in the dev build (`com.araview.viewer.dev`): each screen or action as PASS or FAIL, plus the `selftest` result (`pwsh scripts/araview-drive.ps1 selftest`)

## Docs

<!-- Delete this section if nothing applies. -->

- [ ] `SPEC.md` updated for behavior changes (see §22 for which sections)
- [ ] `README.md`, `docs/`, `AGENTS.md`, `CODING_STANDARDS.md` updated where relevant

## Notes for reviewers

<!-- Anything not verified, known limitations, judgment calls, follow-ups. Delete if empty. -->
