#!/usr/bin/env bash
# GUI verification harness for the 5 GitFileDiffView UX fixes.
#
# Drives the debug zed build via xdotool/ImageMagick and captures a
# screenshot after each step so the agent can visually inspect tab
# titles, tab counts, panel state, and toolbar buttons. All clicks
# use GLOBAL screen coordinates — xdotool's `--window <id>` flag does
# not deliver events to gpui's input loop on X11 (learned the hard way).
#
# Requirements:
#   - DISPLAY set (X11 only — untested on Wayland)
#   - xdotool, ImageMagick's `import`+`convert`
#   - target/debug/zed built with local changes
#
# Usage:
#   bash verify/git_test_setup.sh      # create the fixture repo
#   bash verify/verify_ui.sh           # run the full loop

set -euo pipefail

cd "$(dirname "$0")/.."

REPO_DIR="/tmp/zed-verify-repo"
SHOTS_DIR="verify/shots"
ZED_BIN="target/debug/zed"
USER_DATA_DIR="/tmp/zed-verify-userdata"

if [[ ! -x "$ZED_BIN" ]]; then
    echo "error: $ZED_BIN missing — run 'cargo build -p zed' first." >&2
    exit 1
fi
if [[ ! -d "$REPO_DIR/.git" ]]; then
    echo "error: $REPO_DIR missing — run 'bash verify/git_test_setup.sh' first." >&2
    exit 1
fi

rm -rf "$SHOTS_DIR"
mkdir -p "$SHOTS_DIR"

# Fresh zed instance.
bash verify/launch_zed.sh >/dev/null
WIN=$(xdotool search --name "zed-verify-repo" | head -1)
if [[ -z "$WIN" ]]; then
    echo "error: zed window not found" >&2
    exit 1
fi
echo "[verify] window=$WIN"

xdotool windowactivate --sync "$WIN"
xdotool windowsize "$WIN" 1600 1000
xdotool windowmove "$WIN" 50 50
sleep 2

# Window-local → screen coords: (x + 50, y + 50).
# Panel row centres observed from reference shots (1600×1000, 50,50 offset):
STAGED_README_XY="135 168"    # row at window-local 85, 118
STAGED_CONFIG_XY="135 196"    # window-local 85, 146
UNSTAGED_README_XY="135 252"  # window-local 85, 202
UNSTAGED_GREET_XY="135 280"   # window-local 85, 230
TOOLBAR_LAST_COMMIT_XY="435 143"  # HistoryRerun icon in diff-view toolbar
CV_OLDER_XY="1490 145"            # ← in CommitViewToolbar
CV_NEWER_XY="1520 145"            # → in CommitViewToolbar

shot() {
    sleep 0.6
    import -window "$WIN" "$SHOTS_DIR/$1.png"
    echo "[verify] $SHOTS_DIR/$1.png"
}

gclick() {
    xdotool mousemove "$1" "$2"
    sleep 0.3
    xdotool click 1
    sleep 1.5
}

gclick_mod() {
    xdotool mousemove "$2" "$3"
    sleep 0.3
    xdotool keydown "$1"
    xdotool click 1
    xdotool keyup "$1"
    sleep 1.5
}

echo "[verify] step 0: baseline"
shot 00_launched

echo "[verify] step 1: open git panel"
xdotool key --window "$WIN" ctrl+shift+g
sleep 1
shot 01_git_panel

echo "[verify] step 2: click STAGED README.md → tab '(Index ↔ HEAD)'"
gclick $STAGED_README_XY
shot 02_staged_click

echo "[verify] step 3: click UNSTAGED README.md → tab '(Working ↔ Index)' replaces"
gclick $UNSTAGED_README_XY
shot 03_unstaged_click

echo "[verify] step 4: dedup — click same entry 3 times, tab stays one"
gclick $UNSTAGED_README_XY
gclick $UNSTAGED_README_XY
gclick $UNSTAGED_README_XY
shot 04_dedup

echo "[verify] step 5: Ctrl+Click on greet.py → raw file tab"
gclick_mod ctrl $UNSTAGED_GREET_XY
shot 05_ctrl_click

echo "[verify] step 6: open diff for greet.py, then Last Commit toolbar"
xdotool key --window "$WIN" ctrl+shift+g
sleep 1
gclick $UNSTAGED_GREET_XY
gclick $TOOLBAR_LAST_COMMIT_XY
shot 06_last_commit

echo "[verify] step 7: navigate to older commit via CommitViewToolbar"
gclick $CV_OLDER_XY
shot 07_older_commit

echo "[verify] done. shots saved under $SHOTS_DIR/"
echo "[verify] crop tab area with:"
echo "         convert $SHOTS_DIR/<N>.png -crop 1400x50+180+45 $SHOTS_DIR/<N>_tabs.png"
