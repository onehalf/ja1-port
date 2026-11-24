"""Save-format test: every save file around parses with tools/savefile.py and re-serialises byte-identically.

usage: .venv/bin/python validate/diff/save_diff.py [SAVE ...]
Default set: rom/__support/save/QICKSTRT.SAV, build/*.SAV (day/office/quick saves copied out of DOSBox-X sessions),
validate/run/C/*.SAV. The layout comes from the original's SaveGame / QuickSave fwrite_ sequence, so this checks that
the block list + variable tail (sector files, stash, replay) account for every byte. Exits 1 on any failure.
"""
import glob, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [os.path.join(ROOT, "tools")]
from savefile import Save            # noqa: E402

files = sys.argv[1:] or sorted(set(
    [os.path.join(ROOT, "rom", "__support", "save", "QICKSTRT.SAV")] +
    glob.glob(os.path.join(ROOT, "build", "*.SAV")) + glob.glob(os.path.join(ROOT, "validate", "run", "C", "*.SAV"))))
fails, kinds = 0, {}
for f in files:
    data = open(f, "rb").read()
    try:
        s = Save(data)
        ok = s.serialise() == data
    except SystemExit as e:
        ok, s = False, None
        print(f"{os.path.relpath(f, ROOT)}: {e}")
    if s:
        kinds[s.kind] = kinds.get(s.kind, 0) + 1
    if not ok:
        fails += 1
        print(f"{os.path.relpath(f, ROOT)}: FAIL")
print(f"save_diff: {len(files) - fails}/{len(files)} saves round-trip ({', '.join(f'{n} {k}' for k, n in kinds.items())})")
sys.exit(1 if fails else 0)
