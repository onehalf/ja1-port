"""Differential test: tools/payday.py (the "not enough funds" dialog) vs PayDayDialog? 0x45100 in Unicorn.

usage: .venv/bin/python validate/diff/payday_diff.py [N=400] [DUMP ...] [--seed S] [-v] [--port]
--port: the clean-room library (ja1_payday_dialog, port_lib) instead of the mirror; its host ops (port_api.md, ops
221, 145, 224, 226, 233, 234, 246, 263..280) are mapped back to the original's calls below (PORT_OPS / port_call).
Each case fuzzes the team (1..8 mercs, native guides, dead mercs, salaries, items), the balance and the day accounts,
the speech-wait flag and the store counts, and scripts 1..8 mouse events (clicks on the team faces, on the funds box,
elsewhere; hover). The mouse poll 0x678E8 plays the script; the run ends when it is over or the dialog returns.
Drawing, messages, the yes / no prompt, quotes, the timer, the item store record and the face pictures are stubbed
on both sides with the same scripted results. Compared: the ordered call list, the whole data object afterwards and
how the run ended. Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import payday as pd                  # noqa: E402
from emu import Emu, EmuStop         # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
PORT = "--port" in sys.argv
if PORT:
    import port_lib                  # noqa: E402
    port_lib.INVENTORY = True        # mouse_x / mouse_y / mouse_buttons / tip_item / voice_test
    port_lib.PAY = True              # accounts (0xA8), profiles, stash_counts, stat_progress, greed

DUMPS = ["tac60"]
DATA_LO, DATA_HI = 0x90000, 0x33A730
# callee -> (address, argument kinds, stack bytes popped): s soldier, b byte, n signed int, i raw, p data pointer,
# x not compared; kinds after the 4th are stack arguments
STUBS = {"show": (0x673AC, "", 0), "hide": (0x67BF0, "", 0), "cursor2": (0x67478, "b", 0),
         "w_682ac": (0x682AC, "iiii", 0), "w_681d0": (0x681D0, "nnii", 0), "w_674b0": (0x674B0, "iiii", 0),
         "tip_item": (0x23420, "b", 0), "tip_merc": (0x23264, "niiiipppi", 20), "message": (0x27D24, "p", 0),
         "timer_start": (0x47020, "p", 0), "elapsed": (0x47040, "p", 0), "w_36010": (0x36010, "pxiii", 4),
         "str_replace": (0x252B0, "ppsp", 0), "w_467e4": (0x467E4, "iiii", 0), "yes_no": (0x28C84, "pbsii", 4),
         "w_28698": (0x28698, "i", 0), "quote": (0x24F08, "iiiipiii", 0x10), "balloon": (0x50908, "siii", 0),
         "bass_map": (0x4420C, "", 0), "w_46bf4": (0x46BF4, "", 0), "panel_elem": (0x469F0, "ixiii", 4),
         "face_clear": (0x46AC4, "b", 0), "stash_put": (0x51CB0, "bbb", 0), "blit": (0x52AE0, "ixiiii", 8),
         "w_21b3c": (0x21B3C, "", 0)}
POLL = 0x678E8

args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args and args[0].isdigit() else 400
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
SOL = ai.G_SOLDIERS
COVER = __import__("collections").Counter()


class Ended(Exception):
    pass


class Case:
    """the scripted results and mouse events of one case, replayed once per side"""
    def __init__(self, script, events):
        self.script0, self.events0 = script, events
        self.reset()

    def reset(self):
        self.script = {k: list(v) for k, v in self.script0.items()}
        self.events = list(self.events0)
        self.log = []
        self.ended = False

    def take(self, name):
        q = self.script.get(name)
        return q.pop(0) if q else 0


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    rt = e.rt
    dd = rt(0x90000) - 0x90000
    case = [None]

    def poll():
        c = case[0]
        if not c.events:
            raise Ended
        x, y, b = c.events.pop(0)
        e.w32(rt(pd.MOUSE_X), x)
        e.w32(rt(pd.MOUSE_Y), y)
        e.w8(rt(pd.BUTTONS), b)
        c.log.append(("poll", x, y, b))

    def conv(kinds, vals, orig):
        out = []
        for k, a in zip(kinds, vals):
            if k == "x":
                continue
            if k == "s":
                a = ((a & 0xFFFFFFFF) - dd if orig else a) - SOL
                out.append(("slot", a // 0x200, a % 0x200))
            elif k == "p":
                out.append((a & 0xFFFFFFFF) - dd if orig else a)
            elif k == "b":
                out.append(a & 0xFF)
            elif k == "n":
                out.append(ai._i32(a & 0xFFFFFFFF))
            else:
                out.append(a & 0xFFFFFFFF)
        return tuple(out)

    def stash_count(t):               # 0x51CB0 counts a stored record in g_stashCounts (the original's stub does it)
        e.w8(rt(ai.G_STASH_COUNTS) + t, e.r8(rt(ai.G_STASH_COUNTS) + t) + 1)

    def make_stub(name, kinds):
        def fn(emu, eax, edx, ebx, ecx):
            sp = emu.uc.reg_read(UC_X86_REG_ESP)
            stack = [emu.r32(sp + 4 + 4 * i) for i in range(max(len(kinds) - 4, 0))]
            case[0].log.append((name, conv(kinds, [eax, edx, ebx, ecx] + stack, True)))
            v = case[0].take(name)
            if name == "stash_put" and v & 0xFF:
                stash_count(eax & 0xFF)
            return v
        return fn

    def orig_poll(emu, *_):
        try:
            poll()
        except Ended:
            case[0].ended = True
            emu.uc.emu_stop()

    e.stub(rt(POLL), orig_poll)
    for name, (addr, kinds, pop) in STUBS.items():
        e.stub(rt(addr), make_stub(name, kinds), pop)

    class World:
        def g8(self, a): return e.r8(rt(a))
        def g16(self, a): return e.r16(rt(a))
        def g32(self, a): return e.r32(rt(a))
        def s8(self, a, v): e.w8(rt(a), v)
        def s16(self, a, v): e.w16(rt(a), v)
        def s32(self, a, v): e.w32(rt(a), v)
        def ptr(self, a): return a

        def ext(self, name, *a):
            if name == "poll":
                return poll()
            if name == "cash_drop":
                raise AssertionError("cash_drop is not reached here")
            case[0].log.append((name, conv(STUBS[name][1], a, False)))
            return case[0].take(name)

    w = World()

    def log(name, *vals):
        case[0].log.append((name, conv(STUBS[name][1], vals, False)))
        return case[0].take(name)

    def port_call(op, a, bufs):
        """one host op of the port -> the original's calls (and the scripted result)"""
        a = [ai._i32(x) for x in a]
        if op == "pd_message":
            log("message", (pd.MSG_FUNDS, pd.MSG_CHOOSE, pd.MSG_GUIDE)[a[0]])
        elif op in ("w_67bf0", "mouse_show", "inv_cursor_pic2", "tip_item", "inv_vest_end", "island_map",
                    "face_clear", "team_redraw"):
            name = {"w_67bf0": "hide", "mouse_show": "show", "inv_cursor_pic2": "cursor2", "inv_vest_end": "w_28698",
                    "island_map": "bass_map", "team_redraw": "w_21b3c"}.get(op, op)
            log(name, *a)
        elif op == "tip_merc":
            x0 = 0xC + 0x25 * (a[0] - 1)
            log("tip_merc", a[0], x0 + 1, 0x14, x0 + 0x1D, 0x2D, 0x99E61, 0x90A60, 0x90A60, 0)
        elif op == "stash_put":                 # the library counts the record itself
            return log("stash_put", *a)
        elif op == "face_move":
            log("blit", w.g32(0x93428 + 4 * a[0]), w.g32(0x93428 + 4 * a[1]), 0x20, 0x20, 0x20, 0x20)
        elif op == "runs_off_end":
            log("w_36010", 0x90B18, 0, 0x140, 0x2B, 0x33)
            log("hide")
            log("w_46bf4")
            log("show")
        elif op == "pd_timer_start":
            log("timer_start", 0x22A404)
        elif op == "pd_elapsed":
            return log("elapsed", 0x22A404)
        elif op == "pd_clear":
            log("w_682ac", *port_lib.mouse_op_limit(273))
        elif op == "pd_blink":
            log("w_36010", 0x90A60, 0, 0x140, 0x34 if a[0] else 0x2F, 0x35 if a[0] else 0x33)
        elif op == "pd_ask":
            s_ = SOL + 0x200 * a[0]
            log("str_replace", pd.ASK_FAMILY if a[1] else pd.ASK_MEMBER, pd.QUOTE_BUF, s_ + 0x16E, pd.MEMBER)
            log("hide")
            log("w_467e4", 0, 0x34, 0x25, 1)
            return log("yes_no", pd.QUOTE_BUF, bufs["soldiers"][0x200 * a[0] + 0x179], s_ + 0x16E, 1, 0)
        elif op == "pd_quote":
            log("quote", a[0], a[0] * 15, 0, 200, 0x2299BC, 2, a[0], 1)
        elif op == "pd_balloon":
            log("balloon", SOL + 0x200 * a[0], a[1], 0x14, 5)
        elif op == "pd_end":
            log("hide")
            log("w_681d0", *port_lib.mouse_op_place(278), 0x13F, 0xC7)
            log("w_682ac", *port_lib.mouse_op_limit(278))
            log("w_674b0", 0xC9, 0x92, 0x140, 0x2B)
            log("show")
            log("w_36010", 0x90B18, 0, 0x140, 0x2B, 0x33)
        elif op == "pd_face_frame":
            log("panel_elem", 0x67, 0, 0x140, 0xD + 0x25 * a[0], 0x14)
        elif op == "pd_panel_show":
            log("hide")
            log("w_46bf4")
            log("show")
        else:
            raise SystemExit(f"payday_diff --port: unmapped op {op} {a}")
        return 0

    def port_run():
        bufs, scalars = port_lib.world_load(e, ai)
        smap = port_lib._world_map(ai)[0]
        ended = []

        def call(name, a):
            op = port_lib.ALL_OPS.get(name, name) if isinstance(name, int) else name
            if op == "inv_poll":
                try:
                    poll()
                except Ended:
                    ended.append(1)
                    return 1, None
                for k in ("mouse_x", "mouse_y", "mouse_buttons", "key_flags"):
                    ad, n_, sg = smap[k]
                    scalars[k] = int.from_bytes(e.uc.mem_read(rt(ad), n_), "little", signed=bool(sg))
                return 0, None
            return port_call(op, list(a), bufs) & 0xFFFFFFFF, None

        def rnd(n_):
            raise SystemExit("payday_diff --port: the library asked for a random number")
        r = port_lib.sector_run("payday_dialog", bufs, scalars, [], rnd, call, sync=True)
        port_lib.world_store(e, ai, bufs, scalars)
        return "script end" if ended and r == 1 else "returned" if r == 0 and not ended else f"result {r}"

    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    fails = 0
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        fuzz(w, rng)
        c = Case(make_script(rng), make_events(rng))
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        c.reset()
        case[0] = c
        try:
            e.call(rt(0x45100), max_steps=20_000_000)
            how_o = "returned"
        except EmuStop as x:
            how_o = "script end" if c.ended else f"stop {x}"
        log_o = c.log
        post_o = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        e.uc.mem_write(rt(DATA_LO), pre)
        c.reset()
        case[0] = c
        if PORT:
            how_m = port_run()
        else:
            try:
                pd.payday(w)
                how_m = "returned"
            except Ended:
                how_m = "script end"
        log_m = c.log
        COVER.update({nm for nm, *_ in log_o} | {"end " + how_o})
        post_m = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        bad = []
        if how_o != how_m:
            bad.append(f"end: original {how_o}, mirror {how_m}")
        if log_o != log_m:
            k = next((j for j, (a, b) in enumerate(zip(log_o, log_m)) if a != b), min(len(log_o), len(log_m)))
            bad.append(f"call {k}: original {log_o[k] if k < len(log_o) else None}, "
                       f"mirror {log_m[k] if k < len(log_m) else None}")
        if post_o != post_m:
            diffs = [j for j in range(len(post_o)) if post_o[j] != post_m[j]]
            bad.append("memory: " + ", ".join(f"0x{DATA_LO + j:X} {post_o[j]:02X}/{post_m[j]:02X}" for j in diffs[:6]))
        if bad:
            fails += 1
            if fails <= 5:
                print(f"MISMATCH {os.path.basename(path)} case {i}: " + "; ".join(bad))
                if VERBOSE:
                    print("  events", c.events0)
                    print("  log", log_o[:40])
                    print("  port" if PORT else "  mirror", log_m[:40])
        elif VERBOSE and i < 3:
            print(f"case {i}: {how_o}, {len(log_o)} calls")
    return fails


def fuzz(w, rng):
    nteam = rng.choice([1, 1, 2, 3, 4, 5, 8, rng.randrange(9)])
    for k in range(8):
        s = SOL + 0x200 * k
        w.s8(s + 0x182, 1 if k < nteam else rng.choice([0, 0, 0, 1]))
        w.s8(s + 0x183, rng.choice([0, 1, 40, 99, 0x80, rng.randrange(256)]))
        w.s8(s + 0x19C, rng.choice([0, 0, 2, rng.randrange(256)]))
        w.s8(s + 0x179, rng.choice([rng.randrange(60), rng.randrange(60), rng.randrange(60, 65), 0x3C, 0x40, 0x41]))
        w.s8(s + 0x17A, k)
        w.s16(s + 0x110, rng.choice([0, 10, 50, 300, 1500, rng.randrange(0x10000)]))
        w.s16(s + 0x112, rng.choice([0, 10, 50, 300, rng.randrange(0x10000)]))
        w.s8(s + 0x17E, rng.choice([0, 0, 1, 4, rng.randrange(8)]))
        for j in range(12):
            r = s + 12 * j
            t = rng.choice([0, 0, 0, ai.CASH, rng.randrange(1, 136), w.g8(ai.G_LISTED_ITEMS + rng.randrange(32))])
            w.s8(r, t)
            w.s8(r + 1, rng.choice([0, 1, 2, 5]))
            for b in range(2, 12):
                w.s8(r + b, rng.choice([0, 1, 50, 100, rng.randrange(256)]))
            if w.g8(r + 3) >= 136:            # an attachment type past the 136 item types: the original counts it in
                w.s8(r + 3, rng.randrange(136))   # memory after the store counts (no such item in a real game)
    for t in range(136):
        w.s8(ai.G_STASH_COUNTS + t, rng.choice([0, 0, 1, 5, 99, rng.randrange(256)]))
    for a in range(0x229FA0, 0x22A048, 4):
        if rng.random() < 0.3:
            w.s32(a, rng.choice([0, 1, 500, -5000 & 0xFFFFFFFF, rng.randrange(1 << 32)]))
    w.s32(pd.BALANCE, rng.choice([-1, -100, -5000, -100000, 0, 50, rng.randrange(1 << 32)]) & 0xFFFFFFFF)
    w.s32(ai.G_PREV_BALANCE, rng.choice([-1, -100, -5000, 0, 1000, 100000]) & 0xFFFFFFFF)
    w.s8(pd.SPEECH_WAIT, rng.choice([0, 1]))
    w.s8(pd.TIP, rng.choice([0, 0, 3, 9]))


def make_script(rng):
    return {"yes_no": [rng.choice([1, 1, 0, 0x100]) for _ in range(12)],
            "elapsed": [rng.choice([0, 100, 201, 200, 5000, -1 & 0xFFFFFFFF]) for _ in range(30)],
            "stash_put": [rng.choice([1, 1, 0]) for _ in range(60)]}


def make_events(rng):
    fx, fy = 0x117, 0xA6                  # the funds box (i16 words 0x934FA / 0x934FC in the image)
    ev = []
    for _ in range(rng.choice([1, 2, 4, 6, 8])):
        k = rng.randrange(9)
        x, y = rng.choice([(0xD + 0x25 * k + rng.randrange(0x1C), 0x14 + rng.randrange(0x19)),
                           (0xC + 0x25 * k, 0x2E), (0xD + 0x25 * k + 0x1B, 0x2C), (0xC + 0x25 * k + 0x1E, 0x2D),
                           (rng.randrange(320), rng.randrange(200)), (fx + rng.randrange(0x1B), fy + rng.randrange(0x11)),
                           (100, 0x91 + rng.randrange(50))])
        ev.append((x, y, rng.choice([1, 1, 0, 2, 3])))
    return ev


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
        print(" ".join(f"{k}:{v}" for k, v in sorted(COVER.items())))
    print(f"payday_diff{' --port' if PORT else ''}: {total} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
