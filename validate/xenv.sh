# sourced by the validate/*.sh drivers: run everything on the private display (xdisplay.sh), never the user's.
export DISPLAY=${JA_DISPLAY:-:99}
