# AraView selective backports

## Deferred display-change notifications

- Source: Zed `crates/gpui_windows/src/events.rs`, revision
  [`e5be9608ddd4cb0f2f179e1c0014f0a3a0337260`](https://github.com/zed-industries/zed/blob/e5be9608ddd4cb0f2f179e1c0014f0a3a0337260/crates/gpui_windows/src/events.rs).
- License: Apache-2.0; the upstream license remains in `LICENSE-APACHE`.
  Upstream contributor/copyright notices remain unchanged.
- AraView change: selectively applied the upstream `Rc<Self>` receivers for
  `handle_move_msg`, `handle_display_change_msg`, and `set_display`, and the
  deferred `display_changed` callback in `set_display`. Added provenance comments
  and an AraView-owned deterministic regression harness. No other upstream
  `events.rs` changes were imported; AraView's frame-replenishment and HDR work
  is retained.
- Reason: Windows sent-message dispatch during an existing GPUI update can
  otherwise invoke a nested display callback. The executor delivers the
  callback after the current update instead.

Regression command (from the repository root, Python 3 and Rust required):

```bash
python3 vendor/gpui-pre-windows/tests/check_display_change.py
```

The harness extracts the production `set_display` method and compiles it with
an in-memory queued executor. It checks non-reentrancy, callback restoration,
and retained state when the owning window reference is dropped before dispatch.
The old synchronous method fails these checks. This is not a test of Win32
message delivery, native window teardown, actual GPUI scheduling, mixed-DPI
monitor movement, or hotplug. Those need the Windows runtime pass.
