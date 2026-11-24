#!/usr/bin/env python3
"""Differential test of small game functions, original vs the port. Parts:
  quote : QueueQuote? 0x1B2A4 (play::queue_quote); MercQuoteEvent? is stubbed, ai_diff merc_quote tests it.
  save  : AnyGameSaveExists 0x4FDB8 (menu::any_game_save) on a real directory; the original's FileExists sees the same.
  abort : AskAbortDay 0x11DC0 (question::ask_abort) with scripted answers of QuestionBox?.
  hired : 0x50B04 (aim::hired_this_visit) on fuzzed team records.
  dat   : FormatSectorDatName 0x2F7C8 (sound::sector_dat_name); LoadSanimDat? is stubbed, the name is compared.
  cycle : 0x4C26C (office::cycle_palette) with a scripted timer; PaletteSet? and TimerStart are stubbed and counted.
  table : 0x381C0, the table of soldier bitmap numbers (soldier::bitmap_number), from a cleared table.
  quest : LoadQuestRecord 0x614D4 (play::quest_record), every record of build/game113b/DAT/QUESTS.DAT; stubbed file calls.
  replay: RecordReplayEvent 0x3FA04 (play::record_replay_event, replay_count); stubbed fopen, fwrite, fclose.
  marks : ClearMarksBeforeQuote? 0x11418 (cursor::clear_marks_before_quote), real HidePathMarks with no marks shown.
  temp  : InitTempFiles 0x38038 (world::init_temp_files) on a fake folder.
  keys  : ShowAllStatPopups 0x40024 (screen::key_s), key S; DrawPopupInventory? and the wait are stubbed.
  panels: LoadPanels 0x46EC4 (PanelData::load) on the real MAIN.DAT; invdat: LoadInvDat 0x38190 (invdraw::inv_data) on INV.DAT.
  extra, seek: OpenInvExtra 0x25CA4 + speech seek 0x5C16C (Sound::speech_seek) on the real INV\\EXTRA.
  arc   : OpenPictureArchive? 0x5E8E0 + ArcSeek on the archives it opens; astart: 0x47AE4 (MapUi::assign_start).
  gcell : 0x424BC + 0x42500 (mapui::guard::guard_cell), the mark on a guard screen sector.
  faces : 0x2884C + 0x46AC4 (screen::face_kind, clear_face); LoadMercFace is stubbed, the buffer clear is real.

usage: .venv/bin/python validate/diff/smallrows_diff.py [ncases=500] [DUMP]
Exits 1 on mismatch or when a part does not reach its branches.
"""
import ctypes, os, random, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
ncases = int(argv[0]) if argv else 500
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"smallrows_diff: {dump} missing")
    sys.exit(1)
P = ctypes.CDLL(LIB)
e = Emu(dump=dump, fast=True)
rt = e.rt
r = random.Random(11)
bad = 0


def fail(part, case, text):
    global bad
    bad += 1
    if bad <= 12:
        print(f"{part} case {case}: {text}")


# --- QueueQuote? 0x1B2A4
SLOT, EVENT, FOCUS, DELAY, TILE = 0x92DEC, 0x284F27, 0x284F25, 0x284F26, 0x92DEE
PTRS = 0x100958
played = []


def merc_quote(em, eax, *_):
    played.append((eax, em.r8(rt(DELAY))))


e.stub(rt(0x1B324), merc_quote)
seen = {"play": 0, "quiet": 0, "keep": 0}
for case in range(ncases):
    slot = r.choice([0, 3, 7, 8, 0x3F, 0x40, 0x41, 0x80, 0xFF, r.randrange(256)])
    ev = r.choice([0xFE, 0xFE, 0xFD, 5, r.randrange(256)])
    fo, de, ti = r.randrange(256), r.randrange(256), r.randrange(0x10000)
    a = [r.randrange(256), r.randrange(256), r.randrange(256), r.choice([0xC41, r.randrange(0x10000)]), r.randrange(256)]
    e.w8(rt(SLOT), slot), e.w8(rt(EVENT), ev), e.w8(rt(FOCUS), fo), e.w8(rt(DELAY), de)
    e.uc.mem_write(rt(TILE), ti.to_bytes(2, "little"))
    played.clear()
    # a pointer per slot, so the test sees which slot played
    for k in range(0x40):
        e.w32(rt(PTRS) + 4 * k, 0x10000 + k)
    # the stack argument (focus) goes on the Emu stack as the 5th argument
    e.call(rt(0x1B2A4), a[0], a[1], a[2], a[3], a[4])
    o = (e.r8(rt(SLOT)), e.r8(rt(EVENT)), e.r8(rt(FOCUS)), e.r8(rt(DELAY)), int.from_bytes(e.uc.mem_read(rt(TILE), 2), "little"))
    q = ctypes.create_string_buffer(bytes([slot, ev, fo, de]) + ti.to_bytes(2, "little"), 6)
    pd = ctypes.c_int32(-99)
    pl = P.ja1f_queue_quote(q, (ctypes.c_int32 * 5)(*a), ctypes.byref(pd))
    rq = q.raw
    pq = (rq[0], rq[1], rq[2], rq[3], rq[4] | rq[5] << 8)
    if o != pq:
        fail("quote", case, f"fields {o} vs {pq}, args {a}, start {slot},{ev}")
    op = (played[0][0] - 0x10000, played[0][1]) if played else (-1, None)
    pp = (pl, pd.value & 0xFF) if pl >= 0 else (-1, None)
    if op != pp or len(played) > 1:
        fail("quote", case, f"played {played} vs {pp}, start {slot},{ev}")
    seen["play" if played else "quiet"] += 1
    seen["keep"] += a[3] == 0xC41
if min(seen.values()) == 0:
    fail("quote", -1, f"the cases do not reach all branches: {seen}")

# --- AnyGameSaveExists 0x4FDB8
e.stub_sprintf()
files = set()
asked = []


def exists(em, eax, *_):
    n = em.cstr(eax).decode("latin-1")
    asked.append(n)
    return 1 if n.upper() in files else 0


e.stub(rt(0x4707C), exists)
hits = [0, 0]
with tempfile.TemporaryDirectory() as d:
    for case in range(max(40, ncases // 10)):
        for f in os.listdir(d):
            os.unlink(os.path.join(d, f))
        files.clear()
        names = [f"GAME{k}.SAV" for k in range(10)]
        for n in names[case:case + 1] or r.sample(names, r.choice([0, 0, 1, 1, 2, 10])):   # first: each file alone
            files.add(n)
        if r.random() < .3:        # a neighbour that does not count
            open(os.path.join(d, "GAME10.SAV"), "wb").close()
        for n in files:
            open(os.path.join(d, n), "wb").close()
        asked.clear()
        o = e.call(rt(0x4FDB8)) & 0xFFFFFFFF
        pv = P.ja1f_any_game_save(d.encode())
        hits[o != 0] += 1
        # the original's result register holds 0 from the last FileExists when no file exists
        if (o != 0) != (pv != 0):
            fail("save", case, f"{o} vs {pv} with {sorted(files)}")
        if o not in (0, 1):
            fail("save", case, f"result {o}")
        if o == 0 and len(asked) != 10:
            fail("save", case, f"asked {asked}")
if 0 in hits:
    fail("save", -1, f"the cases do not reach both results: {hits}")

# --- AskAbortDay 0x11DC0
ABORTED, RESTART = 0x92E74, 0x92E75
answers, boxes = [], []


def box(em, eax, edx, *_):
    boxes.append((em.cstr(eax), em.cstr(edx).upper()))
    return answers.pop(0)


e.stub(rt(0x278A0), box, pop=32)
res = set()
for case in range(40):
    bits = case & 3
    answers[:] = [bits & 1, bits >> 1 & 1]
    boxes.clear()
    ab, rs = r.choice([0, 0, 1]), r.choice([0, 0, 1])
    e.w8(rt(ABORTED), ab), e.w8(rt(RESTART), rs)
    ret = e.call(rt(0x11DC0)) & 0xFFFFFFFF
    fa, fr = e.r8(rt(ABORTED)), e.r8(rt(RESTART))
    log = ctypes.create_string_buffer(200)
    pr = P.ja1f_ask_abort(bits, log)
    parts = log.raw.split(b"\0")
    pb = [(parts[i], parts[i + 1].upper()) for i in range(0, 2 * ({0: 1}.get(pr, 2)), 2)]
    # the flags change only by the answers: the original never clears them
    want = (ab | (pr != 0), rs | (pr == 3), int(pr != 0))
    if (fa, fr, ret) != want:
        fail("abort", case, f"flags {fa},{fr} result {ret}, port {pr}, start {ab},{rs}")
    if boxes != pb:
        fail("abort", case, f"boxes {boxes} vs {pb}")
    res.add(pr)
if res != {0, 1, 3}:
    fail("abort", -1, f"the cases do not reach all results: {res}")

# --- 0x50B04
SOL = 0xFA958
P.ja1f_hired_this_visit.argtypes = [ctypes.c_char_p, ctypes.c_int32]
counts = set()
for case in range(ncases):
    t = bytearray(8 * 0x200)
    p = r.randrange(256)
    for k in range(8):
        t[k * 0x200 + 0x182] = r.choice([0, 1, 1, 2, 0x80])
        t[k * 0x200 + 0x179] = r.choice([p, p, (p + 1) & 255, r.randrange(256)])
        t[k * 0x200 + 0x19D] = r.choice([0, 1, 1, 0xFF])
    e.uc.mem_write(rt(SOL), bytes(t))
    o = e.call(rt(0x50B04), p | 0xAB00) & 0xFF
    v = P.ja1f_hired_this_visit(bytes(t), p)
    counts.add(o)
    if o != v:
        fail("hired", case, f"{o} vs {v}")
if len(counts) < 4:
    fail("hired", -1, f"counts {counts}")

# --- FormatSectorDatName 0x2F7C8
GTILESET = 0x22A038
names = []
e.stub(rt(0x5E8E0), lambda em, eax, *_: names.append(em.cstr(eax)) or 0)
for ts in range(0, 8):
    e.w8(rt(GTILESET), ts)
    names.clear()
    e.call(rt(0x2F7C8))
    out = ctypes.create_string_buffer(32)
    P.ja1f_sector_dat_name(ts, out)
    if len(names) != 1 or names[0].upper() != out.value:
        fail("dat", ts, f"{names} vs {out.value}")

# --- 0x4C26C
PAL = 0xEDA08
elapsed = [0]
sets, starts = [], []
e.stub(rt(0x47040), lambda em, eax, *_: elapsed[0] & 0xFFFFFFFF)
e.stub(rt(0x47020), lambda em, eax, *_: starts.append(eax) or 0)
e.stub(rt(0x52870), lambda em, eax, edx, ebx, *_: sets.append((eax, edx, ebx)) or 0)
ran = [0, 0]
for case in range(max(100, ncases // 4)):
    pal = bytes(r.randrange(64) for _ in range(1024))     # the original's entries have 4 bytes, the port's 3
    e.uc.mem_write(rt(PAL), pal)
    elapsed[0] = r.choice([80, 81, 79, 0, -1, 0x50, 0x51, r.randrange(-500, 500)])
    sets.clear(), starts.clear()
    e.call(rt(0x4C26C))
    m = bytes(e.uc.mem_read(rt(PAL), 1024))
    o = b"".join(m[k * 4:k * 4 + 3] for k in range(256))
    q = ctypes.create_string_buffer(b"".join(pal[k * 4:k * 4 + 3] for k in range(256)), 768)
    v = P.ja1f_office_cycle(q, elapsed[0])
    ran[v] += 1
    if o != q.raw:
        fail("cycle", case, f"palette, elapsed {elapsed[0]}, first byte {next(i for i in range(768) if o[i] != q.raw[i])}")
    if len(starts) != v or len(sets) != v:
        fail("cycle", case, f"timer starts {len(starts)}, palette sets {len(sets)}, port {v}, elapsed {elapsed[0]}")
if 0 in ran:
    fail("cycle", -1, f"both results needed: {ran}")

# --- 0x381C0
TAB = 0xA5588
e.uc.mem_write(rt(TAB), bytes(8 * 0x2D8))
e.call(rt(0x381C0))
tab = bytes(e.uc.mem_read(rt(TAB), 8 * 0x2D8))
for f in range(8):
    for fr in range(182):
        o = int.from_bytes(tab[f * 0x2D8 + 4 * fr:f * 0x2D8 + 4 * fr + 4], "little")
        v = P.ja1f_bitmap_number(f, fr)
        if o != v:
            fail("table", f * 182 + fr, f"row {f} frame {fr}: {o} vs {v}")

# --- LoadQuestRecord 0x614D4 on the real file of the target game
QFILE = os.path.join(ROOT, "build", "game113b", "DAT", "QUESTS.DAT")
quests = open(QFILE, "rb").read()
qstate = {}


def q_open(em, eax, *_):
    qstate["name"] = em.cstr(eax).decode("latin-1")
    em.w32(rt(0x92F54), 0x5151)


def q_seek(em, eax, edx, ebx, *_):
    qstate["seek"] = (eax, edx, ebx)


def q_read(em, eax, edx, ebx, ecx):
    qstate["read"] = (edx, ebx, ecx)
    off = qstate["seek"][1]
    em.uc.mem_write(eax, quests[off:off + edx * ebx])
    return ebx


e.stub(rt(0x38304), q_open), e.stub(rt(0x757B1), q_seek), e.stub(rt(0x758AD), q_read)
e.stub(rt(0x75A77), lambda em, eax, *_: qstate.__setitem__("closed", eax))
nrec = len(quests) // 250
Q = (ctypes.c_char * len(quests)).from_buffer_copy(quests)
for case, n in enumerate(list(range(5, 5 + nrec))):
    qstate.clear()
    e.uc.mem_write(rt(0x22A302), bytes([0xA5]) * 252)
    e.call(rt(0x614D4), n)
    o = bytes(e.uc.mem_read(rt(0x22A302), 252))
    out = ctypes.create_string_buffer(250)
    P.ja1f_quest_record(Q, len(quests), n, out)
    if o[:250] != out.raw:
        fail("quest", case, f"record {n}: first byte {next(i for i in range(250) if o[i] != out.raw[i])}")
    if o[250:] != b"\xA5\xA5":
        fail("quest", case, "the original wrote past the record")
    if qstate.get("name", "").upper() != "DAT\\QUESTS.DAT" or qstate.get("read") != (0xFA, 1, 0x5151) or qstate.get("closed") != 0x5151 or qstate["seek"][2] != 0:
        fail("quest", case, f"file calls {qstate}")
if nrec < 2:
    fail("quest", -1, f"QUESTS.DAT holds {nrec} records")

# --- RecordReplayEvent 0x3FA04
recfile = {"data": bytearray(), "opens": [], "closed": 0}


def rec_open(em, eax, edx, *_):
    recfile["opens"].append((em.cstr(eax).decode("latin-1"), em.cstr(edx).decode("latin-1")))
    return 0x7171


def rec_write(em, eax, edx, ebx, ecx):
    if ecx != 0x7171:
        fail("replay", -1, f"write to file {ecx:#x}")
    recfile["data"] += bytes(em.uc.mem_read(eax, edx * ebx))
    return ebx


e.stub(rt(0x75EA0), rec_open), e.stub(rt(0x75FE3), rec_write)
e.stub(rt(0x75A77), lambda em, eax, *_: recfile.__setitem__("closed", recfile["closed"] + (eax == 0x7171)))
COUNT = 0x22A46E
for case in range(max(40, ncases // 10)):
    start = r.choice([0, 1, 7, 300, 0xFFFE, 0xFFFF, r.randrange(2000)])
    ev = [[r.randrange(-300, 300) if r.random() < .3 else r.randrange(256) for _ in range(3)] for _ in range(r.randrange(1, 6))]
    e.uc.mem_write(rt(COUNT), start.to_bytes(2, "little"))
    recfile["data"].clear(), recfile["opens"].clear()
    recfile["closed"] = 0
    for a, b, c in ev:
        e.call(rt(0x3FA04), a & 0xFFFFFFFF, b & 0xFFFFFFFF, c & 0xFFFFFFFF)
    oc = int.from_bytes(e.uc.mem_read(rt(COUNT), 2), "little")
    flat = [v for x in ev for v in x]
    out = ctypes.create_string_buffer(3 * len(ev))
    pc = P.ja1f_replay_record(ctypes.c_size_t(start), (ctypes.c_int32 * len(flat))(*flat), ctypes.c_size_t(len(ev)), out)
    if bytes(recfile["data"]) != out.raw:
        fail("replay", case, f"records {bytes(recfile['data']).hex()} vs {out.raw.hex()}")
    if oc != pc:
        fail("replay", case, f"count {oc} vs {pc}, start {start}")
    if recfile["closed"] != len(ev) or recfile["opens"][0][0].lower() != "replay.rec" or not recfile["opens"][0][1].startswith("a"):
        fail("replay", case, f"file calls {recfile['opens'][:1]} closed {recfile['closed']}")

# --- ClearMarksBeforeQuote? 0x11418, the real HidePathMarks? 0x1D8B8 with no marks shown (select_diff tests its rest)
OVER, WX, WY, AP, TILE2 = 0x284F13, 0x22A47E, 0x22A47C, 0x229DEC, 0x229D2C
for case in range(max(40, ncases // 10)):
    v = [r.randrange(256), r.randrange(-500, 500), r.randrange(-500, 500), r.randrange(-99, 99), r.randrange(0x10000)]
    e.w8(rt(OVER), v[0])
    e.uc.mem_write(rt(WX), (v[1] & 0xFFFF).to_bytes(2, "little"))
    e.uc.mem_write(rt(WY), (v[2] & 0xFFFF).to_bytes(2, "little"))
    e.w32(rt(AP), v[3] & 0xFFFFFFFF), e.w32(rt(TILE2), v[4])
    e.w32(rt(0x229DF4), 0)
    e.call(rt(0x11418))
    s16 = lambda a: int.from_bytes(e.uc.mem_read(rt(a), 2), "little", signed=True)
    o = [e.r8(rt(OVER)), s16(WX), s16(WY), e.r32(rt(AP)), e.r32(rt(TILE2))]
    io = (ctypes.c_int32 * 5)(*v)
    P.ja1f_clear_marks(io)
    if o != list(io):
        fail("marks", case, f"{o} vs {list(io)}, start {v}")

# --- InitTempFiles 0x38038 on a fake folder (sprintf_ is the Python stub of part save)
fake = set()
e.stub(rt(0x4707C), lambda em, eax, *_: 1 if em.cstr(eax).decode("latin-1").lower() in fake else 0)
e.stub(rt(0x75D04), lambda em, eax, *_: fake.discard(em.cstr(eax).decode("latin-1").lower()))
for case in range(max(40, ncases // 10)):
    fake.clear()
    have = [r.random() < r.choice([0, .2, 1]) for _ in range(60)]
    if case < 60:
        have = [k == case for k in range(60)]
    fake.update(f"sec{k:02d}.tmp" for k in range(60) if have[k])
    rec = r.randrange(0, 40) if r.random() < .7 else 0
    if rec:
        fake.add("replay.rec")
    extra = {"sec60.tmp", "game0.sav"} if r.random() < .5 else set()
    fake.update(extra)
    e.call(rt(0x38038))
    if fake != extra:
        fail("temp", case, f"the original left {sorted(fake - extra)}")
    pf = (ctypes.c_uint8 * 60)(*have)
    pr = ctypes.c_int32(rec)
    P.ja1f_init_temp_files(pf, ctypes.c_size_t(60), ctypes.byref(pr))
    left = [k for k in range(60) if pf[k]]
    if left or pr.value:
        fail("temp", case, f"the port left {left}, {pr.value} records")
    if any(f"sec{k:02d}.tmp" in fake for k in range(60)) or ("replay.rec" in fake):
        fail("temp", case, "the fake folder")

# --- ShowAllStatPopups 0x40024 (key S)
keys_log = []
e.stub(rt(0x3FF3C), lambda em, eax, *_: keys_log.append(("pop", eax & 0xFF)))
e.stub(rt(0x673AC), lambda em, *_: keys_log.append(("show", em.r8(rt(OVER)))))
e.stub(rt(0x67F34), lambda em, *_: keys_log.append(("wait", em.r8(rt(OVER)))))
ptr = [rt(0xFA958) + 0x200 * k for k in range(8)]
for k in range(8):            # part quote left its marks in the table
    e.w32(rt(PTRS) + 4 * k, ptr[k])
REC_SIZE = 0x200
s_seen = [0, 0]
for case in range(max(100, ncases // 4)):
    soldiers = bytearray(8 * REC_SIZE)
    for k in range(8):
        a, b = r.choice([0, 0, 1, 0x80, 0xFF]), r.choice([0, 1, 1, 0x80])
        if case < 8:
            a, b = int(k == case), int(k == case)
        if case == 8:
            a, b = (1, 0) if k % 2 else (0, 1)
        e.w8(ptr[k] + 0x182, a), e.w8(ptr[k] + 0x138, b)
        soldiers[k * REC_SIZE + 0x182], soldiers[k * REC_SIZE + 0x138] = a, b
    e.w8(rt(OVER), 1)
    keys_log.clear()
    e.call(rt(0x40024))
    o = [v for t, v in keys_log if t == "pop"]
    tail = keys_log[len(o):]
    over = ctypes.c_uint8(1)
    out = ctypes.create_string_buffer(8)
    n = P.ja1f_key_s(bytes(soldiers), ctypes.byref(over), out)
    if o != list(out.raw[:n]):
        fail("keys", case, f"slots {o} vs {list(out.raw[:n])}")
    if tail != [("show", 0), ("wait", 0)] or over.value != 0:
        fail("keys", case, f"tail {tail}, port over_map {over.value}")
    s_seen[bool(o)] += 1
if 0 in s_seen:
    fail("keys", -1, f"both results needed: {s_seen}")

# --- 0x2884C with 0x46AC4 (LoadMercFace 0x286C4 stubbed; 0x28C.. faces_diff tests its picture)
FACES = 0x93428
faces_log = []
e.stub(rt(0x286C4), lambda em, eax, edx, *_: faces_log.append((eax & 0xFF, edx & 0xFF)))
f_seen = [0, 0, 0]
for case in range(max(60, ncases // 8)):
    recs = bytearray(8 * REC_SIZE)
    for k in range(8):
        here = r.choice([0, 1, 1, 0x80]) if case else int(k < 4)
        prof, hp = r.randrange(256), r.choice([0, 1, 50, 0x80, 0xFF, 0x7F])
        e.w8(ptr[k] + 0x182, here), e.w8(ptr[k] + 0x179, prof), e.w8(ptr[k] + 0x183, hp)
        recs[k * REC_SIZE + 0x182], recs[k * REC_SIZE + 0x179], recs[k * REC_SIZE + 0x183] = here, prof, hp
        e.uc.mem_write(e.r32(rt(FACES) + 4 * k), bytes([0x5A]) * 1024)
    faces_log.clear()
    e.call(rt(0x2884C))
    want = []
    for k in range(8):
        rec = bytes(recs[k * REC_SIZE:(k + 1) * REC_SIZE])
        kind = P.ja1f_face_kind(rec)
        f_seen[kind + 1] += 1
        o = bytes(e.uc.mem_read(e.r32(rt(FACES) + 4 * k), 1024))
        pb = ctypes.create_string_buffer(bytes([0x5A]) * 1024, 1024)
        if kind < 0:
            P.ja1f_face_clear(pb)
        else:
            want.append((k, rec[0x179]))
        if o != pb.raw:
            fail("faces", case, f"slot {k} buffer: first byte {next(i for i in range(1024) if o[i] != pb.raw[i])}")
        if kind >= 0 and kind != (0 if rec[0x183] < 0x80 and rec[0x183] > 0 else 1):
            fail("faces", case, f"slot {k} kind {kind}, hp {rec[0x183]}")
    if faces_log != want:
        fail("faces", case, f"loads {faces_log} vs {want}")
if 0 in f_seen:
    fail("faces", -1, f"all kinds needed: {f_seen}")

# --- 0x1CB78 (the two redraws stubbed: tacticalframe_diff and overview_diff test them): which redraw, with which arguments
rd_log = []
e.stub(rt(0x176C8), lambda em, eax, *_: rd_log.append(("t", eax)))
e.stub(rt(0x23B4C), lambda em, eax, edx, ebx, ecx: rd_log.append(("o", eax, edx & 0xFF, ebx, ecx)))
rd_seen = [0, 0]
P.ja1f_redraw_choice.argtypes = [ctypes.c_uint8, ctypes.c_uint8]
for case in range(max(40, ncases // 8)):
    mode, al = r.choice([0, 0, 1, 2, 3, 0x80]), r.choice([0, 1, 2, 0xFF])
    e.w8(rt(0x92DDF), mode)
    rd_log.clear()
    e.call(rt(0x1CB78), al | r.randrange(1 << 24) << 8)
    got = P.ja1f_redraw_choice(mode, al)
    want = [("t", 1)] if got < 0 else [("o", 1, got, 0, 0)]
    rd_seen[got < 0] += 1
    if rd_log != want:
        fail("redraw", case, f"mode {mode} AL {al}: {rd_log} vs {want}")
if 0 in rd_seen:
    fail("redraw", -1, f"both redraws needed: {rd_seen}")

# --- 0x4E700 (the list 0x4D798, the box 0x27D24 and LoadGame 0x4E964 stubbed: slotheaders, msgbox, save_xcompat test them)
rs_log, rs_ret = [], [0, 0]


def rs_list(em, eax, *_):
    rs_log.append(("list", eax & 0xFFFFFFFF))
    e.w32(rt(0x28ECDC), rs_ret[1])
    return rs_ret[0]


e.stub(rt(0x4D798), rs_list)
e.stub(rt(0x27D24), lambda em, eax, *_: rs_log.append(("box", bytes(em.uc.mem_read(eax, 64)).split(b"\0")[0].decode())))
e.stub(rt(0x4E964), lambda em, eax, *_: rs_log.append(("load", eax & 0xFFFFFFFF == rt(0x28EE19))))
e.stub(rt(0x67BF0), lambda em, *_: rs_log.append(("hide",)))
e.stub(rt(0x673AC), lambda em, *_: rs_log.append(("show",)))
txt = ctypes.create_string_buffer(64)
for case, (ret, flag, kind) in enumerate(((0, 0, 0), (0, 1, 0), (1, 1, 1), (5, 2, 1), (1, 0, 2), (7, 0, 2))):
    rs_ret[:] = [ret, flag]
    rs_log.clear()
    e.w8(rt(0x9344B), 1)
    e.call(rt(0x4E700))
    load = P.ja1f_restore_steps(kind, txt)
    want = [("list", 0)]
    if txt.value:
        want.append(("box", txt.value.decode()))
    if load:
        want += [("hide",), ("load", True), ("show",)]
    if rs_log != want:
        fail("restore", case, f"{rs_log} vs {want}")
    if e.r8(rt(0x9344B)) != 0:
        fail("restore", case, "byte 0x9344B stays set")

# --- LoadPanels 0x46EC4 (real member search and PCX read on MAIN.DAT, stubbed file layer): the sheet the port draws from
from common import game_read  # noqa: E402
ISO, EXE = (os.path.join(ROOT, *q).encode() for q in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
file_st = {"data": b"", "pos": 0, "opens": []}


def f_open(em, a, *_):
    file_st["opens"].append(e.cstr(a).decode())
    file_st["pos"] = 0
    return 5


def f_read(em, a, d, b, *_):
    out = file_st["data"][file_st["pos"]:file_st["pos"] + d * b]
    if out:
        e.uc.mem_write(a, bytes(out))
    file_st["pos"] += len(out)
    return len(out) // max(d, 1)


def f_seek(em, a, d, b, *_):
    file_st["pos"] = {0: d, 1: file_st["pos"] + d, 2: len(file_st["data"]) + d}[b]
    return 0


def f_getc(em, *_):
    if file_st["pos"] >= len(file_st["data"]):
        return 0xFFFFFFFF
    file_st["pos"] += 1
    return file_st["data"][file_st["pos"] - 1]


for addr, fn in ((0x75EA0, f_open), (0x758AD, f_read), (0x757B1, f_seek), (0x7648C, f_getc), (0x75A77, lambda em, *_: 0)):
    e.stub(rt(addr), fn)
file_st["data"] = game_read("DAT\\MAIN.DAT")
SHEET = 320 * 160
sheet_at = e.r32(rt(0x93454))
e.uc.mem_write(sheet_at, b"\xEE" * SHEET)
e.w32(rt(0x229D18), 0x55)
e.call(rt(0x46EC4), max_steps=30_000_000)
if e.r32(rt(0x229D18)) != 0:
    fail("panels", 0, f"result {e.r32(rt(0x229D18)):#x}")
orig_sheet = bytes(e.uc.mem_read(sheet_at, SHEET))
port_sheet = ctypes.create_string_buffer(SHEET)
P.ja1f_panel_sheet.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int]
if P.ja1f_panel_sheet(ISO, EXE, port_sheet, SHEET) != SHEET:
    fail("panels", 0, "port sheet size")
elif orig_sheet != port_sheet.raw:
    d = [i for i in range(SHEET) if orig_sheet[i] != port_sheet.raw[i]]
    fail("panels", 0, f"{len(d)} sheet pixels differ, first ({d[0] % 320},{d[0] // 320})")

# --- LoadInvDat 0x38190 (real LoadOffsetBank + ReadWholeFile; offsetbank_diff tests their rules): once flag, name, buffer
INV_BUF, INV_MAX = 0x1163CC, 0xD73C
file_st["data"] = inv_file = game_read("DAT\\INV.DAT")
e.stub(rt(0x4707C), lambda em, a, *_: 1)
e.stub(rt(0x76693), lambda em, *_: 7)
e.stub(rt(0x768E1), lambda em, *_: len(file_st["data"]))
e.stub(rt(0x7691E), lambda em, *_: 0)
e.w8(rt(0x949F0), 0)
e.w32(rt(0x229D18), 0x55)
file_st["opens"].clear()
e.call(rt(0x38190), max_steps=60_000_000)
inv_len = len(inv_file) - 4 - 4     # the original moves the data down over the count; the last 4 bytes come from after the buffer
orig_inv = bytes(e.uc.mem_read(rt(INV_BUF), inv_len))
if e.r8(rt(0x949F0)) != 1 or e.r32(rt(0x229D18)) != 0:
    fail("invdat", 0, f"flag {e.r8(rt(0x949F0))} result {e.r32(rt(0x229D18)):#x}")
if not file_st["opens"] or not file_st["opens"][-1].lower().endswith("dat\\inv.dat"):
    fail("invdat", 0, f"opens {file_st['opens']}")
port_inv = ctypes.create_string_buffer(INV_MAX)
same = ctypes.c_int32(0)
P.ja1f_inv_image.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint32, ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]
got = P.ja1f_inv_image(ISO, EXE, rt(INV_BUF), port_inv, INV_MAX, ctypes.byref(same))
if got != inv_len + 4 or port_inv.raw[:inv_len] != orig_inv:
    d = [i for i in range(inv_len) if port_inv.raw[i] != orig_inv[i]]
    fail("invdat", 0, f"port size {got} vs {inv_len + 4}; {len(d)} bytes differ, first {d[:1]}")
if not same.value:
    fail("invdat", 0, "the port reads the file a second time")
# the flag set: no file call and no write of the result
file_st["opens"].clear()
e.w32(rt(0x229D18), 0x55)
e.call(rt(0x38190))
if file_st["opens"] or e.r32(rt(0x229D18)) != 0x55 or e.r8(rt(0x949F0)) != 1:
    fail("invdat", 1, f"second call: opens {file_st['opens']} result {e.r32(rt(0x229D18)):#x}")

# --- OpenInvExtra 0x25CA4 + speech seek 0x5C16C on the real INV\\EXTRA (real open, directory read, member search, fseek; stubbed
# file layer). Original side only: the release of the quote timer 0x66D94 (an AIL timer handle, the port has no such timer).
file_st["data"] = extra = game_read("INV\\EXTRA")
closes = []
e.stub(rt(0x75A77), lambda em, a, *_: closes.append(a) or 0)
for flag in (0, 1):
    closes.clear()
    file_st["opens"].clear()
    e.w8(rt(0x92F8F), flag)
    e.call(rt(0x25CA4), max_steps=5_000_000)
    nb = ctypes.create_string_buffer(32)
    P.ja1f_inv_extra_name(nb, 32)
    if len(closes) != flag or e.r8(rt(0x92F8F)) != 1 or not file_st["opens"] or not file_st["opens"][-1].endswith(nb.value.decode()):
        fail("extra", flag, f"closes {closes} flag {e.r8(rt(0x92F8F))} opens {file_st['opens']} port {nb.value}")
    if e.r32(rt(0x229D18)) != 0:
        fail("extra", flag, f"open result {e.r32(rt(0x229D18)):#x}")
seek_log = []
e.stub(rt(0x786F6), lambda em, *_: seek_log.append("end"))
e.stub(rt(0x66D94), lambda em, *_: seek_log.append("timer"))
members = [bytes(extra[0x20 + 0x20 * i:0x30 + 0x20 * i]).split(b"\0")[0] for i in range(int.from_bytes(extra[0x14:0x18], "little"))]
members = [m for m in members if m]
if len(members) < 5:
    fail("seek", -1, f"members {members}")
NAME_AT = rt(0x2F0000 - 0x400)
P.ja1f_speech_seek.argtypes = [ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int]
seek_seen = {"found": 0, "miss": 0, "closed": 0, "ended": 0, "timer": 0}
seek_out = ctypes.create_string_buffer(len(extra))
for case in range(max(60, ncases // 4)):
    name = r.choice(members + [b"NO_SUCH", b""]) if case % 3 else r.choice(members).lower()
    is_open, busy, timer = r.randrange(4) > 0, r.randrange(2), r.randrange(2)
    e.w8(rt(0x92F8F), int(is_open))
    e.uc.mem_write(rt(0x96372), bytes([busy, 0]))
    e.w8(rt(0x284F1E), timer)
    e.w32(rt(0x229D18), 0x55)
    e.w32(rt(0x96394), 0x55)
    e.w32(rt(0x337940), 0x55)
    e.uc.mem_write(NAME_AT, name + b"\0")
    seek_log.clear()
    e.call(rt(0x5C16C), NAME_AT, max_steps=500_000)
    res = e.r32(rt(0x229D18))
    stopped = ctypes.c_int32(0)
    got = P.ja1f_speech_seek(bytes(extra) if is_open else None, len(extra) if is_open else -1, name, busy, ctypes.byref(stopped), seek_out,
                             len(extra))
    ended = "end" in seek_log
    if ended != bool(stopped.value):
        fail("seek", case, f"open {is_open} busy {busy}: original ended {ended}, port {stopped.value}")
    if ("timer" in seek_log) != bool(busy and timer):
        fail("seek", case, f"timer {seek_log} open {is_open} busy {busy} flag {timer}")
    if e.r16(rt(0x96372)) != 0:
        fail("seek", case, f"busy word {e.r16(rt(0x96372))}")
    if not is_open:
        # The original seeks on the old handle (the stub serves the same file); the port gives nothing. Every caller opens first.
        if got != -1:
            fail("seek", case, f"closed: port finds {got} B")
        seek_seen["closed"] += 1
    elif res != 0:
        if got >= 0:
            fail("seek", case, f"{name}: original result {res:#x}, port finds {got} B")
        seek_seen["miss"] += 1
    else:
        off, size = e.r32(rt(0x96394)), e.r32(rt(0x337940))
        if got != size or seek_out.raw[:got] != bytes(extra[off:off + size]) or file_st["pos"] != off:
            fail("seek", case, f"{name}: original at {off:#x} size {size} pos {file_st['pos']:#x}, port {got} B")
        seek_seen["found"] += 1
    seek_seen["ended"] += ended
    seek_seen["timer"] += "timer" in seek_log
if 0 in seek_seen.values():
    fail("seek", -1, f"branches {seek_seen}")

# --- OpenPictureArchive? 0x5E8E0: real open of each archive (the original reads 0x2C x 32 B of its directory) and real ArcSeek of
# each member. Compared: open result and each member's bytes (the port has no handle and reads the member from the whole file).
del e.stubs[rt(0x5E8E0)]     # the dat part stubbed it
for arc_name in ("DAT\\OFFICE.DAT", "DAT\\MF1.DAT", "DAT\\MF2.DAT", "DAT\\SANIM.DAT", "DAT\\SANIM2.DAT"):
    file_st["data"] = arc = game_read(arc_name)
    closes.clear()
    flag = r.randrange(2)
    e.w8(rt(0x92F90), flag)
    e.uc.mem_write(NAME_AT, arc_name.encode() + b"\0")
    e.call(rt(0x5E8E0), NAME_AT, max_steps=2_000_000)
    if e.r8(rt(0x92F90)) != 1 or e.r32(rt(0x229D18)) != 0 or len(closes) != flag:
        fail("arc", 0, f"{arc_name}: flag {e.r8(rt(0x92F90))} result {e.r32(rt(0x229D18)):#x} closes {closes}")
    count = int.from_bytes(arc[0x14:0x18], "little")
    names = [bytes(arc[0x20 + 0x20 * i:0x30 + 0x20 * i]).split(b"\0")[0] for i in range(count)]
    if count > 0x2C:
        fail("arc", 0, f"{arc_name}: count {count}, the original reads a directory of 0x2C rows")
    arc_buf = ctypes.create_string_buffer(len(arc))
    stopped = ctypes.c_int32(0)
    for name in [n for n in names if n] + [b"NO_SUCH"]:
        e.uc.mem_write(NAME_AT, name + b"\0")
        OUT_OFF, OUT_SIZE = NAME_AT + 0x40, NAME_AT + 0x44
        e.w32(OUT_OFF, 0x55), e.w32(OUT_SIZE, 0x55)
        got_o = e.call(rt(0x534A4), e.r32(rt(0x92F48)), rt(0x10C158), NAME_AT, OUT_OFF, OUT_SIZE)
        got_p = P.ja1f_speech_seek(bytes(arc), len(arc), name, 0, ctypes.byref(stopped), arc_buf, len(arc))
        if got_o & 0xFFFFFFFF:
            if got_p >= 0:
                fail("arc", 1, f"{arc_name} {name}: original result {got_o:#x}, port {got_p} B")
        else:
            off, size = e.r32(OUT_OFF), e.r32(OUT_SIZE)
            if got_p != size or arc_buf.raw[:got_p] != bytes(arc[off:off + size]):
                fail("arc", 1, f"{arc_name} {name}: original {off:#x} {size}, port {got_p} B")

# --- 0x47AE4 (DrawAssignMap? 0x478A4: assign_diff; AssignPanelFlipIn? 0x47B68: flipin_diff; both stubbed and logged): item places,
# the saved screen, the back buffer, the order. The port's slide takes the map rows of the back buffer from img (flipin_diff).
st_log = []
screen_at = e.r32(rt(0x95678))
sheet2 = e.r32(rt(0x93E5C))
e.stub(rt(0x478A4), lambda em, a, *_: st_log.append(("draw", a & 0xFF, bytes(em.uc.mem_read(rt(0x22A4EF), 0xB2C0)))))
e.stub(rt(0x47B68), lambda em, *_: st_log.append(("flip",)))
e.stub(rt(0x67F90), lambda em, *_: st_log.append(("flush",)))
P.ja1f_assign_start.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_void_p]
for case in range(3):
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen_at, base)
    e.uc.mem_write(rt(0x949F2), b"\xEE" * 0x38)
    st_log.clear()
    e.call(rt(0x47AE4), max_steps=2_000_000)
    saved, places = ctypes.create_string_buffer(64000), ctypes.create_string_buffer(56)
    if P.ja1f_assign_start(ISO, EXE, base, saved, places) != 0:
        fail("astart", case, "port error")
        continue
    orig_saved = b"".join(bytes(e.uc.mem_read(sheet2 + y * 640, 320)) for y in range(200))
    if saved.raw != orig_saved or saved.raw != base:
        fail("astart", case, "saved screen differs")
    if bytes(e.uc.mem_read(rt(0x949F2), 0x38)) != places.raw:
        fail("astart", case, f"places {bytes(e.uc.mem_read(rt(0x949F2), 0x38)).hex()} port {places.raw.hex()}")
    if [x[:2] for x in st_log] != [("draw", 99), ("flip",), ("flush",)] or st_log[0][2] != base[:0xB2C0]:
        fail("astart", case, f"steps {[x[:2] for x in st_log]}")

# --- 0x424BC + 0x42500: the mark on a sector of the guard screen. 0x42500: RestoreSectorCell? and DrawSectorMarkers? stubbed
# (island_diff tests them, the port repaints the map); original side only: the 16 x 16 save of the cell to the back buffer (the port
# has no such save, it repaints each frame). The call site's point is cell (s % 10) * 31 + 20, (s / 10) * 23 + 10.
gc_log = []
e.stub(rt(0x41640), lambda em, a, d, *_: gc_log.append(("cell", a & 0xFFFFFFFF, d)))
e.stub(rt(0x419BC), lambda em, a, d, *_: gc_log.append(("marks", a & 0xFFFFFFFF, d)))
P.ja1f_guard_cell.argtypes = [ctypes.c_char_p, ctypes.c_char_p] + [ctypes.c_int] * 3 + [ctypes.c_void_p]
gc_seen = [0, 0, 0]
for case in range(max(24, ncases // 16)):
    s_, n = r.randrange(60), r.choice([0, 1, 2, 9, 10, 15, 42, 99, 100, 127])
    x0, y0 = (s_ % 10) * 31 + 20, (s_ / 10).__floor__() * 23 + 10
    save_flag = r.randrange(2)
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen_at, base)
    back_before = bytes(r.randrange(256) for _ in range(0x180 * 16))
    e.uc.mem_write(rt(0x22A4EF), back_before)
    gc_log.clear()
    e.call(rt(0x42500), s_, x0, y0, n, save_flag, max_steps=3_000_000)
    if gc_log != [("cell", rt(0x22979C), s_), ("marks", rt(0x22979C), s_)]:
        fail("gcell", case, f"steps {gc_log}")
    back = bytes(e.uc.mem_read(rt(0x22A4EF), 0x180 * 16))
    want_back = bytearray(back_before)
    if save_flag:
        for y in range(16):
            want_back[y * 0x180:y * 0x180 + 16] = base[(y0 + y) * 320 + x0:(y0 + y) * 320 + x0 + 16]
    if back != bytes(want_back):
        fail("gcell", case, f"save flag {save_flag}: the back buffer differs")
    orig_px = bytes(e.uc.mem_read(screen_at, 64000))
    port_px = ctypes.create_string_buffer(base, 64000)
    if n:
        if P.ja1f_guard_cell(ISO, EXE, n, x0, y0, port_px) != 0:
            fail("gcell", case, "port error")
    if orig_px != port_px.raw:
        d = [i for i in range(64000) if orig_px[i] != port_px.raw[i]]
        fail("gcell", case, f"sector {s_} n {n}: {len(d)} pixels, first ({d[0] % 320},{d[0] // 320}) orig {orig_px[d[0]]} port {port_px.raw[d[0]]}")
    gc_seen[0 if n == 0 else 1 if n < 10 else 2] += 1
    # 0x424BC alone
    if n:
        e.uc.mem_write(screen_at, base)
        e.call(rt(0x424BC), n, x0, y0, max_steps=2_000_000)
        if bytes(e.uc.mem_read(screen_at, 64000)) != port_px.raw:
            fail("gcell", case, "0x424BC alone differs")
if 0 in gc_seen:
    fail("gcell", -1, f"cases {gc_seen}")

print(f"smallrows_diff: {ncases} cases, {bad} mismatches ({seen})")
sys.exit(1 if bad else 0)
