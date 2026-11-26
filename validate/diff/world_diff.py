"""World loader + sector entry test (docs/spec/world_load.md) against a REAL game: three guest memory dumps written
by validate/world_ref.py (original from the QICKSTRT save to home sector 60):
  build/mem_wl_restore.bin  right after Restore Game of QICKSTRT (slot 0)
  build/mem_wl_pre.bin      TacticalLoop just before LoadTacticalSector? (debugger breakpoint)
  build/mem_wl_post.bin     right after it returned

load:  a world built from the files exactly as world_load.md sections 1-3 say (the reference loader below, written
       from the spec's numbers) against the dumps: read-only tables vs wl_post, day-save fields + start values vs
       wl_restore. --port: also the clean-room front-end (`ja1front ISO EXE --world SAVE OUT`, world image =
       section 6) must give the reference world byte for byte.
entry: the port world from wl_pre, the map from TERRAIN.DAT (section 4 step 1), then the entry: reference = the
       spec's steps 1-3 and 5 here around the library's ja1_setup_sector; --port = the library's ja1_enter_sector.
       Random = the game's own generator from wl_pre's seed (section 5); file host ops from the CD; ops 51 replace,
       58 reserve_ap, 33 radio_result run the ORIGINAL routines in Unicorn on the current world (not in the library
       yet). Compared with wl_post: every world byte, the end seed and the number of random draws.
enter (--port only): the front-end as host (section 7): `ja1front ISO EXE --enter build/DAY_before_restart.SAV
       SEED OUT` (the original's day save at the start of day 1 in sector 60; its world = wl_pre except sector_state
       bytes 540.., which other data shares) against wl_post, plus its `seed S draws 215` line.
leave (--port only): the same save with the home sector lost, `ja1front --script` one pass: the leave path runs
       (host ops 125, 215, 85, 216, 220; cur_sector 0xFF, leave_reason 4, "passes 1 left 1").
inventory (--port only): `ja1front --script` on build/DAY_fight48d.SAV with `key I` and `inv` lines (world_load.md
       section 8): take the hand item and put it back = the world image of a plain open + Escape except soldier
       +0xB4 (i32 timer); take it and drop it in the ground box = hand empty, its 12 bytes in the ground list; then
       pick it up and put it back = the hand item as before.
eod (--port only): story byte 0x1C = 1 ends the day; `eod 40` script: the end-of-day screen shows 41 frames,
       polls 119 times, draws the text once, writes JA1_EOD_PNG (world_load.md section 8).
dayloop (--port only): two day loops in one script on eod_port.SAV; the third entry of sector 59 reads the tmp
       file of op 219 save_tmp (world_load.md sections 8, 9).
Not compared (not world data or written by the front-end): soldier +0x11A / +0x11C (drawing anchors, world_load.md
section 4 step 3), enemy_watch bytes 0..39 and text_buf (other variables share those addresses), t0_class bytes
157.., the read-only tables' bytes past the real tables (NEIGHBOURS).

usage: .venv/bin/python validate/diff/world_diff.py [--port]
"""
import os
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [os.path.join(ROOT, "tools"), HERE]
import ai  # noqa: E402
import port_lib  # noqa: E402
from common import cd_read, game_read  # noqa: E402
from emu import Emu  # noqa: E402

PORT = "--port" in sys.argv
DUMP = {k: os.path.join(ROOT, "build", f"mem_wl_{k}.bin") for k in ("restore", "pre", "post")}
SAVE = os.path.join(ROOT, "rom", "__support", "save", "QICKSTRT.SAV")
EXE = os.path.join(ROOT, "build", "game113b", "JA.EXE")
SEED = 0x9BBBC                      # Watcom rand_ seed (u32)

# --- world_load.md, as numbers -------------------------------------------------------------------------------------
TABLES = {"item_types": ("exe", 900764, 1632), "anim_table": ("exe", 893450, 2048), "anim_rows": ("exe", 893446, 2048),
          "anim_free": ("exe", 893241, 256), "anim_sounds": ("exe", 893310, 512), "anim_steps_tile": ("exe", 893173, 256),
          "anim_turn_while": ("exe", 893990, 256), "fresher": ("exe", 903691, 2304), "dir_steps": ("exe", 893020, 514),
          "listed": ("exe", 892758, 32), "crates": ("exe", 904548, 16), "fixed_items": ("exe", 905276, 88),
          "weapons": ("exe", 900309, 1536), "absorb": ("exe", 892866, 576), "equip_skill": ("exe", 904428, 6),
          "attach": ("exe", 899918, 328), "anim_scripts": ("jag", 7984, 5440), "t0_class": ("jag", 22128, 157),
          "t1_class": ("jag", 22285, 256), "t2_class": ("jag", 22541, 155),
          "splash": ("exe", 892698, 60), "view_rays": ("exe", 894456, 306), "obj_block": ("exe", 902968, 685),
          "silencer": ("exe", 892836, 17), "shot_sound": ("exe", 900736, 34), "impact_noise": ("exe", 892869, 33),
          "shot_range": ("exe", 906223, 9),
          "posture": ("exe", 893105, 256), "window": ("exe", 903212, 2056), "attack_sound": ("exe", 892682, 10),
          "diff_mod": ("exe", 904434, 6)}
ENTRY_TILES = 904564
START_EXE = {"busy_timer": 892504, "calls": 892510, "combat": 892527, "mission_slot": 892492, "mission_target": 892494,
             "mission_spot": 892660, "turn_based": 892496, "busy": 892636, "show_ai": 892529, "opt_onscreen": 892547,
             "effect_tile": 892656, "bomb_count": 892640,
             "screen_drawn": 892528, "interrupt_go": 892531, "turn_moves": 892532, "interrupted": 892535,
             "quote_slot": 892560, "quote_tile": 892562, "follow_all": 894204, "stop_timer": 904812,
             "in_flight": 892520, "merc_shown": 892524, "interrupt_turn": 892544, "tick_step": 892932, "story_finder": 892551, "spot_stop": 892680,
             "merc_to_show": 892523, "focus_slot": 892662, "round_lock": 892548, "round_lock2": 892549,
             "handed_over": 892545, "redraw_pending": 892488, "turn_flag": 892525, "interrupter": 892534,
             "show_timer": 896504, "speaker": 892537,
             "marked_slot": 904880, "knockdown": 892694, "focus_last": 892663, "hit_scale": 892521,   # (shooting.md batch 2)
             "sees_slot": 892522, "preview_slot": 892668,   # (select.md)
             "ripple_timer": 892828}   # (soldier_update.md Update end)
START_FIXED = {"view_w": 224, "view_h": 168}
SAVE_AT = {"soldiers": (28, 4096), "sector_state": (9524, 540), "sectors": (10064, 10620), "day": (20688, 4),
           "team_on_duty": (20696, 4), "santino_word": (20788, 4), "guard_skill": (20828, 1), "cur_sector": (20837, 1),
           "story": (20844, 35), "crate_state": (20887, 4), "crate_shown": (20891, 4), "supplies": (20970, 4),
           "santino_spoke": (20974, 1), "purifier": (20983, 1), "safe_opened": (21045, 1), "difficulty": (20984, 1), "alarm": (20986, 1),
           "mission_step": (20988, 1), "opt_reload_msg": (20826, 1), "ap_warned": (21016, 8), "alone_warned": (21035, 1),
           "last_spotter": (21044, 1), "opt_step_text": (21543, 1), "opt_half_delay": (21547, 1), "opt_follow": (21548, 1), "plants": (21078, 48), "mission_done": (21287, 80),
           "opt_sound": (21549, 1), "notes": (21619, 30), "opt_move_fx": (21553, 1),
           "merc_tire": (20869, 1), "weather_tire": (21036, 4),
           "replay_m": (20989, 1), "first_hit": (21281, 1), "squad_lost": (21047, 1), "last_killer": (20990, 1), "sector_spoken": (20897, 60), "stage": (21278, 1),
           "redraw_next": (21279, 1), "opt_path_marks": (21545, 1), "opt_ambient": (21546, 1), "enemies_warned": (20987, 1)}
# bytes of the read-only fields past the real tables (other variables at those addresses in the original)
NEIGHBOURS = {"anim_table": lambda o: o >= 68 * 8, "anim_rows": lambda o: o >= 68 * 8,
              "anim_turn_while": lambda o: o >= 68, "fresher": lambda o: not 1152 - 27 <= o <= 1152 + 36,
              "absorb": lambda o: 0x26 <= o < 0x3A or o >= 0x6C, "t0_class": lambda o: o >= 157,
              "t2_class": lambda o: o >= 155,
              "window": lambda o: o in (0x63B, 0x63C)}     # (two UI variables of the original inside the window table)

SMAP, BMAP = port_lib._world_map(ai)
UPDATE_FIELDS = set()   # (all soldier_update.md fields are loaded now, world_load.md)
BUFS = [k for k in BMAP if k != "scratch" and k not in UPDATE_FIELDS]
SCALARS = [k for k in SMAP if k not in UPDATE_FIELDS]


def files():
    return open(EXE, "rb").read(), cd_read("DAT/JAGGED.DAT")


def ref_world(save):
    """world_load.md sections 1-3 (the reference loader)"""
    exe, jag = files()
    bufs = {k: bytearray(n) for k, (_, n) in BMAP.items() if k != "scratch"}
    sc = {k: 0 for k in SCALARS}
    for k, (f, off, n) in TABLES.items():
        bufs[k][:n] = (exe if f == "exe" else jag)[off:off + n]
    for k, off in START_EXE.items():
        if k in bufs:
            bufs[k][:] = exe[off:off + len(bufs[k])]
        else:
            n, sg = SMAP[k][1], SMAP[k][2]
            sc[k] = int.from_bytes(exe[off:off + n], "little", signed=bool(sg))
    sc.update(START_FIXED)
    for k, (off, n) in SAVE_AT.items():
        if k in bufs:
            bufs[k][:n] = save[off:off + n]
            if k == "soldiers":                 # +0x90: the original's address, 0 in the port's world
                for r in range(8):
                    bufs[k][0x200 * r + 0x90:0x200 * r + 0x94] = bytes(4)
        else:
            sc[k] = int.from_bytes(save[off:off + n], "little", signed=bool(SMAP[k][2]))
    sec = bufs["sectors"]
    for i in range(60):
        sec[177 * i:177 * i + 48] = jag[184 + 48 * i:184 + 48 * (i + 1)]
        sec[177 * i + 48:177 * i + 88] = jag[3064 + 40 * i:3064 + 40 * (i + 1)]
    return bufs, sc


def image(bufs, sc):
    """world image (world_load.md section 6)"""
    return b"".join(bytes(bufs[k]) for k in BUFS) + b"".join(struct.pack("<i", ai._i32(sc[k])) for k in SCALARS)


def dump_world(name):
    e = Emu(dump=DUMP[name], fast=True)
    return e, port_lib.world_load(e, ai)


def diffs(a, b, n):
    return [i for i in range(n) if a[i] != b[i]]


fails = 0


def report(tag, bad):
    global fails
    if bad:
        fails += 1
        print(f"MISMATCH {tag}: {bad[:12]}{' ...' if len(bad) > 12 else ''}")


def part_load():
    save = open(SAVE, "rb").read()
    bufs, sc = ref_world(save)
    _, (post, _) = dump_world("post")
    _, (rest, rsc) = dump_world("restore")
    for k in TABLES:
        report(f"table {k}", [hex(o) for o in diffs(bufs[k], post[k], len(bufs[k])) if not NEIGHBOURS.get(k, lambda o: 0)(o)])
    for k in BUFS:
        if k in TABLES or k in ("tiles", "text_buf"):     # the map: built at sector entry; text_buf: other data
            continue
        bad = diffs(bufs[k], rest[k], len(bufs[k]))
        if k == "soldiers":                     # the dump's +0x90 is converted from an address (port_lib)
            bad = [o for o in bad if not 0x90 <= o % 0x200 < 0x94]
        report(f"save/start {k}", [hex(o) for o in bad])
    _, (_, psc) = dump_world("post")
    for k in SCALARS:
        want = psc[k] if k in START_FIXED else rsc[k]      # the view size is set when tactical starts
        if sc[k] != want:
            report(f"save/start {k}", [(sc[k], want)])
    print(f"load: {len(TABLES)} tables, {len(SAVE_AT)} save fields, {len(BUFS) + len(SCALARS) - len(TABLES) - len(SAVE_AT)} start values")
    if PORT:
        out = os.path.join(ROOT, "build", "world_port.bin")
        exe = os.path.join(ROOT, "port", "ja1front", "target", "release", "ja1-port")
        r = subprocess.run([exe, os.path.join(ROOT, "rom", "JA1.gog"), EXE, "--world", SAVE, out],
                           capture_output=True, text=True)
        if r.returncode:
            report("port ja1front --world", [r.stderr.strip()[-300:]])
            return
        # The map before the first sector (InitProgramWorld? 0x377B0): the live dump after Restore Game must hold it too.
        start_map = bytes([0x20, 0, 0, 0xFF, 0, 0x40, 0xFF, 0]) * 3136
        report("start map vs the live restore dump", [hex(o) for o in diffs(rest["tiles"], start_map, len(start_map))])
        got, want = open(out, "rb").read(), image({**bufs, "tiles": start_map}, sc)
        if len(got) != len(want):
            report("port world image size", [(len(got), len(want))])
            return
        bad, o = [], 0
        for k in BUFS:
            n = len(bufs[k])
            bad += [f"{k}+{hex(i)}" for i in diffs(got[o:o + n], want[o:o + n], n)]
            o += n
        for k in SCALARS:
            if got[o:o + 4] != want[o:o + 4]:
                bad.append(f"{k} {struct.unpack('<i', got[o:o + 4])[0]} want {sc[k]}")
            o += 4
        report("port world vs reference", bad)
        print("load --port: ja1front world image compared")


PATCH_KINDS = ("CRATE", "ITEM", "DEADPL", "LIVEPL")
ORIGINAL_OPS = {"replace": 0x2FB98, "reserve_ap": 0x17C88, "radio_result": 0x63254,
                "action_ap": 0x56310, "quest_check": 0x633D8, "shot_ap": 0x564F8, "tile_dist": 0x15A24,
                "reach_ap": 0x3DC4C, "path_ap": 0x1D32C, "direction": 0x52560}     # --port: the library runs replace + reserve_ap (batch 9) and asks action_ap


def part_entry():
    e, (bufs, sc) = dump_world("pre")
    _, (want, wsc) = dump_world("post")
    post_e = Emu(dump=DUMP["post"], fast=True)
    rt = e.rt
    seed = [e.r32(rt(SEED))]
    draws = [0]

    def random(n):
        seed[0] = (seed[0] * 0x41C64E6D + 0x3039) & 0xFFFFFFFF
        draws[0] += 1
        r, m = (seed[0] >> 16) & 0x7FFF, ai.s16(n)
        if m <= 0:
            return 0
        v = ai.cdiv(r * m, 0x7FFF)
        return v - ai.cdiv(v, m) * m
    e.stub(rt(0x53538), lambda em, eax, edx, ebx, ecx: random(eax))   # Random inside the original ops
    cur = sc["cur_sector"]
    if bufs["sectors"][177 * cur + 0xA9]:
        raise SystemExit("entry: the sector has a tmp file; this test serves none")
    ter = cd_read("DAT/TERRAIN.DAT")[cur * 0x3D41:(cur + 1) * 0x3D41]
    t = bufs["tiles"]
    for i in range(3136):
        t[8 * i:8 * i + 8] = ter[5 * (3135 - i):5 * (3135 - i) + 5] + bytes([0x40, 0xFF, 0])
    entry = struct.unpack("<i", open(EXE, "rb").read()[ENTRY_TILES + 4 * cur:ENTRY_TILES + 4 * cur + 4])[0]
    cd, calls = {"data": b"", "pos": 0}, []

    def call(op, a):
        a = [x & 0xFFFFFFFF for x in a]
        calls.append(op)
        if op == "open_cd":
            path = "DAT/XRA.DAT" if a[0] == 4 else f"MISC/{PATCH_KINDS[a[0]]}{ai._i32(a[1]):02d}.BIN"
            try:
                cd["data"], cd["pos"] = game_read(path), 0  # OpenCDFile: game folder (1.13b) first
                return 0, None
            except subprocess.CalledProcessError:
                return 0xFFFFFFFF, None
        if op == "fread":
            d = cd["data"][cd["pos"]:cd["pos"] + a[0]]
            cd["pos"] += len(d)
            return len(d), d
        if op == "fseek":
            cd["pos"] = a[0]
            return 0, None
        if op == "open_tmp":
            return 0xFFFFFFFF, None
        if op == "sector_resets":          # no draw: the library draws speech_wait (sector_start.md Setup sector)
            return 0, None
        if op in ORIGINAL_OPS:
            port_lib.bufs_store(e, ai, bufs)
            args = ([] if op == "radio_result" else [rt(ai.G_SOLDIERS) + 0x200 * a[0]] +
                    (a[1:3] if op == "action_ap" else []))
            if op == "quest_check":
                args = a[:1]
            if op in ("tile_dist", "direction"):      # no record argument
                args = a
            if op in ("shot_ap", "reach_ap"):
                args = [rt(ai.G_SOLDIERS) + 0x200 * a[0]] + a[1:]
            if op == "path_ap":                       # PathApCost(rec, tile, 0, 0, stack 0, 0, 0)
                args = [rt(ai.G_SOLDIERS) + 0x200 * a[0], a[1], 0, 0, 0, 0, 0]     # ActionApCost: EAX record, EDX tile, EBX flag
            r = e.call(rt(ORIGINAL_OPS[op]), *args)
            port_lib.bufs_load(e, ai, bufs)
            return r, None
        if op in ("keep_selected", "sector_tail", "replay_event", "fclose", "w_67c54", "mouse_release"):   # 198, 315: Time compression (0)
            return 0, None
        raise SystemExit(f"entry: host op {op} {a} not served by this test")
    sc["idle_day"] = 0x55                    # step 3 writes it (original byte 0x22A10A)
    if PORT:
        port_lib.sector_run("enter_sector", bufs, sc, (entry,), random, call, port_lib.SECTOR_LIB_OPS)
        print(f"entry --port: host calls replace {calls.count('replace')}, reserve_ap {calls.count('reserve_ap')}, "
              f"radio_result {calls.count('radio_result')}, action_ap {calls.count('action_ap')}, "
              f"quest_check {calls.count('quest_check')}, shot_ap {calls.count('shot_ap')}")
    else:                       # world_load.md section 4, steps 1-3 and 5 around the library's setup_sector
        sol, state = bufs["soldiers"], bufs["sector_state"][9 * cur:9 * cur + 9]
        for k in range(8):
            sol[0x200 * k + 0x138] = 1 if sol[0x200 * k + 0x17E] == 0 else 0
        sc["selected"] = next((k for k in range(8) if sol[0x200 * k + 0x182] and sol[0x200 * k + 0x17E] in (0, 8)
                               and sol[0x200 * k + 0x183]), 8)
        sc["kept_merc"] = 0x40               # step 0
        sc["last_killer"] = 0x40
        sc["ambient_t1"], sc["ambient_t2"] = 200, 300
        sc["turn_tick"] = 10                 # (LoadTacticalSector? 0x2AE2F; soldier_update.md Loop head)
        sc["idle_day"] = int(sc["selected"] >= 8)
        if sc["selected"] < 8 and state[2]:
            call("replay_event", [cur, cur, 0x58])
        port_lib.sector_run("setup_sector", bufs, sc, (entry, 1), random, call)
        if bufs["sector_state"][9 * cur + 2]:
            r = 0x200 * sc["selected"]
            tt = ai.s16(int.from_bytes(sol[r + 0x114:r + 0x116], "little"))
            px, py = ai._i32(tt % 56 if tt >= 0 else -(-tt % 56)) * 16, ai.cdiv(tt, 56) * 14
            vx, vy, vw, vh = sc["view_x"], sc["view_y"], sc["view_w"], sc["view_h"]
            if sc["opt_onscreen"] and (px < vx or px >= vx + vw or py < vy or py >= vy + vh):
                sc["view_x"] = px - (0x70 if sc["opt_onscreen"] < 2 else 0xA0) + 16
                vy = py - ai.cdiv(vh, 2) + 14
                while vy - ai.cdiv(vy, 14) * 14:
                    vy -= 1
                sc["view_y"] = vy
        else:
            sc["view_x"], sc["view_y"] = 336, 308
    sc["opt_reload_msg"] = 1               # step 3: the front-end's first draw
    for k in BUFS:
        bad = diffs(bufs[k], want[k], len(bufs[k]))
        if k == "soldiers":
            bad = [o for o in bad if o % 0x200 not in (0x11A, 0x11B, 0x11C, 0x11D)]
        if k == "enemy_watch":
            bad = [o for o in bad if o >= 40]
        if k in ("text_buf",):
            bad = []
        if k in NEIGHBOURS:
            bad = [o for o in bad if not NEIGHBOURS[k](o)]
        report(f"entry {k}", [hex(o) for o in bad])
    for k in SCALARS:
        if sc[k] != wsc[k]:
            report(f"entry {k}", [(sc[k], wsc[k])])
    if sc["idle_day"] != post_e.r8(post_e.rt(0x22A10A)):
        report("entry idle_day", [(sc["idle_day"], post_e.r8(post_e.rt(0x22A10A)))])
    real = post_e.r32(post_e.rt(SEED))
    if seed[0] != real:
        report("entry random", [f"seed {seed[0]:#x} after {draws[0]} draws, real {real:#x}"])
    placed = sum(1 for k in range(48) if int.from_bytes(want["soldiers"][0x200 * k + 0x114:0x200 * k + 0x116], "little") < 0xC40)
    print(f"entry{' --port' if PORT else ''}: sector {cur + 1}, entry tile {entry}, {draws[0]} random draws, "
          f"{len(calls)} host calls, {placed} soldiers on the map")


def part_enter():
    """--port: the front-end as host (world_load.md section 7). ja1front --enter from the day save the original wrote
    when the day started in sector 60 (its world = wl_pre, checked below) must give wl_post and the real end seed."""
    if not os.path.exists(DAY_SAVE):
        report("enter", [f"missing {DAY_SAVE}"])
        return
    bufs, sc = ref_world(open(DAY_SAVE, "rb").read())
    _, (pre, psc) = dump_world("pre")
    bad = [f"{k}+{hex(o)}" for k in BUFS if k not in ("tiles", "text_buf")
           for o in diffs(bufs[k], pre[k], len(bufs[k]))
           if not (k == "soldiers" and 0x90 <= o % 0x200 < 0x94) and not (k == "sector_state" and o >= 540)
           and not NEIGHBOURS.get(k, lambda o: 0)(o)]
    bad += [k for k in SCALARS if sc[k] != psc[k]]
    report("enter: day save world vs wl_pre", bad)
    post_e, (want, wsc) = dump_world("post")
    for k in range(48):         # a null script pointer is 0 in the port (section 3), not null minus the table base
        if post_e.r32(post_e.rt(ai.G_SOLDIERS) + 0x200 * k + 0x90) == 0:
            want["soldiers"][0x200 * k + 0x90:0x200 * k + 0x94] = bytes(4)
    e = Emu(dump=DUMP["pre"], fast=True)
    seed = e.r32(e.rt(SEED))
    out = os.path.join(ROOT, "build", "enter_port.bin")
    exe = os.path.join(ROOT, "port", "ja1front", "target", "release", "ja1-port")
    r = subprocess.run([exe, os.path.join(ROOT, "rom", "JA1.gog"), EXE, "--enter", DAY_SAVE, str(seed), out],
                       capture_output=True, text=True)
    if r.returncode:
        report("port ja1front --enter", [r.stderr.strip()[-300:]])
        return
    got = open(out, "rb").read()
    if len(got) != len(image(want, wsc)):
        report("port --enter image size", [(len(got), len(image(want, wsc)))])
        return
    bad, o = [], 0
    for k in BUFS:
        n = len(want[k])
        for i in diffs(got[o:o + n], want[k], n):
            if k == "soldiers" and i % 0x200 in (0x11A, 0x11B, 0x11C, 0x11D) or k == "enemy_watch" and i < 40 \
                    or k == "text_buf" or k == "sector_state" and i >= 540 or NEIGHBOURS.get(k, lambda o: 0)(i):
                continue
            bad.append(f"{k}+{hex(i)}")
        o += n
    for k in SCALARS:
        v = struct.unpack("<i", got[o:o + 4])[0]
        if v != ai._i32(wsc[k]):
            bad.append(f"{k} {v} want {wsc[k]}")
        o += 4
    report("port --enter world vs wl_post", bad)
    real = post_e.r32(post_e.rt(SEED))
    line = r.stdout.strip().splitlines()[-1] if r.stdout.strip() else ""
    if line != f"seed {real} draws 215":
        report("port --enter seed line", [f"{line!r}, want 'seed {real} draws 215'"])
    print(f"enter --port: ja1front --enter {os.path.basename(DAY_SAVE)}: {line}")


def part_leave():
    """leave (--port only): DAY_SAVE with the home sector (59) not the player's (sector_state byte 0 bit 0 clear, no tappers or guards, one enemy),
    `ja1front --script` with one pass: the leave tests end the sector (soldier_update.md Leave tests) and the
    front-end runs ja1_leave_path once (Leaving the sector; leave_reason 4 = no end of day)."""
    d = bytearray(open(DAY_SAVE, "rb").read())
    d[0x2534 + 9 * 59] &= 0xFE
    d[0x2534 + 9 * 59 + 3] = d[0x2534 + 9 * 59 + 4] = 0    # no tappers or guards, one enemy: else Sector won (no
    d[0x2534 + 9 * 59 + 1] = 1                              # enemies) or a battle makes the sector the player's again
    save, script, out = (os.path.join(ROOT, "build", f) for f in ("leave_port.SAV", "leave_port.txt", "leave_port.bin"))
    open(save, "wb").write(d)
    open(script, "w").write("wait 1\n")
    exe = os.path.join(ROOT, "port", "ja1front", "target", "release", "ja1-port")
    r = subprocess.run([exe, os.path.join(ROOT, "rom", "JA1.gog"), EXE, "--script", save, "1", script, out],
                       capture_output=True, text=True)
    if r.returncode:
        report("port ja1front --script leave", [r.stderr.strip()[-300:]])
        return
    ops = [int(l.split()[2]) for l in r.stderr.splitlines() if l.startswith("host op ")]
    want_ops = [125, 215, 85, 216, 220]      # message 6, fade, cursor_back, music_stop, timer_off
    got = open(out, "rb").read()
    o = len(got) - 4 * len(SCALARS)          # the scalars close the world image (section 6)
    sc = {k: struct.unpack("<i", got[o + 4 * i:o + 4 * i + 4])[0] for i, k in enumerate(SCALARS)}
    bad = []
    if ops[-5:] != want_ops:
        bad.append(f"host ops {ops} (want the tail {want_ops})")
    if (sc["cur_sector"], sc["leave_reason"]) != (0xFF, 4):
        bad.append(f"cur_sector {sc['cur_sector']} leave_reason {sc['leave_reason']} (want 255, 4)")
    if not r.stdout.strip().endswith("passes 1 left 1"):
        bad.append(f"stdout {r.stdout.strip().splitlines()[-1:]}")
    report("port --script leave path", bad)
    print(f"leave --port: home sector lost in pass 1, host ops {ops[-5:]}, cur_sector {sc['cur_sector']}")


def part_inventory():
    """inventory (--port only): key I + inv lines of a play script (world_load.md section 8, inventory.md)."""
    save = os.path.join(ROOT, "build", "DAY_fight48d.SAV")
    exe = os.path.join(ROOT, "port", "ja1front", "target", "release", "ja1-port")
    hand, ground = "inv 64 112 1\ninv 64 112 0\n", "inv 120 155 1\ninv 120 155 0\n"
    scripts = {"open": "", "back": hand + hand, "drop": hand + ground, "again": hand + ground + ground + hand}
    img = {}
    for k, body in scripts.items():
        script, out = (os.path.join(ROOT, "build", f"inv_port_{k}.{e}") for e in ("txt", "bin"))
        open(script, "w").write("key I\n" + body + "inv 0 0 0 Esc\n")
        r = subprocess.run([exe, os.path.join(ROOT, "rom", "JA1.gog"), EXE, "--script", save, "1", script, out],
                           capture_output=True, text=True)
        if r.returncode:
            report(f"port ja1front --script inventory {k}", [r.stderr.strip()[-300:]])
            return
        img[k] = open(out, "rb").read()
    a = img["open"]
    bad = []
    diff = lambda b: [i for i in range(len(a)) if a[i] != b[i]]
    if [i for i in diff(img["back"]) if not 0xB4 <= i < 0xB8]:
        bad.append(f"take + put back: bytes {diff(img['back'])[:8]} differ (only soldier +0xB4..+0xB7 may)")
    item = a[0:12]
    if item[0] == 0:
        bad.append("the hand of soldier 0 is empty in the save")
    d = img["drop"]
    if d[0:12] != bytes(12):
        bad.append(f"drop: hand {d[0:12].hex()} (want empty)")
    if d.find(item[0:3], 512 * 48) < 0:
        bad.append(f"drop: item {item[0:3].hex()} not in the ground list")
    if img["again"][0:6] != a[0:6]:
        bad.append(f"pick up + put back: hand {img['again'][0:6].hex()} (want {a[0:6].hex()})")
    report("port --script inventory", bad)
    print(f"inventory --port: take/put back, drop, pick up on DAY_fight48d (hand item {item[0:3].hex()})")


def part_eod():
    """eod (--port only): DAY_SAVE with story byte 0x1C = 1 and 0x0E = 0, so the leave tests end the day; script
    `eod 40` + `wait 1`: the library's End of day (op 217) opens the end-of-day screen (world_load.md section 8),
    poll 40 answers 1 (frames 0..40 shown, the text at frame 30), and the loop polls all 119 times."""
    d = bytearray(open(DAY_SAVE, "rb").read())
    d[0x516C + 0x1C], d[0x516C + 0x0E] = 1, 0
    save, script, out, png = (os.path.join(ROOT, "build", f) for f in ("eod_port.SAV", "eod_port.txt", "eod_port.bin",
                                                                       "eod_port.png"))
    open(save, "wb").write(d)
    open(script, "w").write("eod 40\nwait 1\n")
    if os.path.exists(png):
        os.remove(png)
    exe = os.path.join(ROOT, "port", "ja1front", "target", "release", "ja1-port")
    r = subprocess.run([exe, os.path.join(ROOT, "rom", "JA1.gog"), EXE, "--script", save, "1", script, out],
                       capture_output=True, text=True, env=dict(os.environ, JA1_EOD_PNG=png))
    if r.returncode:
        report("port ja1front --script eod", [r.stderr.strip()[-300:]])
        return
    ops = [int(l.split()[2]) for l in r.stderr.splitlines() if l.startswith("host op ")]
    line = [l for l in r.stderr.splitlines() if l.startswith("eod ")]
    bad = []
    if line != ["eod frames 41 polls 119 texts 1"]:
        bad.append(f"stderr eod lines {line} (want ['eod frames 41 polls 119 texts 1'])")
    if 254 not in ops or 256 not in ops:
        bad.append(f"host ops {sorted(set(ops))} (want 254 eod_begin and 256 eod_text)")
    if not os.path.exists(png) or os.path.getsize(png) < 1000:
        bad.append("no JA1_EOD_PNG picture")
    report("port --script end of day", bad)
    print(f"eod --port: {line[0] if line else '-'}, picture {os.path.getsize(png) if os.path.exists(png) else 0} B")


def part_dayloop():
    """dayloop (--port only, after eod): build/eod_port.SAV with two day loops in one script (world_load.md section
    9): the day ends, the morning, sunrise and strategic screen opening run, the team goes back to sector 59 (Y),
    the day ends again and the third entry of sector 59 reads the tmp file that op 219 save_tmp made at the leave
    (section 8). Both `start` lines must be `start 0 1`, the second `day` line day 3."""
    save = os.path.join(ROOT, "build", "eod_port.SAV")
    script, out = (os.path.join(ROOT, "build", f) for f in ("dayloop_port.txt", "dayloop_port.bin"))
    open(script, "w").write("eod 0\nwait 1\nday\nmap 59 0 1\nstart 1\neod 0\nwait 5\nday\nmap 59 0 1\nstart 1\nwait 3\n")
    exe = os.path.join(ROOT, "port", "ja1front", "target", "release", "ja1-port")
    r = subprocess.run([exe, os.path.join(ROOT, "rom", "JA1.gog"), EXE, "--script", save, "1", script, out],
                       capture_output=True, text=True, timeout=600)
    if r.returncode:
        report("port ja1front --script dayloop", [r.stderr.strip()[-300:]])
        return
    lines = [l for l in r.stdout.splitlines() if l.split()[:1] in (["day"], ["start"])]
    days = [l.split()[-1] for l in lines if l.startswith("day morning ")]
    starts = [l for l in lines if l.startswith("start ")]
    bad = []
    if days != ["2", "3"] or starts != ["start 0 1", "start 0 1"]:
        bad.append(f"day lines {lines} (want days 2, 3 and two 'start 0 1')")
    report("port --script two day loops", bad)
    print(f"dayloop --port: {'; '.join(lines)}")


DAY_SAVE = os.path.join(ROOT, "build", "DAY_before_restart.SAV")     # the original's day save at the start of day 1
for k, p in DUMP.items():
    if not os.path.exists(p):
        raise SystemExit(f"missing {p}: run validate/world_ref.py")
part_load()
part_entry()
if PORT:
    part_enter()
    part_leave()
    part_inventory()
    part_eod()
    part_dayloop()
print(f"world_diff: {fails} mismatching parts")
sys.exit(1 if fails else 0)
