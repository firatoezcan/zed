#!/usr/bin/env bash
# Starts ffmpeg screen recording of the zed window. Records up to 120s.
set -e
WIN="${1:-58720257}"
OUT="${2:-verify/shots/file_history_traversal.mp4}"

xdotool windowactivate --sync "$WIN"
xdotool windowsize "$WIN" 1600 1000
xdotool windowmove "$WIN" 50 50
sleep 1

ffmpeg -y -f x11grab -framerate 20 -video_size 1600x1000 -i :0.0+50,50 \
    -t 120 -c:v libx264 -pix_fmt yuv420p -preset ultrafast "$OUT" \
    </dev/null >/tmp/ffmpeg.log 2>/tmp/ffmpeg.err &

echo "ffmpeg pid=$!"
echo "$!" > /tmp/ffmpeg.pid
