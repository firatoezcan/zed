#!/usr/bin/env bash
# Counts how many of the 5 issues have their code markers in place.
# Outputs METRIC lines consumed by the autoresearch loop.

set -euo pipefail

cd "$(dirname "$0")"

issues_fixed=0

# Issue 1: GitFileDiffView::open accepts a DiffBase parameter and
#   open_single_file_diff passes derivation from section_for_selected_entry.
if grep -qE '^\s*(pub enum|pub\(crate\) enum|enum)\s+DiffBase' crates/git_ui/src/git_file_diff_view.rs \
   && grep -q 'DiffBase' crates/git_ui/src/git_panel.rs \
   && grep -qE 'section_for_selected_entry' crates/git_ui/src/git_panel.rs; then
    issues_fixed=$((issues_fixed + 1))
fi

# Issue 2: dedup tabs by (repo_path, diff_base). Look for activation of existing
#   GitFileDiffView in open(), mirroring the commit_view.rs:148 pattern.
if grep -Pzo '(?s)\.items\(\).*?downcast::<Self>\(\).*?activate_item' \
       crates/git_ui/src/git_file_diff_view.rs > /dev/null 2>&1; then
    issues_fixed=$((issues_fixed + 1))
fi

# Issue 3: preview tabs. replace_preview_item_id call in git_file_diff_view.
if grep -q 'replace_preview_item_id' crates/git_ui/src/git_file_diff_view.rs; then
    issues_fixed=$((issues_fixed + 1))
fi

# Issue 4: Alt+Click multi-select, Ctrl+Click falls through.
# Look for the shift to alt in the row on_click handler (must NOT have control intercept).
if grep -qE 'modifiers\(\)\.alt' crates/git_ui/src/git_panel.rs \
   && ! grep -qPzo '(?s)\.on_click\(\{\s*cx\.listener\(move \|this, event: &ClickEvent.*?if event\.modifiers\(\)\.control \{\s*this\.toggle_marked_entry' \
         crates/git_ui/src/git_panel.rs; then
    issues_fixed=$((issues_fixed + 1))
fi

# Issue 5: toolbar integration + no view_file_history + CommitViewToolbar file nav.
#  5a: GitFileDiffViewToolbar exists & registered
#  5b: CommitViewToolbar renders file-history nav arrows (conditional on file_filter)
#  5c: view_file_history method removed
markers_5=0
if grep -qE '(pub struct|struct)\s+GitFileDiffViewToolbar' crates/git_ui/src/git_file_diff_view.rs \
   && grep -q 'GitFileDiffViewToolbar' crates/zed/src/zed.rs; then
    markers_5=$((markers_5 + 1))
fi
if grep -qE 'file_filter' crates/git_ui/src/commit_view.rs \
   && grep -qE 'fn\s+(next_file_commit|prev_file_commit|navigate_file_history)' \
              crates/git_ui/src/commit_view.rs; then
    markers_5=$((markers_5 + 1))
fi
if ! grep -q 'fn view_file_history' crates/git_ui/src/git_file_diff_view.rs; then
    markers_5=$((markers_5 + 1))
fi
if [ "$markers_5" -eq 3 ]; then
    issues_fixed=$((issues_fixed + 1))
fi

echo "METRIC issues_fixed=$issues_fixed"
echo "METRIC issue5_submarkers=$markers_5"
