"""Differential test: the A.I.M. screen's logic (strategic.md "The A.I.M. screen"), original vs port (ja1_aim_*).
usage: .venv/bin/python validate/diff/aim_diff.py [N=1200] [DUMP ...] [--seed S] [-v] [--port]
Without --port only the original runs (a smoke run of the harness). Sections (the original's procedure runs in
Unicorn for real; the port's entry point through port_lib):
- refusal: MercRefusalReason 0x219CC (AimRate 0x1FDEC, TeamHasProfile 0x50AC0 for real; PlayMercVoice 0x24D14
  stubbed and logged = host op 337 aim_remark, which also stores m in 0x22A1F0) vs ja1_aim_refusal. Compared: the
  result, the call list and the data object.
- hire: AimHire 0x22014 (merc join 0x21B74 for real, random scripted; quote, sound, face and drawing calls stubbed,
  not compared: the port's host does them) vs ja1_aim_hire. The screen mode bytes 0x95F6C / 0x95F6D are set by the
  host in the port (not compared).
- cancel: AimCancelPending 0x20060 (FireMerc 0x5097C, MercLeaves, TeamCompact? 0x50750 for real; the face, stash
  and team redraw calls stubbed and logged; the sound 0x23420 = host op 338 aim_removed; __delay_ stubbed) vs
  ja1_aim_cancel_pending.
- browse: AimBrowse 0x221B0 (the page draw 0x21648 stubbed) vs ja1_aim_browse; compared: the data object (the page
  profile byte 0x22A044).
- open: the opening block of AimScreen 0x2011C (entry to 0x20199: the face timer Random(1000)+500 and the three
  "on assignment" picks Random(80) / Random(5) over profiles 0..59, with TeamHasProfile 0x50AC0 for real) vs
  ja1_aim_open; the original run stops on an int3 planted at 0x20199, the timer is read from its stack frame
  [esp+0x6C]. Compared: the face timer, the new teamStatus bytes of the 60 profiles and the random calls.
- pay: the exit branch of AimScreen 0x20F2E..0x210FD with the question answered yes and the funds enough (count of the
  mercs on the way, arrivals word, AddFunds?, link, b19D, teamStatus, hirings), entered in the middle with the link
  pointer in the frame; ConfirmDialog, the text join, RestoreSaveArea0 and AddFunds? 0x514A4 are stubbed (the
  last one adds the amount; corpse_diff tests it) vs ja1_aim_pay. Compared: n and the data object. Own stream.
Each case fuzzes the team (in sector, profile, b19D, items), the 60 profiles' A.I.M. bytes and the accounts.
Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
from emu import Emu, EmuStop, STACK_TOP  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

DUMPS = ["tac60", "aim_hired"]
DATA_LO, DATA_HI = 0x90000, 0x33A730
SOL, PROFILES, PSIZE = ai.G_SOLDIERS, ai.G_PROFILES, ai.PROFILE_SIZE
ACC = 0x229FA0
A_ARRIVALS = 0x8C
RANDOM = 0x53538
FACES = 0x93428
MODE = (0x95F6C, 0x95F6D)
REMARK = 0x22A1F0
STASH_COUNTS = 0x93DD4
args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args and args[0].isdigit() else 1200
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
PORT = "--port" in sys.argv
if PORT:
    import port_lib                  # noqa: E402
    port_lib.PAY = True              # accounts (0xA8), profiles
COVER = __import__("collections").Counter()
# AimHire's screen calls (address: stack bytes popped), not logged
HIRE_UI = {0x24F08: 0x10, 0x23420: 0, 0x28FB8: 4, 0x28698: 0, 0x52481: 0, 0x469F0: 4, 0x67BF0: 0, 0x52C98: 12,
           0x673AC: 0, 0x286C4: 0, 0x21198: 0}


class Case:
    def __init__(self, rolls):
        self.rolls0 = rolls
        self.reset()

    def reset(self):
        self.rolls = list(self.rolls0)
        self.k = 0
        self.log = []

    def random(self, n):
        n = ai.s16(n & 0xFFFF)
        if not self.rolls:
            self.k += 1
            return self.k % n if n > 0 else 0
        r = self.rolls.pop(0)
        return r % n if n > 0 else 0


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    rt = e.rt
    case = [None]
    mode = ["ui"]

    def lg(*a):
        case[0].log.append(a)

    e.stub(rt(RANDOM), lambda emu, eax, *_: case[0].random(eax & 0xFFFF))
    e.stub(rt(0x24D14), lambda emu, eax, edx, *_: lg("voice", eax & 0xFF, edx & 0xFF), 4)
    e.stub(rt(0x46AC4), lambda emu, eax, *_: lg("face_clear", eax & 0xFF))
    e.stub(rt(0x52AE0), lambda emu, eax, edx, ebx, ecx: lg("blit", eax & 0xFFFFFFFF, edx & 0xFFFFFFFF), 8)
    e.stub(rt(0x21B3C), lambda emu, *_: lg("team_redraw"))

    def stash_put(emu, eax, edx, ebx, ecx):           # the store's put: the record logged, its type counted
        lg("stash_put", eax & 0xFF, edx & 0xFF, ebx & 0xFF)
        e.w8(rt(STASH_COUNTS + (eax & 0xFF)), e.r8(rt(STASH_COUNTS + (eax & 0xFF))) + 1)
        return 1
    e.stub(rt(0x51CB0), stash_put)
    e.stub(rt(0x7DB24), lambda emu, *_: None)
    e.stub(rt(0x21648), lambda emu, *_: None)
    for a, pop in HIRE_UI.items():
        if a == 0x23420:
            e.stub(rt(a), lambda emu, *_: lg("removed") if mode[0] == "cancel" else None)
        else:
            e.stub(rt(a), lambda emu, *_: None, pop)

    def g32(a):
        return e.r32(rt(a))

    def port_run(name, arg):
        bufs, scalars = port_lib.world_load(e, ai)
        late = []

        def call(op, a):
            a = [ai._i32(x) for x in a]
            if op == "aim_remark":
                lg("voice", a[0] & 0xFF, a[1] & 0xFF)
                late.append(a[1] & 0xFF)
            elif op == "aim_removed":
                lg("removed")
            elif op == "face_clear":
                lg("face_clear", a[0] & 0xFF)
            elif op == "face_move":
                lg("blit", g32(FACES + 4 * a[0]), g32(FACES + 4 * a[1]))
            elif op == "team_redraw":
                lg("team_redraw")
            elif op == "stash_put":
                lg("stash_put", a[0] & 0xFF, a[1] & 0xFF, a[2] & 0xFF)
                return 1, None
            else:
                raise SystemExit(f"aim_diff --port: unmapped op {op} {a}")
            return 0, None

        argv_ = list(arg) if name == "browse" else [] if name in ("cancel", "open", "pay") else [arg]
        entry = {"refusal": "aim_refusal", "hire": "aim_hire", "cancel": "aim_cancel_pending", "browse": "aim_browse",
                 "open": "aim_open", "pay": "aim_pay"}[name]
        r = port_lib.sector_run(entry, bufs, scalars, argv_, lambda n_: case[0].random(n_ & 0xFFFF), call)
        port_lib.world_store(e, ai, bufs, scalars)
        for m in late:
            e.w8(rt(REMARK), m)
        return r

    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    e.uc.mem_write(rt(0x20199), b"\xCC")       # int3 after AimScreen's opening block (LoadSanimDat follows)
    e.uc.mem_write(rt(0x210FD), b"\xCC")       # int3 after the exit branch
    pay_rng = random.Random(0x20F2E)
    fails = 0
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        name = rng.choice(["refusal", "refusal", "hire", "cancel", "browse"])
        fuzz(e, rng, name)
        if name == "browse":
            arg = (rng.choice([rng.randrange(60), 0, 59, rng.randrange(-3, 64)]), rng.choice([1, -1]))
        else:
            arg = rng.choice([rng.randrange(60), rng.randrange(60), rng.randrange(64)])
        c = Case([rng.randrange(1 << 16) for _ in range(rng.choice([3, 20, 60]))])
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        c.reset()
        case[0] = c
        mode[0] = name
        if name == "refusal":
            r_o = e.call(rt(0x219CC), arg, max_steps=2_000_000) & 0xFF
        elif name == "hire":
            e.call(rt(0x22014), arg, 10, max_steps=4_000_000)
            r_o = None
        elif name == "cancel":
            e.call(rt(0x20060), max_steps=8_000_000)
            r_o = None
        else:
            e.call(rt(0x221B0), arg[0], arg[1], max_steps=2_000_000)
            r_o = None
        log_o = c.log
        post_o = bytearray(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        COVER.update({name} | ({f"refusal {r_o}"} if name == "refusal" else set()) |
                     {f"{name} {x[0]}" for x in log_o})
        # AimScreen's opening block (0x2011C..0x20199, int3-planted stop) vs ja1_aim_open. Its own rolls keep the
        # shared rng stream (and so every later case) unchanged; the last main-section state is the start state.
        ocase = Case([((i + 1) * 7919 + (j + 1) * 104729) & 0xFFFF for j in range(16)])
        pre2 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        case[0] = ocase
        mode[0] = "open"
        try:
            e.call(rt(0x2011C), 0, max_steps=200_000)
            t_o = None
        except EmuStop as stop:
            t_o = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 0x6C) if "INT 0x03" in str(stop) else None
        post_o2 = bytearray(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        if t_o is None:
            fails += 1
            if fails <= 5:
                print(f"MISMATCH {os.path.basename(path)} open case {i}: the original did not reach 0x20199")
        elif PORT:
            COVER["open"] += 1
            e.uc.mem_write(rt(DATA_LO), pre2)
            ocase.reset()
            t_p = port_run("open", None)
            log_p = ocase.log
            post_p2 = bytearray(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
            bad = []
            if t_p != t_o:
                bad.append(f"face timer {t_o}/{t_p}")
            if log_p:
                bad.append(f"port host calls {log_p[:3]}")
            if post_o2 != post_p2:
                diffs = [j for j in range(len(post_o2)) if post_o2[j] != post_p2[j]]
                bad.append(f"memory ({len(diffs)}): " +
                           ", ".join(f"0x{DATA_LO + j:X} {post_o2[j]:02X}/{post_p2[j]:02X}" for j in diffs[:6]))
            if bad:
                fails += 1
                if fails <= 5:
                    print(f"MISMATCH {os.path.basename(path)} open case {i}: " + "; ".join(bad))
        fails += pay_case(e, pay_rng, case, port_run, data0, i, os.path.basename(path))
        if not PORT:
            continue
        e.uc.mem_write(rt(DATA_LO), pre)
        c.reset()
        case[0] = c
        r_p = port_run(name, arg)
        log_p = c.log
        post_p = bytearray(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        if name == "hire":
            for a in MODE:
                post_p[a - DATA_LO] = post_o[a - DATA_LO]
        bad = []
        if name == "refusal" and (r_p & 0xFF) != r_o:
            bad.append(f"result {r_o}/{r_p}")
        if log_o != log_p:
            k = next((j for j, (a, b) in enumerate(zip(log_o, log_p)) if a != b), min(len(log_o), len(log_p)))
            bad.append(f"call {k}: original {log_o[k] if k < len(log_o) else None}, "
                       f"port {log_p[k] if k < len(log_p) else None}")
        if post_o != post_p:
            diffs = [j for j in range(len(post_o)) if post_o[j] != post_p[j]]
            bad.append(f"memory ({len(diffs)}): " + ", ".join(f"0x{DATA_LO + j:X} {post_o[j]:02X}/{post_p[j]:02X}"
                                                             for j in diffs[:6]))
        if bad:
            fails += 1
            if fails <= 5:
                print(f"MISMATCH {os.path.basename(path)} {name} {arg} case {i}: " + "; ".join(bad))
                if VERBOSE:
                    print("  original", log_o[:20])
                    print("  port", log_p[:20])
    return fails

def pay_case(e, r, case, port_run, data0, i, tag):
    """The exit branch with the charge paid: original 0x20F2E..0x210FD vs ja1_aim_pay."""
    rt = e.rt

    def add_funds(emu, eax, edx, *_):
        case[0].log.append(("funds", eax & 0xFFFFFFFF, edx & 0xFF))
        e.w32(rt(0x229FC4), (e.r32(rt(0x229FC4)) + eax) & 0xFFFFFFFF)
    pay_stubs = ((0x36C5C, lambda emu, *_: None, 20), (0x275E4, lambda emu, *_: 1, 12), (0x514A4, add_funds, 0))
    for a, fn, pop in pay_stubs:
        e.stub(rt(a), fn, pop)
    pre3 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    e.uc.mem_write(rt(DATA_LO), data0)
    d = r.randrange(3)
    e.w8(rt(ACC + 0x12C), d)
    e.w32(rt(ACC + 0x24), r.randrange(1100, 100000))
    e.w16(rt(ACC + A_ARRIVALS), r.randrange(1 << 16))
    e.w8(rt(ACC + 0xA3), r.choice([1, 1, 0, 2]))
    e.w8(rt(ACC + 0x9D), r.choice([0, 1, 7, 0xFE, 0xFF, r.randrange(256)]))
    pending = [r.choice([0, 1, 1, 2, 0xFF]) for _ in range(8)]
    pending[r.randrange(8)] = 1
    for k in range(8):
        s = SOL + 0x200 * k
        e.w8(rt(s + 0x19D), pending[k])
        e.w8(rt(s + 0x179), r.randrange(60))
    for p in range(60):
        e.w8(rt(PROFILES + PSIZE * p + 0x4B), r.randrange(256))
    start = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    sp = STACK_TOP - 0x104
    e.w32(sp + 0x68, rt(ACC + 0xA3))
    case[0] = Case([1])
    n_o = None
    try:
        e.call(rt(0x20F2E), max_steps=200_000)
    except EmuStop as stop:
        if "INT 0x03" in str(stop):
            n_o = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 0x74)
    for a, _fn, _pop in pay_stubs:
        del e.stubs[rt(a)]
    post_o = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    bad = []
    if n_o is None:
        bad.append("the original did not reach 0x210FD")
    elif PORT:
        COVER["pay"] += 1
        e.uc.mem_write(rt(DATA_LO), start)
        n_p = port_run("pay", None)
        post_p = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        if n_p != n_o:
            bad.append(f"n {n_o}/{n_p}")
        if post_o != post_p:
            diffs = [j for j in range(len(post_o)) if post_o[j] != post_p[j]]
            bad.append(f"memory ({len(diffs)}): " + ", ".join(f"0x{DATA_LO + j:X} {post_o[j]:02X}/{post_p[j]:02X}"
                                                             for j in diffs[:6]))
    e.uc.mem_write(rt(DATA_LO), pre3)
    if bad and i < 40:
        print(f"MISMATCH {tag} pay case {i}: " + "; ".join(bad))
    return 1 if bad else 0


def fuzz(e, rng, name):
    rt = e.rt

    def s8(a, v):
        e.w8(rt(a), v & 0xFF)
    for o, vals in ((0x00, [0, 0, 1, 3, rng.randrange(256)]), (0x04, [0, 1, 2, 5, 20, 40, 255, rng.randrange(256)]),
                    (0x9A, [0, 1, 5, 20, rng.randrange(256)]), (0x9B, [0, 0, 1, 2, 4, rng.randrange(256)]),
                    (0x9C, [0, 0, 1, rng.randrange(256)]), (0x9D, [0, 0, 1, 4, 10, rng.randrange(256)]),
                    (0xA0, [0, 0, 1, rng.randrange(256)]), (0xA1, [0, 0, 1, 2, rng.randrange(256)])):
        s8(ACC + o, rng.choice(vals))
    s8(ACC + 0x05, rng.choice([0, 0, 0, 1, 0xFF]))
    # the high word of the balance sits before the day: a negative or a large balance must not change the day compare
    for o in (0x02, 0x03):
        s8(ACC + o, rng.choice([0, 0, 1, 0xFF, rng.randrange(256)]))
    pending = rng.choice([0, 0.3, 0.7, 1])
    # hire: a full team writes record 8 in the original (the screen checks the size first; the port refuses)
    size = rng.choice([0, 1, 3, 5, 7, 7, rng.randrange(8)] + ([] if name == "hire" else [8, 8, rng.randrange(9)]))
    for k in range(8):
        s = SOL + 0x200 * k
        s8(s + 0x182, 1 if k < size else 0 if k == size else rng.choice([0, 0, 0, 1, 2]))
        s8(s + 0x179, rng.choice([rng.randrange(60), rng.randrange(60), 59, 60, 70, rng.randrange(64)]))
        s8(s + 0x19D, 1 if rng.random() < pending else rng.choice([0, 0, 2]))
        s8(s + 0x17A, rng.choice([k, k, k, rng.randrange(8)]))
    for p in range(60):
        a = PROFILES + PSIZE * p
        if name == "browse":
            s8(a + 0x2E, rng.choice([0, 0, 0, 1, 50, rng.randrange(256)]))
            continue
        for o in (0x36, 0x37, 0x38, 0x39):
            s8(a + o, rng.choice([0, 1, 5, 20, 50, 100, 101, 255, rng.randrange(256)]))
        s8(a + 0x35, rng.choice([0, 0, 1, 3, 100, 101, 105, 200, rng.randrange(256)]))
        s8(a + 0x3B, rng.choice([99, 99, 99, rng.randrange(64), rng.randrange(256)]))
        s8(a + 0x3C, rng.choice([99, rng.randrange(64), rng.randrange(256)]))
        s8(a + 0x3D, rng.choice([0] * 8 + [1, rng.randrange(256)]))
        b3e = rng.choice([0] * 12 + [1, 0x100, rng.randrange(1 << 16)])
        s8(a + 0x3E, b3e)
        s8(a + 0x3F, b3e >> 8)
        for o in (0x4C, 0x4D):
            s8(a + o, rng.choice([99, 99, 99, rng.randrange(64), 60, 59, rng.randrange(256)]))
        s8(a + 0x4B, rng.choice([0, 0, 1, 50, 100, rng.randrange(256)]))
    if name == "browse":
        s8(PROFILES + PSIZE * rng.randrange(60) + 0x2E, rng.randrange(1, 256))


def main():
    rng = random.Random(SEED)
    total = fails = 0
    per = max(N // len(dumps), 1)
    for d in dumps:
        if not os.path.exists(d):
            print("missing", d)
            sys.exit(2)
        fails += run_dump(d, rng, per)
        total += per
    if VERBOSE:
        print("cover", dict(sorted(COVER.items())))
    print(f"CASES aim={total}")
    print(f"aim_diff{' --port' if PORT else ' (original only)'}: {total} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
