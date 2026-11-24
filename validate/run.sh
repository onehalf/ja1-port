#!/bin/sh
# Launch the ORIGINAL JA.EXE in DOSBox (X window). Fresh scratch copy of rom/ (minus the CD image) in
# validate/run/C on first use or with --fresh; saves made in-game land there, never in rom/. Then the official 1.13b
# patch files go over it (port target 1.13b, #24; validate/patch113b.sh).
#   validate/run.sh [--fresh] [--sound] [--wait]     (--wait: block until DOSBox exits)
set -e
cd "$(dirname "$0")"
fresh=0; sound=0; wait=0
for a in "$@"; do case $a in --fresh) fresh=1;; --sound) sound=1;; --wait) wait=1;; esac; done
if [ $fresh = 1 ] || [ ! -e run/C/JA.EXE ]; then
  rm -rf run/C; mkdir -p run/C
  (cd ../rom && tar cf - --exclude=JA1.gog --exclude=__support --exclude='*.pdf' .) | tar xf - -C run/C
  chmod -R u+w run/C
fi
./patch113b.sh
conf=dosbox.conf
if [ $sound = 1 ]; then sed 's/^nosound=true/nosound=false/' dosbox.conf > run/dosbox_sound.conf; conf=run/dosbox_sound.conf; fi
if [ $wait = 1 ]; then exec dosbox -conf "$conf" -noconsole; fi
dosbox -conf "$conf" -noconsole >run/dosbox.log 2>&1 &
echo $! > run/dosbox.pid
