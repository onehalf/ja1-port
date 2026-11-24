"""Differential test: the speech balloon (panel.md "Speech balloon"), original vs port.

usage: .venv/bin/python validate/diff/balloon_diff.py [ncases=200] [DUMP] [--port] [--lib SO]
Per case: a merc record of tac60 with fuzzed panel slot (+0x17A 0..7), side (+0x180), profile (+0x179 0..63), quote
(0..32), health (+0x183 >= 10, so that the balloon shows), the Text option byte 0x1163BD on / off and a noise screen.
The original runs ShowMercMessage? 0x299B8 (how = the quote, radio test passed) in Unicorn on tac60: PlayQuote
0x24F08 writes the DAYQUOTE.EDT record (33 p + q) into its buffer, the PCC loader 0x532BC writes the MAIN.DAT face;
SpeechWait 0x28034, the frame delay 0x67250 and the redraws are stubbed. The screens at the 8 delays and at the
wait (9) are compared with the port's (port/ja1front ja1f_speech_balloon). Without --port only the original runs.
Then 20 Santino cases (panel.md "Santino's balloon"): SantinoSpeech? 0x5F4C4 with the Subtitles byte 0 on a noise
screen (LoadPanels 0x46EC4, the mouse calls and YesNoPrompt? 0x28C84 stubbed) vs ja1f_santino_balloon.
Then 20 guide and Brenda cases (panel.md "The guide's and Brenda's balloons"): 0x5F5AC (quote q, category c) and
BrendaSpeech? 0x5F76C (quote q) with q, c fuzzed (some with the second part) vs ja1f_npc_balloon; with a second
part the original waits twice (one more screen, equal to the first wait). The SpeechWait? calls (quote, text time,
hold) are compared with ja1f_npc_waits.
Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 200
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_speech_balloon.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_int32] * 2 + [ctypes.c_char_p,
                                                                                        ctypes.c_void_p]
    PORT.ja1f_speech_balloon.restype = ctypes.c_int32
    PORT.ja1f_santino_balloon.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_char_p, ctypes.c_void_p]
    PORT.ja1f_santino_balloon.restype = ctypes.c_int32
    PORT.ja1f_balloon_wait.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_int32] * 2
    PORT.ja1f_balloon_wait.restype = ctypes.c_int32
    PORT.ja1f_npc_balloon.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] * 3 + [ctypes.c_char_p, ctypes.c_void_p]
    PORT.ja1f_npc_waits.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] * 3 + [ctypes.c_void_p]
    PORT.ja1f_npc_balloon.restype = ctypes.c_int32
    PORT.ja1f_npc_sub_waits.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] * 3 + [ctypes.c_void_p]
    PORT.ja1f_npc_sub_waits.restype = ctypes.c_int32
    PORT.ja1f_sector_balloon.argtypes = PORT.ja1f_speech_balloon.argtypes
    PORT.ja1f_sector_balloon.restype = ctypes.c_int32
    PORT.ja1f_live_mercs.argtypes = [ctypes.c_char_p]
    PORT.ja1f_live_mercs.restype = ctypes.c_int32
    PORT.ja1f_npc_quote.argtypes = [ctypes.c_int32] * 3 + [ctypes.c_char_p]
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
QUOTES = open(os.path.join(ROOT, "rom", "EDT", "DAYQUOTE.EDT"), "rb").read()
NATQ = open(os.path.join(ROOT, "rom", "EDT", "NATQUOTE.EDT"), "rb").read()
CATS = ["AIM", "DAY", "PST", "JAC", "INV", "SIG", "QST", "BRE", "NAT", "SAN", "INT"]
_edt = {}


def edt(cat):
    """The quote texts of category `cat` (speech.md "Quote categories"), as the game reads them."""
    if cat not in _edt:
        import common
        _edt[cat] = common.game_read(f"EDT/{CATS[cat]}QUOTE.EDT") if cat < len(CATS) else b""
    return _edt[cat]
SCREEN_P, OPT_TEXT, SOLDIERS, LASTQ = 0x95678, 0x1163BD, 0xFA958, 0x22A1EA
_faces = {}

e = Emu(dump=dump, fast=True)
screen = e.r32(e.rt(SCREEN_P))
shots = []
cur = {}


def load(em, eax, edx, ebx, ecx):
    n = em.cstr(ebx).decode("latin-1").upper()
    if n not in _faces:
        try:
            w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
        except Exception:
            w, h, px = 0, 0, b""
        _faces[n] = (w, h, px)
    w, h, px = _faces[n]
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def quote(em, eax, edx, ebx, ecx):
    buf = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    cat = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 8) & 0xFF
    at = (edx + ebx) * ecx
    rec = edt(cat)[at:at + ecx]
    # A record past the end of the file reads nothing: the buffer keeps the last quote (balloon.rs shared_text).
    if rec:
        em.uc.mem_write(buf, rec + b"\0")
    cur["quote"] = (eax, edx, ebx, ecx)
    # PlayQuote 0x24F68..0x24FA6: a quote of category 4, 5 or 10 leaves the last quote 0x22A1EA; else speaker, category,
    # row u16, quote u16 (the stub keeps the original's own write, so the compare reads the record the game keeps).
    if cat not in (4, 5, 10):
        em.uc.mem_write(em.rt(LASTQ), bytes([eax & 0xFF, cat]) + (edx & 0xFFFF).to_bytes(2, "little")
                        + (ebx & 0xFFFF).to_bytes(2, "little"))
    return 0


for a in (0x678E8, 0x67BF0, 0x673AC, 0x4B730, 0x4B764, 0x47020, 0x47040, 0x11D40, 0x67368, 0x67398, 0x67C54,
          0x1CB78, 0x320A0, 0x3F724, 0x1D8A4, 0x3F490, 0x681D0, 0x67478, 0x674B0, 0x467E4, 0x46EC4):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x67250), lambda em, *_: shots.append(bytes(em.uc.mem_read(screen, 64000))) or 0)
waits = []


def npc_past(op, c, q):
    """balloon.rs past_end: the record of quote q lies past the end of the speaker's EDT file (c = 7 for Brenda)."""
    return 200 * max(q, 0) + 200 > len(edt(c if op == 302 else 7))




def speech_wait(em, eax, edx, ebx, ecx):
    # SpeechWait? 0x28034 (x, y, text ms, hurt; 1, text, hold): the quote last read, the text time and the hold
    sp = em.uc.reg_read(UC_X86_REG_ESP)
    waits.append((cur.get("quote", (0, 0, -1, 0))[2], ebx, em.r32(sp + 12)))
    shots.append(bytes(em.uc.mem_read(screen, 64000)))
    return 0


e.stub(e.rt(0x28034), speech_wait, pop=12)
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x24F08), quote, pop=16)
TEXT = e.alloc(b"\0" * 16)
SLOT = 0
REC = e.rt(SOLDIERS + 512 * SLOT)
orig_rec = bytes(e.uc.mem_read(REC, 512))

r = random.Random(7)
cases = [(k, k >= 4, 16, 0, 1) for k in range(8)] + [(0, 0, 16, 9, 1), (5, 1, 16, 9, 0)]
while len(cases) < ncases:
    k = r.randrange(8)
    cases.append((k, (k >= 4) if r.random() < 0.8 else r.randrange(2), r.randrange(64), r.randrange(33),
                  int(r.random() < 0.8)))
bad = texts = 0


def bx(k, side):
    return min(max((0 if k < 4 else 272) + (14 if side == 0 else 3) + 2 - 15, 1), 0x105)


for i, (k, side, prof, q, opt) in enumerate(cases):
    rec = bytearray(orig_rec)
    rec[0x17A], rec[0x180], rec[0x179] = k, side, prof
    rec[0x30], rec[0x2F + 3] = rec[0x30] or 0x2A, max(rec[0x32], 10)
    rec[0x183] = r.choice([15, 20, 40]) if q != 9 else r.choice([10, 12, 30])
    e.uc.mem_write(REC, bytes(rec))
    e.w8(e.rt(OPT_TEXT), opt)
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen, base)
    shots.clear()
    waits.clear()
    e.call(e.rt(0x299B8), REC, q, 0x40, TEXT, 1)
    if len(shots) != 9:
        print(f"case {i}: {len(shots)} screens (expected 9)")
        bad += 1
        continue
    ch = [m for m in range(64000) if shots[4][m] != base[m]]
    if not ch:
        print(f"case {i}: the original drew nothing")
        bad += 1
    texts += any(not (bx(k, side) <= m % 320 < bx(k, side) + 58) for m in ch)
    if PORT:
        out = ctypes.create_string_buffer(9 * 64000)
        # The wait without a voice: the text time of SpeechWait? (80 ms per byte of the quote).
        pw = PORT.ja1f_balloon_wait(ISO, EXE, bytes(rec), q, opt)
        if not waits or pw != waits[-1][1]:
            print(f"MISMATCH case {i} profile {prof} q {q}: wait {waits} vs port {pw}")
            bad += 1
            continue
        rc = PORT.ja1f_speech_balloon(ISO, EXE, bytes(rec), q, opt, base, out)
        p = out.raw
        for f in range(9):
            o, pf = shots[f], p[f * 64000:(f + 1) * 64000]
            if rc != 9 or o != pf:
                n = next((m for m in range(64000) if o[m] != pf[m]), -1)
                print(f"MISMATCH case {i} slot {k} side {side} profile {prof} q {q} text {opt}: rc {rc}, screen {f} "
                      f"@({n % 320},{n // 320}) {o[n]:#x} vs {pf[n]:#x}" if n >= 0 else f"MISMATCH case {i}: rc {rc}")
                bad += 1
                break
        if bad > 10:
            break
san = 0
for i in range(20 if bad <= 10 else 0):
    e.w8(e.rt(OPT_TEXT), 0)
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen, base)
    shots.clear()
    waits.clear()
    e.call(e.rt(0x5F4C4))
    san += 1
    if len(shots) != 9 or shots[4] == base:
        print(f"santino case {i}: {len(shots)} screens (expected 9, the face drawn)")
        bad += 1
        continue
    if PORT:
        out = ctypes.create_string_buffer(9 * 64000)
        pw = PORT.ja1f_balloon_wait(ISO, EXE, None, 0, 0)
        if not waits or pw != waits[-1][1]:
            print(f"MISMATCH santino case {i}: wait {waits} vs port {pw}")
            bad += 1
            continue
        rc = PORT.ja1f_santino_balloon(ISO, EXE, base, out)
        p = out.raw
        for f in range(9):
            o, pf = shots[f], p[f * 64000:(f + 1) * 64000]
            if rc != 9 or o != pf:
                n = next((m for m in range(64000) if o[m] != pf[m]), -1)
                print(f"MISMATCH santino case {i}: rc {rc}, screen {f} @({n % 320},{n // 320}) {o[n]:#x} vs {pf[n]:#x}"
                      if n >= 0 else f"MISMATCH santino case {i}: rc {rc}")
                bad += 1
                break
npc = two = 0
for i in range(20 if bad <= 10 else 0):
    e.w8(e.rt(OPT_TEXT), 0)
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen, base)
    shots.clear()
    waits.clear()
    op = 302 if i % 2 == 0 else 317
    q = r.choice([r.randrange(0x28), r.randrange(0x28, 0x60)]) if op == 302 else r.randrange(20)
    if i < 8:
        # The bounds of the second part: guide 0x27 / 0x28, Brenda 9 / 10 (NativeGuidePrompt? says Brenda quote 10).
        q = (0x27, 10, 0x28, 9, 0x26, 11, 0x29, 0)[i]
    c = r.choice([6, 6, 3]) if op == 302 else 7
    parts = 2 if (op == 302 and q < 0x28 and c == 6) or (op == 317 and q < 10) else 1
    e.uc.mem_write(e.rt(LASTQ), b"\x77" * 6)
    if op == 302:
        e.call(e.rt(0x5F5AC), q, c)
    else:
        e.call(e.rt(0x5F76C), q)
    npc += 1
    two += parts == 2
    if len(shots) != 8 + parts or shots[4] == base or (parts == 2 and shots[5] != shots[4]):
        print(f"npc case {i} op {op} q {q} c {c}: {len(shots)} screens (expected {8 + parts}, the face drawn)")
        bad += 1
        continue
    if PORT:
        # The waits: one per part, each with its quote, the text time and the hold (ja1f_npc_waits; a record past
        # the end of the file waits no key either, #2 row 62).
        pw = (ctypes.c_int32 * 6)()
        n = PORT.ja1f_npc_waits(ISO, EXE, op, q, c, pw)
        got = [tuple(pw[3 * k:3 * k + 3]) for k in range(max(n, 0))]
        # The voices: the quote list that the sound side plays (sound::npc_parts) = the quotes the original says.
        pq = (ctypes.c_int32 * 3)()
        nq = PORT.ja1f_npc_parts(op, q, c, pq)
        # The last quote that key L says again (play::npc_quote vs the record the original's PlayQuote calls leave).
        lq = ctypes.create_string_buffer(6)
        PORT.ja1f_npc_quote(op, q, c, lq)
        if lq.raw != bytes(e.uc.mem_read(e.rt(LASTQ), 6)):
            print(f"MISMATCH npc case {i} op {op} q {q} c {c}: last quote {lq.raw.hex()} vs "
                  f"{bytes(e.uc.mem_read(e.rt(LASTQ), 6)).hex()}")
            bad += 1
            continue
        if list(pq[:max(nq, 0)]) != [w[0] for w in waits]:
            print(f"MISMATCH npc case {i} op {op} q {q} c {c}: voices {list(pq[:max(nq, 0)])} vs {[w[0] for w in waits]}")
            bad += 1
            continue
        if len(got) != len(waits) or any(g[0] != w[0] or g[2] != w[2] or g[1] != w[1]
                                         for g, w in zip(got, waits)):
            print(f"MISMATCH npc case {i} op {op} q {q} c {c}: waits {waits} vs port {got}")
            bad += 1
            continue
        # Subtitles on: the first part's box waits the original's text time; a record past the end of the file waits
        # for a key instead (#2 row 62, seen live 2026-11-06).
        sw = (ctypes.c_int32 * 2)()
        ns = PORT.ja1f_npc_sub_waits(ISO, EXE, op, q, c, sw)
        if ns < 1 or (npc_past(op, c, q) and sw[0] != -1) or (not npc_past(op, c, q) and sw[0] != waits[0][1]):
            print(f"MISMATCH npc case {i} op {op} q {q} c {c}: Subtitles wait {list(sw)} ({ns}) vs {waits}")
            bad += 1
            continue
        out = ctypes.create_string_buffer(9 * 64000)
        rc = PORT.ja1f_npc_balloon(ISO, EXE, op, q, c, base, out)
        p = out.raw
        for f in range(9):
            o, pf = shots[f if f < 5 else f + parts - 1], p[f * 64000:(f + 1) * 64000]
            if rc != 9 or o != pf:
                n = next((m for m in range(64000) if o[m] != pf[m]), -1)
                print(f"MISMATCH npc case {i} op {op} q {q} c {c}: rc {rc}, screen {f} @({n % 320},{n // 320}) "
                      f"{o[n]:#x} vs {pf[n]:#x}" if n >= 0 else f"MISMATCH npc case {i}: rc {rc}")
                bad += 1
                break
if PORT:
    # #2 row 62: the plant-repair quote number of an unlisted capacity, past the end of the file (seen live 2026-11-06).
    sw = (ctypes.c_int32 * 2)()
    if PORT.ja1f_npc_sub_waits(ISO, EXE, 302, 2629216, 6, sw) != 1 or sw[0] != -1:
        print(f"MISMATCH npc past the end: Subtitles wait {list(sw)}")
        bad += 1
# turns.md "Sector line": the native guide's line, 0x297D4 (record, sector) vs ja1f_sector_balloon; the state it
# sets (shown_slot 0x229CB4, show_timer 0x93D54, sector_spoken 0x22A075[cur_sector]); +0x30 = 0 draws nothing.
# Then op 167 speech_ready = CountLiveMercs? 0x410C4 on fuzzed merc records vs ja1f_live_mercs.
nat = live = 0
SHOWN, TIMER, SPOKEN, CUR = 0x229CB4, 0x93D54, 0x22A075, 0x22A039
for i in range(40 if bad <= 10 else 0):
    k, sector = r.randrange(8), r.choice([r.randrange(60), r.randrange(256)])
    rec = bytearray(orig_rec)
    rec[0x17A], rec[0x180], rec[0x179] = k, (k >= 4) if r.random() < 0.8 else r.randrange(2), r.choice([60, 61, 62, 63, r.randrange(64)])
    rec[0x30] = 0 if i % 8 == 7 else r.choice([0x2A, 1, 0x50])
    e.uc.mem_write(REC, bytes(rec))
    opt = int(r.random() < 0.7)
    e.w8(e.rt(OPT_TEXT), opt)
    cur_s = r.randrange(60)
    e.w8(e.rt(CUR), cur_s)
    e.w8(e.rt(SPOKEN + cur_s), 0)
    e.uc.mem_write(e.rt(SHOWN), b"\x77\0\0\0")
    e.uc.mem_write(e.rt(TIMER), b"\0\0\0\0")
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen, base)
    shots.clear()
    e.call(e.rt(0x297D4), REC, sector)
    nat += 1
    state = (e.r32(e.rt(SHOWN)), e.r32(e.rt(TIMER)), e.r8(e.rt(SPOKEN + cur_s)))
    want = (k, 5000, 1) if rec[0x30] else (0x77, 0, 0)
    if state != want or len(shots) != (9 if rec[0x30] else 0):
        print(f"nat case {i}: state {state} (want {want}), {len(shots)} screens")
        bad += 1
        continue
    if PORT and rec[0x30]:
        out = ctypes.create_string_buffer(9 * 64000)
        rc = PORT.ja1f_sector_balloon(ISO, EXE, bytes(rec), sector, opt, base, out)
        p = out.raw
        for f in range(9):
            o, pf = shots[f], p[f * 64000:(f + 1) * 64000]
            if rc != 9 or o != pf:
                n = next((m for m in range(64000) if o[m] != pf[m]), -1)
                print(f"MISMATCH nat case {i} slot {k} profile {rec[0x179]} sector {sector} text {opt}: rc {rc}, screen {f} "
                      f"@({n % 320},{n // 320}) {o[n]:#x} vs {pf[n]:#x}" if n >= 0 else f"MISMATCH nat case {i}: rc {rc}")
                bad += 1
                break
mercs0 = bytes(e.uc.mem_read(e.rt(SOLDIERS), 8 * 512))
for i in range(300 if bad <= 10 else 0):
    m = bytearray(mercs0)
    for j in range(8):
        b = 512 * j
        m[b + 0x179] = r.choice([r.randrange(0x3A, 0x3F), r.randrange(256)])
        m[b + 0x182] = r.choice([0, 1, r.randrange(256)])
        m[b + 0x183] = r.choice([0, 1, 0x7F, 0x80, 0xFF, r.randrange(256)])
    e.uc.mem_write(e.rt(SOLDIERS), bytes(m))
    o = e.call(e.rt(0x410C4)) & 0xFFFFFFFF
    live += 1
    if PORT:
        pv = PORT.ja1f_live_mercs(bytes(m))
        if pv != o:
            print(f"MISMATCH live case {i}: {o} vs {pv}")
            bad += 1
e.uc.mem_write(e.rt(SOLDIERS), mercs0)
print(f"balloon_diff: {nat} native sector-line cases + {live} live-merc counts")
print(f"balloon_diff: {len(cases)} cases + {san} Santino cases ({texts} with text beside the face) + {npc} guide / Brenda"
      f" cases ({two} with a second part), {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
