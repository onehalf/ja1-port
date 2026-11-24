"""Whole-AI integration test: the original AiThink 0x6D1A8 runs unstubbed in Unicorn on real game dumps (real
Random, real searches, real pathfinder) against tools/ai.py think() with every ported search wired in.

ai_diff.py tests each procedure alone with its callees scripted; this test composes them. Only leaves with effects
outside the AI (animation, sound, walking, speech, UI) are stubbed on both sides, and their calls are compared.
Callees without a Python port in ai.py (pathfinder, chance to hit, AP costs, DirectionTo, ...) run as the original
code in the same emulator from the port side too ("oracle" calls). The port's soldier records are the emulator's
memory itself (MemRec), so oracle calls see the port's writes. After each tick the whole data object
(0x90000..0x33A730) must match byte for byte, plus the leaf call log.

  .venv/bin/python validate/diff/compose_diff.py [N ticks] [dump ...]   (default: every build/mem_*.bin, 300 ticks)
--port: the AiThink side is the clean-room port (port/ja1logic) with the composed mirror as its host.

--flight: N whole shots per dump instead of AiThink ticks. A soldier in play gets a gun (or a fuzzed main-hand item)
aimed at an opponent's body (end point, height, target tile), LaunchProjectile 0x58DE4 runs, then ProjectileStep
0x57BB8 frame by frame (frame timer +0xCC cleared, as the tactical loop counts it down) until the projectile is
gone (+0x1BF != 3, at most 400 frames): launch, flight, hit test, impact, BulletDamage, TakeDamage, the hit-location
rule and the hit reaction run as port code on one side and as the original on the other. Compared as above, once
per shot.

--melee: N melee blows per dump: MeleeHit? 0x3521C for a soldier whose attack tile is an opponent's (knife 0x0F / 0x10
or another item, condition, aim varied), with MeleeChance (oracle), TakeDamage and the hit reaction composed.

--blast: N blasts per dump: Blast? 0x38B60 at a tile near soldiers (radius 0..5, explosion or gas kinds, the
blaster varied), with BlastTile, TakeDamage, fatigue, item wear and the hit reaction composed; every fourth case
runs the gas cloud tick GasTick? 0x38520 over five fuzzed clouds instead.

--newturn: N side turn changes per dump: NewTurn? 0x126C8 (combat flag, interrupt state, the merc to show, gas
clouds, story counters varied), with AiThink for the guards, GasTick, StartTurn for every soldier and the rest
composed; its UI calls are leaves.

--windows: window wall objects (0x7F..0x82) on tiles between the AI soldier and another soldier in EVERY tick (else
1 tick in 4), so the line of fire meets the window rule of ProjectileHitTest (no dump has an enemy line across one).

--turn: N turn starts per dump: StartTurn? 0x11FB4 for a soldier in play (bleeding, drowning, sightings, AP;
TakeDamage and the hit chain composed), with wounds / bleeding timer / energy / water animation / combat flag varied.
"""
import glob, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "tools"))
import ai                            # noqa: E402
import combat                        # noqa: E402
import emu                           # noqa: E402
from emu import Emu                  # noqa: E402
import struct                        # noqa: E402
import port_lib                     # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

THINK, SEED = 0x6D1A8, 0x9BBBC        # AiThink; Watcom rand_ seed (dword)
G_TILEMAP = 0x100A18
CALLS = {}                             # port-side callee -> calls (COMPOSE_CALLS=1 prints it: what the ticks reached)
LEAF_RET = {}                          # leaf -> return values for this side's run (copied fresh for each side), else 0


def leaf_ret(name):
    q = LEAF_RET.get(name)
    return q.pop(0) if q else 0
STEPS = 2_000_000_000                 # Unicorn instruction cap per call (SearchTile runs thousands of line tests)
DATA = (0x90000, 0x33A730)            # obj2 (initialised data + bss)
FINDPATH_SCRATCH = ((0x9606C, 2), (0x284F50, 0x9102))   # trail count; trails 0x284F50, nodes 0x288F50, best costs 0x28AF50, node count 0x28E050
LEAVES = {
          "mine_flag": (0x19D74, "st"), "enemy_turned": (0x73578, "s"), "guard_turned": (0x7351C, "s"),
          "w_17b70": (0x17B70, "bi"), "w_3f490": (0x3F490, "ii"), "quote": (0x3C574, "st"),
          "use_special": (0x63458, "s"), "door_fx": (0x3B03C, "stb"), "popups": (0x288DC, "si"),
          "w_3cdd4": (0x3CDD4, "s"), "yes_no": (0x28C84, "mbxii"), "w_53708": (0x53708, "iii"), "w_1cb78": (0x1CB78, "b"), "w_3cfd0": (0x3CFD0, "stb"), "bury_mine": (0x3CB24, "st"),

          "w_5ad50": (0x5AD50, "st"), 
          "walk_sound": (0x335F8, "bi"), "sfx": (0x5CB34, "iiii"),
          "mouse_hide": (0x67368, ""), "tac_redraw": (0x176C8, "i"), "open_dat": (0x5E8E0, "m"),
          "load_pcc": (0x532BC, "ixmxi"), "fx_load": (0x5C0EC, "mim"), "merc_voice": (0x24D14, "bimii"),
          "fx_play": (0x5C144, ""), "blit": (0x52AE0, "ixiiii"), "w_320a0": (0x320A0, "bi"), "sector_dat": (0x2F7C8, ""),
          "mouse_show": (0x67398, ""), "sector_won": (0x3A0B0, ""), "quote_event": (0x1B2A4, "bbbii"),
          "w_46a4c": (0x46A4C, "bi"), "squad_lost": (0x3A32C, ""),
          "show_msg": (0x27B98, "miiiiiii"), "w_355d0": (0x355D0, "iiiii"),
         
          "explode_art": (0x33BD4, "ii"),
          "merc_text": (0x15910, "smi"), "w_1c438": (0x1C438, "si"), "reloaded": (0x40094, "s"),
          "item_panel": (0x55FD8, "sii"), "w_32828": (0x32828, ""), "turn_banner": (0x1054C, ""),
          
          "fx_157bc": (0x157BC, "tbbiii"),
          "w_53660": (0x53660, ""), "w_32eb8": (0x32EB8, "bi"),
          "w_67e30": (0x67E30, ""), "w_6a960": (0x6A960, ""), "merc_quote": (0x1B324, "s"), "guards_arrive": (0x1F724, "i"),
          "w_10398": (0x10398, "i"), "w_1583c": (0x1583C, ""), "w_2ac60": (0x2AC60, ""),
          "mouse_in_rect": (0x67C0C, "iiii"), "w_673ac": (0x673AC, ""), "w_124fc": (0x124FC, ""), "w_1148c": (0x1148C, "iii"),
          "w_1259c": (0x1259C, ""), "w_410c4": (0x410C4, ""), "w_297d4": (0x297D4, "si"),
          "w_31550": (0x31550, ""), "w_331a4": (0x331A4, "bi"), "w_1f6e4": (0x1F6E4, ""),
          "energy_bar": (0x32610, "sb"), "w_33204": (0x33204, "b"),
          "w_323d4": (0x323D4, "si"), "anim2": (0x17F5C, "sbbb"),
          "cry": (0x24E44, "bimii"), "voc": (0x5C9C0, "iii"), "replay_event": (0x3FA04, "iii"),
          "puff": (0x156B8, "tiiiiii"),
          "confirm_click": (0x1C514, "t"), "w_1c464": (0x1C464, "s"), "item_cond": (0x55BE4, "isii"),
          "w_328a4": (0x328A4, "s"), "wait": (0x1AB60, "i"), "delay": (0x67250, "i"),
          "santino_speech": (0x5F4C4, ""),
          "clear_rooms": (0x61744, ""), "focus": (0x3F724, "bi"), "message": (0x27D24, "x"),
          "merc_message": (0x299B8, "sbbxi"),
          "portrait": (0x31C98, "bi"), "step_sound": (0x336B4, "bi"), "ui_text": (0x254F0, "ixii"),
          "ui_10310": (0x10310, "x"), "w_54ec8": (0x54EC8, "s"), "w_51a24": (0x51A24, "iiii"), "close_door": (0x3ADB4, "tbi")}
LEAF_POP = {"yes_no": 4, "w_355d0": 4, "load_pcc": 4, "merc_voice": 4, "blit": 8, "quote_event": 4, "merc_message": 4, "fx_157bc": 8,
            "puff": 12, "cry": 4, "show_msg": 16}        # stack argument bytes the callee pops
# no Python port in ai.py: the port side calls the original (address, register arguments, fixed stack arguments)
ORACLE = {"find_path": (0x40810, 3, ()), "path_ap": (0x1D32C, 2, (0, 0, 0, 0, 0)), "chance_to_hit": (0x56630, 3, ()),
          "shot_ap": (0x564F8, 2, ()), "action_ap": (0x56310, 3, ()), "direction": (0x52560, 4, ()),
          "busy": (0x5AD1C, 1, ()), "swap_hand": (0x55B0C, 3, ()), "mission_cancel": (0x72670, 0, ()),
          "quest_check": (0x633D8, 1, ()), "radio_result": (0x63254, 0, ())}   # radio_result: a library op (batch 10)
SOLDIER_SECOND = set()                 # EDX is a soldier pointer too
SOLDIER_FIRST = {"find_path", "path_ap", "chance_to_hit", "shot_ap", "action_ap", "busy", "swap_hand"}
# path_ap: AI callers pass BL = CL = 0 and three zero stack words (PathApCost pops 12)


class MemRec:
    """a 0x200-byte soldier record read and written straight in emulator memory"""
    def __init__(self, e, slot):
        self.e, self.slot, self.base = e, slot, e.rt(ai.G_SOLDIERS) + 0x200 * slot

    def __len__(self):
        return 0x200

    def __getitem__(self, i):
        if isinstance(i, slice):
            a, b, _ = i.indices(0x200)
            return bytes(self.e.uc.mem_read(self.base + a, max(b - a, 0)))
        return self.e.r8(self.base + i)

    def __setitem__(self, i, v):
        if isinstance(i, slice):
            a, _, _ = i.indices(0x200)
            self.e.uc.mem_write(self.base + a, bytes(v))
        else:
            self.e.w8(self.base + i, v & 0xFF)


def cstr(e, a):
    """the C string at emulator address a"""
    b = bytes(e.uc.mem_read(a, 256))
    return b[:b.index(0)].decode("latin-1")


def norm(kinds, args, e):
    """leaf call arguments as logged: soldiers by slot, tiles signed 16, bytes, message text, raw ints; 'x' not logged"""
    out = []
    for k, a in zip(kinds, args):
        if k == "x":
            continue
        if k == "m":                      # message text: the port passes the string, the original a pointer
            out.append(a if isinstance(a, str) else cstr(e, a))
            continue
        if k == "s":
            if isinstance(a, MemRec):
                a = a.slot
            elif a >= 0x10000:
                a = (a - e.rt(ai.G_SOLDIERS)) // 0x200
        out.append(ai.s16(a) if k == "t" else a & 0xFF if k == "b" else a)
    return tuple(out)


class World:
    """the port's view of the machine: globals and records in emulator memory, the real Random, callees wired"""
    def __init__(self, e, log):
        self.e, self.log, self.slot = e, log, 0

    def g8(self, a): return self.e.r8(self.e.rt(a))
    def g16(self, a): return self.e.r16(self.e.rt(a))
    def g32(self, a): return self.e.r32(self.e.rt(a))
    def s8(self, a, v): self.e.w8(self.e.rt(a), v & 0xFF)
    def s16(self, a, v): self.e.w16(self.e.rt(a), v & 0xFFFF)
    def s32(self, a, v): self.e.w32(self.e.rt(a), v & 0xFFFFFFFF)

    def tile8(self, tile, k):
        return self.e.r8(self.e.rt(G_TILEMAP) + 8 * ai.s16(tile) + k)

    def ptr(self, a):
        return self.e.rt(a)

    def raw16(self, p):
        return self.e.r16(p & 0xFFFFFFFF)

    def rec(self, k):
        return MemRec(self.e, k)

    def img(self, a):
        """an item entry argument as an image address: a record, a slot (its entry 0) or a runtime pointer"""
        if isinstance(a, MemRec):
            return ai.G_SOLDIERS + 0x200 * a.slot
        if a < 0x40:
            return ai.G_SOLDIERS + 0x200 * a
        return a - (self.e.rt(ai.G_SOLDIERS) - ai.G_SOLDIERS)

    def random(self, n):
        """Random 0x53538 over Watcom rand_: seed = seed * 0x41C64E6D + 0x3039, r = (seed >> 16) & 0x7FFF;
        (r * n / 0x7FFF) % n in C arithmetic (n = 0 divides by zero like the original)"""
        seed = (self.g32(SEED) * 0x41C64E6D + 0x3039) & 0xFFFFFFFF
        self.s32(SEED, seed)
        r, n = (seed >> 16) & 0x7FFF, ai.s16(n)
        v = ai.cdiv(r * n, 0x7FFF)
        return v - ai.cdiv(v, n) * n

    def rec_of(self, s):
        return s if isinstance(s, MemRec) else MemRec(self.e, s & 0xFF)

    def ext(self, name, *args):
        CALLS[name] = CALLS.get(name, 0) + 1
        if name in LEAVES:
            self.log.append((name, norm(LEAVES[name][1], args, self.e)))
            return leaf_ret(name)
        if name in ORACLE:
            addr, nreg, extra = ORACLE[name]
            regs = [a.base if isinstance(a, MemRec) else
                    self.e.rt(ai.G_SOLDIERS) + 0x200 * a if (i == 0 and name in SOLDIER_FIRST or i == 1 and
                                                             name in SOLDIER_SECOND) and a < 0x40 else
                    a for i, a in enumerate(args)]
            return self.e.call(self.e.rt(addr), *[r & 0xFFFFFFFF for r in regs[:nreg]] + [0] * (4 - nreg)
                               + list(extra), max_steps=STEPS) if extra else \
                self.e.call(self.e.rt(addr), *[r & 0xFFFFFFFF for r in regs[:nreg]], max_steps=STEPS)
        if name == "hit_test":                # pointer outputs: skip byte and chance dword on the scratch heap
            h, x, y, flown, d, skip, cap, tile, roll = args[:9]
            p = emu.HEAP
            self.e.w8(p, skip)
            self.e.w32(p + 4, args[9] if len(args) > 9 else 100)   # LineOfFire starts the chance at 100
            r = self.e.call(self.e.rt(0x577B4), h & 0xFFFFFFFF, x & 0xFFFFFFFF, y & 0xFFFFFFFF, flown & 0xFFFFFFFF,
                            d, p, p + 4, cap & 0xFFFFFFFF, tile & 0xFFFFFFFF, roll, max_steps=STEPS)
            return r, self.e.r8(p), self.e.r32(p + 4)
        r = self.rec_of
        read = lambda a, k: bytes(self.e.uc.mem_read(self.e.rt(a), k))

        def on_copy(s, f):                    # tools/combat.py ports take a bytearray record: copy in, run, write back
            rec, mem = r(s), {}
            b = bytearray(rec[0:0x200])
            v = f(b, mem)
            rec[0:0x200] = b
            for a in mem:
                self.s8(a, mem[a])
            return v
        port = {"hit_location": lambda s, h, d: on_copy(s, lambda b, m: combat.hit_location_damage(
                    b, ai._i32(h), ai._i32(d), self.random, read, m)),
                "bullet_damage": lambda s: on_copy(s, lambda b, m: combat.bullet_damage(b, self.random, read)),
                "idle": lambda s: ai.idle(r(s), self), "noise_tile": lambda s: ai.noise_tile(r(s), self),
                "find_dry_tile": lambda s: ai.find_dry_tile(r(s), self),
                "patrol_tile": lambda s: ai.patrol_tile(r(s), self), "buddy_tile": lambda s: ai.buddy_tile(r(s), self),
                "news_value": lambda s: ai.news_value(r(s), self),
                "ally_call_tile": lambda s: ai.ally_call_tile(r(s), self),
                "retreat_tile": lambda s: ai.retreat_tile(r(s), self),
                "target_tile": lambda s: ai.target_tile(r(s), self),
                "opp_strength": lambda s: ai.opp_strength(r(s), self),
                "move_ap": lambda s: ai.move_ap_cost(r(s), self),
                "gun_option": lambda s: ai.gun_option(r(s), self),
                "throw_option": lambda s: ai.throw_option(r(s), self),
                "knife_option": lambda s: ai.knife_option(r(s), self),
                "tile_dist": lambda a, b: ai.tile_distance(a, b),
                "sector_value": lambda sec, f: ai.sector_value(self, sec, f),
                "approach_tile": lambda s, t: ai.approach_tile(r(s), self, t),
                "search_tile": lambda s, m: ai.search_tile(r(s), self, m),
                "advance_tile": lambda s, m: ai.advance_tile(r(s), self, m),
                "can_stand_at": lambda s, t: ai.can_stand_at(r(s), self, t),
                "reach_ap": lambda s, t, f: ai.reach_tile(r(s), self, t, f),
                "attack_ready": lambda s, t: ai.attack_ready(r(s), self, t),
                "share_sightings": lambda s, f: ai.share_sightings(r(s), self, f),
                "perceive": lambda s, a: ai.update_sightings(r(s), self, a),
                "sight_check": lambda s, i: ai.sight_check(r(s), self, i),
                "fire_line": lambda s, t, th: ai.line_of_fire(r(s), self, ai.s16(t), th),
                "sight_line": lambda s, *a: ai.sight_line(r(s), self, *a),
                "turn": lambda s, d, t: ai.turn_to(r(s), self, d, t),
                "walk_to": lambda s, t: ai.walk_to(r(s), self, t),
                "anim_step": lambda s: ai.anim_step(r(s), self),
                "anim_end": lambda s, f: ai.anim_end(r(s), self),
                "set_anim": lambda s, c, k, z: ai.set_anim(r(s), self, c, k, z),
                "anim_frame": lambda s: ai.anim_frame(r(s), self),
                "anim_turn": lambda s, f: ai.anim_turn(r(s), self, f),
                "end_task": lambda s, f: ai.end_task(r(s), self, f),
                "replace": lambda s: ai.replace_soldier(r(s), self),
                "stop_anim": lambda s, f: ai.stop_anim(r(s), self, f),
                "stop_walk": lambda s, f: ai.stop_walk(r(s), self, f),
                "w_197d8": lambda s, f: ai.snap_to_tile(r(s), self, f),
                "click": lambda s, t, i: ai.tactical_click(r(s), self, t, i),
                "w_59a54": lambda s: ai.jam_check(r(s), self),
                "w_5aa08": lambda s, t: ai.aim_posture(r(s), self, t),
                "w_59ae8": lambda s, sp, t, g, k: ai.use_cost(r(s), self, sp, t, g, k),
                "launch": lambda s: ai.launch_projectile(r(s), self),
                "fire": lambda slot: ai.fire_now(r(slot), self),
                "w_56dd4": lambda s, a, b: ai.shot_end(r(s), self, a, b),
                "impact": lambda s, t, x, y, h: ai.projectile_impact(r(s), self, t, ai._i32(x), ai._i32(y), h),
                "take_damage": lambda s, h, d, k, a, t: ai.take_damage(r(s), self, h, d, k, a, t),
                "w_74304": lambda s, a, b, d: ai.hit_reaction(r(s), self, a, b, d),
                "react": lambda s, a, b: ai.hit_alert(r(s), self, a, b),
                "w_73ab4": lambda s, a, old: ai.interrupt_check(r(s), self, a, old),
                "w_73d28": lambda s, a: ai.merc_interrupt(r(s), self, a),
                "w_73f3c": lambda s, a: ai.enemy_interrupt(r(s), self, a),
                "make_noise": lambda src, t, c, loud, k: ai.make_noise(self, src, t, c, loud, k),
                "hears": lambda s, t, v, k: ai.hears(r(s), self, t, v, k),
                "rest": lambda s, a, b: ai.rest(r(s), self, a, b),
                "ui_refresh": lambda f: ai.hide_path_marks(self, f),
                "explode": lambda slot, t, k, hit: ai.explode(self, slot, t, k, hit),
                "land_item": lambda s, t, it, c, x, y: ai.land_item(r(s), self, t, it, c, x, y),
                "w_401b4": lambda s, f=0: ai.revive_roll(r(s), self),
                "w_3521c": lambda s: ai.melee_hit(r(s), self),
                "w_3a8c0": lambda s, q: ai.kill(r(s), self, q),
                "next_turn": lambda m: ai.next_turn(self, m),
                "w_1304c": lambda: ai.interrupt_turn(self),
                "spend_ap": lambda s, ap, t: ai.spend_ap(r(s), self, ap, t),
                "stat_practice": lambda s, k, n, f: ai.stat_practice(r(s), self, k, n, f),
                "w_32d04": lambda s: ai.turn_rest(r(s), self),
                "aid_tick": lambda s: ai.first_aid_tick(r(s), self),
                "merc_dies": lambda s, q: ai.merc_dies(r(s), self, q),
                "w_73c00": lambda s: ai.noise_interrupt(r(s), self),
                "w_746d4": lambda s, f: ai.shot_alert(r(s), self, f),
                "w_1ac68": lambda s, t, gi, armed: ai.trigger_item(r(s), self, t, gi, armed),
                "workable": lambda tbl, sec: ai.sector_workable(self, tbl, sec),
                "connected_home": lambda tbl, sec: ai.connected_home(self, tbl, sec) if sec & 0xFF < 60 else
                self.e.call(self.e.rt(0x41178), self.e.rt(tbl), sec, max_steps=STEPS),   # off the map: stack bytes
                "w_72568": lambda t, _d: ai.call_enemies(self, t),
                "w_3fc04": lambda typ: ai.listed_item(self, typ),
                "w_3c05c": lambda t, keep: ai.take_ground_item(self, t, keep),
                "w_19eb0": lambda t, typ: ai.take_ground_items_flood(self, t, typ),
                "w_3c16c": lambda s, t, c: ai.pick_up(r(s), self, t, c),
                "door_lookup": lambda t: ai.door_lookup(self, t),
                "break_obj": lambda t, o, snd, _c: ai.break_object(self, t, o, snd),
                "close_door": lambda t, o, snd: ai.close_door(self, t, o, snd),
                "w_35774": lambda m, d: ai.toggle_doors(self, m, d),
                "reserve_ap": lambda s: ai.reserve_ap(r(s), self),
                "w_58574": lambda s: ai.gun_jam(r(s), self),
                "w_635dc": lambda: ai.stage_effects(self),
                "w_741f8": lambda side: ai.age_knowledge(self, side),
                "bury_mine": lambda s, t: ai.bury_mine(r(s), self, t),
                "w_3cdd4": lambda s: ai.pickup_here(r(s), self),
                "w_3cfd0": lambda s, t, b: ai.use_on_tile(r(s), self, t, b),
                "story_hit": lambda: ai.story_hit(self),
                "w_5abe0": lambda s, t: ai.start_attack(r(s), self, t),
                "put_item": lambda t, _a, _b, _c, d0, d1, d2, b6, ex, no, ne: ai.put_item(self, t, d0, d1, d2, b6, ex, no, ne),
                "first_aid": lambda s, t, k, d: ai.first_aid_start(r(s), self, t, k),
                "attack_cursor": lambda: self.e.w32(self.e.rt(0x229DD4), 40000) or 0,   # op 192 (click.md): the cursor timer
                "merc_sees": lambda a, b, c: ai.merc_sees(self, a, b, c),
                "use_throw": lambda a: ai.use_throw(self, self.img(a)),
                "drop_item": lambda a: ai.drop_item(self, self.img(a)),
                "w_3a2d4": lambda: ai.mercs_alert(self)}
        if name not in port:
            raise KeyError(f"compose_diff: no wiring for callee {name}")
        return port[name](*args)


LAUNCH, PSTEP, MELEE = 0x58DE4, 0x57BB8, 0x3521C


def shot(e, a, step):
    """launch, then step the projectile until it is gone; step(name) runs one routine on the soldier at a"""
    step("launch")
    for _ in range(400):
        if e.r8(a + 0x1BF) != 3:
            break
        e.w32(a + 0xCC, 0)
        step("pstep")


def flight(e, rng, n, path, log, w, melee=False):
    """--flight / --melee: n composed shots or blows on one dump; returns (count, mismatches)"""
    rt = e.rt
    rng = random.Random(("melee " if melee else "") + os.path.basename(path))   # per dump: reproducible alone
    live = [k for k in range(0x30) if e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x182)
            and e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x138) and ai.s8(e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x183)) > 0]
    pairs = [(k, j) for k in live for j in live if (k < 0x28) != (j < 0x28)]
    if not pairs:
        print(f"{os.path.basename(path)}: no opposing soldiers in play, skipped")
        return 0, 0
    lo, hi = rt(DATA[0]), rt(DATA[1] - 1) + 1
    fails = 0
    for i in range(n):
        k, j = rng.choice(pairs)
        a, b = rt(ai.G_SOLDIERS) + 0x200 * k, rt(ai.G_SOLDIERS) + 0x200 * j
        e.w32(rt(SEED), rng.randrange(1 << 32))
        throw = [t for t in range(136) if e.r8(rt(ai.G_ITEM_KIND) + 12 * t) == 0x95]
        if throw and rng.random() < 0.3:      # a thrown item; +0x1C9 explosive class (set by the click normally)
            e.w8(a, rng.choice(throw))
            e.w8(a + 0x1C9, rng.choice([0, 0, 1, 2, 2, 3, 3, 4, 6, 9, 10]))
        else:
            e.w8(a, rng.choice(list(range(1, 0xF)) * 4 + [0x16, 0x18, 0x1A, 0x1C, rng.randrange(136)]))
            e.w8(a + 0x1C9, 0)
        e.w8(a + 1, rng.choice([1, 2, 5]))
        e.w8(a + 2, rng.choice([50, 100]))
        e.w8(a + 0x1BC, rng.choice([0, 1, 1]))
        e.w8(b + 0x183, rng.choice([e.r8(b + 0x183), 15, 20, 60]))
        e.w16(a + 0xDE, e.r16(b + 0x116) + 2 + rng.choice([0, 0, 0, rng.randrange(-8, 9)]))
        e.w16(a + 0xE0, e.r16(b + 0x118) + 4 + rng.choice([0, 0, 0, rng.randrange(-8, 9)]))
        e.w16(a + 0xE2, rng.choice([1000, 2500, 4000, 4500, 5500, rng.randrange(7000)]))
        e.w16(a + 0xDC, e.r16(b + 0x114))
        e.w32(rt(ai.G_BUSY), 0)
        if melee:
            e.w8(a, rng.choice([0xF, 0x10, 0x10, 0xF, rng.randrange(1, 0xF)]))
            e.w8(a + 2, rng.choice([5, 11, 30, 60, 100]))
            e.w8(a + 0x1FD, rng.choice([0, 0, 1, 2, 4]))
        snap = bytes(e.uc.mem_read(lo, hi - lo))
        log.clear()
        got_err = want_err = None
        try:
            if melee:
                e.call(rt(MELEE), a, max_steps=STEPS)
            else:
                shot(e, a, lambda name: e.call(rt(LAUNCH if name == "launch" else PSTEP), a, max_steps=STEPS))
        except emu.EmuStop as x:
            got_err = str(x)
        got, got_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
        e.uc.mem_write(lo, snap)
        log.clear()
        rec = MemRec(e, k)
        try:
            if melee:
                ai.melee_hit(rec, w)
            else:
                shot(e, a, lambda name: (ai.launch_projectile if name == "launch" else ai.projectile_step)(rec, w))
        except ZeroDivisionError:
            want_err = "divide by zero"
        want, want_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
        if got != want or got_log != want_log or (got_err is None) != (want_err is None):
            fails += 1
            if fails <= 5:
                diff = [(DATA[0] + o, got[o], want[o]) for o in range(len(got)) if got[o] != want[o]][:12]
                print(f"MISMATCH {os.path.basename(path)} shot {i} {k:#x} -> {j:#x}: errors {got_err} / {want_err}\n"
                      f"  bytes (addr, original, port) {[(hex(x), hex(y), hex(z)) for x, y, z in diff]}\n"
                      f"  leaves original {got_log}\n  leaves port     {want_log}")
            e.uc.mem_write(lo, got)
    print(f"{os.path.basename(path)}: {n} {'blows' if melee else 'shots'} over {len(pairs)} pairs, {fails} mismatches")
    return n, fails


TURN, BLAST, GAS_TICK, NO_GAS, NEW_TURN = 0x11FB4, 0x38B60, 0x38520, 0xC41, 0x126C8


def new_turns(e, n, path, log, w):
    """--newturn: n composed side turn changes on one dump; returns (count, mismatches)"""
    rt = e.rt
    rng = random.Random("newturn " + os.path.basename(path))
    lo, hi = rt(DATA[0]), rt(DATA[1] - 1) + 1
    live = [k for k in range(0x30) if e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x182)] or [0]
    fails = 0
    for i in range(n):
        e.w32(rt(SEED), rng.randrange(1 << 32))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 1]))
        e.w8(rt(0x92DDD), rng.choice([0, 0, 1]))
        e.w8(rt(0x92DD0), rng.choice([0, 0, 0, 1]))
        e.w32(rt(0x92DA4), rng.choice([0, 0, 1]))
        e.w8(rt(0x92DC7), rng.choice(live + [0x30, 0x40]))
        e.w8(rt(ai.G_SELECTED), rng.choice(live + [8, 9, 0x40]))
        e.w8(rt(0x92DEC), rng.choice([0x40, 0x40, rng.choice(live)]))
        e.w8(rt(0x92DD5), rng.choice([0x40, 0x40, rng.choice(live)]))
        e.w8(rt(0x22A1F2), rng.choice([0, 3, 4, 5]))
        e.w8(rt(0x22A1F3), rng.choice([0, 1]))
        for j in range(5):
            e.w8(rt(ai.G_STORY_TIMERS) + 13 * j, rng.choice([0, 1, 5]))
            e.w16(rt(ai.G_GAS_CLOUDS) + 6 * j, rng.choice([NO_GAS, NO_GAS, rng.randrange(3136)]))
            e.w16(rt(ai.G_GAS_CLOUDS) + 6 * j + 2, rng.choice([1, 2]))
            e.w16(rt(ai.G_GAS_CLOUDS) + 6 * j + 4, rng.choice([1, 2]))
        for k in range(0x28, 0x30):
            e.w8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x198, rng.choice([0, 1, 0xFF]))
        for k in range(8, 16):                 # guards in play now and then: they think out of combat
            if rng.random() < 0.3:
                e.w8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x182, 1)
                e.w8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x138, 1)
        e.uc.mem_write(rt(ai.G_ITQ), bytes(rng.choice([0, 3, 0x40]) for _ in range(0x34)))
        if rng.random() < 0.3:                 # the merc to show is down
            e.w8(rt(ai.G_SOLDIERS) + 0x200 * (e.r8(rt(0x92DC7)) & 0x3F) + 0x183, 0)
        if rng.random() < 0.25:                # sector 0 with Santino's countdown running
            e.w8(rt(ai.G_CUR_SECTOR), 0)
            e.w8(rt(0x22A0C2), 1)
        focus = rng.choice([0, 1])
        rets = {"guards_arrive": [rng.choice([0, 0, 0, 1])], "mouse_in_rect": [rng.choice([0, 1])] * 2,
                "w_2ac60": [rng.choice(live + [7, 8, 8, 9, 0x40, 0x40]) for _ in range(2)], "w_410c4": [rng.choice([0, 1])],
                "w_1f6e4": [rng.choice([0, 0, 1])] * 40}
        snap = bytes(e.uc.mem_read(lo, hi - lo))
        log.clear()
        got_err = want_err = None
        LEAF_RET.clear()
        LEAF_RET.update({k_: list(v) for k_, v in rets.items()})
        try:
            got_r = e.call(rt(NEW_TURN), focus, max_steps=STEPS) & 0xFF
        except emu.EmuStop as x:
            got_err, got_r = str(x), None
        got, got_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
        e.uc.mem_write(lo, snap)
        log.clear()
        LEAF_RET.clear()
        LEAF_RET.update({k_: list(v) for k_, v in rets.items()})
        try:
            want_r = ai.new_turn(w, focus)
        except ZeroDivisionError:
            want_err, want_r = "divide by zero", None
        want, want_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
        if got != want or got_log != want_log or got_r != want_r or (got_err is None) != (want_err is None):
            fails += 1
            if fails <= 5:
                diff = [(DATA[0] + o, got[o], want[o]) for o in range(len(got)) if got[o] != want[o]][:12]
                print(f"MISMATCH {os.path.basename(path)} new turn {i}: ret {got_r} / {want_r} errors {got_err} / "
                      f"{want_err}\n  bytes (addr, original, port) {[(hex(x), hex(y), hex(z)) for x, y, z in diff]}\n"
                      f"  leaves original {got_log[:24]}\n  leaves port     {want_log[:24]}")
            e.uc.mem_write(lo, got)
    LEAF_RET.clear()
    print(f"{os.path.basename(path)}: {n} turn changes, {fails} mismatches")
    return n, fails


def blasts(e, n, path, log, w):
    """--blast: n composed blasts on one dump; returns (blasts, mismatches)"""
    rt = e.rt
    rng = random.Random("blast " + os.path.basename(path))
    live = [k for k in range(0x30) if e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x182)
            and e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x138)]
    if not live:
        print(f"{os.path.basename(path)}: no soldiers in play, skipped")
        return 0, 0
    lo, hi = rt(DATA[0]), rt(DATA[1] - 1) + 1
    fails = 0
    for i in range(n):
        k = rng.choice(live)
        centre = (e.r16(rt(ai.G_SOLDIERS) + 0x200 * k + 0x114) + rng.choice([0, 0, 1, -1, 56, -57, 113])) % 3136
        e.w32(rt(SEED), rng.randrange(1 << 32))
        e.w8(rt(ai.G_BLASTER), rng.choice(live + [0x40]))
        radius, kind = rng.choice([0, 1, 2, 3, 3, 4, 5]), rng.choice([2, 3, 3, 4, 5, 6, 8, 9])
        gas = i % 4 == 3                      # every fourth case: the per-turn gas cloud tick (0x38520) instead
        fx = i % 4 == 1                       # every fourth: the explosion effect step (0x173B4) that starts a blast
        power = rng.choice([0, 1, 5, 0x80])
        if fx and rng.random() < 0.3:
            e.w8(rt(ai.G_TILEMAP) + 8 * centre + 1, rng.choice([0x35, 0x36, 0x86]))
        if gas:
            for j in range(5):
                c_ = rng.choice([NO_GAS, centre, (centre + rng.randrange(-120, 120)) % 3136])
                e.w16(rt(ai.G_GAS_CLOUDS) + 6 * j, c_)
                e.w16(rt(ai.G_GAS_CLOUDS) + 6 * j + 2, rng.choice([1, 1, 2, 3]))
                e.w16(rt(ai.G_GAS_CLOUDS) + 6 * j + 4, rng.choice([1, 2, 3]))
        snap = bytes(e.uc.mem_read(lo, hi - lo))
        log.clear()
        got_err = want_err = None
        try:
            if gas:
                e.call(rt(GAS_TICK), max_steps=STEPS)
            elif fx:
                e.call(rt(EFFECT), centre, kind, power, max_steps=STEPS)
            else:
                e.call(rt(BLAST), centre, radius, kind, max_steps=STEPS)
        except emu.EmuStop as x:
            got_err = str(x)
        got, got_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
        e.uc.mem_write(lo, snap)
        log.clear()
        try:
            if gas:
                ai.gas_tick(w)
            elif fx:
                ai.effect_hit(w, centre, kind, power)
            else:
                ai.blast(w, centre, radius, kind)
        except ZeroDivisionError:
            want_err = "divide by zero"
        want, want_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
        if got != want or got_log != want_log or (got_err is None) != (want_err is None):
            fails += 1
            if fails <= 5:
                diff = [(DATA[0] + o, got[o], want[o]) for o in range(len(got)) if got[o] != want[o]][:12]
                print(f"MISMATCH {os.path.basename(path)} blast {i} at {centre} r{radius} k{kind}: errors {got_err} / "
                      f"{want_err}\n  bytes (addr, original, port) {[(hex(x), hex(y), hex(z)) for x, y, z in diff]}\n"
                      f"  leaves original {got_log[:20]}\n  leaves port     {want_log[:20]}")
            e.uc.mem_write(lo, got)
    print(f"{os.path.basename(path)}: {n} blasts, {fails} mismatches")
    return n, fails


DIES, EFFECT = 0x3A4A4, 0x173B4


def dies(e, n, path, log, w):
    """--dies: n composed deaths (MercDies, quiet or not) on one dump; returns (deaths, mismatches)"""
    rt = e.rt
    rng = random.Random("dies " + os.path.basename(path))
    live = [k for k in range(0x30) if e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x182)
            and e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x138)]
    if not live:
        print(f"{os.path.basename(path)}: no soldiers in play, skipped")
        return 0, 0
    lo, hi = rt(DATA[0]), rt(DATA[1] - 1) + 1
    sec = e.r8(rt(ai.G_CUR_SECTOR))
    fails = 0
    for i in range(n):
        k = rng.choice(live)
        a = rt(ai.G_SOLDIERS) + 0x200 * k
        quiet = rng.choice([0, 1])
        e.w8(a + 0x1DA, rng.choice([rng.choice(live), rng.randrange(8), 0x40]))
        e.w8(rt(ai.G_SECTOR_TBL9) + 9 * sec + 1, rng.choice([0, 1, 2, 5]))
        e.w8(rt(ai.G_SECTOR_TBL9) + 9 * sec + 2, rng.choice([0, 1, 3]))
        snap = bytes(e.uc.mem_read(lo, hi - lo))
        log.clear()
        got_err = want_err = None
        try:
            e.call(rt(DIES), a, quiet, max_steps=STEPS)
        except emu.EmuStop as x:
            got_err = str(x)
        got, got_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
        e.uc.mem_write(lo, snap)
        log.clear()
        try:
            ai.merc_dies(MemRec(e, k), w, quiet)
        except ZeroDivisionError:
            want_err = "divide by zero"
        want, want_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
        if got != want or got_log != want_log or (got_err is None) != (want_err is None):
            fails += 1
            if fails <= 5:
                diff = [(DATA[0] + o, got[o], want[o]) for o in range(len(got)) if got[o] != want[o]][:12]
                print(f"MISMATCH {os.path.basename(path)} death {i} slot {k:#x} quiet {quiet}: errors {got_err} / "
                      f"{want_err}\n  bytes (addr, original, port) {[(hex(x), hex(y), hex(z)) for x, y, z in diff]}\n"
                      f"  leaves original {got_log}\n  leaves port     {want_log}")
        e.uc.mem_write(lo, snap)         # every death starts from the dump: soldiers stay alive
    print(f"{os.path.basename(path)}: {n} deaths over {len(live)} soldiers, {fails} mismatches")
    return n, fails


def turns(e, n, path, log, w):
    """--turn: n composed turn starts on one dump; returns (turns, mismatches)"""
    rt = e.rt
    rng = random.Random("turn " + os.path.basename(path))
    live = [k for k in range(0x30) if e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x182)
            and e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x138)]
    if not live:
        print(f"{os.path.basename(path)}: no soldiers in play, skipped")
        return 0, 0
    lo, hi = rt(DATA[0]), rt(DATA[1] - 1) + 1
    fails = endless = 0
    for i in range(n):
        k = rng.choice(live)
        a = rt(ai.G_SOLDIERS) + 0x200 * k
        e.w32(rt(SEED), rng.randrange(1 << 32))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 1]))
        hp, top = e.r8(a + 0x183), e.r8(a + 0x184)
        if rng.random() < 0.5:                # wounded: bleeding timer about to run out
            e.w8(a + 0x185, rng.choice([1, 5, 13, 20, (top - hp) & 0xFF]))
            e.w16(a + 0x1F0, rng.choice([1, 1, 2]))
            e.w8(a + 0x186, rng.choice([0, 0, 1]))
            e.w8(a + 0x183, rng.choice([hp, 9, 10, 15, 16]))
        if rng.random() < 0.15:               # exhausted in water
            e.w8(a + 0x18A, 0)
            e.w16(a + 0x1E8 + 2 * e.r8(a + 0x1F3), rng.choice([0x39, 0x3B, 0x3C]))
            e.w8(a + 0x1BA, rng.choice([0, 1]))
        if rng.random() < 0.4 and len(live) > 1:   # bandaging another soldier (first aid tick)
            q = rng.choice([j for j in live if j != k])
            b = rt(ai.G_SOLDIERS) + 0x200 * q
            pt = e.r16(b + 0x114)
            e.w8(a + 0x1B9, rng.choice([5, 5, 6]))
            e.w16(a + 0x1E8 + 2 * e.r8(a + 0x1F3), 0xF)
            e.w16(a + 0xD6, pt)
            e.w8(a + 0x1CA, rng.choice([0, 1, 2]))
            e.w8(a, rng.choice([0x4A, 0x49]))
            e.w8(a + 1, rng.choice([1, 1, 2, 3]))
            for o in (2, 8, 9):
                e.w8(a + o, rng.choice([1, 5, 20, 100]))
            if pt < 3136:
                e.w8(rt(ai.G_TILEMAP) + 8 * pt + 5, q)
            e.w8(b + 0x1B9, rng.choice([4, 4, 6]))
            e.w8(b + 0x1CA, rng.choice([1, 2]))
            e.w8(b + 0x185, rng.choice([0, 3, 10, 30]))
            e.w8(b + 0x183, rng.choice([1, 9, 14, 15, 40]))
        snap = bytes(e.uc.mem_read(lo, hi - lo))
        log.clear()
        got_err = want_err = None
        try:
            e.call(rt(TURN), a, max_steps=STEPS)
        except emu.EmuStop as x:
            got_err = str(x)
        got, got_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
        e.uc.mem_write(lo, snap)
        log.clear()
        try:
            ai.start_turn(MemRec(e, k), w)
        except ZeroDivisionError:
            want_err = "divide by zero"
        except RecursionError:             # an animation script that never ends: the original must not end either
            want_err = "endless animation"
        want, want_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
        if got_err and want_err:           # both never end (stack overflow / endless script): the state left is moot
            endless += 1
        elif got != want or got_log != want_log or (got_err is None) != (want_err is None):
            fails += 1
            if fails <= 5:
                diff = [(DATA[0] + o, got[o], want[o]) for o in range(len(got)) if got[o] != want[o]][:12]
                print(f"MISMATCH {os.path.basename(path)} turn {i} slot {k:#x}: errors {got_err} / {want_err}\n"
                      f"  bytes (addr, original, port) {[(hex(x), hex(y), hex(z)) for x, y, z in diff]}\n"
                      f"  leaves original {got_log}\n  leaves port     {want_log}")
            e.uc.mem_write(lo, got)
    print(f"{os.path.basename(path)}: {n} turn starts over {len(live)} soldiers" + (f" ({endless} never end on both "
          f"sides)" if endless else "") + f", {fails} mismatches")
    return n, fails


WINDOW_RATE = 0.25                       # --windows: every tick
WINDOW_OBJS = (0x7F, 0x80, 0x81, 0x82)   # wall objects (t1 class 7 / 8) with a window opacity >= 0x62 (sector 60 has 8)


def plant_windows(e, slot, i):
    """1 tick in 4 (--windows: every tick; own Random(i), so the other cases stay the same): window objects on 1..4 tiles along the line
    from the AI soldier to another soldier in play, so LineOfFire / SightLine meet the window rule of
    ProjectileHitTest 0x57AB7 (no real dump has an enemy line across a window). Returns (addr, old byte) pairs."""
    wr = random.Random(9000 + i)
    if wr.random() >= WINDOW_RATE:
        return []
    S = e.rt(ai.G_SOLDIERS)
    others = [k for k in range(0x30) if k != slot and e.r8(S + 0x200 * k + 0x182) and e.r8(S + 0x200 * k + 0x138)]
    if not others:
        return []
    o = wr.choice(others)
    x0, y0 = ai.s16(e.r16(S + 0x200 * slot + 0x116)), ai.s16(e.r16(S + 0x200 * slot + 0x118))
    x1, y1 = ai.s16(e.r16(S + 0x200 * o + 0x116)), ai.s16(e.r16(S + 0x200 * o + 0x118))
    me, them = e.r16(S + 0x200 * slot + 0x114), e.r16(S + 0x200 * o + 0x114)
    out = []
    for _ in range(wr.choice([1, 1, 2, 4])):
        f = wr.uniform(0.1, 0.9)
        t = combat.view_tile(int(x0 + (x1 - x0) * f) + 2, int(y0 + (y1 - y0) * f) + 4)
        if 0 <= t < 3136 and t not in (me, them):
            ad = e.rt(G_TILEMAP) + 8 * t + 1
            out.append((ad, e.r8(ad)))
            e.w8(ad, wr.choice(WINDOW_OBJS))
    return out[::-1]


# The ops below 63 in op::LIBRARY (ai.rs), by name. With host_ops = bit 32 only, the library must run these itself: a
# front-end host has no answer for them, so an op that reaches this host is a port fault even when the mirror's answer
# keeps the compare equal (Insert #12214: ja1logic:373, enemies would stop their walk to a noise).
LIBRARY_LOW = {"noise_tile", "idle", "direction", "tile_dist", "stop_anim", "walk_to", "path_ap", "find_path",
               "find_dry_tile", "patrol_tile", "buddy_tile", "news_value", "sector_value", "connected_home",
               "approach_tile", "ally_call_tile", "can_stand_at", "search_tile", "opp_strength", "retreat_tile",
               "advance_tile", "target_tile", "gun_option", "throw_option", "knife_option", "fire_line", "sight_line"}


def port_tick(e, w, slot):
    """--port: the clean-room AiThink (port_lib) on a copy of the world; the host ops run the composed mirror and
    oracle callees (World.ext) on emulator memory, so the copy is synced around every host call and random()"""
    bufs, scalars = port_lib.world_load(e, ai)
    reached = []

    def out(sc):
        port_lib.world_store(e, ai, bufs, sc)

    def back(sc):
        port_lib.world_load(e, ai, bufs, sc)

    def call(op, args):
        if op == "div_zero":               # the original crashes here (port_api.md, Division by zero)
            raise ZeroDivisionError
        if op in LIBRARY_LOW:
            reached.append(op)
        if op == "connected_home":         # the port passes the sector only; the original also the table address
            args = [ai.G_SECTOR_TBL9] + list(args)
        if op == "hears_msg":              # op 213 (ai.md Hearing a noise): the original's text, then its message call
            ai.hears_text(w, args[0], args[1], args[2] & 0xFF, args[3] & 0xFF)
            return (w.ext("message", ai.G_TEXT_BUF) or 0) & 0xFFFFFFFF, None
        v = w.ext(op, *args)
        if op == "noise_tile":
            return v[0], (v[1] & 0xFFFFFFFF).to_bytes(4, "little")
        if op in ("gun_option", "throw_option", "knife_option"):
            return 0, bytes(v)
        if op == "hit_test":               # (stops, new skip, chance) -> out record
            return v[0] & 0xFFFFFFFF, struct.pack("<BxxxI", v[1] & 0xFF, v[2] & 0xFFFFFFFF)
        return (v or 0) & 0xFFFFFFFF, None
    port_lib.ai_run("think", bufs, scalars, slot, w.random, call, sync=(out, back), host_ops=1 << 32)   # op 32 mission_blast: a leaf, as the original's stub; the library's own searches
    if reached:
        sys.exit(f"library ops reached the host: {sorted(set(reached))}")
    port_lib.world_store(e, ai, bufs, scalars)


def main():
    global PORT, WINDOW_RATE
    argv = sys.argv[1:]
    PORT = "--port" in argv
    if PORT:
        argv.remove("--port")
    fly = "--flight" in argv
    turn = "--turn" in argv
    melee = "--melee" in argv
    blast_mode = "--blast" in argv
    newturn = "--newturn" in argv
    die = "--dies" in argv
    if "--windows" in argv:
        WINDOW_RATE = 1.0
    argv = [a for a in argv if a not in ("--flight", "--turn", "--melee", "--blast", "--newturn", "--dies", "--windows")]
    n = int(([a for a in argv if a.isdigit()] or ["300"])[0])
    dumps = [a for a in argv if not a.isdigit()] or sorted(glob.glob(os.path.join(HERE, "..", "..", "build", "mem_*.bin")))
    rng = random.Random(7)
    fails = total = 0
    acts = {}
    for path in dumps:
        e = Emu(dump=path, fast=True)
        rt = e.rt
        log = []

        def leaf(name, kinds):
            def fn(emu_, eax, edx, ebx, ecx):
                sp = emu_.uc.reg_read(UC_X86_REG_ESP)
                stack = [emu_.r32(sp + 4 + 4 * i) for i in range(max(len(kinds) - 4, 0))]
                log.append((name, norm(kinds, [eax, edx, ebx, ecx] + stack, emu_)))
                return leaf_ret(name)
            return fn
        for name, (addr, kinds) in LEAVES.items():
            e.stub(rt(addr), leaf(name, kinds), LEAF_POP.get(name, 0))
        e.stub_sprintf()
        w = World(e, log)
        if fly or turn or melee or blast_mode or newturn or die:
            k_, f_ = (flight(e, rng, n, path, log, w, melee) if fly or melee else dies(e, n, path, log, w) if die else
                      blasts(e, n, path, log, w) if blast_mode else new_turns(e, n, path, log, w) if newturn else
                      turns(e, n, path, log, w))
            total, fails = total + k_, fails + f_
            continue
        slots = [k for k in range(8, 0x30) if e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x182)
                 and e.r8(rt(ai.G_SOLDIERS) + 0x200 * k + 0x138)]
        if not slots:
            print(f"{os.path.basename(path)}: no AI soldiers in play, skipped")
            continue
        if PORT and e.r8(rt(ai.G_CUR_SECTOR)) >= 60:  # not in a tactical sector (office, strategic): the original
            print(f"{os.path.basename(path)}: no tactical sector, skipped")   # reads past its sector tables there
            continue
        lo, hi = rt(DATA[0]), rt(DATA[1] - 1) + 1
        dfails = 0
        for i in range(n):
            slot = rng.choice(slots)
            a = rt(ai.G_SOLDIERS) + 0x200 * slot
            if i % 3 == 0:                    # vary the state a little: seed, turn mode, alert, AP, rethink
                e.w32(rt(SEED), rng.randrange(1 << 32))
                e.w32(rt(ai.G_TURN_BASED), rng.choice([0, 1]))
                e.w32(rt(ai.G_BUSY), 0)
                e.w8(rt(ai.G_ACTIVE), 0x40)
                e.w8(a + 0xBB, rng.choice([e.r8(a + 0xBB), 0, 1, 2]))
                e.w8(a + 0x1A5, rng.choice([e.r8(a + 0x1A5), 0, 4, 10, 20, 40]))
                e.w8(a + 0x1C1, rng.choice([e.r8(a + 0x1C1), 1]))
                e.w8(a + 0x1B8, rng.choice([e.r8(a + 0x1B8), 1]))
            if i % 3 == 0 and rng.random() < 0.5:   # other hand items, alarm, mission order: the rarer actions
                e.w8(a + 0x00, rng.choice([e.r8(a), 0x18, 0x1A, 0x1C, 0x16, 0x0F, ai.ITEM_RADIO]))
                e.w8(a + 0x01, rng.choice([1, 1, 2]))
                e.w8(a + 0x02, rng.choice([e.r8(a + 2), 100]))
                e.w8(rt(ai.G_ALARM), rng.choice([0, 0, 1]))
                e.w8(rt(ai.G_MISSION), rng.choice([0, 0, 0, 1, 2]))
                e.w8(rt(ai.G_MISSION_ORDER), rng.choice([0x40, slot]))
            planted = plant_windows(e, slot, i)
            snap = bytes(e.uc.mem_read(lo, hi - lo))
            log.clear()
            try:
                e.call(rt(THINK), a, max_steps=STEPS)
                got_err = None
            except emu.EmuStop as x:
                got_err = str(x)
            got, got_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
            e.uc.mem_write(lo, snap)
            log.clear()
            try:
                if PORT:
                    port_tick(e, w, slot)
                else:
                    ai.think(MemRec(e, slot), w)
                want_err = None
            except ZeroDivisionError:
                want_err = "divide by zero"
            want, want_log = bytes(e.uc.mem_read(lo, hi - lo)), list(log)
            if PORT:                             # FindPath's own scratch (trail count, trails, nodes, best cost) is
                want = bytearray(want)           # not world state: the port keeps it inside the library
                for a_, k_ in FINDPATH_SCRATCH:
                    want[a_ - DATA[0]:a_ - DATA[0] + k_] = got[a_ - DATA[0]:a_ - DATA[0] + k_]
                want = bytes(want)
            total += 1
            act = e.r8(a + 0x1D1)
            acts[act] = acts.get(act, 0) + 1
            same = got == want and got_log == want_log and (got_err is None) == (want_err is None)
            if not same:
                fails += 1
                dfails += 1
                if dfails <= 5:
                    diff = [(DATA[0] + o, got[o], want[o]) for o in range(len(got)) if got[o] != want[o]][:12]
                    print(f"MISMATCH {os.path.basename(path)} tick {i} slot {slot:#x}: errors {got_err} / {want_err}\n"
                          f"  bytes (addr, original, port) {[(hex(x), hex(y), hex(z)) for x, y, z in diff]}\n"
                          f"  leaves original {got_log}\n  leaves port     {want_log}")
                e.uc.mem_write(lo, got)          # carry on from the original's state
            for a_, b_ in planted:               # take the planted windows away again
                e.w8(a_, b_)
        print(f"{os.path.basename(path)}: {n} ticks over {len(slots)} AI soldiers, {dfails} mismatches")
    if os.environ.get("COMPOSE_CALLS"):
        print("calls:", dict(sorted(CALLS.items())))
    if fly or turn or melee or blast_mode or newturn or die:
        mode, unit = (("--flight", "shots") if fly else ("--melee", "blows") if melee else ("--dies", "deaths") if die else
                      ("--blast", "blasts") if blast_mode else ("--newturn", "turn changes") if newturn else
                      ("--turn", "turn starts"))
        print(f"compose_diff {mode}: {total} {unit}; {fails} mismatches")
        sys.exit(1 if fails else 0)
    print(f"compose_diff{' (port)' if PORT else ''}: {total} AiThink ticks, actions {dict(sorted(acts.items()))}; {fails} mismatches")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
