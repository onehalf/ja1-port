#!/bin/sh
# Fan patch 1.15m (#70, unofficial, third_party/ja115m/patch/JA115/): two folders that hold jam115.exe (as JA.EXE)
# and the 1.15m DAT/XRA.DAT over the 1.13b files. The port stays 1.13b; these exist only to check the fan_fixes rows.
#  - build/game115m: the port's game folder for 1.15m (hard links to build/game113b, real copies of the two files).
#  - validate/run115/C: DOSBox-X scratch folder (a real copy of validate/run/C minus saves; the game writes saves
#    there) + validate/run115/dosbox-x.conf (validate/dosbox-x.conf with the C: mount moved). JA1_VER=115m
#    validate/run_x.sh starts it.
set -e
cd "$(dirname "$0")"
P=../third_party/ja115m/patch/JA115
[ -e $P/jam115.exe ] || { echo "patch115m: $P missing (see third_party/ja115m/README.md)" >&2; exit 1; }
./patch113b.sh
G=../build/game115m; B=../build/game113b
mkdir -p $G/DAT
for f in $B/*; do
  n=$(basename "$f")
  case $n in DAT|JA.EXE) continue;; esac
  [ -e "$G/$n" ] && continue
  if [ -d "$f" ]; then cp -al "$f" "$G/$n"; else ln "$f" "$G/$n"; fi
done
for f in $B/DAT/*; do
  n=$(basename "$f")
  [ "$n" = XRA.DAT ] && continue
  [ -e "$G/DAT/$n" ] || ln "$f" "$G/DAT/$n"
done
cmp -s $P/jam115.exe $G/JA.EXE || cp $P/jam115.exe $G/JA.EXE
cmp -s $P/DAT/XRA.DAT $G/DAT/XRA.DAT || cp $P/DAT/XRA.DAT $G/DAT/XRA.DAT
if [ -e run/C/JA.EXE ]; then
  R=run115/C
  if [ ! -e $R/JA.EXE ]; then
    mkdir -p $R
    (cd run/C && tar cf - --exclude='*.SAV' --exclude='*.sav' --exclude=REPLAY.REC .) | tar xf - -C $R
    chmod -R u+w $R
  fi
  cmp -s $P/jam115.exe $R/JA.EXE || cp $P/jam115.exe $R/JA.EXE
  cmp -s $P/DAT/XRA.DAT $R/DAT/XRA.DAT || cp $P/DAT/XRA.DAT $R/DAT/XRA.DAT
  sed "s#^MOUNT C .*#MOUNT C $PWD/$R#" dosbox-x.conf > run115/dosbox-x.conf
fi
L=$(find $G -type l 2>/dev/null | head -5)
[ -z "$L" ] || { echo "patch115m: symlinks found (use hard links):" >&2; echo "$L" >&2; exit 1; }
