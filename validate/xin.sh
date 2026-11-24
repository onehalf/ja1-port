#!/bin/sh
# Input helper for the DOSBox-X window (run_x.sh), GAME coords (320x200): xin.sh shot NAME | key K.. | click X Y | rclick X Y | altclick X Y | move X Y
. "$(dirname "$0")/xenv.sh"
W=$(xdotool search --name "DOSBox-X" | head -1)
S=$(cd "$(dirname "$0")" && pwd)/shots
case $1 in
 shot) import -window "$W" "$S/$2.png"; echo "$S/$2.png";;
 key) shift; xdotool windowfocus --sync "$W"; xdotool key --window "$W" --delay 150 "$@";;
 click|rclick) b=1; [ $1 = rclick ] && b=3; xdotool windowfocus --sync "$W" mousemove --window "$W" $(( $2*2 )) $(( $3*2+17 )); sleep 0.3; xdotool mousedown $b; sleep 0.15; xdotool mouseup $b;;
altclick) xdotool windowfocus --sync "$W" mousemove --window "$W" $(( $2*2 )) $(( $3*2+17 )); sleep 0.3; xdotool keydown alt; sleep 0.1; xdotool mousedown 1; sleep 0.15; xdotool mouseup 1; sleep 0.1; xdotool keyup alt;;
 move) xdotool windowfocus --sync "$W" mousemove --window "$W" $(( $2*2 )) $(( $3*2+17 ));;
esac
