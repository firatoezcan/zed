#!/usr/bin/env bash
# Drives the release zed binary via xdotool to verify each of the 5
# UX fixes. Takes a screenshot after each step. The agent inspects the
# screenshots via its multimodal Read tool.
#
# Requirements:
#   - DISPLAY must be set (X11)
#   - xdotool, import (ImageMagick) installed
#   - target/debug/zed exists (cargo build -p zed --release)

set -euo pipefail

cd "$(dirname "$0")/.."

REPO_DIR="/tmp/zed-verify-repo"
SHOTS_DIR="verify/shots"
ZED_BIN="target/debug/zed"
ZED_LOG="/tmp/zed-verify.log"

if [[ ! -x "$ZED_BIN" ]]; then
    echo "error: $ZED_BIN not found. Run 'cargo build -p zed' first." >&2
    exit 1
fi

if [[ ! -d "$REPO_DIR/.git" ]]; then
    echo "error: test repo missing — run verify/git_test_setup.sh first." >&2
    exit 1
fi

rm -rf "$SHOTS_DIR"
mkdir -p "$SHOTS_DIR"

# Kill any leftover zed from previous runs.
pkill -f "target/debug/zed" 2>/dev/null || true
sleep 1

# Launch zed with the test repo. --foreground keeps the process attached.
echo "[verify] launching zed with $REPO_DIR"
"$ZED_BIN" --foreground "$REPO_DIR" > "$ZED_LOG" 2>&1 &
ZED_PID=$!
echo "[verify] zed pid=$ZED_PID"

cleanup() {
    echo "[verify] killing zed pid=$ZED_PID"
    kill "$ZED_PID" 2>/dev/null || true
    sleep 1
    kill -9 "$ZED_PID" 2>/dev/null || true
}
trap cleanup EXIT

# Wait for the zed window to appear.
echo "[verify] waiting for zed window..."
for _ in {1..30}; do
    WIN=$(xdotool search --name "zed-verify-repo" 2>/dev/null | head -1 || true)
    if [[ -n "$WIN" ]]; then
        break
    fi
    WIN=$(xdotool search --name "Zed" 2>/dev/null | head -1 || true)
    if [[ -n "$WIN" ]]; then
        break
    fi
    sleep 0.5
done

if [[ -z "$WIN" ]]; then
    echo "[verify] ERROR: zed window did not appear within 15s"
    tail -20 "$ZED_LOG" >&2
    exit 2
fi

echo "[verify] zed window id=$WIN"
xdotool windowactivate --sync "$WIN"
xdotool windowsize "$WIN" 1600 1000
xdotool windowmove "$WIN" 50 50
sleep 2

shot() {
    local name="$1"
    local path="$SHOTS_DIR/$name.png"
    # Give zed a moment to finish repainting before capturing.
    sleep 0.6
    import -window "$WIN" "$path"
    echo "[verify] wrote $path"
}

key() {
    xdotool windowactivate --sync "$WIN"
    xdotool key --window "$WIN" "$@"
}

click_at() {
    local x="$1" y="$2"
    local count="${3:-1}"
    xdotool windowactivate --sync "$WIN"
    xdotool mousemove --sync --window "$WIN" "$x" "$y"
    xdotool click --window "$WIN" --repeat "$count" --delay 100 1
}

click_with_modifier() {
    local modifier="$1" x="$2" y="$3"
    xdotool windowactivate --sync "$WIN"
    xdotool mousemove --sync --window "$WIN" "$x" "$y"
    xdotool keydown "$modifier"
    xdotool click --window "$WIN" 1
    xdotool keyup "$modifier"
}

echo "[verify] step 0: baseline"
shot "00_launched"

echo "[verify] step 1: open git panel"
key "ctrl+shift+g"
sleep 1
shot "01_git_panel"

echo "[verify] step 2: click unstaged entry (Working ↔ Index)"
# Guessing panel row positions — will refine after inspecting shot 01.
# The git panel is on the left, sections are Staged then Unstaged.
# We need to locate a row with the filename in the panel.
# For now, just take a shot; the agent reads shots and refines coords.

echo "[verify] done. inspect $SHOTS_DIR/"
