# Autoresearch: Fix Git File Diff View UX Issues

## Objective

Fix 5 UX issues in `GitFileDiffView` (the single-file diff tab that opens when
clicking files in the git panel). Each issue corresponds to a concrete
code marker verifiable with grep. The loop applies fixes, runs clippy + tests,
keeps changes that pass, discards changes that break the build or tests.

## Metrics

- **Primary**: `issues_fixed` (count of the 5 issues with their markers
  present AND checks passing, higher is better). Range 0..=5.
- **Secondary**: `checks_ms` (lower is better), `tests_passed` (higher is
  better), `clippy_warnings` (lower is better).

## How to Run

`bash autoresearch.sh` — outputs `METRIC name=value` lines.
`bash autoresearch.checks.sh` — compiles, tests, and clippy-checks the
affected crates. Exit 0 means green.

## Files in Scope

- `crates/git_ui/src/git_file_diff_view.rs` — the single-file diff view
  (tab contents, toolbar header, `GitFileDiffView::open`).
- `crates/git_ui/src/git_panel.rs` — row click handler at ~5523,
  `open_single_file_diff` at 1353, `section_for_selected_entry` at 1297,
  `Section` enum at 267.
- `crates/git_ui/src/commit_view.rs` — `CommitView::open` (line 102),
  `CommitViewToolbar` (line 975) which receives prev/next file-history
  arrows.
- `crates/git_ui/src/git_ui.rs` — module declarations (`init` at 51).
- `crates/zed/src/zed.rs` — toolbar registration (~line 1239, where
  `CommitViewToolbar` is added). New `GitFileDiffViewToolbar` registers
  here.
- `crates/project/src/git_store.rs` — `open_diff_since` at 764,
  `open_unstaged_diff`, `load_staged_text`, `load_committed_text`.
- `crates/workspace/src/pane.rs` — `replace_preview_item_id` at 1019,
  `set_preview_item_id` at 1034 — called to mark the new tab as preview.

## Off Limits

- `FileHistoryView` and `file_history_view.rs` — user wants it removed from
  UX, not extended. Only remove the references to it in
  `GitFileDiffView` (issue 5c).
- Do not add new crates or change `Cargo.toml` dependencies.

## Constraints

- `./script/clippy -p git_ui -p project -p workspace` must pass (no
  warnings, `-- --deny warnings`).
- Existing tests in `git_panel.rs` and `commit_view.rs` must still pass.
- No `unwrap()` — use `?` or `log_err()`/`match`/`if let`.
- Use full words for variable names.

## The 5 Issues

### Issue 1 — Wrong diff base

Clicking a staged entry should show `Index ↔ HEAD`, clicking unstaged
should show `Working ↔ Index`. Currently both show `Working ↔ HEAD`.

Marker: `GitFileDiffView::open` accepts a `DiffBase` parameter and
`git_panel::open_single_file_diff` passes one derived from
`section_for_selected_entry()`.

### Issue 2 — Duplicate tabs on repeated clicks

Clicking the same file opens new tabs each time. Need dedup on
`(repo_path, diff_base)`.

Marker: `git_file_diff_view.rs` `open` iterates `pane.items()` and
calls `activate_item` on match instead of `add_item`.

### Issue 3 — Tabs should open in preview

Current tabs are permanent. Use Zed's preview-tab convention.

Marker: `replace_preview_item_id` called in `GitFileDiffView::open`.

### Issue 4 — Ctrl+Click broken

`event.modifiers().control` intercepts the row click handler before
`secondary()` gets checked. Ctrl+Click should open file without diff.
Multi-select should move to Alt+Click.

Marker: `git_panel.rs` row `on_click` listener uses
`event.modifiers().alt` for toggle_marked_entry (not `.control`).

### Issue 5 — Toolbar integration + history arrows

- 5a: Move "← Last Commit" to a proper `ToolbarItemView` (like
  `CommitViewToolbar`). Remove the custom header `div` in
  `GitFileDiffView::render` (lines 335-374).
- 5b: In `CommitViewToolbar`, add ← / → arrows that walk the file's
  history when `file_filter.is_some()`.
- 5c: Remove "All History" button + `view_file_history` method entirely.

Marker: `GitFileDiffViewToolbar` struct defined + registered in zed.rs;
`view_file_history` method deleted; `CommitViewToolbar` renders nav
arrows when file_filter is present.

## What's Been Tried

_Populated as experiments run._
