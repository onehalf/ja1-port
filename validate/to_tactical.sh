#!/bin/sh
# Start the original in DOSBox-X (run_x.sh) and drive it to tactical in home sector 60 from the QICKSTRT day-0 save.
# Route documented in REVERSE.md (Next Tasks, "Route to tactical"). Ends with shots/tac_start.png.
set -e
. "$(dirname "$0")/xenv.sh"
cd "$(dirname "$0")"
X=./xin.sh
./run_x.sh
for i in $(seq 1 40); do xdotool search --name "DOSBox-X" >/dev/null 2>&1 && break; sleep 0.5; done
sleep 12; for k in 1 2 3 4; do $X key Escape; sleep 2.5; done
$X click 140 74; sleep 4          # Continue Game -> office
$X click 197 72; sleep 3          # calendar bottom: Restore Game
$X click 160 43; sleep 4          # slot 0 "Quick Start"
$X click 50 142; sleep 16         # bed: Sleep -> Day 1 -> strategic map (+ speech portrait)
$X key Return; sleep 2; $X key Return; sleep 2
$X click 39 168; sleep 1.5        # TEAM
$X click 300 128; sleep 2         # home sector 60
$X click 290 191; sleep 6         # OK -> assignment hints
$X key Return; sleep 2; $X key Return; sleep 2
$X click 290 191; sleep 5         # OK -> "Get to work?"
$X click 226 107; sleep 12        # OK -> tactical
$X shot tac_start
