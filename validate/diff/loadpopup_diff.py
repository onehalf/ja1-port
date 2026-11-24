#!/usr/bin/env python3
"""Differential test: the drown / injection popup (host op 71, LoadPopups? 0x288DC), original vs port.

usage: .venv/bin/python validate/diff/loadpopup_diff.py [ncases=8] [DUMP] [--port] [--lib SO] [--seed S]
Per case: a slot 0..7, flag 0 (stab) or 1 (drown), the selected slot 0x284F11, a noise screen, and a soldier record
(g_soldiers + slot x 0x200) whose +0x17A = slot. The original runs in Unicorn on tac60: OpenCDFile / fseek / fread
read DAT\\POPUPS.DAT through common.game_read; LoadSanimDat 0x5E8E0 is stubbed (records DAT\\STAB.DAT / DAT\\DROWN.DAT);
ReadPcx 0x532BC is stubbed (decodes FRAME.PCC / FRAMEG.PCC from MAIN.DAT and STABnn.PCC / DROWNnn.PCC from the
archive); sprintf is Python; BlitCopy 0x52AE0 and ScaledBlit 0x52B7C run for real. A code hook at 0x28BF2 (after each
ScaledBlit) clears the 1 ms timer 0x92EF8 and records the frame rect (x, y, w, h), the 120 x 60 source block and the
whole 320 x 200 screen. Dissolve 0x52C98, AnimDelay 0x15BB8, FormatSectorDatName 0x2F7C8 and HideMouse 0x67368 are
stubbed.
--port: ja1f_popup (the frames into a buffer) + ja1f_popup_draw (frame i onto the screen) from libja1front.so, from
the same noise screen; the test compares the frame count, every rect, every source block and every screen after each
frame. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn import UC_HOOK_CODE     # noqa: E402
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_EBP, UC_X86_REG_EDI,   # noqa: E402
                               UC_X86_REG_ESI, UC_X86_REG_ESP)
import datarc, pcx                   # noqa: E402
from common import game_read         # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 8
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
seed = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 7
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_popup.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] * 3 + [ctypes.c_void_p, ctypes.c_int32]
    PORT.ja1f_popup.restype = ctypes.c_int32
    PORT.ja1f_popup_draw.argtypes = [ctypes.c_void_p, ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_popup_draw.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
ARCS = {}
LOAD_POPUPS, SCREEN, BUF, DATFILE = 0x288DC, 0x95678, 0x93450, 0x92F54
SEL, LEGACY, TIMER, ARC_HANDLE = 0x284F11, 0x92F90, 0x92EF8, 0x92F48
SOLDIERS, REC = 0xFA958, 0x200
FAKE_FP, FAKE_ARC = 0x7777, 0x8888
SEP = 16 + 120 * 60                # one ja1f_popup frame record
W, H = 120, 60
e = Emu(dump=dump, fast=True)
cur, files = {}, {}


def load_pcx(em, eax, edx, ebx, ecx):
    name = e.cstr(ebx).decode("latin-1").upper()
    pitch = e.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    data = MAIN if edx == e.rt(0x10AD58) else ARCS[cur["arc"]]
    w, h, px, _ = pcx.decode(datarc.member(data, name))
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def load_sanim(em, eax, edx, ebx, ecx):
    name = e.cstr(eax).decode("latin-1").upper()
    cur["arc"] = "DROWN.DAT" if "DROWN" in name else "STAB.DAT"
    if cur["arc"] not in ARCS:
        ARCS[cur["arc"]] = open(os.path.join(ROOT, "rom", "DAT", cur["arc"]), "rb").read()
    e.w32(e.rt(ARC_HANDLE), FAKE_ARC)
    e.w8(e.rt(LEGACY), 1)
    return 0


def open_cd(em, eax, edx, ebx, ecx):
    name = e.cstr(eax).decode("latin-1").upper()
    if name not in files:
        files[name] = game_read(name)
    cur["data"], cur["pos"] = files[name], 0
    e.w32(e.rt(DATFILE), FAKE_FP)
    return FAKE_FP


def fseek(em, eax, edx, ebx, ecx):
    assert eax == FAKE_FP and ebx == 0
    cur["pos"] = edx
    return 0


def fread_(em, eax, edx, ebx, ecx):
    assert ecx == FAKE_FP
    d = cur["data"][cur["pos"]:cur["pos"] + edx * ebx]
    cur["pos"] += len(d)
    em.uc.mem_write(eax, d)
    return len(d) // edx if edx else 0


def pre_hit(uc, addr, size, _):
    """at the ScaledBlit call: EAX dst, EAX = screen + x + y*320 (x is added to EAX before the call)"""
    if cur.get("snaps") is None:
        return
    sp = uc.reg_read(UC_X86_REG_ESP)
    eax = uc.reg_read(UC_X86_REG_EAX)
    w, h = uc.reg_read(UC_X86_REG_ESI), uc.reg_read(UC_X86_REG_EDI)
    cy = e.r32(sp + 0x80)                      # the cy local (S2 + 0x70) is 16 bytes above the four pushes
    y = cy - h // 2
    cur["dst"], cur["rect"] = eax, (eax - e.r32(e.rt(SCREEN)) - y * 320, y, w, h)


def frame_hit(uc, addr, size, _):
    """after ScaledBlit: clear the 1 ms timer (the spin would hang here) and record the frame"""
    if cur.get("snaps") is None:
        return
    e.w32(e.rt(TIMER), 0)
    cur["snaps"].append(cur["rect"] + (bytes(uc.mem_read(e.r32(e.rt(BUF)), W * H)),
                                       bytes(uc.mem_read(e.r32(e.rt(SCREEN)), 320 * 200))))


e.stub(e.rt(0x532BC), load_pcx, 4)
e.stub(e.rt(0x5E8E0), load_sanim)
e.stub(e.rt(0x38304), open_cd)
e.stub(e.rt(0x757B1), fseek)
e.stub(e.rt(0x758AD), fread_)
e.stub(e.rt(0x75A77), lambda em, *_: 0)
e.stub_sprintf()
e.stub(e.rt(0x67368), lambda em, *_: 0)          # HideMouse
e.stub(e.rt(0x52C98), lambda em, *_: 0, 12)      # Dissolve (restores the saved block)
e.stub(e.rt(0x15BB8), lambda em, *_: 0)          # AnimDelay
e.stub(e.rt(0x2F7C8), lambda em, *_: 0)          # FormatSectorDatName
e.uc.hook_add(UC_HOOK_CODE, pre_hit, None, e.rt(0x28BED), e.rt(0x28BED))
e.uc.hook_add(UC_HOOK_CODE, frame_hit, None, e.rt(0x28BF2), e.rt(0x28BF2))
e.uc.ctl_flush_tb()


def slots():
    """the dump's g_slotPos must be the port's slot_origin table: check it once"""
    want = [(0, 14), (0, 56), (0, 98), (0, 140), (272, 14), (272, 56), (272, 98), (272, 140)]
    got = [(e.r16(e.rt(0x93686 + 4 * k)), e.r16(e.rt(0x93688 + 4 * k))) for k in range(8)]
    assert got == want, f"g_slotPos {got} != slot_origin {want}"


slots()
r = random.Random(seed)
bad, seen = 0, set()
buf = ctypes.create_string_buffer(4 + 64 * SEP)
for case in range(ncases):
    slot = case % 8
    flag = (case // 8) % 2
    selected = slot if case % 3 else (slot + 1) % 8
    noise = bytes(r.randrange(256) for _ in range(320 * 200))
    sold = e.rt(SOLDIERS) + slot * REC
    e.w8(sold + 0x17A, slot)
    e.w32(e.rt(SEL), selected)
    e.w32(sold + 0xD0, r.getrandbits(32))
    e.w32(sold + 0xB0, r.getrandbits(32))
    e.uc.mem_write(e.r32(e.rt(SCREEN)), noise)
    cur["snaps"] = []
    e.call(e.rt(LOAD_POPUPS), sold, flag, 0, 0)
    snaps, cur["snaps"] = cur["snaps"], None
    got = len(snaps)
    if PORT is None:
        print(f"case {case}: slot {slot} flag {flag} sel {selected}: {got} frames, first {snaps[0][:4] if got else '-'}")
        continue
    n = PORT.ja1f_popup(ISO, EXE, slot, selected, flag, buf, len(buf))
    if n < 0:
        print(f"case {case}: ja1f_popup error {n}")
        bad += 1
        continue
    cnt = int.from_bytes(buf.raw[:4], "little")
    ok = cnt == got
    for i in range(min(cnt, got)):
        sx, sy, sw, sh = (int.from_bytes(buf.raw[4 + i * SEP + 4 * k:8 + i * SEP + 4 * k], "little") for k in range(4))
        src = buf.raw[4 + i * SEP + 16:4 + (i + 1) * SEP]
        x, y, w, h, osrc, oscr = snaps[i]
        if (sx, sy, sw, sh) != (x, y, w, h):
            print(f"case {case} slot {slot} flag {flag} frame {i}: rect port {(sx, sy, sw, sh)} orig {(x, y, w, h)}")
            ok = False
            break
        if src != osrc:
            d = [k for k in range(len(src)) if src[k] != osrc[k]]
            print(f"case {case} slot {slot} flag {flag} frame {i}: src differs at {len(d)} bytes, first {d[:4]}")
            ok = False
            break
    # draw the port frames one by one on its own screen, compare every snapshot
    scr = ctypes.create_string_buffer(noise, 320 * 200)
    for i in range(min(cnt, got)):
        if PORT.ja1f_popup_draw(buf, len(buf), i, scr) != 0:
            print(f"case {case}: ja1f_popup_draw({i}) failed")
            ok = False
            break
        if scr.raw != snaps[i][5]:
            d = [k for k in range(64000) if scr.raw[k] != snaps[i][5][k]]
            print(f"case {case} slot {slot} flag {flag} frame {i}: screen differs at {len(d)} bytes, first {d[:6]}")
            ok = False
            break
    if cnt != got:
        print(f"case {case} slot {slot} flag {flag}: port {cnt} frames, original {got}")
    if not ok:
        bad += 1
        if bad > 3:
            break
    seen.add((slot, flag, selected == slot))

print(f"loadpopup_diff: {ncases} cases, {len(seen)} slot/flag/selected kinds, {bad} mismatches")
sys.exit(1 if bad else 0)
