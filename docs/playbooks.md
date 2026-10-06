# Playbooks - AraView

Task-specific procedures for agents, reached from `../AGENTS.md`. Each section is self-contained: read the one that matches the task. Releases have their own doc: `releasing.md`.

## Add a supported format

1. Add the extension and MIME mapping, with tests, in `src-tauri/src/image.rs`
2. Add the extension to `src/constants/imageExtensions.ts`
3. Add the file association to `src-tauri/tauri.conf.json`
4. Add a sample file to `samples/` and open it in the dev app: `load_image` must succeed, and EXIF-bearing formats also need a `get_exif_data` check. Generate samples with tools installed on the PC, never with repo dependencies: raster/vector via `sharp` in a temp dir, HEIC/HEIF via Python `pillow-heif`, CBZ via `Compress-Archive`. If the PC has no encoder, install one on the PC.
5. Update `SPEC.md` §2 (and §15/§16/§18/§20 when affected), `README.md`, and `docs/`
6. Done when `npm run docs:check` passes and the sample opens

## Add a backend command

1. Implement it in the matching `src-tauri/src/commands/<domain>.rs` with `#[tauri::command]`
2. Register it in `src-tauri/src/lib.rs` `invoke_handler`
3. Update `src/types/index.ts` when the payload or response shape changes
4. Add the row to the `SPEC.md` §15 table and any new type to §16
5. Done when `npm run docs:check` passes

## When Tauri MCP looks missing

"Missing" is usually a misdiagnosis. Check in this order before concluding anything is broken:

1. Names: the repo name `mcp-server-tauri` is not an npm name. The MCP server is `@hypothesi/tauri-mcp-server` (stdio, prints nothing for `--help`/`--version` by design), the terminal CLI is `@hypothesi/tauri-mcp-cli` (`tauri-mcp` binary, no `--version`), the Rust bridge is `tauri-plugin-mcp-bridge`. Probe with `npm ls -g @hypothesi/tauri-mcp-cli` and `tauri-mcp --help`; `npm view mcp-server-tauri` (404) and `tauri-mcp --version` (unknown option) fail by design.
2. Installed vs connected: the CLI responding means installed. Connecting additionally needs `npm run dev:up` plus `driver-session start --port 9323`. The bridge exists only in dev builds (`dev-mcp` feature, `src-tauri/src/lib.rs`); release builds (`--no-default-features`, used by CI) have no bridge, so a connection failure there is expected.
3. Fresh-session flake: MCP loads at session start and the first `npx -y` download can time out. Repair with `get_setup_instructions` or restart the session.

## After bumping a frontend dependency

Kill the old dev processes, then check the bundle is fresh. `dev:up` is idempotent by port, so a stale Vite server keeps serving the old optimized bundle and produces phantom errors from the previous library version. Stale evidence is indistinguishable from a real bug until the served chunk graph is checked (`node_modules/.vite/deps` timestamps and `page.url` chunk hashes).

## Update vendored skills

`animate` and `review-animations` under `.agents/skills/` are copied from `emilkowalski/skills` (provenance in each `SKILL.md` header comment and `skills-lock.json`). Their upstream "Initial Response" greeting block was removed so opencode can auto-invoke them, and `review-animations` keeps its upstream `disable-model-invocation: true` field, which opencode ignores. `npx skills@latest update` overwrites these local edits: review its diff and re-apply the greeting-block removal.
