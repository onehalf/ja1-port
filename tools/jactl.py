"""Live bridge to the original game in DOSBox-X: read typed game state from guest RAM, send input, wait on state.

The emulator (validate/run_x.sh) runs a capability-free copy of dosbox-x with build/ptrace_any.so preloaded, so this
process may read /proc/PID/mem. Guest RAM is found by the BIOS reset vector + date at physical 0xFFFF0; the LE objects
by their bytes (memdump.Dump), so image (labels.csv) addresses work unchanged. Single reads never pause the game;
`dump` and `state` (and scripts inside `with g.frozen():`) stop DOSBox-X with SIGSTOP for the few ms they read, so
fields read together are consistent (one emulated instant).
Input goes to the DOSBox-X window on the private display ($JA_DISPLAY, default :99) with xdotool, in GAME coords
(320x200; window = 2x plus a 17 px menu bar).

usage: ./t jactl state                         summary: day, sector, money, view, soldiers in play
       ./t jactl get EXPR                      evaluate, e.g. g_curSector  'g_soldiers[0].hp'  'g_sectorTbl9[58].dict()'
       ./t jactl wait EXPR [TIMEOUT]           poll (50 ms) until EXPR is true; exit 1 on timeout (default 30 s)
       ./t jactl watch EXPR [SECONDS]          print EXPR whenever it changes
       ./t jactl screen                        current screen: boot intro menu office restore slots sunrise strategic
                                               tactical speech message confirm (NAME/setup while drawing)
       ./t jactl stack                         live call stack of the game (outermost first; screens are nested loops)
       ./t jactl dump NAME                     16 MB snapshot -> build/mem_NAME.bin (same format as validate/xdump.sh)
       ./t jactl click X Y | rclick X Y | move X Y | key KEY.. | shot NAME
       ./t jactl save NAME [NOTE..] | load NAME | states    DOSBox-X save states (whole machine) in build/states/,
                                               catalogued in validate/states.tsv (name, date, state summary, note)
EXPR is Python over the typed globals of defs/types.toml (names from labels.csv), plus u8/u16/i16/u32/i32(addr|name)
and mem(addr, n) for untyped addresses. Example: ./t jactl wait 'g_soldiers[0].tile == 974' 20
As a module: g = Game(); g.v.g_soldiers[0].hp; g.click(140, 74); g.wait(lambda: g.v.g_curSector == 59)
"""
import contextlib
import os
import re
import signal
import struct
import subprocess
import sys
import time
from common import ROOT, load_labels
from jatypes import Types
from memdump import Dump

GUEST_MB = 16                     # dosbox-x.conf memsize
DISPLAY = os.environ.get("JA_DISPLAY", ":99")
SHOTS = os.path.join(ROOT, "validate", "shots")
SLOT = os.path.join(ROOT, "validate", "run", "save", "1.sav")      # DOSBox-X save slot 1 (cwd validate/run)
STATES = os.path.join(ROOT, "build", "states")
LAST_LOADED = os.path.join(STATES, "last_loaded")                  # name of the state load_state restored last
RUNNER = os.path.join(ROOT, "build", "dosbox-x")                   # the copy validate/run_x.sh runs
EXE = os.path.join(ROOT, "validate", "run", "C", "JA.EXE")  # the run copy (1.13b, #24)
CATALOG_HEAD = "name\tdate\tstate\tnote\trunner_sha256\texe_sha256"


def sha256(path):
    """Full sha256 of a file, or '-' if it is missing (states load only in the DOSBox-X build that wrote them)."""
    import hashlib
    if not os.path.exists(path):
        return "-"
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()
CATALOG = os.path.join(ROOT, "validate", "states.tsv")


def find_pid():
    out = subprocess.run(["pgrep", "-x", "dosbox-x"], capture_output=True, text=True).stdout.split()
    if not out:
        raise RuntimeError("DOSBox-X is not running (validate/run_x.sh)")
    return int(out[-1])


class Live:
    """Guest memory of a running DOSBox-X, addressed like a memdump.Dump (image addresses) or physically."""
    def __init__(self, pid=None):
        self.pid = pid or find_pid()
        try:
            self.f = open(f"/proc/{self.pid}/mem", "rb", buffering=0)
            maps = open(f"/proc/{self.pid}/maps").read().splitlines()
        except PermissionError:
            raise RuntimeError("cannot read DOSBox-X memory: start it with validate/run_x.sh (ptrace_any.so, no file caps)")
        self.base = self._find_ram(maps)
        self.objs = None
        self.depth = 0                            # frozen() nesting

    @contextlib.contextmanager
    def frozen(self):
        """Stop the emulator (all threads, SIGSTOP) while the block reads, so the reads see one consistent instant;
        SIGCONT on exit, also on exceptions. Re-entrant. Keep blocks short (no waits/input inside: the game is stopped)."""
        if self.depth == 0:
            os.kill(self.pid, signal.SIGSTOP)
            end = time.monotonic() + 1.0
            while self._proc_state() not in "Tt" and time.monotonic() < end:   # delivery is asynchronous
                time.sleep(0.001)
        self.depth += 1
        try:
            yield
        finally:
            self.depth -= 1
            if self.depth == 0:
                os.kill(self.pid, signal.SIGCONT)

    def _proc_state(self):
        s = open(f"/proc/{self.pid}/stat").read()
        return s[s.rindex(")") + 2]

    def _find_ram(self, maps):
        """Guest RAM = the block whose physical 0xFFFF0 holds the BIOS reset vector (jmp far, 0xEA) + date. Usually
        right at the start of an anonymous rw mapping (behind a malloc header); sometimes inside a larger one."""
        areas = []
        for line in maps:
            f = line.split()
            lo, hi = (int(x, 16) for x in f[0].split("-"))
            if f[1].startswith("rw") and len(f) <= 5 and hi - lo >= GUEST_MB << 20:
                areas.append((lo, hi))
        for lo, hi in areas:                        # fast: malloc header in front of the block
            for k in range(0, 0x1000, 0x10):
                try:
                    v = self._pread(lo + k + 0xFFFF0, 16)
                except OSError:
                    break
                if v[0] == 0xEA and re.fullmatch(rb"\d\d/\d\d/\d\d", v[5:13]):
                    return lo + k
        pat = re.compile(rb"\xEA.{4}\d\d/\d\d/\d\d", re.S)
        for lo, hi in areas:                        # slow: search the whole mapping (16-aligned block start)
            for c in range(lo, hi, 1 << 24):
                try:
                    buf = self._pread(c, min(hi - c, (1 << 24) + 16))
                except OSError:
                    break
                for m in pat.finditer(buf):
                    base = c + m.start() - 0xFFFF0
                    if base % 16 == 0 and lo <= base and base + (GUEST_MB << 20) <= hi:
                        return base
        raise RuntimeError("guest RAM not found in the DOSBox-X process")

    def _pread(self, a, n):
        self.f.seek(a)
        return self.f.read(n)

    def phys(self, a, n):
        return self._pread(self.base + a, n)

    def snapshot(self):
        """All guest RAM (same layout as a MEMDUMPBIN 188:0 dump)."""
        return self.phys(0, GUEST_MB << 20)

    def locate(self):
        """Find where DOS/4GW loaded the LE objects (None while the game is not running). The full search takes ~3 s,
        so the last result (build/jactl_loc.json) is tried first and accepted when sampled bytes still match."""
        import json
        cache = os.path.join(ROOT, "build", "jactl_loc.json")
        try:
            objs = [tuple(o) for o in json.load(open(cache))]
            if self._matches(objs):
                self.objs = objs
                return objs
        except (OSError, ValueError):
            pass
        d = Dump(self.snapshot())
        self.objs = d.objs if all(dl is not None for _, _, dl in d.objs) else None
        if self.objs:
            json.dump(self.objs, open(cache, "w"))
        return self.objs

    def _matches(self, objs):
        """Sampled 16-byte runs of each object's file image found at base + delta: at least a quarter of them (globals
        change at runtime, code holds relocated fixups) and 3; a wrong delta matches none."""
        from le import load
        for o, (lo, hi, dl) in zip(load().objs, objs):
            data = bytes(o.data)
            runs = [p for p in range(0, len(data) - 16, 0x400) if len(set(data[p:p + 16])) >= 6]
            hits = sum(self.phys(o.base + dl + p, 16) == data[p:p + 16] for p in runs)
            if hits < max(3, len(runs) // 4):
                return False
        return True

    def delta(self, addr):
        if self.objs is None and self.locate() is None:
            raise RuntimeError("JA.EXE is not loaded in the guest yet")
        for lo, hi, d in self.objs:
            if lo <= addr < (hi + 0xFFF) & ~0xFFF:     # to the end of the last page: 1.15m tail code (#70)
                return d
        raise ValueError(f"0x{addr:X} is outside the image")

    def read(self, addr, n):
        return self.phys(addr + self.delta(addr), n)


class Input:
    """xdotool on the private display, GAME coordinates."""
    def __init__(self):
        self.env = dict(os.environ, DISPLAY=DISPLAY)
        self.win = None

    def xdo(self, *args):
        subprocess.run(["xdotool", *map(str, args)], env=self.env, check=True)

    def window(self):
        if self.win is None:
            out = subprocess.run(["xdotool", "search", "--name", "DOSBox-X"], env=self.env, capture_output=True,
                                 text=True).stdout.split()
            if not out:
                raise RuntimeError(f"no DOSBox-X window on {DISPLAY}")
            self.win = out[0]
        return self.win

    def move(self, x, y):
        w = self.window()
        self.xdo("windowfocus", "--sync", w, "mousemove", "--window", w, x * 2, y * 2 + 17)

    def click(self, x, y, button=1):
        self.move(x, y)
        time.sleep(0.3)
        self.xdo("mousedown", button)
        time.sleep(0.15)
        self.xdo("mouseup", button)

    def key(self, *keys, delay=150):
        w = self.window()
        self.xdo("windowfocus", "--sync", w)
        self.xdo("key", "--window", w, "--delay", delay, *keys)

    def hostkey(self, k):
        """DOSBox-X host-key combo (F12+k): s = save state, l = load state (slot 1)."""
        w = self.window()
        self.xdo("windowfocus", "--sync", w)
        self.xdo("keydown", "F12", "key", k, "keyup", "F12")

    def shot(self, name):
        path = name if name.endswith(".png") else os.path.join(SHOTS, name + ".png")
        subprocess.run(["import", "-window", self.window(), path], env=self.env, check=True)
        return path


class Globals:
    """Attribute / mapping access to typed globals: g.v.g_day, g.v['g_soldiers'][0].hp."""
    def __init__(self, live, types):
        self._live, self._types = live, types

    def __getattr__(self, name):
        if name.startswith("_"):
            raise AttributeError(name)
        try:
            return self._types.view(self._live, name)
        except KeyError:
            raise AttributeError(f"{name}: not a typed global in defs/types.toml") from None

    __getitem__ = __getattr__


class Game:
    def __init__(self, pid=None):
        self.live = Live(pid)
        self.types = Types()
        self.v = Globals(self.live, self.types)
        self.inp = Input()
        self.labels = {n: a for a, (n, _) in load_labels().items()}

    # input
    def click(self, x, y):
        self.inp.click(x, y)

    def rclick(self, x, y):
        self.inp.click(x, y, 3)

    def key(self, *keys):
        self.inp.key(*keys)

    def shot(self, name):
        return self.inp.shot(name)

    # memory
    def frozen(self):
        """with g.frozen(): ... reads several fields from one emulated instant (see Live.frozen)."""
        return self.live.frozen()

    def addr(self, a):
        return self.labels[a] if isinstance(a, str) else a

    def mem(self, a, n):
        return self.live.read(self.addr(a), n)

    def num(self, fmt, a):
        return struct.unpack(fmt, self.mem(a, struct.calcsize(fmt)))[0]

    def env(self):
        """Namespace for EXPR strings."""
        ns = {"mem": self.mem, "g": self.v, "time": time}
        for n, f in (("u8", "<B"), ("i8", "<b"), ("u16", "<H"), ("i16", "<h"), ("u32", "<I"), ("i32", "<i")):
            ns[n] = (lambda f: lambda a: self.num(f, a))(f)
        ns["screen"] = self.screen
        for name in self.types.globals:          # arrays/structs stay lazy views; scalars are read now (fresh per eval)
            ns[name] = getattr(self.v, name)
        return ns

    def eval(self, expr):
        return eval(expr, self.env())

    def wait(self, pred, timeout=30.0, poll=0.05):
        """pred: callable or EXPR string; returns its truthy value, raises TimeoutError."""
        f = pred if callable(pred) else (lambda: self.eval(pred))
        end = time.monotonic() + timeout
        while True:
            r = f()
            if r:
                return r
            if time.monotonic() > end:
                raise TimeoutError(pred if isinstance(pred, str) else "condition")
            time.sleep(poll)

    def settle(self, expr, quiet=1.0, timeout=30.0, poll=0.05):
        """Wait until EXPR (callable or string) has not changed for `quiet` seconds (e.g. a walking merc's tile)."""
        f = expr if callable(expr) else (lambda: self.eval(expr))
        end = time.monotonic() + timeout
        last, since = f(), time.monotonic()
        while time.monotonic() < end:
            time.sleep(poll)
            v = f()
            if v != last:
                last, since = v, time.monotonic()
            elif time.monotonic() - since >= quiet:
                return v
        raise TimeoutError(f"{expr} did not settle")

    def save_state(self, name, note=""):
        """Whole-machine snapshot (F12+S into slot 1, then copied to build/states/NAME.sav) + a catalog line."""
        import shutil
        os.makedirs(STATES, exist_ok=True)
        old = os.path.getmtime(SLOT) if os.path.exists(SLOT) else 0
        self.inp.hostkey("s")
        self.wait(lambda: os.path.exists(SLOT) and os.path.getmtime(SLOT) > old, 20, 0.2)
        size = -1
        while size != os.path.getsize(SLOT):      # written in one go, but be safe
            size = os.path.getsize(SLOT)
            time.sleep(0.3)
        shutil.copy(SLOT, os.path.join(STATES, name + ".sav"))
        v = self.v
        summary = f"day {v.g_day} sector {v.g_curSector + 1} balance {v.g_balance}"
        rows = [r for r in (open(CATALOG).read().splitlines() if os.path.exists(CATALOG) else [CATALOG_HEAD])
                if not r.startswith(name + "\t")]
        rows.append(f"{name}\t{time.strftime('%Y-%m-%d')}\t{summary}\t{note}\t{sha256(RUNNER)}\t{sha256(EXE)}")
        open(CATALOG, "w").write("\n".join(rows) + "\n")

    def load_state(self, name):
        import shutil
        src = os.path.join(STATES, name + ".sav")
        if not os.path.exists(src):
            raise FileNotFoundError(src)
        shutil.copy(src, SLOT)
        open(LAST_LOADED, "w").write(name + "\n")
        before = self.live.phys(0, 0x1000)
        self.inp.hostkey("l")
        time.sleep(1.5)                          # restore is synchronous in the emulator thread; give it a frame
        return before

    STACK_TOP = 0x33A730                         # LE initial ESP (obj2 end); the stack grows down from here

    def _code(self):
        """(code bytes, code base, sorted function starts, {start: name}) for stack walking (cached)."""
        if not hasattr(self, "_codeinfo"):
            from common import load_funcs
            from le import load
            o = load().objs[0]
            funcs = load_funcs()
            # the live code, not rom/JA.EXE: 1.13b moves main_ by 1 byte, and the frame chain breaks at its call
            self._codeinfo = (bytes(self.mem(o.base, len(o.data))), o.base, sorted(funcs), funcs)
        return self._codeinfo

    def func_of(self, a):
        """Start of the function containing image address a (None outside code)."""
        import bisect
        code, base, starts, _ = self._code()
        i = bisect.bisect_right(starts, a) - 1
        return starts[i] if i >= 0 and base <= a < base + len(code) else None

    def _call_before(self, r):
        """Target of the call instruction that returns to r: an address, "ind" (call r/m), or None (no call)."""
        code, base, _, _ = self._code()
        o = r - base
        if 5 <= o <= len(code) and code[o - 5] == 0xE8:
            return (r + struct.unpack_from("<i", code, o - 4)[0]) & 0xFFFFFFFF
        for n in (2, 3, 6, 7):
            if n <= o <= len(code) and code[o - n] == 0xFF and (code[o - n + 1] >> 3) & 7 == 2:
                return "ind"
        return None

    def stack(self, size=0x4000):
        """The game's live call stack, outermost first: [(slot, ret, caller function start, callee)], read without
        CPU registers (ESP is not in guest RAM): stack dwords that are return addresses (code right after a call) are
        kept only when they chain, i.e. lie in the function the next outer frame called. That drops locals and stale
        frames left below ESP; a stale chain under the live innermost frame can survive (helpers, not screen loops).
        Runtime addresses are image + the code object's load delta."""
        with self.frozen():
            st = self.mem(self.STACK_TOP - size, size)
            cd = self.live.objs[0][2]
        out, want = [], None
        for off in range(size - 4, -1, -4):
            r = struct.unpack_from("<I", st, off)[0] - cd
            f = self.func_of(r)
            if f is None or (want not in (None, "ind") and f != want):
                continue
            t = self._call_before(r)
            if t is None:
                continue
            out.append((self.STACK_TOP - size + off, r, f, t))
            want = t
        return out

    # Screens are nested function loops (GameMain menu loop > PlayLoop > OfficeScreen | SunriseScreen |
    # StrategicScreen | TacticalLoop); no global names the current one, the live call stack does. Per screen
    # function: (name, loop head = first return address inside its loop; a frame returning before it is still
    # setting up / drawing, reported as "NAME/setup"). Found with validate/to_tactical.py runs (REVERSE.md, Screens).
    SCREENS = {
        0x4FFEC: ("menu", 0x50057),           # GameMain: main-menu loop after MainMenu(); no frame yet = "boot"
        0x4EE64: ("office", 0x4EE9F),         # OfficeScreen, after its first draw + TimerStart
        0x4E700: ("restore", None),           # RestoreGameDialog? (slot list) over the office
        0x2A268: ("sunrise", None),           # SunriseScreen? (xsunrise.pcx, funds, new arrivals), day start
        0x432F4: ("strategic", 0x4334D),      # StrategicScreen: before the head = map draw + guide speech dialogs
        0x107BC: ("tactical", 0x107FA),       # TacticalLoop: before the head = LoadTacticalSector? + fade
        0x66604: ("intro", None),             # PlayTitleIntro? (title.flc, credits) inside InitMainDatMouse
        0x4DCCC: ("slots", None),             # SlotListLoop?: save/restore slot list taking input
        0x28034: ("speech", None),            # SpeechWait?: merc speech balloon waiting for a key/click
        0x27B98: ("message", None),           # ShowMessage?: text box until key/click or timeout
        0x275E4: ("confirm", 0x2574B),        # ConfirmDialog? (Get to work?): input from 0x2574B, then travel anim
    }

    def screen(self):
        """Innermost screen on the live call stack: "boot" (before GameMain: press-any-key, intro), "menu",
        "office", "restore", "sunrise", "strategic", "tactical"; "NAME/setup" while that screen is still drawing.
        Modal boxes count as screens too: "intro", "slots", "speech", "message", "confirm"."""
        cur = "boot"
        for _, r, f, _ in self.stack():
            if f in self.SCREENS:
                name, head = self.SCREENS[f]
                cur = name + ("/setup" if head and r < head else "")
        return cur

    def at(self, func, ret=None):
        """True while a live frame of func (image address or label) calls out from return address ret (any if None)."""
        func = self.addr(func)
        return any(f == func and (ret is None or r == ret) for _, r, f, _ in self.stack())

    def stack_text(self):
        names = self._code()[3]
        return "\n".join(f"  {slot:06X}  {names.get(f, f'FUN_{f:05X}')}+0x{r - f:X}  -> "
                         f"{'(indirect)' if t == 'ind' else names.get(t, f'FUN_{t:05X}')}"
                         for slot, r, f, t in self.stack())

    def soldiers(self, slots=None):
        """[(slot, View)] of soldiers on the map (tile < 0xC40, hpMax > 0: empty slots have tile 0) or the given slots."""
        S = self.v.g_soldiers
        ks = slots if slots is not None else range(48)
        return [(k, S[k]) for k in ks if slots is not None or (S[k].tile < 0xC40 and S[k].hpMax > 0)]

    def state(self):
        with self.frozen():
            return self._state()

    def _state(self):
        v = self.v
        lines = [f"day {v.g_day}  sector {v.g_curSector} (UI {v.g_curSector + 1})  balance {v.g_balance}  "
                 f"view {v.g_viewX},{v.g_viewY}  compressed {v.g_timeCompression}"]
        for k, s in self.soldiers():
            x, y = s.x - v.g_viewX + 48, s.y - v.g_viewY + 12
            lines.append(f"  {k:2d} {s.nickname:10s} ({s.tile % 56:2d},{s.tile // 56:2d}) hp {s.hp:3d}/{s.hpMax:3d} "
                         f"screen {x:4d},{y:4d} item0 {s.items[0].type:02x}/{s.items[0].qty}")
        return "\n".join(lines)


def main():
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    cmd = a[0]
    if cmd in ("click", "rclick", "move", "key", "shot"):
        inp = Input()
        if cmd == "key":
            inp.key(*a[1:])
        elif cmd == "shot":
            print(inp.shot(a[1]))
        elif cmd == "move":
            inp.move(int(a[1]), int(a[2]))
        else:
            inp.click(int(a[1]), int(a[2]), 3 if cmd == "rclick" else 1)
        return
    g = Game()
    if cmd == "state":
        print(g.state())
    elif cmd == "stack":
        print(g.stack_text())
    elif cmd == "screen":
        print(g.screen())
    elif cmd == "get":
        print(g.eval(a[1]))
    elif cmd == "wait":
        try:
            print(g.wait(a[1], float(a[2]) if len(a) > 2 else 30))
        except TimeoutError:
            sys.exit(f"timeout: {a[1]}")
    elif cmd == "watch":
        end = time.monotonic() + (float(a[2]) if len(a) > 2 else 1e9)
        last = object()
        while time.monotonic() < end:
            v = g.eval(a[1])
            if v != last:
                print(f"{time.strftime('%H:%M:%S')} {v}", flush=True)
                last = v
            time.sleep(0.05)
    elif cmd == "save":
        g.save_state(a[1], " ".join(a[2:]))
        print(os.path.join(STATES, a[1] + ".sav"))
    elif cmd == "load":
        g.load_state(a[1])
        print(g.state())
    elif cmd == "states":
        print(open(CATALOG).read() if os.path.exists(CATALOG) else "no states yet")
    elif cmd == "dump":
        path = os.path.join(ROOT, "build", f"mem_{a[1]}.bin")
        with g.frozen():
            data = g.live.snapshot()
        open(path, "wb").write(data)
        import json                                # provenance sidecar: which state, runner and exe made this dump
        last = open(LAST_LOADED).read().strip() if os.path.exists(LAST_LOADED) else None
        v = g.v
        json.dump({"date": time.strftime("%Y-%m-%d %H:%M"),
                   "state": f"day {v.g_day} sector {v.g_curSector + 1} balance {v.g_balance}",
                   "last_loaded_state": last, "runner_sha256": sha256(RUNNER), "exe_sha256": sha256(EXE)},
                  open(path[:-4] + ".json", "w"), indent=1)
        print(path)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
