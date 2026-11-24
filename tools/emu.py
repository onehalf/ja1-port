"""Unicorn harness: run functions of the ORIGINAL JA.EXE (fixed-up LE image) in isolation.

usage: ./t emu <addr|label> [arg ...] [--cdecl] [--trace] [--max N]
  default convention is Watcom register (__watcall): args 1-4 in EAX, EDX, EBX, ECX, rest on stack.
  --cdecl pushes all args on the stack (AIL/Miles and some Watcom library code use this).
Prints EAX (return value) and the instruction count. INT n / port I/O stop the run and are reported.

As a module (for differential tests against a port):
    from emu import Emu
    e = Emu(); e.w32(0x339368 + 4*3, 7); r = e.call(0x7C149, 3, 9, cdecl=True); e.r32(0x339368 + 12)
    e.stub(0x19F30, lambda e, a, d, b, c: calls.append((a, d, b)))   # replace a callee; returns its EAX (None = 0)
    e = Emu(dump="build/mem_NAME.bin"); e.call(e.rt(0x169E8), 688); e.r8(e.rt(0x22A4EF))   # replay on a snapshot
"""
import ctypes, os, struct, sys
from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_HOOK_INTR, UC_HOOK_INSN
from unicorn.unicorn_py3.unicorn import uclib
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
                               UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI,
                               UC_X86_REG_EIP, UC_X86_INS_IN, UC_X86_INS_OUT)
from le import load
from common import load_labels, parse_addr, disassembler

STACK_TOP = 0x0F000000
STACK_SIZE = 0x100000
RET_SENTINEL = 0x0E000000     # mapped page; reaching it means the called function returned
HEAP = 0x0D000000             # scratch memory for tests (Emu.alloc)
PAGE = 0x1000
COV_FILE = os.environ.get("JA1_EMU_COV")
COV_SEEN = set()
LIB_START = 0x754F0           # tools/status.py: first library module


def reg_get(uc, reg):
    """uc.reg_read for the general registers, without its register class search per call (a stub call does 5 reads;
    xp_diff makes 3 million stub calls). Pins unicorn 2.1.4's private names; validate/diff/emureg_diff.py checks them."""
    v = ctypes.c_uint64()
    status = uclib.uc_reg_read(uc._uch, reg, ctypes.byref(v))
    if status:
        raise UcError(status, reg)
    return v.value


def reg_set(uc, reg, value):
    """uc.reg_write for the general registers (see reg_get)"""
    status = uclib.uc_reg_write(uc._uch, reg, ctypes.byref(ctypes.c_uint64(value)))
    if status:
        raise UcError(status)


def _game_functions():
    import json
    p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "analysis", "decomp", "procedures.json")
    return [a for a in (int(x["address"], 16) for x in json.load(open(p))) if a < LIB_START]


if COV_FILE:
    import atexit
    atexit.register(lambda: open(COV_FILE, "a").write("".join(f"0x{a:X}\n" for a in sorted(COV_SEEN))))


class EmuStop(Exception):
    pass


class Emu:
    def __init__(self, dump=None, fast=False):
        """dump = a 16 MB guest memory snapshot (jactl dump / xdump.sh file, or bytes): the whole machine state is
        mapped flat at 0 (DOS/4GW, paging off) and code runs where DOS/4GW loaded it: use e.rt(image_addr).
        fast = hook only stub addresses and the return sentinel (no per-instruction Python callback: ~50x faster;
        no tracing, e.count stays 0, the step limit is Unicorn's instruction count)."""
        self.img = load()
        self.uc = Uc(UC_ARCH_X86, UC_MODE_32)
        self.dump = None
        if dump is not None:
            from memdump import Dump
            self.dump = Dump(dump)
            mem = self.dump.mem
            self.uc.mem_map(0, (len(mem) + PAGE - 1) & ~(PAGE - 1))
            self.uc.mem_write(0, bytes(mem))
        for o in self.img.objs if dump is None else ():
            size = (o.vsize + PAGE - 1) & ~(PAGE - 1)
            self.uc.mem_map(o.base, size)
            self.uc.mem_write(o.base, bytes(o.data))
        self.uc.mem_map(STACK_TOP - STACK_SIZE, STACK_SIZE)
        self.uc.mem_map(RET_SENTINEL, PAGE)
        self.uc.mem_write(RET_SENTINEL, b"\xF4")      # hlt
        self.uc.mem_map(HEAP, 0x400000)
        self.heap_ptr = HEAP
        self.stop_reason = None
        self.count = 0
        self.trace = False
        self.uc.hook_add(UC_HOOK_INTR, self._intr)
        self.uc.hook_add(UC_HOOK_INSN, self._io, None, 1, 0, UC_X86_INS_IN)
        self.uc.hook_add(UC_HOOK_INSN, self._io, None, 1, 0, UC_X86_INS_OUT)
        self.fast = fast
        if fast:
            self.uc.hook_add(UC_HOOK_CODE, self._code, None, RET_SENTINEL, RET_SENTINEL)
        else:
            self.uc.hook_add(UC_HOOK_CODE, self._code)
        self.md = disassembler()
        self.stubs, self.hooked = {}, set()
        self.port_in = None                           # f(port) -> value: answers IN instead of stopping
        self.port_out = None                          # f(port, size, value): records OUT instead of stopping (OUTSB too)
        self.cov = None
        self.cov_wait = {}                            # entry -> function: seen stubbed only; _code records its first real run
        if COV_FILE:
            self._cov_hooks()

    def _cov_hooks(self):
        """JA1_EMU_COV=FILE: record each game function (procedures.json, below the library modules) that runs for
        real, i.e. its entry executes while no stub replaces it; written to FILE at exit. Fast mode: one-shot hooks
        at the entries; else the per-instruction hook looks the address up (many range hooks beside an all-address
        hook slow every instruction). ./t check sets it for the diff tests; validate/gate_diff_tested.py reads it."""
        self.cov = {self.rt(a): a for a in _game_functions() if a not in COV_SEEN}
        if not self.fast:
            return
        for ra, a in self.cov.items():
            h = [None]

            def hit(uc, addr, size, _, a=a, h=h):
                if addr not in self.stubs:
                    COV_SEEN.add(a)
                    uc.hook_del(h[0])
                elif addr in self.hooked:    # stub() put _code on this entry for good: it records a later real run,
                    self.cov_wait[addr] = a  # and this hook no longer costs one more callback per stub call
                    uc.hook_del(h[0])
            h[0] = self.uc.hook_add(UC_HOOK_CODE, hit, None, ra, ra)
        self.cov = None

    def rt(self, addr):
        """image address -> address in this emulator (differs only in dump mode)"""
        return addr + self.dump.delta(addr) if self.dump else addr

    # --- memory helpers ---
    def r8(self, a):  return self.uc.mem_read(a, 1)[0]
    def r16(self, a): return struct.unpack("<H", self.uc.mem_read(a, 2))[0]
    def r32(self, a): return struct.unpack("<I", self.uc.mem_read(a, 4))[0]
    def w8(self, a, v):  self.uc.mem_write(a, bytes([v & 0xFF]))
    def w16(self, a, v): self.uc.mem_write(a, struct.pack("<H", v & 0xFFFF))
    def w32(self, a, v): self.uc.mem_write(a, struct.pack("<I", v & 0xFFFFFFFF))

    def alloc(self, data):
        if isinstance(data, int):
            data = bytes(data)
        a = self.heap_ptr
        self.uc.mem_write(a, bytes(data))
        self.heap_ptr = (a + len(data) + 15) & ~15
        return a

    def cstr(self, a, n=256):
        b = bytes(self.uc.mem_read(a, n))
        return b.split(b"\0")[0].decode("latin-1")

    def stub(self, addr, fn, pop=0):
        """Replace the function at addr: fn(emu, eax, edx, ebx, ecx) runs instead, its result (None = 0) goes to EAX,
        then it returns to the caller, popping `pop` stack-arg bytes (Watcom callee cleanup; 0 for cdecl)."""
        if self.fast and addr not in self.hooked:     # one hook per address: a stub deleted and set again keeps it
            self.uc.hook_add(UC_HOOK_CODE, self._code, None, addr, addr)
            self.hooked.add(addr)
            self.uc.ctl_flush_tb()                    # code translated before this hook would skip it
        self.stubs[addr] = (fn, pop)

    def cstr(self, a):
        out = bytearray()
        while self.r8(a + len(out)):
            out.append(self.r8(a + len(out)))
        return bytes(out)

    def stub_sprintf(self, addr=0x75668):
        """sprintf_ (Watcom clib, cdecl) as Python: its segment pushes fault in Unicorn. %s %d %u %c only, with an
        optional width (space padded; a leading 0 pads numbers with zeros after the sign, as C does)."""
        def sprintf(em, *_):
            sp = em.uc.reg_read(UC_X86_REG_ESP)
            buf, fmt = em.r32(sp + 4), em.cstr(em.r32(sp + 8))
            out, k, i = bytearray(), 0, 0
            while i < len(fmt):
                if fmt[i] != 0x25:
                    out.append(fmt[i])
                    i += 1
                    continue
                j = i + 1
                while 0x30 <= fmt[j] <= 0x39:
                    j += 1
                width, zero = int(fmt[i + 1:j] or b"0"), fmt[i + 1:i + 2] == b"0"
                c, v = fmt[j], em.r32(sp + 12 + 4 * k)
                k, i = k + 1, j + 1
                if c == 0x73:
                    f = em.cstr(v)
                elif c in (0x64, 0x75):
                    f = b"%0*d" % (width if zero else 0, v - (1 << 32) if c == 0x64 and v >> 31 else v)
                elif c == 0x63:
                    f = bytes([v & 0xFF])
                else:
                    raise ValueError(f"stub sprintf: %{chr(c)} not supported")
                out += f.rjust(width)
            em.uc.mem_write(buf, bytes(out) + b"\0")
            return len(out)
        self.stub(self.rt(addr), sprintf)

    # --- hooks ---
    def _intr(self, uc, intno, _):
        self.stop_reason = f"INT 0x{intno:02X} at 0x{uc.reg_read(UC_X86_REG_EIP):08X} (EAX=0x{uc.reg_read(UC_X86_REG_EAX):08X})"
        uc.emu_stop()

    def _io(self, uc, *a):
        if self.port_in and len(a) == 3:              # IN (port, size, user): the test's answer goes to AL/AX/EAX
            return self.port_in(a[0]) & 0xFFFFFFFF
        if self.port_out and len(a) == 4:             # OUT (port, size, value, user), also OUTSB/OUTSW/OUTSD
            self.port_out(a[0], a[1], a[2])
            return
        self.stop_reason = f"port I/O at 0x{uc.reg_read(UC_X86_REG_EIP):08X}"
        uc.emu_stop()

    def _code(self, uc, addr, size, _):
        self.count += 1
        if addr == RET_SENTINEL:
            uc.emu_stop(); return
        if self.cov and addr in self.cov and addr not in self.stubs:
            COV_SEEN.add(self.cov.pop(addr))
        if addr in self.stubs:
            fn, pop = self.stubs[addr]
            r = fn(self, reg_get(uc, UC_X86_REG_EAX), reg_get(uc, UC_X86_REG_EDX), reg_get(uc, UC_X86_REG_EBX),
                   reg_get(uc, UC_X86_REG_ECX))
            reg_set(uc, UC_X86_REG_EAX, (r or 0) & 0xFFFFFFFF)
            sp = reg_get(uc, UC_X86_REG_ESP)
            reg_set(uc, UC_X86_REG_EIP, self.r32(sp))
            reg_set(uc, UC_X86_REG_ESP, sp + 4 + pop)
            return
        if self.cov_wait and addr in self.cov_wait:
            COV_SEEN.add(self.cov_wait.pop(addr))
        if self.trace:
            ins = next(self.md.disasm(bytes(uc.mem_read(addr, size)), addr), None)
            print(f"  {addr:08X}  {ins.mnemonic + ' ' + ins.op_str if ins else '?'}")
        if self.count >= self.max_steps:
            self.stop_reason = f"step limit {self.max_steps} at 0x{addr:08X}"
            uc.emu_stop()

    # --- calling ---
    def call(self, addr, *args, cdecl=False, max_steps=5_000_000):
        uc = self.uc
        self.stop_reason, self.count, self.max_steps = None, 0, max_steps
        sp = STACK_TOP - 0x100
        regs = [UC_X86_REG_EAX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ECX]
        stack_args = list(args) if cdecl else list(args[4:])
        for v in reversed(stack_args):
            sp -= 4; self.w32(sp, v)
        sp -= 4; self.w32(sp, RET_SENTINEL)
        for r in regs + [UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP]:
            uc.reg_write(r, 0)
        if not cdecl:
            for r, v in zip(regs, args[:4]):
                uc.reg_write(r, v & 0xFFFFFFFF)
        uc.reg_write(UC_X86_REG_ESP, sp)
        try:
            # fast: a time limit, not count= (count= checks a counter after every instruction and stops
            # block chaining: ~185x slower); about max_steps at 20M instructions/s, at least 2 s
            uc.emu_start(addr, RET_SENTINEL + 1, timeout=int(max(2.0, max_steps / 20e6) * 1e6) if self.fast else 0)
        except UcError as e:
            self.stop_reason = f"{e} at 0x{uc.reg_read(UC_X86_REG_EIP):08X}"
        if self.fast and not self.stop_reason and uc.reg_read(UC_X86_REG_EIP) != RET_SENTINEL:
            self.stop_reason = f"step limit {max_steps} at 0x{uc.reg_read(UC_X86_REG_EIP):08X}"
        if self.stop_reason:
            raise EmuStop(self.stop_reason)
        return uc.reg_read(UC_X86_REG_EAX)


def main():
    if len(sys.argv) < 2:
        print(__doc__); return
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    labels = load_labels()
    by_name = {n: a for a, (n, _) in labels.items()}
    fn = by_name[args[0]] if args[0] in by_name else parse_addr(args[0])
    vals = [int(a, 0) for a in args[1:] if not sys.argv[sys.argv.index(a) - 1] == "--max"]
    e = Emu()
    e.trace = "--trace" in sys.argv
    mx = int(sys.argv[sys.argv.index("--max") + 1]) if "--max" in sys.argv else 5_000_000
    try:
        r = e.call(fn, *vals, cdecl="--cdecl" in sys.argv, max_steps=mx)
        print(f"EAX = 0x{r:08X} ({r if r < 0x80000000 else r - 0x100000000})   {e.count} instructions")
    except EmuStop as s:
        print(f"stopped: {s}   after {e.count} instructions")


if __name__ == "__main__":
    main()
