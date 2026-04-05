#!/usr/bin/env bash
# Records ~18s of the HistoryRerun + file-history navigation flow:
#   1. Open greet.py in editor (project panel)
#   2. Click HistoryRerun in the editor toolbar → CommitView opens
#   3. Click ← multiple times to walk backwards through the file's history
#   4. Click → to walk forwards
# Video is saved as verify/shots/file_history_traversal.mp4.
set -e

WIN="${1:-58720257}"
OUT="verify/shots/file_history_traversal.mp4"

xdotool windowactivate --sync "$WIN"
xdotool windowsize "$WIN" 1600 1000
xdotool windowmove "$WIN" 50 50
sleep 2

# Start recording.
ffmpeg -y -f x11grab -framerate 20 -video_size 1600x1000 -i :0.0+50,50 \
    -t 22 -c:v libx264 -pix_fmt yuv420p -preset ultrafast "$OUT" \
    >/tmp/ffmpeg.log 2>/tmp/ffmpeg.err </dev/null &
FFMPEG_PID=$!
echo "ffmpeg pid=$FFMPEG_PID"

sleep 1

# Open project panel.
xdotool key --window "$WIN" ctrl+shift+e
sleep 1.5

# Click on greet.py in project panel (approx y=108 in window = screen 158).
xdotool mousemove 100 158
sleep 0.3
xdotool click 1
sleep 2

# Click the HistoryRerun icon in the editor toolbar.
# Coordinates from live_toolbar_zoom: the ↻ icon sits around window
# (1406, 93), i.e. screen (1456, 143).
xdotool mousemove 1456 143
sleep 0.5
xdotool click 1
sleep 3

# Click ← (older) arrow several times. The arrow is at screen (1490, 145).
for _ in 1 2 3 4; do
    xdotool mousemove 1490 145
    sleep 0.3
    xdotool click 1
    sleep 2
done

# Now walk forward a couple times with → (newer).
for _ in 1 2; do
    xdotool mousemove 1520 145
    sleep 0.3
    xdotool click 1
    sleep 2
done

wait "$FFMPEG_PID"
echo "recorded: $OUT ($(stat -c %s "$OUT") bytes)"
