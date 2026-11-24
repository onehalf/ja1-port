#!/bin/sh
# The port's game folder build/game113b: the GOG 1.13 files of rom/ (hard links, or copies across file systems),
# with the official 1.13b patch files from patch113b/ over them when that folder exists (JA.EXE, DAT/XRA.DAT,
# DAT/QUESTS.DAT; README "Setup"). The front end reads hard-disk files beside its JA.EXE argument before the CD.
set -e
cd "$(dirname "$0")"
P=${JA1_PATCH113B:-../patch113b}
G=../build/game113b
[ -f ../rom/JA.EXE ] || { echo "patch113b: rom/JA.EXE missing (README \"Setup\")" >&2; exit 1; }
mkdir -p $G/DAT
put() { [ -e "$2" ] && return 0; ln "$1" "$2" 2>/dev/null || cp -r "$1" "$2"; }
for f in ../rom/*; do
  n=$(basename "$f")
  case $n in DAT|__support|*.pdf) continue;; esac
  if [ -d "$f" ]; then [ -e "$G/$n" ] || cp -al "$f" "$G/$n" 2>/dev/null || cp -r "$f" "$G/$n"; else put "$f" "$G/$n"; fi
done
for f in ../rom/DAT/*; do put "$f" "$G/DAT/$(basename "$f")"; done
if [ -d "$P" ]; then
  for f in JA.EXE DAT/XRA.DAT DAT/QUESTS.DAT; do
    [ -f "$P/$f" ] || continue
    cmp -s "$P/$f" "$G/$f" 2>/dev/null || { rm -f "$G/$f"; cp "$P/$f" "$G/$f"; }
  done
else
  echo "patch113b: no $P folder: build/game113b = plain GOG 1.13 (the diff tests target 1.13b)" >&2
fi
# The DOSBox scratch copy of the original (validate/run.sh) gets the same patch files.
if [ -d "$P" ] && [ -d run/C ]; then
  for f in JA.EXE DAT/XRA.DAT DAT/QUESTS.DAT; do
    [ -f "$P/$f" ] && { cmp -s "$P/$f" "run/C/$f" || cp "$P/$f" "run/C/$f"; }
  done
fi
exit 0
