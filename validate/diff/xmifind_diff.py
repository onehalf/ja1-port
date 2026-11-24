"""Differential test: ail_find_XMIDI_sequence? 0x81E50 (the XMI sequence finder of AIL_init_sequence), original vs port.

usage: .venv/bin/python validate/diff/xmifind_diff.py [--lib SO]
The original runs in Unicorn (cdecl (file, n), strncmp_ 0x89E6D runs too) on every real XMI file of the game folder
(build/game113b/MISC/*.XMI) for n = -1, 0 .. count + 1. It skips top-level chunks whose type is not XMID (FORM XDIR),
walks the children of CAT XMID and returns the n-th FORM XMID, or 0 (AIL_init_sequence fails: no music). The port side
is music.rs parse_xmi (via music::xmi_form, ja1f_xmi_form): the FORM offset of its sequence n, -1 when it has none.
Compared: the returned offset (0 = -1). The FORM XMID at the top level branch (0x81EE9) is not in any game file.
Box 1 review (#45 c13004). Exits 1 on any mismatch.
"""
import ctypes, glob, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_xmi_form.argtypes = [ctypes.c_char_p, ctypes.c_int32, ctypes.c_int32]
PORT.ja1f_xmi_form.restype = ctypes.c_int64
FIND = 0x81E50

e = Emu()
files = sorted(glob.glob(os.path.join(ROOT, "build", "game113b", "MISC", "*.XMI")))
if not files:
    sys.exit("xmifind_diff: no XMI file in build/game113b/MISC (run validate/patch113b.sh)")
cases = fails = found = 0
for f in files:
    d = open(f, "rb").read()
    base = e.alloc(d + bytes(16))
    n = 0
    while True:                      # count the sequences on the port side, then test n = -1 .. count + 1
        if PORT.ja1f_xmi_form(d, len(d), n) < 0:
            break
        n += 1
    for k in range(-1, n + 2):
        got = e.call(FIND, base, k & 0xFFFFFFFF, cdecl=True)
        want = got - base if got else -1
        port = PORT.ja1f_xmi_form(d, len(d), k)
        cases += 1
        found += want >= 0
        if want != port:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH {os.path.basename(f)} n={k}: original {want}, port {port}")
print(f"xmifind_diff: {len(files)} files, {cases} cases ({found} found), {fails} mismatches")
sys.exit(1 if fails else 0)
