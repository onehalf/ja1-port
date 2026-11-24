"""Differential test: the key part of TacticalKeyHandler 0x13490, original vs port (port/ja1front tactkeys.rs).

usage: .venv/bin/python validate/diff/keyhandler_diff.py [ncases=6000] [DUMP] [--port] [--lib SO]
The original runs in Unicorn from its entry with KeyPressed 0x5616C and GetKey 0x560F4 scripted, every callee of the
key part stubbed and logged, and the common tail 0x1409B as the end of a run. A case: fuzzed gates (selected, enemies
near, screen drawn, turn shown, view hold, pause, compression, cursor item, debug level, save-anywhere, view mode,
QUICK.SAV, in play, hp) and a key: every plain byte and every extended byte in turn, then random ones. The port's
route for the same bytes and gates must expect the same calls, the same number of GetKey calls, and the same writes:
the marks 0x229C94 / 0x229C8C, the debug level 0x955EC, the arrow direction 0x284F17 and turn shown 0x92DCA.
A second part runs 0x1409B..0x141A8 from its own entry: the Alt, Ctrl and Shift latches and the hand cursor of Ctrl.
Without --port only the original runs. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, RET_SENTINEL    # noqa: E402
from unicorn import UC_HOOK_CODE  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESI, UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 6000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_tactical_key.argtypes = [ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p]
    PORT.ja1f_tactical_key.restype = None

NAMES = {0x1002C: "Pause", 0x104C8: "StopWalk", 0x11448: "SeesMsg", 0x1148C: "SelectMerc", 0x11D40: "TimeCompress",
         0x11DC0: "AskAbort", 0x11E2C: "TurnSightings", 0x121D8: "EndTurn", 0x126C8: "NewTurn", 0x12D58: "NextTurn",
         0x1315C: "NextMerc", 0x1328C: "Map", 0x176C8: "Redraw", 0x197D8: "SnapToTile", 0x1CBA8: "Traverse",
         0x1D950: "TilePopup", 0x1EE84: "AtWaypoint", 0x1F2C4: "AnimStep", 0x23760: "TileObject", 0x23944: "TileOccupant",
         0x248E4: "SetView", 0x27CE0: "MsgStrInt", 0x27D24: "Status", 0x29D54: "SayAgain", 0x2C898: "QuickRestore",
         0x2D880: "QuickSave", 0x32754: "Rest", 0x331A4: "RedrawSlot", 0x38084: "AskQuit", 0x392FC: "TakeDamage",
         0x3ACB8: "BreakObject", 0x3D98C: "EndTask", 0x3DDB8: "ClearTask", 0x3F724: "FocusOn", 0x3F7C8: "NextEnemy",
         0x3F8A4: "PlaceAt", 0x3F994: "Toggle", 0x40024: "StatPopups", 0x40568: "Swap", 0x4B730: "Lock", 0x4B764: "Unlock",
         0x4B788: "Options", 0x53660: "Back", 0x54D50: "Inventory", 0x56210: "MaxAP", 0x56ED0: "ClearTarget",
         0x5CE94: "SoundVol", 0x5CEB4: "MusicVol", 0x6BC28: "DbgFlags", 0x6CF88: "DbgOpp"}
TILE_VIEW, TILE_OVER = 777, 888      # the answers of ViewPixelToTile? and OverviewTileAtMouse

e = Emu(dump=dump, fast=True)
rt = e.rt
log, keys, getkeys, file_there, occupant = [], [], [0], [0], [0]


def logger(name):
    def f(em, a, d, b, c):
        log.append((name, a, d, b))
        if name == "TileOccupant":
            return occupant[0]
        return 0
    return f


for addr, name in NAMES.items():
    e.stub(rt(addr), logger(name))
e.stub(rt(0x678E8), lambda em, *a: 0)                    # MousePoll?
e.stub(rt(0x15E50), lambda em, *a: 10)                   # TileToX
e.stub(rt(0x15E6C), lambda em, *a: 10)                   # TileToY
e.stub(rt(0x159CC), lambda em, *a: TILE_VIEW)
e.stub(rt(0x24488), lambda em, *a: TILE_OVER)
e.stub(rt(0x4707C), lambda em, *a: file_there[0])        # FileExists?
e.stub(rt(0x5616C), lambda em, *a: 1)                    # KeyPressed


def get_key(em, *a):
    getkeys[0] += 1
    return keys.pop(0) if keys else 0x7F     # a read past the key bytes: seen by the GetKey count


def tail(em, *a):                            # 0x1409B: the run ends, whatever the frame holds
    em.w32(em.uc.reg_read(UC_X86_REG_ESP), RET_SENTINEL)
    return 0


e.stub(rt(0x560F4), get_key)
e.stub(rt(0x1409B), tail)

SELECTED, COMBAT, DRAWN, SHOWN, HOLD = 0x284F11, 0x92DCB, 0x92DCC, 0x92DCA, 0x92DCE
PAUSED, COMPRESS, HELD, LEVEL, ANYWHERE, MODE = 0x22A129, 0x9300C, 0x222B50, 0x955EC, 0x92DC3, 0x92DDF
SPOTTED, VIEW_TILE, ARROW, SEES = 0x229C94, 0x229C8C, 0x284F17, 0x92DC6
TABLE = 0x100958
ROUTES = {0: "None", 1: "EndPause", 2: "EndCompress", 3: "AskQuit", 4: "QuickSave", 5: "QuickRestore", 6: "Debug", 7: "Merc",
          8: "SeesLine", 9: "Options", 10: "Arrow", 11: "View", 12: "ForceTurn", 13: "NextEnemy", 14: "Escape", 15: "NextMerc",
          16: "Volume", 17: "Abort", 18: "Compress", 19: "NoCompress", 20: "EndTurn", 21: "CursorBack", 22: "Inventory",
          23: "SayAgain", 24: "Map", 25: "Pause", 26: "StatPopups", 27: "Traverse", 28: "Version", 29: "Swap", 30: "Popup"}
# route -> the calls that the original must log (names only; the arguments are checked below)
PLAIN = {3: ["AskQuit"], 4: ["QuickSave"], 5: ["QuickRestore"], 9: ["Back", "Options"], 12: ["NewTurn"],
         13: ["NextEnemy"], 15: ["NextMerc"], 17: ["Back", "AskAbort"], 18: ["TimeCompress"], 19: ["Status"],
         20: ["Back", "EndTurn"], 21: ["Back"], 22: ["Inventory"], 23: ["SayAgain"], 24: ["Map"], 25: ["Pause"],
         26: ["Lock", "StatPopups", "Unlock"], 27: ["Back", "Traverse"], 28: ["Status"], 29: ["Swap"], 30: ["TilePopup"],
         1: ["Pause"], 2: ["TimeCompress"], 7: ["SelectMerc"], 0: []}
INV_POINTER = 0x67BF0
e.stub(rt(INV_POINTER), lambda em, *a: 0)
DEBUG_FIRST = {0x110: "Toggle", 0x112: "Toggle", 0x126: "Rest", 15: "TileOccupant", 16: "FocusOn", 20: "PlaceAt",
               ord("@"): "TileObject", ord("E"): "MsgStrInt", ord("F"): "TileObject", ord("G"): "MsgStrInt",
               ord("K"): "TileOccupant"}
rng = random.Random(int(os.environ.get("KEYHANDLER_SEED", "1")))
fails, seen, kinds = 0, set(), {}
keylist = [(b, 0) for b in range(1, 256)] + [(0, b) for b in range(256)]


def bit(p=0.5):
    return int(rng.random() < p)


for i in range(ncases):
    first, second = keylist[i % len(keylist)] if rng.random() < 0.6 else rng.choice(keylist)
    if i % 3 == 0:     # the keys with arms, more often under each gate
        first, second = rng.choice([(c, 0) for c in b"\x01\x02\x04\x06\x09\x0c\x0d\x0f\x10\x12\x14\x1b +,-.@AaCcDdEFGIiKLMOPSTVXZ"]
                                   + [(0, s) for s in (0x10, 0x12, 0x13, 0x1F, 0x26, 0x2D, 0x43, 0x44, 0x48, 0x49, 0x4B, 0x4D, 0x50, 0x51, 0x52, 0x5E)])
    g = dict(selected=rng.choice([0, 1, 3, 7, 8, 9, 20, 0x2F]), combat=bit(), drawn=bit(0.7), shown=bit(0.7), hold=bit(0.3),
             paused=bit(0.08), compress=bit(0.08), held=bit(0.3), level=rng.choice([0, 1, 2, 3, 4, 4, 4]),
             anywhere=bit(0.3), mode=rng.choice([0, 1, 2, 3]), quick=bit(), in_play=bit(0.8),
             hp=rng.choice([0, 14, 15, 16, 60, 0x80, 0xFF]))
    s = e.r32(rt(TABLE) + 4 * g["selected"])
    e.w8(rt(SELECTED), g["selected"])
    for a, v in ((COMBAT, g["combat"]), (DRAWN, g["drawn"]), (SHOWN, g["shown"]), (HOLD, g["hold"]), (PAUSED, g["paused"]),
                 (COMPRESS, g["compress"]), (HELD, g["held"]), (LEVEL, g["level"]), (ANYWHERE, g["anywhere"]), (MODE, g["mode"]),
                 (SEES, rng.choice([0x40, 3])), (ARROW, 0)):
        e.w8(rt(a), v)
    e.w8(s + 0x138, g["in_play"])
    e.w8(s + 0x183, g["hp"])
    e.w32(rt(SPOTTED), 5)
    e.w32(rt(VIEW_TILE), 1000)
    file_there[0] = g["quick"]
    occupant[0] = rng.choice([1, 0x41])
    log.clear()
    keys[:] = [first, second] if first == 0 else [first]
    getkeys[0] = 0
    sees = e.r8(rt(SEES))
    e.call(rt(0x13490))
    names = [c[0] for c in log]
    one = log[0] if log else ("", 0, 0, 0)
    if not PORT:
        continue
    gates = (ctypes.c_int32 * 14)(g["selected"], g["combat"], g["drawn"], g["shown"], g["hold"], g["paused"], g["compress"],
                                  g["held"], g["level"], g["anywhere"], g["mode"], g["quick"], g["in_play"],
                                  g["hp"] - 256 if g["hp"] >= 128 else g["hp"])
    o = (ctypes.c_int32 * 5)()
    PORT.ja1f_tactical_key(first, second, ctypes.addressof(gates), ctypes.addressof(o))
    r, arg, back, marks, level = o
    kinds[ROUTES[r]] = kinds.get(ROUTES[r], 0) + 1
    bad = []
    tile = TILE_VIEW if g["mode"] else TILE_OVER
    if r in PLAIN and names != PLAIN[r]:
        bad.append(f"calls {names}")
    if ("Back" in names) != bool(back):
        bad.append(f"back {back}")
    if getkeys[0] != (2 if first == 0 else 1):
        bad.append(f"GetKey count {getkeys[0]}")
    if (e.r32(rt(SPOTTED)), e.r32(rt(VIEW_TILE))) != ((0x40, 0xC41) if marks else (5, 1000)):
        bad.append(f"marks {marks}")
    if e.r8(rt(LEVEL)) != level:
        bad.append(f"level {e.r8(rt(LEVEL))} port {level}")
    if r == 1 and one[1] & 0xFF != 0 or r == 2 and one[1] & 0xFF != 0:
        bad.append("end argument")
    if r == 18 and one[1] & 0xFF != 1:
        bad.append("compress argument")
    if r == 25 and one[1] & 0xFF != 1:
        bad.append("pause argument")
    if r == 7 and (one[1] & 0xFF != arg or one[3] & 0xFF != 1):
        bad.append(f"merc {one} port {arg}")
    if r in (29, 22) and one[1] != s:
        bad.append("soldier argument")
    if r == 29 and one[2] != tile:
        bad.append(f"swap tile {one[2]} want {tile}")
    if r == 30 and (one[1] != tile or one[2] & 0xFF != 1):
        bad.append(f"popup {one} want {tile}")
    if r == 12 and (one[1] & 0xFF != 1 or e.r8(rt(SHOWN)) != 1):
        bad.append("force turn")
    if r == 19 or r == 28:
        text = e.cstr(one[1]) if log else b""
        if (r == 19) != text.startswith(b"Can't compress") or (r == 28) != text.startswith(b"Release"):
            bad.append(f"status text {text!r}")
    if r == 8:
        want = ["SeesMsg"] if sees == 0x40 else []
        if names != want or (sees != 0x40 and e.r8(rt(SEES)) != 0x40):
            bad.append(f"sees {names} latch {sees}")
    if r == 10:
        if names or e.r8(rt(ARROW)) != {0x48: 1, 0x4B: 7, 0x4D: 3, 0x50: 5}[arg]:
            bad.append(f"arrow {names} {e.r8(rt(ARROW))}")
    elif e.r8(rt(ARROW)):
        bad.append(f"arrow byte {e.r8(rt(ARROW))}")
    if r == 11 and set(names) - {"SetView"}:
        bad.append(f"view {names}")
    if r == 14 and names[:1] != ["ClearTarget"]:
        bad.append(f"escape {names}")
    if r == 16 and names != [{ord("+"): "MusicVol", ord("-"): "MusicVol", ord("."): "SoundVol", ord(","): "SoundVol"}[arg]]:
        bad.append(f"volume {names}")
    if r == 6:
        if arg in (1, 2, 4, 12):
            if names:
                bad.append(f"level key {names}")
        elif arg == 0x15E:
            if e.r8(rt(ANYWHERE)) != g["anywhere"] ^ 1:
                bad.append("save-anywhere byte")
        elif arg != 18 and names[:1] != [DEBUG_FIRST[arg]]:
            bad.append(f"debug {arg:#x} {names}")
    elif r != 8 and "Toggle" in names:
        bad.append("debug arm ran")
    seen.add(r)
    if bad:
        fails += 1
        if fails <= 12:
            print(f"MISMATCH case {i}: key ({first:#x},{second:#x}) gates {g} route {ROUTES[r]} {arg:#x}: {bad}")
# The key flags part 0x1409B..0x141A8: Alt, Ctrl and Shift latches, the mode kept while Ctrl holds the hand cursor.
FLAGS, L_ALT, L_CTRL, L_SHIFT, SAVED, CMODE, CTILE = 0x22A46C, 0x284F20, 0x284F1F, 0x284F21, 0x284F22, 0x284F1B, 0x229D2C
fe = Emu(dump=dump, fast=True)
frt = fe.rt
fcalls = []
for addr, name in ((0x1C464, "Move"), (0x1C438, "Set")):
    def stub(em, *a, name=name):
        fcalls.append((name, a[0], a[1] & 0xFF))
        return 0
    fe.stub(frt(addr), stub)


def flags_end(em, *a):
    em.w32(em.uc.reg_read(UC_X86_REG_ESP), RET_SENTINEL)
    return 0


fe.stub(frt(0x141A8), flags_end)
SOL = 0x1234
fe.uc.hook_add(UC_HOOK_CODE, lambda uc, *_: uc.reg_write(UC_X86_REG_ESI, SOL), begin=frt(0x1409B), end=frt(0x1409B))
flag_seen = set()
flag_cases = 3000
for i in range(flag_cases):
    flags = random.choice((0, 1, 2, 3, 4, 8, 0x100, 0x200, 0x104, 0x208, 5, 9, 12, 0x20, random.randrange(0x10000)))
    st = [random.randrange(2), random.randrange(2), random.randrange(2), random.randrange(4), random.randrange(4), 100 + i]
    fe.w16(frt(FLAGS), flags)
    for a, v in ((L_ALT, st[0]), (L_CTRL, st[1]), (L_SHIFT, st[2]), (SAVED, st[3]), (CMODE, st[4])):
        fe.w8(frt(a), v)
    fe.w32(frt(CTILE), st[5])
    fcalls.clear()
    fe.call(frt(0x1409B))
    got = [fe.r8(frt(a)) for a in (L_ALT, L_CTRL, L_SHIFT, SAVED, CMODE)] + [fe.r32(frt(CTILE))]
    orig = 0
    if fcalls:
        name, sol, arg = fcalls[0]
        orig = 1 if name == "Move" else 2 + arg
        if sol != SOL or len(fcalls) != 1:
            fails += 1
            print(f"MISMATCH flags case {i}: soldier or call count {fcalls}")
    flag_seen.add(orig)
    if PORT:
        io = (ctypes.c_int32 * 6)(*st)
        r = PORT.ja1f_key_flags(flags, io)
        if r != orig or list(io) != got:
            fails += 1
            if fails <= 12:
                print(f"MISMATCH flags case {i}: flags {flags:#x} from {st}: original {orig} {got} port {r} {list(io)}")
if not {0, 1, 2, 3} <= flag_seen:
    fails += 1
    print("key flags: a call kind never ran", flag_seen)
print(f"key flags: {flag_cases} cases, call kinds {sorted(flag_seen)}")
if PORT:
    missing = sorted(set(ROUTES) - seen)
    if missing:
        fails += 1
        print("route never taken:", [ROUTES[m] for m in missing])
print(f"keyhandler_diff: {ncases} cases, {len(seen)} routes{'' if PORT else ' (original only)'}, {fails} mismatches")
sys.exit(1 if fails else 0)
