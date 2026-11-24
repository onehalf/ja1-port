"""Differential test: tools/combat.py bullet_damage / hit_location_damage vs the original BulletDamage 0x39FD8 and
HitLocationDamage 0x38D80 in Unicorn.

usage: .venv/bin/python validate/diff/damage_diff.py [DUMP ...] [n=4000] [--port]
--port: the clean-room library's ja1_bullet_damage / ja1_hit_location (docs/spec/shooting.md batch 3) instead of
tools/combat.py; the stat-loss message is its host op 183 loss_msg (written here as the original's bytes).
Random 0x53538 is stubbed on both sides with the same scripted results (the arguments of every call must match
too). Shooters/targets are dumped soldiers with fuzzed weapon, flight distance, margin, armour (helmet / vest /
Kevlar types and conditions), stats, posture, plus fuzzed hit height and damage. Compared: return value, every
call to Random, the whole target record afterwards (stat losses, armour wear) and the knockdown flag 0x92E72.
"""
import glob, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import combat                        # noqa: E402
from emu import Emu                  # noqa: E402
from memdump import Dump             # noqa: E402

BULLET_DAMAGE, HIT_LOCATION, RANDOM = 0x39FD8, 0x38D80, 0x53538
MSG = [(combat.G_LOSS_SLOT, 1), (combat.G_LOSS_TIMER, 4), (combat.G_LOSS_AMOUNT, 1), (combat.G_LOSS_PART, 0x20)]
PORT = "--port" in sys.argv
if PORT:
    sys.argv.remove("--port")
    import ai                        # noqa: E402
    import port_lib                  # noqa: E402
args = [a for a in sys.argv[1:] if not a.isdigit()]
n = int(([a for a in sys.argv[1:] if a.isdigit()] or ["4000"])[0])
dumps = args or sorted(glob.glob(os.path.join(HERE, "..", "..", "build", "mem_fight2*.bin")))
rng = random.Random(2)
e = Emu()
buf = e.alloc(0x200)
read = lambda a, k: bytes(e.uc.mem_read(a, k))
script, calls = [], []


def stub_random(emu, eax, *_):
    k = combat.s16(eax)
    calls.append(k)
    return script[len(calls) - 1] % k if k > 0 else 0


e.stub(RANDOM, stub_random)


def port_rng():
    seen = []
    def r(k):
        seen.append(k)
        return script[len(seen) - 1] % k if k > 0 else 0
    return r, seen


LOSS_TEXTS = {1: (b"head", b"wisdom"), 2: (b"arm", b"dexterity"), 3: (b"leg", b"agility")}


def port_run(name, s, r, extra=()):
    """--port: one library entry point on the world from emulator memory with record s in its slot ->
    (result, record after, knockdown, message bytes {address: byte})"""
    bufs, scalars = port_lib.world_load(e, ai)
    slot = s[0x17A]
    bufs["soldiers"][0x200 * slot:0x200 * slot + 0x200] = s
    scalars["knockdown"] = 0
    mem = {}

    def call(op, a):
        if op != "loss_msg":
            raise SystemExit(f"port called unknown host op {op} {a}")
        combat._loss_msg(bufs["soldiers"][0x200 * a[0]:0x200 * a[0] + 0x200], mem, *LOSS_TEXTS.get(a[2], (None, None)),
                         a[1] & 0xFF)
        return 0, None
    v = port_lib.ai_run(name, bufs, scalars, slot, r, call, extra if extra else 0)
    return combat.s16(v), bytes(bufs["soldiers"][0x200 * slot:0x200 * slot + 0x200]), scalars["knockdown"], mem


real = []
for p in dumps:
    d = Dump(p)
    for k in list(range(16)) + list(range(0x28, 0x30)):
        s = bytearray(d.read(0xFA958 + k * 0x200, 0x200))
        if s[0x184]:
            real.append(s)
if not real:
    real = [bytearray(0x200)]
fails = 0
cases = {"BulletDamage": 0, "HitLocation": 0}


def check(what, got, want, detail):
    global fails
    if got != want:
        fails += 1
        if fails <= 10:
            diff = [k for k in range(len(got)) if got[k] != want[k]]
            print(f"MISMATCH {what} in items {diff}: original {[got[k] for k in diff]} port {[want[k] for k in diff]} | {detail}")


for i in range(n):
    s = bytearray(rng.choice(real))
    script[:] = [rng.randrange(0x7FFF) for _ in range(8)]
    if i % 2 == 0:                                    # --- BulletDamage
        s[0x1F5] = rng.choice(list(range(0, 17)) + [rng.randrange(256)])
        struct.pack_into("<h", s, 0xC8, rng.choice([rng.randrange(0, 400), rng.randrange(-32768, 32768)]))
        s[0x1FC] = rng.choice([rng.randrange(0, 100), rng.randrange(256)])
        e.uc.mem_write(buf, bytes(s))
        calls.clear()
        got = combat.s16(e.call(BULLET_DAMAGE, buf))
        r, seen = port_rng()
        want = port_run("bullet_damage", s, r)[0] if PORT else combat.bullet_damage(s, r, read)
        cases["BulletDamage"] += 1
        check("BulletDamage", (got, calls[:]), (want, seen), f"wpn {s[0x1F5]} flown {struct.unpack_from('<h', s, 0xC8)[0]} margin {s[0x1FC]}")
    else:                                             # --- HitLocationDamage
        for slot, types in ((2, [0, 0x23, 0x24, 0x25, rng.randrange(136)]), (5, [0, 0x30, 0x31, 0x32, rng.randrange(136)]),
                            (6, [0, 0x31, 0x31, rng.randrange(136)])):   # (--port: body armour types 5+, below
            if PORT and slot != 2:                                                    # them the wear table starts earlier)
                types = [t_ if t_ == 0 or t_ >= 5 else t_ + 5 for t_ in types]
            if rng.random() < 0.7:
                s[12 * slot] = rng.choice(types)
                s[12 * slot + 2] = rng.choice([rng.randrange(0, 101), rng.randrange(256)])
        for o in (0x18B, 0x18C, 0x18D):
            s[o] = rng.choice([rng.randrange(1, 101), rng.randrange(256)])
        s[0x1F3] = rng.randrange(5)
        struct.pack_into("<H", s, 0x1E8 + 2 * s[0x1F3], rng.choice([0, 2, 4, 5, 0xD, 0x15, 0x28, 0x29, 0x38, 0x39, 0x40,
                                                                    rng.randrange(0x10000)]))
        height = rng.choice([rng.randrange(-500, 7000), rng.randrange(0, 1100), 2500, 5000, 6000, 999, 1000, 1001])
        dmg = rng.choice([0, 1, 2, rng.randrange(1, 60), rng.randrange(1, 200)])
        e.uc.mem_write(buf, bytes(s))
        e.w8(combat.G_KNOCKDOWN, 0)
        for a, k in MSG:                              # stat-loss message: sentinel bytes show every write
            e.uc.mem_write(a, b"\xee" * k)
        calls.clear()
        got = combat.s16(e.call(HIT_LOCATION, buf, height & 0xFFFFFFFF, dmg))
        got_s, got_kd = read(buf, 0x200), e.r8(combat.G_KNOCKDOWN)
        got_m = [read(a, k) for a, k in MSG]
        r, seen = port_rng()
        mem = {combat.G_KNOCKDOWN: 0}
        t = bytearray(s)
        if PORT:
            want, t_, kd_, mem = port_run("hit_location", s, r, (height, dmg))
            t[:] = t_
            mem[combat.G_KNOCKDOWN] = kd_
        else:
            want = combat.hit_location_damage(t, height, dmg, r, read, mem)
        want_m = [bytes(mem.get(a + o, 0xEE) for o in range(k)) for a, k in MSG]
        cases["HitLocation"] += 1
        check("HitLocation", (got, calls[:], got_s, got_kd, got_m), (want, seen, bytes(t), mem[combat.G_KNOCKDOWN], want_m),
              f"height {height} dmg {dmg} helmet {s[0x18]}/{s[0x1A]} e5 {s[0x3C]}/{s[0x3E]} e6 {s[0x48]}/{s[0x4A]} "
              f"anim {struct.unpack_from('<H', s, 0x1E8 + 2 * s[0x1F3])[0]:#x}")
print(f"damage_diff{' --port' if PORT else ''}: {cases['BulletDamage']} BulletDamage + {cases['HitLocation']} HitLocationDamage cases from "
      f"{len(real)} dumped soldiers, {fails} mismatches")
sys.exit(1 if fails else 0)
