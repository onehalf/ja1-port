"""Differential test: the merc stat popup (tools/popup.py) vs the original DrawPopupInventory? 0x3FF3C in Unicorn.

usage: .venv/bin/python validate/diff/popup_diff.py [ncases=200] [DUMP ...] [--port [--lib SO]]
--port: the clean-room front-end (port/ja1front ja1f_draw_popup, docs/spec/panel.md "Stat popup") draws instead of
the mirror; the loader result is then not compared (the port does not return it).
Per case: one dump, one slot 0..7, fuzzed values of the 10 numbers (+0x112 u16, hp, +0x18B..+0x192; negative and
5-digit values too) and noise in the panel buffer. The picture loader 0x532BC is stubbed on both sides (POPINV.PCC
from rom/DAT/MAIN.DAT, decoded into the panel buffer at its pitch); the mouse hide / show 0x67368 / 0x67398 are
no-ops. Compared: the panel buffer, the screen buffer, the loader result [0x229D18]. Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import panel as P                    # noqa: E402
import popup                         # noqa: E402
import datarc, pcx                   # noqa: E402

DUMPS = ["tac60", "fight1", "combat1", "death1", "f5"]
argv = sys.argv[1:]
PORT = None
if "--port" in argv:
    import ctypes
    LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target",
                                                                             "release", "libja1front.so")
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_draw_popup.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32] + [ctypes.c_void_p] * 3
    PORT.ja1f_draw_popup.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args and args[0].isdigit() else 200
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
W, H, PIX, _ = pcx.decode(datarc.member(datarc.read(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT")), "POPINV.PCC"))
assert (W, H) == (100, 42), (W, H)


def load(e, dst, pitch):
    for y in range(H):
        e.uc.mem_write(dst + y * pitch, PIX[y * W:(y + 1) * W])
    return 0


def emu(d):
    e = Emu(dump=d, fast=True)
    e.read = lambda a, n: bytes(e.uc.mem_read(a, n))
    e.write = lambda a, b: e.uc.mem_write(a, bytes(b))

    def orig_load(emu_, eax, edx, ebx, ecx):
        return load(e, ecx, e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4))
    e.stub(e.rt(0x532BC), orig_load, 4)
    e.stub(e.rt(0x67368), lambda *_: 0)
    e.stub(e.rt(0x67398), lambda *_: 0)
    return e


def fuzz(e, slot, r):
    p = P.Panel(e)
    e.uc.mem_write(p.panel, bytes(r.randrange(256) for _ in range(320 * 200)) if r.random() < .5 else bytes(64000))
    e.w32(e.rt(popup.LOAD_RESULT), r.randrange(1 << 32))
    s = p.soldier(slot)
    e.w16(s + 0x112, r.choice([0, 7, 999, 1000, 9999, 10000, 65535, r.randrange(65536)]))
    for a in range(0x183, 0x193):
        if a == 0x183 or a >= 0x18B:
            e.w8(s + a, r.choice([0, 1, 9, 10, 99, 100, 127, 0x80, 0xFF, r.randrange(256)]))


def port_popup(e, slot):
    """ja1f_draw_popup on emulator e's state; writes the panel and the screen back"""
    p = P.Panel(e)
    recs = ctypes.create_string_buffer(e.read(e.r32(e.rt(P.SOLDIERS)), 48 * 0x200), 48 * 0x200)
    scr_a = e.r32(e.rt(popup.P_SCREEN))
    panel = ctypes.create_string_buffer(e.read(p.panel, 320 * 200), 320 * 200)
    screen = ctypes.create_string_buffer(e.read(scr_a, 320 * 200), 320 * 200)
    r = PORT.ja1f_draw_popup(ISO, EXE, slot, ctypes.addressof(recs), ctypes.addressof(panel), ctypes.addressof(screen))
    if r:
        raise RuntimeError(f"ja1f_draw_popup returned {r}")
    e.write(p.panel, panel.raw)
    e.write(scr_a, screen.raw)


def state(e):
    p = P.Panel(e)
    return {"panel": bytes(e.uc.mem_read(p.panel, 320 * 200)),
            "screen": bytes(e.uc.mem_read(e.r32(e.rt(popup.P_SCREEN)), 320 * 200)),
            "result": e.r32(e.rt(popup.LOAD_RESULT))}


fails = cases = 0
r = random.Random(1)
for d in dumps:
    if not os.path.exists(d):
        print(f"missing {d}")
        sys.exit(2)
    A, B = emu(d), emu(d)
    snap = bytes(A.uc.mem_read(0, 16 << 20))
    for i in range(max(8, ncases // len(dumps))):
        slot = i % 8
        A.uc.mem_write(0, snap)
        B.uc.mem_write(0, snap)
        if i >= 8:
            seed = r.randrange(1 << 30)
            fuzz(A, slot, random.Random(seed))
            fuzz(B, slot, random.Random(seed))
        A.call(A.rt(0x3FF3C), slot)
        if PORT:
            port_popup(B, slot)
        else:
            popup.draw_popup(B, slot, lambda dst, pitch: load(B, dst, pitch))
        cases += 1
        sa, sb = state(A), state(B)
        bad = []
        for k in sa:
            if sa[k] != sb[k] and not (PORT and k == "result"):
                if k == "result":
                    bad.append(f"result {sa[k]:#x} vs {sb[k]:#x}")
                    continue
                n = next(j for j in range(len(sa[k])) if sa[k][j] != sb[k][j])
                bad.append(f"{k} @({n % 320},{n // 320}) {sa[k][n]:#x} vs {sb[k][n]:#x} "
                           f"({sum(x != y for x, y in zip(sa[k], sb[k]))} bytes)")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH {os.path.basename(d)} case {i} slot {slot}: " + "; ".join(bad))
print(f"popup_diff{' (port)' if PORT else ''}: {cases} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
