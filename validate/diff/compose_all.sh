#!/bin/sh
# Every compose_diff mode over every dump, one process per (mode, dump), in parallel.
# usage: validate/diff/compose_all.sh [OUTDIR] [JOBS]   (default build/compose_all, nproc - 4)
# MODES="ai-port" validate/diff/compose_all.sh runs only the listed modes (ai-port = AiThink of the clean-room port).
# Prints a summary per mode and the failed or mismatching (mode, dump) pairs; the exit code is 1 if any.
cd "$(dirname "$0")/../.." || exit 2
out=${1:-build/compose_all}
jobs=${2:-$(($(nproc) - 4))}
mkdir -p "$out"
modes=${MODES:-"ai --flight --turn --melee --blast --newturn --dies ai-port"}
rm -f "$out"/*.out "$out"/status.txt
for m in $modes; do
    for d in build/mem_*.bin; do echo "$m $d"; done
done | xargs -P "$jobs" -L 1 sh -c '
    m=$0; d=$1; b=$(basename "$d" .bin)
    if [ "$m" = ai ]; then .venv/bin/python validate/diff/compose_diff.py "$d" > "'"$out"'/$m.$b.out" 2>&1
    elif [ "$m" = ai-port ]; then .venv/bin/python validate/diff/compose_diff.py --port "$d" > "'"$out"'/$m.$b.out" 2>&1
    else .venv/bin/python validate/diff/compose_diff.py "$m" "$d" > "'"$out"'/$m.$b.out" 2>&1; fi
    echo "$m $b $?" >> "'"$out"'/status.txt"'
bad=0
for m in $modes; do
    tail -qn 1 "$out"/"$m".*.out | awk -v m="$m" '
        { for (i = 1; i <= NF; i++) if ($i ~ /^[0-9]+$/) { n += $i; break }
          for (i = 1; i < NF; i++) if ($(i + 1) ~ /^mismatch/) mm += $i }
        END { printf "%-10s %6d cases, %d mismatches\n", m, n, mm }'
done
awk '$3 != 0' "$out"/status.txt | grep . && bad=1
grep -L " 0 mismatches" "$out"/*.out | sed 's/^/mismatch or no result: /' | grep . && bad=1
exit $bad
