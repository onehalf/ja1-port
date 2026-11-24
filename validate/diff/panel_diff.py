"""Differential test: the merc panel slot drawing (tools/panel.py) vs the original DrawMercSlot? 0x32EB8 in Unicorn.

usage: .venv/bin/python validate/diff/panel_diff.py [ncases=300] [DUMP ...] [--png] [--port] [--lib SO]
Per case: one tactical dump, one slot 0..7 and fuzzed soldier fields / panel globals (case 0 of each dump and slot
unfuzzed). The original runs 0x32EB8(slot, DL 0) in one emulator, the mirror in a second one on the same state;
the panel buffer, the soldier record, the name buffer and the remap table are compared. --png writes
gfx/panel_diff.png (original | mirror) for the first mismatch. Exits 1 on any mismatch.
--switch (with --port): panel.md "The switch view" instead: per case the switch view 0x330C0 (DL 0), an open/close
frame DrawSlotBack? 0x32E74 (element 1..3) or a pressed switch face 0x32FFC, vs ja1f_draw_switch.
--port: the clean-room front-end (port/ja1front ja1f_draw_slot, docs/spec/panel.md) draws instead of the mirror, on
the emulator's face buffers, soldier records and remap table 0; the name buffer is not compared (front-end only).
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn import UC_HOOK_CODE      # noqa: E402
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EIP   # noqa: E402
import panel as P                    # noqa: E402

DUMPS = ["tac60", "fight1", "combat1", "death1", "f5"]
argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
PORT = None
if "--port" in argv:
    import ctypes

    class Ja1fPanel(ctypes.Structure):
        _fields_ = [("slot", ctypes.c_uint8), ("selected", ctypes.c_uint8), ("combat", ctypes.c_uint8),
                    ("opt_onscreen", ctypes.c_uint8), ("speaker", ctypes.c_uint8), ("hover", ctypes.c_uint8),
                    ("ap_warned", ctypes.c_uint8 * 8), ("faces", ctypes.c_void_p), ("soldiers", ctypes.c_void_p),
                    ("table0", ctypes.c_void_p)]
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_draw_slot.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32, ctypes.POINTER(Ja1fPanel),
                                    ctypes.c_void_p]
    PORT.ja1f_draw_slot.restype = ctypes.c_int32
    PORT.ja1f_draw_switch.argtypes = PORT.ja1f_draw_slot.argtypes + [ctypes.c_int32]
    PORT.ja1f_draw_switch.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
    TSBYTE = 0x22A038


SWITCH = "--switch" in argv


def port_slot(e, slot, what=-1):
    """ja1f_draw_slot on emulator e's state; writes the panel, the soldier records and remap table 0 back"""
    p = P.Panel(e)
    g = lambda a: e.r8(e.rt(a))
    recs = ctypes.create_string_buffer(e.read(e.r32(e.rt(P.SOLDIERS)), 48 * 0x200), 48 * 0x200)
    faces = ctypes.create_string_buffer(b"".join(e.read(e.r32(e.rt(P.P_FACES + 4 * k)), 1024) for k in range(8)),
                                        8 * 1024)
    t0 = p.remap() & ~0xFF
    table0 = ctypes.create_string_buffer(e.read(t0, 256), 256)
    panel = ctypes.create_string_buffer(e.read(p.panel, 320 * 200), 320 * 200)
    pp = Ja1fPanel(slot, g(0x284F11), g(0x92DCB), g(0x92DDF), g(0x284F0E), g(0x93448),
                   (ctypes.c_uint8 * 8)(*e.read(e.rt(0x22A0EC), 8)), ctypes.addressof(faces), ctypes.addressof(recs),
                   ctypes.addressof(table0))
    if what < 0:
        r = PORT.ja1f_draw_slot(ISO, EXE, g(TSBYTE), ctypes.byref(pp), ctypes.addressof(panel))
    else:
        r = PORT.ja1f_draw_switch(ISO, EXE, g(TSBYTE), ctypes.byref(pp), ctypes.addressof(panel), what)
    if r:
        raise RuntimeError(f"ja1f_draw returned {r}")
    e.write(p.panel, panel.raw)
    e.write(e.r32(e.rt(P.SOLDIERS)), recs.raw)
    e.write(t0, table0.raw)
ncases = int(args[0]) if args else 300
dumps = args[1:] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]


def emu(d):
    e = Emu(dump=d, fast=True)
    e.read = lambda a, n: bytes(e.uc.mem_read(a, n))
    e.write = lambda a, b: e.uc.mem_write(a, bytes(b))
    e.uc.hook_add(UC_HOOK_CODE, name_copy, e, e.rt(0x32DCE), e.rt(0x32DCE))
    return e


def name_copy(uc, addr, size, e):
    """0x32DCE..0x32E07 = an inline strcpy with ds/es pushes (they fault in Unicorn): done here, then on at 0x32E07"""
    src, dst = uc.reg_read(UC_X86_REG_EBP) + 0x16E, e.rt(0xEBAC0)
    raw = e.cstr(src)
    e.write(dst, raw + b"\0")
    uc.reg_write(UC_X86_REG_EAX, dst)
    uc.reg_write(UC_X86_REG_EIP, e.rt(0x32E07))


GLOBALS = [(0x284F11, 1), (0x284F0E, 1), (0x93448, 1), (0x92DCB, 1), (0x92DDF, 1), (0x22A0EC, 8)]
# +0x181 icon: 0xB7..0xB9 = first aid (FirstAidStart? 0x3D750); any item sprite of the bank 0xA92BC works
FIELDS = {0x183: lambda r: r.choice([0, 1, 5, 14, 15, 16, 40, 77, 99, 100, r.randrange(256)]),
          0x184: lambda r: r.choice([0, 15, 60, 99, 100, r.randrange(1, 101)]),
          0x185: lambda r: r.choice([0, 0, 1, 4, 5, 30, r.randrange(101)]),
          0x18A: lambda r: r.choice([0, 1, 3, 4, 50, 99, 100, r.randrange(256)]),
          0x1A5: lambda r: r.choice([0, 1, 9, 10, 24, 25, 26, 40, r.randrange(256)]),
          0x0EF: lambda r: r.choice([0, 0, 4, 6, r.randrange(30)]),
          0x17E: lambda r: r.choice([0, 0, 0, r.randrange(1, 9)]),
          0x17F: lambda r: r.choice([0, 0, 1]),
          0x180: lambda r: r.randrange(2),
          0x181: lambda r: r.choice([0, 0, 0, 0, 0xB7, 0xB8, 0xB9, r.randrange(1, 0xB7)]),
          0x182: lambda r: r.choice([0, 1, 1, 1]),
          0x138: lambda r: r.choice([0, 1, 1, 1]),
          0x186: lambda r: r.choice([0, 0, 0xFF, r.randrange(4)]),
          0x189: lambda r: r.choice([0, 0, 0, 1, 2, 3]),
          0x194: lambda r: r.choice([0, 0, 0, 1]),
          0x1BB: lambda r: r.choice([0, 0, 0, 1]),
          0x1A8: lambda r: r.choice([0, 1]),
          0x1AA: lambda r: r.randrange(20),
          0x1AC: lambda r: r.randrange(5),
          0x16A: lambda r: r.choice([0, 1, 1, r.randrange(256)]),
          0x16B: lambda r: r.choice([0, 1, 1, r.randrange(256)]),
          0x16C: lambda r: r.choice([0, 1, 1, r.randrange(256)])}


def fuzz(es, slot, r):
    noise = bytes(r.randrange(256) for _ in range(48 * 42)) if r.randrange(2) else bytes(48 * 42)
    for e in es:
        p = P.Panel(e)
        x, y = p.pos(slot)
        for row in range(42):
            e.write(p.panel + (y + row) * 320 + x, noise[row * 48:(row + 1) * 48])
    vals = {a: f(r) for a, f in FIELDS.items()}
    if PORT:    # game range: AP left 0..127 (a negative value makes the AP bar read past the panel sheet)
        vals[0x1A5] &= 0x7F
    g = {0x284F11: r.choice([slot, slot, r.randrange(8), 0xFF]), 0x284F0E: r.choice([slot, 0xFF, 0xFF, r.randrange(8)]),
         0x93448: r.choice([slot, 0xFF, r.randrange(8)]), 0x92DCB: r.randrange(2), 0x92DDF: r.choice([0, 0, 0, 1, 2]),
         0x22A0EC + slot: r.randrange(2)}
    if r.random() < .15:                          # the eye rule edge: hp 15, AP 0, not dying
        vals[0x183], vals[0x1A5], vals[0x189] = r.choice([14, 15, 16]), 0, 0
    if r.random() < .2:
        vals[0x184] = r.choice([95, 96, 97, 98])
    # the waypoint frame test: +0x1B9, the animation word +0x1E8 + 2 * +0x1F3, the three position pairs, +0x120
    words = {}
    if r.random() < .5:
        vals[0x1B9] = r.choice([0, 4, 5, 6, 0xFF, r.randrange(256)])
        k = r.randrange(5)
        vals[0x1F3] = k
        words[0x1E8 + 2 * k] = r.choice([0, 3, 4, 5, r.randrange(0x100)])
        words[0x120] = r.choice([0xC40, 0xC41, 0xC42, r.randrange(0x10000)])
        same = r.random() < .5
        for a in (0x114, 0x116, 0x118):
            words[a] = r.choice([0, 7, 100])
            words[a + 0x10] = words[a] if same or r.random() < .6 else words[a] + 1
    for e in es:
        s = e.r32(e.rt(P.SOLDIERS + 4 * slot))
        for a, v in vals.items():
            e.w8(s + a, v)
        for a, v in words.items():
            e.w16(s + a, v)
        for a, v in g.items():
            e.w8(e.rt(a), v)


def state(e, slot):
    p = P.Panel(e)
    s = p.soldier(slot)
    return {"panel": e.read(p.panel, 320 * 200), "soldier": e.read(s, 0x200),
            "name_buf": e.read(e.rt(P.NAME_BUF), 16), "remap": e.read(p.remap() & ~0xFF, 0x200)}


fails = cases = 0
r = random.Random(1)
first = None
for d in dumps:
    if not os.path.exists(d):
        print(f"SKIP {d} (missing)")
        continue
    A, B = emu(d), emu(d)
    snap = bytes(A.uc.mem_read(0, 16 << 20))
    per = max(8, ncases // len(dumps))
    for i in range(per):
        slot = i % 8
        if i >= 8:
            A.uc.mem_write(0, snap); B.uc.mem_write(0, snap)
            fuzz((A, B), slot, r)
        else:
            A.uc.mem_write(0, snap); B.uc.mem_write(0, snap)
        err = None
        what = -1
        if SWITCH:
            what = r.choice([0, 0, 0, 1, 2, 3, 0x100 + r.randrange(6)])
            if what == 0:
                A.w8(A.r32(A.rt(P.SOLDIERS + 4 * slot)) + 0x17F, 1)
                B.w8(B.r32(B.rt(P.SOLDIERS + 4 * slot)) + 0x17F, 1)
        try:
            if what < 0:
                A.call(A.rt(0x32EB8), slot, 0)
            elif what == 0:
                A.call(A.rt(0x330C0), A.r32(A.rt(P.SOLDIERS + 4 * slot)), 0)
            elif what < 0x100:
                A.call(A.rt(0x32E74), slot, what)
            else:
                i, on = (what - 0x100) // 2, what & 1
                sel = 4 if A.r8(A.rt(0x284F11)) == slot else 0
                x, y = P.Panel(A).pos(slot)
                A.call(A.rt(0x32FFC), 0x46 + on + sel, x, y + 5 + 9 * i)
        except Exception as ex:
            err = f"original: {ex}"
        try:
            port_slot(B, slot, what) if PORT else P.draw_slot(B, slot)
        except Exception as ex:
            err = (err or "") + f" mirror: {ex!r}"
        cases += 1
        bad = []
        if err:
            bad.append(err)
        else:
            sa, sb = state(A, slot), state(B, slot)
            for k in sa:
                if sa[k] != sb[k] and not (PORT and k == "name_buf"):
                    n = next(j for j in range(len(sa[k])) if sa[k][j] != sb[k][j])
                    bad.append(f"{k} @{n} ({n % 320},{n // 320}) {sa[k][n]:#x} vs {sb[k][n]:#x}"
                               f" ({sum(x != y for x, y in zip(sa[k], sb[k]))} bytes)")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH {os.path.basename(d)} case {i} slot {slot}: " + "; ".join(bad))
            if first is None and not err:
                first = (sa["panel"], sb["panel"])
if first and "--png" in sys.argv:
    from pcx import png
    from csprite import palette
    a, b = first
    pix = b"".join(a[y * 320:(y + 1) * 320] + b[y * 320:(y + 1) * 320] for y in range(200))
    png(os.path.join(ROOT, "gfx", "panel_diff.png"), 640, 200, pix, palette(0))
print(f"panel_diff{' (port)' if PORT else ''}{' switch' if SWITCH else ''}: {cases} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
