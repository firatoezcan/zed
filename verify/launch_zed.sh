#!/usr/bin/env bash
# Launch zed (debug build) pointed at the test repo, using an isolated
# user-data-dir so it doesn't touch the user's main zed state or share
# a running instance. Outputs the PID and window id.
set -e

# Nuke any previous verify-zed instances (but NOT the user's main zed).
pkill -f "zed-verify-userdata" || true
sleep 1

rm -rf /tmp/zed-verify-userdata
mkdir -p /tmp/zed-verify-userdata/config
cat > /tmp/zed-verify-userdata/config/settings.json <<'EOF'
{
  "git_panel": {
    "single_file_diff": true
  },
  "telemetry": {
    "diagnostics": false,
    "metrics": false
  },
  "preview_tabs": {
    "enabled": true
  },
  "session": {
    "trust_all_worktrees": true
  }
}
EOF

ZED_VERIFY_SENTINEL=zed-verify-userdata \
    nohup target/debug/zed \
        --user-data-dir /tmp/zed-verify-userdata \
        /tmp/zed-verify-repo \
        >/tmp/zed-verify.log 2>/tmp/zed-verify.err </dev/null &
PID=$!
echo "zed pid=$PID"
disown || true

# Give zed time to start and register its window.
for _ in $(seq 1 40); do
    sleep 0.5
    # Find windows owned by this PID tree.
    WIN=$(xdotool search --pid "$PID" 2>/dev/null | head -1 || true)
    if [[ -n "$WIN" ]]; then
        # Make sure the window has a name (i.e. is ready).
        NAME=$(xdotool getwindowname "$WIN" 2>/dev/null || true)
        if [[ -n "$NAME" ]]; then
            break
        fi
    fi
done

if [[ -z "${WIN:-}" ]]; then
    echo "ERROR: zed window not found within 20s" >&2
    head -20 /tmp/zed-verify.err >&2 || true
    exit 1
fi

echo "zed window id=$WIN name=\"$NAME\""

xdotool windowactivate --sync "$WIN" || true
xdotool windowsize "$WIN" 1600 1000 || true
xdotool windowmove "$WIN" 50 50 || true
echo "WIN=$WIN" > /tmp/zed-verify-win
