"""Differential test: the item's voice (panel.md "The item's voice"), original vs port.

usage: .venv/bin/python validate/diff/itemvoice_diff.py [ncases=1500] [DUMP] [--port] [--lib SO]
Per case: a profile, an item type (every type first), a count, a value and the voice flag b. The original runs the item
info box 0x25D20 in Unicorn on tac60 with Item Info off (no box): ArcSeek 0x534A4 answers from the real INV archives
(rom/INV), OpenArchiveForSpeech? 0x5C0EC keeps the archive name, SpeechStream 0x5CC6C and PlayVocSample 0x5C9C0 log what
plays, SpeechPlaying? 0x5CA74 says "not busy" so every voice step runs. InvGradeName 0x25BDC and OpenInvExtra 0x25CA4 run
for real. The poll presses the left button after 12 passes (the steps need at most 7).
Taken: the list of `archive\\member`, in order. --port: ja1f_item_voice_list gives Sound::inv_has + iteminfo::voice +
Sound::item_voice (names, and whether the speech channel now plays); a member that plays must make the channel busy.
The port plays the parts as one sample, so the list is compared, not the channel. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc                        # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 1500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_item_voice_list.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] * 5 + [ctypes.c_void_p, ctypes.c_int32]
    PORT.ja1f_item_voice_list.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

ARC = {a: {n.upper() for n, *_ in datarc.members(open(os.path.join(ROOT, "rom", "INV", a), "rb").read())}
       for a in ("1_54", "55_81", "82_135", "EXTRA", "MONEY")}
QUOTES = open(os.path.join(ROOT, "rom", "EDT", "INVQUOTE.EDT"), "rb").read()
BUTTONS, OPT_INFO = 0x229CF4, 0x1163BE
e = Emu(dump=dump, fast=True)
st = {"arc": None, "name": None, "voc": None, "log": [], "polls": 0}


def open_arc(em, eax, *_):
    st["arc"] = e.cstr(eax).decode("latin-1").upper().split("\\")[-1]
    return 0


def seek(em, eax, edx, ebx, ecx):
    st["name"] = e.cstr(ebx).decode("latin-1").upper()
    return 0 if st["name"] in ARC.get(st["arc"], ()) else 0xFFFFFFFE


def speech(em, *_):
    st["log"].append(f"{st['arc']}\\{st['name']}")
    return 0


def load_voc(em, eax, *_):
    st["voc"] = e.cstr(eax).decode("latin-1").upper()
    return 0


def play_voc(em, *_):
    if st["voc"] in ARC["MONEY"]:
        st["log"].append(f"MONEY\\{st['voc']}")
    return 0


def poll(em, *_):
    st["polls"] += 1
    if st["polls"] > 12:
        e.w8(e.rt(BUTTONS), 1)
    return 0


def quote(em, eax, edx, ebx, ecx):
    buf = e.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    at = ((edx & 0xFFFFFFFF) + ebx) * ecx
    e.uc.mem_write(buf, QUOTES[at:at + ecx].ljust(ecx, b"\0"))
    return 0


for a in (0x67BF0, 0x673AC, 0x67C54, 0x67C68, 0x47020, 0x46EC4, 0x5CCF8, 0x5CE08, 0x5C144, 0x681D0, 0x67E18, 0x67E30, 0x5CA74):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x5C0EC), open_arc)
e.stub(e.rt(0x534A4), seek, 4)
e.stub(e.rt(0x5CC6C), speech)
e.stub(e.rt(0x5C960), load_voc)
e.stub(e.rt(0x5C9C0), play_voc)
e.stub(e.rt(0x532BC), lambda em, *_: 1, 4)
e.stub(e.rt(0x24F08), quote, 16)
e.stub(e.rt(0x678E8), poll)
e.stub_sprintf()
e.stub(e.rt(0x75BB9), lambda em, eax, edx, *_: (lambda k: 0 if k < 0 else eax + k)(e.cstr(eax).find(e.cstr(edx))))
e.stub(e.rt(0x756A0), lambda em, eax, edx, ebx, ecx: e.uc.mem_write(eax, bytes([edx & 0xFF]) * ebx) or eax)
SHEET = e.r32(e.rt(0x93454))
sheet = bytes(e.uc.mem_read(SHEET, 0xC800))

r = random.Random(7)
bad = voiced = found_none = 0
for i in range(ncases):
    t = i % 137 if i < 274 else r.choice([r.randrange(1, 136), 0x6F, 0x59])
    count = r.choice([1, 1, 2, 5, 99, -1, -3, r.randrange(-128, 128)])
    value = r.choice([r.randrange(0, 128), r.choice([10, 11, 25, 26, 50, 51, 75, 76, 99, 100]), r.randrange(0, 32768)])
    if t == 0x6F:
        value = r.choice([0, 1, 9, 10, 19, 20, 50, 99, 100, 101, 250, 1000, 2345, 32767, r.randrange(0, 5000)])
    prof, b = r.randrange(0, 64), r.choice([0, 1, 1])
    e.uc.mem_write(SHEET, sheet)
    e.w8(e.rt(OPT_INFO), 0)
    e.w8(e.rt(BUTTONS), 0)
    st.update(arc=None, name=None, voc=None, log=[], polls=0)
    e.call(e.rt(0x25D20), prof, t, count & 0xFF, value & 0xFFFF, 0, b, 0)
    o = st["log"]
    voiced += bool(o)
    found_none += not o
    if PORT:
        buf = ctypes.create_string_buffer(2048)
        n = PORT.ja1f_item_voice_list(ISO, EXE, prof, t, count, value, b, buf, 2048)
        busy, _, names = buf.raw[:n].decode("latin-1").partition("|")
        p = names.upper().split()
        if p != o or (busy == "1") != bool(o):
            print(f"MISMATCH case {i} t {t} count {count} value {value} b {b} profile {prof}: {o} vs {p} busy {busy}")
            bad += 1
            if bad > 10:
                break
assert bad or (voiced > ncases // 3 and found_none > 0), (voiced, found_none)
print(f"itemvoice_diff: {i + 1} cases ({voiced} with a voice, {found_none} silent), {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
