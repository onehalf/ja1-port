#!/bin/sh
# The setup screen of the port: mouse, window, sound, fixes, A.I.M. grid. The settings go to ja1.cfg beside the
# program. Pull the red lever to play with them; Esc closes the setup.
exec "$(dirname "$0")/ja1-port" --setup
