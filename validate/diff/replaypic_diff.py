"""Differential test: the replay map pictures (strategic.md "The replay pictures": replay_draw 0..3, the flag frame
and the animations replay_anim 0, 1, 3, 4).

usage: .venv/bin/python validate/diff/replaypic_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case: a fuzzed replay table R (60 x 9 bytes at 0x2254F0), noise in the screen *0x95678 and the clean map
*0x93E60, k = 0..3 and a sector a. The original runs in Unicorn on tac60: the 640-wide map buffer *0x93E5C holds
MAPDARK.PCC | MAP.PCC, the flag sheet REPLAY.PCX sits at 0x22A4EF with pitch 0x180 (as DayReplay loads it), the
marks 0x229D8C are 0; replay_draw k = DrawMapTable 0x41C3C (EAX R), 0x41B24 (EAX R, EDX 1 as in the game), 0x41BB8 (EAX a),
0x41C60 (EAX R, EDX a, BL 1); the frame wait 0x67250 is a stub that adds its argument (ms) to the wait total.
The port: ja1f_replay_draw (port/ja1front replaypic.rs) on the same R, screen and clean map. Screen, clean map and
the wait total are compared.
Cases k 4..7 = replay_anim 0 ReplayFlagAnim 0x64658 (EAX s, EDX mode 1 / 2, EBX whoosh), 1 ReplayMarchAnim 0x64D20
(EAX from, EDX a neighbour, EBX kind 1 / 2, the carry in 0x9562D), 3 ReplayBurnAnim 0x643F8 (EAX s, EDX mode),
4 ReplayExplosionAnim 0x664FC (AL s); PlaySfxSlot 0x5CB34 and EscPressed? 0x67300 are stubs (0), the wait 0x67EEC
adds to the wait total like 0x67250. The port: ja1f_replay_anim. Without --port only the original runs. Exits 1 on any mismatch.
--pfix 14 (#71, #2 row 32): the original without the DrawMarkersTo call 0x64B5E of ReplayBattle, the port at fix level
`all`; half the battle shows in sectors 0..2 (the only ones where the markers reach the flash).
Then ncases // 10 battle shows (replay_anim 2, strategic.md "The battle show"): ReplayBattle 0x6494C (EAX s) with the
show byte 0x963BC = 1 and Random 0x53538 a stub that draws from a seeded list; the screen at each wait is one frame.
The port: ja1f_replay_seq with the 100 calls (2, s, j, b0, b1, b2) whose burst words this script makes from the same
Random values (the burst rules of the spec); every frame and its wait are compared.
Then ncases // 10 death pictures (replay_anim 5): ReplayDeathAnim 0x65FB0 (EAX c, EDX s, BL slot) with LoadSanimDat?
0x5E8E0 a stub, the picture loader 0x532BC a stub that puts MORFc.PCC (MF1.DAT / MF2.DAT) into its buffer at its pitch,
sprintf_ in Python; the port: ja1f_replay_seq (5, c, s, slot). Shown frames are compared with runs of the same screen
merged (their waits summed), and the end screen.
Then ncases // 5 animations (k 4..7) with every shown frame compared, not only the end screen.
Then (with --port) ncases // 6 map cells: DrawCellAndMarkers? 0x41994 with fuzzed guard marks vs island::sector: the clean map.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
import pfix                          # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP    # noqa: E402
from datarc import read, member      # noqa: E402
from pcx import decode               # noqa: E402

argv = sys.argv[1:]
PFIX = pfix.parse(argv)
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_replay_draw.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int32,
                                      ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
    PORT.ja1f_replay_draw.restype = ctypes.c_int32
    PORT.ja1f_replay_anim.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int32,
                                      ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p,
                                      ctypes.c_void_p]
    PORT.ja1f_replay_anim.restype = ctypes.c_int32
    PORT.ja1f_replay_seq.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p,
                                     ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p,
                                     ctypes.c_int32]
    PORT.ja1f_replay_seq.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

R, MARKS, SCREEN_P, CLEAN_P, MAPBUF_P, SHEET = 0x2254F0, 0x229D8C, 0x95678, 0x93E60, 0x93E5C, 0x22A4EF
DRAW = {0: 0x41C3C, 1: 0x41B24, 2: 0x41BB8, 3: 0x41C60}
ANIM = {4: (0, 0x64658), 5: (1, 0x64D20), 6: (3, 0x643F8), 7: (4, 0x664FC)}     # test k: (replay_anim k, address)
CARRY = 0x9562D

if not os.path.exists(dump):
    print(f"replaypic_diff: {dump} missing")
    sys.exit(1)
main = read("rom/DAT/MAIN.DAT")
dw, dh, dark, _ = decode(member(main, "MAPDARK.PCC"))
mw, mh, bright, _ = decode(member(main, "MAP.PCC"))
fw, fh, flags, _ = decode(member(main, "REPLAY.PCX"))
assert (dw, mw, fw, fh) == (320, 320, 330, 48) and dh == mh <= 200
mapbuf = b"".join(dark[y * 320:(y + 1) * 320] + bright[y * 320:(y + 1) * 320] for y in range(dh)) + bytes(640 * (200 - dh))
sheet = b"".join(flags[y * 330:(y + 1) * 330] + bytes(384 - 330) for y in range(48))

e = Emu(dump=dump, fast=True)
if PFIX:
    class _FrontFixes:                    # the port side is libja1front: its own copy of the fix masks
        @staticmethod
        def port_fixes(m):
            if PORT:
                PORT.ja1_port_fixes_set.argtypes = [ctypes.c_uint64]
                PORT.ja1_port_fixes_set(m)
    pfix.apply(e, PFIX, _FrontFixes)
screen, clean, mapp = e.r32(e.rt(SCREEN_P)), e.r32(e.rt(CLEAN_P)), e.r32(e.rt(MAPBUF_P))
assert len({screen, clean, mapp}) == 3, "screen, clean map and map buffer overlap in the dump"
e.uc.mem_write(mapp, mapbuf)
e.uc.mem_write(e.rt(SHEET), sheet)
e.uc.mem_write(e.rt(MARKS), bytes(60))
waits, shots, rands = [], [], []


def wait(emu, a, *_):
    waits.append(a)
    if shots is not None:
        shots.append((bytes(emu.uc.mem_read(screen, 64000)), a))
    return 0


e.stub(e.rt(0x67250), wait)
e.stub(e.rt(0x67EEC), wait)
e.stub(e.rt(0x5CB34), lambda emu, *_: 0)
e.stub(e.rt(0x67300), lambda emu, *_: 0)
snap = bytes(e.uc.mem_read(0, 16 << 20))


def count(r):
    return r.choice([0, 0, 0, 1, 2, 3, 5, 8, 10, r.randrange(16), r.randrange(256)])


def fuzz(r):
    t = bytearray()
    for s in range(60):
        rec = [r.randrange(256) if r.random() < .3 else r.randrange(2)] + [count(r) for _ in range(8)]
        if r.random() < .5:
            rec[7] = r.randrange(rec[6] + 1)
            rec[8] = r.randrange(rec[6] + 1)
        t += bytes(rec)
    k = r.randrange(8)
    a = 1 if k == 1 else r.randrange(60)     # the game calls replay_draw 1 with EDX = 1 only (DayReplay, 0x6450A)
    if k >= 4:
        s = a
        to = [t for t in (s - 10, s + 10, s - 1 if s % 10 else -1, s + 1 if s % 10 < 9 else -1) if 0 <= t < 60]
        a = [s, r.choice([1, 2]), r.randrange(2), r.randrange(256), 0]
        if k == 5:
            a[1:3] = [r.choice(to), r.choice([1, 2])]
    noise = lambda: bytes(r.randrange(256) for _ in range(64)) * 1000
    return bytes(t), k, a, noise(), noise()


def original(tbl, k, a, scr, cln):
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(R), tbl)
    e.uc.mem_write(screen, scr)
    e.uc.mem_write(clean, cln)
    waits.clear()
    shots.clear()
    if k >= 4:
        e.uc.mem_write(e.rt(CARRY), bytes([a[3]]))
        e.call(e.rt(ANIM[k][1]), *a[:3])
    elif k == 0:
        e.call(e.rt(DRAW[0]), e.rt(R))
    elif k == 1:
        e.call(e.rt(DRAW[1]), e.rt(R), a)
    elif k == 2:
        e.call(e.rt(DRAW[2]), a)
    else:
        e.call(e.rt(DRAW[3]), e.rt(R), a, 1)
    return bytes(e.uc.mem_read(screen, 64000)), bytes(e.uc.mem_read(clean, 64000)), sum(waits)


def port(tbl, k, a, scr, cln):
    s, c, w = ctypes.create_string_buffer(scr, 64000), ctypes.create_string_buffer(cln, 64000), ctypes.c_int32(0)
    t = ctypes.create_string_buffer(tbl, 540)
    if k >= 4:
        rc = PORT.ja1f_replay_anim(ISO, EXE, ctypes.addressof(t), ANIM[k][0], (ctypes.c_int32 * 5)(*a), a[3],
                                   ctypes.addressof(s), ctypes.addressof(c), ctypes.addressof(w))
        if rc:
            raise RuntimeError(f"ja1f_replay_anim returned {rc}")
        return s.raw, c.raw, w.value
    rc = PORT.ja1f_replay_draw(ISO, EXE, ctypes.addressof(t), k, a, ctypes.addressof(s), ctypes.addressof(c),
                               ctypes.addressof(w))
    if rc:
        raise RuntimeError(f"ja1f_replay_draw returned {rc}")
    return s.raw, c.raw, w.value


def first(a, b):
    n = next(j for j in range(64000) if a[j] != b[j])
    return f"@({n % 320},{n // 320}) {a[n]:#x} vs {b[n]:#x} ({sum(x != y for x, y in zip(a, b))} bytes)"


r = random.Random(1)
fails, seen = 0, set()
for i in range(ncases):
    tbl, k, a, scr, cln = fuzz(r)
    o = original(tbl, k, a, scr, cln)
    seen.add((k, o[0] != scr, o[1] != cln, o[2] > 0))
    if not PORT:
        continue
    p = port(tbl, k, a, scr, cln)
    bad = []
    if o[0] != p[0]:
        bad.append("screen " + first(o[0], p[0]))
    if o[1] != p[1]:
        bad.append("clean map " + first(o[1], p[1]))
    if o[2] != p[2]:
        bad.append(f"waits {o[2]} vs {p[2]} ms")
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i} (k {k}, a {a}): " + "; ".join(bad))


def bursts(s, vals):
    """strategic.md "The battle show": the 100 calls (s, j, b0, b1, b2) from the Random values vals (as drawn)."""
    z, m, x, y = [0] * 3, [99] * 3, [0] * 3, [0] * 3
    vals, calls = list(vals), []
    for j in range(100):
        r = 99 - j
        for k in range(3):
            L = 3 if z[k] == 0 else 8
            if m[k] < L:
                m[k] += 1
            else:
                x[k], y[k] = vals.pop(0) % 25 + 3, vals.pop(0) % 18 + 3
                z[k] = 1 if vals.pop(0) % 10 > 4 else 0
                L = 3 if z[k] == 0 else 8
                z[k], m[k] = (2, 99) if r <= L else (z[k], 0)
        calls.append([s, j] + [z[k] | m[k] << 8 | x[k] << 16 | y[k] << 24 for k in range(3)])
    return calls


nb = max(2, ncases // 10)
e.stub(e.rt(0x53538), lambda emu, n, *_: rands.pop(0) % max(n, 1) if rands else 0)
snap = bytes(e.uc.mem_read(0, 16 << 20))
for i in range(nb):
    tbl, _, _, scr, cln = fuzz(r)
    s = [0, 1, 2][i % 3] if i < 3 or (PFIX and i % 2) else r.randrange(60)   # --pfix 14: half in sectors 0..2
    vals = [r.randrange(1 << 15) for _ in range(400)]
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(R), tbl)
    e.uc.mem_write(screen, scr)
    e.uc.mem_write(clean, cln)
    e.uc.mem_write(e.rt(0x963BC), b"\x01")
    waits.clear()
    shots.clear()
    rands[:] = vals
    e.call(e.rt(0x6494C), s)
    o = list(shots)
    used = vals[:len(vals) - len(rands)]
    seen.add(("battle", o[-1][0] != scr if o else False))
    if not PORT:
        continue
    calls = bursts(s, used)
    sb, cb = ctypes.create_string_buffer(scr, 64000), ctypes.create_string_buffer(cln, 64000)
    t = ctypes.create_string_buffer(tbl, 540)
    frames, ws = ctypes.create_string_buffer(64000 * 101), (ctypes.c_int32 * 101)()
    flat = (ctypes.c_int32 * 500)(*[v for c in calls for v in c])
    nf = PORT.ja1f_replay_seq(ISO, EXE, ctypes.addressof(t), 2, flat, 100, ctypes.addressof(sb), ctypes.addressof(cb),
                              ctypes.addressof(frames), ws, 101)
    p = [(frames.raw[f * 64000:(f + 1) * 64000], ws[f]) for f in range(max(0, min(nf, 101)))]
    bad = None
    if "--verbose" in argv:
        print("battle", i, "sector", s, "frames", nf, len(o), "randoms", len(used), "changed", sum(a[0] != scr for a in o))
    if nf != len(o):
        bad = f"{nf} frames vs {len(o)}"
    else:
        for f, (a, b) in enumerate(zip(o, p)):
            if a[0] != b[0]:
                bad = f"frame {f} screen " + first(a[0], b[0])
                break
            if a[1] != b[1]:
                bad = f"frame {f} wait {a[1]} vs {b[1]} ms"
                break
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH battle case {i} (sector {s}): {bad}")
mf = {1: read("rom/DAT/MF1.DAT"), 2: read("rom/DAT/MF2.DAT")}
morf = {}


def load_pic(emu, _h, _pal, name, dst):
    """0x532BC (handle, palette, name, buffer; pitch on the stack): the strip of the archive named by the stub of
    0x5E8E0 into the buffer."""
    n = emu.cstr(name).decode().upper()
    w, h, px, _ = decode(member(mf[morf["arc"]], n))
    pitch = emu.r32(emu.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        emu.uc.mem_write(dst + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


e.stub(e.rt(0x5E8E0), lambda emu, a, *_: morf.__setitem__("arc", 1 if b"mf1" in emu.cstr(a).lower() else 2) or 0)
e.stub(e.rt(0x532BC), load_pic, pop=4)
e.stub_sprintf()
snap = bytes(e.uc.mem_read(0, 16 << 20))


def runs(fr):
    """Frames (screen, ms) with runs of the same screen merged."""
    out = []
    for scr, ms in fr:
        if out and out[-1][0] == scr:
            out[-1][1] += ms
        else:
            out.append([scr, ms])
    return out


faces = [c for c in range(66) if c != 66]
nd = max(2, ncases // 10)
for i in range(nd):
    tbl, _, _, scr, cln = fuzz(r)
    c, s, slot = ([0, 30, 65][i] if i < 3 else r.choice(faces)), r.randrange(60), r.randrange(8)
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(R), tbl)
    e.uc.mem_write(screen, scr)
    e.uc.mem_write(clean, cln)
    e.uc.mem_write(e.rt(0x963BC), b"\x01")
    shots.clear()
    e.call(e.rt(0x65FB0), c, s, slot)
    o, oend = runs(shots), bytes(e.uc.mem_read(screen, 64000))
    seen.add(("death", any(a[0] != scr for a in o)))
    if not PORT:
        continue
    sb, cb = ctypes.create_string_buffer(scr, 64000), ctypes.create_string_buffer(cln, 64000)
    t = ctypes.create_string_buffer(tbl, 540)
    frames, ws = ctypes.create_string_buffer(64000 * 64), (ctypes.c_int32 * 64)()
    nf = PORT.ja1f_replay_seq(ISO, EXE, ctypes.addressof(t), 5, (ctypes.c_int32 * 5)(c, s, slot, 0, 0), 1,
                              ctypes.addressof(sb), ctypes.addressof(cb), ctypes.addressof(frames), ws, 64)
    p = runs([(frames.raw[f * 64000:(f + 1) * 64000], ws[f]) for f in range(max(0, min(nf, 64)))])
    if p and p[-1][1] == 0:     # (the port shows the end screen as a frame with no wait; the original at its next frame)
        p.pop()
    if "--verbose" in argv:
        print("death", i, "face", c, "sector", s, "frames", len(o), len(p), "ms", sum(x[1] for x in o), sum(x[1] for x in p))
    bad = None
    if len(o) != len(p):
        bad = f"{len(p)} frames vs {len(o)}"
    for f, (a, b) in enumerate(zip(o, p)):
        if a[0] != b[0]:
            bad = f"frame {f} screen " + first(a[0], b[0])
            break
        if a[1] != b[1]:
            bad = f"frame {f} wait {a[1]} vs {b[1]} ms"
            break
    if not bad and oend != sb.raw:
        bad = "end screen " + first(oend, sb.raw)
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH death case {i} (face {c}, sector {s}, slot {slot}): {bad}")
# DrawCellAndMarkers? 0x41994 (AssignScreen? 0x4A320, EAX = the sector table, EDX = s): CleanMapCell and DrawMarkersTo into
# the clean map, with fuzzed guard marks; the port: island::sector, which ReplayPic::clean_cell uses (ja1f_map_cell).
ncell = ncases // 6 if PORT else 0
cell_changed = 0
r = random.Random(0x41994)
if PORT:
    PORT.ja1f_map_cell.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_map_cell.restype = ctypes.c_int32
for i in range(ncell):
    tbl, _, _, scr, cln = fuzz(r)
    a = r.randrange(60)
    mark = r.choice([0, 0, 1, 2, 3, -1, -2, r.randrange(-128, 128)])
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(R), tbl)
    e.uc.mem_write(screen, scr)
    e.uc.mem_write(clean, cln)
    e.uc.mem_write(e.rt(MARKS) + a, bytes([mark & 0xFF]))
    e.call(e.rt(0x41994), e.rt(R), a)
    o_clean = bytes(e.uc.mem_read(clean, 64000))
    o_screen = bytes(e.uc.mem_read(screen, 64000))
    pc = ctypes.create_string_buffer(cln, 64000)
    tb_ = ctypes.create_string_buffer(tbl, 540)
    q_ = PORT.ja1f_map_cell(ISO, EXE, tb_, a, mark, pc)
    cell_changed += o_clean != cln
    if q_ != 0 or o_clean != pc.raw[:64000] or o_screen != scr:
        fails += 1
        diff = [j for j in range(64000) if o_clean[j] != pc.raw[j]]
        print(f"cell case {i}: sector {a} mark {mark} port {q_}: {len(diff)} clean bytes differ, first {diff[:3]}, screen same {o_screen == scr}")
if PORT and not cell_changed:
    print("replaypic_diff: DrawCellAndMarkers? never changed the clean map")
    fails += 1
if not any(x[0] == "death" and x[1] for x in seen):
    print("replaypic_diff: the death picture never changed the screen")
    fails += 1
if not any(x[0] == "battle" and x[1] for x in seen):
    print("replaypic_diff: the battle show never changed the screen")
    fails += 1
for k in range(8):
    if not any(x[0] == k and x[1] for x in seen):
        print(f"replaypic_diff: {f'replay_draw {k}' if k < 4 else f'replay_anim {ANIM[k][0]}'} never changed the screen")
        fails += 1
# Ledger rows 5650 + 5651: the k 4..7 cases above compare the end screen only, and the rising flag of ReplayFlagAnim
# 0x64658 is gone at the end. So every shown frame of each animation is compared here (runs of the same screen merged).
nanim = max(4, ncases // 5)
frames_a, ws_a = ctypes.create_string_buffer(64000 * 256) if PORT else None, (ctypes.c_int32 * 256)()
NMADE = 12
for i in range(nanim + NMADE):
    tbl, _, _, scr, cln = fuzz(r)
    k = 4 + i % 4
    s = r.randrange(60)
    to = [t for t in (s - 10, s + 10, s - 1 if s % 10 else -1, s + 1 if s % 10 < 9 else -1) if 0 <= t < 60]
    a = [s, r.choice([1, 2]), r.randrange(2), 0, 0]
    if k == 5:
        a[1:3] = [r.choice(to), r.choice([1, 2])]
    if i >= nanim:
        # Ledger row 6183 (the north side of the workable trees): the burn animation in mode 2 sets sector s owned and
        # counts its workable trees again. Every sector is owned and has no barrier bit, only the north neighbour of s
        # is not owned: 3 of 4 sides are secure, so 15 of the 20 trees are workable. The fuzz table above almost never
        # gives a sector that is owned, connected to home and has trees, so no case showed the north side before.
        mr = random.Random(f"workable{i}")
        k, s = 6, mr.randrange(10, 60)
        t9 = bytearray(tbl)
        for q in range(60):
            t9[9 * q] = 1
            t9[9 * q + 1] = 0
        t9[9 * (s - 10)] = 0
        t9[9 * s] = 0
        t9[9 * s + 6:9 * s + 9] = bytes([20, 0, 0])
        tbl, a = bytes(t9), [s, 2, 0, 0, 0]
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(R), tbl)
    e.uc.mem_write(screen, scr)
    e.uc.mem_write(clean, cln)
    e.uc.mem_write(e.rt(CARRY), b"\x00")
    shots.clear()
    e.call(e.rt(ANIM[k][1]), *a[:3])
    o = runs(shots + [(bytes(e.uc.mem_read(screen, 64000)), 0)])     # the last picture has no wait behind it
    seen.add(("anim frames", k, len(o) > 1))
    if not PORT:
        continue
    sb, cb = ctypes.create_string_buffer(scr, 64000), ctypes.create_string_buffer(cln, 64000)
    t = ctypes.create_string_buffer(tbl, 540)
    nf = PORT.ja1f_replay_seq(ISO, EXE, ctypes.addressof(t), ANIM[k][0], (ctypes.c_int32 * 5)(*a), 1, ctypes.addressof(sb),
                              ctypes.addressof(cb), ctypes.addressof(frames_a), ws_a, 256)
    # The port gives only the frames that a player sees (play::replay_shown, also the window's rule): an in-between
    # copy with the next frame right behind it is not shown (the original draws straight on the VGA memory and
    # only its wait keeps a picture on the screen). The last port frame is the end screen.
    p = runs([(frames_a.raw[f * 64000:(f + 1) * 64000], ws_a[f]) for f in range(max(0, min(nf, 256)))] + [(sb.raw, 0)])
    bad = None
    if len(o) != len(p):
        bad = f"{len(p)} frames vs {len(o)}"
    else:
        for f, (x, y) in enumerate(zip(o, p)):
            if x[0] != y[0]:
                bad = f"frame {f} screen " + first(x[0], y[0])
                break
            if x[1] != y[1]:
                bad = f"frame {f} wait {x[1]} vs {y[1]} ms"
                break
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH anim frames case {i} (k {k}, a {a}): {bad}")
print(f"replaypic_diff{' (port)' if PORT else ''}{' --pfix ' + ','.join(map(str, PFIX)) if PFIX else ''}: {ncases} cases + {nb} battle shows + {nd} death pictures + {ncell} map cells + {nanim} animations frame by frame + {NMADE} made tree cases, {fails} mismatches")
sys.exit(1 if fails else 0)
