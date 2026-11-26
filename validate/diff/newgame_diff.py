"""Differential test: a new game (world_load.md section 11), original NewGameInit 0x37A0C vs port.

usage: .venv/bin/python validate/diff/newgame_diff.py [ncases=12] [DUMP] [--port] [--fan 31] [--dump-facts]
Per case: difficulty 0..2 (the Set Difficulty screen; other bytes make the original fault) and quick-save mode (any byte), noise in every block that a day save holds
(soldiers, profiles, sector_state, sectors, the 0x229FA0 block) and in the item store counts. The original
NewGameInit runs in Unicorn on tac60 after the program-start values (FUN_000377b0's options, the EXE's sector order
and notes, an empty save name; a fresh program has nothing else in those blocks). Stubs: OpenCDFile 0x38304 +
fseek/fread/fclose = DAT\\JAGGED.DAT from the CD, EDT\\NEWPROF.EDT from rom/; InitTempFiles 0x38038 (deletes files);
ClearItemStash 0x515DC (the counts go to 0, the store empties); the store's put 0x51CB0 (record kept, count + 1).
Then the original SaveGame 0x4EBC8 writes the day save (fwrite_ captured; SaveItemStash 0x51590 = the counts, then
the records sorted as strategic.md "The item store"). --port: `ja1front ISO EXE --new-game D Q OUT` must write the
same file byte for byte (bytes 2..27, the save name, included). Exits 1 on any mismatch.
--fan 31 (with --port; #70 row 21): a new game after another game. The notes' found flags (and 0x92E2D) hold noise
from the game before; the original runs the 1.15m NewGameInit call 0x37B5A + cave 0x18FE9 (Emu.put_115m), the port
`JA1_FAN_FIXES=F31 JA1_NEW_GAME_OLD=<the plain port save with those flags> ja1front --new-game`.
--dump-facts prints the non-zero bytes of the 0x229FA0 block per difficulty (for the spec).
"""
import os, random, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
from common import cd_read           # noqa: E402
from le import load as le_load       # noqa: E402

argv = sys.argv[1:]
FAN = None
if "--fan" in argv:
    FAN = [int(x_.strip().lstrip("Ff")) for x_ in argv[argv.index("--fan") + 1].split(",")]
    del argv[argv.index("--fan"):argv.index("--fan") + 2]
    if "--port" not in argv or FAN != [31]:
        raise SystemExit("newgame_diff: --fan needs --port and only knows 31")
FAN_RUNS = {31: [(0x37B5A, 0x37B5F), (0x18FE9, 0x19008)]}   # NewGameInit call + cave (notes flags, 0x92E2D = 99)
args = [a for a in argv if not a.startswith("--")]
ncases = int(args[0]) if args else 12
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
FRONT = os.path.join(ROOT, "port/ja1front/target/release/ja1-port")
ISO, EXE = os.path.join(ROOT, "rom/JA1.gog"), os.path.join(ROOT, "build/game113b/JA.EXE")

JAGGED = cd_read("DAT/JAGGED.DAT")
NEWPROF = open(os.path.join(ROOT, "rom", "EDT", "NEWPROF.EDT"), "rb").read()
DATFILE, FAKE_FP, STASH_COUNTS, SAVE_NAME = 0x92F54, 0x7F00, 0x93DD4, 0x28EDFE
SOLDIERS, PROFILES, TBL9, SECTORS, BLOCK, FACES = 0xFA958, 0xEBB40, 0x22979C, 0x222B74, 0x229FA0, 0x93428
OPTIONS, ORDER, NOTES = 0x1163B8, 0x92DF0, 0x92ED2

img = le_load()


def exe_bytes(a, n):
    o = img.obj_at(a)
    return bytes(o.data[a - o.base:a - o.base + n])


e = Emu(dump=dump, fast=True)
rt = e.rt
cur = {"data": b"", "pos": 0}
opened, store, out = [], [], bytearray()


def open_cd(em, eax, edx, ebx, ecx):
    name = e.cstr(eax).decode("latin-1").replace("\\", "/").upper()
    opened.append(name)
    cur["data"], cur["pos"] = (NEWPROF if name == "EDT/NEWPROF.EDT" else cd_read(name)), 0
    e.w32(rt(DATFILE), FAKE_FP)
    return FAKE_FP


def fread(em, eax, edx, ebx, ecx):
    assert ecx == FAKE_FP
    d = cur["data"][cur["pos"]:cur["pos"] + edx * ebx]
    cur["pos"] += len(d)
    e.uc.mem_write(eax, d)
    return len(d) // edx if edx else 0


def fseek(em, eax, edx, ebx, ecx):
    assert ebx == 0
    cur["pos"] = edx
    return 0


def clear_stash(em, *_):
    e.uc.mem_write(rt(STASH_COUNTS), bytes(136))
    store.clear()
    return 0


def put(em, eax, edx, ebx, ecx):
    t = eax & 0xFF
    store.append((t, edx & 0xFF, ebx & 0xFF))
    e.w8(rt(STASH_COUNTS) + t, e.r8(rt(STASH_COUNTS) + t) + 1)
    return 1


def fwrite(em, eax, edx, ebx, ecx):
    out.extend(e.uc.mem_read(eax, edx * ebx))
    return ebx


def save_stash(em, *_):
    out.extend(e.uc.mem_read(rt(STASH_COUNTS), 136))
    # strategic.md "The item store": type ascending, condition descending (signed), a new record after its equals
    for t, q, c in sorted(store, key=lambda r: (r[0], -(r[2] - 256 if r[2] > 127 else r[2]))):
        out.extend((t, q, c))
    return 0


e.stub(rt(0x38304), open_cd)
e.stub(rt(0x758AD), fread)
e.stub(rt(0x757B1), fseek)
e.stub(rt(0x75A77), lambda em, *_: 0)
e.stub(rt(0x38038), lambda em, *_: 0)
e.stub(rt(0x515DC), clear_stash)
e.stub(rt(0x51CB0), put)
e.stub(rt(0x38370), lambda em, *_: FAKE_FP)        # OpenMiscFile (SaveGame)
e.stub(rt(0x75FE3), fwrite)
e.stub(rt(0x51590), save_stash)
e.stub(rt(0x27B98), lambda em, *_: 0, 0)           # "Saving..." message (not shown with EDX = 1)


def clear_block(em, *_):
    """0x37A1A..0x37A38 (rep stosw over the 0x229FA0 block between push es / pop es): Unicorn loses the pushed EDI
    there, so the clear runs here and the stub 'returns' to 0x37A39 (pushed as the return address)."""
    e.uc.mem_write(rt(BLOCK), bytes(0x356))
    sp = e.uc.reg_read(UC_X86_REG_ESP) - 4
    e.uc.reg_write(UC_X86_REG_ESP, sp)
    e.w32(sp, rt(0x37A39))
    return 0


e.stub(rt(0x37A1A), clear_block)
if FAN:
    e.put_115m(only=[r_ for f_ in FAN for r_ in FAN_RUNS[f_]])

# Program start: FUN_000377b0's settings (options), then the EXE's sector order and notes, no save name.
e.stub(rt(0x53564), lambda em, *_: 0)              # srand(time)
e.stub(rt(0x47020), lambda em, *_: 0)              # TimerStart (ftime)
e.call(rt(0x377B0))
e.uc.mem_write(rt(ORDER), exe_bytes(ORDER, 0x3D))
e.uc.mem_write(rt(NOTES), exe_bytes(NOTES, 0x1E))
e.uc.mem_write(rt(SAVE_NAME), bytes(0x1A))
fresh = {a: bytes(e.uc.mem_read(rt(a), n)) for a, n in ((OPTIONS, 0x14), (ORDER, 0x3D), (NOTES, 0x1E))}

r = random.Random(1)


def noise(a, n):
    e.uc.mem_write(rt(a), bytes(r.randrange(256) for _ in range(n)))


notes_before = bytearray()


def original(diff, qs):
    for a, n in ((SOLDIERS, 0x6000), (PROFILES, 64 * 0x5A), (TBL9, 0x21C), (SECTORS, 0x297C), (BLOCK, 0x356)):
        noise(a, n)
    for a, v in fresh.items():
        e.uc.mem_write(rt(a), v)
    if FAN:                              # the game before found some notes (flag bytes 1 or noise)
        for k in range(15):
            e.w8(rt(NOTES) + 2 * k + 1, r.choice([0, 1, 1, r.randrange(256)]))
        e.w8(rt(0x92E2D), r.choice([99, 5, 0x3B]))
    notes_before[:] = e.uc.mem_read(rt(NOTES), 0x1E)
    noise(STASH_COUNTS, 136)
    store[:] = [(1, 1, 1)]
    opened.clear()
    e.call(rt(0x37A0C), diff, qs)
    out.clear()
    e.call(rt(0x4EBC8), SAVE_NAME, 1)
    return bytes(out)


CASES = [(d, q) for d in (0, 1, 2) for q in (0, 1)]
if "--dump-facts" in argv:
    for d in (0, 1, 2):
        s = original(d, 0)
        b = s[20684:20684 + 0x356]
        print(f"difficulty {d}: opened {opened}; block non-zero:",
              " ".join(f"+{i:03X}={b[i]:02X}" for i in range(0x356) if b[i]))
        print("  tbl9:", s[9524:9524 + 540].hex())
        print("  stash tail:", s[21649:].hex())
        print("  options:", s[21538:21558].hex(" "), "len", len(s))
    sys.exit(0)

bad = 0
tmp = tempfile.mkdtemp(prefix="newgame_")
for i in range(ncases):
    d, q = CASES[i] if i < len(CASES) else (r.choice([0, 1, 2]),
                                           r.choice([0, 1, 2, 0xFF, r.randrange(256)]))
    want = original(d, q)
    if len(want) != 21649 + 136 + 3 * len(store):
        print(f"case {i}: original save length {len(want)}")
        bad += 1
        continue
    if PORT:
        p = os.path.join(tmp, f"ng{i}.SAV")
        env = None
        if FAN:                          # the game before: the plain port save with the notes of the original's case
            old = os.path.join(tmp, f"old{i}.SAV")
            subprocess.run([FRONT, ISO, EXE, "--new-game", str(d), str(q), old], capture_output=True, check=True,
                           env=dict(os.environ, JA1_FAN_FIXES="0"))
            b = bytearray(open(old, "rb").read())
            b[21619:21649] = notes_before
            open(old, "wb").write(b)
            env = dict(os.environ, JA1_FAN_FIXES=",".join(f"F{x_}" for x_ in FAN), JA1_NEW_GAME_OLD=old)
            if e.r8(rt(0x92E2D)) != 99:
                print(f"MISMATCH case {i}: the original's 0x92E2D is {e.r8(rt(0x92E2D))}, not 99")
                bad += 1
        rc = subprocess.run([FRONT, ISO, EXE, "--new-game", str(d), str(q), p], capture_output=True, text=True, env=env)
        got = open(p, "rb").read() if rc.returncode == 0 and os.path.exists(p) else b""
        if got != want:
            diffs = [k for k in range(max(len(got), len(want))) if k >= len(got) or k >= len(want) or got[k] != want[k]]
            print(f"MISMATCH case {i} difficulty {d} qs {q}: rc {rc.returncode} {rc.stderr.strip()[:200]} lengths "
                  f"{len(got)}/{len(want)}, {len(diffs)} bytes differ: " +
                  " ".join(f"@{k}:{got[k] if k < len(got) else -1:02x}/{want[k] if k < len(want) else -1:02x}"
                           for k in diffs[:12]))
            bad += 1
print(f"CASES newgame={ncases}")
if PORT:
    # The last 4 profile records (EDT\\NEWPROF.EDT bytes 5400..5760): not in the day save, so the file compare above does
    # not see them. The original's memory after NewGameInit (with noise in them before) vs the port's new world.
    import ctypes
    lib = ctypes.CDLL(os.path.join(ROOT, "port/ja1front/target/release/libja1front.so"))
    lib.ja1f_new_game_tail.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p]
    tail = ctypes.create_string_buffer(360)
    rc = lib.ja1f_new_game_tail(os.path.join(ROOT, "rom/JA1.gog").encode(), os.path.join(ROOT, "build/game113b/JA.EXE").encode(), tail)
    original(0, 0)
    want = bytes(e.uc.mem_read(rt(PROFILES) + 5400, 360))
    if rc != 360 or tail.raw != want:
        print(f"MISMATCH profile records 60..63: rc {rc}, original {want[:16].hex()}, port {tail.raw[:16].hex()}")
        bad += 1
print(f"newgame_diff{' --fan ' + str(FAN) if FAN else ''}: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
