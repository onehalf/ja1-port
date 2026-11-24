"""Differential test: the SAB script player (docs/spec/ending.md "SAB scripts"), original 0x5D2D4 vs port.

usage: .venv/bin/python validate/diff/sab_diff.py [ARCHIVE MEMBER]... [DUMP] [--port] [--subtitles] [--log] [--png DIR]
Default scripts: DAT\\ENDSAB.DAT LOS1GAME.SAB, LOS2GAME.SAB, WINGAME.SAB. The original runs in Unicorn on tac60:
0x5D0C8 (ARCHIVE, MEMBER) = open the script (0x5D0EC) and play it (0x5D2D4). Stubs: OpenFileOrCD 0x53338 + the clib
file calls (files from rom/ or the CD); the game timer (every wait is over at once); KeyPressed = no key; the VGA
palette calls (logged); the sound, voice, music and AIL calls (logged with their arguments); the FLC player 0x5BFF8
(logged). At the top of each script step the harness logs the record's op and a CRC-32 (zlib) of the screen and the first
64000 bytes of work buffer 1. --port: `ja1front ISO EXE --sab ARCHIVE MEMBER` must print the same log. Exits 1 on any
mismatch. --subtitles: the Subtitles option on (byte 0x1163BD = 1; port: JA1_SAB_SUBTITLES=1), so op 29 draws the
INTQUOTE texts (OpenCDFile 0x38304 stubbed with the file from rom/ or the CD). --png DIR writes the screen at each wait step (op 17, 23, 36) as PNG for a look.
"""
import os, zlib, struct, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop          # noqa: E402
from common import cd_read            # noqa: E402

argv = sys.argv[1:]
args = [a for a in argv if not a.startswith("--")]
PORT, LOG, SUBTITLES = "--port" in argv, "--log" in argv, "--subtitles" in argv
OP35 = "--op35" in argv        # a crafted script for op 0x35 (MovePaletteEntry + the DAC copy)
PNG = argv[argv.index("--png") + 1] if "--png" in argv else None
if PNG:
    args.remove(PNG)
dump = next((a for a in args if a.endswith(".bin")), os.path.join(ROOT, "build", "mem_tac60.bin"))
pairs = [a for a in args if not a.endswith(".bin")]
SCRIPTS = list(zip(pairs[::2], pairs[1::2])) or [("DAT\\ENDSAB.DAT", m) for m in
                                                 ("LOS1GAME.SAB", "LOS2GAME.SAB", "WINGAME.SAB")]
FRONT = os.path.join(ROOT, "port/ja1front/target/release/ja1front")
ISO, EXE = os.path.join(ROOT, "rom/JA1.gog"), os.path.join(ROOT, "build/game113b/JA.EXE")
SCREEN_PTR, BUFS = 0x95678, 0x93E80      # screen pointer; work buffers: 4 x (ptr, w, h)
DAC_COPY = 0xEDA08                       # the DAC copy (the tile palette, RGBX) that op 0x35 shows


def archive(entries):
    """A Sir-tech archive: the count at 0x14 (the entries plus an empty terminator), 0x20-byte entries at 0x20
    (name 16, offset, size). ArcSeek 0x534A4 searches the entries 1..count-1."""
    at = 0x20 + 0x20 * (len(entries) + 1)
    head = bytearray(at)
    head[0x14:0x18] = struct.pack("<I", len(entries) + 1)
    for i, (name, data) in enumerate(entries):
        e = 0x20 + 0x20 * i
        head[e:e + 16] = name.encode().ljust(16, b"\0")[:16]
        head[e + 16:e + 24] = struct.pack("<II", at, len(data))
        at += len(data)
    return bytes(head) + b"".join(d for _, d in entries)


def record(op, a0=0, name="", w=()):
    """A 44-byte SAB record (ending.md "Script records")."""
    b = bytearray(44)
    b[0:4] = struct.pack("<Hh", op, a0)
    nm = name.encode()[:18]
    b[4:4 + len(nm)] = nm
    for k, v in enumerate(w, 1):
        b[0x18 + 2 * (k - 1):0x1A + 2 * (k - 1)] = struct.pack("<h", v)
    return bytes(b)


def op35_setup():
    """Craft the archives and the DAC copy for op 0x35; write the archives beside the port's exe.

    Script: op 54 opens the graphics archive, op 41 fills palette slot 0 from the member "GFX" (768 palette bytes
    behind the 0x0C marker, each >> 2), op 0x35 moves entries (down, to entry 0, to the same place, up with no shift, to the last entries) and shows DAC
    colours 0xA5..0xB2 of 0xEDA08, op 8 shows the whole slot (so the moves are in the log), a last op 0x35 leaves the
    DAC copy on the screen (the line "dac" holds the VGA colours at the end), op 99 ends."""
    global DAC, SCRIPTS
    DAC = bytes((0x40 + 7 * i) & 0xFF for i in range(1024))
    pal = bytes((i * 37) & 0xFF for i in range(768))
    gfx = archive([("GFX", b"\x0c" + pal)])
    script = archive([("TEST.SAB", b"".join((
        record(0x36, 0, "SAB35G.DAT"),
        record(0x29, 0, "GFX"),
        record(0x35, 0, "", (10, 20)),
        record(0x35, 0, "", (0, 5)),
        record(0x35, 0, "", (7, 7)),
        record(0x35, 0, "", (30, 25)),
        record(0x35, 0, "", (254, 255)),
        record(0x08, 0, "", (0, 255)),
        record(0x35, 0, "", (1, 2)),
        record(99))))])
    FIXTURES.update({"SAB35.DAT": script, "SAB35G.DAT": gfx})
    dir_ = os.path.dirname(EXE)
    os.makedirs(dir_, exist_ok=True)
    for name, data in FIXTURES.items():
        with open(os.path.join(dir_, name), "wb") as f:
            f.write(data)
    dac_path = os.path.join(ROOT, "build", "sab35_dac.bin")
    with open(dac_path, "wb") as f:
        f.write(DAC)
    SCRIPTS = [("SAB35.DAT", "TEST.SAB")]
    return dac_path


FIXTURES = {}    # name -> bytes: the crafted archives of --op35 (the file is not on disk for the original)
DAC = b""        # the DAC copy 0xEDA08 seeded for --op35


def game_file(name):
    if name in FIXTURES:
        return FIXTURES[name]
    rel = name.replace("\\", "/").split(":")[-1].lstrip("/")
    p = os.path.join(ROOT, "rom", rel)
    if os.path.exists(p):
        return open(p, "rb").read()
    return cd_read(rel)


def run_original(archive, member):
    e = Emu(dump=dump, fast=True)
    rt = e.rt
    if OP35:
        e.uc.mem_write(rt(DAC_COPY), DAC)
    files, log = {}, []
    nxt = [0x7F00]

    def new_fp(data):
        fp = nxt[0]
        nxt[0] += 0x40
        files[fp] = [data, 0]
        return fp

    def cstr(a):
        return bytes(e.uc.mem_read(a, 64)).split(b"\0")[0].decode("latin-1")

    def open_arc(em, eax, edx, ebx, ecx):
        name = cstr(eax)
        fp = new_fp(game_file(name))
        e.w32(edx, fp)
        d = files[fp]
        e.uc.mem_write(ebx, d[0][:ecx * 32])
        d[1] = ecx * 32
        log.append(f"open {name}")
        return 0

    def fread(em, eax, edx, ebx, ecx):
        d = files[ecx]
        b = d[0][d[1]:d[1] + edx * ebx]
        d[1] += len(b)
        e.uc.mem_write(eax, b)
        return len(b) // edx if edx else 0

    def fseek(em, eax, edx, ebx, ecx):
        d = files[eax]
        d[1] = {0: edx, 1: d[1] + edx, 2: len(d[0]) + edx}[ebx]
        return 0

    def fgetc(em, eax, *_):
        d = files[eax]
        if d[1] >= len(d[0]):
            return 0xFFFFFFFF
        d[1] += 1
        return d[0][d[1] - 1]

    def getpos(em, eax, edx, *_):
        e.w32(edx, files[eax][1])
        return 0

    def setpos(em, eax, edx, *_):
        files[eax][1] = e.r32(edx)
        return 0

    def open_cd(em, eax, *_):
        # OpenCDFile 0x38304 (EAX name): PlayQuote's INTQUOTE read for op 29 with Subtitles on; g_datFile = the file
        # (returns 0 when the file opened, -1 when not; the voice cue reader 0x5EDE0 of op 59 exits on -1)
        data = game_file(cstr(eax))
        if "INTQUOTE" not in cstr(eax).upper():
            log.append(f"open_cd {cstr(eax)}")
        if data is None:
            e.w32(rt(0x92F54), 0)
            return 0xFFFFFFFF
        e.w32(rt(0x92F54), new_fp(data))
        return 0

    e.stub(rt(0x53338), open_arc, 4)
    e.stub(rt(0x38304), open_cd)
    if SUBTITLES:
        e.w8(rt(0x1163BD), 1)
    e.stub(rt(0x758AD), fread)
    e.stub(rt(0x757B1), fseek)
    e.stub(rt(0x7C689), lambda em, eax, *_: files[eax][1])          # ftell_
    e.stub(rt(0x7648C), fgetc)
    e.stub(rt(0x7C66A), getpos)
    e.stub(rt(0x7C67D), setpos)
    e.stub(rt(0x75A77), lambda em, *_: 0)                            # fclose_
    e.stub(rt(0x47020), lambda em, *_: 0)                            # TimerStart
    e.stub(rt(0x47040), lambda em, *_: 0x7FFFFFFF)                   # TimerElapsedMs: every wait is over
    e.stub(rt(0x5616C), lambda em, *_: 0)                            # KeyPressed

    dac = bytearray(1024)                                            # the VGA colours (RGBX, 6 bits)

    def pal(em, eax, edx, ebx, ecx):
        # PaletteSet 0x52870: l - f + 1 colours from palette + 4 f (past its end too); the colour index wraps at 256
        f, l = edx & 0xFFFF, ebx & 0xFFFF
        p = bytes(e.uc.mem_read(eax + 4 * f, 4 * (l - f + 1)))
        for i in range(l - f + 1):
            j = (f + i) & 0xFF
            dac[4 * j:4 * j + 3] = p[4 * i:4 * i + 3]
        log.append(f"palette {f} {l} {zlib.crc32(p):08x}")
        return 0

    def fade(em, eax, edx, ebx, ecx):
        """0x52908 (EAX palette, EDX, EBX steps, ECX from, [ESP+4] to): the colours go from `from`/256 to `to`/256
        of the palette; the harness keeps the end colours."""
        from unicorn.x86_const import UC_X86_REG_ESP
        to = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
        p = bytes(e.uc.mem_read(eax, 1024))
        dac[:] = bytes(c * to // 256 for c in p)
        log.append(f"fade {edx} {ebx} {ecx} {to} {zlib.crc32(p):08x}")
        return 0

    e.stub(rt(0x52870), pal)
    e.stub(rt(0x52908), fade, 4)

    def logger(tag, fmt):
        def f(em, eax, edx, ebx, ecx):
            log.append(tag + " " + fmt(eax, edx, ebx, ecx))
            return 0
        return f

    s = cstr
    i16 = lambda v: (v & 0xFFFF) - 0x10000 * ((v & 0xFFFF) >= 0x8000)  # noqa: E731
    e.stub(rt(0x5C960), logger("voc_load", lambda a, d, b, c: s(a)))
    e.stub(rt(0x5C9C0), logger("voc_play", lambda a, d, b, c: f"{a} {d & 0xFF} {b}"))
    e.stub(rt(0x5CA98), logger("sfx_load", lambda a, d, b, c: f"{s(a)} {d}"))
    e.stub(rt(0x5CB34), logger("sfx_play", lambda a, d, b, c: f"{a} {d} {b & 0xFF} {c}"))
    e.stub(rt(0x5CC6C), logger("speech", lambda a, d, b, c: f"{a} {d}"))
    e.stub(rt(0x5C4C8), logger("music_load", lambda a, d, b, c: s(a)))
    e.stub(rt(0x5C5EC), logger("music", lambda a, d, b, c: f"{i16(a)} {d}"))
    e.stub(rt(0x5BFF8), logger("flc", lambda a, d, b, c: f"{s(a)} {d} {b} {c & 0xFF}"), 4)
    e.stub(rt(0x5CCF8), lambda em, *_: 0)                            # speech buffer feed
    e.stub(rt(0x5CDD8), logger("speech_stop", lambda *_: ""))
    e.stub(rt(0x5CE7C), logger("voc_stop", lambda *_: ""))
    e.stub(rt(0x66D94), lambda em, *_: 0)
    for a, n in ((0x786F6, "ail_end_sample"), (0x79FA9, "ail_end_sequence"), (0x7A098, "ail_seq_volume"),
                 (0x787D9, "ail_sample_volume"), (0x78763, "ail_sample_rate")):
        e.stub(rt(a), lambda em, *_: 0)
    for name in ("OFF",):
        pass

    ctx = {"n": 0}
    rec = rt(0x337914)

    def top(uc, addr, size, _):
        op, a0 = struct.unpack("<Hh", bytes(uc.mem_read(rec, 4)))
        scr = e.r32(rt(SCREEN_PTR))
        h = zlib.crc32(bytes(uc.mem_read(scr, 64000)))
        # Buffer 1: its first 64000 bytes (rows 0..199) only. The scripts never write rows 200..302, and the original's
        # memory there is not all buffer (the PCX loader keeps its data at the end); buffer 2 is not used.
        h = zlib.crc32(bytes(uc.mem_read(e.r32(rt(BUFS) + 8), 64000)), h)
        log.append(f"step {ctx['n']} op {op} {a0} {h:08x}")
        ctx["n"] += 1
        if PNG and op in (17, 23, 36):
            write_png(e, f"{PNG}/{member}_{ctx['n']:05d}.png", scr, dac)
    e.uc.hook_add(__import__('unicorn').UC_HOOK_CODE, top, None, rt(0x5D2FF), rt(0x5D2FF))

    # A fixed start: the screen and the four work buffers are black (in the game they hold the last screen).
    e.uc.mem_write(e.r32(rt(SCREEN_PTR)), bytes(64000))
    for k in range(4):
        p, w, hh = struct.unpack("<IHH", bytes(e.uc.mem_read(rt(BUFS) + 8 * k, 8)))
        if p and w and hh:
            e.uc.mem_write(p, bytes(w * hh))
    a = e.alloc(archive.encode() + b"\0")
    m = e.alloc(member.encode() + b"\0")
    try:
        r = e.call(rt(0x5D0C8), a, m, max_steps=2_000_000_000)
        if OP35:
            log.append(f"dac {zlib.crc32(bytes(dac)):08x}")
        log.append(f"end {r}")
    except EmuStop as x:
        log.append(f"STOP {x}")
    return log


def write_png(e, path, scr, pal):
    import pcx
    os.makedirs(os.path.dirname(path), exist_ok=True)
    pix = bytes(e.uc.mem_read(scr, 64000))
    pcx.png(path, 320, 200, pix, bytes(min(255, pal[4 * i + j] * 4) for i in range(256) for j in range(3)))


def main():
    bad = 0
    dac_path = op35_setup() if OP35 else None
    for arc, mem in SCRIPTS:
        o = run_original(arc, mem)
        if o[-1] != "end 1":
            bad += 1
            print(f"{mem}: the original did not play to the end: {o[-1]}")
            if LOG:
                print("\n".join(o[-12:]))
            continue
        if LOG:
            print("\n".join(o))
        if PORT:
            env = {**os.environ, "JA1_SAB_SUBTITLES": "1" if SUBTITLES else "0"}
            if dac_path:
                env["JA1_SAB_DAC"] = dac_path
            p = subprocess.run([FRONT, ISO, EXE, "--sab", arc, mem], capture_output=True, text=True,
                               env=env).stdout.split("\n")
            p = [x for x in p if x]
            n = next((i for i, (x, y) in enumerate(zip(o, p)) if x != y), None)
            if n is None and len(o) != len(p):
                n = min(len(o), len(p))
            if n is not None:
                bad += 1
                print(f"{mem}: line {n}: orig {o[n] if n < len(o) else '-'} | port {p[n] if n < len(p) else '-'}")
                continue
        print(f"{mem}: {len(o)} lines, {sum(x.startswith('step') for x in o)} steps, last {o[-1]}")
    print("sab_diff:", "FAIL" if bad else "ok")
    sys.exit(1 if bad else 0)


main()
