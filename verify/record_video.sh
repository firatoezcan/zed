#!/usr/bin/env bash
# Records a ~15s video of the staged-vs-unstaged flow:
#   1. Open git panel
#   2. Click staged README → shows Index ↔ HEAD diff
#   3. Click unstaged README → shows Working ↔ Index diff
# Video is saved as verify/shots/staged_vs_unstaged.mp4.
set -e

WIN="${1:-58720257}"
OUT="verify/shots/staged_vs_unstaged.mp4"

xdotool windowactivate --sync "$WIN"
xdotool windowsize "$WIN" 1600 1000
xdotool windowmove "$WIN" 50 50
sleep 2

# Start recording the window region (50,50 → 1650,1050).
ffmpeg -y -f x11grab -framerate 20 -video_size 1600x1000 -i :0.0+50,50 \
    -t 15 -c:v libx264 -pix_fmt yuv420p -preset ultrafast "$OUT" \
    >/tmp/ffmpeg.log 2>/tmp/ffmpeg.err </dev/null &
FFMPEG_PID=$!
echo "ffmpeg pid=$FFMPEG_PID"

sleep 1

# Open git panel.
xdotool key --window "$WIN" ctrl+shift+g
sleep 1.5

# Click STAGED README.md.
xdotool mousemove 135 168
sleep 0.3
xdotool click 1
sleep 3

# Reopen git panel (the diff stole focus).
xdotool key --window "$WIN" ctrl+shift+g
sleep 1

# Click UNSTAGED README.md.
xdotool mousemove 135 252
sleep 0.3
xdotool click 1
sleep 3

wait "$FFMPEG_PID"
echo "recorded: $OUT ($(stat -c %s "$OUT") bytes)"
