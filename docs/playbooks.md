# Playbooks - AraView

Task-specific procedures for agents, reached from `../AGENTS.md`. Read the section that matches the task. Releases have their own doc: `releasing.md`.

## Add a supported format

1. Add the extension and MIME mapping, with tests, in `src-tauri/crates/araview-core/src/image.rs`
2. Add the extension to `src/constants/imageExtensions.ts`
3. Add the file association to `src-tauri/tauri.conf.json`
4. Add a sample file to `samples/`, generated with tools installed on the PC and outside the repo's dependencies: raster/vector via `sharp` in a temp dir, HEIC/HEIF via Python `pillow-heif`, CBZ via `Compress-Archive`. If the PC has no encoder, install one on the PC.
5. Update `SPEC.md` §2 (and §15/§16/§18/§20 when affected), `README.md`, and `docs/`
6. Done when `npm run docs:check` exits 0 and the sample opens in the dev app: `load_image` succeeds, and for an EXIF-bearing format `get_exif_data` succeeds too

## Add a backend command

1. Implement it as a sync function in the matching `src-tauri/crates/araview-core/src/ops/<domain>.rs`, then add the `#[tauri::command]` wrapper in `src-tauri/src/commands.rs`
2. Register it in `src-tauri/src/lib.rs` `invoke_handler`
3. Update `src/types/index.ts` when the payload or response shape changes
4. Add the row to the `SPEC.md` §15 table and any new type to §16
5. Done when `npm run docs:check` exits 0

## When Tauri MCP looks missing

"Missing" is usually a misdiagnosis. Run these checks in order before concluding anything is broken:

1. Names: the repo name `mcp-server-tauri` is not an npm name. The MCP server is `@hypothesi/tauri-mcp-server` (stdio, prints nothing for `--help`/`--version` by design), the terminal CLI is `@hypothesi/tauri-mcp-cli` (`tauri-mcp` binary, no `--version`), the Rust bridge is `tauri-plugin-mcp-bridge`. Probe with `npm ls -g @hypothesi/tauri-mcp-cli` and `tauri-mcp --help`; `npm view mcp-server-tauri` (404) and `tauri-mcp --version` (unknown option) fail by design.
2. Installed vs connected: the CLI responding means installed. Connecting additionally needs `npm run dev:up` plus `driver-session start --port 9323`. The bridge exists only in dev builds (`dev-mcp` feature, `src-tauri/src/lib.rs`); release builds (`--no-default-features`, used by CI) have no bridge, so a connection failure there is expected.
3. Fresh-session flake: MCP loads at session start and the first `npx -y` download can time out. Repair with `get_setup_instructions` or restart the session.

## After bumping a frontend dependency

Kill the old dev processes, then check the bundle is fresh (`node_modules/.vite/deps` timestamps and `page.url` chunk hashes). `dev:up` is idempotent by port, so a stale Vite server keeps serving the old optimized bundle, and its phantom errors from the previous library version look exactly like a real bug.

## Update vendored skills

`animate` and `review-animations` under `.agents/skills/` are copied from `emilkowalski/skills` (provenance in each `SKILL.md` header comment and `skills-lock.json`), with the upstream "Initial Response" greeting block removed so opencode can auto-invoke them. `npx skills@latest update` overwrites that edit: review its diff and re-apply the greeting-block removal.
