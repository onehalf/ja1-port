"""Differential test: the slot list of Save Game / Restore Game (strategic.md "Save Game and Restore Game").

usage: .venv/bin/python validate/diff/slots_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case: mode (1 save, 0 restore), fuzzed slot headers (in use, day, name), the save name and `day`, a noise
screen, and a tape of mouse polls with key bytes (row moves, clicks, typed names, Backspace, the extended keys
Home/Left/Right/End/Delete, keys that are not allowed followed by G K M O S, Enter, Escape, right clicks). The
original runs SelectSaveSlot? 0x4D798(mode) in Unicorn on tac60 with MousePoll 0x678E8 / KeyPressed 0x5616C /
GetKey 0x560F4 fed from the tape (a GetKey with no key waiting takes the keys of the next poll), the slot headers
0x4DB34 stubbed, TEXTSLAB.PCC by the PCX loader stub 0x532BC, and g_panelBuf = the screen (the final copy back is
then a no-op). The port (ja1f_slots) runs the same tape. Compared: the screen, the outcome (cancel 0x28ECDC, no
saves, the chosen slot from the file name 0x28EE19), the save name g_saveName 0x28EDFE and the polls used.
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes
import tempfile, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop         # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
import pfix                          # noqa: E402  #71 --pfix 7: the name line cursor keys on both sides
PFIX = pfix.parse(argv)
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_slots.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] + [ctypes.c_char_p] * 4 + [ctypes.c_int32] + \
        [ctypes.c_char_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p]
    PORT.ja1f_slots.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

OFFICE = open(os.path.join(ROOT, "rom", "DAT", "OFFICE.DAT"), "rb").read()
SCREEN_P, PANEL_P, DAY, SAVENAME, CANCEL, FNAME = 0x95678, 0x93E60, 0x229FA4, 0x28EDFE, 0x28ECDC, 0x28EE19
LIST, DAYS, NAMES = 0x28E060, 0x28ECE4, 0x28ECF8
MOUSE_X, MOUSE_Y, BUTTONS = 0x229D00, 0x229D04, 0x229CF4

e = Emu(dump=dump, fast=True)
if PFIX is not None:

    class _FrontFixes:                    # the port side is libja1front: its own copy of the fix masks
        @staticmethod
        def port_fixes(m):
            if PORT:
                PORT.ja1_port_fixes_set.argtypes = [ctypes.c_uint64]
                PORT.ja1_port_fixes_set(m)
    pfix.apply(e, PFIX, _FrontFixes)


def load(em, eax, edx, ebx, ecx):
    f = datarc.member(OFFICE, e.cstr(ebx).decode("latin-1").upper() if isinstance(e.cstr(ebx), bytes)
                      else e.cstr(ebx).upper())
    w, h, px, _ = pcx.decode(f)
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


st = {}


def take(j):
    """The keys of poll j go to the queue (once)."""
    if j < len(st["tape"]) and j not in st["pushed"]:
        st["pushed"].add(j)
        st["q"].extend(st["tape"][j][3])
        st["touched"] = max(st["touched"], j + 1)


def poll(em, *_):
    j = st["next"]
    if j >= len(st["tape"]):
        em.stop_reason = "tape end"
        em.uc.emu_stop()
        return 0
    st["next"] = j + 1
    take(j)
    st["touched"] = max(st["touched"], j + 1)
    x, y, b, _ = st["tape"][j]
    e.w32(e.rt(MOUSE_X), x)
    e.w32(e.rt(MOUSE_Y), y)
    e.w8(e.rt(BUTTONS), b)
    return 0


def get_key(em, *_):
    """A key byte; none waiting: the read waits, polls without keys pass unseen, the next poll's keys come (that
    poll itself still comes later)."""
    k = st["next"]
    while not st["q"]:
        if k >= len(st["tape"]):
            em.stop_reason = "tape end"
            em.uc.emu_stop()
            return 0
        take(k)
        if not st["q"]:
            k += 1
    st["next"] = k
    return st["q"].pop(0)


def headers(em, *_):
    for n in range(10):
        e.w32(e.rt(LIST + 4 * n), 0xFFFFFFFF)
    k = 0
    for n in range(10):
        used, day, name = st["hdr"][n]
        e.uc.mem_write(e.rt(DAYS + 2 * n), struct.pack("<H", day if used else 0xFFFF))
        e.uc.mem_write(e.rt(NAMES + 26 * n), name if used else b"\0" * 26)
        if used:
            e.w32(e.rt(LIST + 4 * k), n)
            k += 1
    return k


for a in (0x67BF0, 0x673AC, 0x28698, 0x47020):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x4DB34), headers)
e.stub(e.rt(0x678E8), poll)
e.stub(e.rt(0x5616C), lambda em, *_: int(bool(st["q"])))
e.stub(e.rt(0x560F4), get_key)
e.stub_sprintf()
screen = e.alloc(64000)
e.w32(e.rt(SCREEN_P), screen)
e.w32(e.rt(PANEL_P), screen)

CHARS = b"abcxyzABQZ 019:;?,.!-&'/"
NAMEPOOL = [b"", b"a", b"Day one", b"Grunty and Ivan", b"x" * 23, b"y" * 24, b"z" * 25, b"Save: 3.", b"- . :"]


def row_xy(r, rng):
    return rng.randrange(43, 277), 26 + 14 * r - 3 + rng.randrange(13)


def make_case(rng):
    mode = rng.choice([0, 1, 1])
    hdr = []
    for n in range(10):
        used = rng.random() < rng.choice([0.0, 0.3, 0.7, 1.0])
        day = rng.choice([1, 3, 9, 10, 99, 255, rng.randrange(65536), 0xFFFF, 0])
        nm = rng.choice(NAMEPOOL)
        hdr.append((used, day, nm.ljust(26, b"\0")[:26]))
    name = rng.choice(NAMEPOOL).ljust(26, b"\0")[:26]
    day = rng.choice([1, 4, 12, 100, rng.randrange(1000)])
    tape = [(rng.randrange(320), rng.randrange(200), rng.choice([1, 1, 0]), [])]    # the opening click
    for _ in range(rng.randrange(3, 30)):
        kind = rng.random()
        r = rng.randrange(0, 12)
        x, y = row_xy(r, rng) if rng.random() < 0.8 else (rng.randrange(320), rng.randrange(200))
        if kind < 0.35:
            tape.append((x, y, 0, []))
        elif kind < 0.5:
            tape.append((x, y, 1, []))
            tape.append((x, y, rng.choice([0, 0, 1]), []))
        elif kind < 0.85:
            ks = []
            for _ in range(rng.randrange(1, 6)):
                c = rng.random()
                if c < 0.5:
                    ks.append(rng.choice(CHARS))
                elif c < 0.65:
                    ks.append(8)
                elif c < 0.8:
                    ks += [0, rng.choice([0x47, 0x4B, 0x4D, 0x4F, 0x53, 0x3B, 0x48])]
                elif c < 0.92:
                    ks += [rng.choice(b"#*$"), rng.choice(b"GKMOSa")]
                else:
                    ks.append(rng.choice([13, 27, 13]))
            tape.append((x, y, 0, ks))
        elif kind < 0.93:
            tape.append((x, y, 2, []))
            tape.append((x, y, rng.choice([0, 2]), []))
        else:
            tape.append((x, y, 0, [rng.choice(b"#")]))
            tape.append((x, y, 0, []))
    tape += [(5, 5, 2, [27])] * 4 + [(5, 5, 0, [27])] * 4
    return mode, hdr, name, day, tape


def run_orig(mode, hdr, name, day, tape, noise):
    st.clear()
    st.update(tape=tape, q=[], pushed=set(), next=0, touched=0, hdr=hdr)
    e.uc.mem_write(screen, noise)
    e.uc.mem_write(e.rt(SAVENAME), name)
    e.w32(e.rt(DAY), day)
    e.w32(e.rt(CANCEL), 0)
    e.uc.mem_write(e.rt(FNAME), b"\0" * 12)
    try:
        cnt = e.call(e.rt(0x4D798), mode)
    except EmuStop:
        cnt = None
    if e.stop_reason == "tape end":
        res = -1
    elif cnt is None:
        res = -9                    # the original faulted (a Delete key put 0x7F into the name: issue #2 row 26)
    elif mode == 0 and cnt == 0:
        res = -3
    elif e.r32(e.rt(CANCEL)) & 0xFF:
        res = -2
    else:
        fn = bytes(e.uc.mem_read(e.rt(FNAME), 12)).split(b"\0")[0]
        res = int(fn[4:-4])
    return res, st["touched"], bytes(e.uc.mem_read(screen, 64000)), bytes(e.uc.mem_read(e.rt(SAVENAME), 26))


def run_port(mode, hdr, name, day, tape, noise):
    used = bytes(int(u) for u, _, _ in hdr)
    days = struct.pack("<10H", *[d if u else 0xFFFF for u, d, _ in hdr])
    names = b"".join(nm if u else b"\0" * 26 for u, _, nm in hdr)
    t = b"".join(struct.pack("<hhBB", x, y, b, len(ks)) + bytes(ks) for x, y, b, ks in tape)
    nb = ctypes.create_string_buffer(name, 26)
    buf = ctypes.create_string_buffer(noise, 64000)
    out = (ctypes.c_int32 * 2)()
    rc = PORT.ja1f_slots(ISO, EXE, mode, used, days, names, nb, day, t, len(t), buf, out)
    return rc, out[0], out[1], buf.raw, nb.raw


rng = random.Random(1)
bad = ended = faulted = 0
for i in range(ncases):
    mode, hdr, name, day, tape = make_case(rng)
    noise = bytes(rng.randrange(256) for _ in range(64000))
    o = run_orig(mode, hdr, name, day, tape, noise)
    if os.environ.get("SLOTS_SHOW") == str(i):
        print(name, day, [h[0] for h in hdr], *tape[:o[1] + 3], sep="\n")
    ended += o[0] not in (-1, -9)
    faulted += o[0] == -9
    if PORT and o[0] == -9:
        # the original crashed on a text glyph of width 0: the port must report the stop point (strategic.md)
        sys.stderr.flush()
        with tempfile.TemporaryFile() as tf:
            keep = os.dup(2)
            os.dup2(tf.fileno(), 2)
            try:
                run_port(mode, hdr, name, day, tape, noise)
            finally:
                os.dup2(keep, 2)
                os.close(keep)
            tf.seek(0)
            if b"stop point" not in tf.read():
                print(f"MISMATCH case {i}: the original crashed, the port made no stop point")
                bad += 1
    if PORT and o[0] != -9:
        rc, res, used, scr, nm = run_port(mode, hdr, name, day, tape, noise)
        if rc != 0 or (res, used, scr, nm) != o:
            nb = next((q for q in range(64000) if scr[q] != o[2][q]), -1)
            print(f"MISMATCH case {i} mode {mode}: rc {rc}, outcome {o[0]} vs {res}, polls {o[1]} vs {used}, name "
                  f"{o[3]!r} vs {nm!r}, " + (f"screen @({nb % 320},{nb // 320}) {o[2][nb]:#x} vs {scr[nb]:#x}"
                                              if nb >= 0 else "screen equal"))
            bad += 1
            if bad > 10:
                break
if PFIX is not None and 7 in PFIX and faulted:   # P07 takes 0x7F out of the name: no crash is left
    print(f"MISMATCH: {faulted} cases crashed the original with P07")
    bad += faulted
print(f"slots_diff: {ncases} cases ({ended} ended in the tape, {faulted} skipped: the original faulted), {bad} mismatches" + ("" if PORT else " (original only)"))
print(f"CASES slots={ncases - faulted}")
sys.exit(1 if bad else 0)
