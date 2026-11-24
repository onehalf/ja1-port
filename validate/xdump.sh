#!/bin/sh
# Dump all 16 MB of the running DOSBox-X guest (run_x.sh) to build/mem_NAME.bin and resume the game.
# Breaks into the debugger (Alt+Pause), runs MEMDUMPBIN on the flat DOS/4GW data selector, presses F5.
# The break must land in protected-mode game code (CS=0180): if it lands in a real-mode interrupt handler,
# "188:0" means segment 0x188 and the dump is garbage, so resume and break again until CS is 0180.
# Read it with tools/memdump.py (maps image addresses to dump offsets: DOS/4GW relocates the LE objects).
#   validate/xdump.sh NAME
set -e
. "$(dirname "$0")/xenv.sh"
cd "$(dirname "$0")"
[ -n "$1" ] || { echo "usage: xdump.sh NAME" >&2; exit 1; }
W=$(xdotool search --name "DOSBox-X" | head -1)
[ -n "$W" ] || { echo "no DOSBox-X window (validate/run_x.sh)" >&2; exit 1; }
ok=
for try in 1 2 3 4 5 6 7 8 9 10; do
  xdotool windowfocus --sync "$W"; xdotool key --window "$W" alt+Pause; sleep 1.5
  # clear the input line with backspaces (Escape would merge with the next key into a Meta-key)
  tmux send-keys -t jax $(printf 'BSpace %.0s' $(seq 1 60))
  # first line of the code overview after its header = current CS:EIP
  cs=$(tmux capture-pane -t jax -p | grep -A1 'Code Overview' | tail -1 | cut -c1-4)
  [ "$cs" = 0180 ] && { ok=1; break; }
  echo "break landed in CS=$cs, retrying" >&2
  tmux send-keys -t jax F5; sleep 0.7
done
[ -n "$ok" ] || { echo "could not break in protected mode" >&2; exit 1; }
# DOSBox-X 2026.10 parses this as selector 188 and writes the dump to a file named ":0 1000000" in its cwd (run/)
rm -f "run/:0 1000000"
tmux send-keys -t jax "MEMDUMPBIN 188:0 1000000" Enter
for i in $(seq 1 40); do [ -s "run/:0 1000000" ] && break; sleep 0.25; done
sleep 1
mv "run/:0 1000000" "../build/mem_$1.bin"
tmux send-keys -t jax F5
echo "build/mem_$1.bin"
