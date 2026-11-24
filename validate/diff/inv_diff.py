"""Differential test: tools/invscreen.py (inventory screen loop) vs UseInventoryItem? 0x53A9C in Unicorn.

usage: .venv/bin/python validate/diff/inv_diff.py [N=400] [DUMP ...] [--seed S] [-v] [--port] [--strip]
--strip (with --port): panel.md "Clicks on the bottom strip": the original's strip click 0x54F3C vs ja1_strip_click
(mouse in the strip, left / right / no button, ctrl, cursor mode, the under-panel flag); waits, the inventory
screen runner 0x54D50, the mouse move, WeaponCursor and AnimStep are stubbed and logged on both sides.
--port: the clean-room library (ja1_inventory_screen, port_lib) instead of the mirror; its host ops are mapped to
the same stubs and argument kinds, the world goes back to memory and the same data area is compared.
Each case fuzzes one merc's items, the cursor item, AP, hp, the vest owner, the ground item of the merc's tile, and
scripts 1..6 mouse actions (left / right clicks on the item rects, the merc slots, the mouth / body / face rects, below
the panel; shift / ctrl; keys Esc and T). The mouse poll 0x678E8 plays the script; the run ends when it is over or the
loop leaves. UI drawing, sounds, prompts (merge question, disarm), Random, Spend AP, Rest, PutItem, PickUpItem, SelectSoldier ... are
stubbed on both sides with the same scripted results. Compared: the ordered call list (callee + arguments), the whole
data object afterwards (soldiers, cursor item, ground items, flags) and how the run ended. Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import invscreen as inv               # noqa: E402
import port_lib                      # noqa: E402
port_lib.INVENTORY = True
from emu import Emu, EmuStop         # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

DUMPS = ["tac60", "fight1", "combat1", "death1", "f5"]
DATA_LO, DATA_HI = 0x90000, 0x33A730
# callee -> (address, argument kinds, stack bytes popped): s soldier, b byte, n signed int, i raw, t tile, m string,
# p data pointer, x not compared; kinds after the 4th are stack arguments
STUBS = {"show": (0x673AC, "", 0), "hide": (0x67BF0, "", 0), "cursor": (0x674B0, "b", 0), "cursor2": (0x67478, "b", 0),
         "click": (0x5C880, "", 0), "w_4b730": (0x4B730, "b", 0), "w_4b764": (0x4B764, "", 0),
         "item_value": (0x25B84, "p", 0), "item_info": (0x25D20, "bbnnnnn", 12), "body": (0x555DC, "s", 0),
         "tip_item": (0x23420, "b", 0), "tip_merc": (0x23264, "n", 20), "w_682ac": (0x682AC, "iiii", 0),
         "text_toggle": (0x3F994, "pp", 0), "message": (0x27D24, "m", 0), "inv_extra": (0x25AD8, "mm", 0),
         "draw_slot": (0x4AAD4, "snxi", 0), "vestmap": (0x48000, "s", 0), "merc_text": (0x15910, "smi", 0),
         "sfx": (0x5CB34, "iiii", 0), "slot_redraw": (0x331A4, "bi", 0), "delay": (0x67250, "i", 0),
         "w_467e4": (0x467E4, "iii", 0), "w_28698": (0x28698, "i", 0),
         "spend_ap": (0x329BC, "sbi", 0), "rest": (0x32754, "snb", 0), "reserve_ap": (0x17C88, "s", 0),
         "select": (0x17D80, "bbb", 0), "w_2b6a0": (0x2B6A0, "s", 0), "merge_prompt": (0x278A0, "mmbiiibiiiii", 0x20),
         "w_55fd8": (0x55FD8, "sii", 0), "stat_practice": (0x62A68, "sbni", 0), "mine_spotted": (0x3C31C, "stb", 0),
         "disarm": (0x3B9D8, "stbii", 4), "pick_up": (0x3C16C, "stb", 0), "focus": (0x3F724, "bi", 0),
         "put_item": (0x3FC78, "txxxiiiiiii", 0x1C),
         "random": (0x53538, "n", 0), "quote": (0x24F08, "iiiipiii", 0x10), "yes_no": (0x28C84, "pbsii", 4)}
POLL, KEY_PRESSED, GET_KEY = 0x678E8, 0x5616C, 0x560F4
STRIP = "--strip" in sys.argv
if STRIP:
    STUBS.update({"anim_step": (0x1F2C4, "s", 0), "weapon_cursor": (0x1C39C, "s", 0), "mouse_to": (0x681D0, "iib", 0),
                  "wait_up": (0x67C54, "", 0), "wait_rup": (0x67C68, "", 0), "inv_open": (0x54D50, "sb", 0)})
STRIP_EDGES = [0, 49, 94, 139, 184, 229, 272]

args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args and args[0].isdigit() else 400
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
PORT = "--port" in sys.argv
SOL = 0xFA958
MSG = {7: 0x97887, 8: 0x98F7F, 9: 0x98FF7, 10: 0x9AE05}          # op 125 text number -> the original's string
SOUNDS = (None, 0x91020, 0x91018, 0x9102C, 0x9100C)              # op 237 sound / text numbers (port_api.md)
TEXTS = (0x91028, 0x99EC9, 0x9AC17, 0x99BBD, 0x99EE3)
MERC_TEXT = {1: "doesn't have enough Action Points!", 2: 0x9A083}
# port op name -> (stub name, the op's args -> the mirror's arguments of that stub)
sl = lambda a: SOL + 0x200 * a                                  # noqa: E731
ent = lambda a, k: inv.CURSOR if a == -1 else sl(a) + 12 * k    # noqa: E731
PORT_OPS = {"mouse_show": ("show", lambda a: a), "w_67bf0": ("hide", lambda a: a),
            "inv_cursor_pic": ("cursor", lambda a: a), "inv_cursor_pic2": ("cursor2", lambda a: a),
            "inv_click": ("click", lambda a: a), "inv_begin": ("w_4b730", lambda a: a), "inv_end": ("w_4b764", lambda a: a),
            "item_value": ("item_value", lambda a: [ent(a[0], a[1])]), "item_info": ("item_info", lambda a: a),
            "inv_body": ("body", lambda a: [sl(a[0])]), "tip_item": ("tip_item", lambda a: a),
            "tip_merc": ("tip_merc", lambda a: a), "drop_answer": ("w_682ac", lambda a: a),
            "text_toggle": ("text_toggle", lambda a: [0x9ADD1, inv.TEXT_ON]),
            "message": ("message", lambda a: [MSG.get(a[0], f"text {a[0]}")]),
            "inv_extra": ("inv_extra", lambda a: [SOUNDS[a[0]] if a[0] else "", TEXTS[a[1]]]),
            "draw_slot": ("draw_slot", lambda a: [sl(a[0]), a[1], 0, 0x140]), "vestmap": ("vestmap", lambda a: [sl(a[0])]),
            "merc_text": ("merc_text", lambda a: [sl(a[0]), MERC_TEXT.get(a[1], f"text {a[1]}"), a[2]]),
            "w_331a4": ("slot_redraw", lambda a: a), "no_room": ("no_room", lambda a: [ent(a[0], a[1]), inv.CURSOR]),
            "spend_ap": ("spend_ap", lambda a: [sl(a[0])] + a[1:]), "rest": ("rest", lambda a: [sl(a[0])] + a[1:]),
            "reserve_ap": ("reserve_ap", lambda a: [sl(a[0])]), "inv_select": ("select", lambda a: a),
            "inv_leave_merc": ("w_2b6a0", lambda a: [sl(a[0])]), "inv_moved": ("w_55fd8", lambda a: [sl(a[0])] + a[1:]),
            "inv_vest_begin": ("w_467e4", lambda a: a), "inv_vest_end": ("w_28698", lambda a: a),
            "mine_spotted": ("mine_spotted", lambda a: [sl(a[0])] + a[1:]), "disarm": ("disarm", lambda a: [sl(a[0])] + a[1:]),
            "pick_up": ("pick_up", lambda a: [sl(a[0])] + a[1:]), "focus": ("focus", lambda a: a),
            "inv_put_item": ("put_item", lambda a: a[:1] + [0, 0, 0] + a[1:]),
            "merge_prompt": ("merge_prompt", lambda a: [inv.NOTE_MERGE, inv.PCC_MERGE, a[0], 10, 0x14, 0x10, a[1],
                                                        0x14, 0x32, 1, 1, 0]),
            "merge_quote": ("quote", lambda a: [a[0], 33 * a[0], 6, 200, inv.QUOTE_BUF, 1, a[0], 1]),
            "stat_practice": ("stat_practice", lambda a: [sl(a[0])] + a[1:]), "delay": ("delay", lambda a: a),
            "sfx": ("sfx", lambda a: a)}


COVER = __import__("collections").Counter()


class Ended(Exception):
    pass


class Case:
    """the scripted results and mouse events of one case, replayed once per side"""
    def __init__(self, script, events):
        self.script0, self.events0 = script, events

    def reset(self):
        self.script = {k: list(v) for k, v in self.script0.items()}
        self.events = list(self.events0)
        self.key = 0
        self.log = []

    def take(self, name, a0=0):
        q = self.script.get(name)
        v = q.pop(0) if q else 0
        return v % max(ai._i32(a0 & 0xFFFFFFFF), 1) if name == "random" else v


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    rt = e.rt
    dd = rt(0x90000) - 0x90000
    sol0 = rt(ai.G_SOLDIERS) if hasattr(ai, "G_SOLDIERS") else rt(0xFA958)
    case = [None]

    def poll(*_):
        c = case[0]
        e.w32(rt(0x229CF0), e.r32(rt(0x229D00)))
        e.w32(rt(0x229CEC), e.r32(rt(0x229D04)))
        if not c.events:
            raise Ended
        x, y, b, kb, key = c.events.pop(0)
        e.w32(rt(0x229D00), x)
        e.w32(rt(0x229D04), y)
        e.w8(rt(0x229CF4), b)
        e.w16(rt(0x22A46C), kb)
        c.key = key
        c.log.append(("poll", x, y, b))

    def key_pressed(*_):
        return 1 if case[0].key else 0

    def get_key(*_):
        k, case[0].key = case[0].key, 0
        return k

    def conv(kinds, vals, orig):
        out = []
        for k, a in zip(kinds, vals):
            if k == "x":
                continue
            if k == "s":
                if isinstance(a, inv.Rec):
                    a = a.addr
                elif orig:
                    a = (a & 0xFFFFFFFF) - sol0 + 0xFA958
                out.append(("slot", (a - 0xFA958) // 0x200, (a - 0xFA958) % 0x200))
            elif k == "m":
                out.append(a if isinstance(a, str) else e.cstr(a if orig else rt(a)).decode("latin-1"))
            elif k == "p":
                out.append((a & 0xFFFFFFFF) - dd if orig else a)
            elif k == "t":
                out.append(ai.s16(a))
            elif k == "b":
                out.append(a & 0xFF)
            elif k == "n":
                out.append(ai._i32(a & 0xFFFFFFFF))
            else:
                out.append(a & 0xFFFFFFFF)
        return tuple(out)

    def make_stub(name, kinds):
        def fn(emu, eax, edx, ebx, ecx):
            sp = emu.uc.reg_read(UC_X86_REG_ESP)
            stack = [emu.r32(sp + 4 + 4 * i) for i in range(max(len(kinds) - 4, 0))]
            case[0].log.append((name, conv(kinds, [eax, edx, ebx, ecx] + stack, True)))
            return case[0].take(name, eax)
        return fn

    def orig_poll(emu, *_):
        try:
            poll()
        except Ended:
            case[0].ended = True
            emu.uc.emu_stop()

    e.stub(rt(POLL), orig_poll)
    e.stub(rt(KEY_PRESSED), lambda *_: key_pressed())
    e.stub(rt(GET_KEY), lambda *_: get_key())
    for name, (addr, kinds, pop) in STUBS.items():
        e.stub(rt(addr), make_stub(name, kinds), pop)

    class World:
        def g8(self, a): return e.r8(rt(a))
        def g16(self, a): return e.r16(rt(a))
        def g32(self, a): return e.r32(rt(a))
        def s8(self, a, v): e.w8(rt(a), v)
        def s16(self, a, v): e.w16(rt(a), v)
        def s32(self, a, v): e.w32(rt(a), v)
        def tile8(self, t, k): return e.r8(rt(0x100A18) + 8 * ai.s16(t) + k)
        def ptr_image(self, p): return (p & 0xFFFFFFFF) - sol0 + 0xFA958

        def ext(self, name, *a):
            if name == "poll":
                return poll()
            if name == "key_pressed":
                return key_pressed()
            if name == "get_key":
                return get_key()
            kinds = STUBS[name][1]
            case[0].log.append((name, conv(kinds, a, False)))
            return case[0].take(name, a[0] if a else 0)

    w = World()

    def port_run(c, slot, redraw, entry="inventory_screen"):
        bufs, scalars = port_lib.world_load(e, ai)
        smap = port_lib._world_map(ai)[0]
        ended = []

        def call(name, a):
            if name == "inv_poll":
                try:
                    poll()
                except Ended:
                    ended.append(1)
                    return 1, None
                for k in ("mouse_x", "mouse_y", "mouse_buttons", "key_flags"):
                    ad, n, sg = smap[k]
                    scalars[k] = int.from_bytes(e.uc.mem_read(rt(ad), n), "little", signed=bool(sg))
                return 0, None
            if name == "key_pressed":
                return key_pressed(), None
            if name == "get_key":
                return get_key(), None
            if name == "wait_up":                   # 0x67C54 left / 0x67C68 right
                stub, vals = ("wait_up" if a[0] == 1 else "wait_rup"), []
            elif name in ("anim_step", "weapon_cursor"):
                stub, vals = name, [sl(a[0])]
            elif name == "inv_open":
                stub, vals = name, [sl(a[0]), 0]
            elif name == "mouse_to":
                stub, vals = name, list(a)
            elif name == "merge_yes_no":
                p = bufs["soldiers"][0x200 * a[0] + 0x179]
                stub, vals = "yes_no", [inv.QUOTE_BUF, p, sl(a[0]) + 0x16E, 0, 0]
            elif name in PORT_OPS:
                stub, f = PORT_OPS[name]
                vals = f(list(a))
            else:
                raise SystemExit(f"inv_diff --port: unmapped op {name} {list(a)}")
            c.log.append((stub, conv(STUBS[stub][1], vals, False)))
            return c.take(stub, vals[0] if vals else 0) & 0xFFFFFFFF, None

        def rnd(n):
            c.log.append(("random", conv("n", [n], False)))
            return c.take("random", n)
        r = port_lib.sector_run(entry, bufs, scalars, [slot, redraw], rnd, call, sync=True)
        port_lib.world_store(e, ai, bufs, scalars)
        if entry == "strip_click":
            return "script end" if ended else "returned"
        return "script end" if ended and r == 1 else "returned" if r == 0 and not ended else f"result {r}"

    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    item_pos = [(ai.s16(w.g16(inv.ITEM_POS + 4 * k)), ai.s16(w.g16(inv.ITEM_POS + 4 * k + 2))) for k in range(14)]
    slot_pos = [(ai.s16(w.g16(inv.SLOT_POS + 4 * k)), ai.s16(w.g16(inv.SLOT_POS + 4 * k + 2))) for k in range(8)]
    stackable = [t for t in range(1, 0x88) if w.g8(inv.ITEM_TBL + 12 * t + 6) > 1]
    ammo = sorted({w.g8(inv.AMMO_OF + 6 * t) for t in range(1, 15)} - {0})
    fails = 0
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        slot = rng.choice([w.g8(ai.G_SELECTED) & 7] * 3 + list(range(8)))
        pair = fuzz(w, rng, slot, stackable, ammo)
        w.s8(ai.G_SECTORS + 0xB1 * w.g8(ai.G_CUR_SECTOR) + 0xAF, rng.choice([1, 2, 7]))   # the sector's mine count: a picked-up mine counts it down
        if PORT:                                  # counts so big that Take one runs on into soldier +0x90
            for k in range(8):                    # (a pointer in the original, a script offset in the port)
                for j in range(12):
                    a = inv.G_SOLDIERS + 0x200 * k + 12 * j + 1
                    if 12 * j + 6 + ai.s8(w.g8(a)) >= 0x8E:
                        w.s8(a, 5)
        if STRIP:
            s_ = inv.rec(w, slot)
            s_[0x138] = rng.choice([1, 1, 1, 0])
            s_[0x177] = rng.choice([0, 1, 1])
            if rng.random() < .3:                     # the hand: an item or none (right click on box 0)
                inv.clear(w, s_.addr, 12) if rng.random() < .5 else w.s8(s_.addr, rng.choice([0x4D, 1, 0x20, 0x11]))
            for k_ in rng.sample(range(12), rng.randrange(4)):    # free places for the right click on box 0
                inv.clear(w, s_.addr + 12 * k_, 12)
            x_ = rng.choice([rng.randrange(320)] * 3 + [v_ + d_ for v_ in STRIP_EDGES for d_ in (-2, -1, 0)])
            w.s32(0x229D00, x_)
            w.s32(0x229D04, rng.choice([182, 190, 198]))
            w.s8(0x229CF4, rng.choice([1, 2, 1, 2, 0, 3]))
            w.s8(0x284F1F, rng.choice([0, 0, 1]))
            mode = rng.choice([0, 2, 2, 1, 9])
            w.s8(0x284F1B, mode)
            w.s8(0x955C9, rng.choice([0, 1]))
            script = make_script(rng)
            script["mouse_to"] = [rng.randrange(5)]
            c = Case(script, [])
        else:
            c = Case(make_script(rng), make_events(rng, item_pos, slot_pos, pair))
        g2 = random.Random(f"give{i}")            # own generator: the cases of the main stream stay as they were
        if not STRIP and g2.random() < .15:
            # An item goes to another merc and comes back (GaveItem? 0x53958 with the merc equal to give slot 0 or
            # 1): kind 0 = take at A, face B, face A, put at A; kind 1 = take at A, face B, swap at B, face A, put at A.
            kind, b_ = g2.randrange(2), g2.choice([k for k in range(8) if k != slot])
            pa, pb = g2.randrange(7, 12), g2.randrange(7, 12)
            inv.clear(w, inv.CURSOR, 12)
            ta, tb = g2.sample([0x41, 0x4F, 0x47], 2)       # two types that do not join
            for who, pl, t in ((slot, pa, ta), (b_, pb, tb)):
                a_ = inv.rec(w, who).addr + 12 * pl
                inv.clear(w, a_, 12)
                for j, v in enumerate((t, 1, 100)):
                    w.s8(a_ + j, v)
            press = lambda xy: [(xy[0] + 5, xy[1] + 5, 1, 0, 0), (xy[0] + 5, xy[1] + 5, 0, 0, 0)]   # noqa: E731
            ev = press(item_pos[pa]) + press(slot_pos[b_]) + (press(item_pos[pb]) if kind else [])
            ev += press(slot_pos[slot]) + press(item_pos[pa])
            script = make_script(g2)
            script["select"] = [1] * 6
            c = Case(script, ev)
        if PORT and not STRIP:
            c.events0 = port_safe_events(c.events0, item_pos)
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        redraw = rng.choice([0, 0, 1])
        # original
        c.reset(); c.ended = False; case[0] = c
        sp = e.r32(rt(0x100958) + 4 * slot)
        try:
            if STRIP:
                e.call(rt(0x54F3C), sp, max_steps=20_000_000)
            else:
                e.call(rt(0x53A9C), sp, redraw, max_steps=20_000_000)
            how_o = "returned"
        except EmuStop as x:
            how_o = "script end" if c.ended else f"stop {x}"
        log_o = c.log
        post_o = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        e.uc.mem_write(rt(DATA_LO), pre)
        c.reset(); case[0] = c
        if STRIP:
            how_m = port_run(c, slot, mode, "strip_click")
        elif PORT:
            how_m = port_run(c, slot, redraw)
        else:                                     # mirror
            try:
                inv.inventory_screen(w, inv.Rec(w, w.ptr_image(sp)), redraw)
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
            bad.append(f"call {k}: original {log_o[k] if k < len(log_o) else None}, mirror {log_m[k] if k < len(log_m) else None}")
        if post_o != post_m:
            diffs = [j for j in range(len(post_o)) if post_o[j] != post_m[j]]
            bad.append("memory: " + ", ".join(f"0x{DATA_LO + j:X} {post_o[j]:02X}/{post_m[j]:02X}" for j in diffs[:6]))
        if bad:
            fails += 1
            if fails <= 5:
                print(f"MISMATCH {os.path.basename(path)} case {i} slot {slot}: " + "; ".join(bad))
                if VERBOSE:
                    print("  events", c.events0)
                    print("  log", log_o[:40])
                    print("  port" if PORT else "  mirror", log_m[:40])
        elif VERBOSE and i < 3:
            print(f"case {i}: {how_o}, {len(log_o)} calls")
    return fails


def rand_item(w, rng, stackable, ammo):
    t = rng.choice([0, 0, rng.randrange(1, 15), rng.choice(ammo), rng.choice(stackable), 0x41, 0x4F, 0x47,
                    rng.randrange(0x32, 0x36), 0x27, 0x2B, 0x6F, rng.randrange(1, 0x88),
                    rng.choice([0x28, 0x2A, 0x2D, 0x30, 0x31, 0x23, 0x26])])
    m = max(w.g8(inv.ITEM_TBL + 12 * t + 6), 1)
    q = rng.choice([0, 1, 1, rng.randrange(1, m + 1), rng.randrange(256), 99, 2]) if t else 0
    b = [t, q, rng.choice([100, rng.randrange(101), rng.randrange(256), 20, 21, 1, 2, 49, 50, 51])] + [rng.randrange(256) if rng.random() < .3
                                                                             else 0 for _ in range(9)]
    if t and t <= 14 and rng.random() < .5:
        b[1] = 0
    return b


def fuzz(w, rng, slot, stackable, ammo):
    """returns True when the cursor holds a merge partner of the hand item (the script then starts on rect 12)"""
    s = inv.rec(w, slot)
    pair = False
    for k in range(12):
        if rng.random() < .6:
            for j, v in enumerate(rand_item(w, rng, stackable, ammo)):
                w.s8(s.addr + 12 * k + j, v)
    if rng.random() < .6:
        for j, v in enumerate(rand_item(w, rng, stackable, ammo)):
            w.s8(inv.CURSOR + j, v)
    else:
        inv.clear(w, inv.CURSOR, 12)
    if rng.random() < .3:
        gun = rng.randrange(1, 15)
        w.s8(s.addr + 12 * rng.choice([0, 1, 3]), gun)
        w.s8(s.addr + 12 * rng.choice([0, 1, 3]) + 1, 0)
        w.s8(inv.CURSOR, w.g8(inv.AMMO_OF + 6 * gun))
        w.s8(inv.CURSOR + 1, 1)
    if rng.random() < .3:                             # same type on the cursor and in a pocket (stack / no room)
        k = rng.randrange(12)
        st = rng.choice(stackable)
        w.s8(s.addr + 12 * k, st)
        w.s8(s.addr + 12 * k + 1, rng.randrange(0, w.g8(inv.ITEM_TBL + 12 * st + 6) + 2))
        w.s8(inv.CURSOR, st)
        w.s8(inv.CURSOR + 1, rng.randrange(0, 4))
    if rng.random() < .3:                             # vests: entry 6 and the cursor
        w.s8(s.addr + 72, rng.randrange(0x30, 0x37))
        if rng.random() < .5:
            w.s8(inv.CURSOR, rng.randrange(0x31, 0x37))
            o = rng.randrange(8)
            w.s8(inv.VEST_OWNER, o)
            if rng.random() < .5:                     # the owner's vest pocket: a type with count 0, or empty
                k = inv.G_SOLDIERS + 0x200 * o + 12 * rng.randrange(7, 12)
                w.s8(k, rng.choice([0, rng.randrange(1, 15)]))
                w.s8(k + 1, 0)
    if rng.random() < .3:                             # a merge table pair: cursor on the hand item (entry 0)
        e = inv.MERGE_TBL + 4 * rng.choice([rng.randrange(82), rng.randrange(20, 26), rng.randrange(26, 30),
                                            rng.randrange(77, 82), rng.randrange(30, 77)])
        w.s8(inv.CURSOR, w.g8(e))
        w.s8(s.addr, w.g8(e + 1))
        for b in (inv.CURSOR, s.addr):
            w.s8(b + 1, rng.choice([1, 1, 0, 2, 99]))
            w.s8(b + 2, rng.choice([100, rng.randrange(1, 101), rng.randrange(256), 1, 60]))
            w.s8(b + 3, rng.choice([0, 0, 0, w.g8(e + 1)]))
        for k in (0x18C, 0x18F, 0x190):
            s[k] = rng.choice([s[k], rng.randrange(101), 0, 100, 0x90])
        if rng.random() < .5:                         # mechanical 50, dexterity 100: the merge roll edge is 50
            s[0x190], s[0x18C] = 50, 100
        w.s8(0x92F8B, 0)
        pair = True
    s[0x1D5] = rng.choice([s[0x1D5], 0, 1, 0x80])
    for k in range(8):
        o = inv.rec(w, k)
        if rng.random() < .3:
            o[0x183] = rng.choice([0, 10, 14, 15, 50, 100])
        if rng.random() < .2:
            o[0x1BB] = rng.choice([0, 1])
        if rng.random() < .5:
            o[0x1A5] = rng.choice([0, 3, 4, 5, 6, 9, 10, 20])
            o[0x1E2] = rng.choice([0, 0, 0, 1, 2])
    if rng.random() < .15:                            # the ground rect with an empty cursor (pick-up AP)
        inv.clear(w, inv.CURSOR, 12)
    # a stack on the cursor holds 1..5 units in play (conditions +2, +8..+11); PlayNoRoom? 0x53A34 reads the unit at
    # cursor + 6 + count, past the 12 bytes for a larger count (inventory.md "No room")
    if w.g8(inv.ITEM_TBL + 12 * w.g8(inv.CURSOR) + 6) > 1 and not -6 <= ai.s8(w.g8(inv.CURSOR + 1)) <= 5:
        w.s8(inv.CURSOR + 1, 5)
    s[0x192] = rng.choice([s[0x192], 1, 3, 4, 7])
    s[0x1D5] = rng.choice([s[0x1D5], 0, 1, 0x80])
    s[0x18A] = rng.randrange(0, 101)
    w.s8(ai.G_SELECTED, rng.choice([slot, slot, slot, rng.randrange(8), 0x40]))
    w.s8(ai.G_COMBAT, rng.choice([0, 1, 1]))
    if rng.random() < .7:
        w.s8(inv.VEST_OWNER, rng.choice([0x40, 0x40, slot, rng.randrange(8)]))
    w.s8(inv.TEXT_ON, rng.choice([0, 1]))
    w.s32(0x229C48, rng.choice([0, 0, 14]))
    t = inv.tile(s)
    if 0 <= t < 3136:
        g = rng.choice([0xFF, rng.randrange(200), rng.randrange(200), 0xD0])
        w.s8(0x100A18 + 8 * t + 6, g)
        if g < 200 and rng.random() < .3:
            w.s8(inv.G_GROUND + 12 * g + 6, 0xFF)
            w.s8(inv.G_GROUND + 12 * g + 2, rng.choice([0x80, 0xF0, 50]))
    if rng.random() < .08:                            # 0x538FC: a smaller vest on place 6, pockets drop to the ground
        w.s8(s.addr + 72, rng.choice([0x33, 0x34, 0x35]))
        v = w.g8(s.addr + 72)                         # a smaller vest, or the same type (no drop; box 1 #45 c13004)
        w.s8(inv.CURSOR, rng.choice([0x32, 0x32, 0x33, v]) if v > 0x33 else rng.choice([0x32, 0x32, v]))
        w.s8(inv.CURSOR + 1, 1)
        w.s8(inv.VEST_OWNER, rng.choice([0x40, slot]))
        for q in range(7, 12):
            w.s8(s.addr + 12 * q, rng.choice([0, 0x27, 0x41, 0x47]))
            w.s8(s.addr + 12 * q + 1, 1)
            w.s8(s.addr + 12 * q + 2, 100)
        return ("press", 6)
    if rng.random() < .05:                            # MergeItemsPrompt 0x4A838: the edge types of the slot rules
        t = rng.choice([0x22, 0x23, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x30, 0x31])
        q = rng.choice([2, 3, 4, 5])
        w.s8(s.addr + 12 * q, 0)
        w.s8(s.addr + 12 * q + 1, 0)
        w.s8(inv.CURSOR, t)
        w.s8(inv.CURSOR + 1, 1)
        w.s8(inv.CURSOR + 2, 100)
        return ("press", q)
    if rng.random() < .05:                            # PlayNoRoom? 0x53A34: a stack on the cursor, too much for the pocket
        t = rng.choice(stackable)
        m = w.g8(inv.ITEM_TBL + 12 * t + 6)
        if m > 1:
            q = rng.choice([0, 1, 7, 8, 9, 10, 11])
            w.s8(s.addr + 72, 0x35)                   # no pocket is missing
            w.s8(s.addr + 12 * q, t)
            w.s8(s.addr + 12 * q + 1, rng.randrange(1, m + 1))
            w.s8(inv.CURSOR, t)
            w.s8(inv.CURSOR + 1, min(5, rng.randrange(1, m + 1)))
            return ("press", q)
    if rng.random() < .05:                            # stack join 0x53B2C?: an empty stack of the same type takes the units
        t = rng.choice(stackable)                     # (the first unit goes to +2, not to +7)
        m = w.g8(inv.ITEM_TBL + 12 * t + 6)
        if m > 1:
            q = rng.choice([0, 1, 7, 8, 9, 10, 11])
            w.s8(s.addr + 72, 0x35)                   # no pocket is missing
            w.s8(s.addr + 12 * q, t)
            w.s8(s.addr + 12 * q + 1, 0)
            w.s8(s.addr + 12 * q + 2, 0x11)
            w.s8(s.addr + 12 * q + 7, 0x22)
            w.s8(inv.CURSOR, t)
            w.s8(inv.CURSOR + 1, rng.randrange(1, min(5, m) + 1))
            w.s8(inv.CURSOR + 2, rng.randrange(40, 101))
            return ("press", q)
    return pair


def make_script(rng):
    r = rng.random
    return {"select": [rng.choice([1, 1, 0]) for _ in range(6)],
            "merge_prompt": [rng.choice([1, 1, 1, 0]) for _ in range(4)],
            "random": [rng.choice([rng.randrange(256), rng.randrange(100), rng.randrange(3), 49, 50, 51, 1, 2])
                       for _ in range(8)],
            "body": [rng.choice([0, 10, 25, 40]) for _ in range(12)],
            "item_info": [rng.choice([0, 0, 1]) for _ in range(8)],
            "item_value": [rng.choice([0, 0, 7, 0x1234]) for _ in range(8)],
            "mine_spotted": [rng.choice([0, 0, 1]) for _ in range(4)],
            "disarm": [rng.choice([1, 1, 0, -1 & 0xFFFFFFFF]) for _ in range(4)],
            "w_682ac": [rng.randrange(5) for _ in range(2)]} if r() < 2 else {}


def port_safe_events(events, item_pos):
    """--port only: drop ctrl from left clicks whose first item rect is 12 (attachment) or 13 (ground). The original
    then unstacks soldier bytes +0x90 / +0x9C (issue #2: the animation pointer and the bytes after it); the port keeps
    +0x90 as a script offset, so the two can not agree byte for byte (docs/spec/inventory.md Item click)."""
    out = []
    for x, y, b, kb, key in events:
        if b & 1 and kb & 0x104:
            hit = next((k for k, (rx, ry) in enumerate(item_pos)
                        if rx - 4 <= x <= rx + 0x2D and ry - 4 <= y <= ry + 0x13), None)
            if hit in (12, 13):
                kb &= ~0x104
        out.append((x, y, b, kb, key))
    return out


def make_events(rng, item_pos, slot_pos, pair=False):
    ev = []
    if isinstance(pair, tuple):                       # a first press that fuzz prepared (fuzz, the end)
        x, y = item_pos[pair[1]]
        ev += [(x + 2, y + 2, 1, 0, 0), (x + 2, y + 2, 0, 0, 0)]
    elif pair and rng.random() < .8:
        x, y = item_pos[12]
        x, y = x + rng.randrange(0, 0x28), y + rng.randrange(0, 0x10)
        ev += [(x, y, 1, rng.choice([0, 0, 1]), 0), (x, y, 0, 0, 0)]
    for _ in range(rng.randrange(1, 7)):
        kb = rng.choice([0, 0, 0, 1, 2, 4, 0x100])
        kind = rng.random()
        if kind < .45:
            x, y = rng.choice(item_pos + [item_pos[12]] * 4)
            x, y = x + rng.randrange(-4, 0x2E), y + rng.randrange(-4, 0x14)
        elif kind < .6:
            x, y = rng.choice(slot_pos)
            x, y = x + rng.randrange(0, 0x30), y + rng.randrange(0, 0x2A)
        elif kind < .7:
            x, y = rng.choice([(0x84, 0x4A), (0x83, 0x64), (0x56, 0x2A), (0x50, 0x25)])
        elif kind < .75:                          # on or next to the edge of a button or an item place
            x0, y0, x1, y1 = rng.choice([(72, 31, 100, 55), (108, 62, 156, 86), (112, 82, 150, 118), (70, 28, 102, 56)]
                                        + [(px, py, px + 47, py + 41) for px, py in slot_pos]
                                        + [(px - 4, py - 4, px + 45, py + 19) for px, py in item_pos])
            x = rng.choice([x0 - 1, x0, x1, x1 + 1, rng.randrange(x0, x1 + 1)])
            y = rng.choice([y0 - 1, y0, y1, y1 + 1, rng.randrange(y0, y1 + 1)])
        elif kind < .8:
            x, y = rng.randrange(320), rng.randrange(0xB0, 200)
        elif kind < .88:
            ev.append((rng.randrange(320), rng.randrange(200), 0, kb, rng.choice([0x1B, ord("T"), ord("t"), ord("q"), 0xE0])))
            continue
        else:
            x, y = rng.randrange(320), rng.randrange(200)
        b = rng.choice([1, 1, 1, 2])
        ev.append((x, y, b, kb, 0))
        for _ in range(rng.choice([0, 0, 1, 2])):
            ev.append((x, y, b, kb, 0))
        ev.append((x, y, 0, kb, 0))
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
    print(f"inv_diff: {total} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
