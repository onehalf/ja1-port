#!/bin/sh
# usage: ./t <tool> [args]  -> runs tools/<tool>.py (the project venv .venv when present, else python3)
d=$(dirname "$0"); t=$1; shift
py="$d/.venv/bin/python"; [ -x "$py" ] || py=python3
exec "$py" "$d/tools/$t.py" "$@"
