"""Fan patch 1.15m fixes (#70): each fix of the port's fan_fixes toggle (ja1logic fanfix.rs) against the 1.15m files.

xra  F01 (data): every soldier template of every DAT/XRA.DAT record (59 sectors x 16 templates of 0x200 bytes), as
     the port reads it (ja1_xra_fan_fix, called by the sector set-up on each template read): with F01 on = the
     1.15m file's bytes, with the mask 0 = the 1.13b file's bytes. Also: outside the templates (tile bytes, items)
     the two files are equal, so the table covers every change; the 1.15m code that reads the file is not changed
     (no 1.15m byte run in SetupSectorFromXra's XRA read part 0x2E918..0x2EDC0: template fread 0x2EC89, tile and item freads up to 0x2EDAB).
maxap F06 (code, #70 row 88, 1.15m switch HEALTHY): MercMaxAP 0x56210 of the 1.15m code (Emu.put_115m; at 0x562AA it
     jumps to 0x3C042, which tests the switch byte 0x91463 bit 0 through 0x42760) vs the port's ja1_max_ap, on
     fuzzed soldiers (health, max health, wounds +0x185, stats, energy) of each fight dump: switch on vs F06 on and
     switch off vs mask 0. --plant F06: the port runs with F06 off in the switch-on round (the proof that it catches).
melee F06 at MeleeChance 0x56A78 (1.15m 0x56B66 -> 0x3C007 attacker skill, 0x56C4C -> 0x3C025 defender skill; no
     other 1.15m byte run in the function) vs the port's ja1_melee_chance: fuzzed attacker + defender (health, wounds,
     stats, level, energy, knife/fist item, side, aim), same two rounds and --plant as maxap.
aim, throw F06 at ChanceToHit 0x56630 (1.15m 0x5698F -> 0x3BFCF) and ThrowChance 0x58BB0 (1.15m 0x58D2F ->
     0x3BFED): both functions hold other 1.15m fixes (ChanceToHit +0xC7/+0xED/+0x202, ThrowChance row 89), so the
     original side gets only the F06 bytes (Emu.put_115m(only=...): the site, the stubs 0x3BFCF..0x3C059, the switch
     test 0x42751..0x427B4 and the tail) over the 1.13b code; port ja1_chance_to_hit / ja1_throw_chance. Fuzzed
     shooter (health, wounds, stats, energy, hand item) at the tile of another soldier, aim 0..4.
throw89 F42 (#70 row 89) at ThrowChance 0x58BB0: the 1.15m site 0x58BFE (call tail 0x8AE8E: -20 when +0x1B9 = 4, lower
     limit 1) over 1.13b; round 1 = 1.15m site vs F42 on, round 0 = 1.13b bytes vs mask 0. Same fuzz plus +0x1B9
     (half the cases 4). --plant: F42 off in round 1.

Skips (exit 0, "SKIP") without third_party/ja115m. usage: .venv/bin/python validate/diff/fanfix_diff.py [PART...]
"""
import ctypes
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(ROOT, "tools"))
import port_lib  # noqa: E402

P = os.path.join(ROOT, "third_party", "ja115m")
XRA_113B, XRA_115M = (os.path.join(P, "patch", d, "DAT", "XRA.DAT") for d in ("JA113B", "JA115"))
REC, TPL = 0x2E98, 0x200
F = lambda n: 1 << (n - 1)


def part_xra():
    a, b = open(XRA_113B, "rb").read(), open(XRA_115M, "rb").read()
    lib = port_lib.lib()
    lib.ja1_xra_fan_fix.argtypes, lib.ja1_xra_fan_fix.restype = [ctypes.c_int32, ctypes.c_int32, ctypes.c_char_p], None
    bad = cases = changed = 0
    for s in range(len(a) // REC):
        rec_a, rec_b = a[s * REC:(s + 1) * REC], b[s * REC:(s + 1) * REC]
        if rec_a[16 * TPL:] != rec_b[16 * TPL:]:
            print(f"MISMATCH xra sector {s}: the files differ outside the templates")
            bad += 1
        for r in range(16):
            ta, tb = rec_a[r * TPL:(r + 1) * TPL], rec_b[r * TPL:(r + 1) * TPL]
            for mask, want in ((F(1), tb), (0, ta)):
                port_lib.fan_fixes(mask)
                buf = ctypes.create_string_buffer(ta, TPL)
                lib.ja1_xra_fan_fix(s, r, buf)
                cases += 1
                if buf.raw != want:
                    bad += 1
                    if bad <= 10:
                        d = [hex(i) for i in range(TPL) if buf.raw[i] != want[i]][:6]
                        print(f"MISMATCH xra sector {s} template {r} mask {mask:#x}: offsets {d}")
            changed += ta != tb
    port_lib.fan_fixes(0)
    runs = [l for l in open(os.path.join(P, "diffs", "exe_1.13b_to_1.15m.txt"))
            if (m := re.match(r"0x([0-9A-F]{6}) ", l)) and 0x2E918 <= int(m[1], 16) < 0x2EDC0]
    if runs:
        print(f"MISMATCH xra: 1.15m changes the template loop: {runs[0].strip()[:80]}")
        bad += 1
    print(f"CASES xra={cases}")
    print(f"xra: {cases} template reads ({changed} templates changed by 1.15m), {bad} mismatches")
    return bad


def part_maxap(n=1500):
    import glob
    import random
    import ai  # noqa: E402
    from emu import Emu  # noqa: E402
    plant = "--plant" in sys.argv
    dumps = sorted(glob.glob(os.path.join(ROOT, "build", "mem_fight2*.bin")))[:3]
    rng = random.Random(6)
    bad = cases = bandaged = 0
    for path in dumps:
        e = Emu(dump=path, fast=True)
        e.put_115m()
        sold = e.rt(ai.G_SOLDIERS)
        slots = [k for k in list(range(16)) + list(range(0x28, 0x30)) if e.r8(sold + k * 0x200 + 0x183)]
        bufs, scalars = port_lib.world_load(e, ai)
        for i in range(n // len(dumps)):
            k = rng.choice(slots)
            base = sold + k * 0x200
            keep = bytes(e.uc.mem_read(base, 0x200))
            mh = rng.randrange(1, 101)
            h = rng.randrange(0, mh + 1)
            e.w8(base + 0x184, mh)
            e.w8(base + 0x183, h)
            e.w8(base + 0x185, rng.choice([mh - h, rng.randrange(0, mh - h + 1), rng.randrange(256)]) & 0xFF)
            for o in (0x18A, 0x18B, 0x192, 0x1A9):
                if rng.random() < 0.5:
                    e.w8(base + o, rng.choice([rng.randrange(1, 101), rng.randrange(256)]))
            bufs["soldiers"][k * 0x200:(k + 1) * 0x200] = e.uc.mem_read(base, 0x200)
            bandaged += h < mh and e.r8(base + 0x185) < mh - h
            for sw, mask in ((1, F(6)), (0, 0)):
                e.w8(e.rt(0x91463), sw)
                port_lib.fan_fixes(0 if plant else mask)
                want = e.call(e.rt(0x56210), base) & 0xFF
                got = port_lib.ai_run("max_ap", bufs, scalars, k, lambda q: 0, lambda op, a: (0, None)) & 0xFF
                cases += 1
                if got != want:
                    bad += 1
                    if bad <= 10:
                        print(f"MISMATCH maxap {os.path.basename(path)} slot {k} switch {sw}: original {want} port {got}"
                              f" | hp {h}/{mh} wounds {e.r8(base + 0x185)}")
            e.uc.mem_write(base, keep)
            bufs["soldiers"][k * 0x200:(k + 1) * 0x200] = keep
    port_lib.fan_fixes(0)
    print(f"CASES maxap={cases}")
    print(f"maxap: {cases} MercMaxAP calls on {len(dumps)} dumps ({bandaged} with a bandage), {bad} mismatches")
    return bad


def part_melee(n=1500):
    import glob
    import random
    import ai  # noqa: E402
    from emu import Emu, EmuStop  # noqa: E402
    plant = "--plant" in sys.argv
    dumps = sorted(glob.glob(os.path.join(ROOT, "build", "mem_fight2*.bin")))[:3]
    rng = random.Random(7)
    bad = cases = nonzero = divs = 0
    for path in dumps:
        e = Emu(dump=path, fast=True)
        e.put_115m()
        sold = e.rt(ai.G_SOLDIERS)
        slots = [k for k in list(range(16)) + list(range(0x28, 0x30)) if e.r8(sold + k * 0x200 + 0x183)]
        bufs, scalars = port_lib.world_load(e, ai)
        for i in range(n // len(dumps)):
            a, b = rng.sample(slots, 2)
            keep = {k: bytes(e.uc.mem_read(sold + k * 0x200, 0x200)) for k in (a, b)}
            for k in (a, b):
                base = sold + k * 0x200
                mh = rng.randrange(1, 101)
                h = rng.randrange(1, mh + 1)
                e.w8(base + 0x184, mh)
                e.w8(base + 0x183, h)
                e.w8(base + 0x185, rng.choice([mh - h, rng.randrange(0, mh - h + 1), rng.randrange(256)]) & 0xFF)
                for o in (0x18A, 0x18B, 0x192, 0x1A9, 0x1B9):
                    if rng.random() < 0.5:
                        e.w8(base + o, rng.choice([rng.randrange(1, 101), rng.randrange(256), 4]))
                bufs["soldiers"][k * 0x200:(k + 1) * 0x200] = e.uc.mem_read(base, 0x200)
            if rng.random() < 0.9:
                e.w8(sold + a * 0x200, rng.choice([0x0F, 0x10]))
                bufs["soldiers"][a * 0x200] = e.r8(sold + a * 0x200)
            aim = rng.randrange(5)
            for sw, mask in ((1, F(6)), (0, 0)):
                e.w8(e.rt(0x91463), sw)
                port_lib.fan_fixes(0 if plant else mask)
                ops = []
                got = port_lib.ai_run("melee_chance", bufs, scalars, a, lambda q: 0,
                                      lambda op, x: (ops.append(op), (0, None))[1], fallback=(b, aim)) & 0xFF
                if "div_zero" in ops:    # skill a + skill d = 0: the original's divide fault (Unicorn cannot go on)
                    divs += 1
                    continue
                try:
                    want = e.call(e.rt(0x56A78), sold + a * 0x200, sold + b * 0x200, aim) & 0xFF
                except EmuStop as x:     # a divide fault the port did not have: a mismatch; a fresh emulator
                    want = str(x)
                    e = Emu(dump=path, fast=True)
                    e.put_115m()
                    for k in (a, b):
                        e.uc.mem_write(sold + k * 0x200, bytes(bufs["soldiers"][k * 0x200:(k + 1) * 0x200]))
                cases += 1
                nonzero += want not in (0, None) and isinstance(want, int)
                if got != want:
                    bad += 1
                    if bad <= 10:
                        print(f"MISMATCH melee {os.path.basename(path)} {a}->{b} aim {aim} switch {sw}: "
                              f"original {want} port {got}")
            for k in (a, b):
                e.uc.mem_write(sold + k * 0x200, keep[k])
                bufs["soldiers"][k * 0x200:(k + 1) * 0x200] = keep[k]
    port_lib.fan_fixes(0)
    print(f"CASES melee={cases}")
    print(f"melee: {cases} MeleeChance calls on {len(dumps)} dumps ({nonzero} with a knife or fist, {divs} divide faults skipped), {bad} mismatches")
    return bad


F06_ONLY = [(0x3BFCF, 0x3C059), (0x42751, 0x427B4), (0x5698F, 0x56994), (0x58D2F, 0x58D34)]


def part_tile_chance(name, addr, n=1500, part=None, fid=0, sites=()):
    import glob
    import random
    import ai  # noqa: E402
    from emu import Emu  # noqa: E402
    plant = "--plant" in sys.argv
    dumps = sorted(glob.glob(os.path.join(ROOT, "build", "mem_fight2*.bin")))[:3]
    rng = random.Random(8)
    bad = cases = hurt = 0
    for path in dumps:
        e = Emu(dump=path, fast=True)
        olds = [bytes(e.uc.mem_read(e.rt(lo), k)) for lo, k in sites]
        e.put_115m(only=[(lo, lo + k) for lo, k in sites] if fid else F06_ONLY)
        news = [bytes(e.uc.mem_read(e.rt(lo), k)) for lo, k in sites]
        sold = e.rt(ai.G_SOLDIERS)
        slots = [k for k in list(range(16)) + list(range(0x28, 0x30)) if e.r8(sold + k * 0x200 + 0x183)
                 and e.r16(sold + k * 0x200 + 0x114) < 0xC40]
        bufs, scalars = port_lib.world_load(e, ai)
        for i in range(n // len(dumps)):
            a, b = rng.sample(slots, 2)
            base = sold + a * 0x200
            keep = bytes(e.uc.mem_read(base, 0x200))
            mh = rng.randrange(1, 101)
            h = rng.randrange(1, mh + 1)
            e.w8(base + 0x184, mh)
            e.w8(base + 0x183, h)
            e.w8(base + 0x185, rng.choice([mh - h, rng.randrange(0, mh - h + 1), rng.randrange(256)]) & 0xFF)
            for o in (0x18A, 0x18B, 0x18C, 0x191, 0x192, 0x1A9):
                if rng.random() < 0.5:
                    e.w8(base + o, rng.choice([rng.randrange(1, 101), rng.randrange(256)]))
            if fid == 42:
                e.w8(base + 0x1B9, rng.choice([4, 4, 0, rng.randrange(8)]))
            if fid == 43:                         # sun goggles 0x27 in entry 3 / scope 0x42 in entry 0, condition
                if rng.random() < 0.6:
                    e.w8(base + 0x24, 0x27)
                    e.w8(base + 0x26, rng.choice([rng.randrange(20), rng.randrange(101)]))
                if name == "chance_to_hit" and rng.random() < 0.6:
                    e.w8(base + 3, 0x42)
                    e.w8(base + 5, rng.choice([rng.randrange(20), rng.randrange(101)]))
            bufs["soldiers"][a * 0x200:(a + 1) * 0x200] = e.uc.mem_read(base, 0x200)
            hurt += {42: e.r8(base + 0x1B9) == 4, 43: e.r8(base + 0x24) == 0x27 or e.r8(base + 3) == 0x42}.get(fid, h < mh)
            tile = e.r16(sold + b * 0x200 + 0x114)
            if fid == 43 and rng.random() < 0.5:
                tile = rng.randrange(0xC40)               # far tiles: the scope term needs distance > 150
            aim = rng.randrange(5)
            for sw, mask in (((1, F(fid)), (0, 0)) if fid else ((1, F(6)), (0, 0))):
                if fid:
                    for (lo, _k), old, new in zip(sites, olds, news):
                        e.uc.mem_write(e.rt(lo), new if sw else old)
                    e.uc.ctl_flush_tb()           # else Unicorn runs the code translated before the write
                else:
                    e.w8(e.rt(0x91463), sw)
                port_lib.fan_fixes(0 if plant else mask)
                got = port_lib.ai_run(name, bufs, scalars, a, lambda q: 0, lambda op, x: (0, None),
                                      fallback=(tile, aim)) & 0xFFFFFFFF
                want = e.call(e.rt(addr), base, tile, aim) & 0xFFFFFFFF
                if name == "throw_chance":
                    got, want = got & 0xFF, want & 0xFF
                cases += 1
                if got != want:
                    bad += 1
                    if bad <= 10:
                        print(f"MISMATCH {name} {os.path.basename(path)} {a} at {tile} aim {aim} switch {sw}: "
                              f"original {want} port {got} | hp {h}/{mh} wounds {e.r8(base + 0x185)}")
            e.uc.mem_write(base, keep)
            bufs["soldiers"][a * 0x200:(a + 1) * 0x200] = keep
    port_lib.fan_fixes(0)
    part = part or ('aim' if name == 'chance_to_hit' else 'throw')
    print(f"CASES {part}={cases}")
    print(f"{part} {name}: {cases} calls on {len(dumps)} dumps ({hurt} {dict(throw89='bandaged', aim93='goggles/scope', throw93='goggles').get(part, 'wounded')} shooters), "
          f"{bad} mismatches")
    return bad


PARTS = {"xra": part_xra, "maxap": part_maxap, "melee": part_melee,
         "aim": lambda: part_tile_chance("chance_to_hit", 0x56630),
         "throw": lambda: part_tile_chance("throw_chance", 0x58BB0),
         "throw89": lambda: part_tile_chance("throw_chance", 0x58BB0, part="throw89", fid=42, sites=[(0x58BFE, 6)]),
         "aim93": lambda: part_tile_chance("chance_to_hit", 0x56630, part="aim93", fid=43,
                                           sites=[(0x566F7, 5), (0x5671D, 6)]),
         "throw93": lambda: part_tile_chance("throw_chance", 0x58BB0, part="throw93", fid=43, sites=[(0x58C50, 5)])}


def main():
    if not os.path.exists(XRA_115M):
        print("SKIP fanfix_diff: third_party/ja115m missing")
        return 0
    want = [a for a in sys.argv[1:] if not a.startswith("-")] or list(PARTS)
    bad = sum(PARTS[p]() for p in want)
    print(f"fanfix_diff: {len(want)} parts, {bad} mismatches")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
