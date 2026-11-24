"""Differential test: the guide's advice (strategic.md "The guide's advice").

usage: .venv/bin/python validate/diff/guide_diff.py [ncases=3000] [DUMP] [--port] [--lib SO] [-v]
The original: NativeGuidePrompt? 0x5F94C runs in Unicorn on tac60 (CalcLedgerAll, SectorConnectedToHome, GuideMerc
0x62A00 and the count 0x410C4 run for real). Host ops: guide_say 0x5F5AC (302), BrendaSpeech? 0x5F76C (317), the
panel draw 0x36010 (318: ECX, stack arg), LoadQuestRecord 0x614D4 (307: 250 fuzzed bytes to 0x22A302), the hire
dialog YesNoPrompt? 0x28C84 (319: logged random answer; 0x67BF0, 0x467E4, 0x28698, 0x673AC are drawing, no-ops)
and the merc join 0x21FC0 (320). Random 0x53538 is a logged Python RNG (the port gets the same draws). EBP = 0x281E60 at
the call (the DOSBox-X value, as the port): the quote of an unlisted process capacity (spec).
Fuzzed: story bytes 2, 5..0x0E, 0x12, 0x14, 0x15, 0x1E, 0x1F, 0x23..0x2A, guide_queue, plants 1..3, mission_done, the sector
table (flags, enemies of 43/52/54, tappers/guards of 5, unhealthy), tree price / trees / process cap / sectors
owned, day, difficulty 0..2, guides_hired, guide_wait, soldiers 0..7 (inCurSector, hp, profileId), the quest record.
--port: ja1_guide_advice on the same state; the world and the host log are compared. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from common import game_read          # noqa: E402
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
N = int(args[0]) if args else 3000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
FRONT = None
if PORT:
    FRONT = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
    FRONT.ja1f_guide_nick.argtypes = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_int32, ctypes.c_void_p]
    FRONT.ja1f_guide_nick.restype = ctypes.c_int32
port_lib.PAY = True                  # accounts, profiles
port_lib.STORYDAY = True             # guide_queue
port_lib.GUIDE = True                # guides_hired, guide_wait, hard_day
VERBOSE = "-v" in argv

SOLDIERS, SECTORS, RECORDS = 0xFA958, ai.G_SECTOR_TBL9, ai.G_SECTORS
STORY, QUEUE, QUEST, ACC, PLANTS = 0x22A040, 0x22A4C9, 0x22A302, 0x229FA0, ai.G_PLANTS
CAP_QUOTES = {45: 0x31, 50: 0x32, 55: 0x30, 75: 0x34, 80: 0x33, 85: 0x35, 110: 0x36}

if not os.path.exists(DUMP):
    print(f"guide_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
ENDSAB = 0x281E60     # EBP at the call (PlayLoop 0x4FBAD: the run-time address of DAT\ENDSAB.DAT in DOSBox-X)


def _ebp(uc, addr, size, user):
    from unicorn.x86_const import UC_X86_REG_EBP
    uc.reg_write(UC_X86_REG_EBP, ENDSAB)


e.uc.hook_add(__import__("unicorn").UC_HOOK_CODE, _ebp, begin=rt(0x5F94C), end=rt(0x5F94C))
log, draws = [], []
case = {"quest": b"", "answers": []}
nicks = []
rng = random.Random(7)
edge = random.Random(8)                              # own stream: the edge cases do not move the case stream
TIP2 = [20, 50, 80]                                  # tip 2 waits while Random(100) >= this value (per difficulty)


def rnd(emu, eax, *_):
    n = eax & 0xFFFF
    n = n - 0x10000 if n & 0x8000 else n
    v = rng.randrange(n) if n > 0 else 0
    if n == 100 and edge.random() < 0.5:             # the edges of the 20 / 50 / 80 tests
        v = edge.choice([19, 20, 49, 50, 79, 80])
    if n == 100 and case.get("tip2") is not None:    # a case made for tip 2: this draw decides it
        v = TIP2[case["tip2"]] - edge.choice([0, 1])
    draws.append((n, v))
    return v


def say(emu, eax, edx, *_):
    log.append(("say", eax & 0xFFFFFFFF, edx & 0xFFFFFFFF))


def brenda(emu, eax, *_):
    log.append(("brenda", eax & 0xFFFFFFFF))


def panel(emu, eax, edx, ebx, ecx):
    log.append(("panel", ecx & 0xFFFFFFFF, emu.r32(emu.uc.reg_read(port_lib_esp()) + 4)))


def port_lib_esp():
    from unicorn.x86_const import UC_X86_REG_ESP
    return UC_X86_REG_ESP


QUESTS = game_read("DAT/QUESTS.DAT")    # the 1.13b file (port target, #24): a case with "real" uses its records


def quest_bytes(n):
    """the record op 307 gives for n: the fuzzed one, or in a "real" case QUESTS.DAT record n - 5 (zeros outside)"""
    if not case.get("real"):
        return case["quest"]
    return QUESTS[250 * (n - 5):250 * (n - 4)] if 5 <= n <= 7 else bytes(250)


def quest(emu, eax, *_):
    log.append(("quest", eax & 0xFFFFFFFF))
    emu.uc.mem_write(rt(QUEST), quest_bytes(eax & 0xFFFFFFFF))
    return 0


def offer(emu, eax, edx, ebx, *_):
    a = case["answers"].pop(0) if case["answers"] else 0
    log.append(("offer", edx & 0xFF, a))
    # The nickname of the box (EBX): compared with the port's guide_nick in main().
    nicks.append((edx & 0xFF, bytes(emu.uc.mem_read(ebx, 16)).split(b"\0")[0]))
    return a


def join(emu, eax, *_):
    log.append(("join", eax & 0xFF))


e.stub(rt(0x53538), rnd)
e.stub(rt(0x5F5AC), say)
e.stub(rt(0x5F76C), brenda)
e.stub(rt(0x36010), panel, pop=4)
e.stub(rt(0x614D4), quest)
e.stub(rt(0x28C84), offer, pop=4)
e.stub(rt(0x21FC0), join)
for a in (0x67BF0, 0x467E4, 0x28698, 0x673AC):
    e.stub(rt(a), lambda *_: 0)


def fuzz(r):
    e.uc.mem_write(rt(ai.G_MISSION_DONE), bytes(r.choice([0, 1, 1, 2]) for _ in range(80)))   # a plant at countdown 0 clears its entry
    st = bytearray(e.uc.mem_read(rt(STORY), 43))
    for i in (2, 5, 6, 7, 8, 9, 0x0B, 0x0C, 0x0D, 0x0E):
        st[i] = r.choice([0, 0, 1, 1, 2, 2, 3, r.randrange(256)])
    st[2] = r.choice([0, 0, 1, 1, 2, r.randrange(256)])
    st[0x12] = r.choice([0, 1])
    st[0x14] = r.choice([0, 1, 5, 10, 11, r.randrange(256)])
    for i in (0x15, 0x1E, 0x1F, 0x23, 0x25, 0x29, 0x2A):
        st[i] = r.choice([0, 0, 1, r.randrange(256)])
    st[0x24] = r.choice([0, 3, 3, r.randrange(256)])
    st[0x26] = r.choice([0, 1, 2, 2, r.randrange(256)])
    st[0x27] = r.choice([0, 1, 2, 3, 3, 3, 4, 5, r.randrange(256)])
    st[0x28] = r.choice([0, 1, 2, 3, 4, 0xFE, 0xFD, 0xFC, r.randrange(256)])
    e.uc.mem_write(rt(STORY), bytes(st))
    q = [0xFF] * 7
    q[0] = r.choice([0, 0, 1])
    for i in (1, 3, 5, 6):
        if r.random() < .25:
            q[i] = r.randrange(0x60)
    q[2], q[4] = r.choice([6, 6, 3]), r.choice([3, 6])
    e.uc.mem_write(rt(QUEUE), bytes(q))
    tbl = bytearray(e.uc.mem_read(rt(SECTORS), 540))
    po = r.choice([.3, .7, .95])
    for s in range(60):
        o = 9 * s
        if r.random() < .8:
            tbl[o] = (tbl[o] & 0xFE) | (1 if r.random() < po else 0)
        if r.random() < .3:
            tbl[o + 8] = r.choice([0, 0, 1])
    for s in (43, 52, 54):
        tbl[9 * s + 1] = r.choice([0, 3, 6, 7, 12])
    for o in (3, 4):
        tbl[45 + o] = r.choice([0, 0, 1, 3])
    e.uc.mem_write(rt(SECTORS), bytes(tbl))
    for i in (1, 2, 3):
        p = rt(PLANTS) + 12 * i
        e.w32(p, r.choice([0, 5, 10, 15, 20, 25, 30, 35, 45, 50, 55, 75, 80, 85, 110]))
        e.w32(p + 4, r.randrange(60))
        e.w8(p + 8, r.choice([0, 1, 1, 1, 2, r.randrange(256)]))
        e.w8(p + 0xB, r.choice([0, 1, 1, 2, 3, 0x80, r.randrange(256)]))
    acc = rt(ACC)
    e.w32(acc + 0x84, r.choice(list(CAP_QUOTES) + [0, 40, 100, 109, 200]))
    e.w32(acc + 0x1C, r.choice([0, 10, 16, 17, 20, 21, 40, 80, 105, 110, 200]))
    if edge.random() < 0.3:                          # the edges of the tapping limit: gap 5, 6, 0, -1
        e.w32(acc + 0x1C, (e.r32(acc + 0x84) - edge.choice([5, 6, 7, 0, -1])) & 0xFFFFFFFF)
    e.w32(acc + 0x18, r.choice([0, 32, 33, 40, 41, 100]))
    e.w32(acc + 0x10, r.choice([300, 320, 340, 400, 460, 480, 500, 520, r.randrange(1000)]))
    e.w32(acc + 0x74, r.choice([0, 6, 7, 8, 23, 24, 25, 40, 60]))
    e.w32(rt(ai.G_DAY), r.choice([0, 1, 2, 3, 4, 11, 12, 13, 30]))
    e.w8(rt(ai.G_DIFFICULTY), r.randrange(3))
    e.w8(rt(0x22A073), r.choice([0, 1, 2, 3, 4, 5, r.randrange(256)]))
    e.w8(rt(0x22A074), r.choice([0, 1, 2, 3, 6, r.randrange(256)]))
    for k in range(8):
        sl = rt(SOLDIERS) + 0x200 * k
        e.w8(sl + 0x182, r.choice([0, 1, 1, 1]))
        e.w8(sl + 0x183, r.choice([0, 30, 60, 0x90]))
        e.w8(sl + 0x179, r.choice([0, 5, 20, 59, 60, 61, 64, 65]) if r.random() < .5 else e.r8(sl + 0x179))
    qr = bytearray(r.randrange(256) for _ in range(250))
    for i in range(25):
        for o in (0, 6):
            qr[10 * i + o:10 * i + o + 2] = r.randrange(60).to_bytes(2, "little")
    case["quest"] = bytes(qr)
    case["real"] = r.random() < .25
    case["answers"] = [r.choice([0, 1, 1])]
    # Tip 2 reaches its Random only with no other reason to wait: about 1 case in 2000 by chance. Every 8th case
    # (own stream) clears the reasons, so each difficulty sees the threshold value and the value below it.
    case["tip2"] = None
    if edge.random() < 0.125:
        d = case["tip2"] = edge.randrange(3)
        e.w8(rt(ai.G_DIFFICULTY), d)
        e.w32(rt(ai.G_DAY), edge.choice([3, 4, 11]))
        st = bytearray(e.uc.mem_read(rt(STORY), 43))
        st[5], st[6], st[7] = 1, 2, 0
        e.uc.mem_write(rt(STORY), bytes(st))
        for sector in (28, 0):
            e.w8(rt(SECTORS) + 9 * sector, e.r8(rt(SECTORS) + 9 * sector) & 0xFE)
        e.w32(acc + 0x74, edge.choice([1, 6, 7, 8]))


def original():
    log.clear()
    nicks.clear()
    draws.clear()
    e.call(rt(0x5F94C))
    return list(log), list(draws)


def port(bufs, scalars, dr, answers):
    q, a_ = list(dr), list(answers)
    got, plog = [], []

    def random_(n):
        v = q.pop(0)[1] if q else 0
        got.append((n, v))
        return v

    def call(op, a):
        a = [x & 0xFFFFFFFF for x in a]
        if op == "guide_say":
            plog.append(("say", a[0], a[1]))
        elif op == "brenda_say":
            plog.append(("brenda", a[0]))
        elif op == "panel_draw":
            plog.append(("panel", a[0], a[1]))
        elif op == "quest_record":
            plog.append(("quest", a[0]))
            return 0, quest_bytes(a[0])
        elif op == "guide_offer":
            v = a_.pop(0) if a_ else 0
            plog.append(("offer", a[0] & 0xFF, v))
            return v, None
        elif op == "merc_join":
            plog.append(("join", a[0] & 0xFF))
        else:
            raise SystemExit(f"guide_diff --port: unmapped op {op} {a}")
        return 0, None
    port_lib.sector_run("guide_advice", bufs, scalars, [], random_, call)
    return plog, got


def main():
    r = random.Random(1)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    for i in range(N):
        e.uc.mem_write(0, snap)
        fuzz(r)
        answers = list(case["answers"])
        pre = port_lib.world_load(e, ai) if PORT else None
        st0 = bytes(e.uc.mem_read(rt(STORY), 43))
        lo, dr = original()
        st1 = bytes(e.uc.mem_read(rt(STORY), 43))
        if case["tip2"] is not None:
            for _, v in dr:
                if v in (TIP2[case["tip2"]], TIP2[case["tip2"]] - 1):
                    side = "at" if v == TIP2[case["tip2"]] else "below"
                    cover[f"tip2 d{case['tip2']} {side} {'said' if ('say', 6, 6) in lo else 'quiet'}"] += 1
                    break
        for k in lo:
            cover[k[0]] += 1
            if k[0] in ("say", "brenda"):
                cover[f"{k[0]} {k[1]:#x}"] += 1
            if k[0] == "offer":
                cover[f"offer {k[2]}"] += 1
        if st0[0x27] < 4 and st1[0x27] != st0[0x27]:
            cover[f"price {st0[0x27]}->{st1[0x27]}"] += 1
        for j in (0x0B, 0x0C):
            if st1[j] != st0[j]:
                cover[f"S{j:X} {st0[j]}->{st1[j]}"] += 1
        if e.r8(rt(0x22A10C)):
            cover["hard day"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        pre_profiles = bytes(bufs["profiles"])
        lp, got = port(bufs, scalars, dr, answers)
        for pid, nick in nicks:
            # A fuzzed profile past the table reads other memory in the original; real offers name a profile of the table.
            if (pid + 1) * 0x5A > len(pre_profiles):
                continue
            out = ctypes.create_string_buffer(16)
            prof = bytes(pre_profiles)
            n = FRONT.ja1f_guide_nick(prof, len(prof), pid, out)
            cover["offer names"] += 1
            if out.raw[:max(n, 0)] != nick:
                print(f"MISMATCH offer profile {pid}: name {nick} vs port {out.raw[:max(n, 0)]}")
                fails += 1
        bad = []
        if lo != lp:
            j = next((j for j in range(min(len(lo), len(lp))) if lo[j] != lp[j]), min(len(lo), len(lp)))
            bad.append(f"log from #{j}: {lo[j:j + 4]} vs {lp[j:j + 4]}")
        if [n for n, _ in dr] != [n for n, _ in got]:
            bad.append(f"random {dr} vs {got}")
        for b in (bufs_o, bufs):        # views of one byte in two fields (the scalar / the story byte is compared):
            b["accounts"][0xA0:0xA8] = bytes(8)     # accounts +0xA0..0xA7 = story +0..7
            b["accounts"][0xC:0x10] = bytes(4)      # accounts +0xC = team_on_duty
            b["story"][0x19] = 0                    # story +0x19 = merc_tire
        for k in bufs_o:
            if bufs_o[k] != bufs[k]:
                j = next(j for j in range(len(bufs[k])) if bufs[k][j] != bufs_o[k][j])
                bad.append(f"{k} @{j:#x} {bufs_o[k][j]:#x} vs {bufs[k][j]:#x}")
        for k in sc_o:
            if sc_o[k] != scalars[k]:
                bad.append(f"{k} {sc_o[k]} vs {scalars[k]}")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i}: " + "; ".join(bad)[:500])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    need = ["quest", "panel", "join", "offer 0", "offer 1", "hard day", "price 0->1", "price 1->2", "price 2->3",
            "price 3->4", "S B 0->1".replace(" B", "B"), "SC 0->1", "SC 0->2", "brenda 0xa", "brenda 0x6", "brenda 0x0"]
    need += [f"say {q:#x}" for q in list(CAP_QUOTES.values()) + list(range(0x28, 0x2F)) + [8, 10, 12, 0xE, 0x47, 0x49,
             0x16, 0x48, 0x37, 4, 0x4A, 0x14, 0x12, 0x1A, 2, 6, 0x10, 0x4B, 0x4C, 0x38, 0x4E, 0x43, 0x4F]]
    need += [f"tip2 d{d} {side}" for d in range(3) for side in ("at quiet", "below said")]
    missing = [k for k in need if not cover[k]]
    if missing:
        print(f"guide_diff: never reached: {missing}")
        fails += 1
    print(f"guide_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
