#!/bin/sh
# Launch the ORIGINAL JA.EXE in DOSBox-X (built-in debugger) inside tmux session "jax", so the debugger console can
# be scripted (xdump.sh). Same scratch copy as run.sh (validate/run/C). Window is 640x417: a 17 px menu bar on top,
# then the 320x200 game screen doubled -> window y = 2*game_y + 17.
#   validate/run_x.sh [--quit]
# JA1_VER=115m starts the unofficial fan patch 1.15m instead (validate/run115/C, validate/patch115m.sh; #70).
# JA1_CYCLES="fixed 23880" overrides cycles=max (23880 = DOSBox-X menu "486DX2 66MHz", src/gui/menu_callback.cpp).
set -e
. "$(dirname "$0")/xenv.sh"
cd "$(dirname "$0")"
[ "$1" = --quit ] && { tmux kill-session -t jax 2>/dev/null || true; exit 0; }
[ -e run/C/JA.EXE ] || { echo "run validate/run.sh --fresh once to create run/C" >&2; exit 1; }
RUN=run; CONF=../dosbox-x.conf
./patch113b.sh
if [ "$JA1_VER" = 115m ]; then ./patch115m.sh; RUN=run115; CONF=dosbox-x.conf; fi
tmux kill-session -t jax 2>/dev/null || true
./xdisplay.sh $XD_MODE
# ptrace_any.so lets tools/jactl.py read guest memory live from /proc/PID/mem (Yama ptrace_scope=1)
# /usr/bin/dosbox-x has file capabilities (net), which disable LD_PRELOAD and make it non-dumpable: run a plain copy
SO=../build/ptrace_any.so; BIN=../build/dosbox-x
[ -e $SO ] || gcc -shared -fPIC -O2 -o $SO bridge/ptrace_any.c
# Pinned: copied once, never refreshed automatically: save states (build/states/) only load in the exact binary that
# wrote them, so a system DOSBox-X update must not slip in. Deliberate refresh: rm build/dosbox-x, then re-record the
# states (validate/to_tactical.py).
[ -e $BIN ] || cp /usr/bin/dosbox-x $BIN
[ /usr/bin/dosbox-x -nt $BIN ] && echo "note: /usr/bin/dosbox-x is newer than the pinned build/dosbox-x (kept)" >&2
# A shell stays between tmux and dosbox-x ("; exit": no exec): tmux SIGCONTs a pane process that stops, which would
# undo jactl's SIGSTOP freeze; a plain sh parent leaves a stopped child alone.
CYC=; [ -n "$JA1_CYCLES" ] && CYC="-set 'cpu cycles=$JA1_CYCLES'"
tmux new-session -d -s jax -x 200 -y 60 "cd '$PWD/$RUN' && LD_PRELOAD='$PWD/$SO' DISPLAY=$DISPLAY '$PWD/$BIN' -conf $CONF $CYC -fastlaunch; exit \$?"
# A tmux server with window-size latest can shrink the window to 60x29 (a client of that size); then the debugger pane
# is too small and save_xcompat.set_breakpoints never sees "Pr32" (2026-11-06). Pin the size.
tmux set-option -t jax window-size manual 2>/dev/null || true
tmux resize-window -t jax -x 200 -y 60 2>/dev/null || true
