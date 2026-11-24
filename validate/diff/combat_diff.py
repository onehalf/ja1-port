"""Differential test: tools/combat.py vs the original MercMaxAP 0x56210 / ShotApCost 0x564F8 in Unicorn.

usage: .venv/bin/python validate/diff/combat_diff.py [DUMP ...] [nfuzz=3000]
Soldiers from live dumps (build/mem_*.bin) plus fuzzed stat/health/weapon bytes. Exits 1 on any mismatch.
"""
import glob, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import combat                        # noqa: E402
from emu import Emu                  # noqa: E402
from memdump import Dump             # noqa: E402

MAX_AP, SHOT_COST = 0x56210, 0x564F8
args = [a for a in sys.argv[1:] if not a.isdigit()]
nfuzz = int(([a for a in sys.argv[1:] if a.isdigit()] or ["3000"])[0])
dumps = args or sorted(glob.glob(os.path.join(HERE, "..", "..", "build", "mem_fight2*.bin")))
rng = random.Random(1)
e = Emu()
buf = e.alloc(0x200)
fails = n = 0


def wpn(t):
    return bytes(e.uc.mem_read(combat.WEAPONS + 6 * t, 6))


def test(s, tile):
    global fails, n
    e.uc.mem_write(buf, bytes(s))
    n += 1
    for what, got, want in (("MaxAP", e.call(MAX_AP, buf) & 0xFF, combat.max_ap(s)),
                            ("ShotCost", e.call(SHOT_COST, buf, tile) & 0xFF,
                             combat.shot_ap_cost(s, tile, wpn(s[0])))):
        if got != want:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH {what}: original {got} port {want} | type {s[0]} hp {s[0x183]}/{s[0x184]}/"
                      f"{s[0x185]} {bytes(s[0x18A:0x193]).hex(' ')} tile {tile}")


real = []
for p in dumps:
    d = Dump(p)
    for k in list(range(16)) + list(range(0x28, 0x30)):
        s = bytearray(d.read(0xFA958 + k * 0x200, 0x200))
        if s[0x183]:
            real.append(s)
            test(s, struct.unpack_from("<H", d.read(0xFA958 + 0x28 * 0x200, 0x200), 0x114)[0])
print(f"{n} dumped soldiers from {len(dumps)} dumps")
for i in range(nfuzz):
    s = bytearray(rng.choice(real)) if real else bytearray(0x200)
    s[0] = rng.choice([0, 1, 2, 3, 8, 9, 14, 15, 16, 17, rng.randrange(256)])
    s[0x184] = rng.randrange(1, 101)
    s[0x183] = rng.randrange(0, s[0x184] + 1)
    s[0x185] = rng.choice([s[0x184] - s[0x183], rng.randrange(256)]) & 0xFF
    for o in range(0x18A, 0x193):
        s[o] = rng.choice([rng.randrange(1, 101), rng.randrange(256)])
    s[0x192] = rng.choice([rng.randrange(1, 11), s[0x192]])
    tile = rng.choice([struct.unpack_from("<H", s, 0xD0)[0], 0xC41, rng.randrange(3136)])
    test(s, tile)

# --- ChanceToHit 0x56630: whole data object from each dump into the emulator, every live shooter x target x aim,
# then fuzzed shooter/target bytes (stats, health, weapon, goggles/scope, anim state, motion, sight flags)
CTH, DATA_LO, DATA_HI = 0x56630, 0x90000, 0x33A730
nh = 0
for p in dumps:
    d = Dump(p)
    e.uc.mem_write(DATA_LO, bytes(d.read(DATA_LO, DATA_HI - DATA_LO)))
    rd = lambda a, k: bytes(e.uc.mem_read(a, k))
    slots = [k for k in list(range(16)) + list(range(0x28, 0x30))
             if rd(0xFA958 + k * 0x200 + 0x183, 1)[0] and struct.unpack("<H", rd(0xFA958 + k * 0x200 + 0x114, 2))[0] < 0xC40]
    for it in range(nfuzz // 10 + 1):
        a, b = rng.choice(slots), rng.choice(slots)
        sa, sb = 0xFA958 + a * 0x200, 0xFA958 + b * 0x200
        save = rd(sa, 0x200), rd(sb, 0x200)
        if it:                                    # first round untouched, then fuzz
            for base in (sa, sb):
                for o in [0, 2, 3, 5, 0x24, 0x183, 0x184, 0x185, 0x18A, 0x18B, 0x18C, 0x191, 0x192, 0x1A5, 0x1B1, 0x1B3,
                          0x1B9, 0x1D5, 0x1F2, 0x1F3] + list(range(0x1E8, 0x1F2)):
                    if rng.random() < 0.3:
                        v = {0: rng.randrange(1, 17), 3: rng.choice([0, 0x42]), 0x24: rng.choice([0, 0x27]),
                             0x1B9: rng.choice([0, 4]), 0x1F3: rng.randrange(5)}.get(o, rng.randrange(256))
                        e.w8(base + o, v)
                e.w8(base + 0x112 + a, rng.choice([0, 1])); e.w8(base + 0x13A + a, rng.choice([0, 1]))
                if rd(base + 0x183, 1)[0] > rd(base + 0x184, 1)[0]:
                    e.w8(base + 0x184, rd(base + 0x183, 1)[0])
                if rd(base + 0x184, 1)[0] == 0:
                    e.w8(base + 0x184, 1)
        tile = struct.unpack("<H", rd(sb + 0x114, 2))[0]
        for aim in range(5):
            s = rd(sa, 0x200)
            got = e.call(CTH, sa, tile, aim) & 0xFFFFFFFF
            want = combat.chance_to_hit(s, tile, aim, rd)
            nh += 1
            if got != want:
                fails += 1
                if fails <= 10:
                    print(f"MISMATCH ChanceToHit {p[-14:]} {a}->{b} aim {aim}: original {got} port {want}")
        e.uc.mem_write(sa, save[0]); e.uc.mem_write(sb, save[1])
print(f"ChanceToHit: {nh} cases")
print(f"{n} + {nh} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
