#!/usr/bin/env python3
"""Differential test of open #33 sweep rows, original vs the port. Parts:
  opencd : OpenCDFile 0x38304 (iso::game_file): real body with FileExists? and fopen_ stubbed over a fuzzed game folder;
           the source (folder or CD), the path form and the bytes.
  bio    : LoadBioQuoteEdt 0x234E0 (aim::edt + the A.I.M. page slice), every profile, real OpenCDFile.
  face   : FacePanelX? 0x23214 and FacePanelY? 0x23244 (aim::face_xy), every slot byte.
  merc   : MsgMercText 0x15910 (play::merc_text): the text at each of the 12 call sites read from the code, ShowMessage?
           stubbed; the text, the merc rule (+0x17A < 8), and the box arguments.
  retreat: DrawMercText 0x27B38 with the real DrawWrappedText? (eod::EodHost op 57), the 2 call sites of
           MsgRetreatCasualty? 0x30B04; all 64000 screen bytes over a fuzzed picture.
  tileanim: StartTileAnim 0x15720 (play::AnimTimers::start_table), the frame tables of its 3 call sites over fuzzed free
           and busy animations: counts, tiles, frame tables, the tile's byte 3.
  replay : OpenReplayRec 0x2C834 and CopyReplayRec? 0x2C7D0 with fopen_, fread_, fwrite_, fclose_ stubbed: the 3 x n bytes
           that reach QUICK.SAV and that come back, against quicksave::write and restore (their count from block 36).
  chunks : CopySaveChunks? 0x4E840 (World::from_parts): fopen_, fread_, fwrite_, fclose_ stubbed; the row bytes 169 of a real
           day save fuzzed, its tail random; each file's name, mode, bytes, and the read sizes in order.
  sees   : MercSees? 0x56D28 (sighting::merc_sees): the 3 bytes and the byte after them, fuzzed registers.
  stop   : RedrawMercPortrait? 0x336B4 (play::stop_redraw): 0x31C98 stubbed; the draw, its slot, and cursor tile 0x229D2C.
  lip    : ReleaseTacticalTimer? 0x66D94 (faceanim::LipSync::release): AIL_release_timer_handle stubbed; a real scan and
           timer run before it (some cases stop inside a silence); the handle, the silence byte and the running byte.

usage: .venv/bin/python validate/diff/sweeprows_diff.py [ncases=60] [DUMP]
Exits 1 on mismatch or when a part does not reach its cases.
"""
import ctypes, os, random, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402
import common  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
ncases = int(argv[0]) if argv else 60
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"sweeprows_diff: {dump} missing")
    sys.exit(1)
P = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
ISO, EXE = (os.path.join(ROOT, *q).encode() for q in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
e = Emu(dump=dump, fast=True)
rt = e.rt
r = random.Random(0x234E0)
bad = 0
SOLDIER = 512


def fail(part, case, text):
    global bad
    bad += 1
    if bad <= 12:
        print(f"{part} case {case}: {text}")


e.stub_sprintf()
# --- OpenCDFile 0x38304
OPEN_CD, FILE_EXISTS, FOPEN, FP = 0x38304, 0x4707C, 0x75EA0, 0x7F00
DRIVE = chr(e.r8(rt(0x92F8C)))     # the CD letter that InitCDROM found
FILES = ["DAT\\QUESTS.DAT", "DAT\\XRA.DAT", "DAT\\TILES0.DAT", "EDT\\BIOQUOTE.EDT", "MAIN.DAT", "NO\\SUCH.FIL"]
CD = {}
for nm in FILES:
    # Some names are not on the CD (the EDT folder, MAIN.DAT): those come from the game folder only.
    q = subprocess.run(["bsdtar", "-xOf", os.path.join(ROOT, "rom", "JA1.gog"), nm.replace("\\", "/")],
                       capture_output=True)
    CD[nm] = q.stdout if q.returncode == 0 else None
folder = {}
st = {"paths": [], "modes": [], "data": b"", "pos": 0}


def file_exists(em, eax, *_):
    return 1 if em.cstr(eax).decode("latin-1") in folder else 0


def fopen(em, eax, edx, *_):
    path, mode = em.cstr(eax).decode("latin-1"), em.cstr(edx)
    st["paths"].append(path)
    st["modes"].append(mode)
    data = folder.get(path)
    if data is None and path[:3] == DRIVE + ":\\":
        data = CD.get(path[3:])
    if data is None:
        return 0
    st["data"], st["pos"] = data, 0
    return FP


e.stub(rt(FILE_EXISTS), file_exists)
e.stub(rt(FOPEN), fopen)
seen = {"folder": 0, "cd": 0, "none": 0}
with tempfile.TemporaryDirectory() as tmp:
    fake_exe = os.path.join(tmp, "JA.EXE").encode()
    os.makedirs(os.path.join(tmp, "DAT"))
    os.makedirs(os.path.join(tmp, "EDT"))
    name_at = e.alloc(bytes(64))
    for case in range(ncases):
        folder.clear()
        for nm in FILES:
            f = os.path.join(tmp, *nm.split("\\"))
            if r.random() < 0.4 and "NO" not in nm:
                folder[nm] = b"folder " + nm.encode() + bytes([case & 255])
                open(f, "wb").write(folder[nm])
            elif os.path.exists(f):
                os.remove(f)
        for nm in FILES:
            e.uc.mem_write(name_at, nm.encode().ljust(64, b"\0"))
            st["paths"].clear()
            st["modes"].clear()
            got = e.call(rt(OPEN_CD), name_at) & 0xFFFFFFFF
            want = nm if nm in folder else DRIVE + ":\\" + nm
            if st["paths"] != [want] or st["modes"] != [b"rb"]:
                fail("opencd", case, f"{nm}: original path {st['paths']} mode {st['modes']}, wanted {want}")
            data = folder.get(nm, CD[nm])
            if (got == 0) != (data is not None) or (got != 0 and got != 0xFFFFFFFF):
                fail("opencd", case, f"{nm}: original result {got:#x}")
            cap = 1 << 22
            out = ctypes.create_string_buffer(cap)
            n = P.ja1f_game_file(ISO, fake_exe, nm.encode(), out, cap)
            if data is None:
                seen["none"] += 1
                if n != -1:
                    fail("opencd", case, f"{nm}: port found {n} bytes, the original finds nothing")
            else:
                seen["folder" if nm in folder else "cd"] += 1
                if n != len(data) or out.raw[:min(n, cap)] != data[:cap]:
                    fail("opencd", case, f"{nm}: port {n} bytes, original {len(data)}")
if 0 in seen.values():
    fail("opencd", -1, f"cases {seen}")

# --- LoadBioQuoteEdt 0x234E0 (the game folder holds the file)
LOAD_BIO, BIO_BUF, DAT_FILE = 0x234E0, 0xED1C0, 0x92F54
BIO = common.game_read("EDT/BIOQUOTE.EDT")
folder.clear()
folder["EDT\\BIOQUOTE.EDT"] = BIO
closed = []


def fseek(em, eax, edx, ebx, *_):
    if eax != FP or ebx != 0:
        fail("bio", -1, f"fseek file {eax:#x} whence {ebx}")
    st["pos"] = edx
    return 0


def fread(em, eax, edx, ebx, ecx):
    if ecx != FP:
        fail("bio", -1, f"fread file {ecx:#x}")
    b = st["data"][st["pos"]:st["pos"] + edx * ebx]
    st["pos"] += len(b)
    em.uc.mem_write(eax, b)
    return len(b) // edx


e.stub(rt(0x757B1), fseek)
e.stub(rt(0x758AD), fread)
e.stub(rt(0x75A77), lambda em, eax, *_: closed.append(eax) or 0)
nprof = 0
for pi in range(len(BIO) // 320):
    e.uc.mem_write(rt(BIO_BUF), bytes(320))
    closed.clear()
    e.call(rt(LOAD_BIO), pi)
    orig = bytes(e.uc.mem_read(rt(BIO_BUF), 320))
    out = ctypes.create_string_buffer(320)
    n = P.ja1f_aim_bio(ISO, EXE, pi, out)
    if closed != [FP]:
        fail("bio", pi, f"closed {closed}")
    if st["paths"][-1] != "EDT\\BIOQUOTE.EDT":
        fail("bio", pi, f"path {st['paths'][-1]}")
    if n != 320 or out.raw != orig:
        fail("bio", pi, f"record differs, port length {n}")
    nprof += 1
if nprof < 60:
    fail("bio", -1, f"only {nprof} records")

# --- FacePanelX? 0x23214, FacePanelY? 0x23244
for k in range(256):
    x, y = ctypes.c_int32(), ctypes.c_int32()
    P.ja1f_face_xy(k, ctypes.byref(x), ctypes.byref(y))
    ox, oy = e.call(rt(0x23214), k) & 0xFFFFFFFF, e.call(rt(0x23244), k) & 0xFFFFFFFF
    if (ox, oy) != (x.value & 0xFFFFFFFF, y.value & 0xFFFFFFFF):
        fail("face", k, f"original ({ox},{oy}) port ({x.value},{y.value})")

# --- MsgMercText 0x15910
MSG, SHOW_MSG = 0x15910, 0x27B98
SITES = [0x108C2, 0x15354, 0x153AF, 0x159B3, 0x314FD, 0x3A65C, 0x3D78D, 0x3D968, 0x40193, 0x405EF, 0x497A1, 0x5430A]
boxes = []


def show_message(em, eax, edx, ebx, ecx):
    sp = em.uc.reg_read(UC_X86_REG_ESP)
    boxes.append((em.cstr(eax), edx, ebx, ecx, [em.r32(sp + 4 + 4 * i) for i in range(4)]))
    return 0


e.stub(rt(SHOW_MSG), show_message, 16)
PORT_TEXT = {}
blank = bytes(SOLDIER)
for code in list(range(0, 30)) + [100]:
    b = ctypes.create_string_buffer(200)
    n = P.ja1f_merc_text(blank, code, b, 200)
    if n >= 0:
        PORT_TEXT[b.raw[:n].lstrip(b" ")] = code
site_text = {}
for site in SITES:
    code = bytes(e.uc.mem_read(rt(site - 48), 48))
    at = code.rfind(b"\xBA")   # mov edx, imm32
    while at >= 0 and not rt(0x90000) <= int.from_bytes(code[at + 1:at + 5], "little") < rt(0x9CB37):
        at = code.rfind(b"\xBA", 0, at)
    if at < 0:
        site_text[site] = None
        continue
    site_text[site] = int.from_bytes(code[at + 1:at + 5], "little")
    txt = e.cstr(site_text[site])
    if txt not in PORT_TEXT:
        fail("merc", site, f"site text {txt!r} is not in play::merc_text")
if sum(v is not None for v in site_text.values()) < 8:
    fail("merc", -1, f"site texts {site_text}")
rec_at = e.alloc(bytes(SOLDIER))
ms_seen, m_seen = set(), {"merc": 0, "other": 0}
texts = sorted({v for v in site_text.values() if v is not None})
for case in range(ncases * 4):
    rec = bytearray(r.randbytes(SOLDIER))
    slot = r.choice([0, 1, 7, 8, 9, 20, 47, 255])
    rec[0x17A] = slot
    name = bytes(r.randint(0x41, 0x7A) for _ in range(r.randint(1, 8)))
    rec[0x16E:0x177] = name.ljust(9, b"\0")
    e.uc.mem_write(rec_at, bytes(rec))
    ta = r.choice(texts)
    txt = e.cstr(ta)
    ms = r.choice([0, 700, 800, 1500, 2000])
    boxes.clear()
    e.call(rt(MSG), rec_at, ta, ms)
    b = ctypes.create_string_buffer(200)
    n = P.ja1f_merc_text(bytes(rec), PORT_TEXT[txt], b, 200) if txt in PORT_TEXT else -1
    m_seen["merc" if slot < 8 else "other"] += 1
    if slot >= 8:
        if boxes or n != -1:
            fail("merc", case, f"slot {slot}: original boxes {len(boxes)} port {n}")
        continue
    if len(boxes) != 1 or n < 0 or boxes[0][0] != b.raw[:n]:
        fail("merc", case, f"original {boxes} port {b.raw[:max(n, 0)]!r}")
        continue
    # The box: x 0, y 0, ecx 1, then the stack (1, 0, ms, 1): the time limit is the third stack word.
    _, edx, ebx, ecx, stack = boxes[0]
    if (edx, ebx, ecx) != (0, 0, 1) or stack != [1, 0, ms, 1]:
        fail("merc", case, f"box arguments {edx} {ebx} {ecx} {stack} ms {ms}")
if 0 in m_seen.values():
    fail("merc", -1, f"cases {m_seen}")

# --- DrawMercText 0x27B38 (real DrawWrappedText? and fonts)
DRAW, SCREEN = 0x27B38, 0x95678
scr = e.r32(rt(SCREEN))
RTEXT = {3: 0x9AD4F, 4: 0x9AD69}
for n_, a in RTEXT.items():
    t = e.cstr(rt(a))
    if t not in (b"is shot retreating!", b"dies retreating!"):
        fail("retreat", -1, f"text {n_} is {t!r}")
for case in range(ncases):
    pic = r.randbytes(64000)
    rec = bytearray(SOLDIER)
    name = bytes(r.choice(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz") for _ in range(r.randint(1, 8)))
    rec[0x16E:0x177] = name.ljust(9, b"\0")
    n_ = r.choice([3, 4])
    e.uc.mem_write(rec_at, bytes(rec))
    e.uc.mem_write(scr, pic)
    e.call(rt(DRAW), rec_at, rt(RTEXT[n_]), max_steps=2_000_000)
    orig = bytes(e.uc.mem_read(scr, 64000))
    port = ctypes.create_string_buffer(pic, 64000)
    if P.ja1f_retreat_line(ISO, EXE, bytes(rec), n_, port) != 0:
        fail("retreat", case, "port error")
    if orig == pic:
        fail("retreat", case, "the original drew nothing")
    if orig != port.raw:
        d = [i for i in range(64000) if orig[i] != port.raw[i]]
        fail("retreat", case, f"{name} text {n_}: {len(d)} pixels, first ({d[0] % 320},{d[0] // 320}) "
                              f"orig {orig[d[0]]:#x} port {port.raw[d[0]]:#x}")


# --- StartTileAnim 0x15720
ANIM, TILES = 0x229E24, 0x100A18
TABLES = {0x95659: (1, 0x12), 0x9562E: (2, 0x2B)}   # 0x110EF the ripple; 0x1769B, 0x24068 the fire
ta_seen = {"free": 0, "full": 0, "one": 0}
for case in range(ncases * 10):
    table = r.choice(list(TABLES))
    pid, cnt = TABLES[table]
    if case % 4 == 3:          # other counts than the call sites' (count 1 = no byte 3, 0x15750)
        cnt = r.choice([1, 1, 2, r.randrange(1, 0x80)])
    busy = r.choice([0.0, 0.5, 0.9, 1.0])
    counts = [r.randrange(1, 0x2C) if r.random() < busy else 0 for _ in range(10)]
    atile = [r.randrange(3136) for _ in range(10)]
    frames = [r.randrange(1, 3) for _ in range(10)]
    id_to_addr = {1: 0x95659, 2: 0x9562E}
    tile = r.randrange(3136)
    for k in range(10):
        a = rt(ANIM) + 11 * k
        e.w8(a, counts[k])
        e.uc.mem_write(a + 5, atile[k].to_bytes(2, "little"))
        e.w32(a + 7, id_to_addr[frames[k]])
    old_byte3 = r.randrange(256)
    e.w8(rt(TILES) + tile * 8 + 3, old_byte3)
    e.call(rt(0x15720), tile, table, cnt, 5)
    o_counts = [e.r8(rt(ANIM) + 11 * k) for k in range(10)]
    o_tile = [int.from_bytes(bytes(e.uc.mem_read(rt(ANIM) + 11 * k + 5, 2)), "little") for k in range(10)]
    o_frames = [e.r32(rt(ANIM) + 11 * k + 7) for k in range(10)]
    o_byte3 = e.r8(rt(TILES) + tile * 8 + 3)
    pc = (ctypes.c_int * 10)(*counts)
    pt = (ctypes.c_int16 * 10)(*atile)
    pf = (ctypes.c_uint8 * 10)(*frames)
    tb = bytearray(25088)
    tb[tile * 8 + 3] = old_byte3
    tbuf = (ctypes.c_uint8 * 25088).from_buffer(tb)
    k_ = P.ja1f_tile_anim_start(pc, pt, pf, tile, pid, cnt, tbuf)
    ta_seen["full" if k_ < 0 else "free"] += 1
    ta_seen["one"] += k_ >= 0 and cnt == 1
    p_frames = [id_to_addr[v] if v in id_to_addr else 0 for v in pf]
    # animation 0 is outside the search on both sides: the records 1..9 are the compare
    if (list(pc)[1:], [v & 0xFFFF for v in pt][1:], p_frames[1:], tb[tile * 8 + 3]) != (o_counts[1:], o_tile[1:], o_frames[1:], o_byte3):
        fail("tileanim", case, f"tile {tile} table {table:#x}: original {o_counts} {o_tile} {o_byte3} port {list(pc)} {list(pt)} {tb[tile * 8 + 3]}")
if 0 in ta_seen.values():
    fail("tileanim", -1, f"cases {ta_seen}")

# --- OpenReplayRec 0x2C834 + CopyReplayRec? 0x2C7D0
KBD, BUFP, DATF, REC = 0x22A46C, 0x93E64, 0x92F54, 0x222B4C
rbuf = e.alloc(3 * 0x1000)
e.w32(rt(BUFP), rbuf)
e.w32(rt(DATF), 0xD47)
files = {}
log = []


def rr_fopen(em, eax, edx, *_):
    files["name"] = em.cstr(eax)
    files["mode"] = em.cstr(edx)
    return 0x4EC


def rr_fread(em, eax, edx, ebx, ecx):
    em.uc.mem_write(eax, files[ecx][:edx * ebx])
    log.append(("read", ecx, edx, ebx))
    return ebx


def rr_fwrite(em, eax, edx, ebx, ecx):
    files[("out", ecx)] = bytes(em.uc.mem_read(eax, edx * ebx))
    log.append(("write", ecx, edx, ebx))
    return ebx


for a_, f_, pop_ in ((0x75EA0, rr_fopen, 0), (0x758AD, rr_fread, 0), (0x75FE3, rr_fwrite, 0),
                     (0x75A77, lambda em, *_: log.append(("close",)) or 0, 0)):
    e.stub(rt(a_), f_, pop_)
rp = 0
for case, n_ in enumerate([0, 1, 2, 3, 40, 300, 0xFFF] + [r.randrange(1, 800) for _ in range(ncases // 4)]):
    data = bytes(r.randrange(256) for _ in range(3 * n_))
    e.w32(rt(KBD), (n_ << 16) | r.randrange(0x10000))
    tail, back, cnt = (ctypes.create_string_buffer(3 * n_ + 1), ctypes.create_string_buffer(3 * n_ + 1), ctypes.c_int(-1))
    if P.ja1f_replay_tail(ISO, EXE, data, n_, tail, ctypes.byref(cnt), back) != 0:
        fail("replay", case, f"port error for {n_} records")
        continue
    # Quick Save: REPLAY.REC (rb) -> the dat file
    files.clear()
    log.clear()
    files[0x4EC] = data
    e.call(rt(0x2C834))
    saved = files.get(("out", 0xD47), b"")
    if (files["name"], files["mode"]) != (b"replay.rec", b"rb") or log != [("read", 0x4EC, 3, n_), ("write", 0xD47, 3, n_), ("close",)]:
        fail("replay", case, f"save calls {files.get('name')} {files.get('mode')} {log}")
    if saved != tail.raw[:3 * n_] or cnt.value != n_:
        fail("replay", case, f"save tail of {n_} records, count {cnt.value}")
    # Quick Restore: the dat file -> REPLAY.REC (wb)
    files.clear()
    log.clear()
    files[0xD47] = saved
    e.call(rt(0x2C7D0))
    came = files.get(("out", 0x4EC), b"")
    if (files["name"], files["mode"]) != (b"replay.rec", b"wb") or log != [("read", 0xD47, 3, n_), ("write", 0x4EC, 3, n_), ("close",)]:
        fail("replay", case, f"restore calls {files.get('name')} {files.get('mode')} {log}")
    if came != back.raw[:3 * n_]:
        fail("replay", case, f"restored log of {n_} records")
    rp += 1
if rp < 7:
    fail("replay", -1, f"only {rp} cases")

# --- CopySaveChunks? 0x4E840
ROWS, DAT = 0x222B74, 0x92F54
FIXED, TMP = 21649, 18080
day_save = open(os.path.join(ROOT, "build", "DAY_day3.SAV"), "rb").read()[:FIXED]
keep_rows = bytes(e.uc.mem_read(rt(ROWS), 60 * 177))
keep_dat = e.r32(rt(DAT))
P.ja1f_tmp_chunks.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_void_p]
ch_st = {"tail": b"", "pos": 0, "open": None, "files": {}, "sizes": [], "closed": 0}
ch_seen = [0, 0]


def ch_fopen(em, eax, edx, *_):
    ch_st["open"] = (em.cstr(eax).decode(), em.cstr(edx).decode())
    ch_st["files"][ch_st["open"]] = bytearray()
    return 0x5A0


def ch_fread(em, eax, edx, ebx, ecx):
    n_ = edx * ebx
    em.uc.mem_write(eax, ch_st["tail"][ch_st["pos"]:ch_st["pos"] + n_])
    ch_st["pos"] += n_
    if ecx != 0xD48:
        fail("chunks", -2, f"fread handle {ecx:#x}")
    if n_ != 5:
        ch_st["sizes"].append(n_)
    return ebx


def ch_fwrite(em, eax, edx, ebx, ecx):
    ch_st["files"][ch_st["open"]] += bytes(em.uc.mem_read(eax, edx * ebx))
    if ecx != 0x5A0:
        fail("chunks", -2, f"fwrite handle {ecx:#x}")
    return ebx


def ch_fclose(em, eax, *_):
    ch_st["closed"] += 1
    return 0


for a_, f_ in ((0x75EA0, ch_fopen), (0x758AD, ch_fread), (0x75FE3, ch_fwrite), (0x75A77, ch_fclose)):
    e.stub(rt(a_), f_, 0)
e.w32(rt(DAT), 0xD48)
for case, cnt_ in enumerate([0, 1, 2, 4]):
    secs = sorted(r.sample(range(60), cnt_))
    if case == 3:
        secs[0], secs[-1] = 0, 59
    rows = bytearray(keep_rows)
    sv = bytearray(day_save)
    for s_ in range(60):
        v_ = r.choice([1, 2, 0x80, 255]) if s_ in secs else 0
        rows[s_ * 177 + 0xA9] = v_
        sv[10064 + s_ * 177 + 169] = v_
    tail = r.randbytes(TMP * cnt_)
    e.uc.mem_write(rt(ROWS), bytes(rows))
    ch_st.update(tail=tail, pos=0, open=None, files={}, sizes=[], closed=0)
    e.call(rt(0x4E840))
    want_names = {(f"sec{s_:02d}.tmp", "wb") for s_ in secs}
    if set(ch_st["files"]) != want_names or ch_st["closed"] != cnt_ or ch_st["sizes"] != [0x960] * cnt_ or ch_st["pos"] != len(tail):
        fail("chunks", case, f"original files {sorted(ch_st['files'])} closed {ch_st['closed']} read {ch_st['pos']}")
    out = ctypes.create_string_buffer(60 * TMP)
    have = ctypes.create_string_buffer(60)
    sv_all = bytes(sv) + tail + bytes(136)
    q_ = P.ja1f_tmp_chunks(ISO, EXE, sv_all, len(sv_all), out, have)
    if q_ != 0 or [i_ for i_ in range(60) if have.raw[i_]] != secs:
        fail("chunks", case, f"port {q_}: sectors {[i_ for i_ in range(60) if have.raw[i_]]}, original {secs}")
    for s_ in secs:
        o_ = bytes(ch_st["files"].get((f"sec{s_:02d}.tmp", "wb"), b""))
        if o_ != out.raw[s_ * TMP:(s_ + 1) * TMP] or len(o_) != TMP:
            fail("chunks", case, f"sector {s_}: {len(o_)} bytes, file differs")
    ch_seen[0] += 1
    ch_seen[1] += cnt_
e.uc.mem_write(rt(ROWS), keep_rows)
e.w32(rt(DAT), keep_dat)

# --- MercSees? 0x56D28
for case in range(ncases):
    a_, b_, c_ = (r.choice([r.randrange(256), r.randrange(1 << 32)]) for _ in range(3))
    sentinel = r.randrange(256)
    e.w8(rt(0x229E03), sentinel)
    e.call(rt(0x56D28), a_, b_, c_)
    out = (ctypes.c_uint8 * 3)()
    P.ja1f_merc_sees(ctypes.c_int(a_ - (1 << 32) if a_ >> 31 else a_), ctypes.c_int(b_ - (1 << 32) if b_ >> 31 else b_),
                     ctypes.c_int(c_ - (1 << 32) if c_ >> 31 else c_), out)
    o_ = [e.r8(rt(0x229E00 + k)) for k in range(3)]
    if o_ != list(out) or e.r8(rt(0x229E03)) != sentinel:
        fail("sees", case, f"{a_:#x} {b_:#x} {c_:#x}: original {o_} port {list(out)}")

# --- RedrawMercPortrait? 0x336B4
draws = []
e.stub(rt(0x31C98), lambda em, eax, edx, *_: draws.append((eax & 0xFF, edx)) or 0, 0)
stop_seen = {0: 0, 1: 0, 3: 0}
TILE, OFF, SEL = 0x229D2C, 0x92DDF, 0x284F11
keep_off, keep_sel, keep_tile = e.r8(rt(OFF)), e.r8(rt(SEL)), e.r32(rt(TILE))
for case in range(ncases * 4):
    off, slot, sel = r.choice([0, 1, 2, 3, 200]), r.choice([r.randrange(16), r.randrange(256)]), r.randrange(16)
    if r.random() < 0.4:
        sel = slot
    old = r.randrange(0x1000, 0x7000)
    dl = r.randrange(256)
    e.w8(rt(OFF), off)
    e.w8(rt(SEL), sel)
    e.w32(rt(TILE), old)
    draws.clear()
    e.call(rt(0x336B4), slot, dl)
    q_ = P.ja1f_stop_redraw(off, slot, sel)
    stop_seen[q_] = stop_seen.get(q_, 0) + 1
    want_draw = [(slot, dl - 256 if dl > 127 else dl)] if q_ & 1 else []
    got = [(s_, d_ - (1 << 32) if d_ >> 31 else d_) for s_, d_ in draws]
    if got != want_draw or e.r32(rt(TILE)) != (0xC41 if q_ & 2 else old):
        fail("stop", case, f"off {off} slot {slot} sel {sel}: original draws {got} tile {e.r32(rt(TILE)):#x}, port {q_}")
e.w8(rt(OFF), keep_off)
e.w8(rt(SEL), keep_sel)
e.w32(rt(TILE), keep_tile)
if 0 in stop_seen.values():
    fail("stop", -1, f"cases {stop_seen}")

# --- ReleaseTacticalTimer? 0x66D94 after a real lip-sync scan and timer run
handles = []
e.stub(rt(0x77B88), lambda em, *_: handles.append(em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)) or 0, 0)
lip_seen = {0: 0, 1: 0}
for case in range(ncases):
    parts = []
    for _ in range(r.randrange(1, 6)):
        parts.append(bytes([128]) * r.randrange(0, 12000))
        parts.append(bytes(r.choice([0, 40, 220, 255]) for _ in range(r.randrange(1, 6000))))
    pcm = b"".join(parts)[:60000]
    ticks = r.randrange(0, 600)
    e.call(rt(0x66D50))
    e.w8(rt(0x337C9E), 1)
    pb = e.alloc(pcm)
    e.call(rt(0x66DB8), pb, len(pcm))
    for _ in range(ticks):
        e.call(rt(0x66CC4))
    was = e.r8(rt(0x337C9F))
    handle = r.randrange(1, 1 << 16)
    e.w32(rt(0x229DFC), handle)
    handles.clear()
    e.call(rt(0x66D94))
    now, running = e.r8(rt(0x337C9F)), e.r8(rt(0x284F1E))
    q_ = P.ja1f_lip_release(pcm, len(pcm), ticks)
    lip_seen[1 if was else 0] += 1
    if handles != [handle] or running != 0 or (q_ & 1) != (1 if was else 0) or (q_ >> 1 & 1) != now or q_ >> 2 & 1:
        fail("lip", case, f"{len(pcm)} bytes, {ticks} ticks: original was {was} now {now} running {running} handles {handles}, port {q_}")
if 0 in lip_seen.values():
    fail("lip", -1, f"cases {lip_seen}")

print(f"sweeprows_diff: {ncases} cases, {bad} mismatches (opencd {seen}, bio {nprof}, merc {m_seen}, "
      f"sites {sum(v is not None for v in site_text.values())}/{len(SITES)}, tileanim {ta_seen}, replay {rp}, stop {stop_seen}, lip {lip_seen}, chunks {ch_seen})")
sys.exit(1 if bad else 0)
