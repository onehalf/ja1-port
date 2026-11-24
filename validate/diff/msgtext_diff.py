"""Differential test: the small tactical message functions, original vs the Rust port (libja1front.so).

usage: .venv/bin/python validate/diff/msgtext_diff.py [ncases=400] [DUMP]
Sections (the original runs in Unicorn on tac60; the port entries are ja1f_sees_text, ja1f_weapon_text,
ja1f_nearby_msg, ja1f_guards_text and ja1f_sight_head; #39: this test compared tools/ai.py mirrors before):
- sees: MsgSeesEnemies 0x11448. The nickname (+0x16E, up to the NUL), the signed count +0x1D5 and the slot +0x17A
  are fuzzed; sprintf_ is the harness emulation. Compared: the text at 0x10DA50 and play::sees_text; the slot at
  0x92DC6 against the record's byte (the port's op 197 reads +0x17A itself).
- weapon: WeaponStatusText 0x56F4C (HandGunState 0x56F20 runs for real). The main-hand type, count and condition
  and the how/near arguments are fuzzed; ShowMercMessage? 0x299B8 is stubbed and logged. Compared: g_textBuf
  0xEBAC0 and the call's text with sighting::weapon_text; the call's record, how, near and stack argument 1
  with the fuzzed inputs.
- nearby: MsgEnemiesNearby 0x1B0EC. ShowMessage? 0x27B98 is stubbed and logged: text "Enemies nearby!", the eight
  arguments (0, 0, 1, 1, 0, 1500, 1) and the flag 0x22A0CF = 1. Port: play::NEARBY_MSG (text and ms).
- str_int: MsgStrInt 0x27CE0 with GuardsArriveMsg's text, the i32 fuzzed (sprintf_ is the harness emulation): the
  text and the ms with play::guards_text(1, n) and play::STATUS_MS; the other box arguments (0, 0, 1, 1, 1, 1).
- reset: FUN_2F7A4 0x2F7A4's four bytes at 0x229E00..3 with the bytes after a port loop head that reacts.
- reaction: FUN_56D3C 0x56D3C (HandGunState and FUN_2F7A4 run for real): the effect byte 0x229E03 and its
  0x12-byte table entry 0xEAFF0, the soldier slot 0x229E00, how 0x229E01 and near 0x229E02 are fuzzed; the soldier
  record's main hand is fuzzed; ShowMercMessage? is stubbed and logged. Compared with sighting::loop_head_with:
  the call or no call (text, how, near) and the four bytes after it.
- hears: MsgHearsNoise? 0x72A60 with a merc as listener and an empty tile: strength, kind and the result of DirectionTo?
  0x52560 (stubbed) are fuzzed; StatusMessage? 0x27D24 is stubbed and logged. Its text with play::hears_text.
- tables: the texts in JA.EXE at their addresses (the sites: ResolveSectorBattle 0x3E508, ShowTurnBanner? 0x1054C) with
  play::battle_text, play::Banner::from_kind (colour byte 0x92DDE) and play::message_text (each text once in the image).
Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP as ESP   # noqa: E402

args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args else 400
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(DUMP):
    print(f"msgtext_diff: {DUMP} missing")
    sys.exit(1)

LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
PORT = ctypes.CDLL(LIB)
_P, _I = ctypes.c_char_p, ctypes.c_int32
PORT.ja1f_sees_text.argtypes = [_P, ctypes.c_void_p, _I]
PORT.ja1f_weapon_text.argtypes = [_P, ctypes.c_void_p, _I]
PORT.ja1f_guards_text.argtypes = [_I, _I, ctypes.c_void_p, _I, ctypes.c_void_p]
PORT.ja1f_hears_text.argtypes = [_P, _I, _I, _I, ctypes.c_void_p, _I]
PORT.ja1f_battle_text.argtypes = [_I, _I, ctypes.c_void_p, _I]
PORT.ja1f_message_text.argtypes = [_I, ctypes.c_void_p, _I]
PORT.ja1f_banner.argtypes = [_I, ctypes.c_void_p, _I, ctypes.c_void_p]
PORT.ja1f_nearby_msg.argtypes = [ctypes.c_void_p, _I, ctypes.c_void_p]
PORT.ja1f_sight_head.argtypes = [ctypes.c_void_p] * 6 + [_I]
TXT = ctypes.create_string_buffer(256)
MS = ctypes.c_uint32()


def port_text(n):
    assert n >= 0, n
    return TXT.raw[:n]


def p_sees(r):
    return port_text(PORT.ja1f_sees_text(bytes(r), TXT, 256))


def p_weapon(r):
    return port_text(PORT.ja1f_weapon_text(bytes(r), TXT, 256))


def p_guards(number):
    return port_text(PORT.ja1f_guards_text(1, ctypes.c_int32(number).value, TXT, 256, ctypes.byref(MS))), MS.value


def p_nearby():
    return port_text(PORT.ja1f_nearby_msg(TXT, 256, ctypes.byref(MS))), MS.value


SOLDIERS = (ctypes.c_ubyte * (48 * 512))()
FX = (ctypes.c_ubyte * (16 * 0x12))()


def p_head(r, slot, how, near, fx, table):
    """the port's loop head: (call or None, the four bytes after it). call = (text, slot, how, near)."""
    ctypes.memmove(ctypes.addressof(SOLDIERS) + 512 * slot, bytes(r), 512)
    if fx < 16:
        ctypes.memmove(ctypes.addressof(FX) + 0x12 * fx, table, 0x12)
    sight = (ctypes.c_ubyte * 4)(slot, how, near, fx)
    called, shn = ctypes.c_ubyte(), (ctypes.c_ubyte * 3)()
    n = PORT.ja1f_sight_head(SOLDIERS, FX, sight, ctypes.byref(called), shn, TXT, 256)
    call = (port_text(n), shn[0], shn[1], shn[2]) if called.value else None
    return call, bytes(sight)


def s8(v):
    return v - 256 if v & 0x80 else v


SOLD = 512
e = Emu(dump=DUMP, fast=True)
rt = e.rt
e.stub_sprintf()
REC = e.alloc(bytes(SOLD))
TB = e.alloc(64)
SEES_OUT, SEES_SLOT = rt(0x10DA50), rt(0x92DC6)     # dump mode: image addresses shift
TEXT_BUF, ENEMIES_FLAG = rt(0xEBAC0), rt(0x22A0CF)
REACT, FX_TABLE, SOLD_PTRS = rt(0x229E00), rt(0xEAFF0), rt(0x100958)
log = []                             # one tuple per stubbed message call


def show_msg(em, eax, edx, ebx, ecx):
    """ShowMessage? 0x27B98 (text in EAX; 4 more arguments on the stack)."""
    sp = em.uc.reg_read(ESP)
    text = bytes(em.uc.mem_read(eax, 160)).split(b"\0")[0]
    log.append((text, edx & 0xFFFFFFFF, ebx & 0xFFFFFFFF, ecx & 0xFFFFFFFF,
                em.r32(sp + 4), em.r32(sp + 8), em.r32(sp + 12), em.r32(sp + 16)))
    return 0


def show_merc(em, eax, edx, ebx, ecx):
    """ShowMercMessage? 0x299B8 (record EAX, how DL, near BL, text ECX, one stack argument)."""
    sp = em.uc.reg_read(ESP)
    text = bytes(em.uc.mem_read(ecx, 32)).split(b"\0")[0]
    log.append((eax & 0xFFFFFFFF, edx & 0xFF, ebx & 0xFF, text, em.r32(sp + 4)))
    return 0


e.stub(rt(0x27B98), show_msg, 0x10)
e.stub(rt(0x299B8), show_merc, 4)

rng = random.Random(int(os.environ.get("MSGTEXT_SEED", "1")))
fails = {k: 0 for k in ("sees", "weapon", "nearby", "str_int", "reset", "reaction", "hears", "tables")}


def report(sec, msg):
    fails[sec] += 1
    if fails[sec] <= 5:
        print(f"MISMATCH {sec}: {msg}")


def nick(n):
    return bytes(rng.randrange(0x21, 0x7F) for _ in range(n))


def rec_for_case():
    r = bytearray(SOLD)
    t = rng.choice([0, 0, rng.randrange(1, 0x11), rng.randrange(0x12, 0x100)])
    r[0], r[1], r[2] = t, rng.randrange(256), rng.randrange(256)
    r[0x17A] = rng.randrange(256)
    r[0x1D5] = rng.randrange(256)
    return r


# 1. MsgSeesEnemies 0x11448
SEES_COUNTS = (0, 1, 2, 3, 0x7F, 0x80, 0xFF)
for i in range(N):
    r = rec_for_case()
    if i < len(SEES_COUNTS):
        r[0x1D5] = SEES_COUNTS[i]
    nm = nick(rng.randrange(0, 10))
    r[0x16E:0x16E + len(nm)] = nm
    r[0x16E + len(nm)] = 0
    e.uc.mem_write(REC, bytes(r))
    e.w8(SEES_OUT, 0xAA)             # the fill byte after the written text is not compared
    e.call(rt(0x11448), REC)
    got, want = e.cstr(SEES_OUT), p_sees(r)
    if got != want:
        report("sees", f"count {s8(r[0x1D5])} name {nm!r}: original {got!r} port {want!r}")
    if e.r8(SEES_SLOT) != r[0x17A]:
        report("sees", f"slot: original {e.r8(SEES_SLOT)} record {r[0x17A]}")

# 2. WeaponStatusText 0x56F4C
WEAPON_EDGES = ((0, 0, 0), (0, 0xFF, 0xFF), (1, 0, 10), (1, 0, 9), (1, 0xFF, 10), (1, 1, 10),
                (0x10, 1, 10), (0x11, 1, 10), (0xFF, 0x80, 0x80))
for i in range(N):
    r = rec_for_case()
    if i < len(WEAPON_EDGES):
        r[0], r[1], r[2] = WEAPON_EDGES[i]
    how, near = rng.randrange(256), rng.randrange(256)
    e.uc.mem_write(REC, bytes(r))
    log.clear()
    e.call(rt(0x56F4C), REC, how, near)
    got, want = e.cstr(TEXT_BUF), p_weapon(r)
    if got != want:
        report("weapon", f"item {r[0]} count {s8(r[1])} cond {s8(r[2])}: original {got[:24]!r} port {want!r}")
    exp = [(REC, how, near, want, 1)]
    if log != exp:
        report("weapon", f"call: original {log[-1]!r} port {exp!r}")

# 3. MsgEnemiesNearby 0x1B0EC
e.w8(ENEMIES_FLAG, 0)
for _ in range(max(1, N // 20)):
    log.clear()
    e.call(rt(0x1B0EC))
    text, ms = p_nearby()
    exp = [(text, 0, 0, 1, 1, 0, ms, 1)]
    if log != exp:
        report("nearby", f"message: original {log[-1]!r} port {exp!r}")
    if e.r8(ENEMIES_FLAG) != 1:
        report("nearby", f"flag 0x22A0CF = {e.r8(ENEMIES_FLAG)}, expected 1")

# 4. MsgStrInt 0x27CE0 with GuardsArriveMsg's text
STR_NUMS = (0, 1, -1, (1 << 31) - 1, -(1 << 31), 10, -10, 123456789)
GUARD_TEXT = b"1 Guard arrives in sector "
e.uc.mem_write(TB, GUARD_TEXT + b"\0")
for i in range(N):
    number = STR_NUMS[i] if i < len(STR_NUMS) else rng.randrange(-(1 << 31), 1 << 31)
    log.clear()
    e.call(rt(0x27CE0), TB, number & 0xFFFFFFFF)
    want, ms = p_guards(number)
    exp = (want, 0, 0, 1, 1, 1, ms, 1)
    if not log or log[0] != exp:
        report("str_int", f"number {number}: original {log[-1] if log else None!r} port {exp!r}")

# 5. FUN_2F7A4 0x2F7A4
for _ in range(max(1, N // 20)):
    e.uc.mem_write(REACT, bytes([0xAA, 0xAA, 0xAA, 0xAA]))
    e.call(rt(0x2F7A4))
    got = bytes(e.uc.mem_read(REACT, 4))
    _, want = p_head(bytes(SOLD), 0, 0, 0, 0xFF, bytes(0x12))
    if got != want:
        report("reset", f"original {got.hex()} port {want.hex()}")

# 6. FUN_56D3C 0x56D3C
# (item type, count, condition, how, effect byte, table entry 0)
REACTION_EDGES = ((1, 1, 10, 0, 0, 1), (1, 1, 10, 0, 0, 0), (1, 0, 10, 0, 0, 0), (1, 0xFF, 10, 0, 0, 0),
                  (1, 1, 9, 0, 0, 0), (0, 0, 0, 0, 0, 0), (1, 0, 10, 5, 0, 0), (1, 0, 10, 4, 0, 0),
                  (1, 0, 10, 0, 0x80, 1), (1, 0, 10, 0, 0, 0xFF), (1, 0, 10, 0, 0, 0x80))
for i in range(N):
    r = rec_for_case()
    slot = rng.randrange(48)
    fx = rng.choice([rng.randrange(0, 16), rng.randrange(0x80, 0x100)])
    table = bytes(rng.randrange(256) for _ in range(0x12))
    how, near = rng.randrange(256), rng.randrange(256)
    if i < len(REACTION_EDGES):
        r[0], r[1], r[2], how, fx, t0 = REACTION_EDGES[i]
        table = bytes([t0]) + table[1:]
    e.uc.mem_write(REC, bytes(r))
    e.w32(SOLD_PTRS + 4 * slot, REC)
    e.uc.mem_write(REACT, bytes([slot, how, near, fx]))
    e.uc.mem_write(FX_TABLE + 0x12 * (fx & 0x7F), table)
    log.clear()
    e.call(rt(0x56D3C))
    pcall, pafter = p_head(r, slot, how, near, fx, table)
    after = bytes(e.uc.mem_read(REACT, 4))
    ocall = (log[0][3], slot, log[0][1], log[0][2]) if log else None
    if ocall != pcall:
        report("reaction", f"fx {fx} entry {s8(table[0])} slot {slot} how {how} near {near} item {r[0]} "
                           f"count {s8(r[1])} cond {s8(r[2])}: original {ocall!r} port {pcall!r}")
    if log and (log[0][0] != REC or log[0][4] != 1 or len(log) != 1):
        report("reaction", f"call arguments: {log!r}")
    if after != pafter:
        report("reaction", f"bytes after: original {after.hex()} port {pafter.hex()}")

# 7. MsgHearsNoise? 0x72A60 (a merc hears a noise on a tile with no soldier)
hear_log, hear_dir = [], [1]
e.stub(rt(0x27D24), lambda em, eax, *_: hear_log.append(em.cstr(eax)) or 0, 0)
e.stub(rt(0x52560), lambda em, *_: hear_dir[0], 0)
L0 = rt(0xFA958)                         # g_soldiers: merc 0 with its real state
saved_rec = bytes(e.uc.mem_read(L0, SOLD))
seen_hear = set()
NAMES = (b"Ivan\0", b"Grunty\0", b"Night\0", b"Thunder\0", b"Spike\0")
for i in range(N):
    strength, kind, d = rng.randrange(0, 13), rng.choice((0, 1, 2, 3, 7)), rng.randrange(0, 9)
    nick = rng.choice(NAMES)
    hear_dir[0] = d
    e.uc.mem_write(L0, saved_rec)
    e.uc.mem_write(L0 + 0x16E, nick)
    tile = rng.randrange(0x100, 0xB00)
    hear_log.clear()
    e.call(rt(0x72A60), L0, tile, strength, kind)
    r = bytearray(saved_rec)
    r[0x16E:0x16E + len(nick)] = nick
    level = 0 if strength < 3 else 1 if strength < 6 else 2 if strength < 10 else 3
    want = port_text(PORT.ja1f_hears_text(bytes(r), level, kind, d, TXT, 256))
    if not hear_log:
        continue
    seen_hear.add((level, kind, d))
    if hear_log[0] != want:
        report("hears", f"strength {strength} kind {kind} dir {d}: original {hear_log[0]!r} port {want!r}")
e.uc.mem_write(L0, saved_rec)
if len(seen_hear) < 4 * 5 * 9 // 2:                 # (levels x kinds x directions: half of them at least)
    report("hears", f"only {len(seen_hear)} level/kind/direction cases reached the message")

# 8. The texts of the tables, read from the image
def image_text(addr):
    return e.cstr(rt(addr))


for t, addr in enumerate((0x985A7, 0x9A035, 0x98697, 0x9A01B, 0x9A001, 0x9861F)):
    want = image_text(addr) + (b" 47" if t == 0 else b"47")    # (text 0: format "%s %d" 0x909FC; the others end in a space)
    got = port_text(PORT.ja1f_battle_text(t, 47, TXT, 256))
    if got != want:
        report("tables", f"battle text {t}: original {want!r} port {got!r}")
for kind, addr, colour in ((0, 0x9AB2D, 0x72), (1, 0x9AB47, 0xA2), (2, 0x9AB61, 0x72), (3, 0x9AB7B, 0xA2), (4, 0x9AC7F, 0x72)):
    col = ctypes.c_int32()
    n = PORT.ja1f_banner(kind, TXT, 256, ctypes.byref(col))
    got = port_text(n) if n >= 0 else None
    if got != image_text(addr) or col.value != colour:
        report("tables", f"banner {kind}: original {image_text(addr)!r} {colour:#x} port {got!r} {col.value:#x}")
if PORT.ja1f_banner(5, TXT, 256, ctypes.byref(col)) != -1:
    report("tables", "banner 5 has a text")
image = bytes(e.uc.mem_read(rt(0x90000), 0xCB37))
for k in range(0, 21):
    n = PORT.ja1f_message_text(k, TXT, 256)
    if k in (0, 5, 19, 20):
        if n != -1:
            report("tables", f"message {k} has a text")
    elif n < 0 or b"\0" + port_text(n) + b"\0" not in image:
        report("tables", f"message {k}: no such text in JA.EXE ({n})")

bad = sum(fails.values())
print(f"msgtext_diff: {N} x 6 sections, hears cases {len(seen_hear)}, " + " ".join(f"{k} {v}" for k, v in fails.items()))
sys.exit(1 if bad else 0)
