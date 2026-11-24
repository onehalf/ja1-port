"""Differential test: item moves on the item panel of the assignment screen (strategic.md "Item moves on the item
panel", "The vest map"), original vs port.
usage: .venv/bin/python validate/diff/assign_items_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case as validate/diff/assign_panel_diff.py (fuzzed team, picture), plus a sorted item store (records and counts
that agree), an item on the cursor, the vest owner, shift / ctrl and the answer of the discard question. Script: a
press on a face box selects a merc, then up to 8 item presses (each left down, then up at the same place): item
places 0..13, the body, the store places, the rest of the store box, a place that holds nothing, or the key E
(strategic.md "Key E: all items into the store"; the port's ja1_assign_key_e). The original
runs AssignScreen? 0x4891C in Unicorn (input polls 0x678E8 / 0x6782C scripted, malloc_ / free_ stubbed) until
the script ends. The port = port/ja1front ja1f_assign_items (ja1logic ja1_assign_item_press per press, with the
front-end's host ops). Compared: the screen, the panel buffer, team slots 0..8, the cursor and swap items, the vest
owner, the store (records in list order, counts, view, current record, top type, shown types), the loader result
and the log of sounds, cursor pictures, texts and questions. Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop         # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_EAX, UC_X86_REG_EDX   # noqa: E402
from unicorn import UC_HOOK_CODE   # noqa: E402
import datarc, pcx                   # noqa: E402
import assign_fuzz as fz             # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
MAXREC, MAXLOG = 256, 256
DBG = os.environ.get("ITEMS_DBG")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_assign_items.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p] * 8 + [ctypes.c_int32] * 2 + \
        [ctypes.c_void_p, ctypes.c_int32] + [ctypes.c_void_p] * 5
    PORT.ja1f_assign_items.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SOLDIERS, FACES, ACC, PANEL_P, SCREEN_P = 0xFA958, 0x93428, 0x229FA0, 0x93E60, 0x95678
MOUSE_X, MOUSE_Y, BUTTONS = 0x229D00, 0x229D04, 0x229CF4
MODE, CUR, COUNTS, TOP, SHOWN, LOADRES = 0x93EA4, 0x229C7C, 0x93DD4, 0x229C88, 0x93DD0, 0x229D18
HEAD, TAIL, V, LAST = 0x229C78, 0x229C60, 0x934ED, 0x229C20
CURSOR, SWAP, VEST, SHIFT, CTRL = 0x222B50, 0x222B68, 0x92E40, 0x284F21, 0x284F1F
SOUNDS = {"empty.8": 1, "hmm.8": 2, "huh.8": 3, "noroom.8": 4}
MERC_TEXTS = {"doesn't have enough Action Points!": 1, "has reloaded.": 2}
_pics = {}


def pic(name):
    n = name.decode("latin-1").upper()
    if n not in _pics:
        try:
            w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
            _pics[n] = (w, h, px)
        except (KeyError, ValueError):
            _pics[n] = None
    return _pics[n]


e = Emu(dump=dump, fast=True)


def load(em, eax, edx, ebx, ecx):
    p = pic(e.cstr(ebx))
    if p is None:
        return 0xFFFFFFFF
    w, h, px = p
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


st = {}
NONE_XY = (300, 150)


def store_spec_gap(x, y):
    """A press on a store place in view 0 with no current record and an empty cursor (SPEC GAP: the original reads
    memory address 0)."""
    return (25 <= x <= 72 and 60 <= y <= 132 and e.r32(e.rt(MODE)) == 0 and e.r32(e.rt(CUR)) == 0
            and e.r8(e.rt(CURSOR)) == 0 and any(29 <= x <= 70 and 59 + 19 * j <= y <= 74 + 19 * j for j in range(4)))


def poll(em, *_):
    if not st:
        return 0
    j = st["polls"]
    st["polls"] = j + 1
    if j >= len(st["ev"]):
        em.stop_reason = "script end"
        em.uc.emu_stop()
        return 0
    x, y, b = st["ev"][j][:3]
    st["key"] = st["ev"][j][3] if len(st["ev"][j]) > 3 else 0
    if b == 2 and j >= 2 and e.r32(e.rt(MODE)) == 0 and e.r32(e.rt(CUR)) == 0 and \
            any(29 <= x <= 70 and 59 + 19 * q <= y <= 74 + 19 * q for q in range(4)):
        st["ev"][j] = NONE_XY + (2,)          # right press on a store place, view 0, no current record (SPEC GAP)
        st["gaps"] += 1
        x, y, b = st["ev"][j]
    if b == 1 and j >= 2 and store_spec_gap(x, y):
        st["ev"][j], st["ev"][j + 1] = NONE_XY + (1,), NONE_XY + (0,)
        st["gaps"] += 1
        x, y, b = st["ev"][j]
    e.w32(e.rt(MOUSE_X), x)
    e.w32(e.rt(MOUSE_Y), y)
    e.w8(e.rt(BUTTONS), b)
    return 0


def mouse_to(em, eax, edx, *_):
    e.w32(e.rt(MOUSE_X), eax)
    e.w32(e.rt(MOUSE_Y), edx)
    return 0


def logger(kind, f=lambda eax, edx, ebx: 0, ret=0):
    def stub(em, eax, edx, ebx, ecx):
        if st.get("log") is not None and st["polls"] >= 1:    # not the screen's start
            st["log"].append((kind, f(eax, edx, ebx)))
            if DBG:
                print("  log", kind, f(eax, edx, ebx), "from", hex(e.r32(e.uc.reg_read(UC_X86_REG_ESP)) - e.rt(0x10000) + 0x10000),
                      "poll", st["polls"])
        return ret() if callable(ret) else ret
    return stub


HEAP = e.alloc(bytes(7 * 4096))


def malloc(em, eax, *_):
    a = st["heap"]
    st["heap"] = a + ((eax + 7) & ~7)
    return a


def free(em, eax, *_):
    st.setdefault("freed", []).append(eax)
    return 0


def get_key(em, *_):
    k, st["key"] = st.get("key", 0), 0
    return k


for a in (0x67BF0, 0x673AC, 0x67E18, 0x67E30, 0x47AE4, 0x5C8A0, 0x28698, 0x5C900,
          0x6742C, 0x67444, 0x23264, 0x23420, 0x467E4):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x5C880), logger(1))
e.stub(e.rt(0x67478), logger(2, lambda a, d, b: a & 0xFF))      # 0x674B0 with the mouse hidden
e.stub(e.rt(0x674B0), logger(2, lambda a, d, b: a & 0xFF))
e.stub(e.rt(0x25AD8), logger(3, lambda a, d, b: (lambda n: SOUNDS.get(n, -1) if n else 0)(
    e.cstr(a).decode("latin-1").lower() if a else "")))
e.stub(e.rt(0x5CB34), logger(4, lambda a, d, b: a & 0xFF))
e.stub(e.rt(0x15910), logger(5, lambda a, d, b: MERC_TEXTS.get(e.cstr(d).decode("latin-1"), -1)))
# PlayNoRoom? 0x53A34 runs (inventory.md "No room"): it calls 0x25AD8 above or moves units
e.stub(e.rt(0x278A0), logger(7, ret=lambda: st["answer"]), 0x20)
e.stub(e.rt(0x24F08), logger(8, lambda a, d, b: 252), 0x10)
e.stub(e.rt(0x28C84), logger(8, lambda a, d, b: 253), 4)
e.stub(e.rt(0x62A68), logger(8, lambda a, d, b: 105))
e.stub(e.rt(0x53538), logger(9))
e.stub(e.rt(0x27D24), logger(10))
e.stub(e.rt(0x76383), malloc)
e.stub(e.rt(0x76325), free)
def item_info(em, eax, edx, ebx, ecx):
    sp = em.uc.reg_read(UC_X86_REG_ESP)
    a = [em.r32(sp + 4 * q) for q in (1, 2, 3)]
    for kd, v in ((11, eax & 0xFF), (12, edx & 0xFF), (13, ebx), (14, ecx), (15, a[0] * 4 + a[1] * 2 + a[2])):
        logger(kd, lambda *_: (v + 2**31) % 2**32 - 2**31)(em, eax, edx, ebx, ecx)
    return st["answer"]


e.stub(e.rt(0x25D20), item_info, 0x0C)
e.stub(e.rt(0x5616C), lambda em, *_: int(bool(st.get("key"))))   # key waiting (the key E presses)
e.stub(e.rt(0x560F4), get_key)
e.stub(e.rt(0x678E8), poll)
e.stub(e.rt(0x6782C), poll)
e.stub(e.rt(0x681D0), mouse_to)
e.stub_sprintf()
e.stub(e.rt(0x532BC), load, 4)
CASE = [0]
if os.environ.get("ITEMS_PROBE"):              # debug: print EIP, EAX, EDX, cursor, swap at given addresses
    def _probe(uc, a, size, _):
        if os.environ.get("ITEMS_CASE") and int(os.environ["ITEMS_CASE"]) != CASE[0]:
            return
        print(f"  probe {a - e.rt(0x10000) + 0x10000:#x} eax {uc.reg_read(UC_X86_REG_EAX):#x} edx "
              f"{uc.reg_read(UC_X86_REG_EDX):#x} cursor {bytes(uc.mem_read(e.rt(CURSOR), 6)).hex()} swap "
              f"{bytes(uc.mem_read(e.rt(SWAP), 6)).hex()} head {e.r32(e.rt(HEAD)):#x} cur {e.r32(e.rt(CUR)):#x}")
    for _a in os.environ["ITEMS_PROBE"].split(","):
        e.uc.hook_add(UC_HOOK_CODE, _probe, None, e.rt(int(_a, 16)), e.rt(int(_a, 16)))
    e.uc.ctl_flush_tb()
if os.environ.get("ITEMS_WPROBE"):             # debug: print writes to the cursor item
    from unicorn import UC_HOOK_MEM_WRITE
    e.uc.hook_add(UC_HOOK_MEM_WRITE, lambda uc, acc, a, size, v, _: print(
        f"  cursor write +{a - e.rt(CURSOR)} {v:#x} at {uc.reg_read(UC_X86_REG_EIP) - e.rt(0x10000) + 0x10000:#x}"),
        None, e.rt(CURSOR), e.rt(CURSOR) + 11)
panel = e.r32(e.rt(PANEL_P))
screen = e.r32(e.rt(SCREEN_P))
faces = [e.r32(e.rt(FACES) + 4 * k) for k in range(8)]
base_sol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
base_acc = bytearray(e.uc.mem_read(e.rt(ACC), 0xA8))
e.uc.mem_write(e.rt(0x949F2), bytes(e.uc.mem_read(e.rt(0x94A72), 0x38)))
TYPES = list(range(1, 0x71))
AMMO = bytes(e.uc.mem_read(e.rt(0x94C32), 16))
STACK = bytes(e.uc.mem_read(e.rt(0x94DF8), 136 * 12))[6::12]   # item type byte 6 = the stack class (max 5)


def stack_count(r, t, n):
    # a stack never holds more than its class (merges stop at it); above 5 the original reads past the item
    return r.randrange(1, STACK[t] + 1) if t < 136 and STACK[t] > 1 and STACK[t] < n < 128 else n


def rand_item(r, t=None):
    t = r.choice(TYPES) if t is None else t
    it = bytearray(12)
    it[0] = t
    it[1] = r.choice([1, 1, 2, 3, 5, 0, 9, 10, 50, 0xFF, r.randrange(256)])
    it[1] = stack_count(r, t, it[1])
    it[2] = r.choice([1, 5, 9, 10, 50, 99, 100, 0xEC, r.randrange(101)])
    if r.random() < .2:
        it[3:6] = bytes([r.choice(TYPES), r.randrange(256), r.choice([1, 50, 100])])
        it[4] = stack_count(r, it[3], it[4])
    if r.random() < .3:
        it[7:12] = bytes(r.choice([0, 10, 50, 100, r.randrange(110), r.randrange(128, 256)]) for _ in range(5))
    return it


def fuzz_items(r, s):
    for k in range(12):
        a = 12 * k
        if r.random() < .3:
            s[a:a + 12] = bytes(12)
            continue
        s[a:a + 12] = rand_item(r)
    for o, arm in ((0x18, [0x23, 0x24, 0x25, 0x26]), (0x3C, [0x2D, 0x2E, 0x2F, 0x30]),
                   (0x48, [0x31, 0x32, 0x33, 0x34, 0x35, 0x36])):
        if r.random() < .5:
            s[o:o + 3] = bytes([r.choice(arm), 1, r.choice([1, 5, 50, 99, 100, r.randrange(1, 101)])])
    if r.random() < .3:                       # an empty gun with its ammo on the cursor (reload)
        s[0:3] = bytes([r.randrange(1, 15), 0, r.choice([50, 100])])
    if s[3] > 0x70:
        s[3] = r.choice(TYPES)
    for k in range(14):                       # the original's condition bar never ends at 110..127
        if 110 <= s[12 * k + 2] <= 127:
            s[12 * k + 2] = 100
    if 110 <= s[5] <= 127:
        s[5] = 100
    s[0x17C] = r.choice([r.randrange(11), r.randrange(11), r.randrange(256)])
    s[0x1B1] = r.choice([0, 0, 1, 7])


def scond(c):
    return c - 256 if c > 127 else c


def fuzz_store(r):
    recs = []
    for _ in range(r.choice([0, 1, 2, 3, 4, 5, 6, 8, 12, 20])):
        t = r.choice(TYPES + [0x41, 0x47, 0x32])
        c = r.choice([1, 9, 10, 11, 50, 100, r.randrange(110), r.randrange(128, 256)])
        recs.append(bytes([t, stack_count(r, t, r.choice([1, 1, 1, 2, 5, 0xFF, r.randrange(256)])), c]))
    if r.random() < .1:                       # a type near the limit of 100 (counts always match the records)
        t = r.choice(TYPES)
        recs = [x for x in recs if x[0] != t] + [bytes([t, 1, r.choice([10, 50, 100])])
                                                 for _ in range(r.choice([97, 98, 99]))]
    recs.sort(key=lambda x: (x[0], -scond(x[2])))
    counts = bytearray(136)
    for x in recs:
        counts[x[0]] += 1
    return counts, recs


PLACES = [(108, 73), (203, 89), (161, 8), (108, 31), (200, 28), (108, 52), (257, 18), (257, 37), (257, 56),
          (257, 75), (257, 94), (257, 113), (108, 92), (159, 115)]
BUTTONS_XY = [(8, 25), (69, 25), (79, 72), (79, 108), (79, 90)]


def busy(x, y):
    rects = [(34, 12, 62, 36), (161, 48, 199, 84), (25, 60, 72, 132), (269, 166, 294, 181)]
    rects += [(bx, by, bx + 19, by + 15) for bx, by in BUTTONS_XY]
    rects += [(qx - 4, qy - 4, qx + 45, qy + 19) for qx, qy in PLACES]
    return any(a <= x <= c and b <= y <= d for a, b, c, d in rects)


def press(r, kind, ctrl):
    if kind == "place":
        while True:
            q = r.choice(range(13) if ctrl else range(14))
            qx, qy = PLACES[q]
            x, y = r.randint(qx - 4, qx + 45), r.randint(qy - 4, qy + 19)
            # the first place that holds the mouse must be q, and no button / the face before it
            hit = next(k for k, (ax, ay) in enumerate(PLACES) if ax - 4 <= x <= ax + 45 and ay - 4 <= y <= ay + 19)
            on_btn = any(bx <= x <= bx + 19 and by <= y <= by + 15 for bx, by in BUTTONS_XY) or \
                (34 <= x <= 62 and 12 <= y <= 36) or (161 <= x <= 199 and 48 <= y <= 84)
            if hit == q and not on_btn and not (ctrl and hit == 13):
                return x, y
    if kind == "body":
        return r.randint(161, 199), r.randint(48, 84)
    if kind == "store_place":
        j = r.randrange(4)
        return r.randint(29, 70), r.randint(59 + 19 * j, 74 + 19 * j)
    if kind == "store":
        return r.randint(25, 72), r.randint(60, 132)
    while True:
        x, y = r.randrange(320), r.randrange(143)
        if not busy(x, y):
            return x, y


def read_store():
    out, a, seen = [], e.r32(e.rt(HEAD)), 0
    idx = {}
    while a and seen < 1000:
        idx[a] = len(out)
        out.append(bytes(e.uc.mem_read(a, 3)))
        a = e.r32(a + 3)
        seen += 1
    c = e.r32(e.rt(CUR))
    return out, (idx.get(c, -1) if c else 1 << 20)


r = random.Random(int(os.environ.get("ITEMS_SEED", 1)))
bad = gaps = 0
outcomes, logkinds = {}, {}
for i in range(ncases):
    CASE[0] = i
    n = r.choice([1, 2, 3, 4, 5, 6, 7, 8, 8])
    sol = bytearray(base_sol)
    for k in range(9):
        s = bytearray(sol[k * 0x200:(k + 1) * 0x200])
        fz.fuzz_soldier(r, s, k, n)
        fuzz_items(r, s)
        sol[k * 0x200:(k + 1) * 0x200] = s
    fc = [bytes(r.randrange(256) for _ in range(1024)) for _ in range(8)]
    acc = bytearray(base_acc)
    for o in (0x24, 0x28, 0x30, 0x34, 0x38):
        acc[o:o + 4] = r.randint(-99999, 999999).to_bytes(4, "little", signed=True)
    mode = 1 if r.random() < 0.5 else 0
    counts, recs = fuzz_store(r)
    r2 = random.Random(f"stack{i}")           # own generator: the cases of the main stream stay as they were
    stack_case = r2.random() < 0.25           # 2..7 store records of one stacking type: a Shift press takes a stack
    if stack_case:
        t = r2.choice([x for x in TYPES if x < 136 and STACK[x] > 1])
        recs = recs[:12] + [bytes([t, 1, r2.choice([10, 50, 100])]) for _ in range(r2.randrange(2, 8))]
        recs.sort(key=lambda x: (x[0], -scond(x[2])))
        counts = bytearray(136)
        for x in recs:
            counts[x[0]] += 1
    k = r.randrange(n)
    if r.random() < .15:                      # an item type of merc k near the limit of 100 (key E loses it)
        ts = [sol[k * 0x200 + 12 * q] for q in range(12) if 0 < sol[k * 0x200 + 12 * q] < 0x71]
        if ts:
            t = r.choice(ts)
            recs = [x for x in recs if x[0] != t] + [bytes([t, 1, r.choice([10, 50, 100])])
                                                     for _ in range(r.choice([97, 98, 99]))]
            recs.sort(key=lambda x: (x[0], -scond(x[2])))
            counts = bytearray(136)
            for x in recs:
                counts[x[0]] += 1
    cursor = bytearray(12)
    if r.random() < 0.6:
        cursor = rand_item(r, r.choice([None, None, 0x47, 0x32, 0x33, 0x34, 0x35, 0x41, 0x27]))
        if r.random() < 0.25:                 # ammo for the gun in hand 0
            g = sol[k * 0x200]
            if 1 <= g <= 14:
                cursor = bytearray([AMMO[g] if g < len(AMMO) else 0, 1, 100] + [0] * 9)
    if r.random() < 0.5:                      # the cursor's stacking type in the pockets: join, no room, place 3 (strategic.md "Item moves on the item panel")
        cursor = bytearray([0x27, stack_count(r, 0x27, r.randrange(1, 11)), 100] + [0] * 9)
        for q in (1, 2, 3, 4, 5):
            sol[k * 0x200 + 12 * q:k * 0x200 + 12 * q + 12] = bytes([0x27, stack_count(r, 0x27, r.randrange(1, 11)), 100] + [0] * 9)
    vest = r.choice([0x40, 0x40, k, r.randrange(9)])
    # A vest out of hand 0 onto place 6 (the opening clears the cursor, so the first press picks it up): 0x470E8 pocket
    # swap with another vest owner, 0x5193C pockets into the store when the new vest is smaller.
    vest_case = r.random() < 0.2
    if vest_case:
        sol[k * 0x200:k * 0x200 + 12] = rand_item(r, r.choice([0x32, 0x33, 0x34]))
        sol[k * 0x200 + 6 * 12] = r.choice([0x33, 0x34, 0x35])
        for q in range(7, 12):
            sol[k * 0x200 + 12 * q] = r.choice([0, 0x47, 0x27, 0x41])
            sol[k * 0x200 + 12 * q + 1] = 1
            sol[k * 0x200 + 12 * q + 2] = 100
        vest = r.choice([0x40, k, r.randrange(n)])
        if vest < 0x40 and vest != k:         # MsgVestNotEmpty? 0x4A7BC stops the move when the owner's pockets hold items
            for q in range(7, 12):
                sol[vest * 0x200 + 12 * q] = 0
    shift, ctrl = int(r.random() < 0.3), int(r.random() < 0.15)
    if stack_case and r2.random() < 0.7:
        shift = 1
    answer = r.choice([0, 1])
    e.uc.mem_write(e.rt(SOLDIERS), bytes(sol))
    for q in range(8):
        e.uc.mem_write(faces[q], fc[q])
    e.uc.mem_write(e.rt(ACC), bytes(acc))
    e.uc.mem_write(panel, bytes(r.randrange(256) for _ in range(64000)))
    e.w32(e.rt(MODE), mode)
    e.uc.mem_write(e.rt(COUNTS), bytes(counts))
    st.clear()
    st.update(heap=HEAP, answer=answer, log=None)
    nodes = [malloc(e, 7) for _ in recs]
    for j, rec in enumerate(recs):
        e.uc.mem_write(nodes[j], rec + struct.pack("<I", nodes[j + 1] if j + 1 < len(recs) else 0))
    e.w32(e.rt(HEAD), nodes[0] if recs else 0)
    e.w32(e.rt(TAIL), nodes[-1] if recs else 0)
    cur = r.choice([0, 0, 1, 2, r.randrange(len(recs))]) if recs else 0
    cur = min(cur, len(recs) - 1) if recs else 1 << 20
    e.w32(e.rt(CUR), nodes[cur] if recs else 0)
    e.w32(e.rt(TOP), 0)
    e.w32(e.rt(LOADRES), 0x55)
    e.uc.mem_write(e.rt(SHOWN), bytes(4))
    vb = r.choice([0x38, 0x39])
    e.w8(e.rt(V), vb)
    e.uc.mem_write(e.rt(CURSOR), bytes(cursor))
    e.uc.mem_write(e.rt(SWAP), bytes(12))
    e.w8(e.rt(VEST), vest)
    e.w8(e.rt(SHIFT), shift)
    e.w8(e.rt(CTRL), ctrl)
    e.w16(e.rt(0x22A46C), shift | 4 * ctrl)   # the screen loop reads shift (bits 0, 1) and ctrl (0x104) from here
    e.w32(e.rt(LAST), n - 1)
    e.call(e.rt(0x478A4), 99)
    pan = bytearray(e.uc.mem_read(panel, 64000))
    scr = bytearray(pan)
    e.uc.mem_write(screen, bytes(scr))
    sol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
    if not sol[k * 0x200 + 0x182]:
        continue
    fx = 11 + 37 * k
    px, py = r.randint(fx, fx + 30), r.randint(20, 46)
    ev = [(px, py, 1), (r.randint(0, 319), r.randint(0, 142), 0)]
    kinds, presses = [], []
    for _ in range(r.choice([2, 3, 4, 6, 8] if vest_case else [1, 2, 3, 4, 6, 8])):
        kind = r.choice(["place"] * 6 + ["body", "store_place", "store_place", "store", "none", "key"] +
                        ["rplace", "rplace", "rstore", "rnone"])
        if vest_case and len(kinds) < 2:
            kind = "place"
        x, y = press(r, {"key": "none", "rplace": "none", "rstore": "store_place", "rnone": "none"}.get(kind, kind), ctrl)
        if vest_case and len(kinds) < 2:
            qx, qy = PLACES[6 if kinds else 0]
            x, y = qx + 2, qy + 2
        if kind == "rplace":                  # the right button's place rectangles (13 places)
            qx, qy = PLACES[r.randrange(13)]
            x, y = r.randint(qx, qx + 41), r.randint(qy, qy + 15)
        kinds.append(kind)
        if kind == "key":                     # the key E (either case) with the left button up
            ev.append((x, y, 0, r.choice([0x45, 0x65])))
            presses += [-1, 0]
        elif kind[0] == "r":                  # right press: a place hit waits for the right button up twice
            hit = any(qx <= x <= qx + 41 and qy <= y <= qy + 15 for qx, qy in PLACES[:13])
            ev += [(x, y, 2)] + [(x, y, 0)] * (2 if hit else 1)
            presses += [x + 1000, y]
        else:
            ev += [(x, y, 1), (x, y, 0)]
            presses += [x, y]
    if os.environ.get("ITEMS_NPRESS"):        # debug: keep only the first N presses
        nk = int(os.environ["ITEMS_NPRESS"])
        ev = ev[:2 + sum(1 if kd == "key" else 3 if kd == "rplace" else 2 for kd in kinds[:nk])]
        kinds, presses = kinds[:nk], presses[:2 * nk]
    st.update(polls=0, ev=ev, gaps=0, log=[], freed=[])
    try:
        e.call(e.rt(0x4891C), max_steps=150_000_000)
        print(f"case {i}: the original returned")
        bad += 1
        continue
    except EmuStop as x:
        if e.stop_reason != "script end":
            print(f"case {i}: original stopped: {x} k {k} polls {st['polls']} kinds {kinds} "
                  f"eip {e.uc.reg_read(UC_X86_REG_EIP) - e.rt(0x10000) + 0x10000:#x}")
            if os.environ.get("ITEMS_V"):
                sp, d = e.uc.reg_read(UC_X86_REG_ESP), e.rt(0x10000) - 0x10000
                print("  stack code addresses:", [hex(v - d) for v in struct.unpack("<64I", e.uc.mem_read(sp, 256))
                                                  if 0x10000 <= v - d < 0x8AC7D][:8], "ev", st["ev"], "cursor",
                      bytes(cursor).hex())
            bad += 1
            if bad > 10:
                break
            continue
    gaps += st["gaps"]
    orecs, ocur = read_store()
    o = dict(view=e.r32(e.rt(MODE)), cur=ocur, top=e.r32(e.rt(TOP)) & 0xFF, shown=bytes(e.uc.mem_read(e.rt(SHOWN), 4)),
             v=e.r8(e.rt(V)), vest=e.r8(e.rt(VEST)), loader=struct.unpack("<i", e.uc.mem_read(e.rt(LOADRES), 4))[0],
             recs=orecs, counts=bytes(e.uc.mem_read(e.rt(COUNTS), 136)),
             cursor=bytes(e.uc.mem_read(e.rt(CURSOR), 12)), swap=bytes(e.uc.mem_read(e.rt(SWAP), 12)),
             log=st["log"])
    oscr, opan = bytes(e.uc.mem_read(screen, 64000)), bytes(e.uc.mem_read(panel, 64000))
    osol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
    for kd in kinds:
        outcomes[kd] = outcomes.get(kd, 0) + 1
    for kd, _ in st["log"]:
        logkinds[kd] = logkinds.get(kd, 0) + 1
    if PORT:
        presses, j = [], 2                    # from ev: the poll stub replaces store spec-gap presses there
        while j < len(ev):                    # a press = its first event, then the button-up events
            x, y, b = ev[j][:3]
            presses += [-1, 0] if len(ev[j]) > 3 else [x + 1000, y] if b == 2 else [x, y]
            j += 1
            while j < len(ev) and len(ev[j]) == 3 and ev[j][2] == 0:
                j += 1
        ps = ctypes.create_string_buffer(sol, 9 * 0x200)
        pp, psc = ctypes.create_string_buffer(bytes(pan), 64000), ctypes.create_string_buffer(bytes(scr), 64000)
        pco = ctypes.create_string_buffer(bytes(counts), 136)
        prec = ctypes.create_string_buffer(b"".join(recs) + bytes(3 * (MAXREC - len(recs))), 3 * MAXREC)
        nrec = ctypes.c_int32(len(recs))
        stt = (ctypes.c_int32 * 16)(mode, cur, 0, 0, 0, 0, 0, vb, vest, shift, ctrl, answer, 0, 0, 0, 0)
        pcur = ctypes.create_string_buffer(bytes(cursor), 12)
        pswp = ctypes.create_string_buffer(12)
        plog = (ctypes.c_int32 * (2 * MAXLOG))()
        nlog = ctypes.c_int32(0)
        pr = (ctypes.c_int32 * len(presses))(*presses)
        rc = PORT.ja1f_assign_items(ISO, EXE, ps, b"".join(fc), bytes(acc), pco, prec, ctypes.byref(nrec), pp, psc,
                                    px, py, pr, len(presses) // 2, stt, pcur, pswp, plog, ctypes.byref(nlog))
        nr = nrec.value
        p = dict(view=stt[0], cur=stt[1] if stt[1] < nr else 1 << 20, top=stt[2] & 0xFF,
                 shown=bytes(stt[3 + q] & 0xFF for q in range(4)), v=stt[7], vest=stt[8], loader=stt[12],
                 recs=[prec.raw[3 * q:3 * q + 3] for q in range(nr)], counts=pco.raw, cursor=pcur.raw, swap=pswp.raw,
                 log=[(plog[2 * q], plog[2 * q + 1]) for q in range(nlog.value)])
        if o["cur"] != 1 << 20 and o["cur"] >= len(orecs):
            o["cur"] = 1 << 20
        msg = [] if rc == 0 else [f"rc {rc}"]
        for key in o:
            if o[key] != p[key] and key == "recs":
                q = next((q for q in range(min(len(o[key]), len(p[key]))) if o[key][q] != p[key][q]),
                         min(len(o[key]), len(p[key])))
                msg.append(f"recs ({len(o[key])} vs {len(p[key])}) first diff #{q}: {o[key][q:q + 3]} vs {p[key][q:q + 3]}")
            elif o[key] != p[key] and key == "counts":
                msg.append("counts " + " ".join(f"[{q:#x}] {o[key][q]} vs {p[key][q]}" for q in range(136)
                                                if o[key][q] != p[key][q]))
            elif o[key] != p[key]:
                msg.append(f"{key} {o[key]} vs {p[key]}")
        for nm, a, b in (("screen", oscr, psc.raw), ("panel", opan, pp.raw)):
            if a != b:
                nb = next(q for q in range(64000) if a[q] != b[q])
                msg.append(f"{nm} @({nb % 320},{nb // 320}) {a[nb]:#x} vs {b[nb]:#x}")
        if osol != ps.raw:
            nb = next(q for q in range(9 * 0x200) if osol[q] != ps.raw[q])
            msg.append(f"soldier {nb // 0x200} +{nb % 0x200:#x} {osol[nb]:#x} vs {ps.raw[nb]:#x}")
        if msg and DBG:
            for q in range(9):
                if osol[q * 0x200:(q + 1) * 0x200] != ps.raw[q * 0x200:(q + 1) * 0x200]:
                    print(f"  soldier {q} entries 6..11 orig", osol[q * 0x200 + 72:q * 0x200 + 144].hex(),
                          "\n                           port", ps.raw[q * 0x200 + 72:q * 0x200 + 144].hex())
            print("  cursor", o["cursor"].hex(), "swap", o["swap"].hex(), "log", o["log"]); print("  start sol k entries 6..11", sol[k * 0x200 + 72:k * 0x200 + 144].hex(), "slot", sol[k * 0x200 + 0x17A])
        if msg:
            print(f"MISMATCH case {i} k {k} view {mode} recs {len(recs)} cur {cur} shift {shift} ctrl {ctrl} "
                  f"cursor {bytes(cursor[:6]).hex()} presses {list(zip(kinds, zip(presses[::2], presses[1::2])))}: " + "; ".join(msg)[:1500])
            bad += 1
            if bad > 10:
                break
if PORT and ncases:                           # the stop point (strategic.md): an empty store, view 0, store place j
    for j in range(4):
        for rb in (0, 1000):
            ps = ctypes.create_string_buffer(sol, 9 * 0x200)
            pp, psc = ctypes.create_string_buffer(bytes(pan), 64000), ctypes.create_string_buffer(bytes(scr), 64000)
            pco, prec, nrec = ctypes.create_string_buffer(136), ctypes.create_string_buffer(3 * MAXREC), ctypes.c_int32(0)
            stt = (ctypes.c_int32 * 16)(0, 1 << 20, 0, 0, 0, 0, 0, 0x38, 0x40, 0, 0, 0, 0, 0, 0, 0)
            pcur, pswp = ctypes.create_string_buffer(12), ctypes.create_string_buffer(12)
            plog, nlog = (ctypes.c_int32 * (2 * MAXLOG))(), ctypes.c_int32(0)
            pr = (ctypes.c_int32 * 2)(40 + rb, 65 + 19 * j)
            rc = PORT.ja1f_assign_items(ISO, EXE, ps, b"".join(fc), bytes(acc), pco, prec, ctypes.byref(nrec), pp, psc,
                                        px, py, pr, 1, stt, pcur, pswp, plog, ctypes.byref(nlog))
            lg = [(plog[2 * q], plog[2 * q + 1]) for q in range(nlog.value)]
            if rc or (99, 47) not in lg or pcur.raw != bytes(12) or nrec.value or any(k == 7 for k, _ in lg):
                print(f"MISMATCH stop point j {j} {'right' if rb else 'left'}: rc {rc} log {lg} cursor {pcur.raw.hex()}")
                bad += 1
if os.environ.get("ITEMS_V"):
    print("outcomes:", sorted(outcomes.items()), "log kinds:", sorted(logkinds.items()), "spec gaps:", gaps)
print(f"assign_items_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
