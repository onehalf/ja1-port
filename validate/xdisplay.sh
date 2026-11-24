#!/bin/sh
# Private X display for DOSBox-X so driving the game never touches the user's mouse/focus.
#   validate/xdisplay.sh            -> ensure headless Xvfb on $JA_DISPLAY (default :99)
#   validate/xdisplay.sh --watch    -> Xephyr nested window instead (visible on the desktop, same display number)
#   validate/xdisplay.sh --stop
# Other scripts source xenv.sh, which exports DISPLAY=$JA_DISPLAY.
D=${JA_DISPLAY:-:99}
n=${D#:}
case $1 in
 --stop) pkill -f "(Xvfb|Xephyr) $D( |$)"; exit 0;;
esac
if xdpyinfo -display "$D" >/dev/null 2>&1; then exit 0; fi
if [ "$1" = --watch ]; then
  DISPLAY=${REAL_DISPLAY:-:0} setsid Xephyr "$D" -screen 700x460 -title "JA1 ($D)" -nolisten tcp >/tmp/xephyr$n.log 2>&1 &
else
  setsid Xvfb "$D" -screen 0 700x460x24 -nolisten tcp >/tmp/xvfb$n.log 2>&1 &
fi
for i in $(seq 1 40); do xdpyinfo -display "$D" >/dev/null 2>&1 && exit 0; sleep 0.25; done
echo "display $D did not come up" >&2; exit 1
