"""Walk test: the clean-room port against the original, frame by frame (soldier_update.md Move order, One frame,
Timer tick).

Part of ./t check.
usage: .venv/bin/python validate/diff/walk_diff.py [N=20] [DUMP] [--frames F] [--orig] [--combat | --shoot | --enemy | --cross | --water | --mine]
Each case takes the tactical snapshot (default build/mem_tac60.bin), gives one soldier a move order to a tile
near it (MoveMercTo 0x1C608 / ja1_move_order), then runs F frames: 1..4 timer ticks (the 10 ms callback 0x5EE98 /
ja1_tick) and one soldier loop (TacticalLoop 0x10C5B..0x10F4E / ja1_frame). The original runs the game code for
every op the port's library implements (port_api.md "Library ops"); every other op is a host call in the port and
a stub in the original (logged, answered 0). Random is scripted the same on both sides. After each frame the
whole world (port_lib world fields) and the ordered call log are compared. --orig: run the original side only and
print where the soldier walks (harness check). --combat: turn-based combat (turns.md): combat, screen_drawn and
turn_shown are set before the move order, and End turn (EndTurn 0x121D8 / ja1_end_turn) runs after half the frames,
so the walk spends AP and the turn goes round (enemies' turn, new round, start of turn). --shoot (default dump
build/mem_combat1.bin, turn-based combat): a merc with a gun stands 2..5 tiles from an enemy and the ORIGINAL
TacticalClick 0x59C14 aims it at the enemy (not compared: the start state of both sides); then F frames (default
60) on both sides: the attack animation, start attack and attack go (host ops 79 / 80), launch, the projectile
steps, the impact and the damage (shooting.md). The original runs the attack routines (SHOOT_REAL) for real; the
port's host answers the same ops by running those original routines on the port's world (store, call, load: the
host is the original game there). --enemy (default dump build/mem_combat1.bin, turn-based combat): End turn runs
before frame 0, then F frames (default 200) of the enemies' turn: the AI thinks, walks, aims and shoots (ai.md,
shooting.md); the ops of SHOOT_REAL are answered as in --shoot. --fight (default dump build/mem_combat1.bin): a
whole fight, round after round: in the mercs' turn each merc with a loaded gun shoots at the nearest enemy (a click,
then 60 frames), then End turn and the enemies' turn (the AI) until a merc is selected again; it stops 40 frames
after one side has nobody left, or after --rounds R (default 8); --to-end: exit 1 when a fight did not end. An empty
gun is loaded again (the player's reload), a merc more than 8 tiles from its enemy walks a third of the way (item
0x4D, the confirm-move option off), and when a new round cannot select the merc to show (it died) the driver selects
a living merc (op 120, 0x1148C, as a portrait click). A merc without a gun in the main hand and a loaded gun in the
second hand swaps the hands; with no gun left the mercs walk toward the nearest enemy. The original decides these actions; the port replays the same
actions on the same frames. --cross: a second merc gets a move order to the same target or a tile next to it, in
the same frame, so one walk meets the other (walk blocked, op 98: wait, path round, or stop short of the target;
replay_week saw a live walk stop at 568 for 509); the attack routines run as in --combat. --water: the walker often starts on a free
water tile (deep water mostly; VacateTile, OccupyTile and ReplaceSoldier of the original put it there, because a click
path never enters deep water 0x97+) and walks to land, to more water or to shallow water next to it; its energy may
start at 0..5 (drowning, Drown? 0x3843C: a step into water with energy 0 while a swim animation runs; a swimmer in deep
water swims to deep water with energy 1..3, sometimes with health 15..20 and state +0x19C 5, so Drown? hurts and kills).
Deep water soaks items (SoakItems 0x407B8); the summary counts the walks in which each routine ran in the original. --mine:
mines (ground items 0x13 / 0x2C) lie on tiles near the walk and the walker's mine skill +0x192 is fuzzed: a merc whose
skill reaches the mine's power stops before it and flags it (step start, MineFlag 0x19D74); the walker may be
forgetful (+0x19C 2: it forgets where it was going and stops after many steps; with --combat it gets 40..127 AP, the
stop takes Random(AP) + 1). Exits 1 on any mismatch.
"""
import collections
import array, ast, hashlib, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
import orig_cache                    # noqa: E402
port_lib.CLOCK = True                # turns.md The day clock: its world fields (op 159 runs on both sides)
from emu import Emu, EmuStop         # noqa: E402

argv = sys.argv[1:]
ORIG = "--orig" in argv
COMBAT = "--combat" in argv
SHOOT = "--shoot" in argv
ENEMY = "--enemy" in argv
FIGHT = "--fight" in argv
WATER = "--water" in argv   # the move target is a water tile; the walker may start tired (energy 0: drowning)
MINE = "--mine" in argv   # mines near the walk, the mine skill fuzzed (step start: a seen mine stops the walk)
CROSS = "--cross" in argv   # a second merc gets an order to the target or a tile next to it (a walk blocked by a walker)
ROUNDS = int(argv[argv.index("--rounds") + 1]) if "--rounds" in argv else 8
ENEMY = ENEMY or FIGHT
REAL = SHOOT or ENEMY             # the attack routines run for real (SHOOT_REAL)
ATTACK = REAL or "--combat" in argv or "--cross" in argv or MINE or WATER   # (--water: Drown? damage, TakeDamage real) --combat too: the enemies' turn after End turn attacks
    # (JamCheck 0x59A54 is inline in the port; a stub that answers 0 jammed every enemy gun); --cross too: its long
    # real-time runs reach enemy shots (t12: enemy 40 fires in frame 18)
FRAMES = int(argv[argv.index("--frames") + 1]) if "--frames" in argv else 60 if SHOOT else 6000 if FIGHT else 200 if ENEMY else 120
argv = [a for i, a in enumerate(argv) if a not in ("--orig", "--combat", "--shoot", "--enemy", "--fight", "--to-end", "--cross", "--water", "--mine") and a not in
        ("--frames", "--rounds") and (i == 0 or argv[i - 1] not in ("--frames", "--rounds"))]
n = int(([a for a in argv if a.isdigit()] or ["20"])[0])
dump = ([a for a in argv if not a.isdigit()] or [os.path.join(HERE, "..", "..", "build",
                                                               "mem_combat1.bin" if REAL else "mem_tac60.bin")])[0]
# --shoot: the attack routines the original runs for real; the port's host answers these ops with them
SHOOT_REAL = {"click", "w_58574", "attack_ready", "w_59a54", "reach_ap", "take_damage", "hit_location",
              "bullet_damage"}

# CALLEES (name -> (address, argument kinds)) and POP from ai_diff.py, without running it
_src = ast.parse(open(os.path.join(HERE, "ai_diff.py")).read())
_ns = {"ai": ai}
for node in _src.body:
    if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name) and node.targets[0].id in ("CALLEES", "POP", "SEL_STUBS"):
        exec(compile(ast.Module([node], []), "ai_diff.py", "exec"), _ns)
CALLEES, POP = _ns["CALLEES"], _ns["POP"]
CALLEES.update(_ns["SEL_STUBS"])      # (select.md UI calls 0x46A4C, 0x24880, 0x11448, 0x67C54: stubbed here too)
# turns.md The day clock (op 159, DayClock 0x1F724 real on the original side) and Time compression: their host ops,
# stubbed as in clock_diff (every answer 0 on both sides); guards_msg 0x27CE0 has its own stub below
CALLEES.update({"day_bar": (0x3DB3C, "b"), "enemy_pick": (0x3F45C, ""), "enemy_move": (0x3EDE8, "b"),
                "guard_enter": (0x30600, "b"), "mode_banner": (0x23B4C, ""), "mouse_release": (0x67C68, ""),
                "refresh": (0x1CB78, "")})

# library ops (port_api.md): the original runs its own code for them
LIB_OPS = {67, 69, 73, 118, 74, 75, 108, 176, 150, 340, 341, 342, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 17, 18, 20, 21, 22, 23, 24, 25, 26, 29, 33, 40, 41, 42, 44, 45, 51,
           52, 53, 56, 58, 61, 62, 83, 84, 97, 98, 106, 109, 111, 117, 119, 122, 137, 138,
           31, 113, 114, 131, 132, 133, 134, 140, 141, 152,   # (turns.md)
           151, 191, 187,                                     # (combat.md First aid; SectorVictory? 0x3A0B0)
           161, 90, 160, 50, 13, 28, 121, 16, 27,             # (combat.md Blast, Explosion, End of a task; ai.md Walking cost, Turning; Use cost, Swap hands, Soldier busy; turns.md Sector tick)
           39, 81, 136, 110, 173, 175, 180, 182, 78, 82, 77,  # (shooting.md; 77 = MeleeHit 0x3521C)
           46, 48, 79, 80,                                    # (shooting.md batch 5)
           14, 15, 30, 68,                                    # (click.md)
           123,                                               # (turns.md Turn start)
           120,                                               # (select.md Select merc)
           70, 104, 107, 179,                                 # (combat.md Doors)
           190, 102,                                          # (combat.md Reload; op 102 = the same procedure)
           127, 211, 212,                                     # (ai.md Make noise, Hearing a noise, Noise interrupt)
           49,                                                # (ai.md Sight line)
           159,                                               # (turns.md The day clock)
           35, 34, 115, 63, 64, 327,                          # (ai.md Sightings as steps)
           336,                                               # (combat.md Craters and blast leftovers; file stubs below)
           128, 116}                                          # (combat.md Interrupt contests)
HOST_OPS = port_lib.ALL_OPS
for op_ in LIB_OPS:
    HOST_OPS &= ~(1 << min(op_, 63))
LIB_NAMES = {port_lib.OPS[op_] for op_ in LIB_OPS}

e = Emu(dump=dump, fast=True)
rt = e.rt
if os.environ.get("WALK_WATCH"):    # (debug: WALK_WATCH=ADDR prints each original-code write to that byte)
    from unicorn import UC_HOOK_MEM_WRITE
    from unicorn.x86_const import UC_X86_REG_EIP, UC_X86_REG_ESP
    _wa = rt(int(os.environ["WALK_WATCH"], 0))

    def _watch(uc, acc, addr, size, value, _):
        eip = uc.reg_read(UC_X86_REG_EIP)
        sp_ = uc.reg_read(UC_X86_REG_ESP)
        print(f"  watch write {value:#x} eip {eip:#x} stack {[hex(e.r32(sp_ + 4 * k_)) for k_ in range(6)]}")
    e.uc.hook_add(UC_HOOK_MEM_WRITE, _watch, None, _wa, _wa)
G_SOLDIERS = 0xFA958
SP = {rt(G_SOLDIERS) + 0x200 * k: k for k in range(0x30)}
log, script = [], {}


def conv(kinds, args):
    out = []
    for k, a in zip(kinds, args):
        if k in "xm":                    # not compared (a table address / a text pointer)
            continue
        if k == "p":                     # puff look: the original passes its table (shooting.md: 0 splash, 1 puff)
            out.append({rt(0x95670): 0, rt(0x9566B): 1, rt(0x22A4D0): 2, rt(0x229E18): 3}.get(a, a))   # (2 / 3: combat.md Explode)
            continue
        out.append(SP.get(a, a) if k == "s" else ai.s16(a) if k == "t" else a & 0xFF if k == "b" else
                   ai._i32(a) if k == "n" else a)
    return tuple(out)


class TextWorld:
    """ai.hears_text on the port world: soldier names from its records, the text into its text_buf (rest dropped)"""
    def __init__(self, bufs):
        self.bufs = bufs

    def g8(self, a):
        o = a - G_SOLDIERS
        return self.bufs["soldiers"][o] if 0 <= o < len(self.bufs["soldiers"]) else e.r8(rt(a))

    def s8(self, a, v):
        o = a - ai.G_TEXT_BUF
        if 0 <= o < len(self.bufs["text_buf"]):
            self.bufs["text_buf"][o] = v & 0xFF


def make_stub(name, kinds):
    def fn(emu, eax, edx, ebx, ecx):
        sp = emu.uc.reg_read(port_lib_regs.ESP)
        stack = [emu.r32(sp + 4 + 4 * i) for i in range(max(len(kinds) - 4, 0))]
        log.append((name, conv(kinds, [eax, edx, ebx, ecx] + stack)))
        return 0
    return fn


class port_lib_regs:
    from unicorn.x86_const import UC_X86_REG_ESP as ESP


rnd = collections.deque()


def stub_random(emu, eax, *_):
    k = ai.s16(eax)
    log.append(("Random", (k,)))
    v = next_rnd()
    return v % k if k > 0 else 0


rnd_more = [0]


def next_rnd():
    """the next scripted Random value. After the list ends, a fixed sequence follows (the same on both sides): a
    constant 0 made the original loop forever in CalmAction 0x6DDC4 (new facing = Random + 1 until it differs)."""
    if rnd:
        return rnd.popleft()
    rnd_more[0] += 1
    return (rnd_more[0] * 2654435761 >> 9) & 0xFFFF


def set_rnd(vals):
    rnd.clear()
    rnd.extend(vals)
    rnd_more[0] = 0


for name, (addr, kinds) in CALLEES.items():
    if name not in LIB_NAMES and name not in ("frame_active", "frame_other", "path_ap", "find_path", "merc_quote",
                                              "w_1f6e4", "w_197d8", "w_3a2d4",   # (inline in the port: Merc busy, tile point stop, Enemy died)
                                              "rest",                    # (Rest: a library procedure, turns.md)
                                              "w_56dd4", "impact", "use_throw", "drop_item", "w_5aa08",   # (shooting.md procedures)
                                              "w_74304", "react",
                                              "put_item",
                                              "w_32828",                 # (Pick night merc, turns.md The day clock: inline in the port)
                                              "w_3fc04",                 # (IsListedItem 0x3FC04: inline in the port; --mine blasts reach it)
                                              "fx_157bc",                # (EffectStart 0x157BC = op 91, a library op: a mine's timed effect)
                                              ) and not (ATTACK and name in SHOOT_REAL):
        e.stub(rt(addr), make_stub(name, kinds), POP.get(name, 0))
e.stub(rt(0x53538), stub_random)


def guards_msg_stub(emu, eax, edx, *_):           # 0x27CE0 MsgStrInt (text, number) in DayClock: op 314 guards_msg
    t = bytes(emu.uc.mem_read(eax, 40)).split(b"\0")[0]
    n = 1 if t.startswith(b"1 Guard arrives") else int(t.split(b" ")[0])
    log.append(("guards_msg", (n, edx & 0xFFFFFFFF if n == 1 else None)))   # (count > 1: the number is not compared)
    return 0


e.stub(rt(0x27CE0), guards_msg_stub)
e.stub_sprintf()
# door UI calls inside the door effect (combat.md Doors; as in the ai_diff door_fx section): host ops 209 / 210
DOOR_K = {6: "KEY", 9: "KEY", 7: "PCK", 10: "PCK", 8: "HAN", 11: "HAN"}


def door_anim_stub(emu, eax, edx, ebx, ecx):      # 0x5BFF8 (name "I_PDKEY4.FLC", ...): (big, kind, letter)
    b_ = bytes(emu.uc.mem_read(eax, 32))
    nm_ = b_[:b_.index(0)].decode("latin-1")
    log.append(("door_anim", (int(nm_[2:4] == "PD"), nm_[4:7], nm_[0])))
    return 0


def door_snd_stub(which_):                        # 0x31BBC door sounds (0) / 0x31B44 overhead sounds (1)
    def fn_(emu, *_):
        log.append(("door_snd", (which_,)))
        return 0
    return fn_


e.stub(rt(0x31BBC), door_snd_stub(0))
e.stub(rt(0x31B44), door_snd_stub(1))
e.stub(rt(0x5CE08), lambda *_: 0)
e.stub(rt(0x5EDE0), lambda *_: 0)
e.stub(rt(0x5BFF8), door_anim_stub, 4)


class Xdat:
    """op 336 explode_art as a library op (combat.md Craters and blast leftovers): DAT\\EXPLODE.DAT on both sides.
    The original's OpenCDFile 0x38304, fseek 0x757B1, fread_ 0x758AD and fclose 0x75A77 are stubbed with the file's
    bytes; the port's host ops 86 / 96 / 87 / 88 answer the same; both log (open_cd, fseek, fread, fclose)"""
    from common import game_read
    data, pos = game_read("DAT/EXPLODE.DAT"), 0

    @classmethod
    def read(cls, k, out):
        out.append(("fread", (k,)))
        cls.pos += k
        return cls.data[cls.pos - k:cls.pos]


def xdat_open(emu, eax, *_):
    log.append(("open_cd", (bytes(emu.uc.mem_read(eax, 80)).split(b"\0")[0].decode("latin-1").lower(),)))
    Xdat.pos = 0
    return 0x7FF0      # (a file handle; the original does not check it)


def xdat_seek(emu, eax, edx, *_):
    log.append(("fseek", (edx, 0)))
    Xdat.pos = edx
    return 0


def xdat_read(emu, eax, edx, ebx, ecx):
    emu.uc.mem_write(eax, bytes(Xdat.read(edx * ebx, log)))
    return ebx


def xdat_close(emu, *_):
    log.append(("fclose", ()))
    return 0


for a_, f_ in ((0x38304, xdat_open), (0x757B1, xdat_seek), (0x758AD, xdat_read), (0x75A77, xdat_close)):
    e.stub(rt(a_), f_)
frame_ = e.alloc(16)
tramp_ = e.alloc(16)
e.uc.mem_write(tramp_, b"\xBD" + struct.pack("<I", frame_ + 8) + b"\xE9" + struct.pack("<i", rt(0x10C5B) - (tramp_ + 10)))
e.stub(rt(0x10F4E), lambda emu_, *a_: emu_.uc.emu_stop())

DATA = (rt(0x90000), 0x33A730 - 0x90000)
snap = bytes(e.uc.mem_read(*DATA))
rng = random.Random(11)




def world_now():
    bufs, scalars = port_lib.world_load(e, ai)
    return ({k: bytes(v) for k, v in bufs.items() if k not in port_lib.READ_ONLY and k != "scratch"},
            dict(scalars))


def diff(a, b):
    out = []
    for k in a[0]:
        if a[0][k] != b[0][k]:
            bad = [j for j in range(len(a[0][k])) if a[0][k][j] != b[0][k][j]]
            out.append(f"{k} {[hex(j) for j in bad[:8]]}" + (f" (slot {bad[0] // 0x200} +{bad[0] % 0x200:#x}: "
                       f"{a[0][k][bad[0]]:#x} / {b[0][k][bad[0]]:#x})" if k == "soldiers" else
                       f" (tile {bad[0] // 8} byte {bad[0] % 8}: {a[0][k][bad[0]]:#x} / {b[0][k][bad[0]]:#x})" if k == "tiles" else ""))
    out += [f"{k} {a[1][k]} / {b[1][k]}" for k in a[1] if a[1][k] != b[1][k]]
    return out


OUT_ = e.alloc(16)


class UiWorld:
    """a mirror World for host-side UI code (death_show): memory reads / writes on emulator memory (UI state), ext
    calls logged like the original's stubs (a soldier record as its slot's pointer, texts as 0)"""
    def __init__(self, plog):
        self.plog = plog

    def g8(self, a): return e.r8(rt(a))
    def g16(self, a): return e.r16(rt(a))
    def g32(self, a): return e.r32(rt(a))
    def s8(self, a, v): e.w8(rt(a), v & 0xFF)
    def s16(self, a, v): e.w16(rt(a), v & 0xFFFF)
    def s32(self, a, v): e.w32(rt(a), v & 0xFFFFFFFF)

    def ext(self, name, *args):
        a = [rt(G_SOLDIERS) + 0x200 * x[0x17A] if isinstance(x, (bytes, bytearray)) else 0 if x is None or
             isinstance(x, str) else x & 0xFFFFFFFF for x in args]
        self.plog.append((name, conv(CALLEES[name][1], a)))
        return 0


class HostTimer:
    """the front-end's part of the 10 ms timer (ai.front_ticks: tile and effect animations) run by the harness on the
    port's world: tiles and fx_anims in the port's buffers, the gates from its scalars, the tile animation records
    (front-end state, not a world field) in emulator memory"""
    AREAS = (("tiles", ai.G_TILEMAP, 8 * 3136), ("fx_anims", ai.G_FX_ANIMS, 0x12 * 16))
    GATES = {0x92DE1: "round_lock2", 0x92DE0: "round_lock"}

    def __init__(self, bufs, scalars):
        self.bufs, self.scalars = bufs, scalars

    def _get(self, a, k):
        if a in self.GATES:
            return self.scalars[self.GATES[a]] & 0xFF
        for name, base, size in self.AREAS:
            if base <= a < base + size:
                return int.from_bytes(self.bufs[name][a - base:a - base + k], "little")
        return int.from_bytes(e.uc.mem_read(rt(a), k), "little")

    def _set(self, a, k, v):
        for name, base, size in self.AREAS:
            if base <= a < base + size:
                self.bufs[name][a - base:a - base + k] = (v & ((1 << 8 * k) - 1)).to_bytes(k, "little")
                return
        e.uc.mem_write(rt(a), (v & ((1 << 8 * k) - 1)).to_bytes(k, "little"))

    def g8(self, a): return self._get(a, 1)
    def g16(self, a): return self._get(a, 2)
    def g32(self, a): return self._get(a, 4)
    def s8(self, a, v): self._set(a, 1, v)
    def s16(self, a, v): self._set(a, 2, v)
    def s32(self, a, v): self._set(a, 4, v)


ORIG_OPS = {}                      # host op -> how many times the original answered it (printed with WALK_COUNT=1)


def orig_answer(op, args, bufs, scalars, plog):
    """--shoot: a host op answered by the ORIGINAL routine on the port's world (stored to emulator memory, the routine
    called, the world loaded back in place); the routine's own stub calls go to the port's call log"""
    port_lib.world_store(e, ai, bufs, scalars)
    kinds = CALLEES[op][1]
    a = [rt(G_SOLDIERS) + 0x200 * v if k == "s" else v & 0xFFFFFFFF for k, v in zip(kinds, args)]
    log.clear()
    data = None
    if op == "hit_test":               # stack: direction, skip*, chance*, cap, target, roll (port: skip by value)
        e.w8(OUT_, a[5])
        e.w32(OUT_ + 4, 0)                 # (Projectile step: the chance output starts at 0)
        r = e.call(rt(CALLEES[op][0]), a[0], a[1], a[2], a[3], a[4], OUT_, OUT_ + 4, a[6], a[7], a[8], max_steps=50_000_000)
        data = struct.pack("<BxxxI", e.r8(OUT_), e.r32(OUT_ + 4))
    else:
        r = e.call(rt(CALLEES[op][0]), *a, max_steps=50_000_000)
    plog.extend(log)
    port_lib.world_load(e, ai, bufs, scalars)
    return r, data


def shoot_setup():
    """--shoot: a merc with a loaded gun 2..5 tiles from an enemy, both in play, combat on; the original's click aims
    it at the enemy. -> (merc slot, enemy slot) or None"""
    def rec(k):
        return rt(G_SOLDIERS) + 0x200 * k

    def ok(k):
        p = rec(k)
        return e.r8(p + 0x182) and e.r8(p + 0x138) and e.r8(p + 0x183) >= 15 and e.r16(p + 0x114) < 3136
    mercs = [k for k in range(8) if ok(k) and 1 <= e.r8(rec(k)) <= 14 and e.r8(rec(k) + 1)]
    foes = [k for k in range(0x28, 0x30) if ok(k)]
    if not mercs or not foes:
        return None
    m, k = rng.choice(mercs), rng.choice(foes)
    tk = e.r16(rec(k) + 0x114)
    d, step = rng.randint(2, 5), rng.choice([1, -1, 56, -56, 57, -55])
    T = tk + d * step
    if not (0 <= T < 3136) or abs(T % 56 - tk % 56) > d:
        return None
    tm = rt(ai.G_TILEMAP)
    if e.r8(tm + 8 * T + 5) & 0x7F != 0x40 or e.r8(tm + 8 * T + 1) or e.r8(tm + 8 * T) >= 0x6C:
        return None
    p = rec(m)
    old = e.r16(p + 0x114)
    e.w8(tm + 8 * old + 5, (e.r8(tm + 8 * old + 5) & 0x80) | 0x40)
    e.w8(tm + 8 * T + 5, (e.r8(tm + 8 * T + 5) & 0x80) | m)
    x, y = (T % 56) * 16 + 5, (T // 56) * 14 + 5
    for o, v in ((0x114, T), (0x124, T), (0x116, x), (0x118, y), (0x126, x), (0x128, y)):
        e.w16(p + o, v)
    for a_ in (ai.G_COMBAT, 0x92DCC, 0x92DCA):
        e.w8(rt(a_), 1)
    e.w8(rt(ai.G_SELECTED), m)
    e.w8(p + 0x1A5, 25)
    set_rnd([rng.randrange(1 << 16) for _ in range(200)])
    e.call(rt(0x59C14), p, tk, e.r8(p), max_steps=50_000_000)
    return m, k


class FightState:
    def __init__(self):
        self.phase, self.next, self.queue, self.start, self.rounds, self.end = "mercs", 0, None, 0, 0, None
        self.ammo0 = None
        self.clock0 = None
        self.only = None            # a merc's interrupt turn in the enemies' turn: only this merc acts


def fight_driver(f, st):
    """--fight: the actions before frame f on the original's state: [("click", merc, tile, item)] / [("end_turn",)] /
    [] / "stop" (see the module doc)"""
    def rec(k):
        return rt(G_SOLDIERS) + 0x200 * k

    def alive(k):
        p = rec(k)
        return e.r8(p + 0x182) and e.r8(p + 0x138) and ai.s8(e.r8(p + 0x183)) > 0 and not e.r8(p + 0x188)
    mercs = [k for k in range(8) if alive(k)]
    foes = [k for k in range(0x28, 0x30) if alive(k)]
    if st.end is None and (not mercs or not foes):
        st.end = f + 40
    if st.end is not None:
        return "stop" if f >= st.end else []
    if st.clock0 is None:           # the day clock at the start (turns.md The day clock; 5 minutes per new round)
        st.clock0 = (e.r8(rt(0x22A0F4)), e.r8(rt(0x22A0F5)), e.r8(rt(0x22A0F6)), e.r32(rt(0x22A0F7)))
    if st.phase == "enemies":
        if f - st.start > 30 and e.r8(rt(ai.G_SELECTED)) < 8 and e.r8(rt(0x92DDC)) and e.r8(rt(0x92DCC)):
            # a merc won an interrupt in the enemies' turn (InterruptTurn 0x1304C): the player gives orders to
            # the interrupter only (the game selects it, 0x92DD2), then End turn hands the turn back
            st.phase, st.next, st.only = "mercs", f + 10, e.r8(rt(ai.G_SELECTED))
            return []
        if f - st.start > 30 and (e.r8(rt(ai.G_SELECTED)) < 8 or not e.r8(rt(0x92DDC))):
            st.phase, st.next, st.rounds, st.only = "mercs", f + 10, st.rounds + 1, None
            if st.rounds >= ROUNDS:
                st.end = f
            # a fight of 200 rounds is longer than a day (12 hours = 144 new rounds): the clock goes back to its
            # start value at every new round, so the day does not end in the fight (both sides)
            back = [("clock",) + st.clock0]
            if e.r8(rt(ai.G_SELECTED)) >= 8:      # a new round, but the game could not select the merc to show (it
                return back + [("select", mercs[0])]     # died): the player clicks a living merc (op 120, 0x1148C)
            return back
        return []
    if f < st.next:
        return []
    if not e.r8(rt(0x92DCC)):       # an enemy interrupts in the mercs' turn (NextTurn 0x12D58): the player waits until
        st.next = f + 10            # the turn comes back (MercsTurn 0x12DD4 sets 0x92DCC again)
        return []
    if st.ammo0 is None:            # the gun loads at the start: an empty gun is loaded again (the player's reload)
        st.ammo0 = {k: e.r8(rec(k) + 1) for k in range(8)}
    if st.queue is None:
        # no gun in the main hand, a loaded gun in the second hand (item 1): the player swaps the hands
        if st.only is not None:
            mercs = [k for k in mercs if k == st.only]
        swap = [k for k in mercs if not 1 <= e.r8(rec(k)) <= 14 and 1 <= e.r8(rec(k) + 12) <= 14 and e.r8(rec(k) + 13)]
        if swap:
            for k in swap:
                st.ammo0[k] = e.r8(rec(k) + 13)
            st.next = f + 1
            return [("swap", k) for k in swap]
        guns = [k for k in mercs if 1 <= e.r8(rec(k)) <= 14 and (e.r8(rec(k) + 1) or st.ammo0[k])]
        # no gun left: the mercs walk toward the nearest enemy, so that it sees them (with real sightings an enemy
        # that sees nobody stays where it is, and the fight would not end)
        st.queue = list(guns) or [-1 - k for k in mercs]
        reload = [("ammo", k, st.ammo0[k]) for k in guns if not e.r8(rec(k) + 1)]
        if reload:
            st.next = f + 1
            return reload
    while st.queue:
        m = st.queue.pop(0)
        walk_only = m < 0
        m = -1 - m if walk_only else m
        if m not in mercs or st.only is not None and m != st.only:
            continue
        me = e.r16(rec(m) + 0x114)

        def dist(t):
            return max(abs(t % 56 - me % 56), abs(t // 56 - me // 56))
        k = min(foes, key=lambda k_: (dist(e.r16(rec(k_) + 0x114)), k_))
        t = e.r16(rec(k) + 0x114)
        st.next = f + 60
        if os.environ.get("FIGHT_DEBUG"):
            print(f"  f {f} merc {m} at {me} hp {e.r8(rec(m) + 0x183)} ap {e.r8(rec(m) + 0x1A5)} -> foe {k} at {t} walk_only {walk_only} items {[(e.r8(rec(m) + 12 * i), e.r8(rec(m) + 12 * i + 1)) for i in range(12)]}")
        if dist(t) > 8:             # far away: walk a third of the way toward it (move item 0x4D)
            r, c = me // 56 + (t // 56 - me // 56) // 3, me % 56 + (t % 56 - me % 56) // 3
            return [("click", m, 56 * r + c, 0x4D)]
        if walk_only:               # near: walk to the tile before it
            if dist(t) <= 1:
                continue
            sg = lambda v: (v > 0) - (v < 0)
            r, c = t // 56 - sg(t // 56 - me // 56), t % 56 - sg(t % 56 - me % 56)
            return [("click", m, 56 * r + c, 0x4D)]
        return [("click", m, t, e.r8(rec(m)))]
    st.phase, st.start, st.queue = "enemies", f, None
    return [("end_turn",)]


fails = cases = frames_total = 0
fights_won = fights_lost = rounds_total = 0
moved = spent = refilled = short = 0
wet_cases = deep_cases = drowned = 0
mine_hits = [0]       # --mine: MineFlag 0x19D74 entries in the original (walks with a flagged mine)
mines_flagged = 0
water_hits = {0x407B8: 0, 0x3843C: 0}   # --water: SoakItems 0x407B8 / Drown? 0x3843C entries in the original
water_seen = {0x407B8: 0, 0x3843C: 0}
if WATER:
    from unicorn import UC_HOOK_CODE
    for a_ in water_hits:
        e.uc.hook_add(UC_HOOK_CODE, lambda uc, addr, size, ud, a_=a_: water_hits.__setitem__(a_, water_hits[a_] + 1), None, rt(a_), rt(a_))
if MINE:
    from unicorn import UC_HOOK_CODE
    e.uc.hook_add(UC_HOOK_CODE, lambda uc, addr, size, ud: mine_hits.__setitem__(0, mine_hits[0] + 1), None, rt(0x19D74), rt(0x19D74))
shots = hits = 0
foes_moved = foes_shot = mercs_hit = 0
COUNTS = ("rounds_total", "fights_won", "fights_lost", "moved", "spent", "refilled", "short", "wet_cases", "deep_cases",
          "drowned", "mines_flagged", "shots", "hits", "foes_moved", "foes_shot", "mercs_hit")
# (issue #60: the mutation runner sets JA1_ORIG_CACHE; ./t check does not, then oc is None)
oc = orig_cache.open_cache("walk_diff", [os.path.join(HERE, "..", "..", "rom", "JA.EXE"), dump], (
    [os.path.basename(a_) for a_ in sys.argv[1:]], sorted(LIB_OPS), HOST_OPS, sorted(SHOOT_REAL), hashlib.sha256(Xdat.data).hexdigest()))
for case in range(n):
    e.uc.mem_write(DATA[0], snap)
    state0_ = rng.getstate()
    cached_ = oc.get(case, state0_) if oc and not ORIG else None
    if cached_ is None:
        counts0_ = {k_: globals()[k_] for k_ in COUNTS}
        seen0_ = dict(water_seen)
        stopped_ = False
        if SHOOT:
            pair = None
            while pair is None:
                e.uc.mem_write(DATA[0], snap)
                pair = shoot_setup()
            slot, foe = pair
            start = bytes(e.uc.mem_read(*DATA))
            hp0 = e.r8(rt(G_SOLDIERS) + 0x200 * foe + 0x183)
        slot = slot if SHOOT else rng.randrange(8)
        if ENEMY:           # the enemies' turn: no move order; positions and hit points before it
            pos0 = [e.r16(rt(G_SOLDIERS) + 0x200 * k + 0x114) for k in range(0x30)]
            hp_0 = [e.r8(rt(G_SOLDIERS) + 0x200 * k + 0x183) for k in range(8)]
        sp = rt(G_SOLDIERS) + 0x200 * slot
        me = e.r16(sp + 0x114)
        tile = me + rng.choice([1, -1, 56, 57]) if rng.random() < 0.2 else me + rng.randrange(-8, 9) + 56 * rng.randrange(-6, 7)
        if WATER:           # water tiles (IsWaterTile 0x23A10) in the same box
            tm_ = rt(ai.G_TILEMAP)
            wet_ = [(me + dx + 56 * dy) % 3136 for dy in range(-6, 7) for dx in range(-8, 9)]
            wet_ = [t_ for t_ in wet_ if ai.is_water_tile(e.r8(tm_ + 8 * t_), e.r8(tm_ + 8 * t_ + 1))]
            free_ = [t_ for t_ in wet_ if e.r8(tm_ + 8 * t_ + 5) & 0x7F >= 0x30]
            if free_ and rng.random() < 0.7:   # the walker starts in the water (a click path never enters deep water
                deep_ = [t_ for t_ in free_ if e.r8(tm_ + 8 * t_) >= 0x97]
                me = rng.choice(deep_ if deep_ and rng.random() < 0.7 else free_)   # 0x97+): VacateTile 0x2395C, OccupyTile 0x23974, ReplaceSoldier 0x2FB98
                e.call(rt(0x2395C), e.r16(sp + 0x114))
                e.w16(sp + 0x114, me)
                e.w16(sp + 0x124, me)
                for o_ in (0x116, 0x126):   # the point (WalkMove 0x1A048 moves +0x116/+0x118 to +0x126/+0x128; ViewTile of
                    e.w16(sp + o_, me % 56 * 16 + 5)   # the point is the tile), else the first frame puts the walker back
                    e.w16(sp + o_ + 2, me // 56 * 14 + 5)   # on its old tile (TileToX/Y)
                e.call(rt(0x23974), me, slot)
                e.call(rt(0x2FB98), sp)
                if os.environ.get("WATER_DEBUG"):
                    print(f"  placed slot {slot} at {me}: ground {e.r8(tm_ + 8 * me):#x}, +1B2 {e.r8(sp + 0x1B2):#x} anim {e.r16(sp + 0x1E8):#x} hp {e.r8(sp + 0x183)}")
                near_ = [t_ for t_ in wet_ if e.r8(tm_ + 8 * t_) < 0x97 and abs(t_ % 56 - me % 56) <= 2 and abs(t_ // 56 - me // 56) <= 2]
                swim_ = [t_ for t_ in wet_ if e.r8(tm_ + 8 * t_) >= 0x9A and t_ != me and abs(t_ % 56 - me % 56) <= 5 and abs(t_ // 56 - me // 56) <= 5]
                r_ = rng.random()   # a land tile, another water tile, or shallow water next to the start (a swimmer's step into
                tile = (rng.choice(near_) if near_ and r_ < 0.3 else rng.choice(wet_) if r_ < 0.5 else   # water: Drown? 0x3843C)
                        rng.choice(swim_) if swim_ and e.r8(tm_ + 8 * me) >= 0x9A and r_ < 0.8 else   # deep to deep: the swim
                        (me + rng.randrange(-4, 5) + 56 * rng.randrange(-3, 4)) % 3136)   # animation stays, Drown? hurts
            elif wet_:
                tile = rng.choice(wet_)
            swimmer_ = WATER and e.r8(rt(ai.G_TILEMAP) + 8 * tile) >= 0x9A and e.r8(rt(ai.G_TILEMAP) + 8 * me) >= 0x9A
            tired = rng.choice([1, 1, 2, 3] if swimmer_ else [None, 0, 0, 1, 2, 5])   # energy +0x18A (fatigue +0x10C to match: energy = 100 - fatigue / 100)
            if tired is not None:
                e.w8(sp + 0x18A, tired)
                e.w16(sp + 0x10C, 10000 - 100 * tired)
                e.w8(sp + 0x1BA, rng.choice([0, 1]))   # Drown? 0x3843C: 0 = first time (message), 1 = damage
            if swimmer_:    # Drown? 0x3843C damage: state +0x19C 5 doubles it; low health dies (KILL)
                if rng.random() < 0.3:
                    e.w8(sp + 0x19C, 5)
                if rng.random() < 0.4:
                    e.w8(sp + 0x183, rng.choice([15, 16, 18, 20, 25]))   # (below 15 the walk order fails)
                    e.w8(sp + 0x1BA, 1)
            start = bytes(e.uc.mem_read(*DATA))
        if MINE:            # mines on tiles of the box between the walker and the target (free ground items 0..199)
            tm_ = rt(ai.G_TILEMAP)
            forget_ = rng.random() < 0.3
            if forget_:      # the forgetful stop needs path step +0x1AF > 15 (5 in combat): a far target
                tile = me + rng.choice([-1, 1]) * rng.randrange(10, 24) + 56 * rng.randrange(-12, 13)
            x0_, x1_ = sorted((me % 56, tile % 56))
            y0_, y1_ = sorted((me // 56, tile // 56))
            sk_ = rng.choice([0, 20, 40, 60, 80, 100, 0x80])
            e.w8(sp + 0x192, sk_)
            box_ = [(y_ * 56 + x_) % 3136 for y_ in range(y0_ - 1, y1_ + 2) for x_ in range(x0_ - 1, x1_ + 2)]
            for t_ in rng.sample(box_, min(len(box_), rng.choice([1, 3, 6, 12, 24]))):
                if t_ == me % 3136:
                    continue
                used_ = {e.r8(tm_ + 8 * u_ + 6) for u_ in range(3136)}
                used_ |= {e.r8(rt(ai.G_GROUND_ITEMS) + 12 * u_ + 6) for u_ in range(200)}   # (chains: byte 6 = next item)
                free_k = [u_ for u_ in range(200) if u_ not in used_]
                if not free_k:
                    break
                k_ = rng.choice(free_k)
                e.w8(tm_ + 8 * t_ + 6, k_)
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * k_ + 6, 0xFF)
                g_ = rt(ai.G_GROUND_ITEMS) + 12 * k_
                e.w8(g_, rng.choice([0x13, 0x13, 0x2C, 0x2C, 0x11]))
                e.w8(g_ + 2, rng.choice([5, 60, 0xC4, 0xFF, rng.randrange(256)] + [sk_, -sk_, sk_ - 1, sk_ + 1, 1 - sk_] * 2) & 0xFF)   # (power = skill: the >= edge)
            if forget_:
                e.w8(sp + 0x19C, 2)
            start = bytes(e.uc.mem_read(*DATA))
        tile %= 3136        # a click gives a map tile (ViewTile: 0..3135); a merc at a map edge drew one off the map, where both
                            # sides read bytes before or after the tile map (outfit dump, merc at tile 0: -54)
        if CROSS:
            slot2 = rng.choice([k for k in range(8) if k != slot])
            tile2 = (tile + rng.choice([0, 0, 1, -1, 56, -56])) % 3136
            sp2 = rt(G_SOLDIERS) + 0x200 * slot2
        ticks = [rng.randint(1, 4) for _ in range(FRAMES)]
        rand = [rng.randrange(1 << 16) for _ in range(300000 if FIGHT else 4000)]
        if COMBAT or ENEMY:   # turn-based combat, the mercs' turn shown (the state after a sighting); the walker is selected
            for a_ in (ai.G_COMBAT, 0x92DCC, 0x92DCA):
                e.w8(rt(a_), 1)
            e.w8(rt(ai.G_SELECTED), slot)
            forget_ap = rng.choice([40, 60, 90, 127]) if MINE and forget_ else None
            if forget_ap:    # the forgetful stop in combat needs path step +0x1AF > 5 and AP left (step start step 7: Random(AP) + 1 off)
                e.w8(sp + 0x1A5, forget_ap)
        if FIGHT:           # the game's confirm-move option off, as the port (click.md 0x4D)
            e.w8(rt(0x1163C6), 0)
        END = -1 if FIGHT else 0 if ENEMY else FRAMES // 2 if COMBAT else -1

        # the original
        set_rnd(rand)
        log.clear()
        ap0 = e.r8(sp + 0x1A5)
        r0 = 0 if REAL else ai._i32(e.call(rt(0x1C608), sp, tile, 0, 0, 0, 0, 0, 0))
        if CROSS:
            r0 = (r0, ai._i32(e.call(rt(0x1C608), sp2, tile2, 0, 0, 0, 0, 0, 0)))
        want = [(r0, list(log), world_now())]
        if os.environ.get("WATER_DEBUG"): print(f"  walk {e.r16(sp + 0x114)} ({e.r8(rt(ai.G_TILEMAP) + 8 * e.r16(sp + 0x114)):#x}) -> {tile} ({e.r8(rt(ai.G_TILEMAP) + 8 * tile):#x}) r0 {r0} energy {e.r8(sp + 0x18A)} anim {e.r16(sp + 0x1E8):#x} hp {e.r8(sp + 0x183)}")
        if os.environ.get("MINE_DEBUG"): print("  walk_to log", [x for x in log if x[0] != "Random"][:12])
        flew = False
        wet_now = deep_now = False
        hp_w = e.r8(sp + 0x183)
        events, fst = {}, FightState()
        for f in range(FRAMES):
            log.clear()
            if FIGHT:
                acts = fight_driver(f, fst)
                if acts == "stop":
                    break
                events[f] = acts
                for a_ in acts:
                    if a_[0] == "click":
                        e.w8(rt(ai.G_SELECTED), a_[1])
                        e.call(rt(0x59C14), rt(G_SOLDIERS) + 0x200 * a_[1], a_[2], a_[3], max_steps=4_000_000_000)
                    elif a_[0] == "ammo":
                        e.w8(rt(G_SOLDIERS) + 0x200 * a_[1] + 1, a_[2])
                    elif a_[0] == "swap":
                        p_ = rt(G_SOLDIERS) + 0x200 * a_[1]
                        h_ = bytes(e.uc.mem_read(p_, 24))
                        e.uc.mem_write(p_, h_[12:] + h_[:12])
                    elif a_[0] == "clock":
                        e.w8(rt(0x22A0F4), a_[1])
                        e.w8(rt(0x22A0F5), a_[2])
                        e.w8(rt(0x22A0F6), a_[3])
                        e.w32(rt(0x22A0F7), a_[4])
                    elif a_[0] == "select":
                        e.call(rt(0x1148C), a_[1], 1, 0, max_steps=50_000_000)
                    else:
                        e.call(rt(0x121D8), max_steps=4_000_000_000)
            if f == END:
                ap1 = e.r8(sp + 0x1A5)
                spent += ap1 < ap0
                e.call(rt(0x121D8), max_steps=4_000_000_000 if ENEMY else 400_000_000)
            for _ in range(ticks[f]):
                e.call(rt(0x5EE98))
            e.w32(frame_ + 4, 0)
            try:
                e.call(tramp_, max_steps=4_000_000_000 if ENEMY else 400_000_000 if COMBAT else 5_000_000)   # (combat: FindPath with real AP costs)
            except EmuStop as x:
                print(f"case {case}: the original stops in frame {f}: {x}")
                stopped_ = True
                break
            want.append((e.r8(frame_ + 4), list(log), world_now()))
            if WATER:
                g_ = e.r8(sp + 0x1B2)
                wet_now = wet_now or ai.is_water_tile(g_, e.r8(sp + 0x1B4))
                deep_now = deep_now or g_ >= 0x9A
            if os.environ.get("WALK_DEBUG2") and (e.r8(rt(ai.G_SELECTED)) != getattr(fst, "dsel", None) or any(x[0] in ("new_turn", "w_12404", "w_12dd4", "turn_banner", "next_turn", "w_1304c") for x in log)):
                fst.dsel = e.r8(rt(ai.G_SELECTED))
                print(f"  f {f} sel {fst.dsel:#x} intr {e.r8(rt(0x92DDC))} combat {e.r8(rt(ai.G_COMBAT))} turn {e.r8(rt(0x92DCC))} {e.r8(rt(0x92DCD))} t0f {e.r8(rt(0x284F0F))} log {[x for x in log if x[0] not in ('Random',)][:10]}")
            if os.environ.get("WALK_DEBUG") and f % int(os.environ["WALK_DEBUG"]) == 0:   # (debug: the turn state)
                sel_ = e.r8(rt(ai.G_SELECTED))
                q_ = rt(G_SOLDIERS) + 0x200 * min(sel_, 0x2F)
                print(f"  f {f} {fst.phase} sel {sel_:#x} +197 {e.r8(q_ + 0x197)} +1B0 {e.r8(q_ + 0x1B0)} +17E {e.r8(q_ + 0x17E)} "
                      f"+1D1 {e.r8(q_ + 0x1D1)} +1D6 {e.r8(q_ + 0x1D6)} +1A5 {e.r8(q_ + 0x1A5)} alert {e.r8(q_ + 0xBB)} tb {e.r32(rt(ai.G_TURN_BASED))} "
                      f"fast {ai._i32(e.r32(rt(ai.G_FAST_TIMER)))} busy {e.r32(rt(ai.G_BUSY))} itq {e.r8(rt(ai.G_ITQ_N))} "
                      f"intr {e.r8(rt(0x92DDC))} kept {e.r8(rt(0x284F0F))} log {[x[0] for x in log][:8]}")
            flew |= e.r8(sp + 0x1BF) == 3 if SHOOT else any(e.r8(rt(G_SOLDIERS) + 0x200 * k + 0x1BF) == 3 for k in range(0x28, 0x30))
        if FIGHT:
            rounds_total += fst.rounds
            left_m = [k for k in range(8) if e.r8(rt(G_SOLDIERS) + 0x200 * k + 0x182) and e.r8(rt(G_SOLDIERS) + 0x200 * k + 0x138)
                      and ai.s8(e.r8(rt(G_SOLDIERS) + 0x200 * k + 0x183)) > 0]
            left_e = [k for k in range(0x28, 0x30) if e.r8(rt(G_SOLDIERS) + 0x200 * k + 0x182) and
                      e.r8(rt(G_SOLDIERS) + 0x200 * k + 0x138) and ai.s8(e.r8(rt(G_SOLDIERS) + 0x200 * k + 0x183)) > 0]
            fights_won += not left_e
            fights_lost += not left_m
            if ORIG:
                print(f"case {case}: {fst.rounds} rounds, {len(want) - 1} frames, mercs left {left_m}, enemies left {left_e}")
                R_ = lambda k, o: e.r8(rt(G_SOLDIERS) + 0x200 * k + o)
                print("  mercs (hp, hand item, ammo, tile, ap):", [(k, R_(k, 0x183), R_(k, 0), R_(k, 1), e.r16(rt(G_SOLDIERS) + 0x200 * k + 0x114), R_(k, 0x1A5)) for k in range(8)])
                print("  enemies (hp, tile):", [(k, R_(k, 0x183), e.r16(rt(G_SOLDIERS) + 0x200 * k + 0x114)) for k in range(0x28, 0x30)])
        if ENEMY:
            foes_moved += sum(e.r16(rt(G_SOLDIERS) + 0x200 * k + 0x114) != pos0[k] for k in range(0x28, 0x30))
            foes_shot += flew
            mercs_hit += sum(e.r8(rt(G_SOLDIERS) + 0x200 * k + 0x183) < hp_0[k] for k in range(8))
        if SHOOT:
            shots += flew
            hits += e.r8(rt(G_SOLDIERS) + 0x200 * foe + 0x183) < hp0
        end = e.r16(sp + 0x114)
        moved += end != me
        if WATER:
            for a_ in water_hits:
                water_seen[a_] += water_hits[a_] > 0
                water_hits[a_] = 0
            wet_cases += wet_now
            deep_cases += deep_now
            drowned += wet_now and e.r8(sp + 0x183) < hp_w
        if MINE:
            mines_flagged += mine_hits[0] > 0
            mine_hits[0] = 0
        if CROSS:
            short += (end != tile) + (e.r16(sp2 + 0x114) != tile2)
        if COMBAT and len(want) > END + 1:
            refilled += e.r8(sp + 0x1A5) > ap1
        if ORIG:
            print(f"case {case}: slot {slot} {me} -> {tile}: result {r0}, at {end} after {len(want) - 1} frames" +
                  (f"; ground {e.r8(rt(ai.G_TILEMAP) + 8 * tile):#x} at the target, {e.r8(rt(ai.G_TILEMAP) + 8 * end):#x} at the end" if WATER else "") +
                  (f"; shot {flew}, enemy {foe} hp {hp0} -> {e.r8(rt(G_SOLDIERS) + 0x200 * foe + 0x183)}" if SHOOT else "") +
                  (f"; enemies moved {foes_moved}, a shot flew {flew}, mercs hit {mercs_hit} (totals)" if ENEMY else ""))
            continue
        if oc and not stopped_:      # (#60 rule 3: a case that stopped on a time limit is never stored)
            oc.put(case, state0_, dict(
                slot=slot, me=me, tile=tile, slot2=slot2 if CROSS else None, tile2=tile2 if CROSS else None,
                start=start if SHOOT or WATER or MINE else None, forget_ap=forget_ap if COMBAT or ENEMY else None,
                END=END, ticks=ticks, rand=array.array("H", rand).tobytes(), events=events,
                want=orig_cache.pack_frames(want), rng=rng.getstate(),
                counts={k_: globals()[k_] - counts0_[k_] for k_ in COUNTS},
                seen={k_: water_seen[k_] - seen0_[k_] for k_ in water_seen}))
    else:                            # a hit: all that the port side below needs from the original side
        slot, me, tile, slot2, tile2 = (cached_[k_] for k_ in ("slot", "me", "tile", "slot2", "tile2"))
        start, forget_ap, END, ticks, events = (cached_[k_] for k_ in ("start", "forget_ap", "END", "ticks", "events"))
        rand = array.array("H", cached_["rand"]).tolist()
        want = orig_cache.unpack_frames(cached_["want"])
        rng.setstate(cached_["rng"])
        sp = rt(G_SOLDIERS) + 0x200 * slot
        for k_, v_ in cached_["counts"].items():
            globals()[k_] += v_
        for k_, v_ in cached_["seen"].items():
            water_seen[k_] += v_

    # the port, on the same start
    e.uc.mem_write(DATA[0], start if SHOOT or WATER or MINE else snap)
    if COMBAT or ENEMY:
        for a_ in (ai.G_COMBAT, 0x92DCC, 0x92DCA):
            e.w8(rt(a_), 1)
        e.w8(rt(ai.G_SELECTED), slot)
        if forget_ap:
            e.w8(sp + 0x1A5, forget_ap)
    bufs, scalars = port_lib.world_load(e, ai)
    set_rnd(rand)
    plog = []

    def random_(k):
        k = ai.s16(k)
        plog.append(("Random", (k,)))
        v = next_rnd()
        return v % k if k > 0 else 0

    def call(op, args):
        if op == "mouse_check":      # op 162 (turns.md New turn): the original's MouseInRect? on its panel rectangle
            plog.append(("mouse_in_rect", tuple(ai._i32(e.r32(rt(a_))) for a_ in (0x229CB8, 0x229CC4, 0x229CD8, 0x229CDC))))
            plog.append(("w_673ac", ()))     # (the stub answers 0: the mouse is outside)
            return 0, None
        if op == "death_show":       # op 186 (shooting.md Merc dies): the merc death display (UI), answered by the
            ai.merc_death_show(bufs["soldiers"][0x200 * args[0]:0x200 * args[0] + 0x200], UiWorld(plog), args[1])
            return 0, None           # mirror; its calls logged as the original's stubs log them
        if op == "panel_view":       # op 196 (select.md): only in view mode 3 (the dumps have opt_onscreen 1)
            raise SystemExit("walk_diff: host op 196 panel_view is not answered here")
        if op in ("cursor_reset", "attack_cursor", "banner", "tile_marker", "cursor_select"):   # ops 163 / 192 / 194 / 130 / 200: cursor bytes /
            return 0, None                     # attack cursor timer / turn banner text (UI state, not world fields)
        if op == "loss_msg":         # op 183 (shooting.md batch 3, Hit location): the stat-loss message; the original
            return 0, None           # writes its text itself (UI text, not a world field compared here)
        if op == "hears_msg":        # op 213 (ai.md Hearing a noise): the original's text into its buffer, then its message call
            ai.hears_text(TextWorld(bufs), args[0], args[1], args[2] & 0xFF, args[3] & 0xFF)
            plog.append(("message", ()))
            return 0, None
        if op == "door_snd":         # ops 209 / 210 (combat.md Doors): logged as the original's door stubs log them
            plog.append(("door_snd", (args[0],)))
            return 0, None
        if op == "door_anim":
            big_, r_, ph_ = (ai._i32(a_) for a_ in args[:3])
            plog.append(("door_anim", (big_ & 0xFF, DOOR_K.get(r_, f"r {r_}"), "I" if not ph_ else "S" if r_ >= 9 else "F")))
            return 0, None
        if op == "guards_msg":       # op 314 (turns.md The day clock): logged as guards_msg_stub logs it
            plog.append(("guards_msg", (args[0], args[1] & 0xFFFFFFFF if args[0] == 1 else None)))
            return 0, None
        if op == "open_cd":          # ops 86 / 96 / 87 / 88: DAT\\EXPLODE.DAT for op 336 (class Xdat); kind 5 only
            if args[0] != 5:
                raise SystemExit(f"walk_diff: host op 86 open_cd kind {args[0]} is not answered here")
            plog.append(("open_cd", ("dat\\explode.dat",)))
            Xdat.pos = 0
            return 0, None
        if op == "fseek":
            plog.append(("fseek", (args[0] & 0xFFFFFFFF, 0)))
            Xdat.pos = args[0] & 0xFFFFFFFF
            return 0, None
        if op == "fread":
            d_ = Xdat.read(args[0] & 0xFFFFFFFF, plog)
            return len(d_), d_
        if op == "fclose":
            plog.append(("fclose", ()))
            return 0, None
        if op == "connected_home":   # the port passes the sector only; the original also the table address
            args = [ai.G_SECTOR_TBL9] + list(args)
        if op not in CALLEES:
            raise SystemExit(f"port called unknown host op {op} {args}")
        if ATTACK and op in SHOOT_REAL:
            ORIG_OPS[op] = ORIG_OPS.get(op, 0) + 1
            return orig_answer(op, args, bufs, scalars, plog)
        plog.append((op, conv(CALLEES[op][1].replace("s", "i"), [a & 0xFFFFFFFF for a in args])))
        return 0, None

    def port_state():
        port_lib.world_store(e, ai, {k_: bytearray(v_) for k_, v_ in bufs.items()}, dict(scalars))   # (a copy: store
        return world_now()                                                                           # converts +0x90)
    r1 = 0 if REAL else port_lib.ai_run("move_order", bufs, scalars, slot, random_, call, (tile, 0, 0, 0, 0, 0), host_ops=HOST_OPS)
    if CROSS:
        r1 = (r1, port_lib.ai_run("move_order", bufs, scalars, slot2, random_, call, (tile2, 0, 0, 0, 0, 0), host_ops=HOST_OPS))
    got = [(r1, list(plog), port_state())]
    for f in range(len(want) - 1):
        plog.clear()
        for a_ in events.get(f, []):
            if a_[0] == "click":
                scalars["selected"] = a_[1]
                port_lib.ai_run("click", bufs, scalars, a_[1], random_, call, (a_[2], a_[3]),
                                sync=(lambda sc: None, lambda sc: None), host_ops=HOST_OPS)
            elif a_[0] == "ammo":
                bufs["soldiers"][0x200 * a_[1] + 1] = a_[2]
            elif a_[0] == "swap":
                p_ = 0x200 * a_[1]
                bufs["soldiers"][p_:p_ + 24] = bufs["soldiers"][p_ + 12:p_ + 24] + bufs["soldiers"][p_:p_ + 12]
            elif a_[0] == "clock":
                scalars.update({"day_hours": ai.s8(a_[1]), "clock_minute": a_[2], "clock_hour": a_[3],
                                "day_minutes": ai._i32(a_[4])})
            elif a_[0] == "select":
                port_lib.sector_run("select_merc", bufs, scalars, [a_[1], 1, 0], random_, call, HOST_OPS, sync=ATTACK)
            else:
                port_lib.sector_run("end_turn", bufs, scalars, [], random_, call, HOST_OPS, sync=ATTACK)
        if f == END:
            port_lib.sector_run("end_turn", bufs, scalars, [], random_, call, HOST_OPS, sync=ATTACK)
        for _ in range(ticks[f]):
            port_lib.sector_run("tick", bufs, scalars, [], random_, call, HOST_OPS, sync=ATTACK)
            ai.front_ticks(HostTimer(bufs, scalars))
        r = port_lib.sector_run("frame", bufs, scalars, [], random_, call, HOST_OPS, sync=ATTACK) & 0xFF
        got.append((r, list(plog), port_state()))
    cases += 1
    for f, (w_, g_) in enumerate(zip(want, got)):
        frames_total += 1
        d = diff(w_[2], g_[2])
        if w_[0] != g_[0] or w_[1] != g_[1] or d:
            fails += 1
            if fails <= 5:
                c_ = next((j for j in range(min(len(w_[1]), len(g_[1]))) if w_[1][j] != g_[1][j]), min(len(w_[1]), len(g_[1])))
                print(f"MISMATCH case {case} slot {slot} {me} -> {tile}, frame {f} ({'order' if f == 0 else 'frame'}): "
                      f"result {w_[0]} / {g_[0]}; world {d}\n  calls {len(w_[1])} / {len(g_[1])}, first difference at "
                      f"call {c_}:\n  calls original {w_[1][max(c_ - 3, 0):c_ + 6]}\n  calls port     {g_[1][max(c_ - 3, 0):c_ + 6]}")
            break
if FIGHT:
    print(f"walk_diff --fight: {cases if not ORIG else n} fights ({rounds_total} rounds, {fights_won} won by the mercs, "
          f"{fights_lost} by the enemies), {frames_total} frames compared; {fails} mismatches")
elif ENEMY:
    print(f"walk_diff --enemy: {cases if not ORIG else n} enemy turns ({foes_moved} enemy moves, {foes_shot} turns with a "
          f"shot, {mercs_hit} mercs hit), {frames_total} frames compared; {fails} mismatches")
elif SHOOT:
    print(f"walk_diff --shoot: {cases if not ORIG else n} shots ({shots} flew, {hits} hit the enemy), {frames_total} frames "
          f"compared; {fails} mismatches")
else:
    print(f"walk_diff{' --combat' if COMBAT else ''}: {cases if not ORIG else n} walks ({moved} moved"
          f"{f', {spent} spent AP, {refilled} got AP back in the new round' if COMBAT else ''}{f', {short} walkers off their target at the end' if CROSS else ''}{f', a mine flagged in {mines_flagged} walks' if MINE else ''}{f', {wet_cases} walkers in water, {deep_cases} in deep water, {drowned} hurt in water, SoakItems in {water_seen[0x407B8]} walks, Drown in {water_seen[0x3843C]}' if WATER else ''}), {frames_total} frames compared; {fails} mismatches")
if os.environ.get("WALK_COUNT"):
    print("answered by the original:", dict(sorted(ORIG_OPS.items())))
if FIGHT and "--to-end" in sys.argv and fights_won + fights_lost < n:
    print("walk_diff --fight --to-end: a fight did not end")
    fails += 1
sys.exit(1 if fails else 0)
