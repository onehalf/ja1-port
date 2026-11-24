"""Differential test: the clean-room port's ja1_sight_line (library op 49, ai.md "Sight line") vs the original
SightLine 0x58760 in Unicorn.

usage: .venv/bin/python validate/diff/sight_diff.py [n=4000] [DUMP ...] [--mirror]
--mirror: tools/ai.py sight_line (the analysis mirror) instead of the port.
Each case: a looker slot, a start pixel (its own x + 2, y + 4 or another point), a target tile and pixel within 20
tiles, the old flag, the range and the seen slot (its +0x1B1 / +0x1B3 fuzzed); up to 8 tiles near the line get a
window (0x37..0x82), brush (0x27..0x36), 0xE2 / 0xE3 or an empty object. LineOfFire 0x580CC (op 42 fire_line) is
scripted on both sides; direction (op 6) runs for real in both. Compared: the result (low byte), the fire_line
calls (slot, tile, thrown) and Random calls (none expected). Memory changes are undone after each case.
Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402

SIGHT_LINE, LINE_OF_FIRE, RANDOM = 0x58760, 0x580CC, 0x53538
OBJ = 1                              # tile byte 1 = object
nums = [a for a in sys.argv[1:] if a.isdigit()]
n = int(nums[0]) if nums else 4000
dumps = [a for a in sys.argv[1:] if not a.isdigit() and not a.startswith("-")] or \
    [os.path.join(HERE, "..", "..", "build", f"mem_{k}.bin") for k in ("tac60", "combat1", "death1", "fight20", "a10")]
dumps = [d for d in dumps if os.path.exists(d)]
if not dumps:
    raise SystemExit("sight_diff: no dump (build/mem_tac60.bin)")
MIRROR = "--mirror" in sys.argv
HOST_OPS = port_lib.ALL_OPS & ~(1 << 6)      # direction: the library's own; fire_line: the host (scripted)
fails = total = blocked = 0
results = {}
for di, dump in enumerate(dumps):
    e = Emu(dump=dump, fast=True)
    rt = e.rt
    rng = random.Random(1000 + di)
    fire, log = [0], []

    def stub_fire(emu, eax, edx, ebx, *_):
        log.append(("fire_line", ((eax - rt(ai.G_SOLDIERS)) // 0x200, ai.s16(edx & 0xFFFF), ebx & 0xFF)))
        return fire[0] & 0xFFFFFFFF

    def stub_random(emu, eax, *_):
        log.append(("Random", (ai.s16(eax & 0xFFFF),)))
        return 0
    e.stub(rt(LINE_OF_FIRE), stub_fire)
    e.stub(rt(RANDOM), stub_random)
    soldiers = rt(ai.G_SOLDIERS)
    in_play = [k for k in range(48) if e.uc.mem_read(soldiers + 0x200 * k + 0x182, 1)[0]] or [0]
    per = n // len(dumps)
    for i in range(per):
        total += 1
        s = rng.choice(in_play + [rng.randrange(48)])
        rec = soldiers + 0x200 * s
        sx_, sy_ = (ai.s16(int.from_bytes(e.uc.mem_read(rec + o, 2), "little")) for o in (0x116, 0x118))
        if rng.random() < 0.6 and 0 <= sx_ < 56 * 16 - 4 and 0 <= sy_ < 56 * 14 - 6:
            x, y = sx_ + 2, sy_ + 4
        else:
            t0 = rng.randrange(3136)
            x, y = ai.tile_x(t0) + rng.randrange(-5, 11), ai.tile_y(t0) + rng.randrange(-5, 9)
        x, y = min(max(x, 0), 56 * 16 - 1), min(max(y, 0), 56 * 14 - 1)
        col, row = x // 16, y // 14
        tc = min(max(col + rng.randrange(-20, 21), 0), 55)
        tr = min(max(row + rng.randrange(-20, 21), 0), 55)
        tile = tr * 56 + tc
        tx, ty = ai.tile_x(tile) + 2, ai.tile_y(tile) + 4
        if rng.random() < 0.3:
            tx, ty = tx + rng.randrange(-7, 9), ty + rng.randrange(-8, 6)
        tx, ty = min(max(tx, 0), 56 * 16 - 1), min(max(ty, 0), 56 * 14 - 1)
        T = rng.choice([tile, tile, ty // 14 * 56 + tx // 16, rng.randrange(3136)])
        flag = rng.choice([0, 0, 1, 1, 2, 0x101])
        rng_ = rng.choice([14, 14, 14, 12, 8, 3, 0, 20, rng.randrange(-3, 40)])
        seen = rng.choice([0x40, 0x40, rng.randrange(48), rng.randrange(48), 0xFF, 0x140 + rng.randrange(48)])   # real slots
        if rng.random() < 0.4:           # edge cases: the target near the step limit (range x 15 +- a window's 30)
            major = max(1, (rng_ & 0xFF) % 30 * 15 + rng.choice([-31, -30, -29, -16, -15, -14, -1, 0, 1, 2, 29, 30, 31]))
            minor = rng.randrange(major + 1)
            ddx, ddy = (major, minor) if rng.random() < 0.5 else (minor, major)
            tx = min(max(x + ddx * rng.choice([-1, 1]), 0), 56 * 16 - 1)
            ty = min(max(y + ddy * rng.choice([-1, 1]), 0), 56 * 14 - 1)
            tile = ty // 14 * 56 + tx // 16
            T = rng.choice([tile, tile, rng.randrange(3136)])
        undo = []

        def poke(addr, data):
            undo.append((addr, bytes(e.uc.mem_read(addr, len(data)))))
            e.uc.mem_write(addr, bytes(data))
        if (seen & 0xFF) < 0x40:
            v = soldiers + 0x200 * (seen & 0xFF)
            poke(v + 0x1B1, [rng.choice([0, 0, 1, rng.randrange(256)])])
            poke(v + 0x1B3, [rng.choice([0, 1, 2, 3, 4, 5, 6, 7, 8, rng.randrange(256)])])
        steps = max(abs(tx - x), abs(ty - y), 1)
        for _ in range(rng.choice([0, 1, 2, 4, 8])):
            f = rng.random() if rng.random() < 0.5 else min(1.0, rng.choice([1, 2, 5, 10, 14, 15, 16, 17]) / steps)
            px, py = x + (tx - x) * f, y + (ty - y) * f
            t = int(py) // 14 * 56 + int(px) // 16
            if 0 <= t < 3136:
                o = rng.choice([rng.randrange(0x37, 0x83), rng.randrange(0x37, 0x83), rng.randrange(0x27, 0x37),
                                rng.randrange(0x27, 0x37), 0xE2, 0xE3, 0, rng.randrange(256)])
                poke(rt(ai.G_TILEMAP) + 8 * t + OBJ, [o])
        fire[0] = rng.choice([0, 24, 25, 26, 100, -1, rng.randrange(1 << 32)])
        log.clear()
        got = e.call(rt(SIGHT_LINE), rec, x, y, T, tx, ty, flag, rng_ & 0xFFFFFFFF, seen) & 0xFF
        want_log = log[:]
        blocked += bool(want_log)
        log.clear()

        class W:                     # the mirror's world: emulator memory
            g8 = staticmethod(lambda a: e.r8(rt(a)))
            g32 = staticmethod(lambda a: e.r32(rt(a)))
            s8 = staticmethod(lambda a, v: None)          # b95680: no reader
            tile8 = staticmethod(lambda t, k: e.r8(rt(ai.G_TILEMAP) + 8 * t + k))

            @staticmethod
            def ext(name, *a):
                if name == "direction":
                    return ai.direction_to(*(ai._i32(v) for v in a))
                if name == "fire_line":
                    log.append(("fire_line", (a[0][0x17A], ai.s16(a[1] & 0xFFFF), a[2] & 0xFF)))
                    return fire[0]
                raise SystemExit(f"mirror called {name}")
        if MIRROR:
            r_ = bytearray(e.uc.mem_read(rec, 0x200))
            res = ai.sight_line(r_, W, x, y, T, tx, ty, flag, rng_, seen) & 0xFF
            for addr, data in reversed(undo):
                e.uc.mem_write(addr, data)
            results[got] = results.get(got, 0) + 1
            if (got, want_log) != (res, log):
                fails += 1
                if fails <= 10:
                    print(f"MISMATCH {os.path.basename(dump)} case {i}: original {got} {want_log} mirror {res} {log} | "
                          f"s {s} ({x},{y}) -> T {T} ({tx},{ty}) flag {flag:#x} range {rng_} seen {seen:#x}")
            continue
        bufs, scalars = port_lib.world_load(e, ai)

        def call(op, a):
            a = [v & 0xFFFFFFFF for v in a]
            if op == "fire_line":
                log.append(("fire_line", (a[0], ai.s16(a[1] & 0xFFFF), a[2] & 0xFF)))
                return fire[0], None
            raise SystemExit(f"port called host op {op} {a}")

        def random_(k):
            log.append(("Random", (ai.s16(k & 0xFFFF),)))
            return 0
        res = port_lib.sector_run("sight_line", bufs, scalars, [s, x, y, T, tx, ty, flag, rng_, seen], random_, call,
                                  HOST_OPS) & 0xFF
        for addr, data in reversed(undo):
            e.uc.mem_write(addr, data)
        results[got] = results.get(got, 0) + 1
        if (got, want_log) != (res, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH {os.path.basename(dump)} case {i}: original {got} {want_log} port {res} {log} | "
                      f"s {s} ({x},{y}) -> T {T} ({tx},{ty}) flag {flag:#x} range {rng_} seen {seen:#x}")
print(f"sight_diff: {total} cases on {len(dumps)} dumps (results {dict(sorted(results.items()))}, "
      f"{blocked} blocked with fire_line), {fails} mismatches")
sys.exit(1 if fails else 0)
