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

Session 1 (2026-04-05): all 5 issues fixed in four commits after the
infrastructure commit.

- **Run 1 (keep, `issues_fixed: 0 → 1`)**: Switched the row
  `.on_click` handler in `git_panel.rs` from `modifiers().control` to
  `modifiers().alt` so Ctrl/Cmd+Click falls through to the
  `secondary()` branch and calls `open_file`. ASI: the shadowing
  happened because a later feature (multi-select via `marked_entries`)
  reused the most natural modifier without checking the existing
  Ctrl+Click open-file convention. Rule of thumb: when adding a new
  modifier-based gesture, grep for other handlers on the same element
  first.

- **Run 2 (keep, `issues_fixed: 1 → 4`)**: Issues 1-3 bundled because
  they all touch `GitFileDiffView::open`. Added `DiffBase` enum,
  refactored `open()` to branch on it (unstaged → `open_unstaged_diff`,
  staged → scratch buffer + custom `BufferDiff` vs HEAD), added dedup
  on `(repo_path, diff_base)` before `add_item`, called
  `replace_preview_item_id` so tabs open as preview. Also added
  `Repository::head_and_index_text` helper so the staged path can load
  both texts in one round-trip. ASI: the existing `open_diff_since(None, ...)`
  call was semantically wrong (the `None` sets `set_base_text(None)`,
  which isn't HEAD; the diff that users saw was driven by the secondary
  (unstaged) diff, so the "Working ↔ HEAD" label lied). The scratch
  buffer path for `IndexVsHead` copies the pattern from commit_view's
  `build_buffer_diff`: `BufferDiff::new` + `update_diff().await` +
  `set_snapshot(update, ...).await`.
- **Run 3 (keep, `issues_fixed: 4 → 5`)**: Moved "Last Commit" to a
  new `GitFileDiffViewToolbar` registered in zed.rs. Dropped the
  custom header div and the "All History" button from render. Extended
  `CommitViewToolbar` with prev/next arrows that appear only when the
  underlying `CommitView` has a `file_filter`; each arrow loads the
  file's history via `Repository::file_history_paginated`, finds the
  current SHA's index, and opens the adjacent commit. CommitView now
  stores `file_filter` and `workspace` weak ref. Dedup in
  `CommitView::open` now includes `file_filter` so filtered and
  unfiltered commit views don't clash. ASI: `ToolbarItemView` requires
  `use workspace::ItemHandle` (the trait) not `ItemHandle as _` when
  the trait's name appears in a function signature.

### Pre-existing clippy warning

Fixed a redundant `.clone()` on `this` in the open-file IconButton
handler at `git_panel.rs:5466` (it was the last use of `this`). This
was pre-existing but blocked `./script/clippy`, so it had to go.

### Files Touched

- `crates/git_ui/src/git_file_diff_view.rs` (rewrite: DiffBase, open,
  new, display_title, render, toolbar; removed view_file_history)
- `crates/git_ui/src/git_panel.rs` (Alt+Click, DiffBase in
  `open_single_file_diff`, redundant clone fix)
- `crates/git_ui/src/commit_view.rs` (file_filter +
  workspace fields, file-history navigation)
- `crates/project/src/git_store.rs` (new `head_and_index_text` on
  Repository)
- `crates/zed/src/zed.rs` (register GitFileDiffViewToolbar)

- **Run 4 (keep)**: GUI verification loop via xdotool + ImageMagick
  surfaced two bugs in the preview-tab integration:

  1. `add_item` followed by `replace_preview_item_id` left
     `active_item_index` dangling one past the end (because
     `close_current_preview_item` restores the active index verbatim
     after removing an item at a lower index). Fix: close the existing
     preview first, then add at the returned slot.

  2. `BufferRangesUpdated` events from the multibuffer's diff-hunk
     loading emitted `ItemEvent::Edit`, which `handle_item_edit`
     consumed to un-preview the tab (the default `preserve_preview`
     returns `false`). Fix: override `preserve_preview` to `true` on
     `GitFileDiffView` — it is not user-editable.

  Also dropped a redundant `replace_preview_item_id` in the dedup
  branch of `open()` that was closing the item we just activated.

  ASI: xdotool's `--window <id>` flag does NOT deliver click events
  to gpui's X11 input loop. Use GLOBAL screen coordinates
  (`xdotool mousemove X Y; xdotool click 1`) instead. Same for key
  modifiers — `keydown alt; click; keyup alt` is intercepted by the
  window manager (alt-drag). Ctrl/Cmd modifiers work through
  `keydown ctrl`.

### Verification shots

7-shot canonical flow under `verify/shots/`:

```
00_launched          — zed with /tmp/zed-verify-repo
01_git_panel         — Ctrl+Shift+G opens panel (3 files, staged+unstaged)
02_staged_click      — 'README.md (Index ↔ HEAD)'
03_unstaged_click    — 'README.md (Working ↔ Index)' replaces 02's tab
04_dedup             — 3x click on unstaged → still 1 tab
05_ctrl_click        — Ctrl+Click on greet.py → plain 'greet.py' tab
06_last_commit       — toolbar button → 'fcf2bdd — Add farewell'
07_older_commit      — ← arrow → 'd4afc64 — Initial commit'
```

Repro: `bash verify/git_test_setup.sh && bash verify/verify_ui.sh`
