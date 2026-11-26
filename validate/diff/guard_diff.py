"""Differential test: the guard screen of tactical key M (strategic.md "The guard screen"), original vs port.

usage: .venv/bin/python validate/diff/guard_diff.py [ncases=1500] [DUMP] [--port] [--lib SO] [-v] [--fan IDS]   (GUARD_SEED=n)
--fan IDS (with --port; #70): the original runs the fan patch 1.15m code (Emu.put_115m), the port runs with the fan
fixes IDS on (`all` or `F05`); the run must see the 1.15m refusal (msg 18 at a send) at least once.
The original: GuardAssignScreen? 0x427B4 runs whole in Unicorn on tac60 with a frame script. The in-loop MousePoll
0x678E8 (return 0x42918) starts the next pass: mouse x / y 0x229D00 / 0x229D04, buttons 0x229CF4, g_ctrlHeld
0x284F1F; MapKeyHandler? 0x426D0 = the exit key (Enter), 0x67CD8 = the start button; the run stops when the script
ends. Real: SectorConnectedToHome 0x41178, Distance 0x41104, Side 0x30570, the sector under the mouse 0x41410,
Totals 0x42664, Undo 0x4257C, ForgetSoldier? 0x3A148, memset_. Logged stubs: StatusMessage 0x27D24 (message number
by text), the travel text (DrawText of g_textBuf), RecordReplayEvent 0x3FA04, MercDies? 0x3A4A4, the mouse limits
SetMouseLimit? 0x682AC (ja1f_map_limit_step of the front-end). Drawing stubbed.
Fuzzed: the sector table (flags, guards, b5), day_minutes, cur_sector, records 8..15 (+0x182 +0x183 +0x188), pick_no,
the script (mouse on sectors / below the map, left / right / both buttons, Ctrl, Enter, the start button).
--port: the front-end's pass (strategic.md "Each pass") over ja1_guard_open / hover / click / exit / close on the
same state; the world, the screen state (also hover / last / changed from the original's stack frame when the
script ends with the screen open) and the log are compared.
"""
import ctypes, os, random, struct, sys
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu, EmuStop         # noqa: E402
from unicorn import UC_HOOK_CODE   # noqa: E402
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_EIP   # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] not in ("--lib", "--fan"))]
FAN = None
if "--fan" in argv:
    v = argv[argv.index("--fan") + 1]
    FAN = (1 << 128) - 1 if v == "all" else sum(1 << (int(x.strip()[1:]) - 1) for x in v.split(","))
    if "--port" not in argv:
        raise SystemExit("guard_diff: --fan needs --port")
N = int(args[0]) if args else 1500
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
VERBOSE = "-v" in argv
port_lib.CLOCK = True
FRONT = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")) if PORT else None

SOLDIERS, SECTORS, CUR, MINUTES = 0xFA958, ai.G_SECTOR_TBL9, ai.G_CUR_SECTOR, 0x22A0F7
MOVES, MOVES_N, TRAVEL, PICK_N, PICK, PICK_NO, TOTALS = 0x10D680, 0x229D8C, 0x22A119, 0x22A11D, 0x22A121, 0x22A125, \
    0x22A10D
MOUSE_X, MOUSE_Y, BUTTONS, CTRL, TEXTBUF = 0x229D00, 0x229D04, 0x229CF4, 0x284F1F, 0xEBAC0
MSGS = {0x97B57: 1, 0x97ADF: 2, 0x99D77: 18, 0x9AC31: 19, 0x97A67: 20}

if not os.path.exists(DUMP):
    print(f"guard_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
if FAN is not None:
    e.put_115m()
if PORT:
    port_lib.fan_fixes(FAN or 0)
    FRONT.ja1_fan_fixes_set.argtypes = [ctypes.c_uint64]
    FRONT.ja1_fan_fixes_set((FAN or 0) & ((1 << 64) - 1))
    FRONT.ja1_fan_fixes_set_hi.argtypes = [ctypes.c_uint64]
    FRONT.ja1_fan_fixes_set_hi((FAN or 0) >> 64)
rt = e.rt
e.stub_sprintf()
st = {"ev": [], "j": 0, "log": [], "frame": None}


def poll(em, *_):
    sp = em.uc.reg_read(UC_X86_REG_ESP)
    if em.r32(sp) != rt(0x42918):                 # the poll after the loop: not a pass
        return 0
    j = st["j"]
    if j >= len(st["ev"]):
        st["frame"] = (em.r32(sp + 4 + 0x84), em.uc.reg_read(UC_X86_REG_EBP), em.r8(sp + 4 + 0x94))
        em.stop_reason = "script end"
        em.uc.emu_stop()
        return 0
    st["j"] = j + 1
    x, y, b, ctrl = st["ev"][j][:4]
    em.w32(rt(MOUSE_X), x)
    em.w32(rt(MOUSE_Y), y)
    em.w8(rt(BUTTONS), b)
    em.w8(rt(CTRL), ctrl)
    return 0


def msg(em, eax, *_):
    st["log"].append(("msg", next((v for k, v in MSGS.items() if rt(k) == eax), hex(eax))))
    return 0


def draw_text(em, eax, *_):
    if eax == rt(TEXTBUF):
        st["log"].append(("travel", em.cstr(eax).decode("latin-1")))
    return 0


def replay(em, eax, edx, ebx, *_):
    st["log"].append(("replay", eax & 0xFF, edx & 0xFF, ebx & 0xFF))
    return 0


def set_limit(em, eax, edx, ebx, ecx, *_):
    st["limits"].append((eax & 0xFFFF, edx & 0xFFFF, ebx & 0xFFFF, ecx & 0xFFFF))
    return 0


def dies(em, eax, edx, *_):
    st["log"].append(("dies", (eax - rt(SOLDIERS)) // 0x200, edx & 0xFF))
    return 0


for a, p in ((0x43ECC, 0), (0x52AE0, 8), (0x36010, 4), (0x532BC, 4), (0x42500, 4), (0x52908, 4), (0x4147C, 0),
             (0x424BC, 0), (0x681D0, 0), (0x67BF0, 0), (0x673AC, 0), (0x67C54, 0), (0x67C68, 0),
             (0x47040, 0), (0x47020, 0), (0x528AC, 0), (0x52870, 0)):
    e.stub(rt(a), lambda em, *_: 0, p)
e.stub(rt(0x358D0), draw_text, 0x18)
e.stub(rt(0x682AC), set_limit)
e.stub(rt(0x678E8), poll)
e.stub(rt(0x426D0), lambda em, *_: st["ev"][st["j"] - 1][4] if st["j"] else 0)
e.stub(rt(0x67CD8), lambda em, *_: st["ev"][st["j"] - 1][5] if st["j"] else 0)
e.stub(rt(0x27D24), msg)
e.stub(rt(0x3FA04), replay)
e.stub(rt(0x3A4A4), dies)



def undo_hook(uc, a, size, _):
    """SPEC GAP (strategic.md Undo): an undo of a sector without entries (only from a sector that holds more than
    8 guards: the fuzz makes such tables) reads before the move table: the case is not compared."""
    d = uc.reg_read(UC_X86_REG_EAX) & 0xFF
    n = e.r8(rt(MOVES_N) + d) if d < 60 else 0
    if not 1 <= n <= 8:
        st["gap"] = True
    st["undo"] += 1


e.uc.hook_add(UC_HOOK_CODE, undo_hook, None, rt(0x4257C), rt(0x4257C))
FAN_REFUSED = [0]
if FAN is not None:      # 1.15m tail 0x8AED8: the send to a sector with guards on the way is refused (fan fix F05)
    tail = 0x8AED8 + rt(0x10000) - 0x10000    # past obj1's vsize: obj1's delta (as Emu.put_115m)
    e.uc.hook_add(UC_HOOK_CODE, lambda *_: FAN_REFUSED.__setitem__(0, FAN_REFUSED[0] + 1), None, tail, tail)
e.uc.ctl_flush_tb()


def cell(s):
    """a mouse point inside sector s"""
    return (s % 10) * 31 + 3 + random_.randrange(31), (s // 10) * 23 + 3 + random_.randrange(23)


random_ = random.Random(3)


def fuzz(r):
    for s in range(60):
        a = rt(SECTORS) + 9 * s
        e.w8(a, (e.r8(a) & ~1) | (s == 59 or r.random() < .9))
        e.w8(a + 4, r.choice([0, 0, 1, 2, 3, 4, 5, 8, 9]) if r.random() < .7 else r.randrange(256))
        e.w8(a + 5, r.choice([0, 0, 0, 1, 2, 7]))
    e.w32(rt(MINUTES), r.choice([0, 30, 300, 600, 629, 659, 660, 689, 690, 700, 719, -5, r.randrange(720)]))
    cur = r.choice([r.randrange(60)] * 8 + [r.randrange(60, 100), 200])
    e.w8(rt(CUR), cur)
    for k in range(8, 16):
        a = rt(SOLDIERS) + 0x200 * k
        e.w8(a + 0x182, r.choice([0, 1, 1, 3]))
        e.w8(a + 0x183, r.choice([0, 20, 20]))
        e.w8(a + 0x188, r.choice([0, 0, 0, 1]))
    e.w32(rt(PICK_NO), r.randrange(-3, 70))
    owned = [s for s in range(60) if e.r8(rt(SECTORS) + 9 * s) & 1] or [0]
    rich = [s for s in owned if 0 < e.r8(rt(SECTORS) + 9 * s + 4)] or owned
    if cur < 60 and r.random() < .5:              # guards leave the current sector: they leave the map
        e.w8(rt(SECTORS) + 9 * cur, e.r8(rt(SECTORS) + 9 * cur) | 1)
        e.w8(rt(SECTORS) + 9 * cur + 4, r.randrange(1, 6))
        rich = [cur] * 3 + rich
    ev = []
    for _ in range(r.randrange(1, 14)):
        kind = r.random()
        ctrl = int(r.random() < .12)
        if kind < .12:                            # below the map, maybe the start button
            x, y = r.randrange(320), r.randrange(141, 200)
            ev.append((x, y, 0, ctrl, 0, int(r.random() < .4)))
            continue
        s = r.choice(rich) if kind < .45 else r.choice(owned) if kind < .9 else r.randrange(60)
        x, y = cell(s) if r.random() < .95 else (r.randrange(320), r.randrange(141))
        for _ in range(r.choice([0, 1, 1, 2])):   # stay on it (the travel time follows one pass late)
            ev.append((x, y, 0, ctrl, int(r.random() < .03), 0))
        b = r.choice([1, 2, 2, 3]) if kind < .45 else r.choice([1, 1, 2, 3])
        ev.append((x, y, b, ctrl, int(r.random() < .05), 0))
    if r.random() < .35 and len(owned) > 2:     # undo: take at A, drop at B, Ctrl + right on B
        a, b, c = r.sample(owned, 3)
        chain = r.random() < .5                   # A full again: the undo pushes out the guard sent from C to A
        if chain:
            e.w8(rt(SECTORS) + 9 * a + 4, 8 - e.r8(rt(SECTORS) + 9 * a + 5) if e.r8(rt(SECTORS) + 9 * a + 5) < 8 else 8)
            e.w8(rt(SECTORS) + 9 * c + 4, r.randrange(1, 4))
        e.w32(rt(MINUTES), r.choice([0, 100, r.randrange(400)]))
        steps = [(a, 2, 0), (b, 1, 0)] + ([(c, 2, 0), (a, 1, 0)] if chain else []) + [(b, 2, 1)]
        for t, (q, btn, ctrl) in enumerate(steps):
            x, y = cell(q)
            ev += [(x, y, 0, 0, 0, 0)] * r.choice([0, 1, 1])
            ev.append((x, y, btn, ctrl, 0, 0))
    if r.random() < .7:
        ev.append((160, 170, 0, 0, int(r.random() < .5), 1) if r.random() < .5 else (160, 100, 0, 0, 1, 0))
    st["ev"] = ev


def screen_orig():
    g = bytearray(e.uc.mem_read(rt(MOVES), 480)) + bytearray(e.uc.mem_read(rt(MOVES_N), 60))
    for a in (PICK, PICK_N, TRAVEL, PICK_NO, TOTALS, TOTALS + 4, TOTALS + 8):
        g += e.uc.mem_read(rt(a), 4)
    return g


def original():
    st.update(j=0, log=[], frame=None, gap=False, undo=0, limits=[])
    try:
        e.call(rt(0x427B4), max_steps=60_000_000)
        return "closed"
    except EmuStop as x:
        if e.stop_reason == "script end":
            return "open"
        raise SystemExit(f"guard_diff: original stopped: {x} at "
                         f"{e.uc.reg_read(UC_X86_REG_EIP) - rt(0x10000) + 0x10000:#x}")


def sector_at(x, y):
    s = int((y - 3) / 23) * 10 + int((x - 3) / 31)
    s = (s + 128) % 256 - 128
    return min(s, 59)


def cdiv(a, b):
    return abs(a) // abs(b) * (1 if (a < 0) == (b < 0) else -1)


def cmod(a, b):
    return a - b * cdiv(a, b)


def port_limits(ys):
    """The mouse limits of the front-end's rule, one step per pass from the state at the screen's start."""
    below, out, got = ctypes.c_ubyte(1), (ctypes.c_int * 4)(), []
    for y in ys:
        if FRONT.ja1f_map_limit_step(ctypes.byref(below), y, out):
            got.append(tuple(out))
    return got


def port(bufs, scalars, pick_no):
    plog = []
    g = bytearray(random_.randrange(256) for _ in range(580))
    g[552:556] = struct.pack("<i", pick_no)

    def call(op, a):
        a = [x & 0xFFFFFFFF for x in a]
        if op == "replay_event":
            plog.append(("replay", a[0] & 0xFF, a[1] & 0xFF, a[2] & 0xFF))
        elif op == "merc_dies":
            plog.append(("dies", a[0], a[1] & 0xFF))
        else:
            raise SystemExit(f"guard_diff --port: unmapped op {op} {a}")
        return 0, None
    host = port_lib.ALL_OPS & ~(1 << 43)          # connected_home: the library's own

    def run(name, *a):
        return port_lib.sector_run(name, bufs, scalars, [g] + list(a), lambda n: 0, call, host_ops=host)
    if run("guard_open"):
        return plog, g, "closed", None
    for x, y, b, ctrl, key, start in st["ev"]:
        close = False
        scalars["key_ctrl"] = ctrl                # the original reads g_ctrlHeld (a world scalar) itself
        if key:
            m = run("guard_exit")
            close = m == 0
            if m:
                plog.append(("msg", m))
        if y < 141:
            hv = run("guard_hover", sector_at(x, y))
            if hv not in (0, 1):
                plog.append(("hover result", hv))
            if hv:
                t = struct.unpack_from("<i", g, 548)[0]
                plog.append(("travel", "%d.%d" % (cdiv(t, 60), cdiv(cmod(t, 60) * 100, 600))))
            if b & 3:
                m = run("guard_click", sector_at(x, y), b, ctrl)
                if m:
                    plog.append(("msg", m))
        else:
            hv = run("guard_hover", 60)
            if hv:
                plog.append(("hover result", hv))
            if start:
                m = run("guard_exit")
                close = close or m == 0
                if m:
                    plog.append(("msg", m))
        if close:
            run("guard_close")
            return plog, g, "closed", None
    return plog, g, "open", struct.unpack_from("<iiB", g, 568)


def main():
    r = random.Random(int(os.environ.get("GUARD_SEED", 1)))
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    for i in range(N):
        e.uc.mem_write(0, snap)
        fuzz(r)
        pick_no = struct.unpack("<i", e.uc.mem_read(rt(PICK_NO), 4))[0]
        pre = port_lib.world_load(e, ai) if PORT else None
        end = original()
        lo = list(st["log"])
        cover[end] += 1
        for k in lo:
            cover[k[0] if k[0] != "msg" else f"msg {k[1]}"] += 1
        if end == "closed" and any(k[0] == "replay" for k in lo):
            cover["close with moves"] += 1
        cover["undo"] += st["undo"] > 0
        cover["undo chain"] += st["undo"] > 1
        if st["gap"]:
            cover["spec gap (not compared)"] += 1
            continue
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        g_o = screen_orig()
        bufs, scalars = pre
        lp, g, pend, pframe = port(bufs, scalars, pick_no)
        bad = []
        limits = port_limits([ev[1] for ev in st["ev"][:st["j"]]])
        cover["limit over the map"] += any(l[0] == 3 for l in st["limits"])
        cover["limit back"] += any(l[0] == 0 for l in st["limits"])
        if st["limits"] != limits:
            bad.append(f"mouse limits {st['limits']} vs {limits}")
        if end != pend:
            bad.append(f"end {end} vs {pend}")
        if lo != lp:
            j = next((j for j in range(min(len(lo), len(lp))) if lo[j] != lp[j]), min(len(lo), len(lp)))
            bad.append(f"log from #{j}: {lo[j:j + 4]} vs {lp[j:j + 4]}")
        if g_o != g[:568]:
            j = next(j for j in range(568) if g_o[j] != g[j])
            bad.append(f"screen @{j} {g_o[j]:#x} vs {g[j]:#x}")
        if end == "open" and pend == "open":
            if st["frame"] != pframe:
                bad.append(f"hover/last/changed {st['frame']} vs {pframe}")
        for k in bufs_o:
            if k in port_lib.READ_ONLY or k == "text_buf":
                continue
            if k == "enemy_watch":
                bufs_o[k] = bufs[k][:40] + bufs_o[k][40:]
            if bufs_o[k] != bufs[k]:
                j = next(j for j in range(len(bufs[k])) if bufs[k][j] != bufs_o[k][j])
                bad.append(f"{k} @{j:#x} {bufs_o[k][j]:#x} vs {bufs[k][j]:#x}")
        for k in sc_o:
            if sc_o[k] != scalars[k]:
                bad.append(f"{k} {sc_o[k]} vs {scalars[k]}")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i} (cur {e.r8(rt(CUR))}, {len(st['ev'])} passes): " + "; ".join(bad)[:500])
                if os.environ.get("GUARD_V"):
                    print("  script", st["ev"], "\n  log", lo)
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    need = ("open", "closed", "close with moves", "travel", "replay", "dies", "msg 1", "msg 2", "msg 18", "msg 19",
            "msg 20", "undo", "undo chain") + (("limit over the map", "limit back") if PORT else ())
    missing = [k for k in need if not cover[k]]
    if FAN is not None:
        print(f"guard_diff: 1.15m refusals (F05) {FAN_REFUSED[0]}")
        if not FAN_REFUSED[0]:
            missing.append("1.15m refusal")
    if missing:
        print(f"guard_diff: never reached: {missing}")
        fails += 1
    print(f"guard_diff{' (port)' if PORT else ''}{f' --fan {FAN:#x}' if FAN is not None else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
