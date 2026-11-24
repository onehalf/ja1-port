#!/usr/bin/env python3
"""Differential test: which music file a load takes (PlayXmiMusic 0x5C4C8, docs/spec/music.md "Files").

usage: .venv/bin/python validate/diff/music_file_diff.py [--lib SO]
The original's body runs in Unicorn on the loaded image with the driver byte 0x9639A and the General MIDI byte 0x9639B set per
case. Stubs: FileExists 0x4707C (real game folder build/game113b, any case), OpenCDFile 0x38304, FileSize 0x5273C,
fread_, fclose_; each logs its call. Names: every argument of the six callers (MISC\\INTRO, MISC\\intro, MISC\\menus,
and MISC\\MENUS + MISC\\DAY of the film scripts), plus a missing file.
The port: ja1f_music_file = Music::load, the code the window runs. Driver on + FM form: the short name and the byte count
must equal the original's fread_. The port has only the FM driver (the form the GOG install plays), so the GM byte = 1
and driver byte = 0 cases are checked on the original side only. A missing file: the original goes to the CD; the GOG CD
has no MISC folder, so a player cannot reach that with the target files, and the port must load nothing. (The CD path
has no ".xmi" in the asm: the original's CD path could never find such a file either.)
"""
import ctypes
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, STACK_TOP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_music_file.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p]

GAME = os.path.join(ROOT, "build", "game113b")
PLAY_XMI, FILE_EXISTS, OPEN_CD, FILE_SIZE, FREAD, FCLOSE = 0x5C4C8, 0x4707C, 0x38304, 0x5273C, 0x758AD, 0x75A77
DRIVER, GM, DEST, DAT_FILE = 0x9639A, 0x9639B, 0x32F1D8, 0x92F54
NAMES = [b"MISC\\INTRO", b"MISC\\intro", b"MISC\\menus", b"MISC\\MENUS", b"MISC\\DAY", b"MISC\\day", b"MISC\\nofile"]


def on_disk(name):
    """The game-folder file of a DOS name, any case; None when missing."""
    d = GAME
    parts = name.decode().split("\\")
    for i, p in enumerate(parts):
        hit = [x for x in os.listdir(d) if x.upper() == p.upper()]
        if not hit:
            return None
        d = os.path.join(d, hit[0])
    return d if os.path.isfile(d) else None


def orig(e, name, driver, gm):
    log = []

    def exists(em, eax, edx, ebx, ecx):
        n = em.cstr(eax)
        log.append(("exists", n))
        return 1 if on_disk(n) else 0

    def open_cd(em, eax, edx, ebx, ecx):
        log.append(("cd", em.cstr(eax)))

    def size(em, eax, edx, ebx, ecx):
        n = em.cstr(eax)
        p = on_disk(n)
        log.append(("size", n, ebx))
        return os.path.getsize(p) if p else 1234

    def fread(em, eax, edx, ebx, ecx):
        log.append(("read", eax, edx, ebx, ecx))
        return ebx

    e.stub(FILE_EXISTS, exists)
    e.stub(OPEN_CD, open_cd)
    e.stub(FILE_SIZE, size)
    e.stub(FREAD, fread)
    e.stub(FCLOSE, lambda em, eax, edx, ebx, ecx: log.append(("close", eax)))
    e.uc.mem_write(DRIVER, bytes([driver]))
    e.uc.mem_write(GM, bytes([gm]))
    e.uc.mem_write(DAT_FILE, (0x5AA55).to_bytes(4, "little"))
    arg = STACK_TOP - 0x60
    e.uc.mem_write(arg, name + b"\0")
    e.call(PLAY_XMI, arg)
    return log


def main():
    e = Emu(fast=True)
    bad = cases = cd_seen = gm_seen = 0
    exe = os.path.join(GAME, "JA.EXE").encode()
    for name in NAMES:
        for driver in (0, 1):
            for gm in (0, 1):
                log = orig(e, name, driver, gm)
                cases += 1
                if not driver:
                    if log:
                        print("FAIL no driver, the original opens a file", name, log)
                        bad += 1
                    continue
                want_name = name + (b"gm.xmi" if gm else b".xmi")
                if log[0] != ("exists", want_name):
                    print("FAIL first look", name, gm, log[0])
                    bad += 1
                reads = [x for x in log if x[0] == "read"]
                if len(reads) != 1 or reads[0][1] != DEST or reads[0][3] != 1 or reads[0][4] != 0x5AA55 or log[-1] != ("close", 0x5AA55):
                    print("FAIL read", name, gm, log)
                    bad += 1
                    continue
                missing = on_disk(want_name) is None
                if missing:
                    cd_seen += 1
                    cd = [x for x in log if x[0] == "cd"]
                    if not cd or not cd[0][1].endswith(b":\\" + name):
                        print("FAIL no CD path", name, gm, log)
                        bad += 1
                if gm:
                    gm_seen += 1
                    continue
                base = ctypes.create_string_buffer(32)
                n = PORT.ja1f_music_file(exe, name, base)
                want = (b"", 0) if missing else (name.split(b"\\")[-1].upper(), reads[0][2])
                if (base.value, n) != want:
                    print(f"FAIL {name}: port {base.value} {n} bytes, original {want}")
                    bad += 1
    print(f"music_file_diff: {cases} cases, {cases - bad} equal, {bad} mismatches (CD path {cd_seen}, GM form {gm_seen})")
    sys.exit(1 if bad or not cd_seen or not gm_seen else 0)


if __name__ == "__main__":
    main()
