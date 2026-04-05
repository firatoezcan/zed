Here's your autoresearch prompt with the investigation findings baked in:

---

# Autoresearch: Fix Git File Diff View UX Issues

## Objective

Fix 5 UX issues in the single-file git diff view introduced in commits `b74740b3ab` / `e86d824901`. Users click files in the git panel to see diffs, but the current behavior opens wrong-base diffs, duplicates tabs, pollutes tab strip, breaks Ctrl+Click, and lacks toolbar integration for history navigation.

## Files in Scope

- `crates/git_ui/src/git_file_diff_view.rs` — the diff view itself (tab contents, toolbar header, open method)
- `crates/git_ui/src/git_panel.rs` — click handlers at ~5523 and `open_single_file_diff` at 1353
- `crates/git_ui/src/commit_view.rs` — CommitView + CommitViewToolbar (line 975) for adding prev/next arrows
- `crates/git_ui/src/git_ui.rs` — toolbar registration
- `crates/project/src/git_store.rs` — `open_diff_since` at 764 (HEAD-only currently), need index variant
- `crates/workspace/src/pane.rs` — `set_preview_item_id` / `replace_preview_item_id` for preview state

## Off Limits

- Do not touch `FileHistoryView` — user wants it removed from UX, not extended.
- Do not add new crates.

## Constraints

- `./script/clippy` must pass.
- Existing tests in `git_panel.rs` and `commit_view.rs` must pass.
- Screenshots required to verify each fix.

---

## The 5 Issues (with gathered context)

### Issue 1: Wrong diff base for staged vs unstaged entries

**Problem**: Clicking either a staged or unstaged entry in git panel always shows `Working ↔ HEAD`.

**Findings**:

- `git_file_diff_view.rs:73` calls `git_store.open_diff_since(None, ...)` — `None` always means HEAD.
- For an **unstaged** entry click: diff should be `Working ↔ Index` (secondary diff / unstaged hunks only).
- For a **staged** entry click: diff should be `Index ↔ HEAD` (buffer replaced by index content on RHS, HEAD on LHS).
- `git_panel.rs` already has `section_for_selected_entry` helper used by `project_diff.rs`'s `HunkFilter` — reuse that signal.
- `git_store.rs` already has `open_unstaged_diff` (line 824 in open_diff_since) and `open_uncommitted_diff` (line 866). There is no direct `staged ↔ HEAD` diff producer — may need to construct one using `load_committed_text` + `load_staged_text` and build a `BufferDiff` manually, OR swap the multibuffer's buffer to a scratch buffer containing the index content.

**Fix**: Pass a `DiffBase` enum (`WorkingVsIndex` or `IndexVsHead`) into `GitFileDiffView::open` based on the clicked entry's section. Tab title should reflect it: `file.md (Working ↔ Index)` vs `file.md (Index ↔ HEAD)`.

### Issue 2: Multiple clicks open multiple diff tabs

**Problem**: Clicking the same file 3 times → 3 identical diff tabs.

**Findings**:

- `commit_view.rs:149-151` has the exact dedup pattern needed:
  ```rust
  let commit_view = item.downcast::<CommitView>();
  commit_view.is_some_and(|view| view.read(cx).commit.sha == commit_sha)
  ```
- `GitFileDiffView::open` at `git_file_diff_view.rs:79` unconditionally calls `pane.add_item(...)`.

**Fix**: Before `add_item`, iterate `workspace.active_pane().read(cx).items()`, downcast to `GitFileDiffView`, match on `repo_path` **and** diff base. If found, call `pane.activate_item(idx, true, true, ...)` instead.

### Issue 3: Diff tabs should open in preview/peek state

**Problem**: Every diff click permanently adds a tab → bloat.

**Findings**:

- `pane.rs:1064` — `add_item` takes `allow_preview: bool`. Looking at `replace_preview_item_id` (line 1019) and `set_preview_item_id` (line 1034), preview items auto-replace when another preview is opened.
- Current call at `git_file_diff_view.rs:94`: `pane.add_item(Box::new(diff_view.clone()), true, true, None, window, cx)` — this only has 4 bool/option args, so it's `add_item` not `add_item_to_active_pane` — check real signature (may need `Pane::add_item_inner` or caller that accepts `allow_preview`).
- After `add_item`, call `pane.replace_preview_item_id(diff_view.item_id(), window, cx)`.

**Fix**: Open diff views as preview items. Double-click (or explicit "Keep Open" action) promotes them to permanent. Existing Zed convention: double-click on tab promotes preview.

### Issue 4: Ctrl+Click no longer opens file without diff

**Problem**: Ctrl+Click on an entry should open the raw file (as in original Zed), but now does nothing useful.

**Findings**:

- `git_panel.rs:5525`: `if event.modifiers().control { this.toggle_marked_entry(ix); ... return; }` — this was added for multi-select. It **shadows** the original Ctrl+Click behavior.
- Line 5532: `if event.click_count() > 1 || event.modifiers().secondary() { this.open_file(...) }` — `secondary()` **is** Ctrl on Linux/Cmd on macOS, but the control branch above intercepts first.

**Fix**: Move multi-select toggle to **Alt+Click** (`event.modifiers().alt`) so Ctrl+Click falls through to the `secondary()` branch. Update any docs/tooltips.

### Issue 5: "Last Commit" needs toolbar integration + history arrows

**Problem (5a)**: Currently "← Last Commit" is a custom button baked into `GitFileDiffView`'s own header bar. User wants it in the right-side editor toolbar alongside Buffer Search / Inline Assist / breadcrumbs (the standard Zed toolbar with nav arrows).

**Problem (5b)**: When "Last Commit" opens a CommitView, the resulting view has no way to walk backwards through that file's history (HEAD, HEAD~1, HEAD~2 filtered to this file). Screenshot shows a plain commit view with no nav arrows.

**Problem (5c)**: Remove the "All History" button entirely — user explicitly dislikes `FileHistoryView`.

**Findings**:

- `commit_view.rs:975` defines `CommitViewToolbar` implementing `ToolbarItemView` — this is the pattern. It activates when active pane item is CommitView (line 1085: `active_pane_item.and_then(|i| i.act_as::<CommitView>(cx))`).
- `CommitView::open` at `commit_view.rs:102` already takes `file_filter: Option<RepoPath>` (line 107, 125) — so filtering to a single file is already supported.
- Need a parallel `GitFileDiffViewToolbar: ToolbarItemView` registered in `git_ui.rs` that activates for active `GitFileDiffView` and renders prev/next buttons.
- For 5b: extend `CommitViewToolbar` or add buttons to CommitView's toolbar that — when `file_filter.is_some()` — fetch `file_history_paginated(repo, repo_path, offset, Some(1), cx)` and open the next/prev CommitView with the same file filter. Reuse `git_store.file_history_paginated` (already exists, used at git_file_diff_view.rs:186).
- Registration: search for existing toolbar registrations — probably `workspace.register_toolbar_item::<CommitViewToolbar>()` in `git_ui.rs`.

**Fix**:

1. Remove the custom header `div` in `GitFileDiffView::render` (lines 335-374).
2. Remove the "All History" button and `view_file_history` method.
3. Create `GitFileDiffViewToolbar` with "← Last Commit" button → registered in `git_ui.rs`.
4. Extend `CommitViewToolbar` (or the existing pane nav) with ← / → arrows that appear **only when `file_filter.is_some()`**, loading adjacent commits from that file's history.

---

## Verification Plan

Write and use a `git_test_setup.sh` to create a reproducible repo state (one file with staged + unstaged changes, file modification on the README.md). Then manually in the running release build:

1. Click unstaged entry → tab title shows `(Working ↔ Index)`, right pane shows only unstaged hunks.
2. Click staged entry for same file → same tab replaced (preview), title shows `(Index ↔ HEAD)`.
3. Double-click → tab becomes non-preview (italic removed).
4. Ctrl+Click → file opens in regular editor without diff.
5. Alt+Click → row toggles marked state (multi-select).
6. Click "← Last Commit" in toolbar → CommitView opens filtered to that file.
7. In CommitView, click ← → walks through that file's commit history.

Screenshot each step into `/tmp/verify_<N>.png`.

---

## Metrics

- **Primary**: `issues_fixed` (count of 5 issues with passing manual verification, higher is better).
- **Secondary**: `clippy_warnings` (lower is better), `tabs_created_per_click` (should be 0 or 1, lower is better for issue 2).

Save prompt to `autoresearch.md` and begin the loop.
