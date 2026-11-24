#!/usr/bin/env python3
"""Unicorn harness (validate/diff/mdi_diff.py) for the Miles AIL 3 FM driver rom/SBPRO1.MDI (16-bit real mode).

Runs the ORIGINAL driver code: loads SBPRO1.MDI at segment DRV_SEG (driver offset == file offset),
calls it the way the AIL 3 library does (INT-66h style entry at the VDI header's this_ISR = 0x132:
AX = function 0x300..0x306 / 0x500..0x506, BX = driver_num (header +0x32), CX/DX/SI/DI = args;
results come back in AX..DI), serves the driver's own DOS file calls (INT 21h open/seek/read/close)
from rom/MADLAB.AD, and logs every OUT to the OPL ports.

Usage:
  run.py [options] "90 3C 7F" "80 3C 00" ...
  Each argument is one MIDI channel message (hex bytes, status first). One argument = one
  MDI_MIDI_XMIT (0x502) call with count 1; its OPL writes get time_index = argument number (1..).
  "a|b|c" in one argument sends several messages in ONE 0x502 call.
Options:
  --io 0x220         I/O base written into the header IO field (default 0x220, GOG SOUND.CFG)
  --init             also print the init writes (DRV_INIT_DEV 0x305 + the AIL channel reset), time -2/-1
  --no-ail-reset     do not send the 15 controller messages per channel that AIL_install_MDI_driver
                     sends (B0 114 0, C0 0, E0 0 40, B0 112 0, B0 1 0, B0 7 127, B0 10 64, B0 11 127,
                     B0 64 0, B0 91 40, B0 93 0, B0 100 0, B0 101 0, B0 38 0, B0 6 2 on every channel)
  --timbres b:p,..   install these (bank:patch) timbres (default: the ones the message list needs:
                     program changes with the current cc114 bank, and (127,key) for channel 10 notes)
  --raw              print one line per OUT (port, value) instead of the decoded register writes
Output lines: time_index chip reg value   (chip L = base+0/1, R = base+2/3, B = base+8/9 both)
"""
import struct, sys
from unicorn import Uc, UC_ARCH_X86, UC_MODE_16, UC_HOOK_INSN, UC_HOOK_INTR
from unicorn.x86_const import *

ROM = __import__('os').path.join(__import__('os').path.dirname(__import__('os').path.abspath(__file__)), '..', '..', 'rom') + '/'
DRV_SEG = 0x1000          # driver CS = DS
STK_SEG = 0x3000
RET_SEG, RET_OFF = 0x0500, 0x0000   # return stub (hlt), emulation stops there

AIL_RESET = [(0xB0, 114, 0), (0xC0, 0, 0), (0xE0, 0, 0x40), (0xB0, 112, 0), (0xB0, 1, 0),
             (0xB0, 7, 127), (0xB0, 10, 64), (0xB0, 11, 127), (0xB0, 64, 0), (0xB0, 91, 40),
             (0xB0, 93, 0), (0xB0, 100, 0), (0xB0, 101, 0), (0xB0, 38, 0), (0xB0, 6, 2)]


class Driver:
    def __init__(self, io=0x220, raw=False):
        self.drv = open(ROM + 'SBPRO1.MDI', 'rb').read()
        self.gtl = open(ROM + 'MADLAB.AD', 'rb').read()
        self.io, self.raw = io, raw
        self.t = 0
        self.log = []                 # (t, chip, reg, value) or raw (t, port, value)
        self.latch = {}               # chip -> register index
        self.opl_timer = 0            # status emulation for the detection in 0x304
        self.files = {}
        self.uc = uc = Uc(UC_ARCH_X86, UC_MODE_16)
        uc.mem_map(0, 0x100000)
        uc.mem_write(DRV_SEG * 16, self.drv)
        uc.mem_write(RET_SEG * 16 + RET_OFF, b'\xf4')
        uc.hook_add(UC_HOOK_INSN, self._in, None, 1, 0, UC_X86_INS_IN)
        uc.hook_add(UC_HOOK_INSN, self._out, None, 1, 0, UC_X86_INS_OUT)
        uc.hook_add(UC_HOOK_INTR, self._intr)

    # ---- port I/O ----
    def _chip(self, port):
        o = port - self.io
        return {0: ('L', 0), 1: ('L', 1), 2: ('R', 0), 3: ('R', 1), 8: ('B', 0), 9: ('B', 1)}.get(o)

    def _in(self, uc, port, size, user):
        c = self._chip(port)
        if c and c[1] == 0:            # status register
            return 0xC0 if self.opl_timer else 0x00
        return 0xFF

    def _out(self, uc, port, size, value, user):
        value &= 0xFF
        if self.raw:
            self.log.append((self.t, port, value)); return
        c = self._chip(port)
        if c is None:
            self.log.append((self.t, 'port%X' % port, -1, value)); return
        chip, isdata = c
        if not isdata:
            self.latch[chip] = value
        else:
            reg = self.latch.get(chip, -1)
            if reg == 4:               # timer control: emulate timer 1 expiring at once
                self.opl_timer = 1 if (value & 0x01) and not (value & 0x80) else 0
            self.log.append((self.t, chip, reg, value))

    # ---- DOS file services the driver uses (0x501 open, 0x503 seek/read, 0x306 close) ----
    def _intr(self, uc, intno, user):
        if intno != 0x21:
            raise RuntimeError('INT %X' % intno)
        ax = uc.reg_read(UC_X86_REG_AX); ah = ax >> 8
        fl = uc.reg_read(UC_X86_REG_EFLAGS) & ~1
        ds = uc.reg_read(UC_X86_REG_DS)
        if ah == 0x3D:
            dx = uc.reg_read(UC_X86_REG_DX)
            name = bytes(uc.mem_read(ds * 16 + dx, 80)).split(b'\0')[0].decode()
            if name.upper().endswith('MADLAB.AD'):
                h = 5 + len(self.files); self.files[h] = 0
                uc.reg_write(UC_X86_REG_AX, h)
            else:
                uc.reg_write(UC_X86_REG_AX, 2); fl |= 1
        elif ah == 0x42:
            bx = uc.reg_read(UC_X86_REG_BX)
            pos = (uc.reg_read(UC_X86_REG_CX) << 16) | uc.reg_read(UC_X86_REG_DX)
            assert (ax & 0xFF) == 0
            self.files[bx] = pos
            uc.reg_write(UC_X86_REG_AX, pos & 0xFFFF); uc.reg_write(UC_X86_REG_DX, pos >> 16)
        elif ah == 0x3F:
            bx = uc.reg_read(UC_X86_REG_BX); cx = uc.reg_read(UC_X86_REG_CX)
            dx = uc.reg_read(UC_X86_REG_DX)
            p = self.files[bx]; data = self.gtl[p:p + cx]
            uc.mem_write(ds * 16 + dx, data); self.files[bx] = p + len(data)
            uc.reg_write(UC_X86_REG_AX, len(data))
        elif ah == 0x3E:
            self.files.pop(uc.reg_read(UC_X86_REG_BX), None)
        else:
            raise RuntimeError('INT 21h AH=%02X' % ah)
        uc.reg_write(UC_X86_REG_EFLAGS, fl)

    # ---- memory helpers (driver segment) ----
    def w16(self, off, v): self.uc.mem_write(DRV_SEG * 16 + off, struct.pack('<H', v & 0xFFFF))
    def r16(self, off): return struct.unpack('<H', self.uc.mem_read(DRV_SEG * 16 + off, 2))[0]
    def rb(self, off, n=1): return bytes(self.uc.mem_read(DRV_SEG * 16 + off, n))
    def wb(self, off, data): self.uc.mem_write(DRV_SEG * 16 + off, bytes(data))

    def call(self, fn, cx=0, dx=0, si=0, di=0):
        """INT-66h style call of the driver entry this_ISR (header +0x34 = 0x132)."""
        uc = self.uc
        isr = self.r16(0x34)
        sp = 0xFFF0
        # iret frame: IP, CS, FLAGS (FLAGS with IF set)
        frame = struct.pack('<HHH', RET_OFF, RET_SEG, 0x0202)
        sp -= 6
        uc.mem_write(STK_SEG * 16 + sp, frame)
        for r, v in ((UC_X86_REG_SS, STK_SEG), (UC_X86_REG_SP, sp), (UC_X86_REG_DS, 0),
                     (UC_X86_REG_ES, 0), (UC_X86_REG_CS, DRV_SEG), (UC_X86_REG_AX, fn),
                     (UC_X86_REG_BX, self.r16(0x32)), (UC_X86_REG_CX, cx), (UC_X86_REG_DX, dx),
                     (UC_X86_REG_SI, si), (UC_X86_REG_DI, di), (UC_X86_REG_BP, 0)):
            uc.reg_write(r, v)
        uc.emu_start(DRV_SEG * 16 + isr, RET_SEG * 16 + RET_OFF, count=50_000_000)
        return tuple(uc.reg_read(r) for r in (UC_X86_REG_AX, UC_X86_REG_BX, UC_X86_REG_CX,
                                                UC_X86_REG_DX))

    # ---- AIL-level operations ----
    def init(self, ail_reset=True):
        self.t = -2
        self.call(0x300)                       # DRV_INIT (header, desc table)
        self.w16(0x16, self.io)                # IO_PARMS: IO, IRQ, DMA_8, DMA_16 (AIL copies them here)
        self.w16(0x18, 0xFFFF); self.w16(0x1A, 0xFFFF); self.w16(0x1C, 0xFFFF)
        ok = self.call(0x304)[0]               # DRV_VERIFY_IO (OPL detection through base+8)
        assert ok == 1, 'verify_IO failed %X' % ok
        self.call(0x305)                       # DRV_INIT_DEV: ports, reg table, state reset
        name = b'MADLAB.AD\0'
        self.wb(0x2F0, name)                   # GTL file name at desc buffer 0x270 + 0x80
        self.call(0x501)                       # MDI_INIT_INS_MGR: opens the GTL file
        self.t = -1
        if ail_reset:
            for ch in range(16):
                self.xmit([(s | ch, a, b) for s, a, b in AIL_RESET])

    def xmit(self, msgs):
        """MDI_MIDI_XMIT 0x502: messages at desc buffer + 0x100 (0x370), CX = count."""
        buf = bytearray()
        for m in msgs:
            buf += bytes(m[:2] if (m[0] & 0xF0) in (0xC0, 0xD0) else m[:3])
        assert len(buf) <= 0x200
        self.wb(0x370, buf)
        self.call(0x502, cx=len(msgs))

    def install(self, pairs):
        """MDI_INSTALL_T_SET 0x503 with a TIMB chunk at 0x370 (patch, bank pairs); the driver reads
        the GTL file itself. Returns the driver's AX (1 = all installed, 0 = one missing)."""
        body = struct.pack('<H', len(pairs)) + b''.join(bytes([p, b]) for b, p in pairs)
        self.wb(0x370, b'TIMB' + struct.pack('>I', len(body)) + body)
        return self.call(0x503)[0]

    def t_status(self, bank, patch):
        return self.call(0x504, cx=(bank << 8) | patch)[0]


def parse_msg(s):
    return [tuple(int(x, 16) for x in part.split()) for part in s.split('|')]


def needed_timbres(groups):
    bank = [0] * 16; out = []
    for g in groups:
        for m in g:
            st, ch = m[0] & 0xF0, m[0] & 0xF
            if st == 0xB0 and m[1] == 114: bank[ch] = m[2]
            elif st == 0xC0: out.append((bank[ch], m[1]))
            elif st == 0x90 and ch == 9 and m[2]: out.append((127, m[1]))
    seen = []
    for x in out:
        if x not in seen: seen.append(x)
    return seen


def main(argv):
    io, show_init, ail_reset, timbres, raw, msgs = 0x220, False, True, None, False, []
    it = iter(argv)
    for a in it:
        if a == '--io': io = int(next(it), 0)
        elif a == '--init': show_init = True
        elif a == '--no-ail-reset': ail_reset = False
        elif a == '--raw': raw = True
        elif a == '--timbres':
            timbres = [tuple(int(v, 0) for v in x.split(':')) for x in next(it).split(',')]
        elif a in ('-h', '--help'): print(__doc__); return
        else: msgs.append(parse_msg(a))
    d = Driver(io, raw)
    d.init(ail_reset)
    if timbres is None: timbres = needed_timbres(msgs)
    if timbres:
        d.t = 0
        r = d.install(timbres)
        print('# install %s -> %d' % (' '.join('%d:%d' % x for x in timbres), r))
    for i, g in enumerate(msgs, 1):
        d.t = i
        d.xmit(g)
    for e in d.log:
        if e[0] < 0 and not show_init: continue
        if raw: print('%d %X %02X' % e)
        else: print('%d %s %02X %02X' % e)


if __name__ == '__main__':
    main(sys.argv[1:])
