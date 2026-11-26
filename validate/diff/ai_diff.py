"""Differential test: tools/ai.py deciders vs the original AI functions in Unicorn.

usage: .venv/bin/python validate/diff/ai_diff.py [DUMP] [n=12000] [--only NAME[,NAME...]] [--group K/N] [--fan IDS|all (with --port --only take_damage / end_task / swap_places / walk_end; #70 F08..F14)]
Each case takes an enemy record from the snapshot, fuzzes the bytes the AI reads (AI type, alert state, orders/attitude,
energy, hp, AP, facing, flags, the tile's 0x80 bit, the water bytes) and the globals (alarm, combat, enemies left),
then runs one decider in both. Random 0x53538 (biased to multiples of 5 and one below: every threshold constant
is a multiple of 5) and the callees that are not ported yet (searches for tiles etc.) are
stubbed on both sides with the same scripted results. Compared: return value, the whole 0x200-byte soldier record
afterwards and the ordered list of calls (callee + arguments). Exits 1 on any mismatch.
"""
import collections
import ctypes, os, random, struct, sys, time
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import ai                            # noqa: E402
import emu                           # noqa: E402
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP, UC_X86_REG_ESI   # noqa: E402

RANDOM, G_SOLDIERS, G_TILEMAP = 0x53538, 0xFA958, 0x100A18
# callee name -> (address, argument kinds: s soldier, t tile (s16), b byte, i raw)
SEL_STUBS = {"w_46a4c": (0x46A4C, "bi"), "w_24880": (0x24880, "iii"), "w_11448": (0x11448, "s"), "w_67c54": (0x67C54, "")}
# (select.md host ops 195, 196, 197, 198: stubbed only in the select_merc section; walk_diff stubs them everywhere.
# Other deciders reach 0x67C54 for real, e.g. revive_roll.)
CALLEES = {"idle": (0x5AFA4, "s"), "noise_tile": (0x7038C, "s"), "find_dry_tile": (0x713D0, "s"),
           "patrol_tile": (0x6FAEC, "s"), "buddy_tile": (0x6FC90, "s"), "direction": (0x52560, "tttt"),
           "tile_dist": (0x15A24, "tt"), "news_value": (0x705F4, "s"), "sector_value": (0x3E03C, "bb"),
           "approach_tile": (0x7068C, "st"), "ally_call_tile": (0x702A8, "s"), "search_tile": (0x70850, "sb"),
           "move_ap": (0x1B118, "s"), "reach_ap": (0x3DC4C, "sti"), "attack_ready": (0x584A0, "st"),
           "swap_hand": (0x55B0C, "sii"), "action_ap": (0x56310, "stb"), "can_stand_at": (0x72578, "st"),
           "mission_cancel": (0x72670, ""), "opp_strength": (0x6F314, "s"), "retreat_tile": (0x70FD0, "s"),
           "advance_tile": (0x6FE7C, "sb"), "target_tile": (0x70114, "s"), "gun_option": (0x71590, "s"),
           "throw_option": (0x717F8, "s"), "knife_option": (0x71DD4, "s"), "busy": (0x5AD1C, "s"),
           "turn": (0x1AA18, "sbi"), "walk_to": (0x195A0, "st"), "click": (0x59C14, "stb"), "spend_ap": (0x329BC, "sbi"),
           "wait": (0x1AB60, "i"), "radio_result": (0x63254, ""), "share_sightings": (0x736BC, "si"),
           "perceive": (0x740FC, "si"), "delay": (0x67250, "i"), "santino_speech": (0x5F4C4, ""),
           "fire": (0x57B34, "i"), "fire_line": (0x580CC, "stb"),
           "sight_check": (0x72F50, "ss"), "path_ap": (0x1D32C, "st"),
           "find_path": (0x40810, "sti"), "frame_active": (0x1B9D4, "s"), "frame_other": (0x1B694, "s"),
           "itq_pop": (0x12C14, ""), "w_12dd4": (0x12DD4, ""), "w_12404": (0x12404, ""), "new_turn": (0x126C8, "i"),
           "projectile_step": (0x57BB8, "s"), "chance_to_hit": (0x56630, "stb"), "shot_ap": (0x564F8, "st"),
           "sight_line": (0x58760, "snnnnnnnn"), "hit_test": (0x577B4, "nnnnbbntin"), "clear_rooms": (0x61744, ""),
           "connected_home": (0x41178, "xb"), "quest_check": (0x633D8, "i"),
           "stat_practice": (0x62A68, "sbbb"), "merc_sees": (0x56D28, "bbb"), "stop_walk": (0x104C8, "si"), "walk_blocked": (0x19818, "stb"), "anim_next": (0x19470, "s"), "merc_quote": (0x1B324, "s"),
           "consume_trigger": (0x19E20, "i"), "w_735bc": (0x735BC, "s"), "throw_item": (0x19A6C, "s"), "ai_think": (0x6D1A8, "s"), "w_1aaf0": (0x1AAF0, "s"), "w_5afbc": (0x5AFBC, "s"),
           "stop_anim": (0x196F8, "si"), "focus": (0x3F724, "bi"), "message": (0x27D24, "x"), "draw_sectors": (0x41C3C, "x"), "w_36010": (0x36010, "xxiii"),
           "anim_step": (0x1F2C4, "s"), "set_anim": (0x17F90, "sbbb"),
           "end_task": (0x3D98C, "sb"), "replace": (0x2FB98, "s"), "merc_message": (0x299B8, "sbbxi"),
           "anim_frame": (0x18498, "s"), "ui_refresh": (0x1D8B8, "i"), "anim_turn": (0x724E0, "si"),
           "reserve_ap": (0x17C88, "s"), "walk_sound": (0x335F8, "bi"), "sfx": (0x5CB34, "iiii"),
           "mouse_hide": (0x67368, ""), "tac_redraw": (0x176C8, "i"), "open_dat": (0x5E8E0, "m"),
           "load_pcc": (0x532BC, "ixmxi"), "fx_load": (0x5C0EC, "mim"), "merc_voice": (0x24D14, "bimii"),
           "fx_play": (0x5C144, ""), "blit": (0x52AE0, "ixiiii"), "w_320a0": (0x320A0, "bi"), "sector_dat": (0x2F7C8, ""),
           "mouse_show": (0x67398, ""), "sector_won": (0x3A0B0, ""), "quote_event": (0x1B2A4, "bbbii"),
           "w_46a4c": (0x46A4C, "bi"), "squad_lost": (0x3A32C, ""),
           "w_635dc": (0x635DC, ""), "w_355d0": (0x355D0, "iiiii"), "w_72568": (0x72568, "ti"),
           "w_19eb0": (0x19EB0, "ti"), "w_35774": (0x35774, "ii"), "w_3c16c": (0x3C16C, "sti"),
           "workable": (0x41E1C, "xi"), "explode_art": (0x33BD4, "ii"), "w_3c05c": (0x3C05C, "ii"),
           "merc_text": (0x15910, "smi"), "mine_flag": (0x19D74, "st"), "walter_say": (0x3C985, "n"), "enemy_turned": (0x73578, "s"),
           "guard_turned": (0x7351C, "s"), "w_11448": (0x11448, "s"), "w_17b70": (0x17B70, "bi"), "w_3f490": (0x3F490, "ii"), "w_3f578": (0x3F578, "i"),
           "quote": (0x3C574, "st"), "w_58574": (0x58574, "s"), "use_special": (0x63458, "s"),
           "door_fx": (0x3B03C, "stb"), "popups": (0x288DC, "si"), "w_3cdd4": (0x3CDD4, "s"),
           "w_3cfd0": (0x3CFD0, "stb"), "bury_mine": (0x3CB24, "st"), "w_401b4": (0x401B4, "sb"),
           "w_3521c": (0x3521C, "s"), "w_3a8c0": (0x3A8C0, "si"), "w_5abe0": (0x5ABE0, "st"),
           "w_5ad50": (0x5AD50, "st"), "launch": (0x58DE4, "s"),
           "merc_dies": (0x3A4A4, "si"), "anim_end": (0x1F0F0, "sb"), "portrait": (0x31C98, "bi"),
           "w_197d8": (0x197D8, "si"), "step_sound": (0x336B4, "bi"), "ui_text": (0x254F0, "ixii"),
           "ui_10310": (0x10310, "x"), "w_54ec8": (0x54EC8, "s"), "close_door": (0x3ADB4, "tbi"), "w_51a24": (0x51A24, "iiii"), "w_59ae8": (0x59AE8, "sttbi"), "w_1c438": (0x1C438, "si"),
           "w_5aa08": (0x5AA08, "st"), "w_59a54": (0x59A54, "s"), "reloaded": (0x40094, "s"),
           "first_aid": (0x3D750, "stbb"), "confirm_click": (0x1C514, "t"), "w_1c464": (0x1C464, "s"), "item_cond": (0x55BE4, "isii"),
           "item_panel": (0x55FD8, "sii"),
           "next_turn": (0x12D58, "i"), "w_1304c": (0x1304C, ""), "use_throw": (0x535E8, "s"),
           "drop_item": (0x470B0, "s"), "w_56dd4": (0x56DD4, "sbb"), "impact": (0x5713C, "stiii"),
           "w_746d4": (0x746D4, "sb"), "make_noise": (0x727D0, "btbbi"), "fx_157bc": (0x157BC, "tbbiii"),
           "explode": (0x3B79C, "btii"), "land_item": (0x56FE4, "stbiii"), "take_damage": (0x392FC, "siiiii"),
           "bullet_damage": (0x39FD8, "s"), "replay_event": (0x3FA04, "iii"), "resolve_battle": (0x3E508, "bi"), "save_tmp": (0x2F800, ""),
           "fade": (0x52908, "xiiii"), "music_stop": (0x5C82C, ""), "end_of_day": (0x29F44, ""), "timer_off": (0x5F298, ""),
           "leave_sector": (0x30D6C, "b"), "story_hit": (0x61D28, ""),
           "puff": (0x156B8, "tiipiii"), "door_lookup": (0x3AFE0, "t"), "break_obj": (0x3ACB8, "tbi"),
           "rest": (0x32754, "sib"), "hit_location": (0x38D80, "sii"),
           "energy_bar": (0x32610, "sb"), "w_33204": (0x33204, "b"), "w_323d4": (0x323D4, "si"), "anim2": (0x17F5C, "sbbb"),
           "w_74304": (0x74304, "sbib"), "react": (0x743E8, "ssb"),
           "w_53660": (0x53660, ""), "w_32eb8": (0x32EB8, "bi"), "w_3fc04": (0x3FC04, "b"),
           "w_3a2d4": (0x3A2D4, ""), "w_32828": (0x32828, ""), "turn_banner": (0x1054C, ""), "w_10398": (0x10398, "i"),
           "w_1148c": (0x1148C, "iii"), "w_67e30": (0x67E30, ""), "w_328a4": (0x328A4, "s"), "aid_tick": (0x15260, "s"), "w_32d04": (0x32D04, "s"), "w_331a4": (0x331A4, "bi"), "w_1f6e4": (0x1F6E4, ""), "move_merc": (0x1C608, "stbbbbbb"), "w_11fb4": (0x11FB4, "s"), "w_10454": (0x10454, "i"), "w_67bf0": (0x67BF0, ""), "w_681d0": (0x681D0, "ii"), "w_67e18": (0x67E18, ""),
           "w_1ac68": (0x1AC68, "stbi"), "disarm": (0x3B9D8, "stbii"), "mine_spotted": (0x3C31C, "stb"), "put_item": (0x3FC78, "txxxiiiiiii"), "yes_no": (0x28C84, "mbxii"), "w_53708": (0x53708, "iii"), "w_1cb78": (0x1CB78, "b"),
           "hears": (0x72A60, "stbb"), "w_73c00": (0x73C00, "s"), "w_73d28": (0x73D28, "ss"), "w_73f3c": (0x73F3C, "ss"),
           "cry": (0x24E44, "bimii"), "voc": (0x5C9C0, "iii"), "show_msg": (0x27B98, "miiiiiii"), "w_73ab4": (0x73AB4, "ssb"),
           "w_6a960": (0x6A960, ""), "guards_arrive": (0x1F724, "i"), "w_1583c": (0x1583C, ""), "gas_tick": (0x38520, ""),
           "mouse_in_rect": (0x67C0C, "iiii"), "w_673ac": (0x673AC, ""), "w_124fc": (0x124FC, ""), "w_1259c": (0x1259C, ""),
           "w_410c4": (0x410C4, ""), "w_297d4": (0x297D4, "si"), "stage_five": (0x74C44, ""), "w_31550": (0x31550, "")}
POP = {"disarm": 4, "w_36010": 4, "show_msg": 16, "fade": 4, "yes_no": 4, "w_355d0": 4, "load_pcc": 4, "merc_voice": 4, "blit": 8, "quote_event": 4, "put_item": 0x1C, "cry": 4, "fx_157bc": 8, "land_item": 8, "take_damage": 8, "puff": 12, "make_noise": 4, "impact": 4, "w_59ae8": 4, "merc_message": 4, "path_ap": 12, "move_merc": 16, "sight_line": 20, "hit_test": 0x18}   # Watcom stack arguments the callee pops                    # Watcom stack arguments the callee pops
RECORDS = {"gun_option", "throw_option", "knife_option"}      # fill a 10-byte option record through EDX
GLOBALS_W = [(0x92DCD, 1), (0x22A486, 2), (0x92DA8, 4), (0x92DEC, 4), (0x284F24, 4), (0x284F0F, 3), (0x22A0FF, 1), (0x95DC8, 4), (ai.G_SLOT_T34, 0x30), (ai.G_ALARM, 1), (ai.G_MISSION, 1), (ai.G_SANTINO_SPOKE, 1), (0x22A008, 4),
             (ai.G_KNOWN, 0x30), (ai.G_REMEMBER_E, 0x1C0), (ai.G_LAST_SEEN_E, 0x500), (ai.G_LAST_SEEN_M, 0x280),
             (ai.G_LAST_SPOTTER, 1), (ai.G_CALL_M, 3), (ai.G_CALL_E, 3), (0x229C94, 4), (0x92E52, 2),
             (ai.G_MISSION_SPOT, 2), (ai.G_AP_WARNED, 8), (ai.G_STOP_TIMER, 4),
             (0x229E04, 4), (0x92DCA, 1), (ai.G_BUSY_UI, 1),
             (0x229DD4, 4), (ai.G_ACTIVE_SLOT, 1),
             (ai.G_BUSY, 4), (0x92DB4, 4),
             (0x92E4C, 2), (0x92DC4, 1), (0x22A0D1, 1), (0x22A1F5, 1), (0x229E03, 5),
             (ai.G_DMG_TEXT, 0x100), (ai.G_KNOCKDOWN, 1), (ai.G_HIT_SCALE, 1),
             (ai.G_INTERRUPTED, 1), (0x284F0F, 1),
             (ai.G_ITQ, 0x10), (ai.G_ITQ_N, 1), (ai.G_TEXT_BUF, 0x80),
             (ai.G_BLAST_FX, 1), (ai.G_BLAST_FX_ID, 4), (ai.G_BLED, 0x30), (0x92DCC, 1), (ai.G_ACTIVE_SLOT, 1),
             (ai.G_SECTOR_TBL9, 9 * 0x50), (ai.G_TEAM_ON_DUTY, 4), (ai.G_DEAD_MERCS, 12), (0x92DD5, 1), (0x22A0D2, 1),
             (ai.G_MISSION_ORDER, 4), (0x92DCE, 1), (ai.G_SECTOR_REC, 0xB1 * 0x40), (ai.G_STAT_PROGRESS, 18 * 8), (0x92DD0, 1), (0x92DCF, 1), (0x92DD2, 1), (0x93D54, 4), (0x95E0C, 1), (0x284F1B, 3), (ai.G_MARKER, 8), (0x229D20, 4), (0x229D40, 4),
             (ai.G_SELECTED, 1), (0x92DD2, 1), (0x229D2C, 4), (0x229D18, 4), (ai.G_ENEMY_DEATHS, 4), (ai.G_SQUAD_LOST, 1), (0x222BCC, 0xB1 * 0x40 + 0x16), (ai.G_GROUND_ITEMS, 12 * 256), (ai.G_MISSION_STEP, 1),
             (ai.G_FX, 0x41), (ai.G_BLAST_LIST, 60), (ai.G_FX_FLAG, 1), (ai.G_SAFE_OPENED, 1), (0x229DF0, 4),
             (ai.G_NOTES_FOUND, 30)]                   # globals a decider may write: restored between runs, compared
MELEE_HI = random.Random("melee_hi")
VOID = {"walter", "lock_kit", "mine_flag", "fire_gun", "attack_go", "anim_end", "active_frame", "consume_trigger", "throw_scatter", "throw_aim", "throw_follow", "soldier_frame", "frame_checks", "merc_quote", "enemies_see", "mercs_see", "step_noise", "walk_end", "stand_up", "walk_step", "anim_next", "walk_blocked", "xra_enemy", "use_on_tile", "pickup_here", "bury_mine", "shot_alert", "reserve_ap", "gun_jam", "start_attack", "noise_int", "merc_dies", "aid_tick", "turn_rest", "stat_practice", "spend_ap", "kill", "melee_hit", "revive_roll", "start_turn", "bleed", "drown", "land_item", "rest", "hears", "interrupt_check", "hit_alert", "hit_reaction", "take_damage", "impact", "w_56dd4", "pstep", "launch", "fire", "w_5aa08", "stop_anim", "w_197d8", "stop_walk", "replace", "end_task", "anim_turn", "anim_frame", "set_anim", "anim_delay", "anim_step", "walk_to", "update_alert", "think", "update_sightings", "mark_seen", "share_sightings", "idle", "enemy_turned", "guard_turned"}
DECIDERS = {"update_alert": (0x6D574, ai.update_alert), "calm_action": (0x6DB10, ai.calm_action),
            "alert_action": (0x6DEE8, ai.alert_action), "combat_action": (0x6E3F4, ai.combat_action),
            "engaged_action": (0x6EB54, ai.engaged_action), "choose_action": (0x6D7C0, ai.choose_action),
            "action_affordable": (0x6D864, ai.action_affordable), "do_action": (0x6D904, ai.do_action),
            "think": (0x6D1A8, ai.think), "opp_strength": (0x6F314, ai.opp_strength),
            "soldier_power": (0x6F8DC, ai.soldier_power), "target_tile": (0x70114, ai.target_tile),
            "update_sightings": (0x740FC, ai.update_sightings), "move_ap_cost": (0x1B118, ai.move_ap_cost),
            "ally_call_tile": (0x702A8, ai.ally_call_tile), "news_value": (0x705F4, ai.news_value),
            "noise_tile": (0x7038C, ai.noise_tile), "patrol_tile": (0x6FAEC, ai.patrol_tile),
            "approach_tile": (0x7068C, ai.approach_tile), "advance_tile": (0x6FE7C, ai.advance_tile),
            "buddy_tile": (0x6FC90, ai.buddy_tile), "can_stand_at": (0x72578, ai.can_stand_at),
            "find_dry_tile": (0x713D0, ai.find_dry_tile), "retreat_tile": (0x70FD0, ai.retreat_tile),
            "search_tile": (0x70850, ai.search_tile), "gun_option": (0x71590, ai.gun_option),
            "knife_option": (0x71DD4, ai.knife_option), "throw_option": (0x717F8, ai.throw_option),
            "line_of_fire": (0x580CC, ai.line_of_fire), "sight_check": (0x72F50, ai.sight_check),
            "mark_seen": (0x732FC, ai.mark_seen), "no_ally_sees": (0x73268, ai.no_ally_sees),
            "share_sightings": (0x736BC, ai.share_sightings), "idle": (0x5AFA4, ai.idle),
            "enemy_turned": (0x73578, ai.enemy_turned), "guard_turned": (0x7351C, ai.guard_turned),
            "reach_tile": (0x3DC4C, ai.reach_tile), "attack_ready": (0x584A0, ai.attack_ready),
            "turn_to": (0x1AA18, ai.turn_to), "walk_to": (0x195A0, ai.walk_to),
            "anim_step": (0x1F2C4, ai.anim_step), "anim_end": (0x1F0F0, ai.anim_end), "set_anim": (0x17F90, ai.set_anim),
            "anim_delay": (0x15BB8, ai.anim_delay), "msg_not_enough_ap": (0x15960, ai.msg_not_enough_ap),
            "anim_frame": (0x18498, ai.anim_frame), "anim_turn": (0x724E0, ai.anim_turn),
            "end_task": (0x3D98C, ai.end_task), "replace": (0x2FB98, ai.replace_soldier), "xra_enemy": (0x2B6D0, ai.init_xra_enemy),
            "stop_anim": (0x196F8, ai.stop_anim), "w_197d8": (0x197D8, ai.snap_to_tile),
            "stop_walk": (0x104C8, ai.stop_walk), "walk_step": (0x1A108, ai.walk_step), "walk_blocked": (0x19818, ai.walk_blocked), "stand_up": (0x1B188, ai.stand_up), "walk_end": (0x1B8AC, ai.walk_end), "enemies_see": (0x7365C, ai.enemies_see), "mercs_see": (0x735BC, ai.mercs_see), "step_noise": (0x19A6C, ai.step_noise), "nearest_merc": (0x1AAF0, ai.nearest_merc), "frame_checks": (0x1B504, ai.frame_checks), "merc_quote": (0x1B324, ai.merc_quote), "soldier_frame": (0x1B694, ai.soldier_frame), "active_frame": (0x1B9D4, ai.active_frame), "consume_trigger": (0x19E20, ai.consume_trigger), "throw_scatter": (0x598A8, ai.throw_scatter), "throw_aim": (0x59364, ai.throw_aim), "throw_follow": (0x5AFBC, ai.throw_follow), "find_path": (0x40810, ai.find_path_op), "path_ap": (0x1D32C, ai.path_ap_cost), "click": (0x59C14, ai.tactical_click),
            "w_59a54": (0x59A54, ai.jam_check), "w_5aa08": (0x5AA08, ai.aim_posture),
            "w_59ae8": (0x59AE8, ai.use_cost), "fire": (0x57B34, ai.fire_now),
            "launch": (0x58DE4, ai.launch_projectile), "w_56dd4": (0x56DD4, ai.shot_end),
            "pstep": (0x57BB8, ai.projectile_step), "impact": (0x5713C, ai.projectile_impact),
            "take_damage": (0x392FC, ai.take_damage), "hit_reaction": (0x74304, ai.hit_reaction),
            "hit_alert": (0x743E8, ai.hit_alert), "interrupt_check": (0x73AB4, ai.interrupt_check),
            "hears": (0x72A60, ai.hears), "rest": (0x32754, ai.rest), "land_item": (0x56FE4, ai.land_item),
            "revive_roll": (0x401B4, ai.revive_roll), "melee_hit": (0x3521C, ai.melee_hit), "kill": (0x3A8C0, ai.kill), "merc_dies": (0x3A4A4, ai.merc_dies), "noise_int": (0x73C00, ai.noise_interrupt), "shot_alert": (0x746D4, ai.shot_alert), "trigger": (0x1AC68, ai.trigger_item), "reserve_ap": (0x17C88, ai.reserve_ap), "gun_jam": (0x58574, ai.gun_jam), "bury_mine": (0x3CB24, ai.bury_mine), "pickup_here": (0x3CDD4, ai.pickup_here), "lock_kit": (0x63458, ai.use_special), "walter": (0x3C574, ai.walter), "mine_flag": (0x19D74, ai.mine_flag), "use_on_tile": (0x3CFD0, ai.use_on_tile), "start_attack": (0x5ABE0, ai.start_attack), "fire_gun": (0x5959C, ai.fire_gun), "chance_to_hit": (0x56630, ai.chance_to_hit), "attack_go": (0x5AD50, ai.attack_go),
            "spend_ap": (0x329BC, ai.spend_ap), "stat_practice": (0x62A68, ai.stat_practice),
            "turn_rest": (0x32D04, ai.turn_rest), "aid_tick": (0x15260, ai.first_aid_tick),
            "apply_aid": (0x39CA4, ai.apply_first_aid), "first_aid": (0x3D750, ai.first_aid_start), "start_turn": (0x11FB4, ai.start_turn), "bleed": (0x15034, ai.bleed), "drown": (0x3843C, ai.drown), "merc_interrupt": (0x73D28, ai.merc_interrupt), "enemy_interrupt": (0x73F3C, ai.enemy_interrupt), "swap_hand": (0x55B0C, ai.swap_hand), "busy": (0x5AD1C, ai.soldier_busy)}
MEMRUN = {"active_frame", "kill", "shot_alert", "start_attack", "end_task", "fire", "aid_tick", "apply_aid", "first_aid"}         # the port runs on emulator memory (it writes other records that may be the soldier's)
MAPCMP = {"active_frame", "soldier_frame", "walk_step", "walk_blocked", "xra_enemy", "use_on_tile", "trigger", "start_attack", "merc_dies", "anim_frame", "hears", "land_item", "melee_hit", "turn_rest", "start_turn", "kill", "bury_mine"}       # move occupants on the tile map: the map is compared too (kill, bury_mine: Put item writes byte 6; restored between runs)
MAP_HI = G_TILEMAP + 8 * (3136 + 64)       # compared: the pointer table 0x100958, the map and 64 tiles past it
MCCMP = {"trigger"}               # --fan 51: the move-code table is compared too (TriggerItem? 0x1AE80 / 0x1AF42)
VIEW_EDGE = {"active_frame", "soldier_frame", "start_turn"}   # MercView rays past the top edge write before the map (into soldier 0x2F): restored too


def map_lo(name):
    return G_TILEMAP - 0x200 if name in VIEW_EDGE else G_SOLDIERS + 0x6000
OTHERS = {"active_frame", "shot_alert", "start_attack", "first_aid", "aid_tick", "apply_aid", "share_sightings", "end_task", "hit_reaction", "melee_hit", "kill"}       # write other soldiers' records: all 48 are compared
QUIET_N = [0]                     # start_turn cases so far (own generator of the no-order cases)
FP_N = [0]                        # find_path / path_ap cases so far (own generator of the off-map cases)
TURN = {"start_turn", "bleed", "drown", "revive_roll"}           # soldier-only turn-start routines
CONTEST = {"merc_interrupt", "enemy_interrupt"}   # only ever called across sides: the slot bytes index records
PAIR = CONTEST | {"interrupt_check", "hit_alert", "sight_check", "mark_seen", "no_ally_sees"}   # EDX = an opponent soldier, whose record is compared too
OUTREC = emu.STACK_TOP - 0x40            # attack option record (EDX), between the call frame and the stack top
OUT = 0x22A008                           # NoiseTile's EDX score output (in GLOBALS_W: restored and compared)
MASK = {"swap_hand": 0, "busy": 0xFFFFFFFF, "nearest_merc": 0xFFFFFFFF, "path_ap": 0xFFFFFFFF, "apply_aid": 0xFFFFFFFF, "soldier_power": 0xFFFFFFFF, "reach_tile": 0xFFFFFFFF, "attack_ready": 0xFFFFFFFF, "target_tile": 0xFFFF, "move_ap_cost": 0xFFFFFFFF, "line_of_fire": 0xFFFFFFFF, "ally_call_tile": 0xFFFF,
        "noise_tile": 0xFFFF, "patrol_tile": 0xFFFFFFFF, "approach_tile": 0xFFFF, "advance_tile": 0xFFFF, "find_dry_tile": 0xFFFF, "retreat_tile": 0xFFFF, "search_tile": 0xFFFF}   # return bits compared (default: AL)
SIDE = {"opp_strength", "soldier_power", "target_tile", "update_sightings", "ally_call_tile", "news_value",
        "noise_tile", "advance_tile", "buddy_tile", "retreat_tile", "search_tile", "gun_option", "knife_option", "throw_option", "share_sightings",
        "enemy_turned", "guard_turned"} | PAIR   # read other soldiers: fuzz them all

argv = [a for a in sys.argv[1:]]
only = argv[argv.index("--only") + 1] if "--only" in argv else None
if only:
    del argv[argv.index("--only"):argv.index("--only") + 2]
ONLY = set(only.split(",")) if only else set()   # --only a,b,c: several sections
PORT_SECTIONS = ("kept_turn", "sector_won", "swap_places", "next_enemy", "next_merc", "mission_cancel", "disarm", "mine_spotted", "blast_tile", "blast", "gas_tick", "sector_tick", "explode", "effect_hit", "effect_pass", "pay", "view", "rebuild", "radio_result", "quest_check", "action_ap", "shot_ap", "sector_value", "slot_reset", "equip", "guard_leaves", "merc_entry", "put_item", "pickup", "init_soldier", "supplies", "patches", "ground_blast", "tmp_file", "sector_start", "move", "loop", "ticks", "turn_end", "turn_flow", "turn_new", "turn_start", "select_merc", "hide_path_marks", "loop_rest", "fuses", "loop_head", "leave_tests", "door_fx", "doors", "reload", "make_noise", "leave_sector", "leave_path", "end_of_day", "connected", "ledger", "escape_stop", "walk_in", "explode_dat")   # direct sections with a --port branch
FAN_RUNS = {8: [(0x39A2F, 0x39A35), (0x39A3F, 0x39A5E), (0x39A8B, 0x39A8C)], 9: [(0x39A9B, 0x39A9D)], 10: [(0x3974D, 0x3975B), (0x3998A, 0x3999B)], 11: [(0x3996B, 0x39981)], 12: [(0x3DA57, 0x3DA60)], 13: [(0x3F8D0, 0x3F909)], 14: [(0x1B98F, 0x1B991)], 15: [(0x124CA, 0x124D3), (0x18E98, 0x18EC4)], 17: [(0x72FCD, 0x72FE0), (0x73360, 0x73373), (0x7352C, 0x73546)], 18: [(0x1125D, 0x11285), (0x1220D, 0x12239), (0x5F216, 0x5F24B), (0x3D4D5, 0x3D4E2)], 19: [(0x12B50, 0x12B59)], 20: [(0x126D1, 0x126E6), (0x12702, 0x12706), (0x1277C, 0x1277E), (0x127D4, 0x127D6), (0x127E2, 0x127E5), (0x129C9, 0x129D5), (0x12BE0, 0x12C13)], 21: [(0x30B28, 0x30B2D), (0x30C4C, 0x30C4E)], 22: [(0x30F7D, 0x30F82), (0x4889B, 0x488A6)], 23: [(0x1F3A9, 0x1F3B3), (0x1F3FF, 0x1F410), (0x18E35, 0x18E3A), (0x18E78, 0x18E7F), (0x18F18, 0x18F1D), (0x18FC8, 0x18FCA), (0x18FE4, 0x18FE9), (0x192A4, 0x192CD)], 24: [(0x1A588, 0x1A589), (0x1BF45, 0x1BF46)], 25: [(0x1A757, 0x1A787)], 26: [(0x50F75, 0x50F8A), (0x62356, 0x62360)], 28: [(0x11358, 0x113EA)], 29: [(0x12D55, 0x12DD3), (0x1BCDF, 0x1BCE0), (0x1BCEB, 0x1BCF7), (0x130C7, 0x130D0), (0x13103, 0x13155)], 30: [(0x35225, 0x352F6), (0x35517, 0x35521)], 32: [(0x5AD6A, 0x5AD99)], 33: [(0x35590, 0x355C6)], 34: [(0x351A7, 0x351AC), (0x351D5, 0x351F9), (0x18EC4, 0x18F08)], 35: [(0x2B52F, 0x2B53D), (0x2B54D, 0x2B54F), (0x2B562, 0x2B564), (0x2B5B7, 0x2B5C0), (0x2B5C9, 0x2B5CE)], 36: [(0x6D476, 0x6D48D), (0x6D4DE, 0x6D4E0)], 37: [(0x59A77, 0x59A78), (0x59A94, 0x59A95)], 38: [(0x12F23, 0x12F2B), (0x12F6B, 0x12F72), (0x2AC69, 0x2ACA6)], 39: [(0x1F707, 0x1F70D)], 41: [(0x44616, 0x44620), (0x62344, 0x62356)], 43: [(0x32D90, 0x32D95), (0x388BE, 0x388C3), (0x38DF9, 0x38DFE), (0x38FA9, 0x38FAE), (0x39031, 0x39037), (0x566F7, 0x566FC), (0x5671D, 0x56723), (0x56E05, 0x56E0A), (0x58C50, 0x58C55), (0x59187, 0x5918C), (0x6DB37, 0x6DB3C), (0x6EBB1, 0x6EBB6), (0x72151, 0x72156), (0x72181, 0x72186), (0x721B6, 0x721BB), (0x722AE, 0x722B3), (0x722DE, 0x722E4), (0x7231D, 0x72322), (0x7246E, 0x72473), (0x7249C, 0x724A1), (0x725FB, 0x72600)], 44: [(0x2F50D, 0x2F52C), (0x2F580, 0x2F588)], 47: [(0x40B52, 0x40B5A), (0x2C570, 0x2C5A7)], 48: [(0x1A057, 0x1A05D), (0x48249, 0x48267)], 49: [(0x3CB2D, 0x3CB36), (0x3CB6F, 0x3CB71), (0x3CBB3, 0x3CBBF), (0x3CC06, 0x3CC0E), (0x3CC2C, 0x3CC38), (0x3FE43, 0x3FE50)], 50: [(0x3FE2E, 0x3FE33), (0x488A6, 0x488FB)], 51: [(0x173E5, 0x173EA), (0x19D6C, 0x19D71), (0x1AE80, 0x1AE85), (0x1AF42, 0x1AF47), (0x5A855, 0x5A85E)], 52: [(0x3FE43, 0x3FE50), (0x57060, 0x5706B), (0x5711E, 0x57133), (0x572FA, 0x57303), (0x574E9, 0x574F2), (0x57562, 0x57566)], 53: [(0x50EF6, 0x50EFC)], 54: [(0x3D412, 0x3D418), (0x3D4B5, 0x3D4D5)], 56: [(0x3C328, 0x3C32D), (0x42770, 0x42780), (0x3C346, 0x3C34C), (0x1B6D6, 0x1B6F3), (0x3D5B6, 0x3D5BC), (0x3D5CF, 0x3D5D4), (0x3D609, 0x3D619), (0x3CEC0, 0x3CEC7), (0x3CF21, 0x3CF27), (0x3CF5A, 0x3CF6A)], 55: [(0x3D2B6, 0x3D2E1), (0x3D436, 0x3D43B), (0x2118B, 0x21198), (0x3D4F4, 0x3D50A)], 57: [(0x59B61, 0x59B67), (0x59B9E, 0x59C01)], 59: [(0x35225, 0x352F6)], 58: [(0x58C6C, 0x58CBF), (0x58CCE, 0x58CD9), (0x58DDC, 0x58DDE), (0x71C68, 0x71C73)], 68: [(0x39B31, 0x39B51), (0x39B83, 0x39B89), (0x38427, 0x3843C), (0x174E0, 0x174E5), (0x54CB1, 0x54D07)], 69: [(0x1763A, 0x17643), (0x24019, 0x24022)], 70: [(0x1BA9F, 0x1BABE), (0x1BB0C, 0x1BB19)], 61: [(0x12A17, 0x12A1C), (0x13033, 0x13038), (0x14C75, 0x14C7A), (0x1A650, 0x1A655), (0x1BFBF, 0x1BFC4)]}   # 8..11 TakeDamage, 12 EndTask, 13 PlaceAt, 14 WalkEnd (StopWalk -1), 15 EnemiesTurn? bled slide (cave 0x18E98), 17 down opponents seen (SightCheck, MarkSeen, 0x7351C), 18 fuses by tick step + time compression off (TacticalTimer?, TacticalLoop fuses, EndTurn?, helper 0x3D4D5), 19 New turn stage needs Santino spoke = 2 (NewTurn? 0x12B50), 20 New turn redraw_next before step 8 (NewTurn?; row 87 entry calls left out)   # #70 TakeDamage
FAN = None                         # --fan IDS (with --port, --only take_damage / end_task / swap_places / walk_end): F08..F14 bytes of 1.15m (Emu.put_115m)
# #71 port fixes: --pfix IDS (with --port) = the port's Pnn mask bits on, and the ORIGINAL gets the same fix as an asm
# patch (validate/diff/pfix.py). Without --pfix the port must keep the bug (level off / 1.15m).
import pfix
PFIX = pfix.parse(argv)
if "--fan" in argv:
    v_ = argv[argv.index("--fan") + 1]
    del argv[argv.index("--fan"):argv.index("--fan") + 2]
    FAN = sorted(FAN_RUNS) if v_ == "all" else [int(x_.strip().lstrip("Ff")) for x_ in v_.split(",")]
    ai.FAN_FIXES = set(FAN)   # (the mirror's MercView: F61)
    if "--port" not in argv or not ONLY or not ONLY <= {"view", "w_59ae8", "move_ap_cost", "mine_spotted", "pickup_here", "use_on_tile", "impact", "land_item", "bury_mine", "put_item", "effect_hit", "effect_pass", "pickup", "find_path", "path_ap", "walk_step", "w_59a54", "think", "take_damage", "end_task", "swap_places", "walk_end", "turn_end", "sight_check", "mark_seen", "guard_turned", "fuses", "ticks", "turn_new", "end_of_day", "leave_sector", "anim_step", "active_frame", "ledger", "leave_path", "turn_flow", "anim_frame", "melee_hit", "attack_go", "explode_dat", "merc_entry", "start_turn", "pay", "throw_option", "gun_option", "knife_option", "hit_location", "can_stand_at", "engaged_action", "calm_action", "w_32d04", "turn_rest", "w_56dd4", "launch", "blast_tile", "sector_start", "trigger"}:
        raise SystemExit("ai_diff: --fan needs --port --only take_damage / end_task / swap_places / walk_end / turn_end / sight_check / mark_seen / guard_turned")
PORT = "--port" in argv            # rebuild + the AI deciders in port_lib.AI_ENTRY: compare the clean-room port (validate/diff/port_lib.py) instead of tools/ai.py
if PORT:
    argv.remove("--port")
    import port_lib                # noqa: E402
    for o_ in ONLY:
        if o_ not in port_lib.AI_ENTRY and o_ not in PORT_SECTIONS:
            raise SystemExit(f"--port: the port has no {o_}")


GROUP = None                       # --group K/N: only the sections of group K (0..N-1), for parallel check jobs
if "--group" in argv:
    GROUP = tuple(int(x) for x in argv[argv.index("--group") + 1].split("/"))
    del argv[argv.index("--group"):argv.index("--group") + 2]
# seconds per section (deciders by name) in one full run (AI_DIFF_TIMES=1 prints them): --group spreads the sections
# over the groups by these (longest first, each to the group with the least time; 2026-10-25, --port or the mirror,
# the longer); a section not listed counts 0.5 s
TIMES = {"ledger": 62.0, "pickup": 60.8, "sector_start": 34.8, "tmp_file": 28.7, "explode_dat": 21.2, "move": 17.1,
         "effect_hit": 16.2, "supplies": 14.4, "view": 10.8, "pay": 9.4, "ground_blast": 9.3, "turn_end": 9.1,
         "radio_result": 9.0, "patches": 8.7, "loop": 8.3, "ticks": 7.8, "forget": 7.6, "guard_leaves": 7.5,
         "sector_tick": 5.9, "turn_start": 5.8, "connected": 5.7, "sector_value": 5.5, "make_noise": 5.3,
         "doors": 5.0, "turn_new": 4.7, "turn_flow": 4.7, "put_item": 4.5, "select_merc": 4.4, "blast_tile": 4.4,
         "door_fx": 4.3, "merc_entry": 4.1, "disarm": 4.0, "throw_option": 3.6, "equip": 3.1, "end_of_day": 3.1,
         "walk_in": 3.1, "explode": 3.0, "hide_path_marks": 3.0, "loop_head": 2.9, "path_ap": 2.9,
         "init_soldier": 2.8, "escape_stop": 2.8, "find_path": 2.8, "blast": 2.6, "action_ap": 2.6, "shot_ap": 2.6,
         "leave_sector": 2.5, "loop_rest": 2.3, "fuses": 2.2, "line_of_fire": 2.0, "gas_tick": 1.9,
         "slot_reset": 1.9, "quest_check": 1.9, "reload": 1.8, "retreat_tile": 1.7, "sight_line": 1.7,
         "search_tile": 1.5, "find_dry_tile": 1.5, "gun_option": 0.9, "leave_tests": 0.9, "age": 0.9,
         "knife_option": 0.8, "noise_tile": 0.7, "rebuild": 0.7, "story": 0.7, "advance_tile": 0.6,
         "ally_call_tile": 0.6, "buddy_tile": 0.6, "news_value": 0.6, "target_tile": 0.6, "opp_strength": 0.6,
         "crowbar": 0.6, "workable": 0.6, "soldier_power": 0.5, "active_frame": 0.5, "combat_action": 0.5,
         "enemy_turned": 0.5, "leave_path": 0.5, "share_sightings": 0.5, "interrupt_check": 0.5,
         "merc_interrupt": 0.5, "hit_alert": 0.5, "update_sightings": 0.5, "stage": 0.5, "guard_turned": 0.4,
         "mark_seen": 0.4, "sight_check": 0.4, "enemy_interrupt": 0.4, "soldier_frame": 0.4, "patrol_tile": 0.4,
         "approach_tile": 0.4, "walk_step": 0.4, "think": 0.4, "engaged_action": 0.4, "do_action": 0.3,
         "anim_frame": 0.3, "can_stand_at": 0.3, "action_affordable": 0.3, "calm_action": 0.3, "alert_action": 0.3,
         "choose_action": 0.3, "update_alert": 0.3, "use_throw": 0.3, "throw_chance": 0.3, "no_ally_sees": 0.3,
         "tile_standable": 0.3, "pstep": 0.2, "xra_enemy": 0.2, "merc_dies": 0.2, "take_damage": 0.2,
         "enemies_see": 0.2, "kill": 0.2, "consume_trigger": 0.2, "click": 0.2, "mercs_see": 0.2, "w_59a54": 0.2,
         "hears": 0.2, "start_turn": 0.2, "impact": 0.2, "frame_checks": 0.2, "walk_end": 0.2, "end_task": 0.2,
         "set_anim": 0.2, "spend_ap": 0.2, "anim_step": 0.2, "launch": 0.2, "stop_anim": 0.2, "fire_gun": 0.2,
         "turn_to": 0.2, "noise_int": 0.2, "first_aid": 0.2, "throw_aim": 0.2, "anim_end": 0.2, "w_59ae8": 0.2,
         "replace": 0.2, "walk_blocked": 0.2, "throw_follow": 0.2, "throw_scatter": 0.2, "hit_reaction": 0.2,
         "reach_tile": 0.2, "w_56dd4": 0.2, "aid_tick": 0.2, "attack_go": 0.2, "chance_to_hit": 0.2,
         "step_noise": 0.2, "anim_turn": 0.2, "shot_alert": 0.2, "drown": 0.2, "merc_quote": 0.2, "walk_to": 0.2,
         "bleed": 0.2, "anim_delay": 0.2, "gun_jam": 0.2, "stand_up": 0.2, "fire": 0.2, "turn_rest": 0.2,
         "rest": 0.2, "start_attack": 0.2, "idle": 0.2, "swap_hand": 0.2, "move_ap_cost": 0.2, "reserve_ap": 0.2,
         "attack_ready": 0.2, "busy": 0.2, "tile_distance": 0.2, "bury_mine": 0.1, "use_on_tile": 0.1,
         "pickup_here": 0.1, "melee_hit": 0.1, "apply_aid": 0.1, "land_item": 0.1, "revive_roll": 0.1,
         "trigger": 0.1, "msg_not_enough_ap": 0.1, "stat_practice": 0.1, "nearest_merc": 0.1, "w_5aa08": 0.1,
         "w_197d8": 0.1, "stop_walk": 0.1}


def group_of(key):
    if GROUP is None:
        return None
    if not GROUP_MAP:
        load = [0.0] * GROUP[1]
        for k_ in sorted((set(TIMES) | set(DECIDERS)) - ALONE, key=lambda k_: (-TIMES.get(k_, 0.5), k_)):
            g_ = min(range(GROUP[1]), key=lambda j_: (load[j_], j_))
            GROUP_MAP[k_] = g_
            load[g_] += TIMES.get(k_, 0.5)
    key = GROUP_WITH.get(key, key)
    return GROUP_MAP[key] if key in GROUP_MAP else sum(key.encode()) % GROUP[1]   # (a new section: by its name)


GROUP_WITH = {"stragglers": "forget"}   # cases of one shared loop: the section runs in the group of its partner
ALONE = {"lock_kit", "mine_flag", "walter"}    # sections that run only with --only (own check line): out of the default run and the group map


GROUP_MAP = {}
CLOCK = [None, time.monotonic()]     # (running section, its start)
CLOCK_STUB = [None]
SPENT = collections.Counter()


def clock(name):
    """a section (or a decider case) starts: the one before it ends"""
    t_ = time.monotonic()
    if os.environ.get("AI_DIFF_STUB"):    # debug: which section takes a stub away (or puts it in)
        h_ = rt(int(os.environ["AI_DIFF_STUB"], 16)) in e.stubs
        if h_ != CLOCK_STUB[0]:
            print("STUB", os.environ["AI_DIFF_STUB"], "stubbed" if h_ else "real", "before", name, "after", CLOCK[0])
            CLOCK_STUB[0] = h_
    if CLOCK[0] is not None:
        SPENT[CLOCK[0]] += t_ - CLOCK[1]
    CLOCK[:] = [name, t_]


def direct(*keys):
    """a directly called section runs (--port: only those the port covers; --group: only the group's)"""
    ok = any(k in ONLY for k in keys) if only else (not PORT or any(k in PORT_SECTIONS for k in keys)) and \
        (GROUP is None or any(group_of(k) == GROUP[0] for k in keys))   # (an outer block: a member section below tests direct again)
    if ok:
        clock(keys[0])
        if BASE_MEM:                     # (issue #12) each section starts from the same guest memory, whatever ran before
            lo_, hi_ = rt(0x10000), rt(0x8AC7C) + 1
            e.uc.mem_write(0, BASE_MEM[0][:lo_])
            e.uc.mem_write(hi_, BASE_MEM[0][hi_:])
    return ok


BASE_MEM = []                            # guest memory (dump) as it was before the first case, set below
n = int(([a for a in argv if a.isdigit()] or ["12000"])[0])
paths = [a for a in argv if not a.isdigit()]
dump = paths[0] if paths else os.path.join(HERE, "..", "..", "build", "mem_tac60.bin")
rng = random.Random(int(os.environ.get("AI_DIFF_SEED", "6")))
e = Emu(dump=dump, fast=True)
rt = e.rt
if FAN is not None:                # original: only these fixes' 1.15m bytes; port: only their mask bits
    e.put_115m(only=[r_ for f_ in FAN for r_ in FAN_RUNS[f_]])
    if 17 in FAN:                  # 1.15m 0x72FCF F6 8F = TEST r/m8, imm8 in its undocumented /1 form (real x86 and
        e.uc.mem_write(rt(0x72FD0), b"\x87")   # DOSBox run it as /0); Unicorn refuses /1: the same insn as /0
    if 23 in FAN:                  # 1.15m AnimFrame 0x192A4 F6 C8 01 = TEST al, 1 in the /1 form: Unicorn needs /0
        e.uc.mem_write(rt(0x192A5), b"\xC0")
    port_lib.fan_fixes(sum(1 << (f_ - 1) for f_ in FAN))
    if 52 in FAN:                  # F52: 1.15m LandItem? returns PutItem's tile (0x3FE43 EDI): compare EAX
        VOID.discard("land_item")
        MASK["land_item"] = 0xFFFF
        port_lib.VOID_ENTRY = tuple(v_ for v_ in port_lib.VOID_ENTRY if v_ != "land_item")
if PFIX is not None:               # original: the asm form of each port fix; port: its P mask bits
    pfix.apply(e, PFIX, port_lib)
KIND = {}
SP = {rt(G_SOLDIERS) + 0x200 * k: k for k in range(0x30)}   # emulator soldier address -> slot
log, script = [], {}


def take(name):
    q = script.get(name)
    return q.pop(0) if q else 0


UOT_RNG = random.Random(4242)
UOG_RNG = random.Random(4343)   # use_on_tile: which r <= 0 prompt answers also clear the tile's item byte


def uot_port_script():
    """--port use_on_tile: the prompt (op 248) and mine check (op 247) answers. (This script is made before the case
    setup, which then draws cur_sector: a filter on cur_sector here would read the value before the case.)"""
    d = [UOT_RNG.choice([0, 1, 1, 0xFFFFFFFF, 2]) for _ in range(2)]    # own rng: the shared one stays as in the mirror run
    d = [(r_, 1) if r_ in (0, 0xFFFFFFFF) and UOG_RNG.random() < 0.5 else r_ for r_ in d]   # (r, 1): the prompt
    # took the item off the tile (byte 6 = 0xFF), as a set-off or a pick-up inside DisarmExplosivePrompt does: the
    # r <= 0 path then finds no item on the tile (UseOnTile? 0x3D5FA TileItemIndex -> 0xFF, jge 0x3D62B)
    m = UOT_RNG.choice([0, 0, 1])
    if FAN and 56 in FAN and F56_RNG.random() < 0.4:
        m = 0xFF                         # F56: MineSpotted? -1 (an effect on the own tile)
    return {"disarm": d, "mine_spotted": [m]}


PUH_RNG = random.Random(4343)
LK_RNG = random.Random(4545)    # lock_kit: own stream (the other sections keep their cases)
MF_RNG = random.Random(4646)    # mine_flag: own stream
WL_RNG = random.Random(4747)    # walter: own stream
F33_RNG = random.Random(3333)   # melee_hit: the queued action +0x1A6 (own stream: the old cases stay)
F59_RNG = random.Random(5959)   # melee_hit under --fan 59: an off-map target tile (own stream)
F61_RNG = random.Random(6161)   # active_frame --fan 61: AP under the reserve, step within the AP
F24_RNG = random.Random(2424)   # soldier_frame --fan 24: AP under the reserve
SFC_RNG = random.Random(4949)   # soldier_frame not-done-with-target refusal (u_move_to step 2): own stream
TIE_RNG = random.Random(5050)   # engaged_action equal option scores: own stream
F56_RNG = random.Random(5656)   # mine_spotted / use_on_tile / pickup_here --fan 56
F55_RNG = random.Random(5555)   # use_on_tile --fan 55: the tile in the blast list
F68_RNG = random.Random(6868)   # --fan 68: broken bridge pieces (25300 on water) and their 25400 neighbours


def f68_bridges(centre, n=None):
    """--fan 68 (#70 row 71): some tiles near `centre` become code 25300 on a water ground byte (t0 >= 0x6C) or not,
    with t1 0x1C / 0x1F (not water) or another object; some neighbours get the bridge deck code 25400 (own rng)."""
    if not (FAN and 68 in FAN):
        return
    for _ in range(F68_RNG.choice([0, 3, 12]) if n is None else n):
        t_ = min(3135, max(0, centre + F68_RNG.choice([0, 1, -1, 56, -56, 57, F68_RNG.randrange(-120, 120)])))
        e.w16(rt(ai.G_MOVECODE) + 2 * t_, F68_RNG.choice([25300, 25300, 25300, 25400, 1000]))
        e.w8(rt(G_TILEMAP) + 8 * t_, F68_RNG.choice([0x6B, 0x6C, 0x80, 0xFF, 0x10]))
        e.w8(rt(G_TILEMAP) + 8 * t_ + 1, F68_RNG.choice([0x1C, 0x1F, 0, 0x21, 0x26, F68_RNG.randrange(256)]))


UOL_RNG = random.Random(4848)   # use_on_tile lock values + crowbar: own stream (the other sections keep their cases)


def puh_port_script():
    """--port pickup_here: the prompt (op 248) and mine check (op 247) answers (own rng, as uot_port_script)."""
    m = PUH_RNG.choice([0, 0, 1])
    if FAN and 56 in FAN and F56_RNG.random() < 0.4:
        m = 0xFF                         # F56: MineSpotted? -1 (an effect on the own tile)
    return {"disarm": [PUH_RNG.choice([0, 1, 1, 0xFFFFFFFF, 2])], "mine_spotted": [m]}


def conv(kinds, args):
    out = []
    for k, a in zip(kinds, args):
        if k == "x":                     # not compared (a table address)
            continue
        if k == "m":                     # message text: the port passes the string, the original a pointer
            b = b"" if isinstance(a, str) else bytes(e.uc.mem_read(a, 256))
            out.append(a if isinstance(a, str) else b[:b.index(0)].decode("latin-1"))
            continue
        if k == "p":                     # puff look: the original passes its table (shooting.md: 0 splash, 1 puff)
            out.append({rt(0x95670): 0, rt(0x9566B): 1, rt(0x22A4D0): 2, rt(0x229E18): 3}.get(a, a))   # (2 / 3: combat.md Explode)
            continue
        out.append(SP.get(a, a) if k == "s" else ai.s16(a) if k == "t" else a & 0xFF if k == "b" else
                   ai._i32(a) if k == "n" else a)
    return tuple(out)


def make_stub(name, kinds):
    def fn(emu, eax, edx, ebx, ecx):
        sp = emu.uc.reg_read(UC_X86_REG_ESP)
        stack = [emu.r32(sp + 4 + 4 * i) for i in range(max(len(kinds) - 4, 0))]
        if name == "hit_test":              # stack: dir, skip*, chance*, cap, target, roll
            stack = [emu.r32(sp + 4 + 4 * i) for i in range(6)]
            v = take(name) or (0, 0, 100)
            log.append((name, conv(kinds, [eax, edx, ebx, ecx, stack[0], emu.r8(stack[1])] + stack[3:] + [emu.r32(stack[2])])))   # last: chance on entry
            emu.w8(stack[1], v[1])
            emu.w32(stack[2], v[2])
            return v[0]
        log.append((name, conv(kinds, [eax, edx, ebx, ecx] + stack)))
        v = take(name)
        if name == "find_path" and isinstance(v, tuple):   # (found, pathLen): FindPath sets +0xD4
            emu.w16(eax + 0xD4, v[1])
            return v[0]
        if name == "disarm" and isinstance(v, tuple):      # (r, 1): the prompt took the item off the tile
            emu.w8(rt(G_TILEMAP) + 8 * (edx & 0xFFFF) + 6, 0xFF)
            return v[0]
        if name == "noise_tile" and isinstance(v, tuple):   # writes the loudness through EDX (unscripted: nothing)
            emu.w32(edx, v[1])
            return v[0]
        if name in RECORDS:
            emu.uc.mem_write(edx, v)
            return 0
        return v
    return fn


def stub_random(emu, eax, *_):
    k = ai.s16(eax)
    log.append(("Random", (k,)))
    v = take("Random")
    return v % k if k > 0 else 0


e.stub(rt(RANDOM), stub_random)
PATHCMP = {"find_path", "path_ap"}     # deciders that store a path (G_PATHS compared)
MARKCMP = {"path_ap"}                  # path preview: marks flags, count, list and tile byte 7 compared (restored between runs)
MARK_REGS = [(ai.G_MARKS_ON, 4), (ai.G_MARKS_SHOWN, 4), (ai.G_MARKS_N, 4), (ai.G_MARKS, 0x100), (G_TILEMAP, 8 * 3136)]


MARK_PRE = [None]


def marks_read():
    b = [bytes(e.uc.mem_read(rt(a), k)) for a, k in MARK_REGS]
    return b[:-1] + [b[-1][7::8]]
CASE_REAL = {"path_ap": ("find_path",), "walter": ("w_59a54",), **({"start_turn": ("w_1f6e4",), "spend_ap": ("rest",),
                                            "launch": ("w_56dd4", "impact", "use_throw", "drop_item"),   # (shooting.md: library
                                            "pstep": ("w_56dd4", "impact"),
                                            "take_damage": ("rest", "w_74304", "react", "end_task", "w_197d8"),   # (shooting.md batch 2:
                                            "hit_reaction": ("react",), "melee_hit": ("react",),   # (batch 6: Hit alert on a missed sure blow)
                                            "aid_tick": ("drop_item",), "end_task": ("w_197d8",),   # (combat.md First aid: Use charges clears the entry)
                                            "start_attack": ("w_5aa08", "w_56dd4"),   # (batch 5: Aim posture, Shot end)
                                            "click": ("w_5aa08", "w_59a54", "path_ap", "reloaded", "first_aid", "w_59ae8"), "use_on_tile": ("door_lookup",), "bury_mine": ("put_item", "drop_item"), "land_item": ("put_item", "drop_item", "w_3fc04"),   # (combat.md Crowbar: the library reads the door table)
                                              # (click.md: Aim posture, Jam check; Move order runs Path AP; Reload)
                                            "kill": ("rest", "put_item", "drop_item", "w_3fc04", "w_3a2d4")} if PORT else {})}   # library procedures)             # procedures)   # --port: the library has Merc busy (start_turn) and runs Rest inside Spend AP
PORT_LIB = {"path_ap": 1 << 41, "click": 1 << 63 | 1 << 40, "take_damage": 1 << 50}       # --port: decider -> host_ops bits cleared (the library runs those ops itself, as CASE_REAL)     # decider -> callees that run for real in its cases only (the others stub them)
CASE_STUBS = {"anim_next": {"walk_step", "soldier_frame", "active_frame"}, "merc_quote": set(),
              **{k_: set() for k_ in ("frame_active", "frame_other", "itq_pop", "w_12dd4", "w_12404", "new_turn",
                                      "projectile_step", "move_merc", "w_11fb4", "w_67bf0", "w_681d0", "w_67e18")},     # stubbed only in the "loop" section
              "disarm": {"use_on_tile", "pickup_here"} if PORT else set(), "mine_spotted": {"use_on_tile", "pickup_here"} if PORT else set(),   # --port: leaves (host ops 248 / 247)
              "consume_trigger": {"active_frame"} if PORT else set(), "w_735bc": {"active_frame"} if PORT else set(), "throw_item": {"active_frame"} if PORT else set(), "ai_think": {"active_frame"}, **{k_: set() for k_ in ("w_6a960", "guards_arrive", "w_1583c", "gas_tick", "mouse_in_rect", "w_673ac", "w_124fc", "w_1259c", "w_410c4", "w_297d4", "stage_five", "w_31550")},   # stubbed only in the "turn_new" section
              "w_11448": {"guard_turned"}, "w_1aaf0": {"active_frame"}, "w_5afbc": {"active_frame"} if PORT else set()}     # callee -> the only deciders that stub it (the others run it for real)
for name, (addr, kinds) in CALLEES.items():
    if name not in CASE_STUBS:
        e.stub(rt(addr), make_stub(name, kinds), POP.get(name, 0))
e.stub_sprintf()
if FAN and 32 in FAN:              # F32: 1.15m AttackGo? 0x5AD94 calls the anim test 0x5AD1C (1.13b inlines it): real
    e.stubs.pop(rt(0x5AD1C), None)


def stub_walter_say(emu, *_):
    """0x3C985..0x3CACB of PlayWalterQuote? (the clip, the caption, the wait, the redraw): logged as walter_say (q =
    ESI), then on at 0x3CACC (the stub's return address)"""
    q = emu.uc.reg_read(UC_X86_REG_ESI)
    log.append(("walter_say", (ai._i32(q),)))
    sp_ = emu.uc.reg_read(UC_X86_REG_ESP) - 4
    emu.uc.reg_write(UC_X86_REG_ESP, sp_)
    emu.w32(sp_, rt(0x3CACC))
    return 0


e.stub(rt(0x3C985), stub_walter_say)


class MemRec:
    """a soldier record read and written straight in emulator memory (MEMRUN tests)"""
    def __init__(self, slot):
        self.slot, self.base = slot, rt(G_SOLDIERS) + 0x200 * slot

    def __len__(self):
        return 0x200

    def __getitem__(self, i):
        if isinstance(i, slice):
            a, b, _ = i.indices(0x200)
            return bytes(e.uc.mem_read(self.base + a, max(b - a, 0)))
        return e.r8(self.base + i)

    def __setitem__(self, i, v):
        if isinstance(i, slice):
            e.uc.mem_write(self.base + i.indices(0x200)[0], bytes(v))
        else:
            e.w8(self.base + i, v & 0xFF)


class World:
    memrun = False

    def __init__(self, slot):
        self.slot = slot

    def g8(self, a):
        return e.r8(rt(a))

    def tile8(self, tile, k):
        return e.r8(rt(G_TILEMAP) + 8 * ai.s16(tile) + k)

    def random(self, k):
        k = ai.s16(k)
        log.append(("Random", (k,)))
        v = take("Random")
        return v % k if k > 0 else 0

    def g16(self, a):
        return e.r16(rt(a))

    def g32(self, a):
        return e.r32(rt(a))

    def s8(self, a, v):
        e.w8(rt(a), v)

    def ptr(self, a):
        return rt(a)

    def raw16(self, p):
        return e.r16(p & 0xFFFFFFFF)

    def raw8(self, p):
        return e.r8(p & 0xFFFFFFFF)

    def raw_s8(self, p, v):
        e.w8(p & 0xFFFFFFFF, v & 0xFF)

    def s16(self, a, v):
        e.w16(rt(a), v)

    def s32(self, a, v):
        e.w32(rt(a), v)

    def rec(self, k):
        if self.memrun:
            return MemRec(k)
        return bytes(e.uc.mem_read(rt(G_SOLDIERS) + 0x200 * k, 0x200))

    def ext(self, name, *args):
        kinds = CALLEES[name][1]
        if name == "hit_test":
            log.append((name, tuple(args[:10])))
            return take(name) or (0, 0, 100)
        soldier = args[0] if args else None
        args = [a.slot if isinstance(a, MemRec) else self.slot if k == "s" and not isinstance(a, int) else a
                for k, a in zip(kinds, args)]
        log.append((name, conv(kinds.replace("s", "i"), args)))
        v = take(name)
        if name == "find_path" and isinstance(v, tuple):
            ai.w16(soldier, 0xD4, v[1])
            return v[0]
        return v


def fuzz_soldier(s):
    s[0xBA] = rng.choice([0, 1, 2, 3, 4, 5, rng.randrange(256)])
    s[0xBB] = rng.choice([0, 1, 2, 3, 4, rng.randrange(256)])
    s[0x131] = rng.choice([0, 15, 30, rng.randrange(256)])
    s[0x1D3] = rng.choice(list(range(7)) + [rng.randrange(256)])
    s[0x1D4] = rng.choice(list(range(6)) + [rng.randrange(256)])
    s[0x1D5] = rng.choice([0, 0, 1, 0xFF, rng.randrange(256)])
    s[0x1D6] = rng.choice([0, 0, 1])
    s[0x1D7] = rng.choice([0, 0, 1])
    s[0x1AB] = rng.choice([0, 0, 1])
    s[0x1C4] = rng.choice([0, 0, 1])
    s[0x18A] = rng.choice([100, 80, 75, 74, 50, 49, 20, rng.randrange(256)])
    s[0x183] = rng.randrange(0, 101)
    s[0x184] = rng.choice([s[0x183], 100, rng.randrange(256)])
    s[0x18F] = rng.choice([99, 50, rng.randrange(256)])
    s[0x1A1] = rng.choice(list(range(1, 9)) + [0, rng.randrange(256)])
    s[0x1CF] = rng.choice(list(range(0, 10)) + [rng.randrange(256)])
    s[0x1B2] = rng.choice([0x6C, 0x6B, 0x80, 0x10, rng.randrange(256)])
    s[0x1B4] = rng.choice([0, 0x1C, 0x1F, rng.randrange(256)])
    s[0x24] = rng.choice([0, 0x28, rng.randrange(136)])
    hand = lambda: rng.choice([0x40, rng.randrange(136)] + [rng.choice(KIND.get(k, [0])) for k in (0x92, 0x95, 0x96)])
    s[0x00] = rng.choice([s[0x00], hand(), hand()])
    s[0x0C] = rng.choice([s[0x0C], hand(), hand()])
    s[0x9C] = rng.randrange(256)
    ai.w16(s, 0xD0, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
    ai.w16(s, 0x12E, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
    ai.w16(s, 0x132, rng.choice([rng.randrange(0x10000), rng.randrange(3136), rng.randrange(1, 9), ai.NO_TILE]))
    s[0x1D1] = rng.choice(list(range(18)) + [rng.randrange(256)] + [0] * 6)
    s[0x17A] = slot
    s[0xBA] = rng.choice([s[0xBA], 3, 3, 4, 2, 1])
    s[0x182], s[0x138] = rng.choice([1, 1, 1, 0]), rng.choice([1, 1, 1, 0])
    s[0x183] = rng.choice([s[0x183], 15, 14, 50])
    s[0x1F3] = rng.randrange(5)
    for k in range(5):
        ai.w16(s, 0x1E8 + 2 * k, rng.choice([0, 3, 4, 0x15, 0x1B, rng.randrange(0x60)]))
    s[0x197] = rng.choice([0, 0, 0, 1])
    s[0x1C1], s[0x1B8], s[0x1D6] = rng.choice([0, 1]), rng.choice([0, 1, 1]), rng.choice([0, 1])
    s[0x1B0] = rng.randrange(256)
    ai.w16(s, 0xF0, rng.choice([0, 1, 348, 349, 350, rng.randrange(0x10000)]))
    ai.w16(s, 0x120, rng.choice([ai.NO_TILE, ai.u16(s, 0x124), rng.randrange(3136)]))
    s[0x1A2] = rng.choice([s[0x1A1], s[0x132], rng.randrange(256)])
    ai.w16(s, 0x124, rng.choice([ai.u16(s, 0x132), rng.randrange(3136)]))
    s[0x1A8] = rng.randrange(256)
    s[0x1A5] = rng.choice([0, 4, 5, 10, 20, rng.randrange(256)])
    s[0xBA] = rng.choice([s[0xBA], 3])


def record():
    b = bytearray(rng.randrange(256) for _ in range(10))
    b[0] = rng.choice([0, 1, 1, b[0]])
    b[1] = rng.choice([rng.randrange(16), rng.randrange(0x28, 0x30)])
    b[3] = rng.choice([5, 10, 24, 25, 49, 50, 74, 75, 99, b[3]])
    if rng.random() < 0.5:
        b[4:8] = rng.randrange(-200, 200).to_bytes(4, "little", signed=True)
    return bytes(b)


def IMPACT_OPS():
    """--port: Launch and Projectile step run Projectile impact for real (shooting.md: a library procedure), so its
    host ops need answers on the original side too (without --port impact is a stub and these stay unused)"""
    return dict(bullet_damage=[rng.choice([1, 20, 31, 50, 60, 120, 130, 0x1FFFF])] * 2,
                door_lookup=[rng.choice([0, 50, 101, 200])], puff=[rng.choice([0, 3, 0x80])] * 2)


F39_RNG = random.Random(3939)   # start_turn --fan 39: duty bytes in the no-order cases
F58_RNG = random.Random(5858)   # --fan 58: throw targets on the "past reach + 16" edge


def f58_edge(s, slot, script):   # script = the case's base lists (both sides start from them)
    """--fan 58 (#70 row 37): one seen opponent in play; the thrower stands on its row, left of it, so that the
    nearest candidate tile (2 left of the opponent) is reach + 15..18 px away (ThrowChance distance, no goggles)."""
    from combat import distance
    opps = list(range(0x28, 0x30)) if slot < 0x28 else list(range(0x28))
    j = F58_RNG.randrange(len(opps))
    for j_ in range(len(opps)):
        s[0x13A + j_] = 1 if j_ == j else F58_RNG.choice([0, 0, 0, 2])
    if s[0x24] == 0x27:
        s[0x24] = 0
    h, en = ai.s8(s[0x183]), ai.s8(s[0x18A])
    reach = e.r8(rt(ai.G_ITEM_RANGE + 12 * s[0])) * 15 + h - 50
    reach = max(reach - ai.cdiv((100 - en) * reach, 200), 0)
    dd = reach + F58_RNG.choice([15, 16, 16, 17, 17, 18])
    dx = next((x for x in range(0, 900) if distance(0, 0, x, 0) == dd), None)
    row = F58_RNG.randrange(56)
    cols = [c for c in range(2, 56) if ai.tile_x(row * 56 + c - 2) - dx >= 0] if dx is not None else []
    if not cols:
        return
    ot = row * 56 + F58_RNG.choice(cols)
    a = rt(G_SOLDIERS) + 0x200 * opps[j]
    e.w16(a + 0x114, ot)
    for f_ in (0x182, 0x138):
        e.w8(a + f_, F58_RNG.choice([1, 1, 3]))
    e.w8(a + 0x183, F58_RNG.choice([1, 20, 60]))
    ai.w16(s, 0x116, ai.tile_x(ot - 2) - dx)
    ai.w16(s, 0x118, ai.tile_y(ot))
    allies = range(0x28, 0x30) if slot >= 0x28 else range(0x28)
    for k in allies:                  # the thrower is the only ally in play: TileDist runs ally, opponent per tile
        if k != slot:
            e.w8(rt(G_SOLDIERS) + 0x200 * k + 0x183, 0)
    script["action_ap"] = [0] * 400
    script["fire_line"] = [F58_RNG.choice([100, 100, 60])] * 400
    script["tile_dist"] = [5, 0] * 3000


def scripted(name):
    tiles = lambda: [rng.choice([ai.NO_TILE, rng.randrange(3136), rng.randrange(0x10000)]) for _ in range(4)]
    small = lambda: [rng.choice([0, 1, 5, 14, 15, 30, rng.randrange(0x10000)]) for _ in range(4)]
    edge = lambda: rng.choice([rng.randrange(0x7FFF), 5 * rng.randrange(21), 5 * rng.randrange(1, 21) - 1])
    if name == "xra_enemy":
        return dict(scripted("calm_action"), Random=[rng.randrange(0x7FFF) for _ in range(4)])
    if name == "throw_option":
        return dict(scripted("calm_action"),
                    action_ap=[rng.choice([0, 2, 4, 8, 20, 0x105, rng.randrange(256)]) for _ in range(400)],
                    fire_line=[rng.choice([0, 10, 50, 100, 255, rng.randrange(1 << 32)]) for _ in range(400)],
                    tile_dist=[rng.choice([0, 1, 2, 3, 4, 5, 6, 0x103, rng.randrange(1 << 32)]) for _ in range(6000)])
    if name == "knife_option":
        return dict(scripted("calm_action"),
                    action_ap=[rng.choice([0, 2, 4, 8, 20, 0x105, rng.randrange(256)]) for _ in range(64)],
                    shot_ap=[rng.choice([0, 2, 3, 5, 8, 16, 0x102]) for _ in range(64)],
                    tile_dist=[rng.choice([1, 1, 2, 0x101]) for _ in range(64)])
    if name == "sight_check":
        return dict(scripted("calm_action"), sight_line=[rng.choice([0, 1, 0x100, 0x10000])],
                    direction=[rng.choice([1, 2, 3, 4, 5, 6, 7, 8, 0, rng.randrange(1 << 32)])],
                    tile_dist=[rng.choice(list(range(16)) + [0x103, rng.randrange(1 << 32)])])
    if name == "stat_practice":
        return dict(scripted("calm_action"), Random=[rng.choice([0, 1, 5, 50, 98, 99, rng.randrange(100)])
                                                     for _ in range(40)])
    if name == "shot_alert":
        return dict(scripted("calm_action"), sight_line=[rng.choice([0, 1, 0x100])],
                    tile_dist=[rng.choice([0, 5, 14, 15, 30, 0x103])])
    if name == "merc_dies":
        return dict(scripted("calm_action"), sight_line=[rng.choice([0, 1, 0x100])], load_pcc=[rng.randrange(1 << 32)])
    if name == "kill":
        return dict(scripted("calm_action"), w_3fc04=[rng.choice([0, 1, 0x100]) for _ in range(12)])
    if name == "melee_hit":
        return dict(scripted("calm_action"), Random=[rng.choice([0, 1, 5, 15, 30, 98, 99, 0xFF, rng.randrange(100),
                                                                 rng.randrange(2500)]) for _ in range(6)],
                    shot_ap=[rng.choice([0, 2, 4, 9, 0x105, rng.randrange(1 << 32)])],
                    **(dict(sight_line=[rng.choice([0, 1, 0x100, 0x10000])],    # --port: Hit alert runs for real
                            tile_dist=[rng.choice([0, 1, 13, 14, 15, 0x10E, rng.randrange(1 << 32)])]) if PORT else {}))
    if name in TURN:
        return dict(scripted("calm_action"), Random=[rng.choice([0, 1, 5, 9, 94, 95, 99, 0x7F, 0xFF, rng.randrange(100),
                                                                 rng.randrange(100), rng.randrange(100)])
                                                     for _ in range(16)],
                    w_1f6e4=[rng.choice([0, 0, 1, 0x100])])
    if name == "land_item":
        return dict(scripted("calm_action"), Random=[rng.choice([0, 1, 2, 3, 5, 0x7F, 0xFF]) for _ in range(2)],
                    puff=[rng.choice([0, 1, 7, 0x80, 0x1FF])])
    if name == "hears":
        return dict(scripted("calm_action"), sight_line=[rng.choice([0, 1, 0x100])],
                    direction=[rng.choice([0, 1, 2, 5, 8, 8, rng.randrange(256)]) for _ in range(2)],
                    tile_dist=[rng.choice([0, 1, 13, 14, 15, 0x10E, rng.randrange(1 << 32)])])
    if name in CONTEST:
        return dict(scripted("calm_action"), move_ap=[rng.choice([0, 1, 3, 5, 0x80, 0xFF, 0x105, rng.randrange(1 << 32)])])
    if name == "interrupt_check":
        return dict(scripted("calm_action"), w_73d28=[rng.choice([0, 1, 0x100])], w_73f3c=[rng.choice([0, 1, 0x100])])
    if name == "hit_reaction" and PORT:    # --port: Hit alert runs for real (sight_line, tile_dist)
        return scripted("hit_alert")
    if name == "hit_alert":
        return dict(scripted("calm_action"), sight_line=[rng.choice([0, 1, 0x100, 0x10000])],
                    tile_dist=[rng.choice([0, 1, 13, 14, 15, 0x10E, rng.randrange(1 << 32)])])
    if name == "take_damage":       # --port: Hit reaction / Hit alert run for real (sight_line, tile_dist); the damage
        return dict(scripted("calm_action"),   # text fits its 5 bytes (the original writes long texts past the buffer)
                    hit_location=[rng.choice([0, 1, 5, 6, 20, 40, 41, 80, rng.randrange(-999, 1000) if PORT else rng.randrange(1 << 32)])],
                    direction=[rng.choice([1, 4, 8])], sight_line=[rng.choice([0, 1, 0x100])],
                    tile_dist=[rng.choice([0, 1, 14, 15, rng.randrange(1 << 32)])])
    if name == "impact":
        return dict(scripted("calm_action"), Random=[rng.choice([0, 1, 2, 20, 49, 99, rng.randrange(0x7FFF)]) for _ in range(8)],
                    bullet_damage=[rng.choice([1, 20, 31, 50, 60, 120, 130, 0x1FFFF])] * 2, door_lookup=[rng.choice([0, 50, 101, 200])],
                    puff=[rng.choice([0, 3, 0x80])] * 2,
                    **({"land_item": [rng.choice([0, 1, 100, 3135, rng.randrange(3136)])]} if FAN and 52 in FAN else {}))   # F52: the landed tile
    if name == "pstep":
        return dict(scripted("calm_action"),
                    hit_test=[(int(rng.random() < 0.03) * rng.choice([1, 2, 0xFFFFFFFF]), rng.choice([0, 0, 10, 30]),
                               rng.choice([0, 100])) for _ in range(400)], **IMPACT_OPS())
    if name == "launch":
        return dict(scripted("calm_action"), **IMPACT_OPS())
    if name == "use_on_tile":
        return dict(scripted("calm_action"), yes_no=[rng.choice([0, 1, 1, 0x100]) for _ in range(2)],
                    **(uot_port_script() if PORT else {}),
                    **{k_: [0] * 4 for k_ in ("w_53708", "w_1cb78")},
                    door_lookup=[rng.choice([0, 0, 5, 20, 50, 0xFFFF, 0xFFFFFFCE]) for _ in range(2)])
    if name == "pickup_here":
        return dict(scripted("calm_action"), yes_no=[rng.choice([0, 1, 1, 0x100]) for _ in range(2)], **{k_: [0] * 4 for k_ in ("w_53708", "w_1cb78")},
                    **(puh_port_script() if PORT else {}), Random=[rng.choice([0, 1, 3, 7, rng.randrange(0x7FFF)])])
    if name == "mine_flag":
        return dict(scripted("calm_action"), find_path=[MF_RNG.choice([0, 0, 1, 0x100])])
    if name == "walter":
        return dict(scripted("calm_action"), Random=[WL_RNG.choice([0, 1, 5, 0x4F, WL_RNG.randrange(0x7FFF)]) for _ in range(2)],
                    tile_dist=[WL_RNG.choice([0, 1, 2, 0, 1, 2, 3, 4, 5, 6, 7, 30, 0xFFFF]) for _ in range(8)],
                    **{k_: [0] * 4 for k_ in ("walter_say", "merc_message", "stat_practice", "item_panel")})
    if name == "lock_kit":
        return dict(scripted("calm_action"), Random=[LK_RNG.choice([0, 1, 2, 4, 0x7F, LK_RNG.randrange(0x7FFF)])])
    if name == "bury_mine":
        return dict(scripted("calm_action"), direction=[rng.choice([1, 3, 4, 5, 7, 0, 8, 0x104])],
                    Random=[rng.choice([0, 1, rng.randrange(0x7FFF)])])
    if name in ("fire_gun", "attack_go"):
        return dict(scripted("calm_action"), shot_ap=[rng.choice([0, 2, 5, 0x103, 0xFF])] * 2,
                    chance_to_hit=[rng.choice([0, 1, 30, 50, 99, 0x7F, 0x80, 0x1FF, rng.randrange(100)])] * 2,
                    sight_line=[rng.choice([0, 1, 0x100])] * 2,
                    tile_dist=[rng.choice([0, 1, 1, 2, 5, 0x101, 0xFFFFFFFF])] * 2,
                    Random=[rng.choice([0, 1, 15, 16, 18, 19, 49, 50, 98, 99, rng.randrange(0x7FFF)]) for _ in range(12)])
    if name == "start_attack":
        return dict(scripted("calm_action"), dir_atan=[(rng.choice([1, 2, 5, 8, 0x105]), rng.randrange(0x10000))] * 2,
                    reloaded=[rng.choice([0, 0, 1, 0x100])], w_5aa08=[0] * 2)
    if name in ("w_59a54", "w_5aa08", "w_59ae8"):
        return dict(scripted("calm_action"), Random=[rng.choice([0, 10, 40, 79, rng.randrange(0x7FFF)]) for _ in range(4)],
                    dir_atan=[(rng.choice([1, 2, 5, 8, 0x105]), rng.randrange(0x10000))],
                    path_ap=[rng.choice([0, 3, 0xFE, 0x1FF])], shot_ap=[rng.choice([2, 5, 0x103])])
    if name == "click":
        return dict(scripted("calm_action"), reach_ap=[rng.choice([0, 1, 1, 0x100])], shot_ap=[rng.choice([2, 5, 0x103])] * 2,
                    w_59ae8=[rng.choice([0, 2, 5, 10, 0xFF])], w_59a54=[rng.choice([0, 1, 1, 1])] * 2,
                    attack_ready=[rng.choice([1, 1, 0, 0xFFFFFFFB])] * 2, reloaded=[rng.choice([0, 1])],
                    action_ap=[rng.choice([0, 2, 5, 10, 0x105])] * 2, first_aid=[rng.choice([0, 1, 0x100])],
                    direction=[rng.choice([1, 2, 5, 8])] * 4, quest_check=[rng.choice([0, 1])],
                    dir_atan=[(rng.choice([1, 2, 5, 8]), rng.randrange(0x10000))] * 2,   # (--port: Aim posture runs)
                    Random=[rng.choice([0, 50, 99, rng.randrange(0x7FFF)]) for _ in range(4)])
    if name == "anim_frame":
        return dict(scripted("calm_action"), w_1148c=[0] * 8, Random=[rng.choice([0, 0, 1, 5, 11, rng.randrange(0x7FFF)]) for _ in range(40)],
                    find_path=[rng.choice([0, 1])] * 4, direction=[rng.choice([1, 3, 5, 8])] * 4)
    if name == "frame_checks":
        return dict(scripted("calm_action"), ui_refresh=[0] * 2, end_task=[0] * 2, set_anim=[0] * 2,
                    reloaded=[rng.choice([0, 1, 0x100])], quote_event=[0] * 2, merc_message=[0] * 2, cry=[0] * 2,
                    focus=[0] * 2, voc=[0] * 2)
    if name == "merc_quote":
        return dict(scripted("calm_action"), merc_message=[0] * 2, cry=[0] * 2, focus=[0] * 2, voc=[0] * 2)
    if name == "enemies_see":
        return dict(scripted("calm_action"), sight_check=[rng.choice([0, 1, 0x100, 2]) for _ in range(8)],
                    w_73f3c=[0] * 8)
    if name == "nearest_merc":
        return dict(scripted("calm_action"), tile_dist=[rng.choice([0, 1, 5, 6, 98, 99, 100, 0xFFFFFFFF,
                                                                    rng.randrange(1 << 32)]) for _ in range(8)])
    if name == "step_noise":
        return dict(scripted("calm_action"), sfx=[0], show_msg=[0], make_noise=[0],
                    Random=[rng.choice([0, 1, 2, 5, 19, 20, 40, 50, 80, 99, rng.randrange(0x7FFF)]) for _ in range(4)])
    if name == "consume_trigger":
        return dict(scripted("calm_action"))
    if name in ("throw_scatter", "throw_aim", "throw_follow"):
        return dict(scripted("calm_action"), focus=[0] * 2, spend_ap=[0] * 2, set_anim=[0] * 2,
                    action_ap=[rng.choice([0, 1, 4, 8, 0x7D, 0xFF, rng.randrange(1 << 32)]) for _ in range(2)],
                    direction=[rng.choice([1, 3, 5, 8, 0x101, rng.randrange(1 << 32)]) for _ in range(2)],
                    Random=[rng.choice([0, 1, 2, 5, 10, 49, 50, 98, 99, rng.randrange(0x7FFF)]) for _ in range(6)])
    if name == "mercs_see":
        return dict(scripted("calm_action"), sight_check=[rng.choice([0, 1, 0x100, 2]) for _ in range(16)],
                    share_sightings=[0] * 16, w_73d28=[0] * 16)
    if name == "walk_end":
        return dict(scripted("calm_action"), set_anim=[0] * 2, stop_anim=[0] * 2, stop_walk=[0] * 2, ui_refresh=[0] * 2)
    if name == "stand_up":
        return dict(scripted("calm_action"), set_anim=[0] * 2, stop_anim=[0] * 2)
    if name == "walk_blocked":
        return dict(scripted("calm_action"), stop_anim=[0] * 2, anim_step=[0] * 2,
                    find_path=[(rng.choice([0, 0, 1, 0x100]), rng.choice([0, 1, 3, 9])) for _ in range(3)],
                    tile_dist=[rng.choice([0, 2, 3, 4, 5, 0x103, rng.randrange(1 << 32)]) for _ in range(3)])
    if name == "active_frame":
        many = lambda vals, k=8: [rng.choice(vals) for _ in range(k)]
        return {**scripted("soldier_frame"), **{k_: [0] * 8 for k_ in (
                "close_door", "stat_practice", "spend_ap", "consume_trigger", "break_obj", "w_1ac68", "explode", "w_3c05c",
                "take_damage", "w_3a8c0", "enemy_turned", "w_735bc", "focus", "tac_redraw", "w_1304c", "next_turn",
                "guard_turned", "w_73f3c", "share_sightings", "throw_item", "mine_flag", "ai_think",
                "w_1148c", "click", "w_5abe0", "w_5afbc", "turn_banner", "merc_message", "stop_walk", "sfx", "make_noise",
                "show_msg", "share_sightings", "w_73d28")},
                "sight_check": many([0, 1, 0x100]), "w_59ae8": many([0, 1, 5, 20, 0x80, 0xFF, 0x105]),
                "w_1aaf0": many([0, 5, 6, 0xFFFFFFFF, 100]),
                "Random": many([0, 1, 5, 9, 10, 11, 50, 99], 40),
                "direction": many([1, 3, 5, 8, 0x101])}
    if name == "soldier_frame":
        return {**scripted("frame_checks"), **scripted("walk_step"), "stop_anim": [0] * 4, "anim_step": [0] * 4,
                "set_anim": [0] * 4}
    if name == "walk_step":
        return dict(scripted("anim_frame"), walk_blocked=[0] * 2, anim_next=[0] * 2, tile_dist=[rng.choice([0, 8, 9, 0x108, rng.randrange(1 << 32)])],
                    Random=[rng.choice([0, 0, 1, 1, 5, 11, rng.randrange(0x7FFF)]) for _ in range(40)])
    if name == "anim_step":
        return dict(scripted("calm_action"), find_path=[rng.choice([0, 1, 0x100])],
                    tile_dist=[rng.choice([0, 1, 2, 3, 4, 0x103])])
    if name == "reach_tile":
        return dict(scripted("calm_action"), find_path=[(rng.choice([0, 0, 1, 1, 0x100]),
                                                         rng.choice([0, 1, 3, 5, 5, 9, 0xFF, 0x100, 0x101, 0x8001,
                                                                     rng.randrange(0x10000)])) for _ in range(5)])
    if name == "line_of_fire":
        return dict(scripted("calm_action"),
                    sight_line=[rng.choice([0, 1, 0x100])],
                    hit_test=[(int(rng.random() < 0.004) * rng.choice([1, -1, 1]) & 0xFFFFFFFF,
                               rng.choice([0, 0, 5, 200]), rng.choice([100, 100, 100, 90, 50, 0, 120]))
                              for _ in range(3000)])
    if name == "gun_option":
        return dict(scripted("calm_action"),
                    action_ap=[rng.choice([0, 2, 4, 8, 20, 0x105, rng.randrange(256)]) for _ in range(64)],
                    fire_line=[rng.choice([0, 0, 10, 50, 100, 255, rng.randrange(1 << 32)]) for _ in range(200)],
                    shot_ap=[rng.choice([0, 2, 3, 5, 8, 16, 0x102]) for _ in range(64)],
                    chance_to_hit=[rng.choice([0, 1, 25, 50, 75, 99, 255, rng.randrange(1 << 32)]) for _ in range(400)])
    if name == "search_tile":
        return dict(scripted("calm_action"),
                    can_stand_at=[rng.choice([0, 0, 1, 0x100]) for _ in range(300)],
                    path_ap=[rng.choice([0, 1, 3, 5, 8, 20, rng.randrange(1 << 32)]) for _ in range(300)],
                    fire_line=[rng.choice(vals) for vals in [rng.choice([
                        [0, 10, 24, 25, 26, 50, 99, 100, rng.randrange(256), rng.randrange(1 << 32)],
                        [40, 41, 42, 43], [60, 62, 65]])] for _ in range(5000)],
                    tile_dist=[rng.choice([0, 1, 5, 15, 20, 99, 255, 256, rng.randrange(0x10000)]) for _ in range(8)])
    if name == "retreat_tile":
        return dict(scripted("calm_action"),
                    can_stand_at=[rng.choice([0, 0, 1, 0x100]) for _ in range(300)],
                    path_ap=[rng.choice([0, 1, 3, 5, 8, 20, rng.randrange(1 << 32)]) for _ in range(300)])
    if name == "find_dry_tile":                 # up to 51 x 51 tiles per ring
        return dict(scripted("calm_action"),
                    can_stand=[int(rng.random() < p) for p in [rng.choice([1 / 30, 1 / 3000])] for _ in range(6000)],
                    path_ap=[rng.choice([0, 3, 20, 400, 999, 1000, rng.randrange(1 << 32)]) for _ in range(400)])
    if name == "buddy_tile":                    # picks repeat until an untried slot comes up: never run dry
        return dict(scripted("calm_action"), Random=[rng.randrange(0x7FFF) for _ in range(3000)])
    return {"Random": [edge() for _ in range(40)], "idle": [0] * 4,
            "noise_tile": [(t, rng.choice([0, 10, 50, rng.randrange(1 << 32)])) for t in tiles()],
            "find_dry_tile": tiles(), "patrol_tile": tiles(), "buddy_tile": [rng.choice([0, 1, 0x100, 0x101])
                                                                      for _ in range(4)],
            "direction": [rng.choice([1, 2, 5, 8, rng.randrange(256)]) for _ in range(4)], "tile_dist": [rng.choice([0, 1, 5, 14, 30, 55, 255, 256, rng.randrange(0x10000)]) for _ in range(0x40)],
            "news_value": [rng.choice([0, 1, 3, 0x100, rng.randrange(256)]) for _ in range(4)],
            "sector_value": [rng.choice([-100, -50, 0, 30, 100, 101, 200, rng.randrange(0x10000), 34, 35, 36, 49,
                                         50, 51])
                                for _ in range(4)],
            "approach_tile": tiles(), "ally_call_tile": tiles(), "search_tile": tiles(),
            "move_ap": [rng.choice([0, 3, 5, 10, 30, 0xFFFFFFFF, rng.randrange(1 << 32)]) for _ in range(4)],
            "reach_ap": [rng.choice([0, 1, 7]) for _ in range(4)],
            "attack_ready": [rng.choice([1, 1, 0, 0xFA, 0xFB, 0x101, rng.randrange(256)]) for _ in range(4)],
            "swap_hand": [0] * 4, "action_ap": [rng.choice([1, 4, 8, 20, 0x7D, 0xFFFFFFFF]) for _ in range(4)],
            "can_stand_at": [rng.choice([0, 0, 0, 1, 0x100]) for _ in range(0x60)], "mission_cancel": [0] * 4,
            "opp_strength": [rng.choice([0, 1, 2, 3, 4, 0x103] + ([5, rng.randrange(256)]
                                         if name == "combat_action" else [])) for _ in range(4)],
            "retreat_tile": tiles(), "advance_tile": tiles(), "target_tile": tiles(),
            "gun_option": [record() for _ in range(4)], "throw_option": [record() for _ in range(4)],
            "knife_option": [record() for _ in range(4)], "busy": [rng.choice([0, 0, 1, 0x100]) for _ in range(4)],
            "turn": [rng.choice([0, 1, 0x100]) for _ in range(4)], "walk_to": [0] * 4, "click": [0] * 4,
            "spend_ap": [0] * 4, "wait": [0] * 4, "radio_result": [rng.choice([0, 1, 2, 0xFF]) for _ in range(4)],
            "share_sightings": [0] * 4, "perceive": [0] * 4, "redraw": [0] * 4, "delay": [0] * 4,
            "santino_speech": [0] * 4, "fire": [0] * 4, "clear_rooms": [0] * 4,
            "stat_practice": [0] * 8, "merc_sees": [0] * 2, "stop_walk": [0] * 2, "stop_anim": [0] * 2,
            "focus": [0] * 4, "message": [0] * 2, "anim_step": [0] * 2, "set_anim": [0] * 4, "end_task": [0] * 2, "replace": [0] * 2,
            "merc_message": [0] * 2, "anim_frame": [0] * 2, "ui_refresh": [0] * 2, "anim_turn": [0] * 2,
            "reserve_ap": [0] * 2, "walk_sound": [0] * 2, "sfx": [0] * 8, "merc_text": [0] * 2,
            **{k: [0] * 4 for k in ("w_355d0", "w_72568", "w_19eb0", "w_35774", "w_3c16c", "explode",
                                    "w_3c05c", "workable", "explode_art", "w_635dc")},
            **{k: [0] * 12 for k in ("show_msg", "mouse_hide", "tac_redraw", "open_dat", "load_pcc", "fx_load", "merc_voice",
                                     "fx_play", "blit", "w_320a0", "sector_dat", "mouse_show", "sector_won",
                                     "quote_event", "w_46a4c", "squad_lost")},
            **{k: [0] * 4 for k in ("mine_flag", "enemy_turned", "guard_turned", "w_11448", "w_17b70", "w_3f490", "quote",
                                    "w_58574", "use_special", "door_fx", "popups", "w_3cdd4", "w_3cfd0",
                                    "bury_mine", "w_401b4", "w_3521c", "w_3a8c0", "w_5abe0", "w_5ad50", "launch",
                                    "merc_dies", "anim_end", "portrait", "w_197d8", "step_sound", "ui_text", "ui_10310", "w_1c438",
                                    "w_5aa08", "confirm_click", "w_1c464", "item_cond", "item_panel", "next_turn", "w_1304c", "use_throw", "drop_item",
                                    "w_56dd4", "impact", "w_746d4", "make_noise", "fx_157bc", "explode",
                                    "land_item", "take_damage", "replay_event", "story_hit", "break_obj", "close_door",
                                    "rest", "energy_bar", "w_33204", "w_323d4", "anim2", "w_74304", "react", "cry", "voc", "w_73ab4", "w_73d28", "w_73f3c", "w_73c00", "w_1ac68", "put_item", "aid_tick", "w_32d04", "w_331a4", "w_1f6e4", "w_53660", "w_32eb8", "w_3fc04", "w_3a2d4", "w_328a4")},
            "connected_home": [rng.choice([0, 1, 1, 0x100, rng.randrange(256)])],
            "quest_check": [rng.choice([0, 0, 1, 0x100, rng.randrange(256)])],
            "sight_check": [rng.choice([0, 0, 1, 2, 0xFF, rng.randrange(1 << 32)]) for _ in range(0x30)],
            "path_ap": [rng.choice([0, 0, 1, 5, 30, 999, 1000, 0xFFFF, rng.randrange(1 << 32)]) for _ in range(0x30)],
            "find_path": [rng.choice([0, 0, 1, 1, 0x100, rng.randrange(1 << 32)]) for _ in range(8)],
            "fire_line": [rng.choice([0, 1, 50, 100, rng.randrange(1 << 32)]) for _ in range(4)]}


FP_MAP = [None]          # find_path: the dump's move codes + tile map
T1_SPECIAL = sorted(ai.T1_STAND) + [0x3C, 0x3F, 0x40, 0x42, 0x44, 0x5B, 0x5E, 0x61, 0x63, 0x65, 0x1C, 0x1F]


def fuzz_tile(t):
    """map bytes CanStandAt? / TileStandable read: t0 ground, t1 object, byte 3, byte 5 occupant + flags"""
    a = rt(G_TILEMAP) + 8 * t
    e.w8(a, rng.choice([0x10, 0x6B, 0x6C, 0x80, rng.randrange(256)]))
    e.w8(a + 1, rng.choice(T1_SPECIAL + [0, 0, rng.randrange(256)]))
    e.w8(a + 3, rng.choice([0x73, 0, rng.randrange(256)]))
    e.w8(a + 5, rng.choice([0x40, 0x40, 0xC0, 0x80 | rng.randrange(0x30), rng.randrange(0x30), rng.randrange(256)]))
    occ = e.r8(a + 5) & 0x7F
    if occ < 0x30:
        e.w8(rt(G_SOLDIERS) + 0x200 * occ + 0x9C, rng.choice([0, 0, 1, 0xFF]))


def fuzz_side(slot):
    """opp_strength / soldier_power: every soldier's power inputs, sight flags and the knowledge tables"""
    lvl = lambda: rng.choice([0, 0, 0, 1, 1, 2, 3, 4, -1, -2, -3]) & 0xFF
    for k in range(0x30):
        a = rt(G_SOLDIERS) + 0x200 * k
        p = bytearray(e.uc.mem_read(a, 0x200))
        p[0x182], p[0x138] = rng.choice([1, 1, 1, 1, 0]), rng.choice([1, 1, 1, 1, 0])
        p[0x183] = rng.choice([0, 14, 15, 19, 20, 39, 40, 75, 76, 100, rng.randrange(256)])
        p[0x184] = rng.choice([p[0x183], 100])
        p[0x185] = rng.choice([0, 0, rng.randrange(20)])
        if FAN and 17 in FAN:      # F17: a down opponent at 0 health (+0x186 > 0) is seen in 1.15m
            if rng.random() < .3:
                p[0x183] = 0
            p[0x186] = rng.choice([0, 1, 5, 0x7F, 0x80, 0xFF])
        for o in (0x18A, 0x18B, 0x18C, 0x191, 0x192, 0x1A5):
            p[o] = rng.choice([rng.randrange(101), rng.randrange(256), 100, 1])
        p[0x1A1] = rng.choice(list(range(1, 9)))
        p[0x00] = rng.choice([0, rng.randrange(1, 15), 0x0F, 0x10, 0x5D, rng.randrange(136)]
                             + KIND.get(0x95, [0]))
        for o, kinds in ((0x3C, [0x2D, 0x2E, 0x2F, 0x30]), (0x18, [0x23, 0x24, 0x25, 0x26]), (0x48, [0x31])):
            p[o] = rng.choice(kinds + [0, rng.randrange(136)])
            p[o + 2] = rng.choice([100, 50, rng.randrange(256)])
        ai.w16(p, 0xD0, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
        ai.w16(p, 0x114, rng.randrange(3136))
        ai.w16(p, 0x108, rng.choice([rng.randrange(3136), ai.u16(p, 0x114), ai.NO_TILE]))
        p[0x13A:0x162] = bytes(lvl() for _ in range(0x28))
        p[0x17A] = k
        p[0x1C4] = rng.choice([0, 0, 0, 1])
        p[0x1C0] = rng.choice([0, 0, 12, 13, 20, rng.randrange(256)])
        e.uc.mem_write(a, bytes(p))
    e.uc.mem_write(rt(ai.G_KNOWN), bytes(lvl() for _ in range(0x30)))
    e.uc.mem_write(rt(ai.G_REMEMBER_E), bytes(rng.choice([0, 0, 1]) for _ in range(0x140 + 0x80)))
    for k in range(0x280 + 0x80):
        e.w16(rt(ai.G_LAST_SEEN_E) + 2 * k, rng.choice([rng.randrange(3136), ai.NO_TILE, rng.randrange(0x10000)]))
    for k in range(0x80):
        e.w16(rt(ai.G_LAST_SEEN_M) + 2 * k, rng.choice([rng.randrange(3136), ai.NO_TILE, rng.randrange(0x10000)]))
    e.w8(rt(ai.G_SANTINO_SPOKE), rng.choice([0, 1]))
    e.w8(rt(ai.G_LAST_SPOTTER), rng.choice([0x40, rng.randrange(0x28, 0x30), rng.randrange(16)]))
    for c in (ai.G_CALL_M, ai.G_CALL_E):
        e.w16(rt(c), rng.choice([ai.NO_TILE, rng.randrange(3136), rng.randrange(0x10000)]))
        e.w8(rt(c) + 2, rng.choice([0, 12, 13, 30, rng.randrange(256)]))


import combat                        # noqa: E402
GUNS = [k for k in range(136) if e.r8(rt(ai.G_ITEM_KIND) + 12 * k) == 0x92 and e.r8(rt(ai.G_ITEM_RANGE) + 12 * k)]
MOVE_CODES = sorted(combat.STEP_EXTRA) + [combat.MOVE_WALL] * 6 + [0, 1, 2, 999]
MERC_TEXTS = {1: "doesn't have enough Action Points!", 2: "has reloaded.", 5: "abandoned first aid", 6: "is all bandaged!",
              7: "ran out of bandages!", 8: "is already performing first aid! Press ESC to abort that first.",
              9: "doesn't need first aid", 11: "not healthy enough!"}     # combat.md First aid, exact steps
STEP_TEXTS = ["splash", "SPLASH!", "clunk", "CLUNK!", "snap", "SNAP!", "click"]     # port_api.md op 129 step_text: text number -> text ("click" = PlayEmptyClick 0x1ABAC)


def port_ai(name, ps, edx, ebx=0):
    """--port: the clean-room AI entry point on a world copied from emulator memory, written back afterwards so the
    record / globals comparison sees the port's writes (the soldier's own record goes to ps only, as the mirror's)"""
    slot = ps[0x17A]
    blast0_ = port_lib.BLAST
    port_lib.BLAST = blast0_ or name == "use_on_tile"   # (the tileset: combat.md Using the hand item on a tile, locks)
    port_lib.WALTER = name == "walter"   # (combat.md The listening device: walter_said, walter_next, walter_sector)
    port_lib.PROFILES = name in ("use_on_tile", "pickup_here", "mine_spotted")   # (entry 0xFF = profile 5 bytes)
    bufs, scalars = port_lib.world_load(e, ai)
    own = bytes(e.uc.mem_read(rt(G_SOLDIERS) + 0x200 * slot, 0x200))     # pointer form, as in memory
    rec = bytearray(ps)
    port_lib.script_pos(rec, e, ai, True)
    bufs["soldiers"][0x200 * slot:0x200 * slot + 0x200] = rec
    crash = None                           # (soldiers, scalars) at the first div_zero call
    pass_asked = [False]                   # op 358 effect_pass came before the next death_show

    def random_(k):
        k = ai.s16(k)
        log.append(("Random", (k,)))
        v = take("Random")
        return v % k if k > 0 else 0

    def call(op, args):
        nonlocal crash
        if op == "div_zero":               # the original crashes here (port_api.md, Division by zero)
            if crash is None:              # the state at this point is observable, nothing after it
                crash = (bytearray(bufs["soldiers"]), dict(scalars))
            raise ZeroDivisionError
        if op == "effect_pass":            # op 358 (#70 row 30): MercDies? -> TacticalRedraw(1) runs the effect pass;
            pass_asked[0] = True           # the original's call is the tac_redraw stub, logged at death_show
            return 0, None
        if op == "death_show":             # op 186 (shooting.md Merc dies): the original's merc death display, its
            w_ = World(args[0])            # calls logged as the original's (the mirror's part on memory)
            w_.memrun = True
            lib_ = PORT_LIB.get(name, 0) >> 63 & 1   # (bit 63 cleared: the library ran the pass itself)
            ai.merc_death_show(MemRec(args[0]), w_, args[1], redraw=None if lib_ else pass_asked[0])
            pass_asked[0] = False
            return 0, None
        if op == "cursor_reset":           # op 163: the original's two cursor bytes (Take damage)
            e.w8(rt(0x284F1D), 0xA1)
            e.w8(rt(0x284F1B), 0)
            return 0, None
        if op == "tile_marker":            # op 130 (click 0x4D -> Move order): the original stores the marker globals
            ai.tile_marker(World(0), args[0], args[1])
            return 0, None
        if op == "attack_cursor":          # op 192 (click.md): the original's attack cursor timer
            e.w32(rt(0x229DD4), 40000)
            return 0, None
        if op == "hears_msg":              # op 213 (ai.md Hearing a noise): the original's text into its message
            ai.hears_text(World(args[0]), args[0], args[1], args[2] & 0xFF, args[3] & 0xFF)    # buffer, then the message call
            bufs["text_buf"][:] = e.uc.mem_read(rt(ai.G_TEXT_BUF), len(bufs["text_buf"]))   # (the port world's copy)
            log.append(("message", conv(CALLEES["message"][1], [rt(ai.G_TEXT_BUF)])))
            return take("message"), None
        if op == "guard_sees_msg":         # op 326 (ai.md Share sightings): the original's message call with its text
            log.append(("message", conv(CALLEES["message"][1], [rt(0x9AAF9)])))
            return take("message"), None
        if op in ("w_67c54", "mouse_release") and op not in CALLEES:   # TimeCompress(0)'s button-up waits
            return 0, None                 # (#70 F18 calls it from the fuses): the original side stubs 0x67C54 /
        if op not in CALLEES and op != "reveal_view":                  # 0x67C68 silently (platform code)
            raise SystemExit(f"port called unknown host op {op} {args}")
        args = [a & 0xFFFFFFFF for a in args]     # register values, as the original's stubs log them
        if op == "reveal_view":            # runs for real in the original: the mirror on memory answers it
            port_lib.bufs_store(e, ai, bufs)
            w_ = World(args[0])
            w_.memrun = True
            ai.reveal_view(args[1] & 0xFF, args[2] & 0xFF, MemRec(args[0]), w_, args[3] & 0xFF)
            port_lib.bufs_load(e, ai, bufs)
            return 0, None
        if op == "merc_text":              # the port passes a text number (port_api.md, op 57)
            args = [args[0], MERC_TEXTS.get(args[1], f"text {args[1]}")] + args[2:]
        if op == "cry":                    # the port passes a cry number (port_api.md, op 100)
            args = args[:2] + [port_lib.CRIES[args[2]] if args[2] < 5 else f"cry {args[2]}"] + args[3:]
        if op == "show_msg":               # op 129 step_text: the port passes a text number (port_api.md); the original
            args = [STEP_TEXTS[args[0]] if args[0] < len(STEP_TEXTS) else f"text {args[0]}", 0, 0, 1, 1, 0, 500, 1]   # shows it 500 ms
        log.append((op, conv(CALLEES[op][1].replace("s", "i"), args)))
        v = take(op)
        if op == "noise_tile":
            return v[0], struct.pack("<I", v[1] & 0xFFFFFFFF)
        if op in RECORDS:
            return 0, v
        if op == "hit_test":               # (stops, new skip, chance): skip and chance through the out record
            v = v or (0, 0, 100)
            return v[0], struct.pack("<BxxxI", v[1] & 0xFF, v[2] & 0xFFFFFFFF)
        if op == "disarm" and isinstance(v, tuple):        # (r, 1): the prompt took the item off the tile
            bufs["tiles"][8 * (args[1] & 0xFFFF) + 6] = 0xFF
            return v[0], None
        if op == "find_path" and isinstance(v, tuple):     # (found, pathLen): the stub sets the record's +0xD4
            bufs["soldiers"][0x200 * args[0] + 0xD4:0x200 * args[0] + 0xD6] = struct.pack("<H", v[1] & 0xFFFF)
            return v[0], None
        return v, None
    extra = (ai._i32(edx), ai._i32(ebx)) if name in ("soldier_power", "line_of_fire", "walk_blocked", "throw_scatter", "find_path", "spend_ap", "rest", "w_56dd4", "reach_tile", "click", "turn_to", "swap_hand", "use_on_tile") else ai._i32(edx)
    if name == "impact":                   # (tile, x, y, hit): ECX and the stack argument
        extra = (ai.s16(edx), ai._i32(ebx), ai._i32(ecx_arg), ai._i32(stack_arg))
    if name in ("take_damage", "land_item"):           # (height, damage, kind, attacker, tile): ECX and two stack arguments
        extra = (ai._i32(edx), ai._i32(ebx), ai._i32(ecx_arg), ai._i32(stack_arg[0]), ai._i32(stack_arg[1]))
    if name == "hit_reaction":             # (attacker, flag, damage)
        extra = (ai._i32(edx), ai._i32(ebx), ai._i32(ecx_arg))
    if name == "first_aid":                # (tile, task, b): b is not used
        extra = (ai._i32(edx), ai._i32(ebx), 0)
    if name == "chance_to_hit":            # (tile, aim)
        extra = (ai._i32(edx), ai._i32(ebx))
    if name == "hit_alert":                # (opponent record index, flag)
        extra = (SP[edx], ai._i32(ebx))
    if name in ("mark_seen", "sight_check") or name in CONTEST:     # (opponent record index)
        extra = SP[edx]
    if name == "interrupt_check":          # (opponent record index, old flag)
        extra = (SP[edx], ai._i32(ebx))
    if name == "path_ap":                  # Path AP with marks (select.md): tile, mark, extra, sneak, byte177, alt
        extra = (ai._i32(edx), ai._i32(ebx), ai._i32(ecx_arg)) + tuple(ai._i32(a_) for a_ in stack_arg)
    if name == "set_anim":
        extra = (ai._i32(edx), ai._i32(ebx), ai._i32(ecx_arg))
    if name == "w_59ae8":                  # Use cost (combat.md): spot, tile, byte g, kind (stack)
        extra = (ai.s16(edx), ai.s16(ebx), ai._i32(ecx_arg), ai._i32(stack_arg))
    if name == "trigger":                  # (tile, ground entry BL, armed CL)
        extra = (ai._i32(edx), ai._i32(ebx), ai._i32(ecx_arg))
    if name == "hears":                    # (tile, strength, kind)
        extra = (ai._i32(edx), ai._i32(ebx), ai._i32(ecx_arg))
    try:
        r = port_lib.ai_run(name, bufs, scalars, ai._i32(edx) if name == "consume_trigger" else slot, random_, call, extra,
                            sync=(lambda _sc: None, lambda _sc: None),     # (scalars current at each host call)
                            host_ops=port_lib.ALL_OPS & ~PORT_LIB.get(name, 0))
    except ZeroDivisionError:          # the writes before the division stay (as in the original): store them, then re-raise
        bufs["soldiers"][:] = crash[0]
        scalars.update(crash[1])
        port_lib.world_store(e, ai, bufs, scalars)
        port_lib.BLAST = blast0_
        ps[:] = bufs["soldiers"][0x200 * slot:0x200 * slot + 0x200]
        e.uc.mem_write(rt(G_SOLDIERS) + 0x200 * slot, own)
        raise
    port_lib.world_store(e, ai, bufs, scalars)
    port_lib.BLAST = blast0_
    if name == "noise_tile":           # the loudness goes through EDX (16 bits)
        r, loud = r
        e.w16(rt(OUT), loud)
    ps[:] = bufs["soldiers"][0x200 * slot:0x200 * slot + 0x200]
    e.uc.mem_write(rt(G_SOLDIERS) + 0x200 * slot, own)        # as the mirror leaves it: own record in ps only
    return r


def port_move_order(slot, tile, args_):
    """--port: ja1_move_order on the world from emulator memory (the library runs Path AP / Find path itself), written
    back. args_ = marker, sneak, b177, alt, confirm (0), squad"""
    bufs, scalars = port_lib.world_load(e, ai)

    def call(op, args):
        args = [a & 0xFFFFFFFF for a in args]
        if op == "tile_marker":            # op 130: the original stores the marker globals itself (no call)
            ai.tile_marker(World(0), args[0], args[1])
            return 0, None
        if op not in CALLEES:
            raise SystemExit(f"port called unknown host op {op} {args}")
        if op == "merc_text":
            args = [args[0], MERC_TEXTS.get(args[1], f"text {args[1]}")] + args[2:]
        log.append((op, conv(CALLEES[op][1].replace("s", "i"), args)))
        return take(op), None
    m_, sn_, b_, al_, _, sq_ = (ai._i32(a_) for a_ in args_)
    r = port_lib.ai_run("move_order", bufs, scalars, slot, None, call, (ai._i32(tile), m_, sn_, b_, al_, sq_),
                        host_ops=port_lib.ALL_OPS & ~(1 << 40 | 1 << 41))
    port_lib.world_store(e, ai, bufs, scalars)
    return r


def port_merc_view(slot, facing, range_):
    """--port: ja1_merc_view (op 62) on the world from emulator memory, written back; host ops logged + scripted"""
    bufs, scalars = port_lib.world_load(e, ai)

    def random_(k):
        k = ai.s16(k)
        log.append(("Random", (k,)))
        v = take("Random")
        return v % k if k > 0 else 0

    def call(op, args_):
        args_ = [a & 0xFFFFFFFF for a in args_]
        if op not in CALLEES:
            raise SystemExit(f"port called unknown host op {op} {args_}")
        log.append((op, conv(CALLEES[op][1].replace("s", "i"), args_)))
        return take(op), None
    port_lib.ai_run("merc_view", bufs, scalars, slot, random_, call, (facing, range_, 0))
    port_lib.world_store(e, ai, bufs, scalars)


DOOR_HOST = []      # the door_fx section's door_anim logger (the port's host op 210, combat.md "Doors")


START_BLED = {}                    # turn_end: slot -> g_bled byte that the stubbed StartTurn? 0x11FB4 writes (its bleed)


def port_world(name, args=(), sync=False):
    """--port: a world-only entry point (port_lib.SECTOR_ENTRY, e.g. frame / tick) on the world from emulator memory,
    written back; host ops logged and scripted as the original's stubs"""
    bufs, scalars = port_lib.world_load(e, ai)

    def random_(k):
        k = ai.s16(k)
        log.append(("Random", (k,)))
        v = take("Random")
        return v % k if k > 0 else 0

    def call(op, args_):
        args_ = [a & 0xFFFFFFFF for a in args_]
        if op == "mouse_check":            # op 162 (turns.md New turn): the original's MouseInRect? on its panel rectangle,
            r_ = [ai._i32(e.r32(rt(a_))) for a_ in (0x229CB8, 0x229CC4, 0x229CD8, 0x229CDC)]   # then the flag / 0x673AC
            log.append(("mouse_in_rect", tuple(r_)))
            if take("mouse_in_rect") & 0xFFFFFFFF:
                e.w8(rt(0x284F13), 1)
            else:
                log.append(("w_673ac", ()))
                take("w_673ac")
            return 0, None
        if op == "cursor_reset":           # op 163: the original's two cursor bytes
            e.w8(rt(0x284F1B), 0)
            e.w8(rt(0x284F1D), 0xA1)
            return 0, None
        if op == "cry":                    # the port passes a cry number (port_api.md, op 100)
            args_ = args_[:2] + [port_lib.CRIES[args_[2]] if args_[2] < 5 else f"cry {args_[2]}"] + args_[3:]
        if op == "banner":                 # op 194 (turns.md Turn start): the original's banner text and colour
            ai.banner_show(World(0), args_[0])
            return 0, None
        if op == "tile_anim":              # op 205 (soldier_update.md Update end): the front-end starts the ripple
            port_lib.world_store(e, ai, bufs, scalars)                 # (the original's StartTileAnim, not stubbed;
            ai.start_tile_anim(World(0), args_[0], 0x95659, 0x12)      # tile byte 3 goes back into the port world)
            port_lib.world_load(e, ai, bufs, scalars)
            return 0, None
        if op == "door_snd":               # op 209 (combat.md Doors): as the original's door / overhead sound loads
            log.append((op, (args_[0],)))
            return 0, None
        if op == "door_anim" and DOOR_HOST:    # op 210: as the original's FX file name
            DOOR_HOST[0](*[ai._i32(a_) for a_ in args_])
            return 0, None
        if op in ("pal_cycle", "pal_set", "fade"):  # ops 206 / 207 / 215: the original also passes the palette address
            args_ = [0xEDA08] + args_
        if op == "music_stop":             # op 216 (soldier_update.md Leaving the sector): the front-end's hover flag
            e.w8(rt(0x284F13), 0)          # and next-music number (the original clears them after its music stop)
            e.w16(rt(0x96360), 0xFFFF)
        if op == "cursor_select":          # op 200 (select.md): the original's two cursor bytes
            e.w8(rt(0x284F1B), 1)
            e.w8(rt(0x284F1D), 0x8E)
            return 0, None
        if op == "panel_view":             # op 196 (select.md): the original's panel move (0x24880, stubbed), its
            log.append(("w_24880", conv(CALLEES["w_24880"][1], [0, 0x14, 0xD])))   # timer and the view scroll
            take("w_24880")
            e.w32(rt(0x92E30), 3000)
            e.w8(rt(0x92E36), 1)
            y_, b_ = ai._i32(args_[0]), ai._i32(e.r32(rt(0x229D04)))
            if b_ >= 0xB6 and y_ + 14 < 0x310 - ai._i32(e.r32(rt(0x229CC0))):
                e.w32(rt(0x229D04), b_ - 14)
                log.append(("w_681d0", conv(CALLEES["w_681d0"][1], [e.r32(rt(0x229D00)), (b_ - 14) & 0xFFFFFFFF])))
                take("w_681d0")
                return (y_ + 14) & 0xFFFFFFFF, None
            return y_ & 0xFFFFFFFF, None
        if op == "reveal_view":            # op 62 (Merc view): runs for real in the original, the mirror on memory
            port_lib.world_store(e, ai, bufs, scalars)      # (sync: its scalar writes go back into the port world)
            w_ = World(args_[0])
            w_.memrun = True
            ai.reveal_view(args_[1] & 0xFF, args_[2] & 0xFF, MemRec(args_[0]), w_, args_[3] & 0xFF)
            port_lib.world_load(e, ai, bufs, scalars)
            return 0, None
        if op in ("natives_quit_msg", "map_redraw"):   # ops 261 / 262 (strategic.md Natives quit, Worker pool): as
            port_lib.world_store(e, ai, bufs, scalars)  # the original's sprintf_ into g_textBuf + StatusMessage? and
            w_ = World(0)                               # its 0x41C3C / 0x36010 map redraw
            w_.memrun = True
            if op == "natives_quit_msg":
                word_ = (b"guard", b"guards", b"tapper", b"tappers")[args_[1]]
                ai.quit_text(w_, b"%d %s quit this morning!" % (ai._i32(args_[0]), word_))
            else:
                w_.ext("draw_sectors", ai.G_SECTOR_TBL9)
                w_.ext("w_36010", 0x90FAC, w_.g32(0x95678), 0x140, 0, 0xE)
            port_lib.world_load(e, ai, bufs, scalars)
            return 0, None
        if op == "fire_quote":             # op 328 (strategic.md Firing a merc) as the original's calls
            w_ = World(0)
            a_ = [ai._i32(x_) for x_ in args_]
            w_.ext("quote", a_[3], 14 * a_[3], 9, 200, 0x2299BC, 0, a_[3], 1)
            w_.ext("balloon", a_[0], a_[1], a_[2], 5)
            if e.r8(rt(0x1163BD)):
                w_.ext("w_28698", 0x25)
            return 0, None
        if op in ("face_move", "team_redraw", "runs_off", "island_map", "runs_off_end"):   # ops 265..269 (strategic.md
            w_ = World(0)                                                                  # Morning pay) as the original's calls
            w_.memrun = True
            a_ = [ai._i32(x_) for x_ in args_]
            if op == "face_move":
                w_.ext("blit", w_.g32(0x93428 + 4 * a_[0]), w_.g32(0x93428 + 4 * a_[1]), 0x20, 0x20, 0x20, 0x20)
            elif op == "team_redraw":
                w_.ext("w_21b3c")
            elif op == "runs_off":
                w_.ext("quote", a_[2], a_[2] * 15, 0xC, 200, 0x2299BC, 2, a_[2], 1)
                w_.ext("w_67bf0")
                w_.ext("w_467e4", 0, 0x34, 0x25)
                w_.ext("balloon", a_[0], a_[1] * 0x25 + 0xB, 0x14, 5)
                w_.ext("w_28698", 0x25)
                w_.ext("message", 0x99597, 0xE)
            elif op == "island_map":
                w_.ext("bass_map")
            else:
                w_.ext("w_36010", 0x90B18, w_.g32(0x93E60), 0x140, 0x2B, 0x33)
                w_.ext("w_67bf0")
                w_.ext("w_46bf4")
                w_.ext("w_673ac")
            return 0, None
        if name == "end_of_day":           # soldier_update.md End of day: ops 254..260 as the original's front-end
            r_ = eod_host(op, args_, bufs, scalars)
            if r_ is not None:
                return r_, None
        if op in ("w_67c54", "mouse_release") and op not in CALLEES:   # TimeCompress(0)'s button-up waits
            return 0, None                 # (#70 F18 fuses / end turn): the original side stubs 0x67C54 / 0x67C68
        if op not in CALLEES:              # silently (platform code)
            raise SystemExit(f"port called unknown host op {op} {args_}")
        if op == "merc_text":              # the port passes a text number (port_api.md, op 57)
            args_ = [args_[0], MERC_TEXTS.get(args_[1], f"text {args_[1]}")] + list(args_[2:])
        log.append((op, conv(CALLEES[op][1].replace("s", "i"), args_)))
        v_ = take(op)
        if op == "w_11fb4" and args_[0] in START_BLED:   # turn_end: the stub's bleed mark, into the port world
            port_lib.world_store(e, ai, bufs, scalars)
            e.w8(rt(ai.G_BLED) + e.r8(rt(G_SOLDIERS) + 0x200 * args_[0] + 0x17A), START_BLED[args_[0]])
            port_lib.world_load(e, ai, bufs, scalars)
        if op == "noise_tile" and isinstance(v_, tuple):   # (tile, loudness): the loudness through the out record
            return v_[0], struct.pack("<I", v_[1] & 0xFFFFFFFF)
        return v_, None
    r = port_lib.sector_run(name, bufs, scalars, list(args), random_, call, sync=sync)
    port_lib.world_store(e, ai, bufs, scalars)
    return r


EOD = {}    # end_of_day --port: the sunset sound handle and the text


def eod_host(op, args_, bufs, scalars):
    """end_of_day --port: one host op of the port as the original's calls (the stubs and memory writes of
    EndOfDayScreen 0x29F44 / the retreat 0x30B04 around the library's steps); None = the generic translation"""
    w_ = World(0)
    w_.memrun = True
    if op == "eod_begin":              # op 254: sounds, picture, palette, the sunset sound
        port_lib.world_store(e, ai, bufs, scalars)
        w_.ext("load_main_sound")
        w_.s8(0x92DCC, 1)
        w_.s16(0x22A486, 0)
        w_.s16(0x22A484, 0)
        w_.s8(0x92E4F, 1)
        w_.s32(0x229D18, w_.ext("load_pcc", w_.g32(0x92F44), 0x10AD58, "sunset.pcx", w_.g32(0x93E60), 0x140) & 0xFFFFFFFF)
        w_.s32(0x229D18, w_.ext("load_pal", w_.g32(0x92F44), 0x10AD58, "sunset.pcx", 0xFA608) & 0xFFFFFFFF)
        w_.ext("pal_copy", 0xFA608, 0xEDA08, 10)
        EOD["text"] = "End of Day " + str(ai._i32(args_[0]))
        w_.ext("w_67bf0")
        w_.ext("w_46bf4")
        EOD["h"] = w_.ext("sfx", 0x12, 0x2B11, 0x32, 0)
        port_lib.world_load(e, ai, bufs, scalars)
        return 0
    if op == "eod_text":               # op 256
        w_.ext("eod_text", "End of Day " + str(ai._i32(args_[0])), 0, 0x140, 0x9F, 0x40, 1, 0xFF, 1, 2, 0x12C)
        return 0
    if op == "eod_poll":               # op 257: the screen, then the key / button test
        w_.ext("w_6782c")
        return w_.ext("key_any") & 0xFFFFFFFF
    if op == "eod_sound_stop":         # op 258
        w_.ext("w_5ce60", EOD.get("h", 0) & 0xFFFFFFFF)
        return 0
    if op == "overhead_sounds":        # op 259
        w_.ext("w_5ce08")
        w_.ext("overhead_snd", 0x2D)
        return 0
    if op == "show_screen":            # op 260
        w_.ext("w_46bf4")
        return 0
    if op in ("delay", "merc_text"):   # op 37: the original's busy wait on the 10 ms timer; op 57: text, then 1500 ms
        if op == "merc_text":
            w_.ext("merc_text2", MemRec(args_[0]), {3: "is shot retreating!", 4: "dies retreating!"}[args_[1]])
        log.append(("wait", (ai._i32(args_[-1]),)))
        port_lib.world_store(e, ai, bufs, scalars)     # (0x92EF8 lies in a world buffer)
        e.w32(rt(0x92EF8), 0)
        port_lib.world_load(e, ai, bufs, scalars)
        return 0
    if op == "div_zero":               # op 47: the original's Random(0)
        w_.random(0)
        return 0
    if op == "connected_home":         # op 43: the original also passes the sector table
        log.append(("connected_home", conv(CALLEES["connected_home"][1], [0] + args_)))
        return take("connected_home") & 0xFFFFFFFF
    return None


PATCH_KINDS = ("CRATE", "ITEM", "DEADPL", "LIVEPL")    # open_patch kinds (sector_start.md batch 6)


DISARM_TEXTS = ["Attempt to disarm explosive?", "Remove blue flag?"]   # op 345 disarm_ask kinds
PICK_EMPTIES = [False]       # --port pickup "pick": op 344 cursor_to_soldier empties the cursor (as the original's stub)


PORT_OP_HOOK = {}                      # port_sector: op name -> fn(args, bufs, scalars) -> result


def port_sector(name, args, host_ops=None, allow=()):
    """--port: one sector start entry point (port_lib.SECTOR_ENTRY) on the world from emulator memory, written back.
    host_ops: default ALL_OPS (port_lib.SECTOR_LIB_OPS = the library runs replace + reserve_ap itself)"""
    port_lib.PROFILES = name in ("disarm", "mine_spotted", "pick_up", "take_ground_item", "take_ground_patch",
                                 "ground_blast", "crate_drops", "crate_lost", "fixed_items", "plant_sector", "plant_done",
                                 "tile_patch", "item_patch", "put_item", "setup_sector")   # (entries past the 200: text buffer, then profile bytes)
    bufs, scalars = port_lib.world_load(e, ai)

    def random_(k):
        k = ai.s16(k)
        log.append(("Random", (k,)))
        v = take("Random")
        return v % k if k > 0 else 0

    def call(op, args_):
        args_ = [a & 0xFFFFFFFF for a in args_]
        if op in PORT_OP_HOOK:             # a section's own handler (effect_pass: effect_hit, tile_anim)
            return PORT_OP_HOOK[op](args_, bufs, scalars), None
        if op == "div_zero":               # the original crashes here (port_api.md, Division by zero)
            raise ZeroDivisionError
        if op == "open_cd":                # op 86 open_patch(kind, number, count): the path the original opens
            path_ = ("DAT\\XRA.DAT" if args_[0] == 4 else          # kind 4: XRA.DAT (sector_start.md batch 8)
                     "dat\\explode.dat" if args_[0] == 5 else    # kind 5: EXPLODE.DAT (combat.md Craters)
                     f"MISC\\{PATCH_KINDS[args_[0]]}{ai._i32(args_[1]):02d}.BIN")
            log.append(("open_cd", (path_,) if args_[0] == 5 else (path_, args_[2])))   # (kind 5: count not passed)
            return CD.after("open_cd", (path_, args_[2]), take("open_cd")), None
        if op == "fread":                  # op 87 read(n): the bytes of the file opened last
            data_ = CD.read(args_[0])
            return len(data_), data_
        if op == "open_tmp":               # op 92 open_tmp(sector): the original's fopen of sec%02d.tmp
            args__ = ("sec%02d.tmp" % args_[0], "rb")
            log.append(("fopen", args__))
            fp_ = CD.after("fopen", args__, take("fopen"))
            e.w32(rt(ai.G_DAT_FILE), fp_)  # (the host's file handle: the original keeps it in g_datFile)
            return (0 if fp_ else 0xFFFFFFFF), None
        if op == "fseek":                  # op 96 seek(pos) in the file opened last
            log.append(("fseek", (args_[0], 0)))
            CD.after("fseek", (None, args_[0]), 0)
            return 0, None
        if op == "sector_resets":          # op 93: the original's per-sector globals (one Random draw)
            port_lib.bufs_store(e, ai, bufs)  # runs on memory: the port's world buffers there first (ap_warned)
            ai._sector_resets(World(0), speech=False)   # the library draws speech_wait itself
            port_lib.bufs_load(e, ai, bufs)
            return 0, None
        if op == "disarm_ask":             # op 345: the original's yes_no (text, profile, nickname, 1, 0)
            sp_ = rt(G_SOLDIERS) + 0x200 * args_[0]
            log.append(("yes_no", (DISARM_TEXTS[args_[1]], e.r8(sp_ + 0x179), 1, 0)))
            return take("yes_no"), None
        if op == "ground_item_reset":      # op 346: the original's ItemInit 0x53708 (entry, 0, 0)
            log.append(("w_53708", (rt(ai.G_GROUND_ITEMS) + 12 * args_[0], 0, 0)))
            return take("w_53708"), None
        if op == "menu_redraw":            # op 347: the original's 0x1CB78 with its menu byte
            log.append(("w_1cb78", (e.r8(rt(0x92DD4)),)))
            return take("w_1cb78"), None
        if op == "merc_message" and "merc_message" in allow:   # (slot, msg, p, 0, last): the original's 0 is a pointer
            log.append((op, (args_[0], args_[1] & 0xFF, args_[2] & 0xFF, args_[4])))
            return take(op), None
        if op == "w_54ec8" and PICK_EMPTIES[0]:   # op 344 cursor_to_soldier: the pickup section's stub empties the cursor
            log.append((op, (args_[0],)))
            port_lib.bufs_store(e, ai, bufs)
            e.w8(rt(ai.G_HELD), 0)
            port_lib.bufs_load(e, ai, bufs)
            return take(op), None
        if op == "keep_selected":          # op 94
            e.w8(rt(0x92DC7), args_[0])
            return 0, None
        if op == "sector_tail":            # op 95
            port_lib.bufs_store(e, ai, bufs)  # runs on memory: the port's world buffers there first (ap_warned)
            ai._sector_tail(World(0))
            port_lib.bufs_load(e, ai, bufs)
            return 0, None
        if op not in ("replace", "reserve_ap", "w_53660", "fclose", "replay_event", "explode", "fx_157bc",
                      "radio_result", "action_ap", "quest_check", "shot_ap", "tile_dist", "reach_ap", "path_ap",
                      "direction") and op not in allow:
            # (sector_start.md calls only these)
            raise SystemExit(f"port called host op {op} {args_}: the sector start has no such call")
        if op == "cry":                    # the port passes a cry number (port_api.md, op 100)
            args_ = args_[:2] + [port_lib.CRIES[args_[2]] if args_[2] < 5 else f"cry {args_[2]}"] + args_[3:]
        log.append((op, conv(CALLEES[op][1].replace("s", "i").replace("x", ""), args_)))
        return take(op), None
    r = port_lib.sector_run(name, bufs, scalars, args, random_, call,
                            port_lib.ALL_OPS if host_ops is None else host_ops)
    port_lib.world_store(e, ai, bufs, scalars)
    return r


def port_sector_value(sector, flag):
    """--port: ja1_ai_sector_value on the world from emulator memory (the "slot" argument is the sector)"""
    bufs, scalars = port_lib.world_load(e, ai)

    def call(op, args):
        if op not in CALLEES:
            raise SystemExit(f"port called unknown host op {op} {args}")
        log.append((op, conv(CALLEES[op][1].replace("s", "i").replace("x", ""), args)))   # no table address
        return take(op), None
    return port_lib.ai_run("sector_value", bufs, scalars, sector, None, call, flag)


def run_port(name, port, ps, edx, ebx):
    """the port's call for each tested function (extra register arguments, NoiseTile's EDX output)"""
    if PORT:
        return port_ai(name, ps, edx, ebx)
    w = World(ps[0x17A])
    if name in MEMRUN:
        e.uc.mem_write(rt(G_SOLDIERS) + 0x200 * ps[0x17A], bytes(ps))
        w.memrun = True
        if name == "first_aid":
            r = port(MemRec(ps[0x17A]), w, edx, ebx)
        elif name == "apply_aid":
            r = port(MemRec(ps[0x17A]), MemRec(SP[edx]), w, ebx, ecx_arg)
        else:
            r = port(MemRec(ps[0x17A]), w) if name in ("fire", "aid_tick", "active_frame") else port(MemRec(ps[0x17A]), w, edx)
        ps[:] = e.uc.mem_read(rt(G_SOLDIERS) + 0x200 * ps[0x17A], 0x200)
        return r
    if name == "choose_action":
        return port(ps, w, edx)
    if name == "soldier_power":
        return port(ps, w, edx, ebx)
    if name == "line_of_fire":
        return port(ps, w, ai.s16(edx), ebx)
    if name == "reach_tile":
        return port(ps, w, edx, ebx)
    if name == "click":
        return port(ps, w, ai._i32(edx), ebx)
    if name == "w_5aa08":
        return port(ps, w, ai._i32(edx))
    if name == "w_56dd4":
        return port(ps, w, edx, ebx)
    if name == "impact":
        return port(ps, w, edx, ai._i32(ebx), ai._i32(ecx_arg), stack_arg)
    if name == "take_damage":
        return port(ps, w, edx, ebx, ecx_arg, stack_arg[0], stack_arg[1])
    if name in ("hit_reaction", "hears"):
        return port(ps, w, edx, ebx, ecx_arg)
    if name == "rest":
        return port(ps, w, edx, ebx)
    if name in ("kill", "merc_dies"):
        return port(ps, w, edx)
    if name == "trigger":
        return port(ps, w, edx, ebx, ecx_arg)
    if name == "start_attack":
        return port(ps, w, edx)
    if name == "fire_gun":
        return port(ps, w, ai.s16(edx))
    if name == "chance_to_hit":
        return port(ps, w, ai.s16(edx), ebx & 0xFF)
    if name == "attack_go":
        return port(ps, w)
    if name == "bury_mine":
        return port(ps, w, ai.s16(edx))
    if name in ("mine_flag", "walter"):
        return port(ps, w, ai.s16(edx))
    if name in ("pickup_here", "lock_kit"):
        return port(ps, w)
    if name == "use_on_tile":
        return port(ps, w, ai.s16(edx), ebx)
    if name == "spend_ap":
        return port(ps, w, edx, ebx)
    if name == "stat_practice":
        return port(ps, w, edx, ebx, ecx_arg)
    if name == "land_item":
        return port(ps, w, edx, ebx, ecx_arg, stack_arg[0], stack_arg[1])
    if name in TURN or name in ("frame_checks", "merc_quote", "soldier_frame", "active_frame", "enemies_see", "mercs_see", "step_noise", "stand_up", "xra_enemy", "melee_hit", "turn_rest", "noise_int", "reserve_ap", "gun_jam"):
        return port(ps, w)
    if name == "nearest_merc":
        return port(ps, w, edx)
    if name == "consume_trigger":
        return port(w, edx)
    if name == "throw_scatter":
        return port(ps, w, edx & 0xFF, ebx & 0xFFFF)
    if name == "throw_aim":
        return port(ps, w, ai._i32(edx))
    if name in ("throw_follow", "anim_end"):
        return port(ps, w)
    if name == "find_path":
        return port(ps, w, edx, ebx)
    if name == "path_ap":
        return port(ps, w, edx, ebx, ecx_arg, *stack_arg)
    if name == "w_59ae8":
        return port(ps, w, ai.s16(edx), ai.s16(ebx), ecx_arg, stack_arg)
    if name == "swap_hand":
        return port(ps, w, edx, ebx)
    if name == "attack_ready":
        return port(ps, w, ai._i32(edx))
    if name == "turn_to":
        return port(ps, w, edx, ebx)
    if name == "walk_to":
        return port(ps, w, edx)
    if name == "walk_blocked":
        return port(ps, w, ai._i32(edx), ebx & 0xFF)
    if name == "walk_end":
        return port(ps, w, edx)
    if name == "set_anim":
        return port(ps, w, edx, ebx, ecx_arg)
    if name in ("anim_turn", "stop_anim", "w_197d8", "stop_walk"):
        return port(ps, w, edx)
    if name == "msg_not_enough_ap":
        return port(ps, w, edx, ebx)
    if name in ("hit_alert", "interrupt_check"):
        return port(ps, w, SP[edx], ebx)
    if name in CONTEST:
        return port(ps, w, SP[edx])
    if name in PAIR:
        return port(ps, w, SP[edx])
    if name == "share_sightings":
        return port(ps, w, edx)
    if name in ("update_sightings", "approach_tile", "advance_tile", "can_stand_at", "search_tile"):
        return port(ps, w, edx)
    if name == "noise_tile":
        tile, score = port(ps, w)
        e.w16(rt(OUT), score)
        return tile
    return port(ps, w)


PROBE = {"think", "update_alert", "choose_action", "calm_action", "alert_action", "combat_action", "engaged_action",
         "action_affordable", "do_action", "noise_tile", "find_dry_tile", "patrol_tile", "news_value",
         "approach_tile", "ally_call_tile", "can_stand_at", "advance_tile", "target_tile", "opp_strength",
         "soldier_power", "retreat_tile", "search_tile", "gun_option", "knife_option", "throw_option",
         "line_of_fire"}     # deciders whose cases sometimes sit on an edge (probe_edge)
PROBE_REC = [(0xBA, 1), (0xBB, 1), (0xF0, 2), (0x131, 1), (0x183, 1), (0x184, 1), (0x18A, 1), (0x18F, 1), (0x1A1, 1),
             (0x1A2, 1), (0x1A5, 1), (0x1C1, 1), (0x1CF, 1), (0x1D1, 1), (0x1D3, 1), (0x1D4, 1), (0x1D5, 1), (0x1D6, 1),
             (0x9C, 1), (0x1B2, 1)]
PROBE_OPS = ["action_ap", "move_ap", "opp_strength", "tile_dist", "news_value", "sector_value", "reach_ap",
             "attack_ready", "path_ap", "radio_result", "busy", "turn", "can_stand_at", "fire_line", "shot_ap",
             "chance_to_hit", "direction"]


def orig_sig(addr, sp, s, base, edx, ebx):
    """one run of the original on record s + script base; returns (signature of all it changed, call log).
    Everything it may write is put back."""
    global script
    keep = (bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000)), bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)),
            [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W])
    e.uc.mem_write(sp, bytes(s))
    script = {k: list(v) for k, v in base.items()}
    log.clear()
    unstub_ = e.stubs.pop(rt(addr), None)
    try:
        r = e.call(rt(addr), sp, edx, ebx, 0, max_steps=50_000_000) & 0xFF
        sig = hash((r, bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000)), bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)),
                    tuple(bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W), repr(log)))
    except emu.EmuStop as x:
        sig = hash(str(x))
    finally:
        if unstub_:
            e.stubs[rt(addr)] = unstub_
        e.uc.mem_write(rt(G_SOLDIERS), keep[0])
        e.uc.mem_write(rt(G_TILEMAP), keep[1])
        for (a, k), b in zip(GLOBALS_W, keep[2]):
            e.uc.mem_write(rt(a), b)
    return sig, list(log)


def probe_edge(addr, sp, s, base, edx, ebx, only=None):
    """put the case on an edge of the original: one input (a Random result, a record field or a host call's return)
    is bisected until the original's behaviour changes, then set to the value on one side of it. A rule copied with
    an off-by-one (< for <=, 50 for 51) then shows up, which a random input hits about once in a hundred."""
    s, base = bytearray(s), {k: list(v) for k, v in base.items()}
    _, calls = orig_sig(addr, sp, s, base, edx, ebx)
    dims = [("Random", j, 0, a[0] - 1) for j, a in enumerate(v for n_, v in calls if n_ == "Random") if a[0] > 1][:8]
    for op in PROBE_OPS:
        dims += [(op, j, 0, 300) for j in range(min(sum(1 for n_, _ in calls if n_ == op), 4))]
    dims += [("rec", o, 0, 0xFFFF if w == 2 else 0xFF) for o, w in PROBE_REC]
    dims = [d for d in dims if only in (None, d[0])] or dims
    kind, j, lo, hi = rng.choice(dims)

    def put(v):
        if kind == "rec":
            s[j] = v & 0xFF
            if j == 0xF0:
                s[j + 1] = v >> 8
        else:
            q = base.setdefault(kind, [])
            q += [0] * (j + 1 - len(q))
            q[j] = v

    def sig(v):
        put(v)
        return orig_sig(addr, sp, s, base, edx, ebx)[0]
    old = (s[j], s[j + 1]) if kind == "rec" else None
    if kind != "rec":
        old = list(base.get(kind, []))
    spans = [(lo, hi), tuple(sorted(rng.sample(range(lo, hi + 1), 2)))]
    if kind == "rec" and hi == 0xFF:     # signed bytes: 0 and 0xFF (-1) are both "low", so try each half too
        spans += [(0, 0x7F), (0x80, 0xFF)]
    rng.shuffle(spans)
    edge = False
    for a_, b_ in spans:
        s_lo = sig(a_)
        if s_lo != sig(b_):
            lo, hi, edge = a_, b_, True
            break
    if edge:
        while hi - lo > 1:
            mid = (lo + hi) // 2
            if sig(mid) == s_lo:
                lo = mid
            else:
                hi = mid
        put(rng.choice([lo, hi]))
    elif kind == "rec":
        s[j], s[j + 1] = old
    else:
        base[kind] = old
    e.uc.mem_write(sp, bytes(s))
    return s, base


ANIMTBL = bytes(e.uc.mem_read(rt(ai.ANIM_CAN_ACT) - 8, 8 * 256))   # rows from animation -1 (delay / AP columns)
ANIMFREE = bytes(e.uc.mem_read(rt(ai.ANIM_FREE), 0x40C))      # free flags, sounds (restored every case)
SCRIPTS = bytes(e.uc.mem_read(rt(ai.ANIM_FRAMES), 68 * 0x50))      # animation script table (restored every case)
for t in range(136):
    KIND.setdefault(e.r8(rt(ai.G_ITEM_KIND) + 12 * t), []).append(t)
fails, counts, results, hangs = 0, {}, {}, {}
names = [k for k in DECIDERS if k in ONLY] if only else [k for k in (port_lib.AI_ENTRY if PORT else DECIDERS)
                                                             if k in DECIDERS and k not in ALONE]
SHARE_CLICK = 20
n_dec = n
if GROUP is not None and names and not only:      # the group's deciders, each with as many cases as in the whole run
    names_ = [k for k in names if group_of(k) == GROUP[0]]
    n_dec = n * len(names_) // len(names)
    names = names_
if not only and "click" in names:                 # (issue #10: ~100 click cases in the whole run reach 72 % of the
    k_ = len(names)                               # port's Ai::click, 2000 reach 97 %; click cases cost ~2 ms)
    names = names + ["click"] * (SHARE_CLICK - 1)
    n_dec = n_dec * len(names) // k_
BASE_MEM.append(bytes(e.uc.mem_read(0, len(e.dump.mem))))
sees0 = None
uot_keep = []          # use_on_tile: the tileset and cur_sector before its case (the sections take turns case by case)
for i in range(n_dec if names else 0):
    name = names[i % len(names)]
    for a_, b_ in uot_keep:
        e.uc.mem_write(rt(a_), b_)
    uot_keep = []
    clock(name)
    addr, port = DECIDERS[name]
    e.uc.mem_write(rt(ai.ANIM_FRAMES), SCRIPTS)
    e.uc.mem_write(rt(ai.ANIM_FREE), ANIMFREE)
    side = name in SIDE
    slot = rng.choice([rng.randrange(0x28, 0x30), rng.randrange(16)]) if side else rng.randrange(0x28, 0x30)
    if name in CONTEST:                  # a merc / guard against an enemy, or the other way round
        slot = rng.randrange(16) if name == "merc_interrupt" else rng.randrange(0x28, 0x30)
    sp = rt(G_SOLDIERS) + slot * 0x200
    if side:
        fuzz_side(slot)
    if name == "busy":                     # the animation word: often one of the busy ones or a neighbour
        i_ = rng.choice([rng.randrange(12), rng.randrange(256)])
        e.w8(sp + 0x1F3, i_)
        if i_ < 12:                        # (a word past the record stays as it is: a write there outlives the case)
            w_ = rng.choice([0x1A, 0x1B, 0x1C, 0x1E, 0x1F, 0x20, 0x22, 0x23, 0x24, 0x11B, rng.randrange(0x10000)])
            e.uc.mem_write(sp + 0x1E8 + 2 * i_, struct.pack("<H", w_))
    if name == "swap_hand":                # the selected soldier gets the item panel redrawn
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, slot, rng.randrange(0x30), 0x40]))
    s = bytearray(e.uc.mem_read(sp, 0x200))
    fuzz_soldier(s)
    tile = rng.randrange(3136)
    if name == "move_ap_cost":           # edges and corners often; neighbours get random move codes
        tile = rng.choice([tile, 56 * rng.randrange(56), 56 * rng.randrange(56) + 55, rng.randrange(57),
                           rng.randrange(0xC08, 3136), 0, 55, 3080, 3135, ai.NO_TILE])
        for st in ai.DIR_STEP_1 + (0,):
            if 0 <= tile + st < 3136:
                e.w16(rt(ai.G_MOVECODE) + 2 * (tile + st), rng.choice(MOVE_CODES))
        if 0 <= tile < 3136:
            f68_bridges(tile)
    ai.w16(s, 0x114, tile)
    e.uc.mem_write(sp, bytes(s))
    e.w8(rt(G_TILEMAP) + 8 * tile + 5, rng.choice([0x40, 0xC0, 0x80 | slot, slot]))
    e.w8(rt(ai.G_ALARM), rng.choice([0, 0, 1]))
    e.w8(rt(ai.G_COMBAT), rng.choice([0, 0, 1]))
    sector = e.r8(rt(ai.G_CUR_SECTOR))
    e.w8(rt(ai.G_SECTOR_TBL9) + 9 * sector + 1, rng.choice([0, 0, 1, 5]))
    e.w8(rt(ai.G_MISSION), rng.choice([0, 0, 1, 2, 0xFF]))
    e.w8(rt(ai.G_MISSION_DONE) + sector, rng.choice([0, 1]))
    e.w8(rt(ai.G_MISSION_ORDER), rng.choice([slot, slot, 0x40, rng.randrange(256)]))
    e.w16(rt(ai.G_MISSION_ORDER) + 2, rng.choice([ai.NO_TILE, rng.randrange(3136), rng.randrange(0x10000)]))
    e.w16(rt(ai.G_MISSION_SPOT), rng.choice([tile, tile, rng.randrange(3136)]))
    e.w8(rt(ai.G_DIFFICULTY), rng.choice([0, 1, 2]))
    e.w32(rt(ai.G_TURN_BASED), rng.choice([0, 1, 1]))
    e.w32(rt(ai.G_BUSY), rng.choice([0, 0, 1, 0xFFFFFFFF]))
    e.w8(rt(ai.G_ACTIVE), rng.choice([0x40, 0x40, 3]))
    e.w8(rt(ai.G_SANTINO_SPOKE), rng.choice([0, 1]))
    e.w16(rt(ai.G_SANTINO_TIMER), rng.choice([0, 19, 20, 0x8000]))
    e.w8(rt(ai.G_SHOW_AI), rng.choice([0, 1]))
    e.w32(rt(0x22A008), rng.randrange(1 << 32))
    for k in rng.sample(list(range(16)) + list(range(0x28, 0x30)), 4):
        if k != slot:
            e.w8(rt(G_SOLDIERS) + 0x200 * k + 0x183, rng.choice([0, 14, 15, 50, 100]))
    gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]

    base = scripted(name)
    if name == "engaged_action" and TIE_RNG.random() < 0.3:   # made cases: the knife / throw option scores equal the gun's
        for k_ in range(4):                                    # (the gun wins a tie: jge at the compare; ledger row 1465)
            for o_ in ("knife_option", "throw_option"):
                base[o_][k_] = base[o_][k_][:4] + base["gun_option"][k_][4:8] + base[o_][k_][8:]
        if TIE_RNG.random() < 0.7 and KIND.get(0x92) and KIND.get(0x95) and KIND.get(0x96):   # a gun and a knife / thrown item
            s[0x00], s[0x0C] = TIE_RNG.choice(KIND[0x92]), TIE_RNG.choice(KIND[TIE_RNG.choice([0x95, 0x96, 0x96])])
            e.uc.mem_write(sp, bytes(s))
            gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    script = {k: list(v) for k, v in base.items()}
    log.clear()
    edx = rng.randrange(1 << 32)
    ebx = rng.choice([0, 1, 0x100])
    if name == "noise_tile":
        edx = rt(OUT)
        ai.w16(s, 0x122, rng.choice([ai.u16(s, 0x114), rng.randrange(3136), ai.NO_TILE, rng.randrange(0x10000)]))
        s[0x1C0] = rng.choice([0, 12, 13, 40, rng.randrange(256)])
        e.uc.mem_write(sp, bytes(s))
    if name == "swap_hand":                # ai.md Swap hands: the library passes (0, 6) only
        edx, ebx = rng.choice([(0, 6)] * 4 + [(6, 0), (0, 0), (rng.randrange(7), rng.randrange(7))])
    if name == "advance_tile":
        ai.w16(s, 0x122, rng.choice([ai.u16(s, 0x114), rng.randrange(3136), ai.NO_TILE, ai.NO_TILE]))
        e.uc.mem_write(sp, bytes(s))
        edx = rng.choice([0, 1, 0x100])
    if name == "buddy_tile":
        s[0xBA] = rng.choice([0, 1, 2, 3, 4])
    if sees0 is not None:           # (put back after a guard_turned case)
        e.w8(rt(0x92DC6), sees0)
        sees0 = None
    if name == "guard_turned":      # sees_slot 0x92DC6 = this soldier's slot byte in a third of the cases (the message
        sees0 = e.r8(rt(0x92DC6))   # branch); own random source
        if random.Random(1000003 * i + 64).random() < 0.33:
            e.w8(rt(0x92DC6), s[0x17A])
    if name in ("knife_option", "throw_option"):   # the helpers divide by skills / max hp: keep them sane
        hand = [0x0F, 0x10, 0x0F] if name == "knife_option" else [0x16, 0x18, 0x1A, 0x1C] + KIND.get(0x95, [])
        for k in range(0x30):
            a = rt(G_SOLDIERS) + 0x200 * k
            p = bytearray(e.uc.mem_read(a, 0x200))
            for o in (0x18B, 0x18C, 0x191, 0x192, 0x184):
                p[o] = rng.randrange(1, 101)
            p[0x183] = rng.choice([p[0x184], rng.randrange(1, p[0x184] + 1)])
            p[0x185] = rng.choice([0, 0, rng.randrange(10)])
            p[0x18A] = rng.choice([100, rng.randrange(1, 101)])
            p[0x1B9] = rng.choice([0, 4])
            p[0x00] = rng.choice(hand + [rng.randrange(136)])
            p[0x01] = rng.choice([1, 1, 2, 3, 4])
            p[0x02] = rng.choice([100, rng.randrange(256)])
            p[0x08:0x0C] = bytes(rng.choice([100, rng.randrange(256)]) for _ in range(4))
            p[0x24] = rng.choice([0, 0x27, 0x28, p[0x24]])
            p[0x26] = rng.choice([100, 50, rng.randrange(256)])
            e.uc.mem_write(a, bytes(p))
        s[:] = e.uc.mem_read(sp, 0x200)
        s[0xBA] = rng.choice([1, 2, 3, 3, 4])
        s[0x1A5] = rng.choice([0, 3, 5, 8, 12, 20, 40])
        for j in range(0x28):
            s[0x13A + j] = rng.choice([1, 1, 0, 2, 0xFF])
        ai.w16(s, 0x114, tile)
        s[0x17A] = slot
        e.uc.mem_write(sp, bytes(s))
        for k in range(0x30):                 # some opponents right next to the soldier
            if k != slot and rng.random() < 0.3:
                e.w16(rt(G_SOLDIERS) + 0x200 * k + 0x114, (tile + rng.choice([1, -1, 56, -56, 57])) % 3136)
        if name == "throw_option" and rng.random() < 0.5:   # last target among the candidate tiles (ThrowChance +10)
            o = rng.choice(list(range(0x28)) if slot >= 0x28 else list(range(0x28, 0x30)))
            ot = e.r16(rt(G_SOLDIERS) + 0x200 * o + 0x114)
            ai.w16(s, 0xD0, (ot + rng.choice([0, 0, 1, -1, 2, 56, -56, 57, 112, 58])) % 3136)
            e.uc.mem_write(sp, bytes(s))
        if name == "throw_option" and FAN and 58 in FAN and F58_RNG.random() < 0.6:
            f58_edge(s, slot, base)
            script = {k: list(v) for k, v in base.items()}
            e.uc.mem_write(sp, bytes(s))
        e.uc.mem_write(OUTREC, bytes(rng.randrange(256) for _ in range(10)))
        edx = OUTREC
    if name == "line_of_fire":
        s[0x00] = rng.choice(GUNS + [0x16, 0x18, 0x1C] + KIND.get(0x95, []))
        s[0x1B3] = rng.choice([0, 3, 3, 1])
        ai.w16(s, 0x116, ai.tile_x(tile) + rng.choice([0, 0, 1, -3]))
        ai.w16(s, 0x118, ai.tile_y(tile) + rng.choice([0, 0, 2, -1]))
        e.uc.mem_write(sp, bytes(s))
        edx = rng.choice([tile, rng.randrange(3136), rng.randrange(3136),
                          (tile + rng.choice([1, 2, 57, -56, 5 * 56 + 3])) % 3136])
        ebx = rng.choice([0, 0, 1])
    if name == "opp_strength" and rng.random() < 0.5:   # the adjust steps on their edges
        s[0x18A] = rng.choice([76, 75, 74, 41, 40, 39, 11, 10, 9])
        s[0x183] = rng.choice([76, 75, 41, 40, 39, 21, 20, 19, 15])
        e.uc.mem_write(sp, bytes(s))
    if name == "gun_option":
        s[0x00] = rng.choice(GUNS)
        s[0xBA] = rng.choice([1, 2, 3, 3, 4])
        s[0x1A5] = rng.choice([0, 3, 5, 8, 12, 20, 40, rng.randrange(256)])
        for j in range(0x28):
            s[0x13A + j] = rng.choice([1, 1, 0, 2, 0xFF])
        e.uc.mem_write(sp, bytes(s))
        e.uc.mem_write(OUTREC, bytes(rng.randrange(256) for _ in range(10)))
        edx = OUTREC
    if name == "search_tile":
        s[0xBB] = rng.choice([2, 3, 3, s[0xBB]])
        s[0x1D4] = rng.choice([0, 1, 2, 3, 4, 5])
        ai.w16(s, 0x12E, rng.choice([tile, rng.randrange(3136)]))
        e.uc.mem_write(sp, bytes(s))
        edx = rng.choice([0, 1, 2, 0x101, rng.randrange(256)])
    if name == "retreat_tile":
        ai.w16(s, 0x116, ai.tile_x(tile) + rng.choice([0, 0, 3, -2]))
        ai.w16(s, 0x118, ai.tile_y(tile) + rng.choice([0, 0, 3, -2]))
        s[0xBB] = rng.choice([2, 3, 3, s[0xBB]])
        e.uc.mem_write(sp, bytes(s))
    if name == "can_stand_at":
        edx = rng.choice([tile, rng.randrange(3136), rng.randrange(3136), ai.u16(s, 0x10E)])
        if PORT and edx >= 3136:      # off the map the original reads past the tile map; the port reads 0 there
            edx = rng.randrange(3136)
        fuzz_tile(edx)
        if rng.random() < 0.3:
            ai.w16(s, 0x10E, edx)
        s[0x24] = rng.choice([s[0x24], 0x28])
        s[0xBA] = rng.choice([s[0xBA], 1, 1])
        e.uc.mem_write(sp, bytes(s))
    if name in ("patrol_tile", "approach_tile", "buddy_tile"):
        ai.w16(s, 0x12E, rng.choice([rng.randrange(3136), 0, 55, 3080, 3135, 56 * rng.randrange(56), tile]))
        s[0x1D4] = rng.choice([0, 1, 2, 3, 4, 5, rng.randrange(256)])
        ai.w16(s, 0xD4, rng.choice([0, 1, 5, 20, 80]))
        e.uc.mem_write(rt(ai.G_PATHS) + 0x50 * slot, bytes(rng.choice([0, 1, 2, 3, 4, 5, 6, 7, 7, 9]) for _ in range(0x50)))
        e.uc.mem_write(sp, bytes(s))
        edx = rng.choice([ai.u16(s, 0x114), rng.randrange(3136), rng.randrange(0x10000), ai.u16(s, 0x12E)])
    if name in PAIR:                     # mostly an opponent; same side or an off-table slot byte now and then
        k = rng.choice([rng.randrange(16), rng.randrange(0x28, 0x30)] + [rng.randrange(16) if slot >= 0x28 else
                                                                         rng.randrange(0x28, 0x30)] * 6)
        k = k if k != slot else (k + 1) % 0x30
        if name in CONTEST:
            k = rng.randrange(0x28, 0x30) if slot < 0x28 else rng.randrange(16)
        oa = rt(G_SOLDIERS) + 0x200 * k
        o = bytearray(e.uc.mem_read(oa, 0x200))
        o[0x9C] = rng.choice([0, 1, 0xFF, 5])
        o[0x13A] = rng.choice([1, 1, 0, 2, 0xFF])
        ai.w16(o, 0x116, rng.randrange(-20, 900))
        ai.w16(o, 0x118, rng.randrange(-20, 800))
        if rng.random() < 0.02 and name not in CONTEST | {"hit_alert", "interrupt_check"}:    # these index their records by it: on-table
            o[0x17A] = rng.choice([0x30, 0x40, 0xFF])
        e.uc.mem_write(oa, bytes(o))
        s[0xBA] = rng.choice([s[0xBA], 0, 1, 2, 3, 4, 4])
        s[0x9C] = rng.choice([0, 1, 0xFF, s[0x9C]])
        s[0xBB] = rng.choice([0, 1, 2, 3, rng.randrange(256)])
        s[0x1D5] = rng.choice([0, 1, 1, 2, 0xFF, rng.randrange(256)])
        s[0x1D4] = rng.choice([0, 1, 2, 3])
        s[0x18F] = rng.choice([99, 99, 50])
        s[0x1D1] = rng.choice([3, 4, 0, 1, 2, 5, 11])
        s[0x1F3] = rng.choice([0, 1, 2, 3, 4])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([3, 4, 0x15, 0x1B, 0, 0x104, rng.randrange(0x10000)]))
        s[0x1A1] = rng.choice(list(range(1, 9)) * 3 + [0, rng.randrange(256)])
        ai.w16(s, 0x116, rng.randrange(-20, 900))
        ai.w16(s, 0x118, rng.randrange(-20, 800))
        j = k if slot >= 0x28 else k - 0x28
        if 0 <= j < 0x28:
            s[0x13A + j] = rng.choice([0, 0, 1, 1, 2, 3, 4, 0xFF, 0xFE, rng.randrange(256)])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_KNOWN) + (k & 0x3F), rng.choice([1, 1, 2, 0, 0xFF]))
        s[0x1C1] = rng.choice([0, 0, 1])
        e.uc.mem_write(sp, bytes(s))
        if 0 <= j < 0x28 and k < 0x30:     # remember byte and last-seen tile of this pair
            rem = ai.G_REMEMBER_E + (slot - 0x28) * 0x28 + k if slot >= 0x28 else ai.G_REMEMBER_M + slot * 8 + j
            seen = ai.G_LAST_SEEN_E + (slot - 0x28) * 0x50 + 2 * k if slot >= 0x28 else ai.G_LAST_SEEN_M + slot * 16 + 2 * j
            e.w8(rt(rem), rng.choice([0, 1, 0xFF, 2]))
            e.w16(rt(seen), rng.choice([ai.u16(o, 0x114), ai.u16(o, 0x114), e.r16(rt(seen))]))
        if k < 0x30:      # no_ally_sees: allies flagging the same opponent
            for a in rng.sample(range(16) if s[0xBA] < 3 else range(0x28, 0x30), 3):
                if a != slot and 0 <= j < 0x28:
                    e.w8(rt(G_SOLDIERS) + 0x200 * a + 0x13A + j, rng.choice([1, 1, 2, 0]))
        if name in CONTEST:
            s[0x1A5] = rng.choice([0, 1, 2, 4, 5, 10, 0x80, 0xFF])
            for r_ in (s, o):
                r_[0x192] = rng.choice([1, 2, 3, 4, 5, 6, 8, 10, 0x7F, 0x80, rng.randrange(256)])
                r_[0x1D5] = rng.choice([0, 0, 1, 2, 3, 0xFF, rng.randrange(256)])
            j_ = k - 0x28 if slot < 0x28 else slot - 0x28          # the enemy's index
            m_ = slot if slot < 0x28 else k                      # the merc's slot
            s[0x13A + (j_ if slot < 0x28 else m_)] = rng.choice([1, 1, 0, 2, 0xFF])
            o[0x13A + (m_ if slot < 0x28 else j_)] = rng.choice([1, 1, 1, 0, 2, 0xFF])
            s[0xBA] = rng.choice([s[0xBA], 4, 0, 3])
            e.uc.mem_write(oa, bytes(o))
            e.uc.mem_write(sp, bytes(s))
            e.w8(rt(ai.G_REMEMBER_M + 8 * m_ + j_), rng.choice([0, 1, 1, 0xFF]))
            e.w8(rt(ai.G_REMEMBER_E + 0x28 * j_ + m_), rng.choice([0, 1, 1, 0xFF]))
            e.w8(rt(ai.G_KNOWN + m_), rng.choice([0, 0, 1, 0xFF]))
            e.w8(rt(ai.G_HIT_SCALE), rng.choice([0, 0, 1, 4, 25]))
            e.w8(rt(ai.G_DIFFICULTY), rng.choice([0, 1, 2]))
            e.w8(rt(ai.G_SHOW_UI), rng.choice([0, 0, 1]))
            e.w8(rt(ai.G_INTERRUPTED), rng.choice([0, 1]))
            e.w8(rt(ai.G_SELECTED), rng.choice([0, 3, 7, 8, 0x2A, 0x40]))
            e.w8(rt(ai.G_ITQ_N), rng.choice([0, 1, 5]))
            e.w8(rt(0x22A0CD), rng.choice([0, 1]))
            e.w8(rt(0x284F0F), rng.choice([0, 5, 8, 0x40]))
        if name == "interrupt_check":
            s[0x18A] = rng.choice([0, 1, 1, 1])
            ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([0, 3, 4, 5, 0x28, 0xFFFF]))
            s[0x1A5] = rng.choice([0, 1, 2, 4, 5, 10, 0x80, 0xFF])
            s[0x1D5] = rng.choice([0, 0, 1, 0xFF])
            fl = (0x112 + k) if k >= 0x28 else (0x13A + k)
            if fl < 0x200:
                s[fl] = rng.choice([1, 1, 0, 2, 0xFF])
            ofl = (0x13A + slot) if k >= 0x28 else (0x112 + slot)
            if ofl < 0x200:
                o[ofl] = rng.choice([1, 1, 0, 2, 0xFF])
                e.uc.mem_write(oa, bytes(o))
            e.uc.mem_write(sp, bytes(s))
            ebx = rng.choice([0, 1, 2, 3, 0xFF, 0x1FF])
            e.w8(rt(0x92DCF), rng.choice([0, 1]))
            e.w8(rt(0x92DD0), rng.choice([0, 1, 1]))
            e.w8(rt(ai.G_INTERRUPTED), rng.choice([0, 1]))
            e.w8(rt(ai.G_SELECTED), rng.choice([0, 3, 7, 8, 0x40]))
        edx = oa
        osave = bytes(e.uc.mem_read(oa, 0x200))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "reach_tile":
        edx = rng.choice([rng.randrange(3136), 0, 55, 3080, 3135, 56 * rng.randrange(56), 56 * rng.randrange(56) + 55,
                          rng.randrange(56), rng.randrange(3080, 3136)])
        for st in (0, -56, 1, 56, -1):
            if 0 <= edx + st < 3136:
                fuzz_tile(edx + st)
        e.w8(rt(G_TILEMAP) + 8 * edx + 1, rng.choice([0x37, 0x50, 0x82, 0x83, 0x36, e.r8(rt(G_TILEMAP) + 8 * edx + 1)]))
        ebx = rng.choice([0, 0, 1, 5, 0xFD, 0xFE, 0xFF, 0x100])
        s[:] = e.uc.mem_read(sp, 0x200)
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    ecx_arg = 0
    stack_arg = None
    if name in TURN:
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x1C4] = rng.choice([0, 1, 0xFF, 2])
        if not 1 <= s[0x1A1] <= 8:      # an earlier decider may leave any facing; MercView (ViewDirs) reads stack bytes then
            s[0x1A1] = 1 + s[0x1A1] % 8
        s[0x1B9] = rng.choice([0, 0, 0, 1, 4, 6])
        for o in (0x1E1, 0x1E2, 0x1E3, 0x16D, 0x1DB):
            s[o] = rng.choice([0, 1, 5])
        if name == "revive_roll":
            s[0] = rng.choice([0, 0, 0xF, 0x10, 0x11, 3])
            for i_ in range(1, 7):
                idx = e.r8(rt(ai.POCKET_SLOTS) + i_)
                s[12 * idx] = rng.choice([0, 0, 0xF, 0x10, 0xE, 5])
            s[0x18C] = rng.choice([0, 10, 50, 80, 100, 101, 110, 127, 0x80, 0xFF, rng.randrange(256)])
            s[0x18A] = rng.choice([0, 30, 70, 100, 0x80, 0xFF, rng.randrange(256)])
            s[0x179] = rng.choice([0x3B, 0x3C, 5, 0x40, 0x50])
            s[0xBA] = rng.choice([0, 1, 3])
            s[0x188] = rng.choice([0, 0, 0, 1])
        s[0x184] = rng.choice([s[0x184], 15, 16, 30, 60, 100])
        s[0x183] = rng.choice([0, 1, 9, 10, 14, 15, 20, s[0x184], rng.randrange(s[0x184] + 1)])
        s[0x185] = rng.choice([0, 0, 1, 5, 12, 13, 20, (s[0x184] - s[0x183]) & 0xFF])
        s[0x186] = rng.choice([0, 0, 1, 0xFF, 2])
        s[0x187] = rng.choice([0, 0, 1])
        s[0x18A] = rng.choice([s[0x18A], 0, 0, 50, 100])
        s[0x1BA] = rng.choice([0, 1])
        s[0x19C] = rng.choice([0, 5, 3])
        s[0x1F2] = rng.choice([0, 1, 5, 0x80])
        ai.w16(s, 0x1F0, rng.choice([0, 1, 1, 2, 5, 0xFFFF]))
        s[0x1F3] = rng.randrange(5)
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([0, 4, 0x15, 0x28, 0x38, 0x39, 0x3B, 0x40]))
        s[0x9C] = rng.choice([0, 1, 0xFF])
        ai.w16(s, 0x122, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
        s[0x1C0] = rng.choice([0, 1, 2, 5])
        s[0x1A5] = rng.choice([0, 3, 5, 6, 20, 0x80])
        s[0x17E] = rng.choice([0, 8, 8, 3])
        ai.w16(s, 0x120, rng.choice([ai.NO_TILE, ai.u16(s, 0x114), rng.randrange(3136)]))
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 1]))
        e.w8(rt(0x92DCC), rng.choice([0, 1]))
        e.w8(rt(0x22A057), rng.choice([e.r8(rt(ai.G_CUR_SECTOR)), e.r8(rt(ai.G_CUR_SECTOR)), 0x80 | e.r8(rt(ai.G_CUR_SECTOR)), 5]))
        e.w8(rt(0x22A056), rng.choice([0, 0, 1]))
        QUIET_N[0] += 1
        if name == "start_turn" and random.Random(f"quiet{QUIET_N[0]}").random() < 0.5:
            # No merc with an order (0x1F6E4 answers 0): step 11, the view of the story sector at the turn start,
            # runs only then; the dumps hold a merc with +0x1C4 set, so every case skipped it (Insert #12271).
            for k_ in range(8):
                for o_ in (0x1D5, 0x1C4):
                    e.w8(rt(G_SOLDIERS) + 0x200 * k_ + o_, 0)
            s[0x1D5] = s[0x1C4] = 0           # (the port reads the own record from s)
            if FAN and 39 in FAN:             # #70 F39: then only the duty byte +0x17E decides MercBusy? (1.15m)
                k39 = F39_RNG.choice([None, None] + list(range(8)))
                for k_ in range(8):
                    a_ = rt(G_SOLDIERS) + 0x200 * k_
                    e.w8(a_ + 0x17E, 0 if k_ != k39 else F39_RNG.choice([8, 3, 1, 0xFF]))
                    if k_ == k39:
                        e.w8(a_ + 0x182, F39_RNG.choice([1, 1, 0]))
                        e.w8(a_ + 0x138, 1)
                if slot < 8:
                    s[0x17E], s[0x182], s[0x138] = (e.r8(rt(G_SOLDIERS) + 0x200 * slot + o_) for o_ in (0x17E, 0x182, 0x138))
                if F39_RNG.random() < 0.8:    # step 11 asks MercBusy? only in the story sector (story 0x17, 0x16 = 0)
                    e.w8(rt(0x22A057), e.r8(rt(ai.G_CUR_SECTOR)))
                    e.w8(rt(0x22A056), 0)
                    s[0x183] = max(s[0x183], F39_RNG.choice([15, 40]))
                    mt_ = ai.u16(s, 0x114) % 3136     # the view's tiles not seen yet (byte 4 bit 0x80), so the
                    for dy_ in range(-8, 9):           # step shows on the map
                        for dx_ in range(-8, 9):
                            t_ = mt_ + dy_ * 56 + dx_
                            if 0 <= t_ < 3136:
                                a_ = rt(G_TILEMAP) + 8 * t_ + 4
                                e.w8(a_, e.r8(a_) & 0x7F)
            e.uc.mem_write(sp, bytes(s))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name in ("aid_tick", "apply_aid"):   # a medic (mostly a merc) bandaging a patient on its partner tile
        slot = rng.choice([rng.randrange(8)] * 4 + [rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x1B9] = rng.choice([5, 5, 5, 6, 6, 4, 7, 0])
        s[0x1F3] = rng.randrange(4)
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([0xF] * 6 + [0xE, 0x10, 0x10F]))
        s[0] = rng.choice([0x4A, 0x4A, 0x49, 0x4B, rng.randrange(136)])
        s[1] = rng.choice([0, 1, 1, 2, 3, 5, 0x80])
        for o in (2, 8, 9, 10, 11):
            s[o] = rng.choice([0, 1, 2, 5, 20, 50, 99, 100, 0x80, 0xFF])
        for o in (0x18C, 0x18E):
            s[o] = rng.choice([0, 1, 30, 60, 99, 100, 0x80])
        s[0x192] = rng.choice([0, 1, 2, 5, 10])
        s[0x1A5] = rng.choice([0, 1, 4, 10, 20, 25, 0x80, 0xFF])
        k = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(0x30)])
        k = k if k != slot else (k + 1) % 8
        ptile = rng.randrange(3136)
        ai.w16(s, 0xD6, rng.choice([ptile] * 12 + [ai.NO_TILE]))
        e.w8(rt(G_TILEMAP) + 8 * ptile + 5, rng.choice([k] * 12 + [0x40, 0x30, 0x80 | k]))
        pa = rt(G_SOLDIERS) + 0x200 * k
        pr = bytearray(e.uc.mem_read(pa, 0x200))
        pr[0x17A] = k
        pr[0x188] = rng.choice([0] * 6 + [1])
        pr[0x183] = rng.choice([0, 1, 5, 9, 14, 15, 16, 40, 0x90, 0xF0])
        pr[0x185] = rng.choice([0, 0, 1, 3, 10, 30, 0x80, 0xFF])
        pr[0x186] = rng.choice([0, 0, 1, 0xFF])
        pr[0x187] = rng.choice([0, 1])
        pr[0x1B9] = rng.choice([4, 4, 4, 6, 6, 5, 0])
        if name == "aid_tick" and rng.random() < 0.4:   # every gate open, a strong medic: the need / supply bounds of Apply first aid
            s[0x1B9] = rng.choice([5, 6])
            ai.w16(s, 0x1E8 + 2 * s[0x1F3], 0xF)
            ai.w16(s, 0xD6, ptile)
            e.w8(rt(G_TILEMAP) + 8 * ptile + 5, k)
            s[0x1A5] = rng.choice([10, 20, 25])
            s[0x18C], s[0x18E] = rng.choice([30, 60, 99]), rng.choice([30, 60, 99])
            s[0x192] = rng.choice([1, 5, 10])
            s[0], s[1] = rng.choice([0x4A, 0x4A, 0x49]), rng.choice([1, 1, 2])
            s[2], s[8] = rng.randrange(1, 25), rng.randrange(1, 25)
            pr[0x188], pr[0x1B9] = 0, rng.choice([4, 6])
            pr[0x183] = rng.choice([1, 5, 9, 12, 14])
            pr[0x185] = rng.choice([3, 10, 20, 30, 40])
        e.uc.mem_write(pa, bytes(pr))
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(0x92DCB), rng.choice([0, 0, 1]))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, slot, 0x40]))
        if name == "apply_aid":           # EAX = the patient, EDX = the medic
            slot, k, sp, pa, s = k, slot, pa, sp, pr
            edx = pa
            ebx = rng.choice([0, 1, 2, 5, 50, 100, 101, 300, 0xFFFFFFFF, rng.randrange(1 << 32)])
            ecx_arg = rng.choice([ebx, min(ebx, 100), 0, 100, rng.randrange(1 << 32)])
        gsave = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in GLOBALS_W]
    if name == "first_aid":               # a merc starting to bandage the soldier on a tile (5) or itself (6)
        slot = rng.choice([rng.randrange(8)] * 4 + [rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x1B9] = rng.choice([0, 0, 0, 0, 4, 4, 5, 6])
        k = rng.choice([rng.randrange(8), rng.randrange(0x30), slot])
        pa = rt(G_SOLDIERS) + 0x200 * k
        ptile = rng.randrange(3136)
        if k != slot:
            pr = bytearray(e.uc.mem_read(pa, 0x200))
            pr[0x17A] = k
            pr[0x185] = rng.choice([0, 0, 1, 5])
            pr[0x186] = rng.choice([0, 1, 1, 2])
            pr[0x1B9] = rng.choice([0, 0, 4, 5, 6])
            pr[0x183] = rng.choice([5, 14, 15, 40, 0x90])
            pr[0x1CA] = rng.choice([0, 1, 0xFF])
            pr[0x1F3] = rng.randrange(4)
            ai.w16(pr, 0x1E8 + 2 * pr[0x1F3], rng.choice(list(range(12)) + [0x15, 0x1B, 0x3F, 0x40]))
            e.uc.mem_write(pa, bytes(pr))
        else:
            s[0x185] = rng.choice([0, 1, 5])
            s[0x186] = rng.choice([0, 1, 2])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(G_TILEMAP) + 8 * ptile + 5, rng.choice([k] * 8 + [0x40, 0x30, 0x80 | k]))
        e.uc.mem_write(rt(ai.ANIM_CAN_ACT) - 8, ANIMTBL)
        edx = rng.choice([ptile] * 8 + [rng.randrange(3136)])
        ebx = rng.choice([5, 5, 6, 6, 4, 7, 0x105])
        gsave = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in GLOBALS_W]
    if name == "turn_rest":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x1A5] = rng.choice([0, 5, 6, 10, 20, 25, 30, 0x80, 0xFF])
        s[0x1DB] = rng.choice([0, 0, 1])
        s[0x24] = rng.choice([0, 0x28, 5])
        s[0x1F3] = rng.randrange(5)
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice(list(range(0x40)) + [0x40, 0x50, 0xFFFF]))
        t_ = ai.u16(s, 0x114) % 3136
        e.w8(rt(G_TILEMAP) + 8 * t_ + 5, e.r8(rt(G_TILEMAP) + 8 * t_ + 5) ^ rng.choice([0, 0x80]))
        e.uc.mem_write(sp, bytes(s))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "stat_practice":
        slot = rng.choice([rng.randrange(8)] * 5 + [rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x19B] = rng.choice([0, 1, 1, 1, 2, 3])
        s[0x19C] = rng.choice([0, 6, 6, 5])
        for o_ in ai.STAT_FIELDS:
            s[o_] = rng.choice([1, 5, 30, 50, 80, 99, 100, 0x80, 0xFF, rng.randrange(1, 256)])
        e.uc.mem_write(sp, bytes(s))
        if slot < 8:
            e.uc.mem_write(rt(ai.G_STAT_PROGRESS) + 18 * slot, b"".join(
                rng.choice([0, 1, 99, 100, 150, 999, 1000, 4000, -50, -150, rng.randrange(-30000, 30000)]).to_bytes(2, "little", signed=True)
                for _ in range(9)))
        edx = rng.choice(list(range(0, 11)) + [8, 9, 9, 0x109])
        ebx = rng.choice([0, 1, 1, 2, 3, 5, 0x10001, 0xFFFF, 0x8000])
        ecx_arg = rng.choice([0, 0, 1, 0x100])
        stack_arg = None
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "spend_ap":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(16, 0x28), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x183] = rng.choice([0, 1, 15, 40, 100, 0x80])
        s[0x184] = rng.choice([s[0x183], 100, 60])
        s[0x185] = rng.choice([0, 5, 20])
        s[0x188] = rng.choice([0, 0, 0, 1])
        s[0x18A] = rng.choice([0, 30, 70, 100, 0x80, 0xFF])
        s[0x1A5] = rng.choice([0, 1, 5, 10, 25, 0x80, 0xFF])
        s[0x1E2] = rng.choice([0, 0, 1, 3, 10, 0xFE])
        s[0x181] = rng.choice([0, 1, 1])
        s[0x182] = rng.choice([0, 1, 1, 1])
        s[0x138] = rng.choice([0, 1, 1, 1])
        s[0x1B3] = rng.choice([0, 1, 3])
        s[0x1B2] = rng.choice([0, 0x96, 0x97, 0xA0])
        s[0x19C] = rng.choice([0, 5, 5, 7])
        s[0x1F3] = rng.randrange(5)
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([0, 4, 0x15, 0x39, 0x3A, 0x3B, 0x3C, rng.randrange(0x60)]))
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, slot, 0x40, 3]))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 1]))
        e.w32(rt(ai.G_WEATHER_TIRE), rng.choice([0, 0, 10, 50, -20, 200, 0x7FFF, rng.randrange(1 << 32)]) & 0xFFFFFFFF)
        e.w8(rt(ai.G_MERC_TIRE), rng.choice([0, 0, 5, 0xF0, 0x80]))
        edx = rng.choice([0, 0x63, 1, 2, 4, 6, 10, 25, 30, 0x80, 0xFF, 0x105])
        ebx = rng.choice([0, 0, 1, 10, 50, 100, 300, 0x7FFF, 0x8000, 0xFFF6, 0xFF9C, 0x10032])
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "start_attack":           # the attack on a tile starts: face, aim, reload or friendly-target message
        slot = rng.choice([rng.randrange(8)] * 3 + [rng.randrange(8, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x1BE] = rng.choice([1, 1, 1, 0, 2])
        s[0] = rng.choice([0, 1, 5, 0xE, 0xF, 0x16, 0x1B, 0x29, 0x80, rng.randrange(256)])
        s[1] = rng.choice([0, 0, 1, 3, 0xFF])
        s[0x19C] = rng.choice([0, 3, 5])
        for k_ in range(8):
            s[0x162 + k_] = rng.choice([0, 0, 1])
        s[0x1D8] = rng.randrange(0x30)
        if PORT:      # (the game's 5 animation slots, animation words < 0x100: full default run case 854)
            s[0x1F3] %= 5
            ai.w16(s, 0x1E8 + 2 * s[0x1F3], ai.u16(s, 0x1E8 + 2 * s[0x1F3]) & 0xFF)
        e.uc.mem_write(sp, bytes(s))
        edx = rng.randrange(3136)
        occ = rng.choice([rng.randrange(8), rng.randrange(8), slot, rng.randrange(8, 0x30), 0x40, 0xC0, 0x30])
        e.w8(rt(G_TILEMAP) + 8 * edx + 5, occ)
        if occ < 8 and slot < 8:             # pair flags exist for mercs only (+0x162..+0x169)
            v_ = rng.choice([0, 1])
            e.w8(rt(G_SOLDIERS) + 0x200 * occ + 0x162 + slot, v_)
            if occ == slot:                  # the record copy is the start state of both sides
                s[0x162 + slot] = v_
    if name == "chance_to_hit":          # shooter (the record) at a tile, often with a soldier on it
        slot = rng.choice([rng.randrange(8)] * 3 + [rng.randrange(8, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0] = rng.choice([1, 3, 7, 8, 0xC, 0xE, 0xF, 0x10, rng.randrange(1, 17), rng.randrange(136)])
        if PORT:
            s[0x1F3] %= 5
        for o_ in (0x02, 0x183, 0x184, 0x185, 0x18A, 0x18C, 0x191, 0x192, 0x1B9):
            if rng.random() < 0.4:
                s[o_] = rng.choice([0, 1, 4, 10, 15, 50, 99, 100, rng.randrange(256)])
        if s[0x184] == 0 and s[0x183] != 0:
            s[0x184] = 1
        s[0x24] = rng.choice([0, 0, 0x27])
        s[3] = rng.choice([0, 0, 0x42])
        s[5] = rng.choice([0, 50, 100, rng.randrange(256)])
        me_ = ai.s16(ai.u16(s, 0x114)) % 3136
        ai.w16(s, 0x114, me_)
        ai.w16(s, 0x116, (me_ % 56) * 16 + rng.randrange(16))
        ai.w16(s, 0x118, (me_ // 56) * 14 + rng.randrange(14))
        edx = rng.choice([rng.randrange(3136)] + [(me_ + rng.randrange(-6, 7) + 56 * rng.randrange(-6, 7)) % 3136] * 4)
        ai.w16(s, 0xD0, rng.choice([edx, rng.randrange(3136), ai.NO_TILE]))
        occ = rng.choice([rng.randrange(8), rng.randrange(0x28, 0x30), rng.randrange(8, 0x28), 0x40, slot])
        e.w8(rt(G_TILEMAP) + 8 * edx + 5, occ | rng.choice([0, 0x80]))
        if occ < 0x30 and occ != slot:
            op_ = rt(G_SOLDIERS) + 0x200 * occ
            if PORT:      # (the game's 5 animation slots: +0x1F3 is 0..4)
                e.w8(op_ + 0x1F3, e.r8(op_ + 0x1F3) % 5)
            e.w8(op_ + 0x17A, occ)
            e.w16(op_ + 0x1E8 + 2 * e.r8(op_ + 0x1F3), rng.choice([0, 3, 4, 5, 7, 14, 16, 0x38, 0x39, rng.randrange(0x60)]))
            for o_ in (0x183, 0x184, 0x18A, 0x18B, 0x192, 0x1A5, 0x1B1, 0x1B3, 0x1F2):
                if rng.random() < 0.4:
                    e.w8(op_ + o_, rng.choice([0, 1, 2, 5, 15, 20, 50, 100, rng.randrange(256)]))
            e.w8(op_ + 0x1D5, rng.choice([0, 1, 5, 0x80]))
            e.w8(op_ + 0x13A + slot if occ >= 0x28 else op_ + 0x112 + slot, rng.choice([0, 1, 1, 2]))
            e.w8(op_ + 0x1F2, rng.choice([e.r8(op_ + 0x1F2), 10, 20, 40]))
        ebx = rng.choice([0, 0, 1, 2, 3, 4])
        e.uc.mem_write(sp, bytes(s))
    if name in ("fire_gun", "attack_go"):   # the shot: cost, end point and roll (+ the fire animation)
        slot = rng.choice([rng.randrange(8)] * 3 + [rng.randrange(8, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0] = rng.choice([1, 3, 7, 8, 0xC, 0xE, 0xF, 0x10, 0x11, 0x29, rng.randrange(0x80)])
        if PORT:
            s[0x1F3] %= 5
            s[0] = rng.randrange(1, 0x11)   # (only guns and knives fire: attack_sound has weapon classes 0..4)
        s[1] = rng.choice([0, 1, 5])
        s[0x1BE] = rng.choice([2, 2, 2, 1, 0])
        s[0x1FD] = rng.choice([0, 0, 1, 2, 4, 0x80, 0xFF])
        s[0x1A1] = s[0x1A2] = rng.randrange(1, 9)
        s[0x9C] = rng.choice([0, 1, 0x80])
        for k_ in range(8):
            s[0x13A + k_] = rng.choice([0, 1, 1, 2, 0xFF])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([0x1B, 0x1F, 0x23, 0x1B, 0x1F, 0x23, 4, 0x20]))
        me_ = ai.s16(ai.u16(s, 0x114)) % 3136
        ai.w16(s, 0x114, me_)
        ai.w16(s, 0x116, (me_ % 56) * 16 + rng.randrange(16))
        ai.w16(s, 0x118, (me_ // 56) * 14 + rng.randrange(14))
        edx = rng.choice([rng.randrange(3136), me_ + rng.choice([1, -1, 56, 57, -55, 2, 112]), me_])
        edx = edx % 3136
        if PORT and name == "attack_go":   # a missed shot scatters +0xDC; off the map the original reads past the tile
            # map (the port reads 0 there): keep the shooter inside and the target near, so the scatter stays on the map
            me_ = rng.randrange(20, 36) * 56 + rng.randrange(20, 36)
            ai.w16(s, 0x114, me_)
            ai.w16(s, 0x116, (me_ % 56) * 16 + rng.randrange(16))
            ai.w16(s, 0x118, (me_ // 56) * 14 + rng.randrange(14))
            edx = me_ + rng.randrange(-3, 4) * 56 + rng.randrange(-3, 4)
        ai.w16(s, 0xDC, rng.choice([edx] * 6 + [ai.NO_TILE]))
        occ = rng.choice([rng.randrange(8), rng.randrange(0x28, 0x30), rng.randrange(8, 0x28), 0x40, 0x40, 0x30, slot])
        e.w8(rt(G_TILEMAP) + 8 * edx + 5, occ | rng.choice([0, 0x80]))
        if occ < 0x30 and occ != slot:
            op_ = rt(G_SOLDIERS) + 0x200 * occ
            if PORT:      # (the game's 5 animation slots: +0x1F3 is 0..4)
                e.w8(op_ + 0x1F3, e.r8(op_ + 0x1F3) % 5)
            e.w16(op_ + 0x116, (edx % 56) * 16 + rng.randrange(16))
            e.w16(op_ + 0x118, (edx // 56) * 14 + rng.randrange(14))
            e.w16(op_ + 0x1E8 + 2 * e.r8(op_ + 0x1F3), rng.choice([0, 3, 4, 5, 0x38, 0x39, 0x40]))
        e.w8(rt(G_TILEMAP) + 8 * edx + 1, rng.choice([0, 0xE2, 0xE3, rng.randrange(256)]))
        e.uc.mem_write(sp, bytes(s))
        ebx = sp                         # (AttackGo: the caller's EBX is the soldier pointer: sight-line flag 0x58)
    if name == "gun_jam":                # a merc's gun jams on firing, or a jammed one is cleared
        slot = rng.choice([rng.randrange(8)] * 4 + [rng.randrange(8, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x1C9] = rng.choice([0, 0, 0, 1])
        s[1] = rng.choice([0, 1, 5, 0x7F, 0xFF, 0xFB, 0x80])
        s[2] = rng.choice([0, 10, 49, 50, 80, 100, 0x80, 0xF0])
        s[0x190] = rng.choice([0, 1, 9, 40, 80, 0x7F, 0x80, 0xFE])
        s[0x1D8], s[0x1C2], s[0x1BE] = rng.randrange(0x30), rng.randrange(0x30), rng.randrange(256)
        ai.w16(s, 0xD0, rng.randrange(3136))
        ai.w16(s, 0xDC, rng.randrange(3136))
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.randrange(0x60))   # current animation: some step on ClearTarget(1)
        e.uc.mem_write(sp, bytes(s))
    if name == "use_on_tile":            # use the hand item on a tile: containers, locks, break open, mines
        slot = rng.choice([rng.randrange(8)] * 4 + [rng.randrange(8, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        # everything this setup writes goes back before the next case (the next sections read it: view)
        uot_keep = [(a_, bytes(e.uc.mem_read(rt(a_), k_))) for a_, k_ in
                    ((G_SOLDIERS + slot * 0x200, 0x200), (G_TILEMAP, 8 * 3136), (ai.G_GROUND_ITEMS, 12 * 256), (ai.G_FX, 13 * 5),
                     (ai.G_BLAST_LIST, 60), (ai.G_TILESET, 1), (ai.G_CUR_SECTOR, 1), (ai.G_SECTORS, 60 * 0xB1))]
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0] = rng.choice([0x87, 0x87, 0x11, 0, rng.randrange(256)])
        for f_ in (0x1A6, 0x1A7, 0x95, 0x9D, 0x1B3):
            s[f_] = rng.randrange(256)
        edx = rng.randrange(57, 3136 - 57)
        me = rng.choice([edx - 1, edx + 1, edx - 56, edx + 56, edx])
        ai.w16(s, 0x114, me)
        ebx = rng.choice([0, 0x76, 0x76, 1, 0xFE, 0xFF, rng.randrange(256)])
        if rng.random() < 0.05:                      # off the map (0xC41 = no tile): nothing at all
            edx = rng.choice([0xC40, 0xC41]) if not PORT else 0xC41 + 0 * rng.choice([0, 0])   # --port: tile 0xC40 reads past the map (spec gap); same rng draws
        m_ = rt(G_TILEMAP) + 8 * (edx if edx < 0xC40 else me)
        e.w8(m_ + 1, rng.choice([0x8E, 0x8E, 0x9A, 0x9C, 0xA0, 0xA4, 0xA6, 0xAA, 0x9B, 0xA1, 0xA7, 0x3E, 0x45, 0,
                                 rng.randrange(256)]))
        e.w8(m_ + 5, rng.choice([0x40, 0x40, 0x40, 0x7F, slot, rng.randrange(8), rng.randrange(8, 0x30), 0x80 | rng.randrange(0x30)]))
        k_ = rng.choice([0xFF, 0xFF, 0xC8] + [rng.randrange(200)] * 6)
        if k_ == 0xC8 and rng.random() < 0.5:      # tile byte 6 = 201..254: the Ground index of the tile is "none" for
            k_ = rng.randrange(201, 255)           # every value of 200 or more, not only 200
        e.w8(m_ + 6, k_)
        g_ = rt(ai.G_GROUND_ITEMS) + 12 * k_
        e.w8(g_, rng.choice([0x11, 0x11, 0x44, 0x52, 0x70, 0x16, 0x05, rng.randrange(256)]))
        e.w8(g_ + 2, rng.choice([0, 1, 50, 0x80, 0xFF]))
        e.w8(g_ + 6, rng.choice([0xFF, 0xFF, 0, 1, 2]))
        e.w8(rt(ai.G_GROUND_ITEMS) + 12 * 0xFF, rng.choice([0x11, 0x44, 0, 5]))   # = profile 5 +0x52 (kit place 2)
        for i_ in range(5):
            r_ = rt(ai.G_FX) + 13 * i_
            e.w32(r_, rng.choice([0xFFFFFFFF, 0, 5]))
            e.w16(r_ + 6, rng.choice([edx, ai.NO_TILE, rng.randrange(3136)]))
        for i_ in range(30):
            e.w16(rt(ai.G_BLAST_LIST) + 2 * i_, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136)]))
        if FAN and 55 in FAN and F55_RNG.random() < 0.5:   # F55: the used tile in the blast list (a known trap)
            e.w16(rt(ai.G_BLAST_LIST) + 2 * F55_RNG.randrange(30), edx & 0xFFFF)
        e.w8(rt(ai.G_TILESET), rng.randrange(10))
        cs_ = rng.choice([0xB, 0xB, rng.randrange(0x40)])
        e.w8(rt(ai.G_CUR_SECTOR), cs_ if cs_ < 60 or not PORT else 0xB)   # --port: the world's sector states are 60
        e.w8(rt(ai.G_SAFE_OPENED), rng.choice([0, 1]))
        # combat.md Crowbar: a lock value for this tile (door table +0x00 for an unstandable tile, lock table +0x30),
        # and a crowbar in the hand; random tiles almost never match a table pair (ja1-cov 2026-11-06: force path 0 calls)
        sec_ = rt(ai.G_SECTORS) + 0xB1 * (cs_ if cs_ < 60 or not PORT else 0xB)
        if UOL_RNG.random() < 0.5:
            v_ = UOL_RNG.choice([UOL_RNG.randrange(1, 20), UOL_RNG.randrange(1, 120), 50, 100, 200, UOL_RNG.randrange(-5, 300)])
            e.w16(sec_ + UOL_RNG.choice([0x30, 0x30, 0]) + 4 * UOL_RNG.randrange(12), edx & 0xFFFF)
            e.w16(sec_ + 0x30 + 4 * UOL_RNG.randrange(12) + 2, v_ & 0xFFFF)   # (value of a random pair: sometimes the matched one)
            if UOL_RNG.random() < 0.7:
                for o_ in (0, 0x30):
                    for k2 in range(12):
                        if e.r16(sec_ + o_ + 4 * k2) == edx & 0xFFFF:
                            e.w16(sec_ + o_ + 4 * k2 + 2, v_ & 0xFFFF)
                            break
        if UOL_RNG.random() < 0.6:
            s[0] = 0x76
            s[2] = UOL_RNG.choice([0, 9, 10, 10, 11, 12, 15, 30, 100, UOL_RNG.randrange(256)])
            if UOL_RNG.random() < 0.85:
                ebx = 0x76                           # the crowbar flag: bar bonus + break chance (combat.md Crowbar)
            if UOL_RNG.random() < 0.5:                   # force = strength x (hp + hurt/2)/2/100: a large force breaks more bars
                s[0x18A] = UOL_RNG.randrange(30, 101)
                s[0x183] = s[0x184] = UOL_RNG.randrange(20, 101)
        e.uc.mem_write(sp, bytes(s))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "mine_flag":              # combat.md A detected mine gets a blue flag: object 0x86, move code, path again
        slot = MF_RNG.choice([MF_RNG.randrange(8), MF_RNG.randrange(8), MF_RNG.randrange(8, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        me = MF_RNG.randrange(3136)
        ai.w16(s, 0x114, me)
        ai.w16(s, 0x124, MF_RNG.choice([me, MF_RNG.randrange(3136), ai.NO_TILE]))
        for o_ in (0x1A6, 0x1A7, 0x95, 0x1A2):
            s[o_] = MF_RNG.choice([0, 1, 5, 0x4C, 0xFF])
        for o_ in (0x134, 0x96, 0x126, 0x128):
            ai.w16(s, o_, MF_RNG.randrange(0x10000))
        edx = MF_RNG.choice([me + 1, me - 56, MF_RNG.randrange(3136)]) % 3136
        e.w8(rt(G_TILEMAP) + 8 * edx + 1, MF_RNG.choice([0, 0, 0x86, MF_RNG.randrange(256)]))
        e.w16(rt(ai.G_MOVECODE) + 2 * edx, MF_RNG.choice([0, 1000, 1200, 1500, 1600, 3600, 25500, MF_RNG.randrange(0x10000)]))
        e.w8(rt(ai.G_PATHS) + 0x50 * slot, MF_RNG.choice([0, 1, 7, 0xFF, MF_RNG.randrange(256)]))
        mf_keep = (rt(G_TILEMAP) + 8 * edx + 1, 1), (rt(ai.G_MOVECODE) + 2 * edx, 2)
        mf_pre = [bytes(e.uc.mem_read(a_, k_)) for a_, k_ in mf_keep]
        e.uc.mem_write(sp, bytes(s))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "walter":                 # combat.md The listening device at a door: story lines, counters, ticking
        slot = WL_RNG.randrange(8)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        edx = WL_RNG.randrange(3136)
        g_ = WL_RNG.randrange(200)
        cls_ = bytes(e.uc.mem_read(rt(ai.G_T1_CLASS), 256))
        by_ = {c_: [o_ for o_ in range(256) if cls_[o_] == c_] for c_ in (7, 8, 9)}
        wl_keep = [(rt(G_SOLDIERS) + 0x200 * 0x28, 0x200 * 8), (sp, 0x200), (rt(G_TILEMAP) + 8 * edx, 8),
                   (rt(ai.G_GROUND_ITEMS) + 12 * g_, 12), (rt(ai.G_WALTER_SAID), 12), (rt(ai.G_WALTER_NEXT), 8),
                   (rt(ai.G_WALTER_SECTOR), 1), (rt(0x22A040), 43), (rt(ai.G_CUR_SECTOR), 1), (rt(ai.G_ALARM), 1),
                   (rt(ai.G_DAY), 4), (rt(ai.G_CRATE_DROPS), 16)]
        wl_keep = [(a_, bytes(e.uc.mem_read(a_, k_))) for a_, k_ in wl_keep]
        s[0x17A] = slot
        s[0] = WL_RNG.choice([0x45, 0x45, 0x45, 0x45, 0x4C, WL_RNG.randrange(256)])
        s[2] = WL_RNG.choice([100, 100, 90, 50, 11, 10, 5, 0x80, 0xFF, WL_RNG.randrange(256)])
        e.uc.mem_write(sp, bytes(s))
        cur_ = WL_RNG.randrange(60)
        e.w8(rt(ai.G_CUR_SECTOR), cur_)
        sec_ = lambda: WL_RNG.choice([99, 99, cur_, cur_, WL_RNG.randrange(60), 0x80, 0xFF])
        e.w8(rt(G_TILEMAP) + 8 * edx + 1, WL_RNG.choice([WL_RNG.choice(by_[c_]) for c_ in (7, 8, 9) if by_[c_]] * 2 + [WL_RNG.choice(by_[7] or [0])] * 3
                                                        + [WL_RNG.randrange(256)]))
        e.w8(rt(G_TILEMAP) + 8 * edx + 6, WL_RNG.choice([g_, g_, 0xFF, 200, 0xC7]))
        e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_ + 2, WL_RNG.choice([0x80, 0xFF, 0, 0x7F, 50]))
        t0_ = bytes(e.uc.mem_read(rt(ai.G_T0_CLASS), 256))
        water_ = [v_ for v_ in range(256) if t0_[v_] == 3] or [0]
        for k_ in range(0x28, 0x30):
            b_ = rt(G_SOLDIERS) + 0x200 * k_
            e.w8(b_ + 0x182, WL_RNG.choice([0, 1, 1, 1]))
            e.w8(b_ + 0x183, WL_RNG.choice([0, 14, 15, 50, 50, 50, 0x80]))
            e.w8(b_ + 0x1B2, WL_RNG.choice([WL_RNG.choice(water_)] * 3 + [WL_RNG.randrange(256)]))
            e.w8(b_ + 0x9C, WL_RNG.choice([0, 0, 0, 0, 0, 1, 0xFF]))
            # near the door most times (near >= 2 runs the story lines a. to i.), else anywhere or off the map
            nb_ = (edx + WL_RNG.choice([1, -1, 56, -56, 57, -55, 2, -112, 114, 3, 168, -170, 5])) % 3136
            e.w16(b_ + 0x114, WL_RNG.choice([nb_, nb_, nb_, WL_RNG.randrange(3136), 0xC41]))
        for k_ in range(12):
            e.w8(rt(ai.G_WALTER_SAID) + k_, sec_())
        for k_ in range(8):
            e.w8(rt(ai.G_WALTER_NEXT) + k_, WL_RNG.choice([0x13, 0x14, 0x27, 0x28, 0x3B, 0x3C, 0x40, 0x45, 0x59, 0x6D,
                                                          0x7F, 0x80, WL_RNG.randrange(256)]))
        e.w8(rt(ai.G_WALTER_SECTOR), sec_())
        for k_ in range(5, 0xF):
            e.w8(rt(0x22A040) + k_, WL_RNG.choice([0, 1, 1, 2, 0xFF]))
        e.w8(rt(ai.G_ALARM), WL_RNG.choice([0, 1]))
        day_ = WL_RNG.choice([1, 5, 10, 11, 20, WL_RNG.randrange(-3, 40)])
        e.w32(rt(ai.G_DAY), day_ & 0xFFFFFFFF)
        for k_ in range(4):
            e.w8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 1, (day_ + WL_RNG.choice([-1, 0, 1, 3, 5, 5, 6, 9])) & 0xFF)
        focus_ = WL_RNG.choice([None, None, None, "e2", "g", "h"])   # deep story lines: e. third pick, g. crate days, h. day limit
        if focus_:
            for k_ in (0x28, 0x29):          # 2 enemies next to the door: near >= 2
                b_ = rt(G_SOLDIERS) + 0x200 * k_
                e.w8(b_ + 0x182, 1); e.w8(b_ + 0x183, 50); e.w8(b_ + 0x1B2, water_[0]); e.w8(b_ + 0x9C, 0)
                e.w16(b_ + 0x114, (edx + 1) % 3136)
            for k_ in (5, 6, 7, 0xC):        # a. to d. off
                e.w8(rt(0x22A040) + k_, 0)
            e.w8(rt(ai.G_WALTER_SECTOR), (cur_ + 1) % 60)
            other_ = (cur_ + WL_RNG.randrange(1, 60)) % 60
            if focus_ == "e2":
                e.w8(rt(0x22A040) + 0xB, 1)
                for k_, v_ in enumerate([WL_RNG.choice([cur_, other_]), WL_RNG.choice([cur_, other_]), 99]):
                    e.w8(rt(ai.G_WALTER_SAID) + k_, v_)
            elif focus_ == "g":
                for k_ in (0xA, 0xB):
                    e.w8(rt(0x22A040) + k_, 0)
                for k_ in range(4):
                    e.w8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 1, (day_ + WL_RNG.choice([-1, 0, 0, 1, 5, 6, 6, 9])) & 0xFF)
                e.w8(rt(ai.G_WALTER_SAID) + 0xA, WL_RNG.choice([99, 99, cur_, other_]))
                e.w8(rt(ai.G_WALTER_SAID) + 0xB, 99)
            else:
                for k_ in (0xA, 0xB):
                    e.w8(rt(0x22A040) + k_, 0)
                e.w8(rt(0x22A040) + 0xE, 1)
                day_ = WL_RNG.choice([10, 10, 11])
                e.w32(rt(ai.G_DAY), day_)
                for k_ in range(4):
                    e.w8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 1, day_ - 1)
                e.w8(rt(ai.G_WALTER_SAID) + 9, 99)
        wl_cmp = [(rt(ai.G_WALTER_SAID), 12), (rt(ai.G_WALTER_NEXT), 8), (rt(ai.G_WALTER_SECTOR), 1)]
        wl_pre = [bytes(e.uc.mem_read(a_, k_)) for a_, k_ in wl_cmp]
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "lock_kit":               # combat.md Using the lock kit on the mission safe: skill against 80/90/100
        slot = LK_RNG.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        lk_keep = [(sp, bytes(s))] + [(rt(a_), bytes(e.uc.mem_read(rt(a_), 1))) for a_ in   # (restored after the
                   (ai.G_DIFFICULTY, ai.G_SAFE_OPENED, ai.G_MISSION_STEP, ai.G_MISSION_ORDER)]   # case: later sections
        s[0x17A] = slot
        s[0] = LK_RNG.choice([0x4C, 0x4C, 0x4C, 0x87, LK_RNG.randrange(256)])
        s[2] = LK_RNG.choice([100, 100, 90, 50, 6, 5, 1, 0, 0x80, 0xFF, LK_RNG.randrange(256)])
        for f_ in (0x18C, 0x190, 0x18D):
            s[f_] = LK_RNG.choice([100, 95, 90, 80, 60, 30, 0, 0x7F, 0x80, 0xFF, LK_RNG.randrange(256)])
        s[0x192] = LK_RNG.choice([0, 1, 3, 5, 10, 0x33, 0xFF, LK_RNG.randrange(256)])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_DIFFICULTY), LK_RNG.choice([0, 1, 2]))
        e.w8(rt(ai.G_SAFE_OPENED), LK_RNG.choice([0, 1]))
        e.w8(rt(ai.G_MISSION_STEP), LK_RNG.choice([0, 1, 3]))
        e.w8(rt(ai.G_MISSION_ORDER), LK_RNG.choice([0x40, 1, 5]))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "pickup_here":            # pick up / disarm the ground item under the soldier (mines, effects)
        slot = rng.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        me = rng.randrange(3136)
        ai.w16(s, 0x114, me)
        s[0x18F] = rng.choice([0, 3, 30, 90, 0x80, 0xFE, 0xFF, 0xF9])
        s[0x192] = rng.choice([0, 1, 2, 5, 42, 43, 0x80, 0xFF])
        for f_ in (0x1A6, 0x1A7, 0x95):
            s[f_] = rng.randrange(256)
        k_ = rng.choice([0xFF, 0xC8, 0xFE] + [rng.randrange(200)] * 6)
        e.w8(rt(G_TILEMAP) + 8 * me + 6, k_)
        g_ = rt(ai.G_GROUND_ITEMS) + 12 * k_
        e.w8(g_, rng.choice([0x11, 0x11, 0x11, 0x44, 0x52, 0x70, 0x29, 0x3F, 0x71, 0x16, 0, rng.randrange(256)]))
        e.w8(g_ + 2, rng.choice([0, 1, 50, 0x7F, 0x80, 0xFF]))
        e.w8(g_ + 6, rng.choice([0xFF, 0xFF, 0, 1]))
        if k_ < 200 and rng.random() < 0.3:          # the type the 0xFF lookup reads past the list
            e.w8(rt(ai.G_GROUND_ITEMS) + 12 * 0xFF, rng.choice([0x11, 0x44, 0]))
        for i_ in range(5):
            r_ = rt(ai.G_FX) + 13 * i_
            e.w32(r_, rng.choice([0xFFFFFFFF, 0, 5, 0x80000000]))
            e.w16(r_ + 6, rng.choice([me, me, ai.NO_TILE, rng.randrange(3136)]))
            e.w8(r_ + 0xC, rng.randrange(256))
        full_ = rng.random() < 0.3                   # a nearly full list: the last free entry is used
        for i_ in range(30):
            v_ = rng.randrange(3136) if full_ else rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136)])
            e.w16(rt(ai.G_BLAST_LIST) + 2 * i_, me + 1 if v_ == me else v_)
        if full_ and rng.random() < 0.5:
            e.w16(rt(ai.G_BLAST_LIST) + 58, ai.NO_TILE)
        if rng.random() < 0.15:
            e.w16(rt(ai.G_BLAST_LIST) + 2 * rng.randrange(30), me)
        e.w8(rt(ai.G_FX_FLAG), rng.randrange(256))
        e.w8(rt(ai.G_CUR_SECTOR), rng.randrange(60 if PORT else 0x40))   # (--port: 60 sector rows)
        e.uc.mem_write(sp, bytes(s))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "bury_mine":              # a mine is buried, or another explosive set off, on a tile next to the soldier
        slot = rng.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0] = rng.choice([0x11, 0x11, 0x11, 0x16, 0x1B, 0x1C, 0x5D, 0, rng.randrange(136 if PORT else 256)])   # (136 types)
        s[1] = rng.choice([0, 1, 3, 0x80])
        s[2] = rng.choice([0, 9, 10, 11, 100, 0x80, 0xFF])
        s[3:12] = bytes(rng.randrange(256) for _ in range(9))
        me = rng.randrange(57, 3136 - 57)
        ai.w16(s, 0x114, me)
        edx = rng.choice([me, me + 1, me - 56, me + 57, rng.randrange(3136)])
        e.w8(rt(G_TILEMAP) + 8 * edx + 1, rng.choice([0x7F, 0x80, 0x81, 0x82, 0xC9, 0xC9, 0x37, 0, rng.randrange(256)]))
        e.w8(rt(ai.G_CUR_SECTOR), rng.randrange(60 if PORT else 0x40))   # (--port: 60 sectors)
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 2, 0x80, 0xFF]))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, slot, rng.randrange(0x30)]))
        e.uc.mem_write(sp, bytes(s))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "reserve_ap":             # the AP held back for the item in hand
        slot = rng.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0] = rng.choice([0, 1, 5, 0xE, 0xF, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x29, 0x5D, 0x44,
                           rng.randrange(256)])
        s[0xEF] = rng.randrange(256)
        s[0x1A5] = rng.choice([0, 3, 8, 12, 20, 22, 24, 0x7F, 0x80, 0xFF])
        s[0x16A] = rng.choice([0, 1, 0x80])
        e.uc.mem_write(sp, bytes(s))
    if name == "trigger":                # a soldier springing the ground item on a tile (mines, flares, traps)
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 0x28), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0xBA] = rng.choice([0, 1, 4, 4])
        e.uc.mem_write(sp, bytes(s))
        tile = rng.randrange(3136)
        e.w8(rt(G_TILEMAP) + 8 * tile + 1, rng.choice([0x86, 0, 0x10]))
        gi_ = rng.randrange(200)
        g_ = rt(ai.G_GROUND_ITEMS) + 12 * gi_
        e.w8(g_, rng.choice([0x11, 0x13, 0x2C, 0x72, 0x44, 0x52, 0x70, 0x29, 0x3F, 0x71, 0x14, 0x15, 0x16, 0x17, 0x18,
                             0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x5D, 0x10, rng.randrange(136)]))
        e.w8(g_ + 1, rng.choice([0, 1, 3, 0x80]))
        e.w8(g_ + 2, rng.choice([0, 50, 0x80, 0xFF]))
        e.w8(g_ + 6, rng.choice([0, 0xFF]))
        e.w8(rt(ai.G_MISSION_STEP), rng.choice([0, 3]))
        edx, ebx = tile, gi_ | rng.choice([0, 0x100])
        ecx_arg = rng.choice([0, 1, 1, 0x100])
        gsave = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in GLOBALS_W]
    if name == "shot_alert":             # a shooter's target reacting after the shot (ShotEnd)
        mid = 16 if PORT else 0x28     # --port: slots 16..0x27 index past the knowledge tables (the original reads on)
        slot = rng.choice([rng.randrange(8), rng.randrange(8, mid), rng.randrange(0x28, 0x30), rng.randrange(0x28, 0x30)])
        k = rng.choice([rng.randrange(8), rng.randrange(8, mid), rng.randrange(0x28, 0x30)] * 3 + [0x40, 0x28])
        k = k if k != slot else (k + 1) % 0x30
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x1D8] = k
        x, y = rng.randrange(20, 880), rng.randrange(20, 760)
        ai.w16(s, 0x116, x)
        ai.w16(s, 0x118, y)
        ai.w16(s, 0x114, rng.randrange(3136))
        ex, ey = (rng.randrange(0, 896), rng.randrange(0, 784)) if PORT else (rng.randrange(-20, 900), rng.randrange(-20, 790))   # (--port: on the map)
        ai.w16(s, 0xE4, ex)
        ai.w16(s, 0xE6, ey)
        ai.w16(s, 0xC8, rng.choice([0, 10, 50, 100, 200, 400, 0x8000]))
        e.uc.mem_write(sp, bytes(s))
        if k < 0x30:
            oa = rt(G_SOLDIERS) + 0x200 * k
            o = bytearray(e.uc.mem_read(oa, 0x200))
            o[0x17A] = k
            o[0x182] = rng.choice([0, 1, 1, 1])
            o[0x138] = rng.choice([0, 1, 1, 1])
            o[0x183] = rng.choice([5, 14, 15, 15, 40, 0x90])
            o[0xBA] = rng.choice([0, 1, 2, 3])
            o[0xBB] = rng.choice([0, 1, 2, 3, 0xFF])
            o[0x1D4] = rng.choice([0, 1, 2])
            o[0x112 + slot if k < 0x28 else 0x13A + slot] = rng.choice([0, 1, 2, 0xFF, 3])
            ai.w16(o, 0x114, rng.choice([ai.u16(s, 0x114), rng.randrange(3136)]))
            ai.w16(o, 0x116, rng.choice([x + rng.randrange(-60, 60), rng.randrange(900)]))
            ai.w16(o, 0x118, rng.choice([y + rng.randrange(-60, 60), rng.randrange(800)]))
            e.uc.mem_write(oa, bytes(o))
            ot = ai.s16(ai.u16(o, 0x114))           # flown distance right at the pixel distance - 45 now and then
            near = ai.cheb_px(x + 2, y + 4, ai.tile_x(ot) + 2, ai.tile_y(ot) + 4) - 0x2D
            if rng.random() < 0.4:
                ai.w16(s, 0xC8, near + rng.choice([-1, 0, 1]))
                e.uc.mem_write(sp, bytes(s))
        vt = ai.s16(combat.view_tile(ex, ey) & 0xFFFF)
        if 0 <= vt < 3136:
            e.w8(rt(G_TILEMAP) + 8 * vt + 5, rng.choice([0x40, 0x40, 0x30, rng.randrange(0x30)]))
        e.w8(rt(ai.G_KNOWN) + slot, rng.choice([0, 1, 2, 0xFF, 3]))
        edx = rng.choice([0, 0, 1, 0x100])
        gsave = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in GLOBALS_W]
    if name == "noise_int":   # 0x28 = first enemy, the merc / enemy bound of step 2
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(16, 0x28), rng.randrange(0x28, 0x30), 0x27, 0x28])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0xBA] = rng.choice([0, 0, 1, 2, 3])
        s[0x18A] = rng.choice([0, 1, 1, 50])
        s[0x1F3] = rng.randrange(4)
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice(list(range(10)) + [0x104]))
        s[0x1A5] = rng.choice([0, 1, 5, 0x80, 0xFF])
        s[0x1B0] = rng.choice([0, 1])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_SHOW_UI), rng.choice([0, 0, 1]))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, 3, 9, 0x2A, 0x40]))
        e.w8(rt(ai.G_ITQ_N), rng.randrange(6))
        e.w8(rt(0x92DD2), rng.choice([0, 5, 0x40]))
        gsave = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in GLOBALS_W]
    if name == "merc_dies":
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16), rng.randrange(16, 0x28),
                           rng.randrange(0x28, 0x30), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x188] = rng.choice([0, 0, 0, 0, 1, 2])
        s[0x179] = rng.choice([5, 0x1D, 0x1E, 0x3C])
        s[0x1DA] = rng.choice([rng.randrange(8), rng.randrange(8), 8, rng.randrange(0x28, 0x30), 0x40])
        ai.w16(s, 0x114, rng.choice([rng.randrange(3136)] + ([] if PORT else [ai.NO_TILE])))   # (--port: off the map the
        e.uc.mem_write(sp, bytes(s))                                                        # original writes past it)
        sec = e.r8(rt(ai.G_CUR_SECTOR))
        e.w8(rt(ai.G_SECTOR_TBL9) + 9 * sec + 1, rng.choice([0, 1, 1, 2, 5]))
        e.w8(rt(ai.G_SECTOR_TBL9) + 9 * sec + 2, rng.choice([0, 0, 1, 3]))
        for k_ in range(16):
            a_ = rt(G_SOLDIERS) + 0x200 * k_
            if k_ != slot:
                e.w8(a_ + 0x182, rng.choice([0, 1, 1]))
                e.w8(a_ + 0x138, rng.choice([0, 1, 1]))
                e.w8(a_ + 0x1D5, rng.choice([0, 0, 1]))
                e.w8(a_ + 0x1FE, rng.choice([0, 1]))
        e.w8(rt(ai.G_REDRAW_OFF), rng.choice([0, 1, 2]))
        e.w8(rt(ai.G_SQUAD_LOST), rng.choice([0, 1]))
        e.w32(rt(0x229D2C), rng.choice([0, 5, ai.NO_TILE]))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, 0x40, 3]))
        edx = rng.choice([0, 0, 1, 0x100])
        gsave = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in GLOBALS_W]
    if name == "kill":
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16), rng.randrange(16, 0x28),
                           rng.randrange(0x28, 0x30), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x186] = rng.choice([0, 0, 0, 1, 0xFF])
        s[0x179] = rng.choice([5, 0x3B, 0x3C, 0x50])
        s[0x1DA] = rng.choice([rng.randrange(8), slot, rng.randrange(0x28, 0x30), 0x40])
        s[0x192] = rng.choice([1, 5, 0x80, 0x7F])
        for i_ in range(12):
            s[12 * i_] = rng.choice([0, 0, rng.randrange(136)])
        if PORT:                       # Put item takes map tiles (sector_start.md batch 3)
            ai.w16(s, 0x114, rng.randrange(3136))
        e.w8(rt(ai.G_SECTOR_REC) + 0xB1 * e.r8(rt(ai.G_CUR_SECTOR)), rng.choice([0, 0x22, 0x5D, rng.randrange(256)]))
        s[0x6C] = rng.choice([s[0x6C], e.r8(rt(ai.G_SECTOR_REC) + 0xB1 * e.r8(rt(ai.G_CUR_SECTOR)))])
        if PORT and s[0x6C] >= 136:    # (the game's 136 item types: the port's item_types ends there)
            s[0x6C] = rng.randrange(136)
        for o_ in (0x1B9, 0x195, 0x1DC, 0x1DD, 0x1DE, 0x1DF):
            s[o_] = rng.choice([0, 1, 5])
        e.w8(rt(0x92DCE), rng.choice([0, 1]))
        e.w8(rt(0x92DD5), rng.choice([0, 0x40, 3]))
        e.uc.mem_write(sp, bytes(s))
        edx = rng.choice([0, 0, 1, 0x100])
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, 0x40, 3, 9]))
        e.w8(rt(0x92DC7), rng.choice([slot, 0x40]))
        e.w8(rt(ai.G_MISSION_ORDER), rng.choice([slot, 0x40]))
        e.w8(rt(0x22A0D0), rng.choice([0, 1, 0xFF]))
        e.w16(rt(ai.G_MISSION_ORDER + 2), rng.choice([ai.NO_TILE, rng.randrange(3136)]))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "melee_hit":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        occ = rng.choice([rng.randrange(16), rng.randrange(0x28, 0x30), rng.randrange(0x28, 0x30) if slot < 0x28
                          else rng.randrange(16), 0x40, 0x7F])
        tile = rng.choice([e.r16(rt(G_SOLDIERS) + 0x200 * occ + 0x114) if occ < 0x30 else rng.randrange(3136),
                           rng.randrange(3136)]) % 3136
        e.w8(rt(G_TILEMAP) + 8 * tile + 5, occ | rng.choice([0, 0, 0x80]))
        e.w8(rt(G_TILEMAP) + 8 * tile + 1, rng.choice([0xAE, 0, 0x30, rng.randrange(256)]))
        ai.w16(s, 0xDC, tile)
        if FAN and 59 in FAN and F59_RNG.random() < 0.25:   # #70 F59: 1.15m refuses a target tile >= 0xC40
            ai.w16(s, 0xDC, F59_RNG.choice([0xC40, 0xC41, 0xC7F]))
        s[0] = rng.choice([0xF, 0x10, 0x10, 0, 3, rng.randrange(1, 0x11) if PORT else rng.randrange(136)])   # (--port: attack_sound has weapon classes 0..4)
        if PORT and MELEE_HI.random() < 0.3:
            # made cases (own stream, the old cases stay): a hand item id above 43; MeleeHit? 0x3521C reads byte 5 (damage)
            # and byte 0 (class, here 0..1) of its 6-byte record past the 16 weapons, in the 1536 bytes the port copies
            s[0] = MELEE_HI.choice([45, 47, 49, 51, 53, 71, 79, 85])
        s[2] = rng.choice([0, 5, 10, 11, 50, 100, 0x80, 0xFF])
        s[0x1FD] = rng.choice([0, 1, 2, 4, 0xFF])
        s[0x192] = rng.choice([0, 1, 3, 10, 0x80, 0x7F, 0x60])
        s[0x1BE] = rng.choice([0, 1, 5])
        s[0x1A6] = F33_RNG.choice([0, 0, 0xF, 0x10, 0x45, 7])   # (#70 F33: the end clears it)
        s[0x185] = rng.choice([0, 0, 3, 20, 0x90, rng.randrange(256)])   # (wound part of the hp term: (b184 - b183 - b185) & 0xFF)
        if occ < 0x30 and occ != slot:
            a_ = rt(G_SOLDIERS) + 0x200 * occ
            e.w8(a_ + 0x183, rng.choice([0, 10, 14, 15, 30, 60]))
            e.w8(a_ + 0xBA, rng.choice([0, 1, 2, 3]))
            e.w8(a_ + 0x112 + slot if slot < 0x30 else a_ + 0x112, rng.choice([0, 1, 1, 2, 0xFF]))
            if slot < 0x28:
                e.w8(a_ + 0x13A + slot, rng.choice([0, 1, 1, 2, 0xFF]))
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(0x22A057), rng.choice([e.r8(rt(ai.G_CUR_SECTOR)), 5]))
        e.w8(rt(0x22A055), rng.choice([0, 0, 1]))
        e.w8(rt(0x22A056), rng.choice([0, 0, 1]))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "land_item":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        e.uc.mem_write(sp, bytes(s))
        edx = rng.randrange(3136)
        e.w8(rt(G_TILEMAP) + 8 * edx + 6, rng.choice([0xFF, 0xC8, 0xC7, 0, 5, rng.randrange(256)]))
        e.w8(rt(G_TILEMAP) + 8 * edx + 0, rng.choice([0x6B, 0x6C, 0x70, 0, 0x30, rng.randrange(256)]))
        e.w8(rt(G_TILEMAP) + 8 * edx + 1, rng.choice([0, 0x1C, 0x1F, 0x30, rng.randrange(256)]))
        ebx = rng.choice([0x5D, 0x5D, 0x16, 0x18, 0x40, rng.randrange(256), 0x15D])
        if PORT and ebx & 0xFF >= 136:     # (the port's item_types has the game's 136 entries: Put item reads +13)
            ebx = rng.randrange(136)
        ecx_arg = rng.choice([100, 50, 1, 0, 0xFF, 0x164])
        stack_arg = [rng.randrange(-20, 900) & 0xFFFFFFFF, rng.randrange(-20, 800) & 0xFFFFFFFF]
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "rest":
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        ai.w16(s, 0x10C, rng.choice([0, 1, 99, 100, 4950, 5000, 5050, 9999, 10000, 10001, 0x7FFF, 0x8000, 0xFFFF,
                                     rng.randrange(0x10000)]))
        s[0x18A] = rng.choice([s[0x18A], 0, 50, 49, 100, 0x80, 0xFF, 100 - ai.s16(ai.u16(s, 0x10C)) // 100 & 0xFF])
        s[0x1BA] = rng.choice([0, 1, 5])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_REDRAW_OFF), rng.choice([0, 1, 2, 3]))
        edx = rng.choice([0, 1, 50, 100, 101, -100, -2500, -5000, 10000, -0x8000, 0x10001, rng.randrange(1 << 32)]) & 0xFFFFFFFF
        ebx = rng.choice([0, 1, 1, 0x80, 0x100, 0xFF])
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "hears":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        occ = rng.choice([rng.randrange(16), rng.randrange(0x28, 0x30), rng.randrange(16) if slot >= 0x28
                          else rng.randrange(0x28, 0x30), 0x40, 0x7F, 0x30])
        tile = rng.choice([e.r16(rt(G_SOLDIERS) + 0x200 * occ + 0x114) if occ < 0x30 else rng.randrange(3136),
                           rng.randrange(3136)]) % 3136
        e.w8(rt(G_TILEMAP) + 8 * tile + 5, occ | rng.choice([0, 0, 0x80]))
        if occ < 0x30:
            s[(0x112 if slot < 0x28 else 0x13A) + occ] = rng.choice([0, 1, 2, 0xFF, 3])
            e.w8(rt(ai.G_KNOWN) + occ, rng.choice([0, 0, 1, 2, 0xFF, 3]))
        s[0x1C0] = rng.choice([0, 0, 2, 5, 12, 0x80, 0xFF])
        s[0xBB] = rng.choice([0, 1, 2, 3])
        s[0x1D4] = rng.choice([0, 1, 2])
        s[0x1A1] = rng.choice([1, 2, 5, 8, 8])
        e.uc.mem_write(sp, bytes(s))
        edx = tile
        ebx = rng.choice([0, 1, 2, 3, 5, 6, 9, 10, 11, 12, 13, 40, 0x80, 0xFF, 0x105])
        ecx_arg = rng.choice(list(range(10)) + [4, 5, 6, 8])
        stack_arg = None
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "hit_reaction":    # --port: slots 16..0x27 and an opponent slot byte 0x40 write past last_seen_m / knowledge
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16 if PORT else 0x28), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x183] = rng.choice([0, 14, 15, 16, 60, 0x90])
        s[0xBA] = rng.choice([0, 1, 2, 2, 3, 4])
        for o in range(0x112, 0x16A):
            s[o] = rng.choice([0, 1, 1, 2, 0xFF, 3])
        e.uc.mem_write(sp, bytes(s))
        edx = rng.choice([rng.randrange(8), rng.randrange(0x28, 0x30), rng.randrange(0x30), slot])
        if edx != slot:
            e.w8(rt(G_SOLDIERS) + 0x200 * edx + 0x17A, rng.choice([edx] * 4 + [rng.randrange(0x30)] + ([] if PORT else [0x40])))
        ebx = rng.choice([0, 1, 2, 0x101])
        ecx_arg = rng.choice([0, 9, 10, 45, 99, 255, 0x1FF, rng.randrange(1 << 32)])
        stack_arg = None
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "take_damage":
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0xBA] = rng.choice([s[0xBA], 4, 0, 3])
        s[0x183] = rng.choice([0, 5, 9, 10, 14, 15, 16, 20, 30, 60, 100])
        s[0x184] = rng.choice([s[0x183], 100])
        s[0x185] = rng.choice([0, 5])
        s[0x1E1] = rng.choice([0, 0, 0x1A])
        s[0x1F3] = rng.choice([0, 1])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([0, 1, 2, 3, 4, 5, 0xD, 0xE, 0xF, 0x1B, 0x1F, 0x23, 0x28, 0x29, 0x2A,
                                                    0x2B, 0x2C, 0x2F, 0x30, 0x31, 0x32, 0x35, 0x36, 0x38, 0x39, 0x3B,
                                                    0x3F, 0x40, 0x41]))
        for o in (0x186, 0x19C, 0x1B9, 0x1F2, 0x192, 0x10C, 0x10D, 0x1A5, 0xDC, 0xD0):
            s[o] = rng.choice([0, 1, 3, 0x80, rng.randrange(256)])
        if rng.random() < 0.15:                    # first aid goes on (end of a task with flag 0: no stand-up picture)
            s[0x1B9] = rng.choice([4, 5, 6])
        if s[0x1B9] in (5, 6) and ai.u16(s, 0xD6) >= 3136:   # (a task in state 5 / 6 has its partner tile on the map;
            ai.w16(s, 0xD6, rng.randrange(3136))             # off the map the port reads 0 = slot 0, ai.rs tile_byte)
        if s[0x1F2] == 0xF6:                      # hp / (+0x1F2 + 10): the original divides by zero too
            s[0x1F2] = 0
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, 0x40]))
        e.w8(rt(ai.G_KNOCKDOWN), rng.choice([0, 0, 1]))
        e.w8(rt(0x95E0C), rng.choice([slot, 0x40]))
        e.uc.mem_write(rt(ai.ANIM_CAN_ACT) - 8, ANIMTBL)
        edx = rng.choice([4000, 0x1356, 9000, 2500])
        ebx = rng.choice([0, 1, 4, 5, 6, 14, 20, 40, 41, 100, 200])
        ecx_arg = rng.choice([1, 1, 2, 3, 4, 6, 6])
        stack_arg = [rng.choice([0, 3, 0x29, 0x40, 0x41]), rng.choice([ai.u16(s, 0x114), ai.NO_TILE, rng.randrange(3136)])]
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "impact":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x1F5] = rng.choice(list(range(1, 0xF)) + [0x16, 0x18, 0x1C, 0x20, 0x21, 0x22, 0x40])
        s[0x1C9] = rng.choice([0, 0, 1, 2, 3, 0xFF])
        s[0x1E4] = rng.choice([0, 9, 10, 50, 100])
        s[0x1D8] = rng.choice([0, 3, 7, 8, 0x2A])
        ai.w16(s, 0x116, rng.choice([0, 0x8000, 100, 300]))
        ai.w16(s, 0xF2, rng.choice([4000, 9000, 0]))
        e.uc.mem_write(sp, bytes(s))
        tile = rng.randrange(3136)
        fuzz_tile(tile)
        e.w8(rt(G_TILEMAP) + 8 * tile + 1, rng.choice([0, 0, 0xAE, 0x30, e.r8(rt(G_TILEMAP) + 8 * tile + 1)]))
        e.w8(rt(G_TILEMAP) + 8 * tile + 2, rng.choice([0, 0x10, 0x20]))
        e.w8(rt(G_TILEMAP) + 8 * tile, rng.choice([0x10, 0x6C, 0x90, e.r8(rt(G_TILEMAP) + 8 * tile)]))
        e.w8(rt(G_TILEMAP) + 8 * tile + 5, rng.choice([0x40, 0x40, 0x2A, 3, 0x29]))
        if PORT and e.r8(rt(G_TILEMAP) + 8 * tile) >= 157:   # --port: t0_class has 157 real entries (world_load.md); past
            e.w8(rt(G_TILEMAP) + 8 * tile, rng.choice([0x10, 0x6C, 0x90]))   # them the original reads active_slot (set in step 1)
        s[:] = e.uc.mem_read(sp, 0x200)
        for g, vals in ((ai.G_COMBAT, [0, 1]), (0x22A0D1, [0, 1]), (0x22A1F5, [0, 1, 2]), (0x22A057, [0, e.r8(rt(ai.G_CUR_SECTOR))]),
                        (0x22A055, [0, 0, 1]), (0x22A056, [0, 1, 0])):
            e.w8(rt(g), rng.choice(vals))
        edx = tile
        ebx = rng.randrange(-20, 900) & 0xFFFFFFFF
        ecx_arg = rng.randrange(-20, 800) & 0xFFFFFFFF
        stack_arg = rng.choice([1, 0xFFFFFFFF, 2, 0])
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name in ("w_56dd4", "pstep"):
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x1F5] = rng.choice(list(range(1, 0x10)) + [0x16, 0x5D])
        s[0x03] = rng.choice([0x5B, 0])
        s[0x05] = rng.choice([0, 50, 100])
        s[0x1D8] = rng.choice([3, 0x2A, 0x2F, 0x30, 0x40])
        me = rng.randrange(57, 3136 - 57)
        ai.w16(s, 0x114, me)
        x, y = ai.tile_x(me), ai.tile_y(me)
        ai.w16(s, 0xE4, x + rng.randrange(-4, 5))
        ai.w16(s, 0xE6, y + rng.randrange(-4, 5))
        tx, ty = x + rng.randrange(-300, 300), y + rng.randrange(-300, 300)
        ax, ay = abs(tx - x), abs(ty - y)
        sx, sy = (1 if tx >= x else 0xFFFF), (1 if ty >= y else 0xFFFF)
        if ax >= ay:
            fe, f100, major, minor = sx, 0, ax, ay
        else:
            fe, f100, major, minor = 0, sy, ay, ax
        for o, v in ((0xFA, sx), (0xFC, sy), (0xFE, fe), (0x100, f100), (0x104, 2 * minor), (0xF8, 2 * minor - major),
                     (0x102, 2 * minor - 2 * major), (0xF2, rng.choice([4500, 8500, 20000, 50, 0, 0x4651, 23500])),
                     (0xE8, rng.choice([0, 0, -20, 30, -300, -51])), (0xEA, rng.choice([-50, -200])),
                     (0xCA, rng.choice([0, 9000, 22000])), (0xC8, rng.choice([0, 3, 30])),
                     (0xDC, rng.randrange(3136)), (0xE2, rng.choice([4000, ai.NO_TILE]))):
            ai.w16(s, o, v)
        s[0xCC:0xD0] = rng.choice([0, 0, 1, 15]).to_bytes(4, "little")
        s[0x1C9] = rng.choice([0, 0, 1])
        s[0x1CB] = rng.choice([1, 2, 5, 15, 0x80])
        s[0x1FF] = rng.choice([0, 0, 10, 40])
        s[0x1BC] = rng.choice([0, 1, 0xFF])
        s[0x1C6] = rng.randrange(1, 9)
        s[0x1A1] = rng.choice(list(range(1, 9)) + [0])
        e.uc.mem_write(sp, bytes(s))
        e.w32(rt(ai.G_BUSY), rng.choice([0, 1, 3]))
        edx = rng.choice([0, 0, 1, 0x100])
        ebx = rng.choice([0, 1, 0x100])
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "launch":
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x00] = rng.choice([0] * 4 + list(range(1, 0xF)) + [0x16, 0x18, 0x1A, 0x1C] + KIND.get(0x95, []) + [0x0F, 0x11, rng.randrange(136)])
        s[0x01] = rng.choice([0, 1, 2, 5, 0x80])
        s[0x02] = rng.choice([1, 50, 100])
        s[0x03] = rng.choice([0x5B, 0x5B, 0, rng.randrange(256)])
        s[0x05] = rng.choice([0, 50, 100, rng.randrange(256)])
        x, y = rng.randrange(900), rng.randrange(800)
        ai.w16(s, 0x116, x - 2)
        ai.w16(s, 0x118, y - 4)
        far = rng.choice([0, 1, 2, 5, 19, 20, 21, 60, 300])
        ai.w16(s, 0xDE, x + rng.randrange(-far, far + 1))
        ai.w16(s, 0xE0, y + rng.choice([-far, far, rng.randrange(-far, far + 1)]))
        ai.w16(s, 0xE2, rng.choice([4000, 4500, 8000, rng.randrange(0x10000)]))
        ai.w16(s, 0xDC, rng.randrange(3136))
        s[0x1C2] = rng.choice([0xFE, 3, 0x40])
        s[0x1C3] = rng.choice([0, 1])
        s[0x1B3] = rng.choice([3, 1, 0])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, 0x40]))
        e.w32(rt(ai.G_BUSY), rng.choice([0, 1, 5]))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "fire":
        s[0x1D6] = rng.choice([0, 1, 1])
        s[0x1D1] = rng.choice([0xB, 0xC, 0xE, 0x10, 0, 2, 0xD])
        s[0xDC] = rng.randrange(256)
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(0x92DD0), rng.choice([0, 1]))
        e.w8(rt(0x92DCF), rng.choice([0, 1]))
        e.w8(rt(ai.G_ACTIVE_SLOT), rng.randrange(256))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name in ("w_59a54", "w_5aa08", "w_59ae8"):
        s[0x00] = rng.choice(list(range(0x12)) + [0x80, 0x90, rng.randrange(136)])
        s[0x01] = rng.choice([0, 1, 3])
        s[0x02] = rng.choice([10, 15, 16, 30, 49, 50, 80, 0x90])
        if FAN and 37 in FAN:              # F37: the 1.15m floor is condition > 9; 10..15 can jam
            s[0x02] = rng.choice([8, 9, 10, 11, 12, 15, 16, s[0x02]])
        s[0x1F3] = rng.choice([0, 1, 2])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([4, 0x1A, 0x1C, 0x1D, 0x1E, 0x20, 0x21, 0x22, 0x24, 0x25, 0x1B, 0x0A]))
        s[0x1B2] = rng.choice([0x10, 0x10, 0x6C])
        s[0x1B4] = rng.choice([0, 0x1C])
        s[0x1A2] = rng.choice([1, 2, 5, 8])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, 0x40]))
        e.uc.mem_write(rt(ai.ANIM_CAN_ACT) - 8, ANIMTBL)
        edx = rng.choice([tile, rng.randrange(3136)])
        if name == "w_59ae8":
            ebx = rng.randrange(3136)
            fuzz_tile(ebx)
            ecx_arg = rng.choice([0, 5, 199, 200, 0xFF, rng.randrange(256)])
            if ecx_arg < 200:
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * ecx_arg, rng.choice([0x72, 0x72, 1]))
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * ecx_arg + 6, rng.choice([0, 0, 1]))
            stack_arg = rng.choice([0, 0xFE, 0xFF, 0x76, 0x96, 0x93, 0x9B, 0x9D, 0x9E, 0x92, rng.randrange(256)])
            if FAN and 57 in FAN:          # F57: the 1.15m kind table (0x9F/0x97 +10, 0xA3 +8, 0x98 +3, 0x76 none)
                stack_arg = rng.choice([stack_arg, 0x9F, 0x97, 0xA3, 0x98, 0x76, 0])
            s[:] = e.uc.mem_read(sp, 0x200)
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name in ("stop_anim", "w_197d8", "stop_walk"):
        s[0x1A6] = rng.choice([0, 0, 1, 0x4C])
        s[0x1D1] = rng.choice([0, 2, 3, 4, 5, 6, 7, 8, 9, 0xD, 0xE, 0xE, 0xF])
        s[0x1D6] = rng.choice([0, 1, 1])
        s[0x1F6], s[0x196], s[0x1A8] = rng.randrange(256), rng.randrange(256), rng.randrange(256)
        e.w8(rt(0x92DCA), rng.randrange(256))
        s[0x17A] = slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, slot, 0x40]))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 1]))
        e.w8(rt(ai.G_BUSY_UI), rng.choice([0, 1]))
        edx = rng.choice([0, 1, 0x100, 0xFF])
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "xra_enemy":                # XRA start tiles: three candidates, items 5 and 7..9 fuzzed
        for o in (0x114, 0x116, 0x118):
            c = rng.choice([rng.randrange(3136), rng.randrange(3136), tile])
            ai.w16(s, o, c)
            fuzz_tile(c)
            e.w8(rt(G_TILEMAP) + 8 * c + 5, rng.choice([0x40, 0xC0, slot, 0x80 | slot, rng.randrange(0x30), 0x2A]))
            e.w8(rt(G_TILEMAP) + 8 * c + 2, rng.randrange(256))
        s[0x3C] = rng.choice([0x31, 0x31, 0x30, 0x32, rng.randrange(256)])
        for o in (0x54, 0x60, 0x6C):
            s[o] = rng.choice([0, 0, rng.randrange(1, 136)])
            s[o + 3] = rng.choice([0, 0, 1, rng.randrange(256)])
            for k in (1, 2, 4, 5):
                s[o + k] = rng.choice([0, rng.randrange(256)])
        s[0x17B] = rng.choice([0, 0xFC, 0xFE, rng.randrange(256)])
        for o in (0x185, 0x186, 0x1BD, 0x10C, 0x10D, 0x184):
            s[o] = rng.choice([0, rng.randrange(256)])
        e.uc.mem_write(sp, bytes(s))
    if name == "replace":
        fuzz_tile(tile)
        e.w8(rt(G_TILEMAP) + 8 * tile + 2, rng.randrange(256))
        s[0x183], s[0x18A] = rng.choice([10, 60, 100]), rng.choice([10, 60, 100])
        e.uc.mem_write(sp, bytes(s))
    if name == "end_task":
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x1B9] = rng.choice([0, 4, 4, 5, 5, 6, 3])
        s[0x1CA] = rng.choice([0, 1, 2, 0xFF])
        s[0x181] = rng.choice([0, 1])
        me = rng.randrange(3136)
        ai.w16(s, 0x114, me)
        for k in range(8):                 # partners: state 5 on this tile, counters
            a = rt(G_SOLDIERS) + 0x200 * k
            if k != slot:
                e.w8(a + 0x1B9, rng.choice([0, 5, 5, 4]))
                e.w16(a + 0xD6, rng.choice([me, me, rng.randrange(3136)]))
                e.w8(a + 0x1CA, rng.choice([0, 1, 2]))
                e.w8(a + 0x182, 1); e.w8(a + 0x138, rng.choice([1, 1, 0])); e.w8(a + 0x183, rng.choice([0, 50]))
                e.w16(a + 0x114, rng.randrange(3136))
        partner = rng.choice(list(range(8)) + [0x40])
        t = rng.randrange(3136)
        ai.w16(s, 0xD6, rng.choice([t, ai.NO_TILE]))
        e.w8(rt(G_TILEMAP) + 8 * t + 5, partner if partner < 0x40 else 0x40)
        if partner < 8 and rng.random() < 0.5:
            e.w16(rt(G_SOLDIERS) + 0x200 * partner + 0xD6, me)
        e.uc.mem_write(sp, bytes(s))
        edx = rng.choice([0, 1, 0x100])
        osave_all = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    if name == "click":
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        pick_72 = rng.random() < 0.12    # an enemy's empty hand on a ground item 0x72 beside it (cost 2, else 5)
        if pick_72:
            slot = rng.randrange(0x28, 0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        me = rng.randrange(57, 3136 - 57)
        ai.w16(s, 0x114, me)
        ai.w16(s, 0x116, ai.tile_x(me) + rng.choice([0, 0, 1, -1, 2, -3]))
        ai.w16(s, 0x118, ai.tile_y(me) + rng.choice([0, 0, 1, -1, 2, -3]))
        s[0x1F3] = 0
        ai.w16(s, 0x1E8, rng.choice([4, 4, 4, 0x1B, 0x0A, 0x43]))
        for o in (0x1A6, 0x1A7, 0x1C2, 0x1C3, 0x1D8, 0x1BE, 0x1CD, 0xB0, 0xB1, 0xB2, 0xB3):
            s[o] = rng.randrange(256)
        s[0x1A6] = rng.choice([s[0x1A6], 0x0F, 0x45, 0x49, 0])
        s[0x01] = rng.choice([0, 1, 3])
        s[0x02] = rng.choice([0, 50, 100])
        s[0x183] = rng.choice([30, 60, 100, rng.randrange(1, 101)])
        s[0x18A] = rng.choice([30, 60, 100, rng.randrange(1, 101)])
        s[0x1A5] = rng.choice([0, 2, 5, 20, 40])
        s[0x1B2] = rng.choice([0x10, 0x10, 0x6C])
        s[0x1B4] = rng.choice([0, 0x1C])
        e.uc.mem_write(sp, bytes(s))
        tile = rng.choice([me, me + 1, me + 1, me + 56, me + 57, me - 55, me + 2, me + 3 * 56, me + rng.randrange(-400, 400),
                           me + rng.randrange(-8, 9) + 56 * rng.randrange(-8, 9)] * 2) % 3136
        for t_ in (tile, me):
            fuzz_tile(t_)
        e.w8(rt(G_TILEMAP) + 8 * tile + 6, rng.choice([0, 5, 0xC8, rng.randrange(256)]))
        gi = e.r8(rt(G_TILEMAP) + 8 * tile + 6)
        if gi < 0xC8:
            e.w8(rt(ai.G_GROUND_ITEMS) + 12 * gi, rng.choice([0x72, 1, 0x40]))
        spot = rng.choice([tile, (tile + 1) % 3136, rng.randrange(3136)])
        e.w16(rt(ai.G_MISSION_SPOT), spot)
        fuzz_tile(spot)
        s[:] = e.uc.mem_read(sp, 0x200)
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 1]))
        e.w8(rt(ai.G_CTRL_HELD), rng.choice([0, 0, 1]))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, 0x40]))
        e.w8(rt(0x1163BD), rng.choice([0, 1]))
        e.uc.mem_write(rt(ai.ANIM_CAN_ACT) - 8, ANIMTBL)
        if PORT and e.r8(rt(ai.G_GROUND_ITEMS) + 12 * 255) == 0x72:   # (entry 255: never 0x72 in the game; click.md)
            e.w8(rt(ai.G_GROUND_ITEMS) + 12 * 255, 0)
        edx = tile
        ebx = rng.choice([0, 0] + list(range(1, 0xF)) + [0xF, 0x10, 0x11, 0x14, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
                          0x1C, 0x1D, 0x1E, 0x20, 0x21, 0x45, 0x45, 0x46, 0x46, 0x49, 0x4A, 0x4C, 0x4D, 0x57, 0x58,
                          0x5D, 0x76, 0x87, rng.randrange(136)])
        if pick_72:
            ebx = 0
            s[0x1A5] = rng.choice([2, 3, 4, 5, 20])
            ai.w16(s, 0x116, ai.tile_x(tile))
            ai.w16(s, 0x118, ai.tile_y(tile))
            e.uc.mem_write(sp, bytes(s))
            gi = rng.choice([5, 60, 150, 199, 200, 0xFF])
            e.w8(rt(G_TILEMAP) + 8 * tile + 6, gi)
            if gi < 0xC8:
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * gi, rng.choice([0x72, 0x72, 1]))
        if PORT:                   # (0x4D: Move order with confirm 1; the library has no confirm: the front-end
            e.w8(rt(0x1163C6), 0)  # keeps the game's confirm-move option off, port_api.md op 140)
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "anim_turn":
        s[0x1D1] = rng.choice([1, 1, 0, 2])
        s[0x1D6] = rng.choice([1, 1, 0])
        s[0x17A] = slot = rng.choice([rng.randrange(8), rng.randrange(8, 17), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        e.uc.mem_write(sp, bytes(s))
        edx = rng.choice([0, 1, 0x100])
    if name in ("anim_frame", "walk_step", "soldier_frame", "active_frame"):
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        me = rng.choice([rng.randrange(57, 3136 - 57)] * 5 + [rng.randrange(3136 - 57, 3136)] * (not PORT))
        ai.w16(s, 0x114, me)
        ai.w16(s, 0x116, ai.tile_x(me) + rng.choice([0, 0, 5, 7, -5, -6, 9]))
        ai.w16(s, 0x118, ai.tile_y(me) + rng.choice([0, 0, 4, 6, -4, -6, 8]))
        for o in (0x124, 0x126, 0x128):
            ai.w16(s, o, rng.choice([ai.u16(s, 0x114 + (o - 0x124)), rng.randrange(0x10000)]))
        if PORT and rng.random() < 0.5 and ai.u16(s, 0x124) >= 3136:   # a leg to an off-map tile reads past the map
            ai.w16(s, 0x124, rng.choice([rng.randrange(3136), ai.NO_TILE]))
        elif PORT and ai.u16(s, 0x124) >= 3136 and ai.u16(s, 0x124) != ai.NO_TILE:
            ai.w16(s, 0x124, ai.u16(s, 0x114))
        s[0x1A1] = rng.choice(list(range(1, 9)) * 3 + [0, 9, 0x80])
        s[0x1A2] = rng.choice([s[0x1A1], rng.randrange(1, 9)])
        s[0x1A3] = rng.choice([0, 1, 0xFF])
        for o in (0x9C, 0x17D, 0x1A6, 0x185, 0x1B9, 0x1AD, 0x1AF, 0x19E):
            s[o] = rng.choice([0, 0, 1, rng.randrange(256)])
        s[0x1A6] = rng.choice([0x4C, 0, s[0x1A6]])
        s[0x183] = rng.choice([0, 10, 14, 15, 60])
        s[0x18A] = rng.choice([50, 70, 71, 100])
        s[0x1F3] = rng.choice([0, 1, 2])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([4, 0xC, 0x15, 0, 0x2A, 0x3F, 0x40, rng.randrange(0x44)]))
        ai.w16(s, 0xD4, rng.choice([0, 0, 3]))
        ai.w16(s, 0xDC, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
        s[0x01] = rng.choice([0, 1])
        sbuf = rt(ai.ANIM_FRAMES) + 0x50 * rng.randrange(0x44)     # a fuzzed script in the table (restored per case)
        OPS = list(range(0x1B6, 0x1F4)) * 3 + [5, 0xA, 0x2D, 0x37, 0x54, 0x57, 0x85, 0x8F, 0x93, 0x96, 0x98, 0x1CC,
                                               0xB5, 0xB6, 0x1F4, 0x1F4] + list(range(0x1D7, 0x1DF)) * 4
        words = [rng.choice(OPS + [rng.randrange(0x99, 0x1B6), rng.randrange(500, 0x230), rng.randrange(8)])
                 for _ in range(rng.choice([1, 3, 8, 16]))] + [0x99] * 12
        words = [rng.randrange(40) if k and words[k - 1] == 0x1C6 else wd for k, wd in enumerate(words)]
        e.uc.mem_write(sbuf, b"".join((wd & 0xFFFF).to_bytes(2, "little") for wd in words))
        ptr = rng.choice([sbuf, sbuf, sbuf, rt(ai.ANIM_FRAMES) + 0x50 * rng.randrange(0x44)])
        s[0x90:0x94] = ptr.to_bytes(4, "little")
        ai.w16(s, 0x136, 0)
        if rng.random() < 0.1:
            e.w16(sbuf, 0x1E6)
            s[0x90:0x94] = sbuf.to_bytes(4, "little")
            ai.w16(s, 0x136, 0x28)
        e.uc.mem_write(sp, bytes(s))
        for st in (0, 1, -1, 56, -56, 57, -57, 55, -55):
            if 0 <= me + st < 3136:
                fuzz_tile(me + st)
        if me + 1 < 3136:
            e.w8(rt(G_TILEMAP) + 8 * (me + 1) + 1, rng.choice([0x82, 0x83, e.r8(rt(G_TILEMAP) + 8 * (me + 1) + 1)]))
        vx = ai.s16(ai.u16(s, 0x116)) - ai.s16(ai.u16(s, 0x11A))
        e.w32(rt(ai.G_VIEW_X), rng.choice([vx + 20, vx + 21, vx - 300, e.r32(rt(ai.G_VIEW_X))]) & 0xFFFFFFFF)
        vy = ai.s16(ai.u16(s, 0x118)) - (ai.s16(ai.u16(s, 0x11C)) if me >= 56 else 0)
        e.w32(rt(ai.G_VIEW_Y), rng.choice([vy + 20, vy + 21, vy - 200, e.r32(rt(ai.G_VIEW_Y))]) & 0xFFFFFFFF)
        for g_, o_, v_ in ((ai.G_VIEW_W, ai.G_VIEW_X, vx), (ai.G_VIEW_H, ai.G_VIEW_Y, vy)):   # right / bottom edge
            org = ai._i32(e.r32(rt(o_)))
            e.w32(rt(g_), rng.choice([v_ - org, v_ - org + 1, e.r32(rt(g_))]) & 0xFFFFFFFF)
        for k_ in range(0x80 + 0x280):            # last-seen rows hold real tiles (forget tiles, word 0x1F1)
            a_ = rt(ai.G_LAST_SEEN_M) + 2 * k_ if k_ < 0x80 else rt(ai.G_LAST_SEEN_E) + 2 * (k_ - 0x80)
            e.w16(a_, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
        s[:] = e.uc.mem_read(sp, 0x200)
        s[0xBA] = rng.choice([s[0xBA], 2, 1])
        s[0x1BE] = rng.choice([0, 1])
        for o_ in range(0x13A, 0x162):
            s[o_] = rng.choice([0, 0, 1])
        s[0x18A] = rng.choice([50, 70, 71, 100])
        e.uc.mem_write(sp, bytes(s))
        e.uc.mem_write(rt(ai.G_PATHS) + 0x50 * slot, bytes(rng.randrange(8) for _ in range(0x50)))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1]))
        e.w8(rt(0x1163C3), rng.choice([0, 1]))
        e.w8(rt(0x92DDF), rng.choice([0, 0, 1]))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name in ("walk_step", "soldier_frame", "active_frame"):   # a step toward a neighbour tile: the point crosses into it often
        e.uc.mem_write(rt(ai.ANIM_CAN_ACT) - 8, ANIMTBL)
        s[:] = e.uc.mem_read(sp, 0x200)
        me = ai.s16(ai.u16(s, 0x114))
        nb = me + rng.choice([1, -1, 56, -56, 57, -57, 55, -55, 0])
        tx, ty = ai.tile_x(nb), ai.tile_y(nb)
        ai.w16(s, 0x126, tx + rng.choice([0, 0, 0, 1, -2]))
        ai.w16(s, 0x128, ty + rng.choice([0, 0, 0, 1, -2]))
        f = rng.random()
        ai.w16(s, 0x116, int(ai.tile_x(me) + f * (tx - ai.tile_x(me))) + rng.choice([0, 0, 1, -1]))
        ai.w16(s, 0x118, int(ai.tile_y(me) + f * (ty - ai.tile_y(me))) + rng.choice([0, 0, 1, -1]))
        s[0x19F] = rng.choice([0, 1, 2, 3, 4, 6, 8, 16, rng.randrange(256)])
        s[0x1A2] = rng.choice([0, 1, 2, 3, 4, 5, 6, 7, 8, 2, 4, 6])
        s[0x1A5] = rng.choice([0, 4, 5, 6, 0x80, rng.randrange(256)])
        s[0xBA] = rng.choice([0, 1, 2, 0xFF])
        an = rng.choice([0, 1, 2, 3, 4, 5, 0xC, 0x15, 0x16, 0x17, 0x18, 0x3B, 0x3C, 0x3D, rng.randrange(0x44)])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], an)
        s[0x18A] = rng.choice([0, 19, 20, 50, 0x80])
        s[0x1BA] = rng.choice([0, 0, 1])
        s[0x1D5] = rng.choice([0, 0, 1])
        s[0x179] = rng.choice([0, 5, 0x3B, 0x3C, 0x3F, 0x40, 0x50])
        s[0x183] = rng.choice([0, 1, 15, 60, 0x80])
        s[0x9C] = rng.choice([0, 1, 0xFF])
        s[0x1F2] = rng.choice([0, 0xFF, rng.randrange(256)])
        ai.w16(s, 0x1F8, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136)]))
        if rng.random() < 0.3:           # the point one pixel before a tile edge (the facing bias decides the tile)
            ai.w16(s, 0x116, 16 * rng.randrange(1, 56) - rng.choice([1, 1, 2, 5, 6]))
            ai.w16(s, 0x118, 14 * rng.randrange(1, 56) - rng.choice([1, 3, 4]))
            ai.w16(s, 0x126, ai.u16(s, 0x116))
            ai.w16(s, 0x128, ai.u16(s, 0x118))
            me = combat.view_tile(ai.u16(s, 0x116), ai.u16(s, 0x118))
            ai.w16(s, 0x114, me)
        if rng.random() < 0.4:           # the tired-walk message and splash edges
            ai.w16(s, 0x1E8 + 2 * s[0x1F3], an := rng.choice([0x14, 0x15, 0x16, 0x17, 0x18, 0x3B, 0x3C]))
            s[0x18A] = rng.choice([0x13, 0x14])
            s[0x179] = rng.choice([0x3B, 0x3C, 0x3F, 0x40])
            s[0x1BA] = s[0x1D5] = 0
            if rng.random() < 0.5 and slot >= 8:
                s[0x17A] = slot = rng.randrange(8)
                sp = rt(G_SOLDIERS) + slot * 0x200
        if rng.random() < 0.4:           # slots on the merc / guard border
            s[0x17A] = slot = rng.choice([7, 8, 9])
            sp = rt(G_SOLDIERS) + slot * 0x200
            e.w8(rt(ai.G_SELECTED), slot)
        if FAN and 48 in FAN:            # F48: the backward-walk byte +0x196 and odd facings (own random source)
            r48_ = random.Random(104729 * i + 48)
            s[0x196] = r48_.choice([0, 0, 1, 0xFF, r48_.randrange(256)])
            if r48_.random() < 0.2:
                s[0x1A2] = r48_.choice([0x80, 0x84, 0x7F, 9, 12, r48_.randrange(256)])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.ANIM_STILL) + 8 * (an & 0xFF), rng.choice([0, 0, 1]))
        for t_ in range(max(0, me - 57), min(3136, me + 58)):
            if rng.random() < 0.3:
                e.w8(rt(G_TILEMAP) + 8 * t_, rng.choice([0x10, 0x96, 0x97, 0x99, 0x9A, rng.randrange(256)]))
                e.w8(rt(G_TILEMAP) + 8 * t_ + 1, rng.choice([0, 0, 0, 1, rng.randrange(256)]))
            if rng.random() < 0.3:
                e.w8(rt(G_TILEMAP) + 8 * t_ + 5, rng.choice([0x40, 0xC0, 0x40, rng.randrange(0x30), rng.randrange(256)]))
            elif rng.random() < 0.5:
                e.w8(rt(G_TILEMAP) + 8 * t_ + 5, 0x40)
        nt = ai.walk_move(bytearray(s), None)     # the tile the step lands on: free, on the splash edges
        if 0 <= nt < 3136 and nt != me and rng.random() < 0.7:
            e.w8(rt(G_TILEMAP) + 8 * nt + 5, rng.choice([0x40, 0xC0]))
            e.w8(rt(G_TILEMAP) + 8 * nt, rng.choice([0x96, 0x97, 0x97, 0x98, 0x10]))
            e.w8(rt(G_TILEMAP) + 8 * nt + 1, rng.choice([0, 0, 0, 1, 6, rng.randrange(256)]))
            s[0x1AE] = rng.choice([0, 1, 0x33])
            s[0x183] = rng.choice([1, 15, 60, 0, 0x80])
            e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_SPLASH) + e.r8(rt(ai.G_CUR_SECTOR)), rng.choice([0, 1, 2, 5, 0xFF, 0x80]))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, slot, 0x40]))
        e.w8(rt(0x93458), rng.choice([0, 1]))
        e.w8(rt(0x1163C2), rng.choice([0, 1]))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name in ("soldier_frame", "active_frame"):   # the walk_step soldier, then the still-animation part
        s[:] = e.uc.mem_read(sp, 0x200)
        s[0x1B8] = rng.choice([0, 0, 1])
        if rng.random() < 0.3:           # at the step point
            ai.w16(s, 0x126, ai.u16(s, 0x116))
            ai.w16(s, 0x128, ai.u16(s, 0x118))
        ai.w16(s, 0x120, rng.choice([ai.u16(s, 0x114), ai.NO_TILE, 0xC40, 0xC42, 0x8000, rng.randrange(3136)]))
        s[0x9D] = rng.choice([0, 0, 0, 1])
        s[0x19E] = rng.choice([0, 1, 5, 9])
        s[0x1A1] = rng.choice([1, 2, 5, 8])
        s[0xBA] = rng.choice([0, 1, 2, 2, 0xFF])
        an = rng.choice([0, 3, 4, 5, 0x10, 0x11, 0x11, 0x2B, rng.randrange(0x44)])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], an | rng.choice([0, 0, 0 if PORT else 0x100]))   # --port: words past the tables read 0 there
        e.w8(rt(ai.ANIM_STILL) + 8 * an, rng.choice([0, 1]))
        e.w8(rt(ai.ANIM_CAN_ACT) + 8 * an, rng.choice([0, 1]))
        ai.w16(s, 0xD2, rng.choice([0, 1, 5, 0xFFFF, 0x8000]))
        s[0x183] = rng.choice([0, 14, 15, 60, 60, 0x80])
        me_ = ai.u16(s, 0x114)
        ai.w16(s, 0x12E, rng.choice([me_, me_, ai.u16(s, 0x124), rng.randrange(3136)]))
        if rng.random() < 0.3:           # at the waypoint
            for o_ in (0x124, 0x126, 0x128):
                ai.w16(s, o_, ai.u16(s, o_ - 0x10))
        s[0x1B9] = rng.choice([0, 0, 5])
        s[0x1A2], s[0x1A4] = rng.choice([1, 3, 0x80]), rng.choice([1, 4, 0x80])
        s[0x182], s[0x138], s[0x188] = rng.choice([0, 1, 1]), rng.choice([0, 1, 1]), rng.choice([0, 0, 1])
        if name == "soldier_frame" and SFC_RNG.random() < 0.3:   # the move to the waypoint: the not-done-with-target refusal (+0x19C 3,
            s[0x19C] = SFC_RNG.choice([3, 3, 3, 2])                # +0x1D5, the watched tile +0xD0 with a living occupant,
            s[0x1D5] = SFC_RNG.choice([1, 5, 0x80, 0])             # the hand item 1..0x10 + its state byte) (ja1-cov
            s[0xBA] = 2                                            # 2026-11-06: 0 calls)
            if s[0x183] < 15:
                s[0x183] = SFC_RNG.choice([15, 60])
            me_ = ai.u16(s, 0x114)
            d_ = SFC_RNG.choice([me_ - 1, me_ + 1, me_ - 56, me_ + 56, me_ - 57, SFC_RNG.randrange(3136), ai.NO_TILE])
            ai.w16(s, 0xD0, d_)
            s[0] = SFC_RNG.choice([1, 5, 0xE, 0xF, 0x10, 0x10, 0x11, 0])
            s[1] = SFC_RNG.choice([0, 1, 1, 5])
            s[0x17E], s[0x182] = SFC_RNG.choice([0, 0, 0, 1]), SFC_RNG.choice([1, 1, 1, 0])   # (step 1 gates)
            if 0 <= d_ < 3136:
                o_ = SFC_RNG.choice([SFC_RNG.randrange(0x30), SFC_RNG.randrange(8), 0x40, 0x7F])
                e.w8(rt(G_TILEMAP) + 8 * d_ + 5, o_ | SFC_RNG.choice([0, 0x80]))
                if o_ < 0x30:
                    e.w8(rt(G_SOLDIERS) + 0x200 * o_ + 0x183, SFC_RNG.choice([0, 10, 50, 0x80]))
            if SFC_RNG.random() < 0.5:      # made cases for the test after the type gate (ledger row 7212): a type 0xF / 0x10
                s[0x19C], s[0x1D5], s[1], s[0x17E], s[0x182] = 3, 1, 1, 0, 1   # hand item refuses only when TileDistance is 1 AND
                s[0] = SFC_RNG.choice([0xE, 0xF, 0x10, 0x10])                    # both tile pixel values are equal (0x1C6F9..0x1C757)
                d_ = SFC_RNG.choice([me_ - 1, me_ + 1, me_, me_ + 56])
                if 0 <= d_ < 3136:
                    ai.w16(s, 0xD0, d_)
                    o_ = SFC_RNG.choice([k_ for k_ in range(8) if k_ != slot])
                    e.w8(rt(G_TILEMAP) + 8 * d_ + 5, o_)
                    e.w8(rt(G_SOLDIERS) + 0x200 * o_ + 0x183, 50)
                ai.w16(s, 0x1E8 + 2 * s[0x1F3], 5)
                e.w8(rt(ai.ANIM_STILL) + 8 * 5, 1)
                e.w8(rt(ai.ANIM_CAN_ACT) + 8 * 5, 1)
                ai.w16(s, 0x12E, SFC_RNG.choice([t_ for t_ in (me_ + 2, me_ - 2, me_ + 112) if 0 <= t_ < 3136 and t_ != ai.u16(s, 0x124)] or [me_]))
                base["tile_dist"] = [SFC_RNG.choice([1, 1, 1, 0, 5]) for _ in range(4)]   # (the stub's answers)
                script["tile_dist"] = list(base["tile_dist"])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(0x92DEC), rng.choice([slot, 0x40]))
        e.w8(rt(0x284F26), rng.choice([0, 0, 1]))
        e.w8(rt(0x284F27), rng.choice([5, 0x10, 0xFA, 0xFE]))
        e.w8(rt(0x22A02E), rng.choice([0, 1]))
        if name == "active_frame":
            scene = rng.choice(["", "", "act", "step"])
            if rng.random() < 0.2 or (scene == "step" and rng.random() < 0.5):   # merc / guard / enemy borders; mercs
                s[0x17A] = slot = rng.choice([7, 8, 0xF, 0x10, 0x27, 0x28] if rng.random() < 0.4 else range(8))
                sp = rt(G_SOLDIERS) + slot * 0x200
            me_ = ai.u16(s, 0x114)
            ai.w16(s, 0x106, rng.choice([ai.NO_TILE, ai.NO_TILE, me_, rng.randrange(3136)]))
            s[0x1A8], s[0x1D5] = rng.choice([0, 1]), rng.choice([0, 1, 0x80])
            for j_ in range(8):
                s[0x13A + j_] = rng.choice([0, 1, 1, 2])
                e.w8(rt(G_SOLDIERS) + 0x200 * (0x28 + j_) + 0x13A + slot, rng.choice([0, 1, 1, 2]))
            s[0x193] = rng.choice([0, 1, 1])
            ai.w16(s, 0xF4, rng.choice([me_, rng.randrange(3136)]))
            s[0x177], s[0x196] = rng.randrange(256), rng.choice([0, 0, 1])
            if 0 <= me_ < 3136:
                e.w8(rt(G_TILEMAP) + 8 * me_ + 6, rng.choice([0xFF, 0xFF, 0, 3, 199, 200]))
                e.w16(rt(ai.G_MOVECODE) + 2 * me_, rng.choice([0, 1000, 1200, 1500, 1600, 2000, 2001, 3000, 3600, 5000,
                                                              25300, 25400, 25500, e.r16(rt(ai.G_MOVECODE) + 2 * me_)]))
            for k_ in (0, 3, 199):
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * k_, rng.choice([0x70, 0x71, 0x11, 0x11, 5]))
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * k_ + 2, rng.choice([9, 10, 50, 0x80]))
            s[0x18A] = rng.choice([0, 0, 1, 50, 0x80])
            s[0x183], s[0x184] = rng.choice([s[0x183]] * 6 + [0]), rng.choice([0, 30])
            s[0xB0:0xB4] = rng.choice([0, 1, 0xFFFFFFFF]).to_bytes(4, "little")
            s[0x1B2] = rng.choice([0, 0x6B, 0x6C, 0x99, 0x9A, 0xA0])
            s[0x1B4] = rng.choice([0, 0x1C, 0x1F, 5])
            s[0x9C] = rng.choice([0, 1, 0xFF])
            ai.w16(s, 0xEC, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136)]))
            s[0x1A6] = rng.choice([0, 0, 1, 5, 0xFD, 0xFE, 0xFF])
            s[0] = rng.choice([s[0x1A6], s[0x1A6], 0, 5, 0x10, 0x11, 0x20])
            s[0x1B0] = rng.choice([0, 0, 1])
            ai.w16(s, 0x134, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
            s[0x16B], s[0x1A5], s[0xEF], s[0xF0] = rng.choice([0, 1]), rng.choice([0, 5, 20, 40]), rng.choice([0, 5, 30]), 7
            script_c = (s[0x1A5] - (s[0xEF] if rng.random() < 0.5 else 0)) & 0xFF
            base["w_59ae8"] = [rng.choice([script_c - 1, script_c, script_c + 1, 0x80, 0x105]) & 0xFFFF] * 8
            script = {k_: list(v_) for k_, v_ in base.items()}
            ai.w16(s, 0xD4, rng.choice([0, 1, 5, 0xFFFF]))
            s[0x1AF] = rng.randrange(0x40)
            ai.w16(s, 0xDC, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
            s[0x1BE] = rng.choice([0, 1, 2])
            ai.w16(s, 0x12A, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
            s[0x19C] = rng.choice([0, 3, 4, 4])
            if rng.random() < 0.3:       # items that deep water soaks (item table +8), condition edges
                soak_ = [k_ for k_ in range(1, 0xA0) if e.r8(rt(ai.ITEM_TBL) + 12 * k_ + 8)]
                for k_ in range(12):
                    s[12 * k_] = rng.choice(soak_ + [0])
                    s[12 * k_ + 2] = rng.choice([0, 5, 9, 10, 50, 0x80])
            if rng.random() < 0.2:       # the exhausted fall: animation edges
                ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([3, 4, 0x27, 0x28]))
            if rng.random() < 0.4:       # at the step target
                ai.w16(s, 0x124, me_)
            if scene:                    # walking, at the step point, hp 15+, no water
                s[0x1B8], s[0x183], s[0x1B2] = 0, rng.choice([15, 60]), 0
                ai.w16(s, 0x126, ai.u16(s, 0x116))
                ai.w16(s, 0x128, ai.u16(s, 0x118))
            if scene == "act":           # the queued action at the step target
                ai.w16(s, 0x124, me_)
                s[0x1B0], s[0x16B] = 0, rng.choice([1, 1, 0])
                s[0x1A6] = rng.choice([5, 0xFE, 0x20])
                s[0] = rng.choice([s[0x1A6], s[0x1A6], 0])
            if scene == "step":          # the next step of the path
                ai.w16(s, 0x124, (me_ + rng.choice([1, 56, -1])) & 0xFFFF)
                s[0x18A] = rng.choice([1, 50, 0])
                if not s[0x18A]:         # out of energy: the walk stops from animation 4 on
                    ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([3, 4, 5]))
                s[0x196] = rng.choice([0, 1, 1])
                ai.w16(s, 0xD4, rng.choice([1, 2, 5]))
                s[0x1A5], s[0xEF] = rng.choice([0, 1, 2, 3, 4, 5, 0x80, 100]), rng.choice([0, 1, 2])
                s[0x16B] = rng.choice([0, 1, 2])
                s[0xBA], s[0xB9] = rng.choice([0, 1, 2]), rng.choice([0, 1])
                s[0x192] = rng.choice([0, 5, 40, 0x80])
                s[0x19C], s[0x1AF] = rng.choice([0, 2, 2]), rng.choice([0, 5, 6, 15, 16, 30])
                s[0x9C] = rng.choice([0, 1, 2, 0xFF])
                for k_ in range(8):      # mercs in play re-view after a break
                    e.w8(rt(G_SOLDIERS) + 0x200 * k_ + 0x138, rng.choice([0, 1]))
                    e.w8(rt(G_SOLDIERS) + 0x200 * k_ + 0x9D, 0)
                s[0x138], s[0x9D] = rng.choice([0, 1, 1]), 0
                if rng.random() < 0.4:   # wading / swimming step animations
                    ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A]))
                    s[0x18A] = rng.choice([1, 50])
                if rng.random() < 0.4:   # the reserve decides the AP stop
                    s[0x16B], s[0x1A5], s[0xEF] = 1, rng.choice([3, 4, 5, 6]), rng.choice([2, 3, 4])
                    for o_ in (0x116, 0x118):   # the point (= step point) off the tile's own point
                        ai.w16(s, o_, ai.u16(s, o_) + 3)
                        ai.w16(s, o_ + 0x10, ai.u16(s, o_))
                if rng.random() < 0.3:   # a forgetful merc (+0x19C 2): the forgot-where-I-was-going quote 0xD
                    s[0x19C], s[0x1AF], s[0x1A5] = 2, rng.choice([6, 16]), rng.choice([0, 3, 20])
                    base["Random"] = [0, rng.choice([0, 1, 2, 7])] + base["Random"]
            e.uc.mem_write(sp, bytes(s))
            e.w8(rt(ai.G_AP_WARNED) + (slot & 7), rng.choice([0, 1]))
            for a_ in (0x92DDF, 0x92DCF, 0x92DD0, 0x92DCB, 0x22A0FF, 0x1163C1):
                e.w8(rt(a_), rng.choice([0, 0, 1]))
            if FAN and 25 in FAN and slot < 8 and F24_RNG.random() < 0.3:   # F25: the step tile's mine in the blast list
                me_ = ai.u16(s, 0x114)                                        # or not (any next tile: all 8 neighbours)
                for d_ in (-57, -56, -55, -1, 1, 55, 56, 57):
                    if 0 <= me_ + d_ < 3136:
                        e.w16(rt(ai.G_BLAST_LIST) + 2 * F24_RNG.randrange(30), me_ + d_)
            if FAN and 24 in FAN and slot < 8 and s[0x16B] == 1 and F24_RNG.random() < 0.7:   # F24 (StepStart 0x1A587
                e.w8(sp + 0xEF, min(s[0x1A5] + F24_RNG.choice([1, 2, 5, 20]), 255))   # signed): AP under the reserve in combat
                e.w8(rt(ai.G_AP_WARNED) + slot, 0)                           # (own stream: the plain cases stay)
                e.w8(rt(0x92DCB), 1)
            if FAN and 61 in FAN and slot < 8 and F61_RNG.random() < 0.5:   # F61 (StepStart 0x1A650): the step fits in
                a_ = F61_RNG.choice([4, 6, 10, 20, 40])                      # the AP but not over the reserve, so the
                s[0x16B], s[0x1A5], s[0xEF] = 1, a_, a_ - F61_RNG.choice([0, 1, 2, 3])   # low-AP stop selects the
                e.uc.mem_write(sp, bytes(s))                                # merc (own stream)
                e.w8(rt(ai.G_AP_WARNED) + slot, 0)
                e.w8(rt(0x92DCB), 1)
            e.w32(rt(0x229DD8), rng.choice([0, 1, 0x100, 0x1000000]))
            sec_ = ai.G_SECTORS + 0x58 + 0xB1 * e.r8(rt(ai.G_CUR_SECTOR))
            for k_ in range(10):         # sector tile triggers on the soldier's tile
                e.w16(rt(sec_ + 4 * k_), rng.choice([me_, ai.NO_TILE, 0xC40, rng.randrange(3136)]))
                e.w16(rt(sec_ + 4 * k_ + 2), rng.choice([0, 5, 40, 0xFFFB, 0xFFD8, 0x8000]))
            s[0x192] = rng.choice([s[0x192], 5, 40])
            e.uc.mem_write(sp, bytes(s))
            e.w8(rt(0x284F11), rng.choice([slot, slot, 0x40, 3]))
            if scene == "act":
                e.w8(rt(0x92DCB), rng.choice([1, 1, 0]))
                e.w8(rt(0x284F11), rng.choice([slot, slot, 3]))
            if scene == "step":
                e.w8(rt(0x92DD0), 0)
                e.w8(rt(0x92DCF), 0)
                e.w8(rt(0x92DCB), rng.choice([0, 1, 1]))
                for st_ in ai.DIR_STEP_1:   # the neighbours: ground, object, occupant, ground item, move code (AP)
                    nb_ = ai.step_tile(ai.s16(me_), st_)
                    if 0 <= nb_ < 3136:
                        m_ = rt(G_TILEMAP) + 8 * nb_
                        e.w8(m_, rng.choice([0x10, 0x60, 0x6C, 0x96, 0x97, 0x99, 0x9A, e.r8(m_)]))
                        e.w8(m_ + 1, rng.choice([0, 0, 1, 0x20, 0x21, 0x1C, 0x43, 0x3D, e.r8(m_ + 1)]))   # 0x43 / 0x3D: breakable (TileStandable < 0)
                        e.w8(m_ + 5, rng.choice([0x40, 0x40, 0xC0, rng.randrange(0x30)]))
                        e.w8(m_ + 6, rng.choice([0xFF, 0xFF, 3, 199]))
                        e.w16(rt(ai.G_MOVECODE) + 2 * nb_, rng.choice([0, 1000, 1200, 2000, 5000, 25500, 25500]))
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * 3, rng.choice([0x13, 0x2C, 0x12, 0x14]))
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * 3 + 2, rng.choice([0, 5, 0xFB, 0x80, 40]))
                e.w8(rt(ai.G_AP_WARNED) + (slot & 7), rng.choice([0, 0, 1]))
                e.w32(rt(0x95DC8), 0)
                script = {k_: list(v_) for k_, v_ in base.items()}
                e.uc.mem_write(rt(ai.G_PATHS) + 0x50 * slot, bytes(rng.randrange(8) for _ in range(0x50)))
            e.w8(rt(0x284F10), rng.choice([0x40, 0x40, 2]))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "frame_checks":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x1F3] = rng.choice([0, 1, 2])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([3, 4, 5, 0x27, 0x28, 0xFF if PORT else 0x104, rng.randrange(0x44)]))
        s[0x18A] = rng.choice([0, 0x80, 1, 50])     # (--port: animation words past the 256-row table read as 0 there)
        s[0x186] = rng.choice([0, 0, 1])
        s[0xB4:0xB8] = rng.choice([0, 0, 1, 0xFFFFFFFF, 0x100, 0x1000000]).to_bytes(4, "little")
        s[0xB3], s[0xB8] = rng.choice([0, 1]), rng.choice([0, 1])     # the bytes beside the timer must not count
        s[0], s[1] = rng.choice([0, 5, 0x16]), rng.choice([0, 0, 1, 0x80])
        s[0x1BF] = rng.choice([0, 0, 3])
        s[0x1CD] = rng.choice([0, 0, 1])
        e.uc.mem_write(sp, bytes(s))
        e.w32(rt(0x229DD8), rng.choice([0, 1, 0xFFFFFFFF]))
        e.w8(rt(0x22A02E), rng.choice([0, 1, 1]))
        e.w8(rt(0x92DEC), rng.choice([slot, 0x40]))
        e.w8(rt(0x284F26), rng.choice([0, 0, 1]))
        e.w8(rt(0x284F27), rng.choice([5, 0x10, 0xFA, 0xFE]))
        gsave = [bytes(e.uc.mem_read(rt(a), k__)) for a, k__ in GLOBALS_W]
    if name == "merc_quote":
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x179] = rng.randrange(256)
        s[0xC0:0xC4] = rng.choice([0, 0, 1, 400, 0xFFFFFFFF, 0x80000000, 0x100]).to_bytes(4, "little")
        s[0xBF], s[0xC4] = rng.choice([0, 1]), rng.choice([0, 1])     # the bytes beside the timer must not count
        s[0x16C] = rng.choice([0, 0, 1])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(0x92DEC), rng.choice([slot, slot, slot, 0x40, rng.randrange(8)]))
        e.w8(rt(0x92DED), rng.randrange(256))
        e.w16(rt(0x92DEE), rng.choice([ai.NO_TILE, rng.randrange(3136), rng.randrange(0x10000)]))
        e.w8(rt(0x284F24), rng.randrange(256))
        e.w8(rt(0x284F25), rng.choice([0, 1, 0x80]))
        e.w8(rt(0x284F26), rng.choice([0, 0, 0, 1, 2, 0x7F, 0x80, 0xFF]))
        e.w8(rt(0x284F27), rng.choice([0, 1, 5, 5, 0x20, 0x21, 0x80, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
                                       rng.randrange(256)]))
        gsave = [bytes(e.uc.mem_read(rt(a), k__)) for a, k__ in GLOBALS_W]
    if name == "enemies_see":
        slot = rng.randrange(16)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        e.uc.mem_write(sp, bytes(s))
        for k_ in range(0x27, 0x30):     # 0x27: the slot below the enemies, which must not count
            a_ = rt(G_SOLDIERS) + 0x200 * k_
            e.w8(a_ + 0x182, rng.choice([0, 1, 1, 1]))
            e.w8(a_ + 0x138, rng.choice([0, 1, 1, 1]))
            e.w8(a_ + 0x183, rng.choice([0, 14, 15, 60, 0x80]))
        e.w8(rt(ai.G_INTERRUPTED), 0)
        gsave = [bytes(e.uc.mem_read(rt(a), k__)) for a, k__ in GLOBALS_W]
    if name == "nearest_merc":
        slot = rng.choice([0, 0, rng.randrange(8), rng.randrange(8, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x183] = rng.choice([0, 14, 15, 60, 0x80])
        s[0x182] = rng.choice([0, 1, 1, 1])
        s[0x138] = rng.choice([0, 1, 1, 1])
        e.uc.mem_write(sp, bytes(s))
        edx = rng.choice([0, 3, 5, 6, 99, 100, 0xFFFFFFFF, rng.randrange(1 << 32)])
        gsave = [bytes(e.uc.mem_read(rt(a), k__)) for a, k__ in GLOBALS_W]
    if name == "step_noise":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        for o_ in (0x183, 0x184, 0x185, 0x18A, 0x18C, 0x192):
            s[o_] = rng.choice([0, 1, 10, 14, 15, 30, 49, 50, 51, 70, 100, 0x7F, 0x80, 0xC0, 0xFF, rng.randrange(256)])
        s[0x1B2] = rng.choice([0, 0x6B, 0x6C, 0x97, 0x9A, rng.randrange(256)])
        s[0x1B4] = rng.choice([0, 0x1C, 0x1F, 1, rng.randrange(256)])
        s[0x1B3] = rng.choice([0, 3, 3, 8, rng.randrange(256)])
        s[0x19C] = rng.choice([0, 2, 3, 3, 4])
        s[0x1D3] = rng.choice([0, 5, 5, 4])
        s[0x1A8] = rng.choice([0, 0, 1, 2])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(0x1163BD), rng.choice([0, 1]))
        gsave = [bytes(e.uc.mem_read(rt(a), k__)) for a, k__ in GLOBALS_W]
    if name in ("find_path", "path_ap"):  # EDX = destination; find_path: EBX = store; path_ap: ECX extra + stack
        slot = rng.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        me_ = rng.randrange(3136)
        ai.w16(s, 0x114, me_)
        s[0x177] = rng.choice([0, 0, 1])
        s[0xB9] = rng.choice([0, 1])
        s[0x1AF] = rng.choice([0, 3])
        ai.w16(s, 0xD4, rng.choice([0, 7]))
        e.uc.mem_write(sp, bytes(s))
        edx = rng.choice([me_, rng.randrange(3136), min(3135, me_ + rng.choice([1, 57, 3 * 56 + 2, 12])),
                          rng.randrange(3136)])
        ebx = rng.choice([0, 1, 1, 1, 0x10000])
        FP_N[0] += 1
        if name == "find_path" and random.Random(f"offmap{FP_N[0]}").random() < 0.04:
            edx = random.Random(f"offmap{FP_N[0]}").choice([ai.NO_TILE, 3136, 3200])   # a destination off the map: no path (own generator)
        if name == "path_ap":
            ebx = 0                          # BL = mark the path: set below (own random source)
            ecx_arg = 0 if PORT else rng.choice([0, 0, 1])     # CL = one step more (--port: drawn below)
            stack_arg = [rng.choice([0, 1]), rng.choice([0, 0, 1]), rng.choice([0, 0, 1])]
            ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([0, 1, 2, 3, 4, 0xE, 0xF, 0x10, 0x32]))
            e.uc.mem_write(sp, bytes(s))
        if FP_MAP[0] is None:                 # the dump's map, put back before each case (no drift)
            FP_MAP[0] = (bytes(e.uc.mem_read(rt(ai.G_MOVECODE), 2 * 3136)), bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)))
        e.uc.mem_write(rt(ai.G_MOVECODE), FP_MAP[0][0])
        e.uc.mem_write(rt(G_TILEMAP), FP_MAP[0][1])
        e.w8(rt(G_TILEMAP) + 8 * me_ + 5, slot)
        if FAN and 47 in FAN:                 # F47: the visible byte +0x9C of every soldier (own random source)
            r47_ = random.Random(7919 * FP_N[0] + 47)
            for k_ in (k_ for k_ in range(0x30) if k_ != slot):     # the own record is the case's record s
                e.w8(rt(G_SOLDIERS) + k_ * 0x200 + 0x9C, r47_.choice([0, 0, 1, 2, 0xFF, r47_.randrange(256)]))
        for _ in range(rng.choice([0, 5, 40])):        # some move codes and occupants changed near the walk
            t_ = rng.randrange(3136) if rng.random() < 0.5 else min(3135, max(0, me_ + rng.randrange(-200, 200)))
            e.w16(rt(ai.G_MOVECODE) + 2 * t_, rng.choice([1000, 1200, 1201, 1500, 3600, 5000, 0x6338, 0x62D4, 0x639C,
                                                         25300, 25500, 2000]))
            e.w8(rt(G_TILEMAP) + 8 * t_ + 5, rng.choice([0x40, rng.randrange(0x30), 0xC0]))
        f68_bridges(me_, F68_RNG.choice([0, 20, 80]) if FAN and 68 in FAN else 0)
        if name == "path_ap":                # the path marks: own random source, undone after the case (MARK_PRE)
            MARK_PRE[0] = [bytes(e.uc.mem_read(rt(a), k__)) for a, k__ in MARK_REGS]
            r2_ = random.Random(1000003 * i + 17)
            ebx = r2_.choice([0, 0, 1])      # BL = mark the path (select.md Path AP with marks)
            if PORT:                         # CL = one step more (own random source: the --port cases stay)
                ecx_arg = r2_.choice([0, 0, 1])
            for _ in range(r2_.choice([0, 30, 300])):   # tile byte 0 >= 0x9A with byte 1 = 0: the 0xA mark
                t_ = min(3135, max(0, me_ + r2_.randrange(-120, 120)))
                e.w8(rt(G_TILEMAP) + 8 * t_, r2_.choice([0x10, 0x99, 0x9A, 0xC0]))
                e.w8(rt(G_TILEMAP) + 8 * t_ + 1, r2_.choice([0, 0, 1]))
                e.w8(rt(G_TILEMAP) + 8 * t_ + 7, r2_.randrange(256))
            e.w8(rt(0x1163BF), r2_.choice([0, 1, 1]))
            e.w32(rt(ai.G_MARKS_SHOWN), r2_.choice([0, 0, 0, 1]))
            e.w32(rt(ai.G_MARKS_ON), r2_.choice([0, 0, 3]))
            e.w32(rt(ai.G_MARKS_N), r2_.choice([0, 0, 1, 3]))
        gsave = [bytes(e.uc.mem_read(rt(a), k__)) for a, k__ in GLOBALS_W]
    if name in ("throw_scatter", "throw_aim", "throw_follow"):
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 0x30), 7, 8])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        me_ = ai.u16(s, 0x114)
        ai.w16(s, 0x116, ai.tile_x(me_) + rng.choice([0, 0, -3, 4]))
        ai.w16(s, 0x118, ai.tile_y(me_) + rng.choice([0, 0, -3, 4]))
        s[0] = rng.choice(KIND.get(0x95, [0]) * 4 + [0, rng.randrange(136 if PORT else 256)])   # (the port: 136 types)
        s[0x1FD] = rng.choice([0, 1, 2, 4, rng.randrange(256)])
        s[0x1A1] = rng.choice(list(range(10)) + [rng.randrange(256)])
        s[0x9C] = rng.choice([0, 1, 0xFF])
        s[0x1BC] = rng.choice([0, 1, 2])
        ai.w16(s, 0xE2, rng.choice([0, 0x1234]))
        ai.w16(s, 0xDC, rng.randrange(0x10000))
        for o_ in (0x183, 0x18A):
            s[o_] = rng.choice([0, 1, 15, 50, 99, 100, 0x7F, 0x80, rng.randrange(256)])
        t_ = rng.choice([rng.randrange(3136), me_ + rng.choice([1, -1, 56, 57, 3, 2 * 56 + 5, 10 * 56]), me_])
        t_ = t_ if 0 <= t_ < 3136 else rng.randrange(3136)
        ai.w16(s, 0xDE, rng.randrange(0x10000))
        ai.w16(s, 0xE0, rng.randrange(0x10000))
        if name == "throw_scatter":
            ai.w16(s, 0xDE, ai.tile_x(t_))
            ai.w16(s, 0xE0, ai.tile_y(t_))
            edx = rng.choice([0, 1, 14, 15, 30, 60, 99, rng.randrange(256)])
            ebx = rng.choice([0, 1, 29, 30, 60, 300, 900, 0x7FFF, 0x8000, rng.randrange(0x10000)])
        elif name == "throw_aim":
            edx = t_
        else:
            ai.w16(s, 0xDC, rng.choice([t_, t_, ai.NO_TILE]))
        e.uc.mem_write(sp, bytes(s))
        gsave = [bytes(e.uc.mem_read(rt(a), k__)) for a, k__ in GLOBALS_W]
    if name == "consume_trigger":         # EDX = the trigger index (EAX in the original call)
        slot = rng.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        e.uc.mem_write(sp, bytes(s))
        sec_ = rt(ai.G_SECTORS + 0x58 + 0xB1 * e.r8(rt(ai.G_CUR_SECTOR)))
        while True:                       # tiles in clusters (neighbours chain), used / empty slots between them
            c_ = rng.randrange(57, 3136 - 57)
            ts_ = [rng.choice([c_, c_ + 1, c_ - 1, c_ + 56, c_ - 56, c_ + 57, c_ + 2, c_ + 112, ai.NO_TILE, 0xC40,
                               rng.randrange(3136), rng.randrange(0x10000)]) for _ in range(10)]
            edx = rng.randrange(10)
            try:                          # the original never ends on chains that reach 0xC09 / 0xC40 / 0xC42 / 0xC79
                tw_ = World(slot)
                tw_.g16 = lambda a_: ts_[(a_ - (ai.G_SECTORS + 0x58 + 0xB1 * e.r8(rt(ai.G_CUR_SECTOR)))) // 4]
                tw_.s16 = lambda a_, v_: ts_.__setitem__((a_ - (ai.G_SECTORS + 0x58 + 0xB1 * e.r8(rt(ai.G_CUR_SECTOR)))) // 4, v_)
                tw_.g8 = lambda a_: e.r8(rt(a_))
                keep_ = list(ts_)
                ai.consume_trigger(tw_, edx)
                ts_ = keep_
                break
            except ai.Undefined:
                continue
        for k_ in range(10):
            e.w16(sec_ + 4 * k_, ts_[k_])
            e.w16(sec_ + 4 * k_ + 2, rng.choice([0, 5, 0xFFFB, rng.randrange(0x10000)]))
        gsave = [bytes(e.uc.mem_read(rt(a), k__)) for a, k__ in GLOBALS_W]
    if name == "mercs_see":
        slot = rng.randrange(0x28, 0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x9B:0x9E] = bytes(rng.choice([0, 1, 2, 0x7F, 0x80, 0xFF]) for _ in range(3))
        e.uc.mem_write(sp, bytes(s))
        for k_ in range(0x11):           # 0x10: the slot above the mercs and guards, which must not count
            a_ = rt(G_SOLDIERS) + 0x200 * k_
            e.w8(a_ + 0x182, rng.choice([0, 1, 1, 1]))
            e.w8(a_ + 0x138, rng.choice([0, 1, 1, 1]))
            e.w8(a_ + 0x183, rng.choice([0, 14, 15, 60, 0x80]))
        e.w8(rt(ai.G_INTERRUPTED), 0)
        e.w32(rt(0x229C94), rng.choice([0x40, slot, slot, slot + 0x100, 0x28]))
        gsave = [bytes(e.uc.mem_read(rt(a), k__)) for a, k__ in GLOBALS_W]
    if name == "walk_end":
        slot = rng.choice([rng.randrange(8), 7, 8, 0x27, 0x28, rng.randrange(8, 0x28), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x18A] = rng.choice([0, 0x80, 1, 50])
        me = ai.u16(s, 0x114)
        ai.w16(s, 0x120, rng.choice([me, me, ai.NO_TILE, ai.NO_TILE + 1, 0xC40, rng.randrange(0x10000)]))
        ai.w16(s, 0xDC, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136)]))
        s[0x1B2] = rng.choice([0x10, 0x96, 0x97, 0x99, 0x9A, 0xC0])
        s[0x197] = rng.choice([0, 1])
        e.uc.mem_write(sp, bytes(s))
        e.w32(rt(0x229DD8), rng.choice([0, 1, 0xFFFFFFFF]))
        edx = rng.choice([0, 3, 4, 5, 0x31, 0x32, 0x33, 0x39, 0x104, rng.randrange(256)])
        gsave = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in GLOBALS_W]
    if name == "stand_up":
        slot = rng.choice([rng.randrange(8), 7, 8, 0x27, 0x28, rng.randrange(8, 0x28), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x1F3] = rng.choice([0, 1, 2])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([0, 1, 1, 2, 3, 4, 0x100, rng.randrange(0x44)]))
        s[0x1B9] = rng.choice([0, 0, 0, 4])
        s[0x18A] = rng.choice([9, 10, 50, 0x80])
        s[0x1A5] = rng.choice([4, 5, 20, 0x80])
        ai.w16(s, 0xD0, rng.randrange(0x10000))
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(0x92DCC), rng.choice([0, 1]))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1]))
        gsave = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in GLOBALS_W]
    if name == "walk_blocked":
        me = ai.s16(ai.u16(s, 0x114))
        target = rng.choice([rng.randrange(3136), (me + rng.choice([1, 2, 57, 112])) % 3136, me])
        ai.w16(s, 0x124, target)
        ai.w16(s, 0x122, rng.choice([target, me, rng.randrange(3136)]))
        ai.w16(s, 0x12A, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
        s[0x1AE] = rng.choice([0, 0, 0x31, 0x32, 0xFF, rng.randrange(256)])
        s[0x17A] = slot = rng.choice([slot, rng.randrange(8), 7, 8])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s[0x1A7] = rng.choice([0, 1])
        e.uc.mem_write(sp, bytes(s))
        fuzz_tile(target)                # (also writes +0x9C of the tile's occupant: maybe this soldier)
        s[:] = e.uc.mem_read(sp, 0x200)
        if rng.random() < 0.5:
            e.w8(rt(G_TILEMAP) + 8 * target + 1, rng.choice([0, 3, 0x3B, 0x3D, 0x43, 0x50]))
        edx = rng.choice([target, target, (me + rng.choice([1, 56, -1])) % 3136, rng.randrange(3136)])
        k = rng.choice([rng.randrange(16), rng.randrange(0x28, 0x30)])
        ebx = rng.choice([0x40, 0x40, k, k, rng.randrange(0x30), 0x41, 0xC0 | k])
        if k != slot:                    # the occupant at or short of its step target
            oa = rt(G_SOLDIERS) + 0x200 * k
            o = bytearray(e.uc.mem_read(oa, 0x200))
            o[0x1B9] = rng.choice([0, 0, 5])
            o[0x1F3] = 0
            ai.w16(o, 0x1E8, rng.choice([3, 4, 0x15]))
            for a_ in (0x114, 0x116, 0x118):
                ai.w16(o, a_ + 0x10, rng.choice([ai.u16(o, a_), ai.u16(o, a_), ai.u16(o, a_) + 1]))
            e.uc.mem_write(oa, bytes(o))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 0, 1]))
        gsave = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in GLOBALS_W]
    if name in ("set_anim", "anim_delay", "msg_not_enough_ap"):
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30), rng.choice([0x27, 0x28])])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x1F3] = rng.choice([0, 1, 2])
        cur = rng.choice(list(range(0x44)) + [0, 1, 4, 5, 0xF, 0x17, 0x1A, 0x1E, 0x22, 0x33, 0x36, 0x39])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], cur)
        frames = rt(ai.ANIM_FRAMES) + 0x50 * rng.randrange(0x44) + rng.choice([0, 0, 2 * rng.randrange(40)])
        if rng.random() < 0.5:                    # the "frame count" edge: the word at the script position
            e.w16(frames, rng.choice([0x15E, 0x15F, 0, 0x200]))
        s[0x90:0x94] = frames.to_bytes(4, "little")
        for o in (0x183, 0x18A, 0x18B):
            s[o] = rng.choice([0, 15, 50, 99, 100, 120, rng.randrange(256)])
        s[0x1B3] = rng.choice(list(range(11)) + [rng.randrange(256)])
        s[0x1B8] = rng.choice([0, 1])
        s[0x1A5] = rng.choice([0, 2, 5, 10, rng.randrange(256)])
        s[0x1E2] = rng.choice([0, 0, 3, 0xFE])
        s[0xA4:0xA8] = rng.choice([0, 30, 0x12345]).to_bytes(4, "little")
        s[0x9C] = rng.choice([0, 1, 0xFF])
        s[0x177] = rng.choice([0, 1])
        s[0x195] = rng.choice([0, 1])
        if rng.random() < 0.15:                   # the facing flip of code 0x43 from animation 1
            ai.w16(s, 0x1E8 + 2 * s[0x1F3], 1)
            cur = 1
            s[0x1A1] = rng.choice([4, 4, 5, 3, 8])
        e.uc.mem_write(sp, bytes(s))
        e.uc.mem_write(rt(ai.ANIM_CAN_ACT) - 8, ANIMTBL)
        e.w8(rt(ai.ANIM_DELAY) + 8 * cur, rng.choice([0, 0, 1, rng.randrange(256), e.r8(rt(ai.ANIM_DELAY) + 8 * cur)]))
        e.w8(rt(ai.ANIM_CAN_ACT) + 8 * cur, rng.choice([0, 1, e.r8(rt(ai.ANIM_CAN_ACT) + 8 * cur)]))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, slot, 0x40]))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 1]))
        e.w32(rt(0x229DD8), rng.choice([0, 1]))
        edx = rng.choice(list(range(0x44)) + [4, 4, 0x15, 0x15, 0x43, 0x43, 0x24, cur, cur, 9, 0x3C, 0x3D])
        ebx = rng.choice([0, 1, 0x63, 0x100])
        ecx_arg = rng.choice([0, 1, 0x100])
        if name == "msg_not_enough_ap":
            edx = rng.randrange(256)
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "anim_step":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x1F3] = rng.choice([0, 1, 2, 3, 4])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice(list(range(0x40)) + [0x31, 0x32, 4, 4, 5, 0xFFF0, rng.randrange(0x100)]))   # far-negative codes would index across object boundaries
        s[0x1A2] = rng.choice([s[0x1A1], rng.randrange(1, 9)])
        s[0x1B9] = rng.choice([0, 0, 4, 5, 6, rng.randrange(256)])
        me = ai.u16(s, 0x114)
        for o, v in ((0x124, me), (0x126, ai.tile_x(ai.s16(me))), (0x128, ai.tile_y(ai.s16(me)))):
            ai.w16(s, o, rng.choice([v, v, v, rng.randrange(0x10000)]))
        ai.w16(s, 0x116, rng.choice([ai.tile_x(ai.s16(me)), ai.tile_x(ai.s16(me)) + 3]))
        ai.w16(s, 0x118, rng.choice([ai.tile_y(ai.s16(me)), ai.tile_y(ai.s16(me)) - 2]))
        ai.w16(s, 0x12A, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136), (ai.s16(me) + rng.choice([1, 2, 57, 200])) % 3136]))
        for o in (0x1A7, 0x1FA, 0x177, 0x1A8, 0x195, 0x1CC, 0x1E1, 0x1E2, 0x1E3):
            s[o] = rng.choice([0, 0, 1, rng.randrange(256)])
        s[0x1B2] = rng.choice([0x10, 0x10, 0x10, 0x6B, 0x6C, 0x96, 0x97, 0x99, 0x9A, rng.randrange(256)])
        s[0xBA] = rng.choice([s[0xBA], 2])
        s[0x183] = rng.choice([0, 14, 15, 60, 60, 60])
        s[0x1A3] = rng.choice([0, 1, 0xFF])
        if rng.random() < 0.3:                  # hit by a merc: the fire-back path
            s[0x195], s[0x1DA], s[0x1E1] = 1, rng.randrange(8), 0
        s[0x18A] = rng.choice([0, 50])
        s[0x1DA] = rng.choice([0, 3, 7, 8, 0x29])
        s[0x00] = rng.choice([0, 1, 5, 0x10, 0x10, 0x11, 0x40])
        e.uc.mem_write(sp, bytes(s))
        e.uc.mem_write(rt(ai.G_PATHS) + 0x50 * slot, bytes(rng.randrange(8) for _ in range(0x50)))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, 0, 1, 0x40]))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 0, 1]))
        e.uc.mem_write(rt(ai.ANIM_CAN_ACT) - 8, ANIMTBL)
        an = ai.u16(s, 0x1E8 + 2 * s[0x1F3]) & 0xFF
        e.w8(rt(ai.ANIM_CAN_ACT) + 8 * an, rng.choice([0, 1, e.r8(rt(ai.ANIM_CAN_ACT) + 8 * an)]))
        e.w8(rt(ai.ANIM_TURNS) + 8 * an, rng.choice([0, 1, e.r8(rt(ai.ANIM_TURNS) + 8 * an)]))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "anim_end":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x1F3] = rng.choice([0, 1, 1, 1, 2, 3])
        top = 0x100                       # (far codes index across object boundaries; anim_step does the same)
        for j in range(s[0x1F3] + 1):
            ai.w16(s, 0x1E8 + 2 * j, rng.choice(list(range(0x44)) + [4, 4, 0xD, 0xD, 0x15, 0x34, 0x37, 0x3A, 0xFF if PORT else 0x104,
                                                                     rng.randrange(top)]) if j < s[0x1F3] else
                   rng.choice([0xD, 0xD, 4, 9, 10, 0x23, rng.randrange(0x44)]))
        x, y = ai.u16(s, 0x116), ai.u16(s, 0x118)
        ai.w16(s, 0x126, rng.choice([x, x, x + 1]))
        ai.w16(s, 0x128, rng.choice([y, y, y - 1]))
        for o in (0x1FE, 0x1AD, 0x1E2, 0x1A5):
            s[o] = rng.choice([0, 0, 1, 3, rng.randrange(256)])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, slot, 0, 0x40]))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 1]))
        e.uc.mem_write(rt(ai.ANIM_CAN_ACT) - 8, ANIMTBL)
        for j in range(s[0x1F3] + 1):
            an = ai.u16(s, 0x1E8 + 2 * j)
            if an < 0x100:
                for col in (0, 3):
                    e.w8(rt(ai.ANIM_AP) + 8 * an + col, rng.choice([0, 1, 3, 0x63, e.r8(rt(ai.ANIM_AP) + 8 * an + col)]))
                e.w8(rt(ai.ANIM_FREE) + an, rng.choice([0, 1, e.r8(rt(ai.ANIM_FREE) + an)]))
                e.w16(rt(ai.ANIM_SOUND) + 2 * an, rng.choice([0, 1, 0x19, 0xFFFF, e.r16(rt(ai.ANIM_SOUND) + 2 * an)]))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "walk_to":
        edx = rng.choice([rng.randrange(3136), ai.NO_TILE, rng.randrange(0x10000)])
        s[0x1FE] = rng.choice([0, 1])
        s[0x1D5] = rng.choice([0, 0, 1, 0xFF])
        s[0x1F3] = rng.choice([0, 1, 2, 3, 4])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice(list(range(0x48)) + [rng.randrange(0x100 if PORT else 0x10000)]))   # --port: words past the tables read 0 there
        for o in (0x1D8, 0x1C2, 0x1BE, 0xD0, 0xDC):
            s[o] = rng.randrange(256)
        e.uc.mem_write(sp, bytes(s))
    if name == "turn_to":
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        fuzz_soldier(s)
        s[0x17A] = slot
        s[0x1A1] = rng.choice(list(range(1, 9)) + [0, rng.randrange(256)])
        s[0x1F3] = rng.choice([0, 1, 2, 3, 4])
        s[0x1E8 + 2 * s[0x1F3]] = rng.choice(list(range(0x48)) + [0x31, 0x32, 0x3C, 0x3D] * 3 + [rng.randrange(256)])
        e.uc.mem_write(rt(ai.ANIM_CAN_ACT) - 8, ANIMTBL)      # pristine, then fuzz this animation's two flags
        an = s[0x1E8 + 2 * s[0x1F3]]
        e.w8(rt(ai.ANIM_CAN_ACT) + 8 * an, rng.choice([0, 1, e.r8(rt(ai.ANIM_CAN_ACT) + 8 * an)]))
        e.w8(rt(ai.ANIM_TURNS) + 8 * an, rng.choice([0, 1, e.r8(rt(ai.ANIM_TURNS) + 8 * an)]))
        s[0x16B] = rng.choice([0, 1])
        s[0xEF] = rng.choice([0, 0, 1, 3, 0xFF])
        s[0x1A5] = rng.choice([0, 1, 2, 3, 0xFF, rng.randrange(256)])
        e.uc.mem_write(sp, bytes(s))
        e.w8(rt(ai.G_AP_WARNED) + (slot & 7), rng.choice([0, 1]))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 1]))
        edx = rng.choice(list(range(1, 9)) * 2 + [s[0x1A1], 0, 0x105, rng.randrange(256)])
        ebx = rng.choice([30, 0, 0x80, 0xFF, 0x11E])
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "attack_ready":
        edx = rng.choice([tile, ai.NO_TILE, rng.randrange(3136), tile | 0x10000, rng.randrange(1 << 32)])
        s[0x00] = rng.choice(list(range(0x12)) + KIND.get(0x95, []) + [rng.randrange(136)])
        s[0x01] = rng.choice([0, 1, 1, 3])
        s[0x02] = rng.choice([9, 10, 50, 0x80, 0xFF, 0])
        s[0x1B2] = rng.choice([0x10, 0x6B, 0x6C, 0x80])
        s[0x1B4] = rng.choice([0, 0x1C, 0x1F, 0x20])
        s[0x1F3] = rng.choice([0, 1, 2, 3, 4])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice(list(range(0x60)) + [rng.randrange(0x10000)]))
        if PORT:          # (tiles are 16-bit values and animation codes below 256 in the game; click.md)
            edx = rng.choice([tile, ai.NO_TILE, rng.randrange(3136)])
            ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.randrange(0x60))
        e.uc.mem_write(sp, bytes(s))
    if name == "idle":
        s[0x1FE] = rng.choice([0, 1, 0xFF])
        for o in (0x1D8, 0x1C2, 0x1BE, 0xD0, 0xD1, 0xDC, 0xDD):
            s[o] = rng.randrange(256)
        s[0x1F3] = rng.choice([0, 1, 2, 3, 4])
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice(list(range(0x60)) + [rng.randrange(0x100 if PORT else 0x10000)]))
        e.uc.mem_write(sp, bytes(s))
    if name == "share_sightings":
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30), rng.randrange(16)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        ai.w16(s, 0x114, tile)
        ai.w16(s, 0x116, ai.tile_x(tile) + rng.choice([0, 0, 3, -2, rng.randrange(-40000, 40000)]))
        ai.w16(s, 0x118, ai.tile_y(tile) + rng.choice([0, 0, 3, -2]))
        s[0x1C0] = rng.choice([0, 12, 13, 30, rng.randrange(256)])
        for j in range(0x28):
            s[0x13A + j] = rng.choice([1, 1, 1, 0, 0, 2, 3, 0xFF, 0xFE, 4])
        e.uc.mem_write(sp, bytes(s))
        for k in range(0x30):
            a = rt(G_SOLDIERS) + 0x200 * k
            if k != slot:
                e.w8(a + 0x9C, rng.choice([0, 1, 0xFF]))
                e.w8(a + 0x183, rng.choice([e.r8(a + 0x183), 14, 15, 60]))
                e.w8(a + 0x17A, rng.choice([k] * 9 + [rng.randrange(0x30)]))
                if PORT:                    # a foreign slot byte indexes past the last-seen tables (the port reads 0)
                    e.w8(a + 0x17A, k)
                if rng.random() < 0.3:      # a guard's tile near the soldier: Distance ties and order
                    e.w16(a + 0x114, (tile + rng.choice([1, -1, 56, 57, 2])) % 3136)
        e.uc.mem_write(rt(ai.G_KNOWN), bytes(rng.choice([0, 0, 0, 1, 2, 3, 0xFF, 0xFE]) for _ in range(0x30)))
        e.w8(rt(ai.G_SHOW_UI), rng.choice([0, 1]))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, slot, rng.randrange(16)]))
        e.w8(rt(0x93458), rng.choice([0, 0, 1]))
        edx = rng.choice([0, 1, 0x100])
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "combat_action" and rng.random() < 0.3:
        s[0x18A], s[0x1C4] = rng.choice([25, 50, 100]), 0
        if rng.random() < 0.5:           # on the scripted mission's spot, a few AP (Combat step 3)
            s[0x1A5] = rng.choice([1, 2, 2, 3])
            s[0x00], s[0x0C] = rng.choice([0x40, 1, 2]), rng.choice([0x40, 3])
            e.w8(rt(ai.G_MISSION), rng.choice([2, 3, 5, 1]))
            e.w8(rt(ai.G_MISSION_ORDER), slot)
            e.w16(rt(ai.G_MISSION_ORDER) + 2, rng.randrange(3136))
            e.w16(rt(ai.G_MISSION_SPOT), ai.u16(s, 0x114))
            e.w8(rt(ai.G_MISSION_DONE) + e.r8(rt(ai.G_CUR_SECTOR)), rng.choice([0, 1, 1]))
            base["reach_ap"] = [rng.choice([1, 1, 0x100, 0])]
        else:                            # can't walk, no mission, the opponent known nearby (Combat step 8)
            s[0x1A5] = rng.choice([1, 2, 3, 4])
            e.w8(rt(ai.G_MISSION), 0)
            base["move_ap"] = [0x7F] * 4
            base["opp_strength"] = [rng.choice([1, 2, 3, 4])] * 4
            base["target_tile"] = [rng.randrange(3136)] * 4
            base["direction"] = [rng.choice([1, 2, 5, 8])] * 4
            base["tile_dist"] = [rng.choice([3, 13, 14, 15])] * 8
            base["Random"] = [rng.randrange(100) for _ in range(8)]
        e.uc.mem_write(sp, bytes(s))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
        script = {k: list(v) for k, v in base.items()}
        s, base = probe_edge(addr, sp, s, base, edx, ebx, only="Random")
        script = {k: list(v) for k, v in base.items()}
        log.clear()
    if name == "think" and rng.random() < 0.3:     # an action still runs (tick step 10): real time, near the timeout
        acts = [c for c in range(4, 255) if ANIMTBL[8 * (c + 1)]]
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice(acts))
        s[0x182], s[0x138], s[0x197], s[0x1C1], s[0x1B8], s[0x1D6] = 1, 1, 0, 0, 1, 1
        s[0x183] = rng.choice([15, 50, 100])
        s[0xBA] = rng.choice([0, 1, 3])
        s[0x1D1] = rng.choice([14, 14, 15, 13, 11, 12, 16, 1, 2])
        ai.w16(s, 0x120, rng.choice([ai.NO_TILE, ai.NO_TILE, ai.u16(s, 0x124)]))
        ai.w16(s, 0xF0, rng.choice([0, 347, 348, 349, 350, 351, 0xFFFF]))
        tt = rng.randrange(3136)
        ai.w16(s, 0x132, tt)
        e.w8(rt(G_TILEMAP) + 8 * tt + 5, rng.choice([0x40, 0x7F, 0x30, 0x2F, rng.randrange(0x28, 0x30), 0x80 | 3]))
        e.w32(rt(ai.G_TURN_BASED), rng.choice([0, 0, 0, 1]))
        e.w32(rt(ai.G_BUSY), 0)
        e.w8(rt(ai.G_ACTIVE), 0x40)
        e.uc.mem_write(sp, bytes(s))
        gsave = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if PORT and name == "sight_check":   # the game's facings 1..8 and directions 0..8: the port's range table holds
        s[0x1A1] = 1 + (s[0x1A1] - 1) % 8  # only those (ai.md Sight range table); no draws from rng
        base["direction"] = [d if d & 0xFF <= 8 else 1 + (d & 7) for d in base["direction"]]
        script = {k: list(v) for k, v in base.items()}
        e.uc.mem_write(sp, bytes(s))
    if name in PROBE and rng.random() < 0.35:
        s, base = probe_edge(addr, sp, s, base, edx, ebx)
        script = {k: list(v) for k, v in base.items()}
        log.clear()
    if name in OTHERS:
        osave = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    if name in PATHCMP:
        psave_ = bytes(e.uc.mem_read(rt(ai.G_PATHS), 0x50 * 0x30))
    if name == "think":                  # turn-based flag 0x92DAC: compared for think only (F36 clears it after FireNow)
        tbsave_ = bytes(e.uc.mem_read(rt(ai.G_TURN_BASED), 4))
    if name in MARKCMP:
        ksave_ = [bytes(e.uc.mem_read(rt(a), k)) for a, k in MARK_REGS]
    if name in ("kill", "land_item"):     # #12: a tile byte 6 of 200..254 (Put item into a full table, an earlier case)
        for g_ in range(200, 255):        # names a ground entry past the table: the original reads byte 6 of it in
            if e.r8(rt(ai.G_GROUND_ITEMS + 12 * g_ + 6)) == 0xFF:      # g_textBuf / g_profiles, the port reads 0 (SPEC GAP)
                mp_ = bytearray(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
                for t_ in range(3136):
                    if mp_[8 * t_ + 6] == g_:
                        e.w8(rt(G_TILEMAP + 8 * t_ + 6), 0xFF)
    if name in MAPCMP:
        msave = bytes(e.uc.mem_read(rt(map_lo(name)), MAP_HI - map_lo(name)))
    mc_cmp = name in MCCMP and FAN and 51 in FAN   # #70 F51: the move codes after a blue flag is cleared
    if mc_cmp:
        mcsave = bytes(e.uc.mem_read(rt(ai.G_MOVECODE), 2 * 3136))
    if name == "soldier_power":
        edx = rng.choice([ai.NO_TILE, ai.u16(s, 0xD0), rng.randrange(3136), rng.randrange(1 << 32)])
    if PORT and name in ("active_frame", "soldier_frame"):     # the port's item_types has the game's 136 entries
        for k_ in range(12):
            if s[12 * k_] >= 136:
                s[12 * k_] = rng.randrange(136)
        if not 0 <= ai.s16(ai.u16(s, 0x134)) < 3136:   # off the map the original reads past the tile map (port: 0)
            ai.w16(s, 0x134, rng.randrange(3136))
        e.uc.mem_write(sp, bytes(s))
    unstub = e.stubs.pop(rt(addr), None)
    case_stubs = [c_ for c_, users_ in CASE_STUBS.items() if name in users_]
    for c_ in case_stubs:
        e.stub(rt(CALLEES[c_][0]), make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))
    case_real = {c_: e.stubs.pop(rt(CALLEES[c_][0])) for c_ in CASE_REAL.get(name, ())}
    try:
        got = e.call(rt(addr), slot if name == "fire" else edx if name == "consume_trigger" else sp, edx, ebx, ecx_arg,
                     *([] if stack_arg is None else stack_arg if isinstance(stack_arg, list) else [stack_arg]),
                     max_steps=200_000_000) & MASK.get(name, 0xFF)
        if name in RECORDS:
            got = bytes(e.uc.mem_read(OUTREC, 10))
    except emu.EmuStop as x:              # the original divides by zero on some inputs: the port must too
        if "step limit" in str(x):        # the original never returns (fuzzed input): skipped, counted
            if os.environ.get("AI_DIFF_HANGS"): print("hang", name, i, x)
            hangs[name] = hangs.get(name, 0) + 1
            for (a_, k_), b_ in zip(GLOBALS_W, gsave):
                e.uc.mem_write(rt(a_), b_)
            if name in MARKCMP:
                for (a_, k_), b_ in zip(MARK_REGS, MARK_PRE[0]):
                    e.uc.mem_write(rt(a_), b_)
            e.uc.mem_write(sp, bytes(s))
            if unstub:
                e.stubs[rt(addr)] = unstub
            for c_ in case_stubs:
                e.stubs.pop(rt(CALLEES[c_][0]), None)
            for c_, st_ in case_real.items():
                e.stubs[rt(CALLEES[c_][0])] = st_
            continue
        at = int(str(x).split(" at 0x")[1].split()[0], 16)
        ins = next(e.md.disasm(bytes(e.uc.mem_read(at, 8)), at))
        if ins.mnemonic not in ("div", "idiv"):
            raise
        got, log[:] = "divide by zero", []
    if unstub:
        e.stubs[rt(addr)] = unstub
    for c_ in case_stubs:
        e.stubs.pop(rt(CALLEES[c_][0]), None)
    for c_, st_ in case_real.items():
        e.stubs[rt(CALLEES[c_][0])] = st_
    if name in VOID and not isinstance(got, str):       # keep "divide by zero"
        got = 0
    got_s, got_log = bytes(e.uc.mem_read(sp, 0x200)), list(log)
    if name in MAPCMP:                    # read every result first: the map region holds the tail of soldier 0x2F
        got_m = bytes(e.uc.mem_read(rt(map_lo(name)), MAP_HI - map_lo(name)))
    if name in PAIR:
        got_o = bytes(e.uc.mem_read(edx, 0x200))
    if name in OTHERS:
        got_o = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    if mc_cmp:
        got_mc = bytes(e.uc.mem_read(rt(ai.G_MOVECODE), 2 * 3136))
        e.uc.mem_write(rt(ai.G_MOVECODE), mcsave)
    if name in MAPCMP:                    # then restore, the records last
        e.uc.mem_write(rt(map_lo(name)), msave)
    if name in PAIR:
        e.uc.mem_write(edx, osave)
    if name in OTHERS:
        e.uc.mem_write(rt(G_SOLDIERS), osave)
    got_g = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "walter":                 # W, C, walter_sector: compared, then restored
        got_g += [bytes(e.uc.mem_read(a_, k_)) for a_, k_ in wl_cmp]
        for (a_, k_), b_ in zip(wl_cmp, wl_pre):
            e.uc.mem_write(a_, b_)
    if name == "mine_flag":              # the tile's object byte and move code: compared, then restored
        got_g += [bytes(e.uc.mem_read(a_, k_)) for a_, k_ in mf_keep]
        for (a_, k_), b_ in zip(mf_keep, mf_pre):
            e.uc.mem_write(a_, b_)
    if name in PATHCMP:                  # the stored paths: compared for these deciders only
        got_g.append(bytes(e.uc.mem_read(rt(ai.G_PATHS), 0x50 * 0x30)))
        e.uc.mem_write(rt(ai.G_PATHS), psave_)
    if name == "think":
        got_g.append(bytes(e.uc.mem_read(rt(ai.G_TURN_BASED), 4)))
        e.uc.mem_write(rt(ai.G_TURN_BASED), tbsave_)
    if name in MARKCMP:
        got_g += marks_read()
        for (a, k), b in zip(MARK_REGS, ksave_):
            e.uc.mem_write(rt(a), b)
    for (a, k), b in zip(GLOBALS_W, gsave):
        e.uc.mem_write(rt(a), b)
    e.uc.mem_write(sp, bytes(s))

    script = {k: list(v) for k, v in base.items()}
    log.clear()
    ps = bytearray(s)
    try:
        want = run_port(name, port, ps, edx, ebx)
    except ZeroDivisionError:
        want, log[:] = "divide by zero", []
    want = want if name in RECORDS or isinstance(want, str) else (want or 0) & MASK.get(name, 0xFF)
    want_log = list(log)
    want_g = [bytes(e.uc.mem_read(rt(a), k)) for a, k in GLOBALS_W]
    if name == "mine_flag":
        want_g += [bytes(e.uc.mem_read(a_, k_)) for a_, k_ in mf_keep]
        for (a_, k_), b_ in zip(mf_keep, mf_pre):
            e.uc.mem_write(a_, b_)
    if name == "walter":
        want_g += [bytes(e.uc.mem_read(a_, k_)) for a_, k_ in wl_cmp]
        for a_, v_ in wl_keep:             # (after both runs: the setup leaves no trace)
            e.uc.mem_write(a_, v_)
    if name == "lock_kit":                 # (after both runs: the setup leaves no trace)
        for a_, v_ in lk_keep:
            e.uc.mem_write(a_, v_)
    if name in PATHCMP:
        want_g.append(bytes(e.uc.mem_read(rt(ai.G_PATHS), 0x50 * 0x30)))
        e.uc.mem_write(rt(ai.G_PATHS), psave_)
    if name == "think":
        want_g.append(bytes(e.uc.mem_read(rt(ai.G_TURN_BASED), 4)))
        e.uc.mem_write(rt(ai.G_TURN_BASED), tbsave_)
    if name in MARKCMP:
        want_g += marks_read()
        for (a, k), b in zip(MARK_REGS, MARK_PRE[0]):     # (the state before the marks fuzz: later cases unchanged)
            e.uc.mem_write(rt(a), b)
    if name in PAIR:
        want_o = bytes(e.uc.mem_read(edx, 0x200))
        got_g.append(got_o)
        want_g.append(want_o)
    if mc_cmp:
        got_g.append(got_mc)
        want_g.append(bytes(e.uc.mem_read(rt(ai.G_MOVECODE), 2 * 3136)))
        e.uc.mem_write(rt(ai.G_MOVECODE), mcsave)
    if name in MAPCMP:
        # kill, land_item: Put item with no free tile (dest NONE) writes byte 6 past the map in the original (sector_start.md
        # batch 3 step 7: nothing to do in the port): the bytes past the map are not compared
        cut_m = G_TILEMAP + 8 * 3136 - G_SOLDIERS - 0x6000 if name in ("kill", "land_item") else None
        got_g.append(got_m[G_SOLDIERS + 0x6000 - map_lo(name):][:cut_m])
        want_g.append(bytes(e.uc.mem_read(rt(G_SOLDIERS + 0x6000), MAP_HI - G_SOLDIERS - 0x6000))[:cut_m])
    if name in OTHERS:                   # the soldier's own record is compared on its own
        want_o = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
        cut = lambda b: b[:0x200 * slot] + b[0x200 * slot + 0x200:]
        got_g.append(cut(got_o))
        want_g.append(cut(want_o))
    if name == "land_item" and FAN and 52 in FAN and got == sp & 0xFFFF and want == edx:
        got = want                       # F52: an item PutItem does not place (sinks in water) leaves 1.15m's EAX = the
                                         # caller's EDI (the thrower pointer); the port keeps the asked tile (1.13b)
    counts[name] = counts.get(name, 0) + 1
    key = want[0] if name in RECORDS and not isinstance(want, str) else want
    results.setdefault(name, {}).setdefault(key, 0)
    results[name][key] += 1
    if (got, got_s, got_log, got_g) != (want, bytes(ps), want_log, want_g):
        fails += 1
        if os.environ.get("AI_DIFF_DEBUG"):
            print("  start:", {hex(o): hex(s[o]) for o in map(int, os.environ["AI_DIFF_DEBUG"].split(","))})
        if fails <= 10:
            diff = [f"+{o:#x}: {got_s[o]:#x}/{ps[o]:#x}" for o in range(0x200) if got_s[o] != ps[o]]
            gd = [f"{a:#x}+{o:#x}: {x[o]:#x}/{y[o]:#x}" for (a, _), x, y in zip(GLOBALS_W + [(edx, 0)], got_g, want_g)
                  for o in range(len(x)) if x[o] != y[o]]
            print(f"MISMATCH {name} case {i}: ret original {got} port {want}; soldier (orig/port) {diff} globals {gd[:8]}\n"
                  f"  calls original {got_log}\n  calls port     {want_log}")
if direct("tile_standable"):   # pure function of the map: called directly
    for i in range(3000):
        tt = rng.randrange(3136)
        fuzz_tile(tt)
        got, want = ai._i32(e.call(rt(0x23538), tt)), ai.tile_standable(World(0), tt)
        counts["tile_standable"] = counts.get("tile_standable", 0) + 1
        results.setdefault("tile_standable", {}).setdefault(want, 0)
        results["tile_standable"][want] += 1
        if got != want:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH tile_standable({tt}): original {got} port {want}")
if direct("sector_value"):   # strategic globals only: called directly with the stub lifted
    stub = e.stubs.pop(rt(0x3E03C))
    tbl = bytes(e.uc.mem_read(rt(ai.G_SECTOR_TBL9), 9 * 256))
    flags = [0x229FA4, 0x22A046, 0x22A049, 0x22A04B, 0x22A04D, 0x22A055, 0x22A056, 0x22A057, ai.G_DIFFICULTY]
    fsave = {a: e.r32(rt(a)) for a in flags}
    psave = bytes(e.uc.mem_read(rt(ai.G_PLANTS), 48))
    for i in range(6000):
        sector = rng.choice(list(range(61)) + [0x3B, 0x1B, 0x35, 0x11, 0x0B, 0, 5, 0x15] * 3 + [rng.randrange(256)])
        if PORT and sector >= 80:          # the port's sector_state has 80 rows (the AI passes cur_sector < 60)
            sector = rng.randrange(80)
        a = rt(ai.G_SECTOR_TBL9) + 9 * sector
        big = rng.random() < 0.1
        e.uc.mem_write(a, bytes([rng.choice([0, 1, rng.randrange(32), rng.randrange(256)]),
                                 rng.randrange(20), rng.choice([0, 0, 1, 4]), rng.randrange(256 if big else 13),
                                 rng.randrange(10), rng.randrange(256), rng.randrange(256 if big else 7),
                                 rng.randrange(256 if big else 6), rng.choice([0] * 12 + [1, rng.randrange(256)])]))
        day = rng.choice([1, 2, 3, 4, 5, 6, 7, 8, 16, 25, 30, 50, rng.randrange(0x10000), rng.randrange(1 << 32), 0x7FFFFFC0])
        e.w32(rt(0x229FA4), day)
        for f in flags[1:-1]:
            e.w8(rt(f), rng.choice([0, 0, 0, 1, 2, sector, rng.randrange(256)]))
        e.w8(rt(ai.G_DIFFICULTY), rng.choice([0, 1, 2]))
        if rng.random() < 0.15:          # the story condition (0x22A049 = 0x22A046 = 2, 0x22A04B < 1) on its edges
            sector = rng.choice([0x1B, 0x11, 0x1B, 0x11, 0x1C, 0x10, 0x35])
            a = rt(ai.G_SECTOR_TBL9) + 9 * sector
            e.w8(rt(0x22A049), rng.choice([2, 2, 2, 1, 3]))
            e.w8(rt(0x22A046), rng.choice([2, 2, 2, 1, 3]))
            e.w8(rt(0x22A04B), rng.choice([0, 0, 0x80, 0xFF, 1]))
            e.w8(a, rng.choice([0, 1, 1, 3]))
            e.w8(a + 8, rng.choice([0, 0, 0, 1]))
        for k in range(4):
            e.w32(rt(ai.G_PLANTS + 4 + 12 * k), rng.choice([sector, rng.randrange(61), rng.randrange(61),
                                                           rng.randrange(1 << 32)]))
        flag = rng.choice([0, 1, 0x100, rng.randrange(256)])
        base = scripted("sector_value")
        script = {k: list(v) for k, v in base.items()}
        log.clear()
        got, got_log = ai._i32(e.call(rt(0x3E03C), sector, flag)), list(log)
        script = {k: list(v) for k, v in base.items()}
        log.clear()
        want = port_sector_value(sector, flag) if PORT else ai.sector_value(World(0), sector, flag)
        counts["sector_value"] = counts.get("sector_value", 0) + 1
        results.setdefault("sector_value", {}).setdefault(want, 0)
        results["sector_value"][want] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH sector_value({sector}, {flag}) day {day}: original {got} port {want}\n"
                      f"  calls original {got_log}\n  calls port     {log}")
    e.uc.mem_write(rt(ai.G_SECTOR_TBL9), tbl)
    e.uc.mem_write(rt(ai.G_PLANTS), psave)
    for a, v in fsave.items():
        e.w32(rt(a), v)
    e.stubs[rt(0x3E03C)] = stub
if direct("sight_line"):   # stack-argument line test: called directly over a fuzzed map
    stub = e.stubs.pop(rt(0x58760))
    tmap = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
    soldiers = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    wtbl = bytes(e.uc.mem_read(rt(ai.G_WINDOW_TBL), 8 * 256))
    block = e.r32(rt(ai.G_SIGHT_BLOCK))
    OBJ = [0, 0, 0x10, 0x26, 0x27, 0x30, 0x36, 0x37, 0x40, 0x50, 0x60, 0x82, 0x83, 0xE1, 0xE2, 0xE3, 0xE4]
    for i in range(4000):
        x, y = rng.randrange(-20, 900), rng.randrange(-20, 790)
        far = rng.choice([4, 16, 40, 120, 300, 900])
        tx, ty = x + rng.randrange(-far, far + 1), y + rng.randrange(-far, far + 1)
        if rng.random() < 0.05:
            tx, ty = x, y + rng.choice([0, 1])
        rg = rng.choice([0, 1, 3, 8, 12, 14, 14, 30, 3000, rng.randrange(1 << 32)])
        if rng.random() < 0.3:          # the line ends near the range limit (range x 15 steps, maybe cut by 30 %)
            r = rng.choice([rg, rg - rg * 30 // 100]) * 15 % 0x8000 + rng.randrange(-3, 4)
            tx = x + rng.choice([r, -r])
            ty = y + rng.choice([0, rng.randrange(-abs(r), abs(r) + 1)])
        if rng.random() < 0.15:         # exact diagonals: |dx| == |dy|
            ty = y + rng.choice([1, -1]) * abs(tx - x)
        for k in range(rng.choice([0, 0, 8, 40])):     # window opacities 0x62 / 0x63 (step limits) and others
            e.w8(rt(ai.G_WINDOW_TBL) + rng.randrange(0x37 * 8, 0x83 * 8), rng.choice([0, 1, 3, 0x61, 0x62, 0x63, 0x70]))
        e.w32(rt(ai.G_SIGHT_BLOCK), rng.choice([block, block, block, rng.randrange(0, 60)]))
        e.w8(rt(0x95680), 1)
        from combat import view_tile
        for k in range(rng.choice([0, 30, 120, 400])):     # objects along the line
            f = rng.random()
            t = view_tile(x + int((tx - x) * f) + rng.randrange(-20, 21), y + int((ty - y) * f) + rng.randrange(-20, 21))
            if 0 <= t < 3136:
                e.w8(rt(G_TILEMAP) + 8 * t + 1, rng.choice(OBJ + [rng.randrange(256)]))
        tile = rng.choice([view_tile(tx, ty), rng.randrange(3136)])
        flag = rng.choice([0, 0, 1, 2, 0xFF, 0x100])
        slot = rng.choice([rng.randrange(0x30), rng.randrange(0x30), 0x40, 0xFF, 0x13F])
        if (slot & 0xFF) < 0x40:
            a = rt(G_SOLDIERS) + 0x200 * (slot & 0xFF)
            e.w8(a + 0x1B1, rng.choice([0, 1, 5]))
            e.w8(a + 0x1B3, rng.choice(list(range(10)) + [3, 3, 3, rng.randrange(256)]))
        me = rng.choice(list(range(0x28, 0x30)) + list(range(16)))
        base = {"direction": [rng.choice([1, 2, 3, 4, 5, 6, 7, 8, 0, rng.randrange(256)])],
                "fire_line": [rng.choice([0, 24, 25, 100, rng.randrange(1 << 32)])]}
        script = {k: list(v) for k, v in base.items()}
        log.clear()
        sv = bytes(e.uc.mem_read(rt(0x95680), 1))
        got = ai._i32(e.call(rt(0x58760), rt(G_SOLDIERS) + 0x200 * me, x & 0xFFFFFFFF, y & 0xFFFFFFFF,
                             tile, tx & 0xFFFFFFFF, ty & 0xFFFFFFFF, flag, rg, slot))
        got_log, got_g = list(log), bytes(e.uc.mem_read(rt(0x95680), 1))
        e.uc.mem_write(rt(0x95680), sv)
        script = {k: list(v) for k, v in base.items()}
        log.clear()
        ps = bytearray(e.uc.mem_read(rt(G_SOLDIERS) + 0x200 * me, 0x200))
        want = ai.sight_line(ps, World(me), x, y, tile, tx, ty, flag, rg, slot)
        want_g = bytes(e.uc.mem_read(rt(0x95680), 1))
        counts["sight_line"] = counts.get("sight_line", 0) + 1
        results.setdefault("sight_line", {}).setdefault(want, 0)
        results["sight_line"][want] += 1
        if (got, got_log, got_g) != (want, log, want_g):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH sight_line(({x}, {y}) -> ({tx}, {ty}) tile {tile} flag {flag} range {rg} slot {slot}): "
                      f"original {got} port {want}\n  calls original {got_log}\n  calls port     {log}")
    e.uc.mem_write(rt(G_TILEMAP), tmap)
    e.uc.mem_write(rt(G_SOLDIERS), soldiers)
    e.uc.mem_write(rt(ai.G_WINDOW_TBL), wtbl)
    e.w32(rt(ai.G_SIGHT_BLOCK), block)
    e.stubs[rt(0x58760)] = stub
if direct("throw_chance"):   # direct call: the last-target +10 is rarely decisive inside ThrowOption
    for i in range(3000):
        slot = rng.choice(list(range(16)) + list(range(0x28, 0x30)))
        sp = rt(G_SOLDIERS) + 0x200 * slot
        s = bytearray(e.uc.mem_read(sp, 0x200))
        for o in (0x18B, 0x18C, 0x191, 0x192, 0x184):
            s[o] = rng.randrange(1, 101)
        s[0x183] = rng.randrange(1, s[0x184] + 1)
        s[0x185] = rng.choice([0, 0, rng.randrange(20)])
        s[0x18A] = rng.choice([100, rng.randrange(1, 101)])
        s[0x24] = rng.choice([0, 0x27, 0x28])
        s[0x00] = rng.choice([0x16, 0x18, 0x1A, 0x1C] + KIND.get(0x95, []))
        s[0x17A] = slot
        me = rng.randrange(3136)
        ai.w16(s, 0x114, me)
        ai.w16(s, 0x116, ai.tile_x(me))
        ai.w16(s, 0x118, ai.tile_y(me))
        tile = rng.choice([me + rng.randrange(-300, 300), rng.randrange(3136)]) % 3136
        ai.w16(s, 0xD0, rng.choice([tile, tile, ai.NO_TILE, rng.randrange(3136)]))
        e.uc.mem_write(sp, bytes(s))
        aim = rng.choice([0, 1, 2, 3, 4, 0x100])
        got = ai._i32(e.call(rt(0x58BB0), sp, tile, aim))
        want = ai._i32(ai.throw_chance(bytearray(s), World(slot), tile, aim))
        counts["throw_chance"] = counts.get("throw_chance", 0) + 1
        results.setdefault("throw_chance", {}).setdefault(want, 0)
        results["throw_chance"][want] += 1
        if got != want:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH throw_chance(slot {slot}, tile {tile}, aim {aim}): original {got} port {want}")
if direct("tile_distance"):   # pure function: called directly with the stub lifted
    stub = e.stubs.pop(rt(0x15A24))
    for i in range(2000):
        a, b = (rng.choice([rng.randrange(3136), rng.randrange(-200, 0), 56 * rng.randrange(56)]) for _ in "ab")
        got, want = ai._i32(e.call(rt(0x15A24), a & 0xFFFFFFFF, b & 0xFFFFFFFF)), ai.tile_distance(a, b)
        counts["tile_distance"] = counts.get("tile_distance", 0) + 1
        results.setdefault("tile_distance", {}).setdefault(want, 0)
        results["tile_distance"][want] += 1
        if got != want:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH tile_distance({a}, {b}): original {got} port {want}")
    e.stubs[rt(0x15A24)] = stub
if direct("make_noise"):   # no soldier argument: called directly with the stub lifted
    stub = e.stubs.pop(rt(0x727D0))
    soldiers = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    known = bytes(e.uc.mem_read(rt(ai.G_KNOWN), 0x40))
    combat_flag = e.r8(rt(ai.G_COMBAT))
    for i in range(4000):
        src = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30), 0x40, 0x41, 0x42,
                          rng.randrange(0x30, 0x40), 0xFF, rng.randrange(16)])
        if PORT and 0x30 <= src < 0x40:      # (ai.md SPEC GAP: the original reads past the 48 records)
            src = 0x40
        tile = rng.choice([rng.randrange(3136), rng.randrange(3136), ai.NO_TILE])
        tx, ty = ai.tile_x(tile), ai.tile_y(tile)
        for k in range(0x30):
            a = rt(G_SOLDIERS) + 0x200 * k
            if rng.random() < 0.5:           # near the noise
                e.w16(a + 0x116, tx + rng.randrange(-200, 201))
                e.w16(a + 0x118, ty + rng.randrange(-200, 201))
            e.w8(a + 0x182, rng.choice([1, 1, 1, 0]))
            e.w8(a + 0x138, rng.choice([1, 1, 1, 0]))
            e.w8(a + 0x188, rng.choice([0, 0, 0, 1]))
            e.w8(a + 0x183, rng.choice([0, 14, 15, 30, 60, 0x90]))
            e.w8(a + 0xBA, rng.choice([0, 1, 2, 3, 4]))
            e.w8(a + 0x1D0, rng.choice([0, 1, 3, 0xFE, rng.randrange(256)]))
            e.w8(a + 0x1D5, rng.choice([0, 0, 1, 2, 0xFF, rng.randrange(256)]))
            e.w8(a + 0x1B3, rng.choice([0, 1, 3, 3, 5, 10, rng.randrange(256)]))
            if rng.random() < 0.3:           # whole heard-flag table 0x13A..0x179 (0x13A + src / 0x112 + src): src 0x40 reads none
                e.uc.mem_write(a + 0x13A, bytes(rng.choice([0, 0, 1, 2, 0xFF]) for _ in range(0x40)))
            if src < 0x40:
                e.w8(a + (0x13A if src < 0x28 else 0x112) + src, rng.choice([0, 1, 2, 0xFF]))
        e.uc.mem_write(rt(ai.G_KNOWN), bytes(rng.choice([0, 0, 1, 2, 0xFF]) for _ in range(0x40)))
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1, 1, 1, 1]))
        cls = rng.choice([0, 1, 3, 5, 9, 10, 0xFF])
        loud = rng.choice([0, 1, 3, 6, 10, 14, 20, 40, 100, 0x10003, rng.randrange(1 << 32)])
        kind = rng.choice([1, 4, 4, 5, 6, 7, 0x104])
        script = {"hears": [0] * 0x30}
        log.clear()
        e.call(rt(0x727D0), src, tile, cls, loud, kind)
        got_log = list(log)
        log.clear()
        if PORT:
            port_world("make_noise", (src, tile, cls, loud, kind))
        else:
            ai.make_noise(World(0), src, tile, cls, loud, kind)
        counts["make_noise"] = counts.get("make_noise", 0) + 1
        results.setdefault("make_noise", {}).setdefault(len(log), 0)
        results["make_noise"][len(log)] += 1
        if got_log != log:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH make_noise(src {src:#x}, tile {tile}, class {cls}, loud {loud}, kind {kind}):\n"
                      f"  calls original {got_log}\n  calls port     {log}")
    e.uc.mem_write(rt(G_SOLDIERS), soldiers)
    e.uc.mem_write(rt(ai.G_KNOWN), known)
    e.w8(rt(ai.G_COMBAT), combat_flag)
    e.stubs[rt(0x727D0)] = stub
if direct("hide_path_marks"):   # no soldier: called directly with the stub lifted
    stub = e.stubs.pop(rt(0x1D8B8))
    tmap = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
    area = [(0x229DD8, 4), (0x229DF4, 4), (0x229E08, 4), (0x229DEC, 4), (0x229D2C, 4), (0x1163BF, 1), (0x10DF90, 0x100)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    for i in range(3000):
        e.w32(rt(0x229DD8), rng.choice([0, 1, 1, 0xFFFFFFFF, 5]))
        e.w32(rt(0x229DF4), rng.choice([0, 1, 1, 7]))
        e.w8(rt(0x1163BF), rng.choice([0, 1, 1]))
        k = rng.choice([0, 1, 5, 40, 0x80, 0xFFFFFFFF])     # (0x80 = the whole list)
        e.w32(rt(0x229E08), k)
        e.uc.mem_write(rt(0x10DF90), b"".join(rng.randrange(3136).to_bytes(2, "little") for _ in range(0x80)))
        for t in rng.sample(range(3136), 60):
            e.w8(rt(G_TILEMAP) + 8 * t + 7, rng.choice([0, 1, 2, 0xFF]))
        e.w32(rt(0x229DEC), rng.randrange(1 << 32))
        e.w32(rt(0x229D2C), rng.randrange(1 << 32))
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))]
        force = rng.choice([0, 1, 0x100, rng.randrange(1 << 32)])
        e.call(rt(0x1D8B8), force)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        e.uc.mem_write(rt(G_TILEMAP), snap[-1])
        if PORT:                         # select.md Hide path marks; the AP text and cursor tile are the front-end's
            port_world("hide_path_marks", (force,), sync=True)
            e.w32(rt(0x229DEC), 0)
            e.w32(rt(0x229D2C), ai.NO_TILE)
        else:
            ai.hide_path_marks(World(0), force)
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))]
        counts["hide_path_marks"] = counts.get("hide_path_marks", 0) + 1
        results.setdefault("hide_path_marks", {}).setdefault(got != snap, 0)
        results["hide_path_marks"][got != snap] += 1
        if got != want:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH hide_path_marks case {i}: areas differ {[n for n, (x, y) in enumerate(zip(got, want)) if x != y]}")
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    e.uc.mem_write(rt(G_TILEMAP), tmap)
    e.stubs[rt(0x1D8B8)] = stub
BLAST_OPS = ("take_damage", "rest", "item_panel", "w_17b70", "stat_practice", "break_obj", "direction", "energy_bar",
             "w_33204", "ui_refresh", "focus", "cry", "voc", "react", "w_74304", "move_ap", "next_turn", "sight_line",
             "tile_dist", "w_1304c", "w_73ab4", "w_73d28", "w_73f3c", "merc_sees", "share_sightings", "interrupt_check",
             "ask_interrupt", "ask_merc_interrupt", "merc_interrupt", "mark_seen")
EFFECT_OPS = ("explode_art", "w_3c05c", "story_hit", "replay_event")   # combat.md Effect hit
BLAST_REAL = ("rest", "w_74304", "react")   # --port: the library runs Rest and Hit reaction inside (combat.md Blast, exact steps)


def blast_real():
    """--port: lift the original's stubs of the procedures the library runs inside a blast; returns them"""
    if PORT:
        port_lib.BLAST = True
        for a_ in range(rt(G_SOLDIERS), rt(G_SOLDIERS) + 0x6000, 0x200):   # (port fuzz clamp: earlier sections leave
            for o_ in [12 * j_ for j_ in range(12)] + [12 * j_ + 3 for j_ in range(12)]:   # item numbers past the
                if e.r8(a_ + o_) >= 136:                                      # 136-type table, read as 0 by the port)
                    e.w8(a_ + o_, 0)
    return {c_: e.stubs.pop(rt(CALLEES[c_][0])) for c_ in (BLAST_REAL if PORT else ())}


def blast_restore(real):
    if PORT:
        port_lib.BLAST = False
    for c_, st_ in real.items():
        e.stubs[rt(CALLEES[c_][0])] = st_


if direct("blast_tile"):   # tile argument, no soldier record: called directly, port on memory
    stub = e.stubs.pop(rt(0x38638), None)
    real_ = blast_real()
    tmap = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
    soldiers = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    area = [(ai.G_BLASTER, 1)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    # (a record's own slot number +0x17A: an empty enemy record has 0 there. The port names the soldier by +0x17A, the
    # original by its pointer; a sector with fewer than 8 enemies (daysave_orig, wl_pre) gave ~1000 false mismatches.
    # The pick still draws its random number, so the cases of the other dumps stay the same.)
    own_ = {k_ for k_ in list(range(16)) + list(range(0x28, 0x30)) if e.r8(rt(G_SOLDIERS) + 0x200 * k_ + 0x17A) == k_}
    for i in range(4000):
        k = rng.choice([rng.randrange(16), rng.randrange(0x28, 0x30)])
        if k not in own_ and own_:
            k = min(own_, key=lambda j_: (abs(j_ - k), j_))
        a_ = rt(G_SOLDIERS) + 0x200 * k
        tile = e.r16(a_ + 0x114) % 3136
        e.w8(rt(G_TILEMAP) + 8 * tile + 5, rng.choice([k, k, k | 0x80, 0x40, 0x7F]))
        centre = rng.choice([tile, (tile + rng.choice([1, -1, 56, -56, 57, 2, 112])) % 3136, rng.randrange(3136)])
        e.w8(a_ + 0x182, rng.choice([1, 1, 1, 0]))
        e.w8(a_ + 0x138, rng.choice([1, 1, 1, 0]))
        e.w8(a_ + 0x183, rng.choice([0, 5, 14, 15, 40, 100]))
        e.w8(a_ + 0xBA, rng.choice([0, 1, 3, 4]))
        e.w8(a_ + 0x1F3, rng.randrange(5))
        e.w16(a_ + 0x1E8 + 2 * e.r8(a_ + 0x1F3), rng.choice([0, 3, 4, 0x28, 0x39]))
        e.w8(a_ + 0x24, rng.choice([0, 0x28, 0x28, 5]))
        e.w8(a_ + 0x26, rng.choice([0, 50, 100, 0x80]))
        for o_ in (0x1DB, 0x1C1, 0x1C4, 0x1DA):
            e.w8(a_ + o_, rng.choice([0, 3]))
        if rng.random() < 0.5:                  # items to wear: helmets, armour, stacks, attachments
            for j_ in range(12):
                b_ = a_ + 12 * j_
                e.w8(b_, rng.choice([0, 0x23, 0x24, 0x25, 0x26, 0x2D, 0x2F, 0x31, rng.randrange(1, 136)]))
                e.w8(b_ + 1, rng.choice([0, 1, 2, 5, 0x80]))
                e.w8(b_ + 2, rng.choice([1, 50, 100, 0x80]))
                e.w8(b_ + 3, rng.choice([0, 0, rng.randrange(1, 136)]))
                e.w8(b_ + 4, rng.choice([0, rng.randrange(1, 136)]))
                e.w8(b_ + 5, rng.choice([1, 50, 100]))
                for c_ in range(4):
                    e.w8(b_ + 8 + c_, rng.choice([0, 1, 30, 100, 0x80]))
        e.w8(rt(G_TILEMAP) + 8 * e.r16(a_ + 0x114) % (8 * 3136), rng.choice([0x6C, 0x10, 0x70, e.r8(rt(G_TILEMAP) + 8 * tile)]))
        e.w8(rt(ai.G_BLASTER), rng.choice([rng.randrange(8), rng.randrange(8, 0x30), 0x40, k] +
                                          ([] if PORT else [rng.randrange(0x30, 0x40)])))   # (--port: no record 0x30..0x3F)
        kind = rng.choice([0, 1, 2, 2, 3, 3, 4, 5, 6, 7, 8, 9, 10, 0x63, 0x100])
        ring = rng.choice([0, 1, 2, 3, 4, 5, 6, 0xFFFFFFFF])
        base = {"Random": [rng.choice([0, 1, 7, 25, 50, 999, 1000, 2000, rng.randrange(1 << 15)]) for _ in range(40)],
                "direction": [rng.choice([1, 4, 8, 0x105])], "take_damage": [0], "rest": [0], "item_panel": [0],
                "w_17b70": [0], "w_74304": [0], "stat_practice": [0]}
        for k_ in BLAST_OPS:
            base.setdefault(k_, [0] * 200)
        script = {k_: list(v) for k_, v in base.items()}
        snap = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))] + \
               [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
        log.clear()
        e.call(rt(0x38638), centre, tile, ring, kind)
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))] + \
              [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
        e.uc.mem_write(rt(G_TILEMAP), snap[0])
        e.uc.mem_write(rt(G_SOLDIERS), snap[1])
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        if PORT:
            port_sector("blast_tile", (centre, tile, ai._i32(ring), ai._i32(kind)), allow=BLAST_OPS)
        else:
            w_ = World(0)
            w_.memrun = True
            ai.blast_tile(w_, centre, tile, ring, kind)
        want = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))] + \
               [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
        counts["blast_tile"] = counts.get("blast_tile", 0) + 1
        results.setdefault("blast_tile", {}).setdefault(kind, 0)
        results["blast_tile"][kind] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                sd = [hex(o) for o in range(0x6000) if got[1][o] != want[1][o]][:8]
                print(f"MISMATCH blast_tile(centre {centre}, tile {tile}, ring {ring}, kind {kind}): soldier bytes {sd} "
                      f"map {got[0] != want[0]}\n  calls original {got_log}\n  calls port     {log}")
        e.uc.mem_write(rt(G_TILEMAP), snap[0])
        e.uc.mem_write(rt(G_SOLDIERS), snap[1])
    e.uc.mem_write(rt(G_TILEMAP), tmap)
    e.uc.mem_write(rt(G_SOLDIERS), soldiers)
    for (a, k_), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    if stub:
        e.stubs[rt(0x38638)] = stub
    blast_restore(real_)
if direct("blast"):   # the whole blast spread over real soldiers and map, called directly
    real_ = blast_real()
    tmap = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
    soldiers = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    save = bytes(e.uc.mem_read(rt(ai.G_BLASTER), 1))
    OBJS = [0, 0, 0, 0x10, 0x26, 0x27, 0x30, 0x37, 0x38, 0x40, 0x50, 0x60, 0x82, 0x83, 0xE2]
    for i in range(1500):
        k = rng.choice([rng.randrange(16), rng.randrange(0x28, 0x30)])
        centre = (e.r16(rt(G_SOLDIERS) + 0x200 * k + 0x114) + rng.choice([0, 1, -1, 56, -57, 113, rng.randrange(3136)])) % 3136
        for t in range(max(0, centre - 6 * 57), min(3136, centre + 6 * 57)):
            if rng.random() < 0.15:
                e.w8(rt(G_TILEMAP) + 8 * t + 1, rng.choice(OBJS + [rng.randrange(256)]))
        for j in range(0x30):
            if rng.random() < 0.3:              # soldiers near the blast
                a_ = rt(G_SOLDIERS) + 0x200 * j
                t = (centre + rng.randrange(-4, 5) + 56 * rng.randrange(-4, 5)) % 3136
                if e.r8(rt(G_TILEMAP) + 8 * t + 5) & 0x7F >= 0x30:
                    e.w8(rt(G_TILEMAP) + 8 * t + 5, j)
                    e.w16(a_ + 0x114, t)
        e.w8(rt(ai.G_BLASTER), rng.choice([rng.randrange(8), rng.randrange(0x28, 0x30), 0x40]))
        radius = rng.choice([0, 1, 2, 3, 4, 5, 6, 0x10001])
        kind = rng.choice([2, 3, 3, 4, 5, 6, 8, 9, 0x63])
        base = {"Random": [rng.choice([0, 1, 7, 25, 50, 999, 2000, rng.randrange(1 << 15)]) for _ in range(400)],
                "direction": [rng.choice([1, 4, 8])] * 200, "take_damage": [0] * 200, "rest": [0] * 200,
                "item_panel": [0] * 200, "w_17b70": [0] * 200, "w_74304": [0] * 200, "stat_practice": [0] * 200,
                "break_obj": [0] * 20}
        for k_ in BLAST_OPS:
            base.setdefault(k_, [0] * 200)
        script = {k_: list(v) for k_, v in base.items()}
        snap = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))]
        log.clear()
        e.call(rt(0x38B60), centre, radius, kind)
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))]
        e.uc.mem_write(rt(G_TILEMAP), snap[0])
        e.uc.mem_write(rt(G_SOLDIERS), snap[1])
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        if PORT:
            port_sector("blast", (centre, ai._i32(radius), kind), allow=BLAST_OPS)
        else:
            w_ = World(0)
            w_.memrun = True
            ai.blast(w_, centre, radius, kind)
        want = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))]
        counts["blast"] = counts.get("blast", 0) + 1
        results.setdefault("blast", {}).setdefault(sum(1 for n_, _ in log if n_ == "Random"), 0)
        results["blast"][sum(1 for n_, _ in log if n_ == "Random")] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                sd = [hex(o) for o in range(0x6000) if got[1][o] != want[1][o]][:8]
                md = [o // 8 for o in range(8 * 3136) if got[0][o] != want[0][o]][:8]
                print(f"MISMATCH blast(centre {centre}, radius {radius}, kind {kind}): soldier bytes {sd} map tiles {md}\n"
                      f"  calls original {got_log[:30]}\n  calls port     {log[:30]}")
        e.uc.mem_write(rt(G_TILEMAP), snap[0])
        e.uc.mem_write(rt(G_SOLDIERS), snap[1])
    e.uc.mem_write(rt(G_TILEMAP), tmap)
    e.uc.mem_write(rt(G_SOLDIERS), soldiers)
    e.uc.mem_write(rt(ai.G_BLASTER), save)
    blast_restore(real_)
if direct("sector_tick"):   # SectorTick? 0x1583C (op 160): random water-tile splash animation, called directly
    tmap = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
    soldiers = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    area = [(0x229AFC, 4), (ai.G_SPLASH, 60), (ai.G_CUR_SECTOR, 1)]
    save = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
    for i in range(3000):
        sec_ = rng.randrange(60)
        e.w8(rt(ai.G_CUR_SECTOR), sec_)
        e.w8(rt(ai.G_SPLASH) + sec_, rng.choice([1, 5, 8, 12, 20, 20, 0xFF, 0x80, 0x7F, rng.randrange(1, 256)]))
        tiles_ = []
        for j in rng.sample(range(0x30), 8):          # soldiers on fuzzed tiles
            a_ = rt(G_SOLDIERS) + 0x200 * j
            t = rng.randrange(3136)
            tiles_.append(t)
            e.w8(rt(G_TILEMAP) + 8 * t, rng.choice([0x97, 0xA0, 0xFF, 0x96, 0x10]))
            e.w8(rt(G_TILEMAP) + 8 * t + 1, rng.choice([0, 0, 0, 5]))
            e.w8(rt(G_TILEMAP) + 8 * t + 5, rng.choice([j, j, 0x80 | j, 0x40, 0x7F]))   # (0x30..0x3F: past the records)
            e.w8(a_ + 0x183, rng.choice([0, 1, 50, 0x80, 0xFF]))
            k_ = rng.randrange(4)
            e.w8(a_ + 0x1F3, k_)
            e.w16(a_ + 0x1E8 + 2 * k_, rng.choice([0, 4, 0x3B, 0x3C, 0x3D, 0x100, rng.randrange(1 << 16)]))
        base = {"Random": [rng.choice(tiles_ + [rng.randrange(0xC40)]) for _ in range(1000)], "set_anim": [0] * 2}
        script = {k_: list(v) for k_, v in base.items()}
        snap = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))] + \
               [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
        log.clear()
        e.call(rt(0x1583C))
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))] + \
              [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
        e.uc.mem_write(rt(G_TILEMAP), snap[0])
        e.uc.mem_write(rt(G_SOLDIERS), snap[1])
        for (a, k_), b in zip(area, snap[2:]):
            e.uc.mem_write(rt(a), b)
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        if PORT:
            port_sector("sector_tick", (), allow=("set_anim",))
        else:
            w_ = World(0)
            w_.memrun = True
            ai.sector_tick(w_)
        want = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))] + \
               [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
        counts["sector_tick"] = counts.get("sector_tick", 0) + 1
        k_ = sum(1 for c_ in log if c_[0] == "set_anim")
        results.setdefault("sector_tick", {}).setdefault(k_, 0)
        results["sector_tick"][k_] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH sector_tick(sector {sec_}): areas {[n for n, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log[-6:]} ({len(got_log)})\n  calls port     {log[-6:]} ({len(log)})")
        e.uc.mem_write(rt(G_TILEMAP), snap[0])
        e.uc.mem_write(rt(G_SOLDIERS), snap[1])
        for (a, k_), b in zip(area, snap[2:]):
            e.uc.mem_write(rt(a), b)
    for (a, k_), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    e.uc.mem_write(rt(G_TILEMAP), tmap)
    e.uc.mem_write(rt(G_SOLDIERS), soldiers)
if direct("gas_tick"):   # the 5 gas clouds over real soldiers and map, called directly
    real_ = blast_real()
    tmap = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
    soldiers = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    area = [(ai.G_BLASTER, 1), (ai.G_GAS_CLOUDS, 30)]
    save = [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
    for i in range(600):
        for k in range(5):
            a_ = rt(ai.G_GAS_CLOUDS) + 6 * k
            if rng.random() < 0.3:
                e.w16(a_, ai.NO_TILE)
            else:
                j = rng.choice([rng.randrange(16), rng.randrange(0x28, 0x30)])
                e.w16(a_, (e.r16(rt(G_SOLDIERS) + 0x200 * j + 0x114) + rng.choice([0, 1, -56, 57, rng.randrange(3136)])) % 3136)
            e.w16(a_ + 2, rng.choice([0, 1, 1, 2, 3, 5, 0xFFFF, 0x8000]))
            e.w16(a_ + 4, rng.choice([0, 1, 2, 3, 3, 5, 0xFFFF]))
        e.w8(rt(ai.G_BLASTER), rng.choice([rng.randrange(8), rng.randrange(0x28, 0x30), 0x40]))
        base = {"Random": [rng.choice([0, 1, 7, 999, 2000, rng.randrange(1 << 15)]) for _ in range(2000)],
                **{k_: [0] * 1000 for k_ in BLAST_OPS}}
        snap = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))] + \
               [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        e.call(rt(0x38520))
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))] + \
              [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
        e.uc.mem_write(rt(G_TILEMAP), snap[0])
        e.uc.mem_write(rt(G_SOLDIERS), snap[1])
        for (a, k_), b in zip(area, snap[2:]):
            e.uc.mem_write(rt(a), b)
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        if PORT:
            port_sector("gas_tick", (), allow=BLAST_OPS)
        else:
            w_ = World(0)
            w_.memrun = True
            ai.gas_tick(w_)
        want = [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)), bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))] + \
               [bytes(e.uc.mem_read(rt(a), k_)) for a, k_ in area]
        counts["gas_tick"] = counts.get("gas_tick", 0) + 1
        nc_ = sum(1 for k in range(5) if ai.s16(e.r16(rt(ai.G_GAS_CLOUDS) + 6 * k)) == ai.NO_TILE)
        results.setdefault("gas_tick", {}).setdefault(nc_, 0)
        results["gas_tick"][nc_] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                sd = [hex(o) for o in range(0x6000) if got[1][o] != want[1][o]][:8]
                md = [o // 8 for o in range(8 * 3136) if got[0][o] != want[0][o]][:8]
                print(f"MISMATCH gas_tick: soldier bytes {sd} map tiles {md} clouds {got[3] != want[3]}\n"
                      f"  calls original {got_log[:30]}\n  calls port     {log[:30]}")
        e.uc.mem_write(rt(G_TILEMAP), snap[0])
        e.uc.mem_write(rt(G_SOLDIERS), snap[1])
        for (a, k_), b in zip(area, snap[2:]):
            e.uc.mem_write(rt(a), b)
    e.uc.mem_write(rt(G_TILEMAP), tmap)
    e.uc.mem_write(rt(G_SOLDIERS), soldiers)
    for (a, k_), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    blast_restore(real_)
if direct("workable"):   # strategic sector table: called directly with the stub lifted
    stub = e.stubs.pop(rt(0x41E1C))
    tsave = bytes(e.uc.mem_read(rt(ai.G_SECTOR_TBL9), 9 * 60))
    for i in range(4000):
        for k in range(60):
            if rng.random() < 0.5:
                e.w8(rt(ai.G_SECTOR_TBL9) + 9 * k, rng.choice([0, 1, 1, 3, 5, 9, 0x11, 0x1F, rng.randrange(256)]))
        sec = rng.randrange(60)
        b_ = rt(ai.G_SECTOR_TBL9) + 9 * sec
        e.w8(b_ + 6, rng.choice([0, 1, 10, 40, 100, 200, 255]))
        e.w8(b_ + 8, rng.choice([0, 0, 5, 40, 255]))
        base_ = {"connected_home": [rng.choice([0, 1, 1, 0x100])]}
        script = {k_: list(v) for k_, v in base_.items()}
        snap = bytes(e.uc.mem_read(rt(ai.G_SECTOR_TBL9), 9 * 60))
        log.clear()
        got_r = e.call(rt(0x41E1C), rt(ai.G_SECTOR_TBL9), sec) & 0xFF
        got, got_log = bytes(e.uc.mem_read(rt(ai.G_SECTOR_TBL9), 9 * 60)), list(log)
        e.uc.mem_write(rt(ai.G_SECTOR_TBL9), snap)
        script = {k_: list(v) for k_, v in base_.items()}
        log.clear()
        want_r = ai.sector_workable(World(0), ai.G_SECTOR_TBL9, sec) & 0xFF
        want = bytes(e.uc.mem_read(rt(ai.G_SECTOR_TBL9), 9 * 60))
        counts["workable"] = counts.get("workable", 0) + 1
        results.setdefault("workable", {}).setdefault(min(want_r, 9), 0)
        results["workable"][min(want_r, 9)] += 1
        if (got_r, got, got_log) != (want_r, want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH workable(sector {sec}): ret {got_r} / {want_r}, b {snap[9 * sec:9 * sec + 9].hex()}"
                      f"\n  calls original {got_log}\n  calls port     {log}")
        e.uc.mem_write(rt(ai.G_SECTOR_TBL9), snap)
    e.uc.mem_write(rt(ai.G_SECTOR_TBL9), tsave)
    e.stubs[rt(0x41E1C)] = stub
if direct("connected"):   # the maze walk to home: called directly, reads 90 bytes on each side unchecked
    stub = e.stubs.pop(rt(0x41178))
    lo, n_ = rt(ai.G_SECTOR_TBL9) - 90, 9 * 60 + 180
    FRAME = emu.STACK_TOP - 0x104 - 16 - 0xC0   # its locals: junction stack, done flags, b0 copy (left after return)
    tsave = bytes(e.uc.mem_read(lo, n_))
    rg_ = random.Random(1000003 * 80 + 1) if PORT else rng     # --port: its own draws (the later sections keep theirs)
    for i in range(6000 if not PORT else max(n // 2, 100)):
        dens, out = rg_.choice([0.3, 0.6, 0.8, 0.95, 1.0]), rg_.choice([0, 0.3, 1])   # out: rate of outside bytes
        for k in range(-10, 70):
            if PORT and k < 0:          # the port reads entries outside the table as 0 (soldier_update.md Joined to home)
                e.w8(rt(ai.G_SECTOR_TBL9) + 9 * k, 0)
            elif 0 <= k < 60 or rg_.random() < out:
                v = (1 if rg_.random() < dens else 0) | (rg_.choice([0, 0, 0, 2, 4, 8, 0x10, 0x1E, rg_.randrange(256)])
                                                        if rg_.random() < 0.3 else 0)
                e.w8(rt(ai.G_SECTOR_TBL9) + 9 * k, v)
        sec = rg_.choice([rg_.randrange(60), rg_.randrange(60), ai.HOME_SECTOR])   # 60+ reads past the frame
        snap = bytes(e.uc.mem_read(lo, n_))
        e.uc.mem_write(FRAME, bytes(0xB4))
        try:
            got_r = e.call(rt(0x41178), rt(ai.G_SECTOR_TBL9), sec, max_steps=20_000_000) & 0xFF
        except emu.EmuStop as x:
            if "step limit" not in str(x):
                raise
            hangs["connected"] = hangs.get("connected", 0) + 1
            e.uc.mem_write(lo, snap)
            continue
        got = bytes(e.uc.mem_read(lo, n_)) + (bytes(e.uc.mem_read(FRAME, 0xB4)) if sec != ai.HOME_SECTOR else b"")
        e.uc.mem_write(lo, snap)
        if PORT:                        # result and table (the port keeps no walk array)
            got = got[:n_]
            log.clear()
            want_r = port_world("connected_home", (sec,)) & 0xFF
            want = bytes(e.uc.mem_read(lo, n_))
        else:
            fr = bytearray(0xB4)
            want_r = ai.connected_home(World(0), ai.G_SECTOR_TBL9, sec, fr) & 0xFF
            want = bytes(e.uc.mem_read(lo, n_)) + (bytes(fr) if sec != ai.HOME_SECTOR else b"")
        counts["connected"] = counts.get("connected", 0) + 1
        results.setdefault("connected", {}).setdefault(want_r, 0)
        results["connected"][want_r] += 1
        if (got_r, got) != (want_r, want):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH connected(sector {sec}): ret {got_r} / {want_r}, "
                      f"b0 {bytes(snap[90 + 9 * k] for k in range(60)).hex()}")
        e.uc.mem_write(lo, snap)
    e.uc.mem_write(lo, tsave)
    e.stubs[rt(0x41178)] = stub
if direct("ledger"):   # day accounts: CalcLedgerAll / ApplyDayLedger / RecalcTerritory with every callee real
    class LedgerWorld(World):
        def ext(self, name, *args):
            if name == "connected_home":
                return ai.connected_home(self, *args)
            if name == "workable":
                return ai.sector_workable(self, *args)
            return super().ext(name, *args)
    lifted = {a_: e.stubs.pop(rt(a_)) for a_ in (0x41178, 0x41E1C, 0x41574, 0x50EA8, 0x5102C, 0x51068, 0x5113C,
                                                0x50CF4, 0x50D2C) if rt(a_) in e.stubs}
    REG = [(0x229FA0, 0x98), (ai.G_PREV_BALANCE, 4), (ai.G_SECTOR_TBL9, 9 * 60), (ai.G_PLANTS, 48),
           (G_SOLDIERS, 0x200 * 8), (ai.G_TEXT_BUF, 48), (ai.G_SECTOR_ORDER, 61), (ai.G_DIFFICULTY, 1)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    if PORT:
        port_lib.LEDGER = True   # the world gets accounts / prev_balance / sector_order
    PORT_LEDGER = {"all": "calc_ledger_all", "apply": "apply_day_ledger", "territory": "recalc_territory",
                   "pool": "worker_pool", "quit": "natives_quit"}
    KINDS = {"all": (0x51178, ai.calc_ledger_all), "apply": (0x51210, ai.apply_day_ledger),
             "territory": (0x50EA8, ai.recalc_territory), "pool": (0x50BC4, ai.worker_pool),
             "quit": (0x50D2C, ai.natives_quit)}
    for i in range(max(n // 2, 100)):
        kind = rng.choice(list(KINDS))
        dens = rng.choice([0.2, 0.6, 0.9, 1.0])
        for k in range(60):
            b_ = rt(ai.G_SECTOR_TBL9) + 9 * k
            e.w8(b_, (1 if rng.random() < dens else 0) | (rng.choice([0, 2, 4, 8, 0x10, 0x1E, rng.randrange(256)])
                                                           if rng.random() < 0.2 else 0))
            for o_ in (3, 4, 6, 7, 8):
                e.w8(b_ + o_, rng.choice([0, 0, 1, 2, 3, 5, 9, 12, 30, rng.randrange(256)]))
        for k in range(4):
            p_ = rt(ai.G_PLANTS) + 12 * k
            e.w32(p_, rng.choice([0, 20, 35, 25, rng.randrange(1, 100), 0xFFFFFFF0]))
            e.w32(p_ + 4, rng.choice([59, 26, 53, rng.randrange(60), rng.randrange(60)]))   # >= 60: undefined
            e.w8(p_ + 8, rng.choice([0, 1, 2, 2, 3, 0x82]))
        nteam = rng.randrange(10)
        for k in range(8):
            s_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(s_ + 0x182, 1 if k < nteam else rng.choice([0, 0, 1]))
            e.w16(s_ + 0x110, rng.choice([0, 1, 250, 333, 999, 1500, 0xFFFF, rng.randrange(0x10000)]))
            e.w16(s_ + 0x112, rng.randrange(0x10000))
            e.w8(s_ + 0x17E, rng.choice([0, 0, 1, 2, 3, 4, 7]))
        for a_ in range(0x229FA0, 0x22A030, 4):
            if rng.random() < 0.5:
                e.w32(rt(a_), rng.choice([0, 1, 35, 500, 19200, -5000 & 0xFFFFFFFF, rng.randrange(1 << 32)]))
        e.w32(rt(ai.G_PREV_BALANCE), rng.choice([0, 19200, rng.randrange(1 << 32)]))
        for a_ in (ai.G_NATIVE_WAGE, ai.G_TREE_PRICE, ai.G_WORKER_POOL, ai.G_PROCESS_CAP):
            e.w32(rt(a_), rng.choice([0, 3, 35, 500, 40, 1000, rng.randrange(-50, 200) & 0xFFFFFFFF]))
        order_ = list(range(60)) + [rng.choice([59, 0, rng.randrange(60)])]   # sector >= 60: original undefined
        rng.shuffle(order_)
        e.uc.mem_write(rt(ai.G_SECTOR_ORDER), bytes(order_))
        e.w8(rt(ai.G_DIFFICULTY), rng.choice([0, 1, 2, 2]))
        e.w32(rt(ai.G_SECTORS_GAINED), rng.choice([0, 1, 9, 10, 11, 35, 149, 150, 200, -3 & 0xFFFFFFFF,
                                                   rng.randrange(1 << 32)]))
        for a_ in (ai.G_SECTORS_LOST, ai.G_NATIVE_DEATHS_TOTAL, 0x22A030, 0x22A034):
            e.w32(rt(a_), rng.choice([0, 0, 1, 3, 12, 40, rng.randrange(1 << 32)]))
        if kind == "pool" and rng.random() < 0.5:     # many natives hired: the excess quits
            e.w32(rt(ai.G_NATIVE_WAGE), rng.choice([0, 10, 35]))
        if kind == "pool" and random.Random(1000003 * i + 17).random() < 0.15:   # pool 0 = no natives hired:
            for k in range(60):                       # idle exactly 0 (own rng: the shared one stays unshifted)
                e.w8(rt(ai.G_SECTOR_TBL9) + 9 * k + 3, 0)
                e.w8(rt(ai.G_SECTOR_TBL9) + 9 * k + 4, 0)
            e.w32(rt(ai.G_NATIVE_DEATHS_TOTAL), 0x7000)
        nq_ = rng.choice([0, 1, 2, 3, 5, 17, 60, 255, rng.randrange(256)]) | rng.choice([0, 0x100])
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        addr_, fn_ = KINDS[kind]
        script = {"message": [], "draw_sectors": [], "w_36010": []}
        log.clear()
        try:
            e.call(rt(addr_), *([nq_] if kind == "quit" else []), max_steps=20_000_000)
        except emu.EmuStop as x:    # the walk to home never ends on some tables (strategic.md Connected to home)
            if "step limit" not in str(x):
                raise
            hangs["ledger"] = hangs.get("ledger", 0) + 1
            for (a_, k_), b_ in zip(REG, snap):
                e.uc.mem_write(rt(a_), b_)
            continue
        got, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        log.clear()
        script = {"message": [], "draw_sectors": [], "w_36010": []}
        if PORT:
            port_world(PORT_LEDGER[kind], (nq_,) if kind == "quit" else (), sync=True)
        else:
            fn_(LedgerWorld(0), *([nq_] if kind == "quit" else []))
        want = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["ledger"] = counts.get("ledger", 0) + 1
        results.setdefault("ledger", {}).setdefault(kind, 0)
        results["ledger"][kind] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                diff_ = [(hex(a_ + j_), g_[j_], w_[j_]) for (a_, k_), g_, w_ in zip(REG, got, want)
                         for j_ in range(k_) if g_[j_] != w_[j_]][:8]
                print(f"MISMATCH ledger {kind} case {i}: (addr, original, mirror) {diff_}\n  calls {got_log} / {log}")
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    for a_, st_ in lifted.items():
        e.stubs[rt(a_)] = st_
    if PORT:
        port_lib.LEDGER = False
if direct("pay"):   # morning pay: PayMercs? 0x44740 and its parts (money carried, merc leaves, team compaction, stash)
    PAY_STUBS = {"cash_drop": (0x470B0, "i"), "stash_put": (0x51CB0, "bbb"), "face_clear": (0x46AC4, "b"),
                 "w_21b3c": (0x21B3C, ""), "quote": (0x24F08, "iiiixiii"), "w_467e4": (0x467E4, "iii"),
                 "balloon": (0x50908, "siii"), "w_28698": (0x28698, "i"), "bass_map": (0x4420C, ""),
                 "w_67bf0": (0x67BF0, ""), "w_46bf4": (0x46BF4, ""), "w_673ac": (0x673AC, ""),
                 "load_face": (0x286C4, "bb")}
    pay_callees = {c_: CALLEES.get(c_) for c_ in PAY_STUBS}
    pay_saved = {a_: e.stubs.pop(rt(a_), None) for a_ in (0x3FC04, 0x4458C, 0x445DC, 0x50618, 0x50750, 0x50790,
                                                         0x51D50, 0x25B84, 0x51210, 0x51068, 0x5113C, 0x44740, 0x53708)}
    pay_old = {c_: e.stubs.get(rt(a_)) for c_, (a_, k_) in PAY_STUBS.items()}
    for c_, (a_, k_) in PAY_STUBS.items():
        CALLEES[c_] = (a_, k_)
        if PORT and c_ == "cash_drop":     # --port: the real DropItem (a 12-byte clear; the port clears the item itself)
            e.stubs.pop(rt(a_), None)
            continue
        e.stub(rt(a_), make_stub(c_, k_), 0x10 if c_ == "quote" else 0)
    if PORT:
        port_lib.PAY = True

    def stash_put_(emu_, eax_, edx_, ebx_, ecx_):    # 0x51CB0: the record goes to the stash list (not kept), counted
        log.append(("stash_put", (eax_ & 0xFF, edx_ & 0xFF, ebx_ & 0xFF)))
        v_ = take("stash_put")
        if v_ & 0xFF:
            emu_.w8(rt(ai.G_STASH_COUNTS) + (eax_ & 0xFF), emu_.r8(rt(ai.G_STASH_COUNTS) + (eax_ & 0xFF)) + 1)
        return v_
    e.stub(rt(0x51CB0), stash_put_, 0)
    REG = [(G_SOLDIERS, 0x200 * 8), (ai.G_PROFILES, 64 * ai.PROFILE_SIZE), (ai.G_STASH_COUNTS, 136),
           (ai.G_STAT_PROGRESS, 8 * 0x12), (0x229FA0, 0xA8), (0x92F8B, 1), (ai.G_PREV_BALANCE, 4)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    prng = random.Random(int(os.environ.get("AI_DIFF_SEED", "6")) * 7919 + 5)   # own sequence: later sections keep theirs
    listed_ = [e.r8(rt(ai.G_LISTED_ITEMS) + j_) for j_ in range(32)]
    fire_tramp_, fire_cell_ = e.alloc(32), e.alloc(16)   # fire_yes: AssignScreen? 0x4904A..0x49095 with EBP = S,
    e.uc.mem_write(fire_tramp_, b"\x8B\x2D" + struct.pack("<I", fire_cell_) + b"\x81\xEC\xA0\x00\x00\x00"   # [esp +
                   + b"\xA1" + struct.pack("<I", fire_cell_ + 4) + b"\x89\x84\x24\x98\x00\x00\x00"    # 0x98] = x
                   + b"\xE9" + struct.pack("<i", rt(0x4904A) - (fire_tramp_ + 29)))     # (written once: code cache)

    def fire_stop_(emu_, *_):          # LoadPanels 0x46EC4 ends the fire_yes range (returns to the sentinel)
        log.append(("load_panels", ()))
        emu_.w32(emu_.uc.reg_read(UC_X86_REG_ESP), emu.RET_SENTINEL)
    e.stub(rt(0x46EC4), fire_stop_)
    PAY_KINDS = {"cash": 0x4458C, "collect": 0x445DC, "stash": 0x51D50, "leaves": 0x50618, "compact": 0x50750,
                 "pay": 0x44740, "fire": 0x5097C, "fire_yes": None, "join": 0x21FC0}
    for i in range(max(n // 4, 100)):
        kind = prng.choice(list(PAY_KINDS))
        nteam = prng.randrange(9)
        for k in range(8):
            s_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(s_ + 0x182, 1 if k < nteam else prng.choice([0, 0, 0, 1]))
            e.w8(s_ + 0x183, prng.choice([0, 1, 40, 99, prng.randrange(256)]))
            e.w8(s_ + 0x19C, prng.choice([0, 0, 1, 7, 2, prng.randrange(256)]))
            e.w8(s_ + 0x179, prng.choice([prng.randrange(64), prng.randrange(64), 0x3C, 0x3F]))
            e.w8(s_ + 0x17A, k)
            e.w16(s_ + 0x110, prng.choice([0, 1, 10, 50, 100, 300, 1500, prng.randrange(0x10000)]))
            for o_ in (0x184, 0x18B, 0x191, 0x18D, 0x18E, 0x190, 0x18C, 0x18F, 0x192, 0x17E, 0x18A, 0x185, 0x19D):
                e.w8(s_ + o_, prng.randrange(256))
            for j_ in range(12):
                r_ = s_ + 12 * j_
                t_ = prng.choice([0, 0, ai.CASH, ai.CASH, prng.randrange(1, 136), prng.choice(listed_)])
                e.w8(r_, t_)
                e.w8(r_ + 1, prng.choice([0, 1, 2, 3, 5, prng.randrange(256)]))
                if 5 < e.r8(r_ + 1) < 0x80:   # stacks above 5 read past the item: in merc_leaves the stack beyond its
                    e.w8(r_ + 1, 5)           # copy (undefined), from place 11 the animation pointer +0x90 (an offset in the port world)
                for b_ in range(2, 12):
                    e.w8(r_ + b_, prng.choice([0, 1, 9, 50, 100, 127, prng.randrange(256)]))
                if prng.random() < 0.5:
                    e.w8(r_ + 3, prng.choice([0, prng.choice(listed_), prng.randrange(136)]))
        for t_ in range(136):
            e.w8(rt(ai.G_STASH_COUNTS) + t_, prng.choice([0, 0, 1, 5, 97, 98, 99, prng.randrange(256)]))
        for p_ in range(64):
            b_ = rt(ai.G_PROFILES) + ai.PROFILE_SIZE * p_
            e.w8(b_ + 0x3A, prng.choice([0x63, 3, 5]))
            e.w8(b_ + 0x3C, prng.choice([0x63, 3, 5]))
        for a_ in range(0x229FA0, 0x22A048, 4):
            if prng.random() < 0.3:
                e.w32(rt(a_), prng.choice([0, 1, 500, -5000 & 0xFFFFFFFF, prng.randrange(1 << 32)]))
        e.uc.mem_write(rt(ai.G_STAT_PROGRESS), bytes(prng.randrange(256) for _ in range(8 * 0x12)))
        slot_ = prng.randrange(8)
        if kind in ("fire", "fire_yes"):   # strategic.md Firing a merc: hire day, grudge bytes, subtitles
            day_ = prng.choice([1, 2, 5, 30, 200, prng.randrange(1 << 32)])
            e.w32(rt(ai.G_DAY), day_)
            for k in range(8):
                e.w8(rt(G_SOLDIERS) + 0x200 * k + 0x1E5, prng.choice([day_, day_, day_ - 1, day_ - 2, day_ - 9, day_ + 1,
                                                                     prng.randrange(256)]))
            e.w8(rt(G_SOLDIERS) + 0x200 * slot_ + 0x19D, prng.choice([0, 0, 0, 1]))
            e.w8(rt(G_SOLDIERS) + 0x200 * slot_ + 0x179, prng.choice([prng.randrange(60)] * 4 + [60, 63, 0x63, 7]))
            e.w8(rt(G_SOLDIERS) + 0x200 * slot_ + 0x17A, prng.choice([slot_] * 5 + [prng.randrange(8)]))
            for p_ in range(64):
                b_ = rt(ai.G_PROFILES) + ai.PROFILE_SIZE * p_
                e.w8(b_ + 0x4C, prng.choice([0x63, 0x63, 0x63, 3, prng.randrange(256)]))
                e.w8(b_ + 0x2F, prng.choice([0, 1, 3, prng.randrange(256)]))
                e.w8(b_ + 0x3C, prng.choice([0x63, 0x63, prng.randrange(64)]))
                e.w8(b_ + 0x3D, prng.randrange(2))
            e.w8(rt(0x1163BD), prng.choice([0, 1]))
        if kind == "join":    # strategic.md A merc joins the team: the profile's nickname, stats, status, level, kit
            jp_ = prng.randrange(64)
            b_ = rt(ai.G_PROFILES) + ai.PROFILE_SIZE * jp_
            nick_ = bytes(prng.randrange(1, 256) for _ in range(prng.randrange(10)))
            e.uc.mem_write(b_, nick_ + b"\0")
            for o_ in (0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x40, 0x41, 0x42, 0x43):
                e.w8(b_ + o_, prng.choice([0, 1, 50, 99, 100, 101, prng.randrange(256)]))
            e.w8(b_ + 0x44, prng.choice([0, 1, 1, 2, 5, 9, 10, 11, 0x80, prng.randrange(256)]))
            e.w8(b_ + 0x4A, prng.choice([0, 1, 1, 2, 2, 3, prng.randrange(256)]))
            for j_ in range(10):
                e.w8(b_ + 0x50 + j_, prng.choice([0, 0, 1, 0x6F, prng.randrange(1, 136), prng.choice(listed_)]))
            e.w8(rt(G_SOLDIERS) + 0x200 * 7 + 0x182, 0)   # the callers never join an 8th merc (k = 8 writes record 8)
            for k in range(8):     # stale bytes in the record that the merc gets (inCurSector keeps the team size)
                s_ = rt(G_SOLDIERS) + 0x200 * k
                if not e.r8(s_ + 0x182):
                    e.uc.mem_write(s_ + 0x100, bytes(prng.choice([0, prng.randrange(256)]) for _ in range(0x82)))
                    e.uc.mem_write(s_ + 0x183, bytes(prng.choice([0, prng.randrange(256)]) for _ in range(0x60)))
                    e.w8(s_ + 0x192, prng.choice([1, 1, 9, 10, prng.randrange(256)]))
        arg_ = {"join": (jp_,) if kind == "join" else (),"cash": (rt(G_SOLDIERS) + 0x200 * slot_, prng.choice([0, 1, 0x100])),
                "stash": (rt(G_SOLDIERS) + 0x200 * slot_ + 12 * prng.randrange(12), 0),
                "leaves": (rt(G_SOLDIERS) + 0x200 * slot_, prng.choice([1, 0xE, prng.randrange(256)])),
                "compact": (prng.choice([0, 1, 0x100]),),
                "fire": (slot_, prng.choice([11 + 37 * slot_, prng.randrange(320)]), prng.choice([0x14, 0])),
                "fire_yes": (slot_, prng.choice([11 + 37 * slot_, prng.randrange(320)]))}.get(kind, ())
        rands_ = [prng.choice([0, 0, 1, 2, 7, prng.randrange(60), prng.randrange(1 << 16)]) for _ in range(12)]
        puts_ = [prng.choice([1, 1, 1, 0]) for _ in range(40)]
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]

        def fresh():
            return {**{c_: [] for c_ in PAY_STUBS}, "stash_put": list(puts_), "blit": [], "message": [],
                    "Random": list(rands_), "load_panels": [],
                    "w_36010": [], "w_67bf0": [], "w_46bf4": [], "w_673ac": []}
        script = fresh()
        log.clear()
        sc0_ = e.r32(rt(0x93E64))   # fuzzed attachments >= 136 count into the scratch pointer after g_stashCounts
        if kind == "fire_yes":
            e.w32(fire_cell_, rt(G_SOLDIERS) + 0x200 * slot_)
            e.w32(fire_cell_ + 4, arg_[1])
            ret_ = e.call(fire_tramp_, max_steps=20_000_000)
        else:
            ret_ = e.call(rt(PAY_KINDS[kind]), *arg_, max_steps=20_000_000)
        e.w32(rt(0x93E64), sc0_)
        got, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = fresh()
        log.clear()
        w_ = World(0)
        larg_ = [a_ - (rt(G_SOLDIERS) - G_SOLDIERS) if a_ >= rt(G_SOLDIERS) else a_ for a_ in arg_]    # soldier / item pointers -> linear
        place_ = (arg_[0] - rt(G_SOLDIERS) - 0x200 * slot_) // 12 if kind == "stash" else 0
        want_r = {"cash": lambda: port_world("cash_count", (slot_, arg_[1]), sync=True),
                  "collect": lambda: port_world("collect_cash", (), sync=True),
                  "stash": lambda: port_world("stash_item", (slot_, place_), sync=True),
                  "leaves": lambda: port_world("merc_leaves", (slot_, arg_[1]), sync=True),
                  "compact": lambda: port_world("team_compact", (arg_[0],), sync=True),
                  "pay": lambda: port_world("pay_mercs", (), sync=True),
                  "fire": lambda: port_world("fire_merc_core", arg_, sync=True),
                  "fire_yes": lambda: port_world("fire_merc", arg_, sync=True),
                  "join": lambda: port_world("merc_join", arg_, sync=True)}[kind]() if PORT else {
                  "cash": lambda: ai.cash_count(w_, *larg_), "collect": lambda: ai.collect_cash(w_),
                  "stash": lambda: ai.stash_item(w_, larg_[0]), "leaves": lambda: ai.merc_leaves(w_, *larg_),
                  "compact": lambda: ai.team_compact(w_, *larg_), "pay": lambda: ai.pay_mercs(w_),
                  "fire": lambda: ai.fire_merc(w_, *arg_),
                  "fire_yes": lambda: ai.fire_yes(w_, G_SOLDIERS + 0x200 * slot_, arg_[1]),
                  "join": lambda: ai.merc_join(w_, *arg_)}[kind]()
        if kind == "join":    # the host part: LoadMercFace (k, p)
            log.append(("load_face", (ai._i32(want_r) & 0xFF, arg_[0])))
        if kind == "fire_yes":
            log.append(("load_panels", ()))
        e.w32(rt(0x93E64), sc0_)
        want = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        ok_ = (got, got_log) == (want, log) and (kind != "cash" or ai._i32(ret_) == want_r)
        counts["pay"] = counts.get("pay", 0) + 1
        results.setdefault("pay", {}).setdefault(kind, 0)
        results["pay"][kind] += 1
        if not ok_:
            fails += 1
            if fails <= 10:
                diff_ = [(hex(a_ + j_), g_[j_], w2_[j_]) for (a_, k_), g_, w2_ in zip(REG, got, want)
                         for j_ in range(k_) if g_[j_] != w2_[j_]][:8]
                print(f"MISMATCH pay {kind} case {i}: ret {ai._i32(ret_)} / {want_r}, (addr, original, mirror) "
                      f"{diff_}\n  calls {got_log[:6]} / {log[:6]}")
                cd_ = next((j_ for j_ in range(max(len(got_log), len(log)))
                            if got_log[j_:j_ + 1] != log[j_:j_ + 1]), None)
                if cd_ is not None:
                    print(f"  first call diff at {cd_} (of {len(got_log)} / {len(log)}): "
                          f"{got_log[cd_:cd_ + 3]} / {log[cd_:cd_ + 3]}")
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    for c_, (a_, k_) in PAY_STUBS.items():
        if pay_callees[c_]:
            CALLEES[c_] = pay_callees[c_]
        else:
            del CALLEES[c_]
        if pay_old[c_]:
            e.stubs[rt(a_)] = pay_old[c_]
        else:
            e.stubs.pop(rt(a_), None)
    if PORT:
        port_lib.PAY = False
    for a_, st_ in pay_saved.items():
        if st_:
            e.stubs[rt(a_)] = st_
if direct("pickup"):   # ground items to the cursor / away: six routines called directly, stubs lifted
    class PickWorld(World):
        def ext(self, name, *args):          # the routines under test call each other for real on both sides
            if name == "w_3fc04":
                return ai.listed_item(self, *args)
            v = super().ext(name, *args)
            if name == "w_54ec8" and empties:   # the cursor item went to the soldier
                self.s8(ai.G_HELD, 0)
            return v
    lifted = {a_: e.stubs.pop(rt(a_)) for a_ in (0x72568, 0x3FC04, 0x3C05C, 0x19EB0, 0x3C16C) if rt(a_) in e.stubs}
    cursor_stub = e.stubs[rt(0x54EC8)]

    def to_soldier(emu_, *regs):
        v = cursor_stub[0](emu_, *regs)
        if empties:
            e.w8(rt(ai.G_HELD), 0)
        return v
    e.stubs[rt(0x54EC8)] = (to_soldier, cursor_stub[1])
    REG = [(G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (ai.G_GROUND_ITEMS, 12 * 256), (ai.G_HELD, 0x24),
           (ai.G_LISTED_ITEMS, 32), (0x22A05A, 4), (ai.G_BLAST_LIST, 60), (ai.G_CALL_E, 3),
           (G_SOLDIERS, 0x200 * 8), (0x94DF8, 12 * 256), (ai.G_MOVECODE, 3136 * 2)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    STORY = [0x79, 0x86, 0x5F, 0x6E]
    for i in range(n // 3):
        kind = rng.choice(["call", "listed", "take", "take", "flood", "flood", "find", "find", "pick", "pick", "pick"])
        if PORT:                # the port has take (ja1_take_ground_item), flood (ja1_take_ground_patch), pick (ja1_pick_up)
            kind = rng.choice(["take", "flood", "pick"])
        types_ = STORY + [0, 0x6F, 0x57, 0x58, 0x11, 0x2C] + [rng.randrange(1, 136) for _ in range(3)]
        typ = rng.choice(types_)
        for k in range(32):
            e.w8(rt(ai.G_LISTED_ITEMS) + k, rng.choice([0, 0, 0, rng.choice(types_)]) if rng.random() < 0.5 else 0)
        for t_ in types_:
            e.w8(rt(ai.G_ITEM_KEEPS) + 12 * t_, rng.choice([0, 0, 1, 0x80]))
        tile = rng.choice([rng.randrange(3136)] * 6 + [0, 55, 3135, 3080, 0xC40, 0xC41, -1, 0x1C41, 0x10064, 0xFFFF0050])
        t0 = ai.s16(tile & 0xFFFF) if 0 <= ai.s16(tile & 0xFFFF) < 3136 else 100
        cells = [t0] + [t0 + d for d in ai.TILE_STEPS4 if 0 <= t0 + d < 3136]
        cells += [c + d for c in cells[1:] for d in ai.TILE_STEPS4 if 0 <= c + d < 3136 and rng.random() < 0.4]
        for c in cells:
            gi_ = rng.choice([rng.randrange(200), rng.randrange(200), rng.randrange(200), 0xFF, rng.randrange(200, 255)])
            e.w8(rt(G_TILEMAP) + 8 * c + 6, gi_)
            e.w8(rt(G_TILEMAP) + 8 * c + 1, rng.choice([0x86, 0, 0, 0x10]))
            if gi_ != 0xFF:
                g_ = rt(ai.G_GROUND_ITEMS) + 12 * gi_
                e.uc.mem_write(g_, bytes(rng.randrange(256) for _ in range(12)))
                e.w8(g_, typ if rng.random() < 0.6 else rng.choice(types_))
        for k in range(30):
            e.w16(rt(ai.G_BLAST_LIST) + 2 * k, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.choice(cells), rng.randrange(3136)]))
        e.uc.mem_write(rt(ai.G_HELD), bytes(rng.randrange(256) for _ in range(0x24)))
        e.w8(rt(ai.G_HELD), rng.choice([0, typ, rng.choice(types_)]))
        for k in range(4):
            e.w8(rt(0x22A05A) + k, rng.choice([0, 0, 1]))
        carry = rng.choice([0, 0.05]) if PORT else 0.05    # --port: half the cases nobody carries typ (squad search fails)
        for k in range(8):
            r_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(r_ + 0x182, rng.choice([0, 1, 1, 0x80]))
            for j in range(12):
                e.w8(r_ + 12 * j, typ if rng.random() < carry else rng.choice([0, rng.choice(types_)]))
        slot = rng.randrange(8)
        sp_ = rt(G_SOLDIERS) + 0x200 * slot
        e.w8(sp_ + 0x17A, slot)
        drop, who, probe = rng.choice([0, 1, 0x100, 0x80, 0xFF]), rng.choice([0x40, 8, rng.randrange(8), rng.randrange(256)]), \
            rng.choice([0, 0, 1, 0x100])
        keep, cursor = rng.choice([0, 1, 1, 0x100]), rng.choice([0, 1, 1, 0x100])
        empties = rng.random() < 0.5
        if PORT:          # off the map byte 6 reads 0xFF for the port (sector_start.md batch 4): make it so here too
            for t_ in list(range(-64, 0)) + list(range(3136, 3136 + 128)):
                e.w8(rt(G_TILEMAP) + 8 * t_ + 6, 0xFF)
        base_ = {"w_51a24": [rng.choice([0, 0, 1, 0x100])], "w_53660": [0] * 4,
                 "w_54ec8": [0] * 4, "drop_item": [0] * 8, "quote_event": [0] * 4}
        arg = {"call": (0x72568, (tile,)), "listed": (0x3FC04, (typ | rng.choice([0, 0x100]),)),
               "take": (0x3C05C, (tile, keep)), "flood": (0x19EB0, (tile, typ)),
               "find": (0x61528, (typ | rng.choice([0, 0, 0x100]), drop, who, probe)),
               "pick": (0x3C16C, (sp_, tile, cursor))}[kind]
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        script = {k_: list(v) for k_, v in base_.items()}
        log.clear()
        try:
            got_r = e.call(rt(arg[0]), *[a_ & 0xFFFFFFFF for a_ in arg[1]], max_steps=300_000)
        except emu.EmuStop:               # flood over a listed type recurses until the stack runs out
            hangs["pickup"] = hangs.get("pickup", 0) + 1
            for (a_, k_), b_ in zip(REG, snap):
                e.uc.mem_write(rt(a_), b_)
            continue
        got, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v) for k_, v in base_.items()}
        log.clear()
        w_ = PickWorld(slot)
        rec_ = bytes(e.uc.mem_read(sp_, 0x200))
        if PORT:
            if kind == "pick":
                PICK_EMPTIES[0] = empties
                port_sector("pick_up", (slot, ai._i32(tile), cursor), allow=("w_54ec8", "quote_event"))
            else:
                port_sector("take_ground_item" if kind == "take" else "take_ground_patch", tuple(ai._i32(a_) for a_ in arg[1]))
        want_r = None if PORT else {"call": lambda: ai.call_enemies(w_, tile), "listed": lambda: ai.listed_item(w_, arg[1][0]),
                  "take": lambda: ai.take_ground_item(w_, tile, keep),
                  "flood": lambda: ai.take_ground_items_flood(w_, tile, typ),
                  "find": lambda: ai.find_squad_item(w_, *arg[1]),
                  "pick": lambda: ai.pick_up(rec_, w_, tile, cursor)}[kind]()
        want = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        mask = {"listed": 0xFF, "find": 0xFFFFFFFF}.get(kind)
        got_r, want_r = (got_r & mask, (want_r or 0) & mask) if mask else (0, 0)
        counts["pickup"] = counts.get("pickup", 0) + 1
        results.setdefault("pickup", {}).setdefault(kind, 0)
        results["pickup"][kind] += 1
        if (got_r, got, got_log) != (want_r, want, log):
            fails += 1
            if fails <= 10:
                bad = [hex(REG[k][0]) for k in range(len(REG)) if got[k] != want[k]]
                print(f"MISMATCH pickup {kind}{arg[1]}: ret {got_r:#x} / {want_r:#x}, regions differ {bad}"
                      f"\n  calls original {got_log}\n  calls port     {log}")
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    for a_, st_ in lifted.items():
        e.stubs[rt(a_)] = st_
    e.stubs[rt(0x54EC8)] = cursor_stub
if direct("stage"):   # home-sector event stage effects: called directly with the stub lifted
    stub = e.stubs.pop(rt(0x635DC))
    REG = [(ai.G_STAGE_FX, 84), (ai.G_STAGE, 1), (ai.G_DIFFICULTY, 1), (ai.G_LOSS_TIMER, 4), (ai.G_LOSS_SLOT, 1),
           (ai.G_LOSS_AMOUNT, 1)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    for i in range(n // 6):
        stage, diff = rng.choice([0, 0, 1, 2, 3, 4, 0xFF]), rng.choice([0, 1, 2, 0xFF, 0x80])
        e.w8(rt(ai.G_STAGE), stage)
        e.w8(rt(ai.G_DIFFICULTY), diff)
        for k in range(12):
            r_ = rt(ai.G_STAGE_FX) + 7 * k
            e.uc.mem_write(r_, bytes(rng.randrange(256) for _ in range(7)))
            e.w8(r_ + 3, rng.choice([stage, stage, rng.randrange(6)]))
            e.w8(r_ + 6, rng.choice([0, 1, 2, 3, 0x80, 0xFF]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        script = {"fx_157bc": [0] * 12}
        log.clear()
        e.call(rt(0x635DC))
        got, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {"fx_157bc": [0] * 12}
        log.clear()
        ai.stage_effects(World(0))
        want = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["stage"] = counts.get("stage", 0) + 1
        results.setdefault("stage", {}).setdefault(min(len(log), 5), 0)
        results["stage"][min(len(log), 5)] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH stage {stage} diff {diff}: regions differ "
                      f"{[hex(REG[k][0]) for k in range(len(REG)) if got[k] != want[k]]}"
                      f"\n  calls original {got_log}\n  calls port     {log}")
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.stubs[rt(0x635DC)] = stub
if direct("age"):   # knowledge ageing at a turn change: called directly
    REG = [(ai.G_KNOWN, 0x30), (ai.G_CALL_M, 3), (ai.G_CALL_E, 3), (G_SOLDIERS, 0x200 * 0x30)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    for i in range(n // 6):
        for k in range(0x30):
            e.w8(rt(ai.G_KNOWN) + k, rng.choice([0, 1, 2, 3, 4, 5, 0x7F, 0x80, 0xFF, 0xFE, 0xFD, 0xFC, rng.randrange(256)]))
            r_ = rt(G_SOLDIERS) + 0x200 * k
            for f_ in (0x182, 0x138, 0x183):
                e.w8(r_ + f_, rng.choice([1, 1, 1, 0, 0x80]))
            e.w8(r_ + 0x17A, rng.choice([k, k, k, rng.randrange(0x30)]))
        for c_ in (ai.G_CALL_M, ai.G_CALL_E):
            e.w16(rt(c_), rng.randrange(3136))
            e.w8(rt(c_) + 2, rng.choice([0, 1, 1, 2, 0xC, 0xFF]))
        side = rng.choice([0, 1, 0x100, 0xFF])
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        e.call(rt(0x741F8), side)
        got = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        ai.age_knowledge(World(0), side)
        want = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["age"] = counts.get("age", 0) + 1
        results.setdefault("age", {}).setdefault(sum(x != y for x, y in zip(got[0], snap[0])), 0)
        results["age"][sum(x != y for x, y in zip(got[0], snap[0]))] += 1
        if got != want:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH age side {side}: regions differ {[hex(REG[k][0]) for k in range(len(REG)) if got[k] != want[k]]}")
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
if direct("story"):   # a story object is hit: called directly with the stub lifted
    stub = e.stubs.pop(rt(0x61D28))
    REG = [(G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (ai.G_SECTORS, 0xB1 * 60), (0x22A055, 1),
           (ai.G_MISSION_STEP, 1), (ai.G_MISSION_ORDER, 1)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG] + [bytes(e.uc.mem_read(rt(ai.G_CUR_SECTOR), 1))]
    for i in range(n // 12):
        sec = rng.randrange(60)
        e.w8(rt(ai.G_CUR_SECTOR), sec)
        e.w8(rt(ai.G_MISSION_ORDER), rng.randrange(256))
        e.w8(rt(0x22A055), rng.choice([0, 1, 2]))
        rb = rt(ai.G_SECTORS) + 0xB1 * sec
        for k in range(5):
            e.w8(rb + 0x9F + k, rng.choice([0xAE, 0xAF, 0xAC, 0xAD, 0, 0xAB, rng.randrange(256)]))
            e.w16(rb + 0x82 + 2 * k, rng.choice([rng.randrange(3136), rng.randrange(3136), 0, 3135]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"radio_result": [rng.choice([0, 1, 2, 0xFF, 0x100])]}
        script = {k_: list(v) for k_, v in base_.items()}
        log.clear()
        e.call(rt(0x61D28))
        got, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v) for k_, v in base_.items()}
        log.clear()
        ai.story_hit(World(0))
        want = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["story"] = counts.get("story", 0) + 1
        ch = sum(x != y for x, y in zip(got[0], snap[0]))
        results.setdefault("story", {}).setdefault(min(ch, 5), 0)
        results["story"][min(ch, 5)] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH story sector {sec}: regions differ "
                      f"{[hex(REG[k][0]) for k in range(len(REG)) if got[k] != want[k]]}"
                      f"\n  calls original {got_log}\n  calls port     {log}")
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG + [(ai.G_CUR_SECTOR, 1)], tsave):
        e.uc.mem_write(rt(a_), b_)
    e.stubs[rt(0x61D28)] = stub
if direct("forget", "stragglers"):   # a soldier leaves the board (0x3A148 with 0x74BD4), stage-5 enemies die (0x74C44): called directly
    REG = [(ai.G_KNOWN, 0x30), (ai.G_REMEMBER_E, 0x1C0), (ai.G_LAST_SEEN_E, 0x500), (ai.G_LAST_SEEN_M, 0x280),
           (ai.G_LAST_SPOTTER, 1), (G_SOLDIERS, 0x200 * 0x30)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    real_dies = ai.merc_dies
    for i in range(n // 3):
        mode = only or ("forget", "stragglers")[i % 2]
        target = rng.choice([rng.randrange(16), rng.randrange(16, 0x28), rng.randrange(0x28, 0x30)])
        nm_ = [target, rng.randrange(0x30), 0x40]
        for k in range(0x30):
            r_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(r_ + 0x17A, rng.choice([k] * 8 + [rng.randrange(0x30)]))
            for f_ in (0x1D8, 0x1F7, 0x1C2):
                e.w8(r_ + f_, rng.choice(nm_ + [rng.randrange(256)]))
            e.w8(r_ + 0x1D5, rng.choice([0, 1, 2, 0x80, rng.randrange(256)]))
            e.w8(r_ + 0xBA, rng.choice([2, 2, 0, 1, 3, 0x82, rng.randrange(256)]))
            e.uc.mem_write(r_ + 0x112, bytes(rng.choice([0, 0, 1, 1, 2, 0xFF, 0x81]) for _ in range(0x50)))
            e.w16(r_ + 0x108, rng.randrange(0x10000))
            for f_ in (0x182, 0x138):
                e.w8(r_ + f_, rng.choice([1, 1, 1, 0, 0x80]))
            e.w8(r_ + 0x183, rng.choice([15, 14, 50, 0, 0x80, 0x7F, rng.randrange(256)]))
            e.w8(r_ + 0x9C, rng.choice([0, 0, 1, 0x80, 0xFF, rng.randrange(256)]))
            e.w8(r_ + 0x1C4, rng.choice([0, 0, 0, 1, 0x80]))
        for a_, k_ in REG[:4]:
            e.uc.mem_write(rt(a_), bytes(rng.randrange(256) for _ in range(k_)))
        e.w8(rt(ai.G_LAST_SPOTTER), rng.choice([target, 0x40, rng.randrange(0x28, 0x30)]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        log.clear()
        if mode == "forget":
            e.call(rt(0x3A148), rt(G_SOLDIERS) + 0x200 * target)
        else:
            script = {"merc_dies": [0] * 8}
            e.call(rt(0x74C44))
        got, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        log.clear()
        w_ = World(0)
        w_.memrun = True
        if mode == "forget":
            ai.forget_soldier(MemRec(target), w_)
        else:
            script = {"merc_dies": [0] * 8}
            ai.merc_dies = lambda s_, w2, q: w2.ext("merc_dies", s_, q)
            try:
                ai.kill_stragglers(w_)
            finally:
                ai.merc_dies = real_dies
        want = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        key = len(log) if mode == "stragglers" else sum(x != y for x, y in zip(got[0], snap[0]))
        counts[mode] = counts.get(mode, 0) + 1
        results.setdefault(mode, {}).setdefault(key, 0)
        results[mode][key] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH {mode} target {target:#x}: regions differ "
                      f"{[hex(REG[k][0]) for k in range(len(REG)) if got[k] != want[k]]}"
                      f"\n  calls original {got_log}\n  calls port     {log}")
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
if direct("crowbar"):   # UseCrowbar? 0x3B474: lock or door strength against force, the bar may break: called directly
    class CrowWorld(World):
        def ext(self, name, *args):
            return ai.door_lookup(self, *args) if name == "door_lookup" else World.ext(self, name, *args)
    door_stub_ = e.stubs.pop(rt(0x3AFE0))
    sec_ = e.r8(rt(ai.G_CUR_SECTOR))
    REC_ = (ai.G_SECTORS + 0xB1 * sec_, 0x60)
    tsave = bytes(e.uc.mem_read(rt(REC_[0]), REC_[1]))
    tiles_ = [t_ for t_ in range(0, 3136, 7)]
    blocked_ = [t_ for t_ in tiles_ if ai._i32(ai.tile_standable(World(0), t_)) < 0]
    free_ = [t_ for t_ in tiles_ if ai._i32(ai.tile_standable(World(0), t_)) >= 0]
    for i in range(n // 3):
        slot = rng.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        s[0x183] = rng.choice([0, 1, 15, 40, 60, 100, 0x7F, 0x80, rng.randrange(256)])
        s[0x184] = rng.choice([s[0x183], s[0x183], 100, 60, 0x80, rng.randrange(256)])
        s[0x185] = rng.choice([0, 0, 5, 20, 0x80, rng.randrange(256)])
        s[0x18A] = rng.choice([0, 30, 60, 80, 99, 100, 0x7F, 0x80, rng.randrange(256)])
        s[0] = rng.choice([0x76, 0x76, 0x75, rng.randrange(136)])
        s[2] = rng.choice([0, 9, 10, 11, 25, 50, 100, 0x7F, 0x80, 0xFF, rng.randrange(256)])
        tile = rng.choice(blocked_ + free_ if rng.random() < 0.2 else (blocked_, free_)[i % 2])
        rec_ = bytearray(rng.randrange(256) for _ in range(REC_[1]))
        for k_ in range(24):                 # door pairs 0..11, lock pairs 12..23 (+0x30)
            if rng.random() < 0.15:
                ai.w16(rec_, 4 * k_, tile & 0xFFFF)
                ai.w16(rec_, 4 * k_ + 2, rng.choice([0, 1, 5, 10, 20, 35, 50, 90, 200, 0x4000, 0x7FFF, 0x8000, 0x9000, 0xC000, 0xFFFF, 0xFFCE, rng.randrange(0x10000)]))
        e.uc.mem_write(rt(REC_[0]), bytes(rec_))
        e.w8(rt(0x284F11), rng.choice([slot, 0x40, (slot + 1) % 0x30]))
        flag = rng.choice([0x76, 0x76, 0x76, 0, 0x77, 0x176])
        e.uc.mem_write(sp, bytes(s))
        base_ = {"Random": [rng.choice([0, 1, 5, 9, 10, 49, 50, 99, rng.randrange(0x7FFF)]) for _ in range(4)],
                 **{k_: [0] * 4 for k_ in ("spend_ap", "stat_practice", "focus", "cry", "voc", "message", "item_panel")}}
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        try:
            got = ai._i32(e.call(rt(0x3B474), sp, tile, flag))
        except emu.EmuStop as x:
            at = int(str(x).split(" at 0x")[1].split()[0], 16)
            if next(e.md.disasm(bytes(e.uc.mem_read(at, 8)), at)).mnemonic not in ("div", "idiv"):
                raise
            got, log[:] = "divide by zero", []
        got_s, got_log = bytes(e.uc.mem_read(sp, 0x200)), list(log)
        e.uc.mem_write(sp, bytes(s))
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        ps = bytearray(s)
        try:
            want = ai._i32(ai.use_crowbar(ps, CrowWorld(slot), tile, flag))
        except ZeroDivisionError:
            want, log[:] = "divide by zero", []
        counts["crowbar"] = counts.get("crowbar", 0) + 1
        key_ = (want, len([c_ for c_ in log if c_[0] == "message"]))
        results.setdefault("crowbar", {}).setdefault(key_, 0)
        results["crowbar"][key_] += 1
        if (got, got_s, got_log) != (want, bytes(ps), log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH crowbar slot {slot:#x} tile {tile:#x} flag {flag:#x}: {got} / {want}; record "
                      f"{[hex(o) for o in range(0x200) if got_s[o] != ps[o]]}\n  calls original {got_log}\n  calls port     {log}")
        e.uc.mem_write(sp, bytes(s))
    e.uc.mem_write(rt(REC_[0]), tsave)
    e.stubs[rt(0x3AFE0)] = door_stub_
if direct("disarm"):   # DisarmExplosivePrompt 0x3B9D8: spot, ask, disarm or set off: called directly
    # (#12) the unit conditions of a big stack in entry 199 run past the table into g_textBuf in the original; the
    # port writes them there too (combat.md Mine spotted, "G for g of 200 or more")
    REG = [(ai.G_GROUND_ITEMS, 12 * 256 + 0x90), (ai.G_BLAST_LIST, 60), (ai.ITEM_TBL, 12 * 136), (G_TILEMAP, 8 * 3136),
           (ai.G_MOVECODE, 2 * 3136)]   # the move codes: a removed blue flag rebuilds them
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    TYPES_ = [0x11, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x28, 0x29, 0x2A, 0x2C,
              0x3E, 0x3F, 0x40, 0x70, 0x71, 0x72, 0x73]
    for i in range(n // 3):
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8), rng.randrange(8, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        for f_ in (0x18C, 0x18D, 0x18F, 0x192):
            s[f_] = rng.choice([0, 1, 5, 10, 29, 30, 60, 90, 100, 0x7F, 0x80, rng.randrange(256)])
        for f_ in (0x95, 0x1A6, 0x1A7):
            s[f_] = rng.randrange(256)
        ai.w16(s, 0x134, rng.randrange(0x10000))
        ai.w16(s, 0x96, rng.randrange(0x10000))
        tile = rng.randrange(3136)
        k = rng.choice([rng.randrange(200), rng.randrange(256), 0xFF])
        g_ = rt(ai.G_GROUND_ITEMS) + 12 * k
        typ_ = rng.choice(TYPES_ + [rng.randrange(256)])
        if PORT:     # the port's item_types has the game's 136 entries (entries k >= 200: the port's ge_byte)
            typ_ %= 136
        e.uc.mem_write(g_, bytes(rng.randrange(256) for _ in range(12)))
        e.w8(g_, typ_)
        e.w8(g_ + 1, rng.choice([0, 1, 1, 2, 3, 5, 9, 9, 12, 0x80, rng.randrange(256)]))
        e.w8(g_ + 2, rng.choice([0x9B, 0x9C, 0x80, 0x81, 0xCE, 0xFF, 0, 1, 50, 0x7F, rng.randrange(256),
                                 rng.randrange(0xFB, 0x100), rng.randrange(0x98, 0xA2)]))   # negated: clamp bounds 1 / 100
        if typ_ < 136:
            e.w8(rt(ai.ITEM_TBL) + 12 * typ_ + 6, rng.choice([0, 1, 2, 2, 5, 0x80, 0xFF]))
        for j_ in range(30):
            e.w16(rt(ai.G_BLAST_LIST) + 2 * j_, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136)]))
        if rng.random() < 0.4:
            e.w16(rt(ai.G_BLAST_LIST) + 2 * rng.randrange(30), tile)
        mt_ = rt(G_TILEMAP) + 8 * tile + 1
        old_obj_ = e.r8(mt_)
        e.w8(mt_, rng.choice([0x86, 0x86, old_obj_, 0x85, 0x87]))
        e.w8(rt(0x92DD4), rng.randrange(256))
        armed, menu = rng.choice([0, 1, 1, 0x100]), rng.choice([0, 1, 0x100, 0xFF])
        e.uc.mem_write(sp, bytes(s))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        sx_ = lambda o_: ai.s8(s[o_])
        chance_ = (ai.cdiv(sx_(0x18F) * 70, 100) + ai.cdiv(sx_(0x18C) * 20, 100) + sx_(0x192)
                   - ai.cdiv(100 - sx_(0x18D), 5))
        rolls_ = [rng.choice([0, 1, 5, 9, 10, 30, 49, 50, 99, rng.randrange(10), rng.randrange(40), rng.randrange(0x7FFF)])
                  for _ in range(140)]
        rolls_[0] = rng.choice([chance_, chance_ - 1, chance_ + 1, rolls_[0]]) % 100
        base_ = {"Random": rolls_,
                 "yes_no": [rng.choice([0, 1, 1, 0x100]) for _ in range(2)],
                 **{k_: [0] * 4 for k_ in ("stat_practice", "merc_message", "w_1ac68", "w_53708", "w_1cb78")}}
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        got = ai._i32(e.call(rt(0x3B9D8), sp, tile, k, armed, menu))
        got_s, got_g, got_log = bytes(e.uc.mem_read(sp, 0x200)), [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        e.uc.mem_write(sp, bytes(s))
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        if PORT:
            want = ai._i32(port_sector("disarm", (slot, tile, k, armed, menu),
                                       allow=("stat_practice", "merc_message", "w_1ac68")) & 0xFFFFFFFF)
            ps = bytearray(e.uc.mem_read(sp, 0x200))
        else:
            ps = bytearray(s)
            want = ai._i32(ai.disarm_explosive(ps, World(slot), tile, k, armed, menu))
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["disarm"] = counts.get("disarm", 0) + 1
        key_ = (want, len([c_ for c_ in log if c_[0] == "yes_no"]))
        results.setdefault("disarm", {}).setdefault(key_, 0)
        results["disarm"][key_] += 1
        if (got, got_s, got_g, got_log) != (want, bytes(ps), want_g, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH disarm slot {slot:#x} tile {tile:#x} item {k:#x} type {typ_:#x}: {got} / {want}; record "
                      f"{[hex(o) for o in range(0x200) if got_s[o] != ps[o]]} regions "
                      f"{[hex(REG[j][0]) for j in range(len(REG)) if got_g[j] != want_g[j]]}"
                      f"\n  calls original {got_log}\n  calls port     {log}")
        e.uc.mem_write(sp, bytes(s))
        e.w8(mt_, old_obj_)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
if direct("mine_spotted"):   # MineSpotted? 0x3C31C (soldier, tile, ground entry): called directly with the stub lifted
    ms_stub = e.stubs.pop(rt(0x3C31C), None)
    F56 = bool(FAN and 56 in FAN)
    REG = [(ai.G_GROUND_ITEMS, 12 * 200), (ai.G_BLAST_LIST, 60)] + ([(ai.G_FX, 0x41)] if F56 else [])
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    for i in range(n // 6):
        slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(8, 0x30)])
        sp = rt(G_SOLDIERS) + slot * 0x200
        old17a = e.r8(sp + 0x17A)
        e.w8(sp + 0x17A, rng.choice([slot, slot, rng.randrange(256)]))
        tile = rng.choice([rng.randrange(3136), rng.randrange(3136), 0, 3135])
        k = rng.randrange(200)
        g_ = rt(ai.G_GROUND_ITEMS) + 12 * k
        e.uc.mem_write(g_, bytes(rng.randrange(256) for _ in range(12)))
        e.w8(g_, rng.choice([0x11, 0x11, 0x11, 0x11, 0x10, rng.randrange(256)]))
        e.w8(g_ + 6, rng.choice([0xFF, 0xFF, 0xFF, 0, 1, 0xFE]))
        e.w8(g_ + 2, rng.choice([1, 1, 50, 0x7F, 0x7F, 0, 0x80, 0xFF, rng.randrange(256)]))
        for j_ in range(30):
            e.w16(rt(ai.G_BLAST_LIST) + 2 * j_, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136), tile]))
        if F56:                          # F56: explosive types answer -FxHere?(soldier): an effect on the own tile
            e.w8(g_, F56_RNG.choice([0x11, 0x16, 0x18, 0x1A, 0x1C, 0x1D, 0x17, 0x1E, 0x15, e.r8(g_)]))
            if F56_RNG.random() < 0.6:
                e.w32(rt(ai.G_FX), F56_RNG.choice([0, 5, 0xFFFFFFFF]))
                e.w16(rt(ai.G_FX) + 6, e.r16(sp + 0x114))
            if F56_RNG.random() < 0.15:
                k = 0xFF                 # (1.15m: ground entry 0xFF answers 0; the entry is not written)
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        script = {}
        log.clear()
        got = e.call(rt(0x3C31C), sp, tile, k) & 0xFF
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        log.clear()
        if PORT:
            want = port_sector("mine_spotted", (slot, tile, k)) & 0xFF
        else:
            want = ai.mine_spotted(bytes(e.uc.mem_read(sp, 0x200)), World(slot), tile, k) & 0xFF
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["mine_spotted"] = counts.get("mine_spotted", 0) + 1
        results.setdefault("mine_spotted", {}).setdefault(want, 0)
        results["mine_spotted"][want] += 1
        if (got, got_g, got_log) != (want, want_g, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH mine_spotted slot {slot:#x} tile {tile} item {k}: {got} / {want}; regions "
                      f"{[hex(REG[j][0]) for j in range(len(REG)) if got_g[j] != want_g[j]]}\n  calls {got_log} / {log}")
        e.w8(sp + 0x17A, old17a)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    if ms_stub:
        e.stubs[rt(0x3C31C)] = ms_stub
if direct("merc_entry"):   # 0x2B470 mercs onto the entry tile and around it (sector start): called directly
    REG = [(G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (G_SOLDIERS, 0x200 * 48)]   # 48: selected can name any record
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    sel_save = e.r8(rt(ai.G_SELECTED))
    OBJ_ = [0, 0, 0, 0, 0x3B, 0x3D, 0x43, 0x5A, 0x5C, 0x62, 0x30, 0x57, 0x86, rng.randrange(256)]
    skipped_ = 0
    for i in range(n // 6):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        entry_ = rng.choice([rng.randrange(57, 3079), rng.randrange(57, 3079), rng.randrange(3136), 56 * rng.randrange(56)])
        # selected 8..47: the write to placed[selected] goes over other locals (#2 row 48)
        # (not 10, 11, 13, 14: they send the original to tile numbers or stack offsets past mapped memory)
        sel_ = rng.randrange(8) if i % 2 else rng.choice([*range(8, 10), 12, 15, 15, 15, 15, *range(16, 48)])
        crowd_ = rng.choice([0.0, 0.2, 0.5, 0.9, 1.0])     # 1.0: every ring fails, the search moves to other mercs
        if sel_ == 15:                                     # the top byte of `last`: only the search past the rings reads it
            crowd_ = 0.9
        for t_ in range(3136):
            if abs(t_ % 56 - entry_ % 56) <= 4 and abs(t_ // 56 - entry_ // 56) <= 4:
                b_ = rt(G_TILEMAP) + 8 * t_
                e.w8(b_ + 1, rng.choice(OBJ_) if rng.random() < crowd_ else 0)
                e.w8(b_ + 3, rng.choice([0, 0x73, e.r8(b_ + 3)]))
                e.w8(b_ + 5, rng.choice([0x40, 0x40, 0xC0, rng.randrange(0x30), 0x30, 0x3F]) if rng.random() < crowd_ else 0x40)
                e.w8(b_ + 4, rng.randrange(256))
        e.w8(rt(ai.G_SELECTED), sel_)
        for k_ in range(8):
            sp_ = rt(G_SOLDIERS) + 0x200 * k_
            s_ = bytearray(e.uc.mem_read(sp_, 0x200))
            s_[0x17A] = k_
            alive_ = k_ == sel_ or rng.random() < 0.75
            s_[0x182] = rng.choice([1, 1, 2]) if alive_ else rng.choice([0, 1])
            s_[0x138] = rng.choice([1, 1, 0x80]) if alive_ else rng.choice([0, 1])
            s_[0x183] = rng.choice([1, 50, 100, 0x80]) if alive_ else rng.choice([0, 0, 50])
            ai.w16(s_, 0x114, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
            s_[0x179] = rng.choice([0, 5, 0x3B, 0x3C, 0x40, rng.randrange(256)])
            s_[0x18E], s_[0x191] = rng.randrange(256), rng.choice([rng.randrange(256), s_[0x18E]])
            s_[0x19C] = rng.choice(list(range(10)) + [rng.randrange(256)])
            for o_ in (0x1CF, 0x9C, 0x1A1, 0x1A2, 0x1E0, 0x1BD, 0x1D3, 0x1D4):    # fields Place merc writes
                s_[o_] = rng.randrange(256)
            e.uc.mem_write(sp_, bytes(s_))
        facing_ = rng.choice(list(range(1, 9)) + [0, 0x80])
        if FAN and 35 in FAN and facing_ == 0x80:   # F35 rings start at the facing; 0x80 would read past the direction
            facing_ = 8                             # table in the original (callers give 1..8; 9..16 wraps to 9 at +5)
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"Random": [rng.randrange(0x7FFF) for _ in range(16)], "replace": [0] * 32, "reserve_ap": [0] * 2}
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        e.call(rt(0x2B470), entry_, facing_)
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        w_ = World(0)
        w_.memrun = True
        try:
            ai.merc_entry(w_, entry_, facing_)
        except ai.Undefined:         # the original swaps with an uninitialised stack word: not defined behaviour
            skipped_ += 1
            continue
        if PORT:                     # the mirror run above only finds the undefined cases
            for (a_, k_), b_ in zip(REG, snap):
                e.uc.mem_write(rt(a_), b_)
            script = {k_: list(v_) for k_, v_ in base_.items()}
            log.clear()
            port_sector("merc_entry", (entry_, facing_))
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["merc_entry"] = counts.get("merc_entry", 0) + 1
        key_ = sum(1 for c_ in log if c_[0] == "replace")
        results.setdefault("merc_entry", {}).setdefault(key_, 0)
        results["merc_entry"][key_] += 1
        if (got_g, got_log) != (want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [hex(j_) for j_ in range(0x200 * 48) if got_g[1][j_] != want_g[1][j_]][:12]
                print(f"MISMATCH merc_entry entry {entry_:#x} sel {sel_} facing {facing_}: soldier bytes {bad_} map "
                      f"{got_g[0] != want_g[0]}\n  calls original {got_log}\n  calls port     {log}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.w8(rt(ai.G_SELECTED), sel_save)
    if skipped_:
        print(f"merc_entry: {skipped_} cases skipped (selected merc never placed)")
if direct("slot_reset"):   # 0x2B250 per-slot reset at sector start: called directly, all 48 records compared
    REG = [(G_SOLDIERS, 0x200 * 0x30)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    for i in range(n // 6):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        sl_ = rng.choice([rng.randrange(0x30), rng.choice([0, 3, 4, 7, 8, 9, 0x27, 0x28, 0x2F])])
        j_ = sl_ if rng.random() < 0.6 else rng.randrange(0x30)     # the record and the slot number differ
        for m_ in {j_, sl_}:
            sp_ = rt(G_SOLDIERS) + 0x200 * m_
            e.uc.mem_write(sp_, bytes(rng.randrange(256) for _ in range(0x200)))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        log.clear()
        e.call(rt(0x2B250), rt(G_SOLDIERS) + 0x200 * j_, sl_)
        got_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        if PORT:
            port_sector("slot_reset", (j_, sl_))
        else:
            w_ = World(0)
            w_.memrun = True
            ai.slot_reset(MemRec(j_), w_, sl_)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["slot_reset"] = counts.get("slot_reset", 0) + 1
        key_ = (sl_ >= 8, j_ == sl_)
        results.setdefault("slot_reset", {}).setdefault(key_, 0)
        results["slot_reset"][key_] += 1
        if got_g != want_g:
            fails += 1
            if fails <= 10:
                bad_ = [hex(o_) for o_ in range(0x200 * 0x30) if got_g[0][o_] != want_g[0][o_]][:12]
                print(f"MISMATCH slot_reset record {j_} slot {sl_}: bytes {bad_}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
class CdFiles:
    """the files the original reads at sector start, the same bytes on both sides: OpenCDFile 0x38304 opens the
    game-folder file (1.13b patch files, then rom/) or else the CD file (0, or -1 when neither has it), fopen opens sec%02d.tmp (`tmp`; the script decides whether it exists);
    fread reads the file opened last, a short read at its end; fseek moves in it"""
    CALLS = {"open_cd": (0x38304, "mi"), "fopen": (0x75EA0, "mm"), "fseek": (0x757B1, "xii"), "fclose": (0x75A77, "x")}

    def __init__(self):
        self.cache, self.data, self.pos, self.tmp = {}, b"", 0, None

    names = None

    def exists(self, path):
        import subprocess
        from common import cd_read, game_path, game_read, ROOT
        if self.names is None:
            CdFiles.names = set(subprocess.check_output(["bsdtar", "-tf", os.path.join(ROOT, "rom", "JA1.gog")],
                                                        text=True).split())
        p_ = path.replace("\\", "/")
        if p_ not in self.cache:
            # OpenCDFile: the game-folder file first (1.13b patch, then rom/), then the CD (#24)
            self.cache[p_] = game_read(p_) if game_path(p_) or p_ in self.names else None
        return self.cache[p_] is not None

    def read(self, k_):
        log.append(("fread", (k_,)))
        self.pos += k_
        return self.data[self.pos - k_:self.pos]

    def after(self, name, args, v_):
        """bookkeeping after a logged call (both sides); returns the call's result"""
        if name == "open_cd":
            if not self.exists(args[0]):
                return 0xFFFFFFFF
            self.data, self.pos = self.cache[args[0].replace("\\", "/")], 0
        elif name == "fopen" and v_:
            self.data, self.pos = self.tmp, 0
        elif name == "fseek":
            self.pos = args[1]
        return v_

    def stub(self, name):
        inner_ = make_stub(name, self.CALLS[name][1])

        def fn(emu, eax, edx, ebx, ecx):
            v_ = inner_(emu, eax, edx, ebx, ecx)
            path_ = bytes(emu.uc.mem_read(eax, 80)).split(b"\0")[0].decode("latin-1") if name == "open_cd" else None
            return self.after(name, (path_, edx), v_)
        return fn

    def fread_stub(self, emu, eax, edx, ebx, ecx):
        emu.uc.mem_write(eax, bytes(self.read(edx * ebx)))
        return ebx

    def install(self):
        """stub the four calls and fread_; returns what remove() puts back"""
        CALLEES.update(self.CALLS)
        saved_ = {a_: e.stubs.get(rt(a_)) for a_ in [0x758AD] + [a_ for a_, _ in self.CALLS.values()]}
        for name, (a_, _) in self.CALLS.items():
            e.stub(rt(a_), self.stub(name))
        e.stub(rt(0x758AD), self.fread_stub)
        return saved_

    def remove(self, saved_):
        for name in self.CALLS:
            del CALLEES[name]
        for a_, v_ in saved_.items():
            if v_ is None:
                del e.stubs[rt(a_)]
            else:
                e.stubs[rt(a_)] = v_


CD = CdFiles()


class CdWorld(World):
    """World whose file calls read CD's files"""
    def read(self, k_):
        return CD.read(k_)

    def ext(self, name, *args):
        v_ = World.ext(self, name, *args)
        return CD.after(name, args, v_) if name in CdFiles.CALLS else v_


def plant_fuzz_(cur_):
    """g_plants records 0..3 (sector dword, state, record counts), [0x22A045] and the sector's owner flag around the
    current sector; the 3 real records stay most of the time (record 0 and the 12 bytes after record 3 too: the
    original never reads them)"""
    for k_ in range(5):
        p_ = rt(ai.G_PLANTS) + 12 * k_
        if rng.random() < 0.6:
            e.w32(p_ + 4, rng.choice([cur_, cur_, cur_ | 0x100, rng.randrange(256), 0xFFFFFF00 | cur_]))
        if rng.random() < 0.7:
            e.w8(p_ + 8, rng.choice([2, 2, 2, 0, 1, 3, 0x82, rng.randrange(256)]))
        for o_ in (9, 10):
            if rng.random() < 0.3:
                e.w8(p_ + o_, rng.choice([0, 1, 0x7F, 0x80, 0xFF, rng.randrange(256)]))
    e.w8(rt(0x22A045), rng.choice([0, 1, 1, 2, 2, 3, 0x80, 0xFF, rng.randrange(256)]))
    t9_ = rt(ai.G_SECTOR_TBL9) + 9 * cur_
    if cur_ < 60:
        e.w8(t9_, e.r8(t9_) & 0xFE | rng.randrange(2))
    e.w8(rt(ai.G_CUR_SECTOR), cur_)


if direct("sector_start"):   # SetupSectorFromXra 0x2E918 (whole sector start, XRA.DAT served from the CD): called directly
    LOCAL_ = {"rebuild": (0x2E730, "")}

    class SecWorld_(CdWorld):
        def raw_write(self, p_, data_):
            e.uc.mem_write(p_, bytes(data_))

        def ext(self, name, *args):
            if name == "w_53708":       # FUN_53708 runs for real on both sides (mirror: ai.item_init)
                return ai.item_init(self, *args)
            return CdWorld.ext(self, name, *args)
    stub53_ = e.stubs.pop(rt(0x53708))
    REAL_ = [0x56DD4, 0x56ED0, 0x15BB8, 0x74BD4, 0x56210, 0x15E50, 0x15E6C, 0x23760, 0x23530,   # InitSoldierNames' callees
             0x3FC78, 0x355D0, 0x317EC, 0x318C4]                                # PutItem, 0x355D0, patch routines
    if PORT:          # the library has DropItem, TakeGroundItem and the move code rebuild: real on the original too
        REAL_ += [0x470B0, 0x3C05C, 0x3FC04, 0x2FB98, 0x17C88, 0x63254, 0x633D8, 0x56310, 0x564F8]   # + ReplaceSoldier,
        # reserve AP (batch 9), RadioResult (10), QuestCheck (11), ActionApCost (12), ShotApCost (13)
    held_ = {a_: e.stubs.pop(rt(a_)) for a_ in REAL_ if rt(a_) in e.stubs}
    CALLEES.update(LOCAL_)
    for name_, (a_, k_) in LOCAL_.items():
        if not PORT:
            e.stub(rt(a_), make_stub(name_, k_))
    cdsave_ = CD.install()
    REG = [(G_SOLDIERS, 0x200 * 0x30), (G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (0xEB000, 0x160 + 12 * 256),
           (0x92DA0, 0x1C0), (0x93D54, 4), (0x93E64, 4), (0x95E0C, 1), (0xFA928, 0x30), (0x10D650, 0x30), (0x10D860, 0xB30),
           (0x222B68, 12 + 60 * 0xB1), (0x229C80, 0x880), (0x284F00, 0x30), (ai.G_SECTOR_TBL9, 9 * 60)]
    if PORT:          # the map tiles only (the port world has no rows around the map)
        REG[1] = (G_TILEMAP, 8 * 3136)
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    scratch_ = e.r32(rt(0x93E64))
    ssave_ = bytes(e.uc.mem_read(scratch_, 0x200))
    skipped_, cov_ = 0, {}
    CD.exists("DAT/XRA.DAT")
    xra0_ = CD.cache["DAT/XRA.DAT"]
    for i in range(n // 60):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        CD.cache["DAT/XRA.DAT"] = xra0_
        for k_ in range(30):        # resets into world fields: start from other values than the dump's
            e.w16(rt(ai.G_BLAST_LIST) + 2 * k_, rng.choice([ai.NO_TILE, rng.randrange(3136)]))
        for a_, k_ in ((ai.G_CALL_M, 8), (ai.G_KNOWN, 64), (ai.G_REMEMBER_E, 320), (ai.G_REMEMBER_M, 128)):
            for j_ in range(k_):
                if rng.random() < 0.3:
                    e.w8(rt(a_) + j_, rng.randrange(256))
        cur_ = rng.choice([rng.randrange(59)] * 9 + [0x3B])     # XRA.DAT has 59 records: sector 0x3B only when owned
        if rng.random() < 0.35:     # a supply drop's sector around its day, every drop state
            k_ = rng.randrange(4)
            cur_ = e.r8(rt(ai.G_CRATE_DROPS) + 4 * k_)
            e.w32(rt(ai.G_DAY), max(0, ai.s8(e.r8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 1)) + rng.choice([-1, 0, 0, 1, 1, 5])))
            for j_ in range(4):
                e.w8(rt(ai.G_CRATE_STATE) + j_, rng.choice([0, 0, 1, 1, 2, 3, 3, 4, 4, 5]))
                e.w8(rt(ai.G_CRATE_SHOWN) + j_, rng.choice([0, 1, 0xFF, 0xFF]))
        if rng.random() < 0.3:      # a plant's sector, every plant state
            cur_ = e.r8(rt(ai.G_PLANTS) + 4 + 12 * rng.randrange(1, 4))
            cur_ = cur_ if cur_ < 59 else rng.randrange(59)
            plant_fuzz_(cur_)
        if PFIX and 16 in PFIX and rng.random() < 0.5:   # P16: sector 26 holds the broken charge (#2 row 36)
            cur_ = 26
        e.w8(rt(ai.G_CUR_SECTOR), cur_)
        if rng.random() < 0.5 and cur_ < 59:     # stacked ground items in the XRA record (the CD's rarely stack)
            b_ = bytearray(xra0_)
            o_ = cur_ * 0x2E98 + 0x2000 + 0xC40
            cls_ = [e.r8(rt(ai.ITEM_TBL) + 12 * t_ + 6) for t_ in range(136)]
            by_cls_ = [[t_ for t_ in range(1, 136) if cls_[t_] == c_] or [1] for c_ in (1, 2)]
            for g_ in range(50):
                if rng.random() < 0.4:
                    p_ = o_ + 12 * g_
                    # item type 1 and stack classes 1 / 2 sit on the type tests; conditions near -100 and 97..99
                    # put random(10) - 5 + condition on the -100 / 100 limits
                    b_[p_:p_ + 4] = bytes([rng.choice([1, rng.choice(by_cls_[0]), rng.choice(by_cls_[1]),
                                                       rng.randrange(1, 136)]), rng.choice([2, 3, 4, 5]),
                                           rng.choice([0, 1, 50, 95, 97, 98, 99, 100, 0x7F, 0x80, 0x9C, 0x9E, 0xA0,
                                                       0xFF, rng.randrange(256)]),
                                           rng.choice([0, 0, 1])])
                    b_[p_ + 8:p_ + 12] = bytes(rng.choice([0, 1, 100, 0x9C, 0xFF, rng.randrange(256)]) for _ in range(4))
            CD.cache["DAT/XRA.DAT"] = bytes(b_)
        t9_ = rt(ai.G_SECTOR_TBL9) + 9 * cur_
        e.w8(t9_, rng.randrange(256) & 0xFE | (rng.random() < 0.3 or cur_ == 0x3B))
        e.w8(t9_ + 2, rng.choice([0, 0, 1, 5]))
        e.w8(t9_ + 3, rng.choice([0, 0, 1, 2, 3, 4, 5, 7]))
        e.w8(t9_ + 4, rng.choice([0, 0, 1, 2, 3, 5, 8]))
        sec_ = rt(ai.G_SECTORS) + 0xB1 * cur_
        for k_ in range(8):
            e.w8(sec_ + 0x96 + k_, rng.choice([1, 1, 2, 0xFF, 0xFE, 0, 0x80]))
        e.w8(sec_ + 0x9E, rng.choice([0, 0, 0x79, rng.randrange(1, 136)]))
        for k_ in range(5):
            e.w16(sec_ + 0x82 + 2 * k_, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136)]))
            e.w8(sec_ + 0x9F + k_, rng.choice([0, 0xAC, 0xAD, rng.randrange(256)]))
            e.w16(sec_ + 0x8C + 2 * k_, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136)]))
            e.w8(sec_ + 0xA4 + k_, rng.choice([0x86, 0x5E, rng.randrange(136 if PORT else 256)]))   # (136 item types)
        for o_ in (0xAC, 0xAD, 0xAE, 0xB0):
            e.w8(sec_ + o_, rng.choice([0, 0, 1, 2, 5]))
        e.w8(sec_ + 0xA9, rng.choice([0, 1]))
        e.w8(rt(ai.G_SECTORS) + 0xB1 * 0x3B + 0xA9, rng.choice([0, 1]))
        e.w8(rt(ai.G_PURIFIER), rng.choice([0, 0, 1]))
        e.w8(rt(ai.G_STORY_STATE) + 3, rng.choice([0, 2]))
        e.w32(rt(ai.G_TEAM_ON_DUTY), rng.choice([0, 1, 1, 4]))
        e.w8(rt(ai.G_SELECTED), rng.randrange(8))
        for k_ in range(15):        # one-off notes: placed before or not (a placed one on the ground is removed)
            e.w8(rt(ai.G_NOTES_FOUND) + 2 * k_ + 1, rng.choice([0, 1, 1]))
            if rng.random() < 0.2:
                e.w8(rt(ai.G_NOTES_FOUND) + 2 * k_, rng.randrange(1, 0x50))
        for t_ in range(3136):
            if rng.random() < 0.03:
                e.w8(rt(G_TILEMAP) + 8 * t_ + 1, rng.choice([0x34, 0x35, 0x36, 0x36, 0x37]))
        entry_ = rng.randrange(57, 3079)
        facing_ = rng.randrange(1, 9)
        tm_ = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))     # sec%02d.tmp = this map's bytes, a few changed
        TMP_ = bytearray(b for t_ in range(3136) for b in (tm_[8 * t_ + 1:8 * t_ + 5] + tm_[8 * t_ + 6:8 * t_ + 7]))
        TMP_ += bytes(e.uc.mem_read(rt(ai.G_GROUND_ITEMS), 0x960))
        for j_ in range(len(TMP_)):
            if rng.random() < 0.01:
                TMP_[j_] = rng.choice([0, 0xFF, rng.randrange(256)])
        if PORT:          # the game's ranges: ground classes, tree tiles above the last map row (batch 8)
            for t_ in range(3136):
                if e.r8(rt(G_TILEMAP) + 8 * t_) >= 157:   # 157 ground classes: bytes past them are other globals
                    e.w8(rt(G_TILEMAP) + 8 * t_, e.r8(rt(G_TILEMAP) + 8 * t_) % 157)    # (0x284F00 is one)
                if t_ >= 3080 and e.r8(rt(G_TILEMAP) + 8 * t_ + 1) in (0x35, 0x36):
                    e.w8(rt(G_TILEMAP) + 8 * t_ + 1, 0)
                if t_ >= 3080 and TMP_[5 * t_] in (0x35, 0x36):
                    TMP_[5 * t_] = 0
        CD.tmp = bytes(TMP_)
        if rng.random() < 0.5:   # enemies seen from the sector before (a fight dump has 1): the start clears it (0x2F6C3)
            e.w16(rt(0x22A478), rng.randrange(1, 9))
        if rng.random() < 0.5:   # fast-turn timer of an enemy turn still running (fight21/fight22/f1): the start clears it
            e.w32(rt(0x92DB0), rng.choice([10, 7340, 15000]))    # with 0x92DAC (0x2E941)
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {k_: [] for k_ in list(LOCAL_) + list(CdFiles.CALLS) + ["drop_item", "w_3c05c", "put_item", "w_53708",
                                                                         "w_355d0", "replay_event"]}
        base_.update({"Random": [rng.randrange(0x7FFF) for _ in range(3000)], "replace": [0] * 64, "reserve_ap": [0] * 2,
                 "fopen": [rng.choice([0, 0, 0x3F00 + rng.randrange(256), 0xFFFFFFFF])],
                 "radio_result": [rng.randrange(256)], "near_tile": [rng.randrange(3136) for _ in range(40)],
                 "action_ap": [rng.choice([1, 4, 8, 20, 0x7D, 0xFFFFFFFF]) for _ in range(4)],
                 "quest_check": [rng.choice([0, 1, 0x100])], "shot_ap": [rng.choice([1, 4, 5, 0xFF])] * 4,
                 "tile_dist": [rng.choice([0, 1, 2, 5])] * 4, "reach_ap": [rng.choice([0, 1])] * 4,
                 "path_ap": [rng.choice([0, 3, 0xFE])] * 4, "direction": [rng.randrange(1, 9)] * 4})
        if PFIX and 16 in PFIX and cur_ == 26 and rng.random() < 0.8:   # P16: no sec26.tmp, the XRA items load
            base_["fopen"] = [0]
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        e.call(rt(0x2E918), entry_, facing_)
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        got_x = bytes(e.uc.mem_read(scratch_, 0x200))
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        e.uc.mem_write(scratch_, ssave_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        w_ = SecWorld_(0)
        w_.memrun = True
        try:
            ai.setup_sector(w_, entry_, facing_)
        except ai.Undefined:
            skipped_ += 1
            e.uc.mem_write(scratch_, ssave_)
            continue
        if PORT:          # the mirror run above only finds the undefined cases: the port runs on the same input
            for (a_, k_), b_ in zip(REG, snap):
                e.uc.mem_write(rt(a_), b_)
            e.uc.mem_write(scratch_, ssave_)
            script = {k_: list(v_) for k_, v_ in base_.items()}
            log.clear()
            port_sector("setup_sector", (entry_, facing_), port_lib.SECTOR_LIB_OPS)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        want_x = bytes(e.uc.mem_read(scratch_, 0x200))
        e.uc.mem_write(scratch_, ssave_)
        counts["sector_start"] = counts.get("sector_start", 0) + 1
        if os.environ.get("SECCOV"):
            for c_ in log:
                cov_[c_[0]] = cov_.get(c_[0], 0) + 1
        key_ = (e.r8(t9_) & 1, sum(1 for c_ in got_log if c_[0] == "replace"))
        results.setdefault("sector_start", {}).setdefault(key_, 0)
        results["sector_start"][key_] += 1
        if (got_g, got_x, got_log) != (want_g, want_x, log):
            fails += 1
            if fails <= 10:
                bad_ = [(hex(REG[j_][0]), [hex(REG[j_][0] + o_) for o_ in range(REG[j_][1]) if got_g[j_][o_] != want_g[j_][o_]][:8])
                        for j_ in range(len(REG)) if got_g[j_] != want_g[j_]]
                d_ = next((j_ for j_ in range(min(len(got_log), len(log))) if got_log[j_] != log[j_]), None)
                print(f"MISMATCH sector_start sector {cur_} entry {entry_:#x}: {bad_} scratch {got_x != want_x} calls "
                      f"{len(got_log)}/{len(log)} first diff {d_}: {got_log[d_:d_ + 3] if d_ is not None else ''} / "
                      f"{log[d_:d_ + 3] if d_ is not None else ''}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    for name_, (a_, k_) in LOCAL_.items():
        del CALLEES[name_]
        e.stubs.pop(rt(a_), None)
    CD.cache["DAT/XRA.DAT"] = xra0_
    CD.remove(cdsave_)
    e.stubs[rt(0x53708)] = stub53_
    for a_, v_ in held_.items():
        e.stubs[rt(a_)] = v_
    if cov_:
        print("sector_start calls:", sorted(cov_.items()))
    if skipped_:
        print(f"sector_start: {skipped_} cases skipped (selected merc never placed / guard table overrun)")
if direct("guard_leaves"):   # sector start leaves: tapper 0x2BB90, guard 0x2B7DC (stack anchor), guard tile 0x2C70C
    REG = [(G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (G_SOLDIERS, 0x200 * 0x30)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    gsave_ = (e.r16(rt(ai.G_DAY)), e.r8(rt(ai.G_GUARD_SKILL)))
    OBJ_ = [0, 0, 0, 0, 0x3B, 0x3D, 0x43, 0x5A, 0x5C, 0x62, 0x30, 0x57, 0x86, rng.randrange(256)]
    GE8_ = [v_ for v_ in range(256) if e.r8(rt(ai.G_T0_CLASS) + v_) >= 8]    # ground values of class >= 8 / == 3
    EQ3_ = [v_ for v_ in range(256) if e.r8(rt(ai.G_T0_CLASS) + v_) == 3]
    CL7_ = [v_ for v_ in range(256) if e.r8(rt(ai.G_T0_CLASS) + v_) == 7]
    STAND_ = [v_ for v_ in range(256) if e.r8(rt(ai.G_T0_CLASS) + v_) < 7 and e.r8(rt(ai.G_T0_CLASS) + v_) != 3]
    stub0_ = e.stubs.pop(rt(0x53708)) if PORT else None    # port: the library resets the guard's items itself
    for i in range(n // 6):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        kind_ = i % 3
        tile_ = rng.choice([rng.randrange(57, 3079), rng.randrange(3136), rng.choice([0, 55, 3080, 3135])])
        crowd_ = rng.choice([0.0, 0.3, 0.7, 0.95])
        for t_ in range(3136):
            if abs(t_ % 56 - tile_ % 56) <= 8 and abs(t_ // 56 - tile_ // 56) <= 8:
                b_ = rt(G_TILEMAP) + 8 * t_
                e.w8(b_, rng.choice([e.r8(b_), rng.randrange(256)]) if rng.random() < crowd_ else e.r8(b_))
                e.w8(b_ + 1, rng.choice(OBJ_) if rng.random() < crowd_ else 0)
                e.w8(b_ + 5, rng.choice([0x40, 0x40, 0xC0, rng.randrange(0x30), 0x30, 0x3F]) if rng.random() < crowd_ else 0x40)
        if kind_ == 2 and rng.random() < 0.6:     # a hard map: the search runs far (distance limit, wrap, misses)
            deep_, busy_ = rng.choice([0.5, 0.9, 0.99, 1.0]), rng.choice([0.0, 0.5, 0.95])
            ge8_ = rng.choice([1.0, 1.0, 0.95, 0.8, 0.5])
            for t_ in range(3136):
                if abs(t_ % 56 - tile_ % 56) <= 30 and abs(t_ // 56 - tile_ // 56) <= 30:
                    b_ = rt(G_TILEMAP) + 8 * t_
                    r_ = rng.random()
                    if r_ < deep_:
                        e.w8(b_, rng.choice(GE8_) if rng.random() < ge8_ else rng.choice(EQ3_ + CL7_))
                    if rng.random() < busy_:
                        e.w8(b_ + 5, rng.randrange(0x30))
        edge_ = None
        if kind_ == 2 and rng.random() < 0.35:    # the miss limit: the only free tile lies at miss 148..151
            tile_ = 56 * rng.randrange(24, 32) + rng.randrange(24, 32)
            for t_ in range(3136):
                if abs(t_ % 56 - tile_ % 56) <= 30 and abs(t_ // 56 - tile_ // 56) <= 30:
                    b_ = rt(G_TILEMAP) + 8 * t_
                    e.w8(b_, rng.choice(GE8_))
                    e.w8(b_ + 1, 0)
                    e.w8(b_ + 5, 0x40)
            d0_, dd_, want_, misses_ = rng.randrange(1, 9), rng.randrange(4), rng.choice([148, 149, 150, 151]), 0
            edge_ = (d0_, dd_)
            w0_ = World(0)
            while dd_ <= 27:          # the search's visit order (sector_start.md "Guard tile")
                t_ = tile_
                for _ in range(dd_):
                    t_ = ai.step_tile(t_, ai.dir_step(w0_, d0_))
                b_ = rt(G_TILEMAP) + 8 * t_
                if misses_ == want_:
                    e.w8(b_, rng.choice(STAND_))
                    break
                if CL7_ and rng.random() < 0.05:       # class 7, taken: no result and no miss
                    e.w8(b_, rng.choice(CL7_))
                    e.w8(b_ + 5, rng.randrange(0x30))
                else:
                    misses_ += 1
                d0_ = d0_ + 1 if d0_ < 8 else 1
                if d0_ == edge_[0]:
                    dd_ += 1
        slot_ = rng.randrange(0x30)
        sp_ = rt(G_SOLDIERS) + 0x200 * slot_
        s_ = bytearray(rng.randrange(256) for _ in range(0x200))
        s_[0x17A] = slot_
        e.uc.mem_write(sp_, bytes(s_))
        e.w16(rt(ai.G_DAY), rng.choice([0, 1, 5, 30, 94, 95, 200, 0x7FF9, 0xFFFB, rng.randrange(0x10000)]))
        e.w8(rt(ai.G_GUARD_SKILL), rng.choice([0, 0, 5, 20, 50, 100, 200, 255]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"Random": [rng.randrange(0x7FFF) for _ in range(40)], "replace": [0] * 2}
        if not PORT:
            base_["w_53708"] = [0] * 4
        if kind_ == 1 and rng.random() < 0.6:     # InitGuard rolls aimed at its edges (Random order is fixed)
            e.w16(rt(ai.G_DAY), 95)
            e.w8(rt(ai.G_GUARD_SKILL), rng.choice([0, 5, 20, 45, 50, 55, 60, 65, 70]))
            lv_ = rng.choice([0, 9, 10, 19, 79, 80, 89, 90, 98])
            a_ = (min(lv_ // 10 + 1, 9) - 1) * 5
            c_ = (e.r8(rt(ai.G_GUARD_SKILL)) + a_) & 0xFF
            seq_ = [rng.randrange(8), rng.randrange(4), rng.randrange(6), lv_]
            for b0_ in (40, 40, 40, 20, 30):
                t_ = rng.choice([99, 100, 101, rng.randrange(60, 130)]) - a_ - b0_
                r1_ = max(0, min(25, t_ // 2))
                seq_ += [r1_, max(0, min(25, t_ - r1_))]
            seq_ += [rng.randrange(16), rng.randrange(16)]

            def edge_(x__):
                return max(0, min(99, x__ + rng.choice([-1, 0, 1])))
            # (port: item 0's reset runs for real and draws its condition before the next roll)
            seq_ += [edge_(c_), rng.randrange(2)] + ([rng.randrange(0x7FFF)] if PORT else []) + [edge_(c_ + 0x32), edge_(c_ + 0x19)]
            seq_ += [rng.randrange(0x7FFF) for _ in range(10)]
            snap = [bytes(e.uc.mem_read(rt(a__), k__)) for a__, k__ in REG]
            base_["Random"] = seq_
        facing_ = rng.choice(list(range(1, 9)) + [0, 0x80])
        if kind_ == 0:
            args_ = (sp_, tile_, facing_)
            fn_ = lambda w__: ai.init_tapper(MemRec(slot_), w__, tile_, facing_)
            addr_ = 0x2BB90
        elif kind_ == 1:
            orders_ = rng.choice([1, 3, rng.randrange(256)])
            anchor_ = rng.choice([ai.NO_TILE, rng.randrange(3136)])
            args_ = (sp_, tile_, facing_, orders_, anchor_)
            fn_ = lambda w__: ai.init_guard(MemRec(slot_), w__, tile_, facing_, orders_, anchor_)
            addr_ = 0x2B7DC
        else:
            d_ = rng.randrange(1, 9)          # 0 or > 8 never comes back to the start: the original loops forever
            dist_ = rng.choice([0, 1, 2, 3, 5, 10, 26, 27, 28, -1, 24, 25, 26, 26, 27])
            if edge_:
                d_, dist_ = edge_
            args_ = (tile_, d_, dist_)
            fn_ = lambda w__: ai.guard_tile(w__, tile_, d_, dist_)
            addr_ = 0x2C70C
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        got = ai._i32(e.call(rt(addr_), *args_))
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        if PORT:
            want = port_sector(("init_tapper", "init_guard", "guard_tile")[kind_],
                               ((slot_,) + args_[1:]) if kind_ < 2 else args_)
        else:
            w_ = World(slot_)
            w_.memrun = True
            want = fn_(w_)
        want = ai._i32(want) if kind_ == 2 else got
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        name_ = ("tapper", "guard", "guard_tile")[kind_]
        counts[name_] = counts.get(name_, 0) + 1
        key_ = sum(1 for c_ in log if c_[0] == "Random") if kind_ < 2 else (got == tile_)
        results.setdefault(name_, {}).setdefault(key_, 0)
        results[name_][key_] += 1
        if (got, got_g, got_log) != (want, want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [hex(j_ % 0x200) for j_ in range(0x200 * 0x30) if got_g[1][j_] != want_g[1][j_]][:12]
                print(f"MISMATCH {name_} tile {tile_:#x} args {args_[1:]}: {got} / {want} soldier bytes {bad_} map "
                      f"{got_g[0] != want_g[0]}\n  calls original {got_log}\n  calls port     {log}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.w16(rt(ai.G_DAY), gsave_[0])
    e.w8(rt(ai.G_GUARD_SKILL), gsave_[1])
    if stub0_ is not None:
        e.stubs[rt(0x53708)] = stub0_
if direct("equip"):   # equipment roll 0x2FCE4 (EAX soldier, DL flag) and its stack count 0x30A88: called directly
    REG = [(G_SOLDIERS, 0x200 * 0x30)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    GLOB_ = [(ai.G_DAY, 4), (ai.G_DIFFICULTY, 1), (ai.G_EQ_ITEMS, 1), (ai.G_EQ_GUNS, 1), (ai.G_EQ_THROWN, 1),
             (ai.ITEM_TBL, 12 * 0x40)]
    gsave_ = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in GLOB_]
    ATT_ = [e.r8(rt(ai.ATTACH_TBL) + 4 * k_ + 1) for k_ in range(0x53)]     # entry 0x52 = first byte past the table
    stub0_ = e.stubs.pop(rt(0x53708))       # FUN_53708 runs for real on both sides (mirror: ai.item_init)

    class EqWorld_(World):
        def ext(self, name, *args):
            if name == "w_53708":
                return ai.item_init(self, *args)
            return World.ext(self, name, *args)
    for i in range(n // 6):
        for (a_, k_), b_ in zip(REG + GLOB_[-1:], tsave + gsave_[-1:]):
            e.uc.mem_write(rt(a_), b_)
        slot_ = rng.randrange(0x30)
        sp_ = rt(G_SOLDIERS) + 0x200 * slot_
        s_ = bytearray(rng.randrange(256) for _ in range(0x200))
        for k_ in range(12):
            s_[12 * k_] = rng.choice([0, 0, 0, rng.randrange(256), rng.randrange(1, 0x40)])
            s_[12 * k_ + 2] = rng.choice([0, 0, 100, rng.randrange(256)])
        s_[0] = rng.choice([0, 0, 1, 2, 3, 4, 8, 11, 12, 14, 15, rng.choice(ATT_), ATT_[-1], ATT_[-2], rng.randrange(256)])
        s_[3] = rng.choice([0, 0, 0, rng.randrange(256)])
        s_[0xC] = rng.choice([0, 0, 0x1C, rng.randrange(256)])
        s_[0xE] = rng.choice([100, 100, 99, rng.randrange(256)])
        for o_ in (0x18, 0x3C, 0x30, 0x24):
            s_[o_] = rng.choice([0, 0, 0x23, 0x26, 0x2D, 0x30, rng.randrange(256)])
        for o_ in range(0x54, 0x90, 0xC):     # items 7..11: copies of the replaced types, stacks 0x36..0x3E
            s_[o_] = rng.choice([s_[0], s_[0x18], s_[0x3C], rng.randrange(0x35, 0x40), e.r8(rt(ai.WEAPON_TBL) + 1 + 6 * min(s_[0], 15)),
                                 0, rng.randrange(256)])
        s_[0xBA] = rng.choice([0, 0, 0, 4, rng.randrange(256)])
        s_[0x192] = rng.choice([0, 1, 4, 5, 9, 0x80, rng.randrange(256)])
        s_[0x1B4] = rng.choice([0, 1, 2, 3, 4, 5, rng.randrange(256)])
        s_[0x1D3] = rng.choice([5, 5, 0, rng.randrange(256)])
        if PORT:       # the port's item_types has the game's 136 types; the original reads past its table for others
            for k_ in range(12):
                s_[12 * k_] %= 136
        e.uc.mem_write(sp_, bytes(s_))
        e.w32(rt(ai.G_DAY), rng.choice([0, 1, 4, 5, 6, 30, 0xFFFFFFFF, 0x80000000, 0x7FFFFFFF, rng.randrange(1 << 32)]))
        e.w8(rt(ai.G_DIFFICULTY), rng.choice([0, 1, 2] if PORT else [0, 1, 2, 3, 0xFF, rng.randrange(256)]))
        # (the port's equip_skill has the 3 difficulties only; the original reads past its table for others)
        for a_ in (ai.G_EQ_ITEMS, ai.G_EQ_GUNS, ai.G_EQ_THROWN):
            e.w8(rt(a_), rng.choice([0, 0, 1, 10, 50, 100, 200, rng.randrange(256)]))
        flag_ = rng.choice([0, 0, 1, 2, 0x100, 0x1FF])
        day_ = e.r32(rt(ai.G_DAY))       # the chance base c as the routine computes it: Random values aimed at each edge
        c_ = ai.s8((e.r8(rt(ai.G_EQ_SKILL) + 2 * ai.s8(e.r8(rt(ai.G_DIFFICULTY))))
                    + ((e.r8(rt(ai.G_DAY)) * 5 - 0x19) if day_ < 5 or day_ >= 1 << 31 else 0) + (s_[0x192] - 4) * 5) & 0xFF)
        a_, b_ = e.r8(rt(ai.G_EQ_ITEMS)), e.r8(rt(ai.G_EQ_GUNS))
        EDGE_ = [a_ + c_ + 15, a_ + c_ + 50, a_ + c_, a_ + c_ + 25, c_ - 5 + a_, c_ - 10 + a_, a_ + 25, a_ + 50, a_ + 10,
                 a_ + 35, 25, 45, 8, 1, 3, 10, e.r8(rt(ai.G_EQ_THROWN)) + c_ + 15] + [u_ * 5 + c_ + b_ for u_ in range(7)]
        lo_ = rng.choice([0, 3, 10, 30])
        seq_ = [rng.choice([rng.randrange(0x7FFF), rng.randrange(lo_, lo_ + 30), rng.randrange(95, 100) * rng.choice([1, 1, 7]),
                            min(99, max(0, rng.choice(EDGE_) + rng.choice([-1, 0]))), rng.randrange(8)]) for _ in range(200)]
        if rng.random() < 0.4:      # every roll on an edge
            seq_ = [min(99, max(0, rng.choice(EDGE_) + rng.choice([-1, 0]))) for _ in range(200)]
        for t_ in rng.sample(range(0x40), rng.choice([0, 0, 4, 16])) + rng.sample([0xF, 0x10] + list(range(0x36, 0x3F)), 2):
            # item table: class (+6) and max count (+5)
            e.w8(rt(ai.ITEM_TBL) + 12 * t_ + 6, rng.choice([1, 1, 2, 3, 4, 5] if PORT else [1, 1, 2, 3, 5, 6, 0x80, rng.randrange(256)]))
            # (port: the game's classes 1..5 only; a higher class makes item 11's units run into soldier +0x90, a
            # pointer the harness converts for the port, so those bytes cannot compare)
            e.w8(rt(ai.ITEM_TBL) + 12 * t_ + 5, rng.choice([99, 99, 0, 1, 2, 6, 0x80, rng.randrange(256)]))
        direct_ = i % 4 == 3          # FUN_53708 alone on one item, every DL / BL mode
        if direct_:
            ip_ = sp_ + 12 * rng.randrange(12)
            dl_, bl_ = rng.choice([0, 1, 2, 3, 0x101, 0xFF]), rng.choice([0, 1, 2, 3, 0x102, 0xFF])
            e.w8(ip_ + 1, rng.choice([0, 1, 2, 5, 6, 7, 99, 100, 0x7F, 0x80, rng.randrange(256)]))
        snap = [bytes(e.uc.mem_read(rt(a__), k__)) for a__, k__ in REG]
        base_ = {"Random": seq_}
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        if direct_:
            e.call(rt(0x53708), ip_, dl_, bl_)
        else:
            e.call(rt(0x2FCE4), sp_, flag_)
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        if PORT:
            if direct_:
                port_sector("item_init", (("soldiers", ip_ - rt(G_SOLDIERS)), dl_, bl_))
            else:
                port_sector("equip", (slot_, flag_))
        else:
            w_ = EqWorld_(slot_)
            w_.memrun = True
            if direct_:
                ai.item_init(w_, ip_, dl_, bl_)
            else:
                ai.equip(MemRec(slot_), w_, slot_, flag_)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["equip"] = counts.get("equip", 0) + 1
        key_ = "item" if direct_ else (flag_ & 0xFF != 0, sum(1 for c_ in log if c_[0] == "Random") // 10)
        results.setdefault("equip", {}).setdefault(key_, 0)
        results["equip"][key_] += 1
        if (got_g, got_log) != (want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [hex(j_ % 0x200) for j_ in range(0x200 * 0x30) if got_g[0][j_] != want_g[0][j_]][:12]
                d_ = next((j_ for j_ in range(min(len(got_log), len(log))) if got_log[j_] != log[j_]), None)
                if direct_ and os.environ.get("EQDBG"):
                    o_ = ip_ - sp_
                    print("  item", o_, "dl bl", dl_, bl_, "before", snap[0][0x200 * slot_ + o_:0x200 * slot_ + o_ + 12].hex(),
                          "orig", got_g[0][0x200 * slot_ + o_:0x200 * slot_ + o_ + 12].hex(), "mirror",
                          want_g[0][0x200 * slot_ + o_:0x200 * slot_ + o_ + 12].hex(), "class/max",
                          e.r8(rt(ai.ITEM_TBL) + 12 * snap[0][0x200 * slot_ + o_] + 6), e.r8(rt(ai.ITEM_TBL) + 12 * snap[0][0x200 * slot_ + o_] + 5), got_log, log)
                print(f"MISMATCH equip slot {slot_} flag {flag_:#x}: soldier bytes {bad_} calls {len(got_log)}/{len(log)} "
                      f"first diff {d_}: {got_log[d_:d_ + 2] if d_ is not None else ''} / {log[d_:d_ + 2] if d_ is not None else ''}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(GLOB_, gsave_):
        e.uc.mem_write(rt(a_), b_)
    e.stubs[rt(0x53708)] = stub0_
if direct("init_soldier"):   # InitSoldierNames 0x2C034 (per-slot sector reset) with 0x5383C, 0x2C6A0: called directly
    REG = [(G_SOLDIERS, 0x200 * 0x30), (0x10D500, 0xE00), (ai.G_BUSY, 4)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    gsave_ = e.r8(rt(ai.G_CUR_SECTOR))
    REAL_ = [0x56DD4, 0x56ED0, 0x15BB8, 0x74BD4, 0x56210, 0x15E50, 0x15E6C, 0x23760, 0x23530, 0x239CC, 0x23A70,
             0x239D8, 0x5383C, 0x2C6A0]      # the callees run for real on both sides
    held_ = {a_: e.stubs.pop(rt(a_)) for a_ in REAL_ if rt(a_) in e.stubs}
    skip_ = 0
    for i in range(n // 6):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        slot_ = rng.choice([rng.randrange(0x30), 0x28, 0x28, 7, 8, 0xF, 0x10, 0x27])
        sp_ = rt(G_SOLDIERS) + 0x200 * slot_
        s_ = bytearray(rng.randrange(256) for _ in range(0x200))
        s_[0x17A] = slot_
        s_[0x182] = rng.choice([0, 1, 1, rng.randrange(256)])
        ai.w16(s_, 0x114, rng.choice([rng.randrange(3136), rng.randrange(3136), ai.NO_TILE, rng.randrange(-200, 3300) & 0xFFFF]))
        ai.w16(s_, 0x1E8, rng.choice([rng.randrange(0x44), rng.randrange(0x44), rng.randrange(0x100)]))
        if PORT:          # the port reads the map and the script table inside their bounds (sector_start.md batch 5)
            ai.w16(s_, 0x114, rng.randrange(3136))
            ai.w16(s_, 0x1E8, rng.randrange(0x44))
        for k_ in range(12):
            s_[12 * k_] = rng.choice([0, 0, rng.randrange(256)])
        s_[0x30] = rng.choice([0x2B, 0x2B, 0, rng.randrange(256)])
        s_[0x24] = rng.choice([0x27, 0, rng.randrange(256)])
        s_[0x192] = rng.choice([0, 1, 2, 3, 4, 9, 0x80, 0xFF, rng.randrange(256)])
        s_[0x183] = rng.choice([0, 14, 15, 0x80, rng.randrange(256)])
        s_[0x1D4] = rng.choice([0, 1, 2, 3, 4, 5, rng.randrange(256)])
        s_[0x17E] = rng.choice([8, 8, 0, rng.randrange(256)])
        s_[0x1F5] = rng.choice([0, 1, 0xE, 0xF, rng.randrange(256)])
        e.uc.mem_write(sp_, bytes(s_))
        e.w8(rt(ai.G_CUR_SECTOR), rng.choice([0, 0, 5, rng.randrange(60)]))
        e.w32(rt(ai.G_BUSY), rng.choice([0, 1, 5, 0xFFFFFFFF]))
        lv_ = ai.s8(s_[0x192])
        thr_ = {0: 5, 1: 10, 2: 0x32, 3: 0x19, 4: 0x19}.get(s_[0x1D4], 0) + (lv_ - 1) * 5    # the alert roll's edge
        seq_ = [rng.randrange(0x7FFF), rng.choice([rng.randrange(0x7FFF)] + [min(99, max(0, thr_ + rng.choice([-1, 0])))] * 4)]
        for a_, k_ in ((ai.G_SLOT_T34, 0x30), (ai.G_REMEMBER_E, 0x140), (ai.G_REMEMBER_M, 0x80), (ai.G_LAST_SEEN_E, 0x800),
                       (ai.G_LAST_SEEN_M, 0x100)):
            e.uc.mem_write(rt(a_), bytes(rng.randrange(1, 256) for _ in range(k_)))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"Random": seq_}
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        try:
            e.call(rt(0x2C034), sp_)
        except emu.EmuStop:       # MercMaxAP divides by max hp x 3: max hp 0 below hp crashes the original too
            skip_ += 1
            continue
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        if PORT:
            port_sector("soldier_start", (slot_,))
        else:
            w_ = World(slot_)
            w_.memrun = True
            ai.init_soldier(MemRec(slot_), w_)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["init_soldier"] = counts.get("init_soldier", 0) + 1
        key_ = (s_[0x182] != 0, slot_ >= 0x28)
        results.setdefault("init_soldier", {}).setdefault(key_, 0)
        results["init_soldier"][key_] += 1
        if (got_g, got_log) != (want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [(hex(REG[j_][0]), [hex(o_ % 0x200 if j_ == 0 else REG[j_][0] + o_) for o_ in range(REG[j_][1]) if got_g[j_][o_] != want_g[j_][o_]][:10])
                        for j_ in range(len(REG)) if got_g[j_] != want_g[j_]]
                print(f"MISMATCH init_soldier slot {slot_}: {bad_}\n  calls original {got_log}\n  calls port     {log}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.w8(rt(ai.G_CUR_SECTOR), gsave_)
    for a_, v_ in held_.items():
        e.stubs[rt(a_)] = v_
    if skip_:
        print(f"init_soldier: {skip_} cases skipped (the original divides by zero in MercMaxAP)")
if direct("walk_in"):   # edge tile 0x30490, guard walk-in 0x30600 (day clock op 312), enemy walk-in 0x306BC (op 329): direct
    rng_main_, rng = rng, random.Random(0x3A1C)   # own random stream: the later sections keep their cases
    REG = [(G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (G_SOLDIERS, 0x200 * 0x30), (0x10D500, 0xE00), (ai.G_BUSY, 4)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    GLOB_ = [(ai.G_DAY, 4), (ai.G_CUR_SECTOR, 1), (ai.G_GUARD_SKILL, 1)]
    gsave_ = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in GLOB_]
    REAL_ = [0x56DD4, 0x56ED0, 0x15BB8, 0x74BD4, 0x56210, 0x15E50, 0x15E6C, 0x23760, 0x23530, 0x239CC, 0x23A70,
             0x239D8, 0x5383C, 0x2C6A0, 0x53708]      # InitSoldierNames' callees and the item reset run for real
    held_ = {a_: e.stubs.pop(rt(a_)) for a_ in REAL_ if rt(a_) in e.stubs}
    OBJ_ = [0, 0, 0, 0, 0x3B, 0x3D, 0x43, 0x5A, 0x5C, 0x62, 0x30, 0x57, 0x86, rng.randrange(256)]
    EDGES_ = {1: [0xC08 + k_ for k_ in range(56)], 3: [56 * k_ for k_ in range(56)], 5: list(range(56)),
              7: [56 * k_ + 55 for k_ in range(56)], 9: list(ai.EDGE_TILES)}

    class WalkWorld_(World):
        def ext(self, name, *args):
            if name == "w_53708":
                return ai.item_init(self, *args)
            return World.ext(self, name, *args)
    skip_ = 0
    for i in range(n // 12):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        kind_ = i % 3
        d_ = rng.choice([1, 3, 5, 7, 9])
        crowd_ = rng.choice([0.0, 0.3, 0.7, 0.95, 1.0])
        for t_ in EDGES_[d_]:
            b_ = rt(G_TILEMAP) + 8 * t_
            if rng.random() < crowd_:
                e.w8(b_, rng.choice([e.r8(b_), rng.randrange(256), rng.randrange(0x6C, 256)]))
                e.w8(b_ + 1, rng.choice(OBJ_))
                e.w8(b_ + 5, rng.choice([0x40, 0xC0, rng.randrange(0x30), 0x30, 0x3F, 0x7F]))
            else:
                e.w8(b_ + 5, 0x40)
        for k_ in range(0x30):
            if rng.random() < 0.3:
                e.w8(rt(G_SOLDIERS) + 0x200 * k_ + 0x9C, rng.choice([0, 1, 0xFF]))
        if rng.random() < 0.08:       # the whole edge taken: 1000 tries, no tile (result -1 / 0)
            e.w8(rt(G_SOLDIERS) + 0x9C, 1)
            for t_ in EDGES_[d_]:
                e.w8(rt(G_TILEMAP) + 8 * t_ + 5, 0)
        slot_ = rng.choice([0x28, 0x2F, rng.randrange(0x28, 0x30)]) if kind_ == 2 else 8
        if kind_ == 1:
            for k_ in range(8, 16):
                e.w8(rt(G_SOLDIERS) + 0x200 * k_ + 0x182, rng.choice([0, 1, 1, 1, rng.randrange(256)]))
            if rng.random() < 0.15:
                for k_ in range(8, 16):
                    e.w8(rt(G_SOLDIERS) + 0x200 * k_ + 0x182, 1)
        if kind_ > 0:
            sp_ = rt(G_SOLDIERS) + 0x200 * slot_
            e.uc.mem_write(sp_, bytes(rng.randrange(256) for _ in range(0x200)))
        e.w32(rt(ai.G_DAY), rng.choice([1, 2, 3, 4, 10, 30, 50, 51, 52, 100, 0, 0xFFFFFFFF, rng.randrange(1, 120)]))
        e.w8(rt(ai.G_CUR_SECTOR), rng.choice([0, 0, 59, rng.randrange(60)]))
        e.w8(rt(ai.G_GUARD_SKILL), rng.choice([0, 5, 20, 50, 100, 255]))
        e.w32(rt(ai.G_BUSY), rng.choice([0, 1]))
        for a_, k_ in ((ai.G_SLOT_T34, 0x30), (ai.G_REMEMBER_E, 0x140), (ai.G_REMEMBER_M, 0x80), (ai.G_LAST_SEEN_E, 0x800),
                       (ai.G_LAST_SEEN_M, 0x100)):
            e.uc.mem_write(rt(a_), bytes(rng.randrange(1, 256) for _ in range(k_)))
        seq_ = [rng.choice([rng.randrange(0x7FFF), rng.randrange(0x7FFF), rng.randrange(8), rng.choice([0, 1, 7, 8, 27, 64, 125, 216, 343, 512, 511, 999])])
                for _ in range(rng.choice([40, 120, 400]))]
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"Random": seq_, "replace": [0] * 2}
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        addr_, args_ = ((0x30490, (d_,)), (0x30600, (d_,)), (0x306BC, (slot_, d_)))[kind_]
        try:
            got = ai._i32(e.call(rt(addr_), *args_))
        except emu.EmuStop:       # MercMaxAP divides by max hp x 3 (InitSoldierNames): the original crashes too
            skip_ += 1
            continue
        got = ai._i32(got) if kind_ == 0 else got & 0xFF
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        name_ = ("edge_tile", "guard_enter", "enemy_walk_in")[kind_]
        if PORT:
            want = port_sector(name_, args_)
        else:
            w_ = WalkWorld_(slot_)
            w_.memrun = True
            want = (ai.edge_tile(w_, d_) if kind_ == 0 else ai.guard_enter(w_, d_) if kind_ == 1
                    else ai.enemy_walk_in(w_, slot_, d_))
        want = ai._i32(want) if kind_ == 0 else want & 0xFF
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts[name_] = counts.get(name_, 0) + 1
        key_ = (got if kind_ else got == -1, d_)
        results.setdefault(name_, {}).setdefault(key_, 0)
        results[name_][key_] += 1
        if (got, got_g, got_log) != (want, want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [(hex(REG[j_][0]), [hex(o_ % 0x200 if j_ == 1 else REG[j_][0] + o_) for o_ in range(REG[j_][1]) if got_g[j_][o_] != want_g[j_][o_]][:10])
                        for j_ in range(len(REG)) if got_g[j_] != want_g[j_]]
                d0_ = next((j_ for j_ in range(min(len(got_log), len(log))) if got_log[j_] != log[j_]), None)
                print(f"MISMATCH {name_} args {args_}: {got} / {want} {bad_} calls {len(got_log)}/{len(log)} first diff {d0_}: "
                      f"{got_log[d0_:d0_ + 2] if d0_ is not None else ''} / {log[d0_:d0_ + 2] if d0_ is not None else ''}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(GLOB_, gsave_):
        e.uc.mem_write(rt(a_), b_)
    for a_, v_ in held_.items():
        e.stubs[rt(a_)] = v_
    if skip_:
        print(f"walk_in: {skip_} cases skipped (the original divides by zero in MercMaxAP)")
    rng = rng_main_
if direct("supplies"):   # LoadCrateDrops 0x62360, CrateLost? 0x62780, FixedItems? 0x2ACAC, LoadPlantSector 0x62848: direct
    cdsave_ = CD.install()          # the patch routines read MISC files from the CD on both sides
    # (port: TakeGroundItem and the listed test run for real too, the library has them)
    held_ = {a_: e.stubs.pop(rt(a_)) for a_ in (0x53708, 0x3FC78) + ((0x3C05C, 0x3FC04) if PORT else ()) if rt(a_) in e.stubs}
    REG = [(G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (0xEB000, 0x160 + 12 * 256), (0x229C80, 0x880), (0x92DA0, 0x1C0),
           (ai.G_CRATE_DROPS, 16), (ai.G_SECTOR_TBL9, 9 * 60), (combat.G_MOVECODE, 2 * 3136), (ai.G_ITEM_BUF, 12),
           (ai.FIXED_ITEMS, 88)]
    if PORT:          # the map tiles only: with no tile found Put item also writes byte 6 of tile 0xC41 (batch 3)
        REG[0] = (G_TILEMAP, 8 * 3136)
        REG[1] = (0xEB000, 0x160 + 12 * 200)    # the port's 200 ground entries (the unit fill past entry 199: SPEC GAP)
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    for i in range(n // 6):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        kind_ = i % 5
        for k_ in range(4):
            if rng.random() < 0.3:
                e.w8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 1, rng.choice([0, 5, 0x7F, 0x80, 0xFF, rng.randrange(256)]))
            if rng.random() < 0.3:
                e.w8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 2, rng.choice([0, 1, 0x80, rng.randrange(256)]))
            e.w8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 3, rng.choice([0, 0, 1, rng.randrange(256)]))
            e.w8(rt(ai.G_CRATE_STATE) + k_, rng.choice([0, 1, 1, 2, 3, 3, 4, 4, 5, rng.randrange(256)]))
            e.w8(rt(ai.G_CRATE_SHOWN) + k_, rng.choice([0, 1, 0xFF, 0xFF, rng.randrange(256)]))
        for a_ in (ai.G_EQ_ITEMS, ai.G_EQ_GUNS, ai.G_EQ_MEDS, ai.G_EQ_THROWN):
            e.w8(rt(a_), rng.randrange(256))
        k_ = rng.randrange(4)
        e.w8(rt(ai.G_CUR_SECTOR), rng.choice([e.r8(rt(ai.G_CRATE_DROPS) + 4 * k_)] * 4 + [rng.randrange(256)]))
        sec_ = ai.s8(e.r8(rt(ai.G_CRATE_DROPS) + 4 * k_))
        e.w8(rt(ai.G_SECTOR_TBL9) + 9 * sec_, rng.randrange(256))
        e.w32(rt(ai.G_DAY), (ai.s8(e.r8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 1)) + rng.choice([-1, 0, 0, 0, 1, 5, -200])) & 0xFFFFFFFF)
        for t_ in range(3136):
            if rng.random() < 0.05:
                e.w8(rt(G_TILEMAP) + 8 * t_ + 6, rng.choice([0xFF, rng.randrange(0x40)]))
        arg_ = rng.choice([0, 1, 2, 3, 3, 4, 0xFF])
        if PORT:          # the port's crate_state has the 4 drops
            arg_ &= 3
        if kind_ >= 3:
            # (port: the game's 60 sectors; past the sector table the original reads other memory)
            plant_fuzz_(rng.choice([e.r8(rt(ai.G_PLANTS) + 4 + 12 * rng.randrange(1, 4))] * 4 +
                                   [rng.randrange(60 if PORT else 256)]))
        fake_ = {}
        if kind_ in (0, 3) and rng.random() < 0.6:   # fuzzed patch files (the CD's own rarely hit the B / C flags)
            CD.exists("MISC/ITEM03.BIN")
            if kind_ == 0:
                sf_ = ai.s8(e.r8(rt(ai.G_CRATE_DROPS) + 4 * k_)) & 0xFF
                names_ = [f"CRATE{sf_:02d}", f"ITEM{sf_:02d}"]
            else:
                sf_ = e.r8(rt(ai.G_CUR_SECTOR))
                names_ = [f"DEADPL{sf_:02d}", f"LIVEPL{sf_:02d}", f"ITEM{sf_:02d}"]
            nrec_ = rng.choice([1, 2, 5, 12])
            for nm_ in names_:
                d_ = bytearray()
                for _ in range(nrec_):
                    t_ = rng.randrange(1, 3136)
                    d_ += t_.to_bytes(2, "little")
                    if nm_.startswith("ITEM"):
                        d_ += bytes([rng.randrange(1, 136), rng.choice([1, 2]), rng.randrange(101), 0, 0, 0,
                                     rng.choice([0xFF, 0, 3])]) + bytes(5)
                        if rng.random() < 0.5:      # the tile already holds a ground item
                            g_ = rng.randrange(200)
                            e.w8(rt(G_TILEMAP) + 8 * t_ + 6, g_)
                            e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_, rng.randrange(1, 136))
                            e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_ + 6, rng.choice([0xFF, 0, 3]))
                    else:
                        d_ += rng.choice([0x9E, 0xA0, 0xAA, 0x9D, rng.randrange(256)]).to_bytes(2, "little")
                cp_ = f"MISC/{nm_}.BIN"
                fake_[cp_] = (CD.cache.get(cp_), cp_ in CdFiles.names)
                CD.cache[cp_] = bytes(d_)
                CdFiles.names.add(cp_)
            if kind_ == 0:
                e.w8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 2, nrec_)
                e.w8(rt(ai.G_CUR_SECTOR), sf_)
                e.w8(rt(ai.G_SECTOR_TBL9) + 9 * sf_, rng.choice([0, 0, 0, 1, 0xFE, 0xFF]))
                dd_ = ai.s8(e.r8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 1))
                st_, sh_, dl_ = rng.choice([(0, 0, 0), (1, 0, 0), (4, 0xFF, 1), (3, 0xFF, 1), (3, 0, 1), (3, 1, 1)])
                e.w8(rt(ai.G_CRATE_STATE) + k_, st_)
                e.w8(rt(ai.G_CRATE_SHOWN) + k_, sh_)
                e.w8(rt(ai.G_CRATE_DROPS) + 4 * k_ + 3, rng.choice([0, 0, 1]))
                e.w32(rt(ai.G_DAY), (dd_ + dl_ * rng.choice([1, 3])) & 0xFFFFFFFF)
            else:
                for pk_ in range(1, 4):
                    if e.r32(rt(ai.G_PLANTS) + 12 * pk_ + 4) == sf_:
                        e.w8(rt(ai.G_PLANTS) + 12 * pk_ + 9, nrec_)
                        e.w8(rt(ai.G_PLANTS) + 12 * pk_ + 10, nrec_)
        for r_ in range(60):            # the owned bit of the other sector rows (a wrong row stride reads them)
            if rng.random() < 0.5:
                e.w8(rt(ai.G_SECTOR_TBL9) + 9 * r_, e.r8(rt(ai.G_SECTOR_TBL9) + 9 * r_) ^ 1)
        if kind_ >= 3 and rng.random() < 0.2:   # a plant sector word with high bytes set
            e.w8(rt(ai.G_PLANTS) + 12 * rng.randrange(1, 4) + rng.randrange(5, 8), rng.randrange(1, 256))
        if kind_ == 2:          # the fixed item table (read only in the game; restored per case through REG)
            for f_ in range(11):
                if rng.random() < 0.5:
                    fb_ = rt(ai.FIXED_ITEMS) + 8 * f_
                    e.w16(fb_, rng.randrange(1, 3136))
                    e.w8(fb_ + 2, rng.choice([0, rng.randrange(1, 136)]))
                    e.w8(fb_ + 4, rng.randrange(6))
                    e.w8(fb_ + 6, rng.randrange(101))
        jr_ = random.Random(i)          # (own sequence: the shared rng and the later sections stay as they were)
        if jr_.random() < 0.1:          # a full ground list: Put item finds no entry and writes 200 into tile byte 6
            for g_ in range(200):
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_, jr_.randrange(1, 136))
        if PORT:          # ground entries of type 136 or more (left by earlier sections; never in the game): the
            for g_ in range(200):       # original reads past the item table for them
                if e.r8(rt(ai.G_GROUND_ITEMS) + 12 * g_) >= 136:
                    e.uc.mem_write(rt(ai.G_GROUND_ITEMS) + 12 * g_, bytes(12))
        # the bytes past entry 199 (read by a later Put item through a tile byte 6 of 200): earlier sections leave other
        # values in the full run
        e.uc.mem_write(rt(ai.G_GROUND_ITEMS) + 12 * 200, bytes(12 * 56) if jr_.random() < 0.5
                       else bytes(jr_.randrange(256) for _ in range(12 * 56)))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"Random": [rng.randrange(0x7FFF) for _ in range(60)], "w_3fc04": [0] * 20, "replay_event": [],
                 "w_3c05c": [], **{k__: [] for k__ in CdFiles.CALLS}}
        addr_, args_, fn_ = ((0x62360, (), lambda w__: ai.crate_drops(w__)),
                             (0x62780, (arg_,), lambda w__: ai.crate_lost(w__, arg_)),
                             (0x2ACAC, (), lambda w__: ai.fixed_items(w__)),
                             (0x62848, (), lambda w__: ai.plant_sector(w__)),
                             (0x629AC, (), lambda w__: ai.plant_done(w__)))[kind_]
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        e.call(rt(addr_), *args_)
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        if PORT:
            port_sector(("crate_drops", "crate_lost", "fixed_items", "plant_sector", "plant_done")[kind_], args_)
        else:
            fn_(CdWorld(0))
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        name_ = ("crate_drops", "crate_lost", "fixed_items", "plant_sector", "plant_done")[kind_]
        counts[name_] = counts.get(name_, 0) + 1
        key_ = sum(1 for c_ in log if c_[0] == "open_cd")
        results.setdefault(name_, {}).setdefault(key_, 0)
        results[name_][key_] += 1
        for cp_, (v_, had_) in fake_.items():
            if v_ is None:
                CD.cache.pop(cp_, None)
            else:
                CD.cache[cp_] = v_
            if not had_:
                CdFiles.names.discard(cp_)
        if (got_g, got_log) != (want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [(hex(REG[j_][0]), [(hex(REG[j_][0] + o_), snap[j_][o_], got_g[j_][o_], want_g[j_][o_])
                                           for o_ in range(REG[j_][1]) if got_g[j_][o_] != want_g[j_][o_]][:8])
                        for j_ in range(len(REG)) if got_g[j_] != want_g[j_]]     # (address, before, original, port)
                print(f"MISMATCH {name_} arg {arg_}: {bad_}\n  calls original {got_log}\n  calls port     {log}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    CD.remove(cdsave_)
    for a_, v_ in held_.items():
        e.stubs[rt(a_)] = v_
if direct("patches"):   # ApplyTileObjPatch 0x317EC / ApplyItemPatch 0x318C4 on fuzzed MISC files: called directly
    cdsave_ = CD.install()
    held_ = {a_: e.stubs.pop(rt(a_)) for a_ in (0x53708, 0x3FC78) + ((0x3C05C, 0x3FC04) if PORT else ()) if rt(a_) in e.stubs}
    REG = [(G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (ai.G_GROUND_ITEMS, 12 * 256), (0x92DA0, 0x1C0),
           (combat.G_MOVECODE, 2 * 3136), (ai.G_ITEM_BUF, 12), (0xEB000, 0x160), (ai.G_NOTES_FOUND, 30)]
    if PORT:          # the map tiles only: with no tile found Put item also writes byte 6 of tile 0xC41 (batch 3)
        REG[0] = (G_TILEMAP, 8 * 3136)
    else:             # tile -1: its move code lies on the last 2 bytes of g_t1Class (later MoveCodes read them)
        REG[3] = (combat.G_MOVECODE - 2, 2 * 3137)
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    CD.exists("MISC/ITEM03.BIN")
    TILES_ = [0, 1, 0xC3F, 0xC40, 0xC41, 0x7FFF, 0xFFFF]     # (no tile below -1: the original indexes far off the map)
    if PORT:          # negative tiles do not occur in the game files (sector_start.md batch 6)
        TILES_.remove(0xFFFF)
    for i in range(n // 6):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        kind_ = i % 2
        size_ = 4 if kind_ == 0 else 14
        nrec_ = rng.choice([0, 1, 2, 5, 12])
        data_ = bytearray()
        for _ in range(nrec_):
            t_ = rng.choice(TILES_ + [rng.randrange(1, 3136)] * 6)
            data_ += t_.to_bytes(2, "little")
            if kind_ == 0:
                data_ += rng.choice([0x9D, 0x9E, 0xAA, 0xAB, 0x19E, 0xFF9E, rng.randrange(0x9E, 0xAB),
                                     rng.randrange(0x10000)]).to_bytes(2, "little")
            else:
                it_ = bytearray(rng.randrange(256) for _ in range(12))
                it_[0] = rng.choice([0, it_[0], it_[0], 0x59, 0x5A, rng.randrange(0x60, 0x6E)])   # notes: placed once
                if PORT:      # the port's item_types has the game's 136 types (135: its sink flag lies past them)
                    it_[0] %= 135
                data_ += it_
            if 0 < t_ < 3136 and rng.random() < 0.3:
                e.w8(rt(G_TILEMAP) + 8 * t_ + rng.choice([5, 7]), 0xFF)
            if rng.random() < 0.3:      # its tile holds a ground item
                if 0 < t_ < 3136:
                    g_ = rng.randrange(200)
                    e.w8(rt(G_TILEMAP) + 8 * t_ + 6, g_)
                    e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_, rng.choice([0, rng.randrange(1, 136)]))
        e.w8(rt(ai.G_GROUND_ITEMS) + 12 * 0xFF, rng.choice([0, rng.randrange(1, 136)]))   # entry 0xFF = "no item"
        for k_ in range(15):        # one-off notes placed before or not (the second PutItem of a record skips a placed one)
            e.w8(rt(ai.G_NOTES_FOUND) + 2 * k_ + 1, rng.choice([0, 1]))
        if rng.random() < 0.3 and nrec_ >= 2:   # a short last record (the stack words keep the old bytes)
            data_ = data_[:len(data_) - rng.randrange(1, size_)]
            if len(data_) % size_ == 1 and data_[-size_] == 0xFF:   # one tile byte below an old high byte 0xFF:
                data_[-1] = 0xFF                # tile -1, not a tile below -1 (the original reads far off the map)
        path_ = rng.choice(["MISC\\FUZZ.BIN"] * 6 + ["MISC\\NOPE.BIN"])
        CD.cache["MISC/FUZZ.BIN"] = bytes(data_)
        CdFiles.names.add("MISC/FUZZ.BIN")
        if PORT:          # the port names a patch file by kind and number: numbers 90..99 are not on the CD
            pk_, pn_ = rng.randrange(4), rng.randrange(90, 100)
            path_ = f"MISC\\{PATCH_KINDS[pk_]}{pn_:02d}.BIN"
            cp_ = path_.replace("\\", "/")
            if rng.random() < 6 / 7:
                CD.cache[cp_] = bytes(data_)
                CdFiles.names.add(cp_)
            else:
                CD.cache.pop(cp_, None)
                CdFiles.names.discard(cp_)
        cnt_ = rng.choice([nrec_, nrec_, max(nrec_ - 1, 0), 0, 0xFFFFFFFF, 0x80000000] + [nrec_ + 1, nrec_ + 3] * (nrec_ > 0))
        # (no first record at all: the original reads uninitialised stack words, not defined behaviour)
        bl_ = rng.choice([0, 1, 0x80, rng.randrange(256)]) | rng.choice([0, 0x4400])
        cl_ = rng.choice([0, 1, 0x80, rng.randrange(256)]) | rng.choice([0, 0x6600])
        for k_, ch_ in enumerate(path_.encode() + b"\0"):
            e.w8(rt(ai.G_TEXT_BUF) + k_, ch_)
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"Random": [rng.randrange(0x7FFF) for _ in range(20)], "w_3fc04": [0] * 20, "w_3c05c": [],
                 **{k__: [] for k__ in CdFiles.CALLS}}
        addr_, fn_ = ((0x317EC, ai.tile_patch), (0x318C4, ai.item_patch))[kind_]
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        got_r = e.call(rt(addr_), rt(ai.G_TEXT_BUF), cnt_, bl_, cl_)
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        if PORT:
            want_r = port_sector(("tile_patch", "item_patch")[kind_], (pk_, pn_, ai._i32(cnt_), bl_, cl_)) & 0xFFFFFFFF
        else:
            want_r = fn_(CdWorld(0), path_, cnt_, bl_, cl_) & 0xFFFFFFFF
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        name_ = ("tile_patch", "item_patch")[kind_]
        counts[name_] = counts.get(name_, 0) + 1
        key_ = want_r
        results.setdefault(name_, {}).setdefault(key_, 0)
        results[name_][key_] += 1
        if (got_r, got_g, got_log) != (want_r, want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [(hex(REG[j_][0]), [hex(REG[j_][0] + o_) for o_ in range(REG[j_][1]) if got_g[j_][o_] != want_g[j_][o_]][:8])
                        for j_ in range(len(REG)) if got_g[j_] != want_g[j_]]
                print(f"MISMATCH {name_} count {cnt_:#x} bl {bl_:#x} cl {cl_:#x} returns {got_r}/{want_r}: {bad_} file {bytes(data_).hex()[:56]}\n"
                      f"  calls original {got_log[:10]}\n  calls port     {log[:10]}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    CD.remove(cdsave_)
    for a_, v_ in held_.items():
        e.stubs[rt(a_)] = v_
if direct("ground_blast"):   # 0x355D0 ground items of one type taken off / set off (+ 0x629AC): called directly
    REG = [(G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (ai.G_GROUND_ITEMS, 12 * 256), (0x229C80, 0x880), (ai.G_BOMB_COUNT, 4),
           (ai.G_SECTORS, 60 * 0xB1), (ai.G_PLANTS, 48)]
    if PORT:          # the map tiles only; Take ground item runs for real on both sides (move codes, blast list)
        REG[0] = (G_TILEMAP, 8 * 3136)
        REG += [(combat.G_MOVECODE, 2 * 3136), (ai.G_BLAST_LIST, 60), (ai.G_HELD, 12)]
    held_ = {a_: e.stubs.pop(rt(a_)) for a_ in ((0x3C05C, 0x3FC04) if PORT else ()) if rt(a_) in e.stubs}
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    stub355_ = e.stubs.pop(rt(0x355D0))
    TYPES_ = [0x11, 0x11, 0x29, 0x3F, 0x3F, 0x6E, 0x71, 0x13, 0x10, 0x12, 0x40, 0x6D, 0x6F, 0x70, 0x72, 0xFF]
    for i in range(n // 6):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        typ_ = rng.choice(TYPES_ + [rng.randrange(256)])
        for g_ in range(256):       # ground item records: about a third hold the type
            if rng.random() < 0.3:
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_, typ_)
            if rng.random() < 0.3:
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_ + 1, rng.choice([0, 1, 5, 0x7F, 0x80, 0xFF, rng.randrange(256)]))
        dens_ = rng.choice([0.002, 0.01, 0.05])
        for t_ in range(3136):
            if rng.random() < dens_:
                e.w8(rt(G_TILEMAP) + 8 * t_ + 6, rng.choice([0xFF, 0xFE, 200, rng.randrange(256)]))
        cur_ = rng.choice([e.r8(rt(ai.G_PLANTS) + 4 + 12 * rng.randrange(1, 4))] * 3 + [rng.randrange(60)])
        plant_fuzz_(cur_)
        for k_ in range(4):
            if rng.random() < 0.5:
                e.w8(rt(ai.G_PLANTS) + 12 * k_ + 8, rng.choice([3, 3, 2, 0]))
        e.w8(rt(ai.G_SECTORS) + 0xB1 * cur_ + 0xB0, rng.choice([0, 1, 1, 2, 3, 5, 0xFF]))
        e.w32(rt(ai.G_BOMB_COUNT), rng.choice([0, 0, 1, 2, 3, 7, 0xFFFFFFFF]))
        flag_ = rng.choice([0, 0, 1, 2, 0x80, 0xFF, rng.randrange(256)]) | rng.choice([0, 0x1200])
        power_ = rng.choice([0, 50, 99, 100, 101, 0xFFFFFFF6, 0x7FFFFFFF, rng.randrange(1 << 32)])
        thrower_ = rng.choice([0x40, rng.randrange(0x30), rng.randrange(256)]) | rng.choice([0, 0x3300])
        last_ = rng.choice([0, 1, 0x80, 0xFF, rng.randrange(1 << 32)])
        typ_arg_ = typ_ | rng.choice([0, 0, 0x5500])
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"Random": [rng.randrange(0x7FFF) for _ in range(4)], "w_3c05c": [], "explode": [], "fx_157bc": [],
                 "replay_event": []}
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        e.call(rt(0x355D0), typ_arg_, flag_, power_, thrower_, last_)
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        if PORT:
            port_sector("ground_blast", (typ_arg_, flag_, power_, thrower_, last_))
        else:
            ai.ground_blast(World(0), typ_arg_, flag_, power_, thrower_, last_)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["ground_blast"] = counts.get("ground_blast", 0) + 1
        key_ = (typ_, sum(1 for c_ in log if c_[0] in ("explode", "w_3c05c")), sum(1 for c_ in log if c_[0] == "fx_157bc"))
        results.setdefault("ground_blast", {}).setdefault(key_, 0)
        results["ground_blast"][key_] += 1
        if (got_g, got_log) != (want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [(hex(REG[j_][0]), [hex(REG[j_][0] + o_) for o_ in range(REG[j_][1]) if got_g[j_][o_] != want_g[j_][o_]][:8])
                        for j_ in range(len(REG)) if got_g[j_] != want_g[j_]]
                print(f"MISMATCH ground_blast type {typ_:#x} flag {flag_:#x} power {power_:#x}: {bad_}\n"
                      f"  calls original {got_log[:8]}\n  calls port     {log[:8]}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.stubs[rt(0x355D0)] = stub355_
    for a_, v_ in held_.items():
        e.stubs[rt(a_)] = v_
if direct("tmp_file"):   # SectorTmpFile 0x2BCF0: a sector's saved tiles + ground items from sec%02d.tmp: called directly
    cdsave_ = CD.install()
    REG = [(G_TILEMAP, 8 * 3136), (ai.G_GROUND_ITEMS, 0x960)] + ([] if PORT else [(ai.G_DAT_FILE, 4)])
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    gsave_ = e.r8(rt(ai.G_CUR_SECTOR))
    for i in range(max(n // 60, 4)):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        e.w8(rt(ai.G_CUR_SECTOR), rng.choice([gsave_, 0, 5, 59, rng.randrange(256)]))
        CD.tmp = bytes(rng.randrange(256) for _ in range(5 * 3136 + 0x960))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {**{k__: [] for k__ in CdFiles.CALLS}, "fopen": [rng.choice([0, 0x3F00 + rng.randrange(256), 0xFFFFFFFF])]}
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        got_r = e.call(rt(0x2BCF0))
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        want_r = port_sector("tmp_file", ()) if PORT else ai.tmp_file(CdWorld(0))
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["tmp_file"] = counts.get("tmp_file", 0) + 1
        results.setdefault("tmp_file", {}).setdefault(want_r, 0)
        results["tmp_file"][want_r] += 1
        if (got_r & 0xFFFFFFFF, got_g, got_log) != (want_r & 0xFFFFFFFF, want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [(hex(REG[j_][0]), [hex(REG[j_][0] + o_) for o_ in range(REG[j_][1]) if got_g[j_][o_] != want_g[j_][o_]][:8])
                        for j_ in range(len(REG)) if got_g[j_] != want_g[j_]]
                print(f"MISMATCH tmp_file result {got_r:#x} / {want_r:#x}: {bad_}\n  calls original {got_log[:6]}\n"
                      f"  calls port     {log[:6]}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.w8(rt(ai.G_CUR_SECTOR), gsave_)
    CD.remove(cdsave_)
if direct("shot_ap"):   # ShotApCost 0x564F8 (EAX soldier, EDX tile): AP of one shot with the main-hand gun: called directly
    stub_sa_ = e.stubs.pop(rt(0x564F8))
    GUNS_ = [t_ for t_ in range(1, 0x11) if e.r8(rt(ai.G_WEAPON_B5 - 2) + 6 * t_)]    # weapon byte 3 != 0 (else a crash)
    REG = [(G_SOLDIERS, 48 * 0x200)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    for i in range(max(n // 4, 100)):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        slot = rng.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0] = rng.choice(GUNS_ * 4 + [0, 0x11, 0x20, 0xFF])
        for o_ in (0x18B, 0x18C, 0x191):
            s[o_] = rng.choice([0, 1, 30, 60, 99, 100, 0x7F, 0x80, 0xFF, rng.randrange(256)])
        last_ = rng.choice([ai.NO_TILE, rng.randrange(3136), 0x8000])
        ai.w16(s, 0xD0, last_)
        tile_ = rng.choice([ai.s16(last_), ai.NO_TILE, rng.randrange(3136), last_ | 0x10000, -1])
        for o_ in (0x183, 0x184, 0x18A, 0x1A5):      # health, energy, AP: MercMaxAP inputs
            s[o_] = rng.choice([s[o_], 1, 15, 50, 100, rng.randrange(256)])
        e.uc.mem_write(sp, bytes(s))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        script = {}
        log.clear()
        try:
            got_r = e.call(rt(0x564F8), sp, tile_ & 0xFFFFFFFF) & 0xFF
        except emu.EmuStop:
            got_r = None                  # MercMaxAP divides by zero for some records: the port reports div_zero
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        log.clear()
        try:
            want_r = (port_sector("shot_ap", (slot, tile_)) if PORT else ai.shot_ap(MemRec(slot), World(slot), tile_)) & 0xFF
        except ZeroDivisionError:
            want_r = None
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["shot_ap"] = counts.get("shot_ap", 0) + 1
        results.setdefault("shot_ap", {}).setdefault(want_r, 0)
        results["shot_ap"][want_r] += 1
        if (got_r, got_g, got_log) != (want_r, want_g if got_r is not None else got_g, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH shot_ap type {s[0]:#x} tile {tile_} last {last_:#x} result {got_r} / {want_r}\n"
                      f"  calls original {got_log[:6]}\n  calls port     {log[:6]}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.stubs[rt(0x564F8)] = stub_sa_
if direct("action_ap"):   # ActionApCost 0x56310 (EAX soldier, EDX tile, BL flag): the AP of using the hand item: called directly
    stub_aa_ = e.stubs.pop(rt(0x56310))
    aa_stop_ = 0
    stub_sa2_ = e.stubs.pop(rt(0x564F8)) if PORT else None    # port: shot_ap is a library op (batch 13)
    KINDS_ = {}
    for t_ in range(136):
        KINDS_.setdefault(e.r8(rt(ai.G_ITEM_KIND) + 12 * t_), []).append(t_)
    REG = [(G_SOLDIERS, 48 * 0x200), (ai.G_MISSION_SPOT, 2)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    for i in range(max(n // 4, 100)):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        slot = rng.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        kind_ = rng.choice([0x92, 0x92, 0x95, 0x96, 0x96] + [k_ for k_ in KINDS_ if k_ not in (0x92, 0x95, 0x96)][:3])
        s[0] = rng.choice(KINDS_.get(kind_, [0]))
        me_ = rng.randrange(3136)
        ai.w16(s, 0x114, me_)
        tile_ = rng.choice([ai.NO_TILE, ai.NO_TILE, me_ + 1, me_ - 1, me_ + 56, me_ - 56, me_ + 57, me_ + 2,
                            rng.randrange(3136), 0xC40, 0xC42, -1, 0x7FFF])
        flag_ = rng.choice([0, 1, 0x100, 0x80])
        s[0x1F3] = rng.randrange(4)
        ai.w16(s, 0x1E8 + 2 * s[0x1F3], rng.choice([0x1B, 0x1F, 0x23, 0x1E, 0x20, 0x22, 0x1A, 4, 0x24]))
        s[0x1A1] = rng.choice([1, 2, 3, 4, 5, 6, 7, 8, 0x81])
        e.uc.mem_write(sp, bytes(s))
        e.w16(rt(ai.G_MISSION_SPOT), rng.randrange(3136))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"shot_ap": [rng.choice([1, 4, 5, 0xFF, 0x1FE, 0xFFFFFFFF])], "tile_dist": [rng.choice([0, 1, 1, 2, 5, -1])],
                 "reach_ap": [rng.choice([0, 1, 0x100])], "path_ap": [rng.choice([0, 3, 0xFE, 0x1FF])],
                 "direction": [rng.choice([s[0x1A1], s[0x1A1], rng.randrange(1, 9), s[0x1A1] | 0x100])]}
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        try:
            got_r = e.call(rt(0x56310), sp, tile_ & 0xFFFFFFFF, flag_) & 0xFF
        except emu.EmuStop:               # MercMaxAP divides by zero (a record with energy 0): the game crashes
            aa_stop_ += 1
            continue
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        if PORT:
            want_r = port_sector("action_ap", (slot, tile_, flag_), port_lib.SECTOR_LIB_OPS) & 0xFF
        else:
            w_ = World(slot)
            want_r = ai.action_ap(MemRec(slot), w_, tile_, flag_) & 0xFF
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["action_ap"] = counts.get("action_ap", 0) + 1
        results.setdefault("action_ap", {}).setdefault(kind_, 0)
        results["action_ap"][kind_] += 1
        if (got_r, got_g, got_log) != (want_r, want_g, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH action_ap kind {kind_:#x} tile {tile_} flag {flag_:#x} result {got_r:#x} / {want_r:#x}\n"
                      f"  calls original {got_log[:6]}\n  calls port     {log[:6]}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.stubs[rt(0x56310)] = stub_aa_
    if aa_stop_:
        print(f"action_ap: {aa_stop_} cases skipped (the original divides by zero in MercMaxAP)")
    if PORT:
        e.stubs[rt(0x564F8)] = stub_sa2_
if direct("quest_check"):   # QuestCheck? 0x633D8 (quest 0x1C9: sector 0x0B story state, safe, items 0x6E / 0x7C): called directly
    stub_qc_ = e.stubs.pop(rt(0x633D8))
    REG = [(0x22A040, 32), (ai.G_SAFE_OPENED, 1), (G_SOLDIERS, 8 * 0x200), (ai.G_CUR_SECTOR, 1)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    for i in range(max(n // 6, 50)):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        e.w8(rt(ai.G_CUR_SECTOR), rng.choice([0xB] * 6 + [0, 0xA, 0x8B, rng.randrange(60)]))
        quest_ = rng.choice([0x1C9] * 6 + [0x101C9, 0xFFFF01C9, 0x1C8, 0x1CA, 0x81C9, rng.randrange(0x10000)])
        e.w8(rt(0x22A04E), rng.choice([1, 1, 1, 0, 2, 0x81]))
        for a_ in (ai.G_SAFE_OPENED, 0x22A05C, 0x22A05E):
            e.w8(rt(a_), rng.choice([0, 0, 0, 0, 1, 0x80]))
        for k_ in range(8):
            r_ = rt(G_SOLDIERS) + 0x200 * k_
            e.w8(r_ + 0x182, rng.choice([0, 1, 1]))
            for j_ in range(12):
                e.w8(r_ + 12 * j_, rng.choice([0, 0, 1, 0x6D, 0x6F, 0x7B, 0x7D]))
        if rng.random() < 0.5:        # every other condition holds: only the items decide
            e.w8(rt(ai.G_CUR_SECTOR), 0xB)
            quest_ = rng.choice([0x1C9, 0x101C9])
            for a_, v_ in ((0x22A04E, 1), (ai.G_SAFE_OPENED, 0), (0x22A05C, 0), (0x22A05E, 0)):
                e.w8(rt(a_), v_)
        if rng.random() < 0.4:        # one 0x6E / 0x7C item somewhere (a merc not in the sector carries it too)
            e.w8(rt(G_SOLDIERS) + 0x200 * rng.randrange(8) + 12 * rng.randrange(12), rng.choice([0x6E, 0x7C]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        script = {}
        log.clear()
        got_r = e.call(rt(0x633D8), quest_) & 0xFF
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        log.clear()
        want_r = (port_sector("quest_check", (ai._i32(quest_),)) if PORT else ai.quest_check(World(0), quest_)) & 0xFF
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["quest_check"] = counts.get("quest_check", 0) + 1
        results.setdefault("quest_check", {}).setdefault(want_r, 0)
        results["quest_check"][want_r] += 1
        if (got_r, got_g, got_log) != (want_r, want_g, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH quest_check quest {quest_:#x} result {got_r:#x} / {want_r:#x}\n"
                      f"  calls original {got_log[:6]}\n  calls port     {log[:6]}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.stubs[rt(0x633D8)] = stub_qc_
if direct("radio_result"):   # RadioResult? 0x63254: the mission step after a radio call (mission target, story, plants): called directly
    G_SECTORS_ = 0x222B74
    REG = [(0x92DA8, 4), (G_TILEMAP, 8 * 3136), (ai.G_GROUND_ITEMS, 12 * 256), (0x22A040, 32), (ai.G_PLANTS, 48),
           (ai.G_MISSION_DONE, 80), (G_SECTORS_, 60 * 0xB1), (G_SOLDIERS, 48 * 0x200), (ai.G_CUR_SECTOR, 1)]
    stub_rr_ = e.stubs.pop(rt(0x63254))
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    for i in range(max(n // 6, 50)):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        cur_ = rng.choice([0xB, 0xB, 0, 5, 59, rng.randrange(60)])
        e.w8(rt(ai.G_CUR_SECTOR), cur_)
        e.w16(rt(0x92DAA), rng.randrange(0x10000))
        for _ in range(rng.choice([0, 0, 1, 2, 4])):      # ground items of type 0x72 (the radio target) on the map
            t_, g_ = rng.randrange(3136), rng.choice([rng.randrange(200), 0xFF] + ([] if PORT else [0xFE]))
            e.w8(rt(G_TILEMAP) + 8 * t_ + 6, g_)
            if g_ < 0xFF:
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_, rng.choice([0x72, 0x72, 0x71, 0]))
        if rng.random() < 0.3:
            for t_ in range(3136):
                if rng.random() < 0.5:
                    e.w8(rt(G_TILEMAP) + 8 * t_ + 6, 0xFF)
        e.w8(rt(0x22A04B), rng.choice([1, 1, 0, 2, 0x81]))
        e.w8(rt(0x22A057), rng.choice([cur_, cur_, rng.randrange(256)]))
        e.w8(rt(0x22A055), rng.choice([0, 0, 1]))
        e.w8(rt(0x22A056), rng.choice([0, 0, 1]))
        sec_ = rt(G_SECTORS_) + 0xB1 * cur_
        for k_ in range(5):
            e.w8(sec_ + 0x9F + k_, rng.choice([0, 0xAE, 0xAD, 0xAF, rng.randrange(256)]))
            e.w16(sec_ + 0x82 + 2 * k_, rng.randrange(0x10000))
        for k_ in range(4):
            p_ = rt(ai.G_PLANTS) + 12 * k_
            e.w32(p_ + 4, rng.choice([cur_, rng.randrange(60), cur_ | 0x100, 0xFFFFFFFF]) if rng.random() < 0.4
                  else rng.randrange(60, 80))
            e.w8(p_ + 8, rng.choice([3, 3, 0, 2, 0x83]))
        e.w8(rt(ai.G_MISSION_DONE) + cur_, rng.choice([0, 0, 1, 0x80]))
        for k_ in list(range(8)) + list(range(0x28, 0x30)):     # the loop reads the enemies 0x28..0x2F
            r_ = rt(G_SOLDIERS) + 0x200 * k_
            for o_ in (0x182, 0x183, 0x188):
                e.w8(r_ + o_, rng.choice([0, 1, 1, 1, rng.randrange(256)]) if o_ != 0x188 else rng.choice([0, 0, 1]))
            for o_ in (0, 0xC):
                e.w8(r_ + o_, rng.choice([0x40, 0, 0, 1, 0x3F]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"quest_check": [rng.choice([0, 1, 0x100, 0xFFFFFF01])]}
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        got_r = e.call(rt(0x63254)) & 0xFF
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        want_r = (port_sector("radio_result", ()) if PORT else ai.radio_result(World(0))) & 0xFF
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["radio_result"] = counts.get("radio_result", 0) + 1
        results.setdefault("radio_result", {}).setdefault(want_r, 0)
        results["radio_result"][want_r] += 1
        if (got_r, got_g, got_log) != (want_r, want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [(hex(REG[j_][0]), [hex(REG[j_][0] + o_) for o_ in range(REG[j_][1]) if got_g[j_][o_] != want_g[j_][o_]][:8])
                        for j_ in range(len(REG)) if got_g[j_] != want_g[j_]]
                print(f"MISMATCH radio_result cur {cur_:#x} result {got_r:#x} / {want_r:#x}: {bad_}\n"
                      f"  calls original {got_log[:6]}\n  calls port     {log[:6]}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.stubs[rt(0x63254)] = stub_rr_
if PORT and direct("mission_cancel"):   # MissionCancel? 0x72670 (op 19): the enemy nearest the mission spot takes the mission order: called directly
    REG = [(0x92DA8, 4), (ai.G_MISSION_SPOT, 2), (G_SOLDIERS, 48 * 0x200)]
    stub_mc_ = e.stubs.pop(rt(0x72670))
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    for i in range(max(n // 6, 50)):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        e.w8(rt(0x92DA8), rng.choice([0x40, 0x40, 0x28, rng.randrange(256)]))
        e.w16(rt(0x92DAA), rng.choice([0xC41, rng.randrange(3136), rng.randrange(3136), rng.randrange(0x10000)]))
        spot_ = rng.choice([rng.randrange(3136), rng.randrange(0x10000)])
        e.w16(rt(ai.G_MISSION_SPOT), spot_)
        for k_ in range(0x28, 0x30):
            r_ = rt(G_SOLDIERS) + 0x200 * k_
            e.w8(r_ + 0x182, rng.choice([0, 1, 1, 1, 1, rng.randrange(256)]))
            e.w8(r_ + 0x138, rng.choice([0, 1, 1, 1, 1, rng.randrange(256)]))
            e.w8(r_ + 0x183, rng.choice([0, 14, 15, 30, 0x80, rng.randrange(256)]))
            e.w8(r_ + 0x188, rng.choice([0, 0, 0, 1]))
            e.w8(r_ + 0x18A, rng.choice([0x22, 0x23, 40, 0x90, rng.randrange(256)]))
            e.w8(r_ + 0x1D5, rng.choice([0, 0, 0xFF, 1, rng.randrange(256)]))
            e.w8(r_ + 0xB9, rng.randrange(256))
            e.w8(r_ + 0x1C1, rng.randrange(256))
            e.w16(r_ + 0x114, rng.choice([spot_, rng.randrange(3136)]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"reach_ap": [rng.choice([0, 1, 1, 0x100, 0xFFFFFFFF]) for _ in range(8)],
                 "path_ap": [rng.choice([0, 1, 5, 999, 1000, 1001, 0x8000, 0xFFFF, 0x10005, rng.randrange(1 << 32)])
                             for _ in range(8)]}
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        e.call(rt(0x72670))
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k__: list(v__) for k__, v__ in base_.items()}
        log.clear()
        if PORT:
            port_sector("mission_cancel", ())
        else:
            ai.mission_cancel(World(0))
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["mission_cancel"] = counts.get("mission_cancel", 0) + 1
        results.setdefault("mission_cancel", {}).setdefault(want_g[0][0], 0)
        results["mission_cancel"][want_g[0][0]] += 1
        if (got_g, got_log) != (want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [(hex(REG[j_][0]), [hex(REG[j_][0] + o_) for o_ in range(REG[j_][1]) if got_g[j_][o_] != want_g[j_][o_]][:8])
                        for j_ in range(len(REG)) if got_g[j_] != want_g[j_]]
                print(f"MISMATCH mission_cancel: {bad_}\n  calls original {got_log[:8]}\n  calls port     {log[:8]}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.stubs[rt(0x72670)] = stub_mc_
if direct("view"):   # 0x1491C merc view sweep (reveal, mine detector, spotted items, roofs): called directly
    REG = [(G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (combat.G_MOVECODE, 2 * 3136), (ai.G_GROUND_ITEMS, 12 * 256),
           (0x92DE3, 1), (ai.G_SPOT_STOP, 1), (ai.G_STORY_FLAG, 4)]
    GLOB_ = [(0x92DD0, [0, 0, 1]), (0x92DCC, [0, 1, 1]), (0x22A056, [0, 0, 1]), (ai.G_CUR_SECTOR, [0, 0, 5, 12, 0x80, 0x80, 0x9F])]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    gsave_ = [(a_, e.r8(rt(a_))) for a_, _ in GLOB_] + [(0x22A057, e.r8(rt(0x22A057)))]
    t2cls_ = bytes(e.uc.mem_read(rt(ai.G_T2_CLASS), 256))
    ROOFS_ = [v_ for v_ in range(256) if 0x12 <= t2cls_[v_] <= 0x14] or [0x53]
    OBJ_ = [0, 0, 0, 0x57, 0x86, 0xE2, 0x30, 0xAC, 0xAF, 0x90] + list(range(0x37, 0x83))
    for i in range(n // 3):
        slot = rng.choice([rng.randrange(8)] * 6 + [rng.randrange(8, 0x30)])
        if PORT and slot == 0x2F:          # its record lies before the map: the port setup fills those roof bytes
            slot = 0x2E
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        me_ = rng.choice([rng.randrange(3136), rng.randrange(56), rng.randrange(3080, 3136), 56 * rng.randrange(56)])
        ai.w16(s, 0x114, me_)
        ai.w16(s, 0x120, rng.choice([ai.NO_TILE, me_, rng.randrange(3136)]))
        facing_ = rng.choice(range(1, 9))
        range_ = rng.choice([0, 1, 2, 5, 9, 12, 20, rng.randrange(256)])
        s[0x177] = rng.choice([0, 1, 1])
        s[0] = rng.choice([0x4D, 0x4D, rng.randrange(256)])
        s[2] = rng.choice([9, 10, 50, 100, 0xFF, rng.randrange(256)])
        for f_ in (0x1D5, 0x16D):
            s[f_] = rng.choice([0, 0, 1])
        s[0x183] = rng.choice([14, 15, 50, 0x80])
        s[0x9D] = rng.randrange(256)
        for t_ in range(3136):
            if abs(t_ % 56 - me_ % 56) <= 13 and abs(t_ // 56 - me_ // 56) <= 13:
                b_ = rt(G_TILEMAP) + 8 * t_
                if rng.random() < 0.25:
                    e.w8(b_ + 1, rng.choice(OBJ_))
                if rng.random() < 0.3:
                    e.w8(b_ + 2, rng.choice([0x49, 0x4A, 0x4C, 0x4B, rng.randrange(256)]))
                if rng.random() < 0.4:
                    e.w8(b_ + 3, rng.choice(ROOFS_ + [0x73, 0, 0x52, 0x53, 0x54, 0x5D, 0x5E, 0x5F, 0x68, 0x69, 0x6A, rng.randrange(256)]))
                if rng.random() < 0.3:
                    e.w8(b_ + 6, rng.choice([0xFF, rng.randrange(200), rng.randrange(256), 199, 200]))
                if rng.random() < 0.7:
                    e.w8(b_ + 4, rng.randrange(0x80))
                if rng.random() < 0.05:
                    e.w16(rt(combat.G_MOVECODE) + 2 * t_, rng.choice([0x4B1, 0x4B1, rng.randrange(0x10000)]))
        for k_ in range(201):
            if rng.random() < 0.3:
                g_ = rt(ai.G_GROUND_ITEMS) + 12 * k_
                e.w8(g_, rng.choice([0x11, 0x11, rng.randrange(136)]))
                e.w8(g_ + 2, rng.choice([0, 0x80, 0xFF, 1, rng.randrange(256)]))
                e.w8(g_ + 6, rng.choice([0xFF, 0xFF, 0, 0, 1]))
        if rng.random() < 0.5:   # mines on the first steps ahead (the mine detector path)
            for d_ in ai.VIEW_DIRS[facing_]:
                t_ = ai.step_tile(ai.s16(ai.u16(s, 0x114)), ai.dir_step(World(slot), d_))
                if rng.random() < 0.6:
                    k_ = rng.randrange(200)
                    g_ = rt(ai.G_GROUND_ITEMS) + 12 * k_
                    e.w8(rt(G_TILEMAP) + 8 * t_ + 6, k_)
                    e.w8(rt(G_TILEMAP) + 8 * t_ + 1, rng.choice([0, 0, 0x40]))
                    e.w8(g_, rng.choice([0x11, 0x11, 0x13]))
                    e.w8(g_ + 2, rng.choice([0, 0x80, 0xFF, 1, rng.randrange(256)]))
                    e.w8(g_ + 6, rng.choice([0xFF, 0xFF, 0]))
        for a_, vals_ in GLOB_:
            e.w8(rt(a_), rng.choice(vals_))
        e.w8(rt(0x22A057), rng.choice([e.r8(rt(ai.G_CUR_SECTOR)), 0x85, rng.randrange(256)]))
        e.w32(rt(ai.G_STORY_FLAG), rng.choice([0, 1, 200]))
        if rng.random() < 0.2:
            me_ = 0x279 + rng.choice([56, -56, 1, -1, 2 * 56])
            ai.w16(s, 0x114, me_)
        e.uc.mem_write(sp, bytes(s))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"Random": [rng.choice([0, 9, 10, 49, 50, 99, rng.randrange(100)]) for _ in range(60)],
                 "w_1f6e4": [rng.choice([0, 0, 1, 0x100]) for _ in range(40)],
                 **{k_: [0] * 40 for k_ in ("quote_event", "w_197d8", "w_1148c", "stat_practice", "mine_flag", "stop_anim")}}
        if PORT:      # rows 0 / 55: the original reads (and may write) roofs past the map, the port reads 0 there
            for j_ in range(56):
                e.w8(rt(G_TILEMAP) + 8 * (j_ - 56) + 3, 0xFF)
                e.w8(rt(G_TILEMAP) + 8 * (3136 + j_) + 3, 0)
            snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
            unst_ = {a_: e.stubs.pop(rt(a_)) for a_ in (0x1F6E4, 0x197D8)}    # inline in the port
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        e.call(rt(0x1491C), facing_, range_, sp, 0)
        if PORT:
            e.stubs.update({rt(a_): f_ for a_, f_ in unst_.items()})
        got_s, got_g, got_log = bytes(e.uc.mem_read(sp, 0x200)), [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        e.uc.mem_write(sp, bytes(s))
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        ps = bytearray(s)
        if PORT:
            e.uc.mem_write(sp, bytes(s))
            port_merc_view(slot, facing_, range_)
            ps = bytearray(e.uc.mem_read(sp, 0x200))
        else:
            ai.reveal_view(facing_, range_, ps, World(slot), 0)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["view"] = counts.get("view", 0) + 1
        key_ = (slot < 8, min(sum(bin(got_g[0][j] ^ snap[0][j]).count("1") for j in range(4, len(snap[0]), 8)) // 10, 5),
                len(log) > 0)
        results.setdefault("view", {}).setdefault(key_, 0)
        results["view"][key_] += 1
        if (got_s, got_g, got_log) != (bytes(ps), want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [[hex(j) for j in range(len(want_g[r_])) if got_g[r_][j] != want_g[r_][j]][:8] for r_ in range(len(REG))]
                print(f"MISMATCH view slot {slot:#x} tile {me_:#x} facing {facing_} range {range_}: record "
                      f"{[hex(o) for o in range(0x200) if got_s[o] != ps[o]]} regions {bad_}"
                      f"\n  calls original {got_log}\n  calls port     {log}")
        e.uc.mem_write(sp, bytes(s))
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    for a_, v_ in gsave_:
        e.w8(rt(a_), v_)
if direct("move"):   # MoveMercTo? 0x1C608 (confirm click, squad recursion): called directly, port in memory
    REG = [(G_SOLDIERS, 0x200 * 0x30), (ai.G_PATHS, 0x50 * 0x30), (ai.G_PATH_SAVE, 0x50), (ai.G_CURSOR, 1),
           (ai.G_WALK_AP, 4), (0x229CA8, 8), (0x229D20, 4), (0x229D40, 4), (G_TILEMAP, 8 * 3136)]
    GLOB_ = [(ai.G_CONFIRM_MOVE, [0, 1, 1]), (0x1163C7, [0, 1]), (ai.G_COMBAT, [0, 1, 1]), (ai.G_ALT, [0, 1]),
             (ai.G_SHIFT, [0, 1]), (ai.G_CURSOR, [0, 0x8F, 0xA1])]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    gsave_ = [(a_, e.r8(rt(a_))) for a_, _ in GLOB_] + [(ai.G_SELECTED, e.r8(rt(ai.G_SELECTED)))]
    w_ = World(0)
    w_.memrun = True
    for i in range(n // 3):
        slot = rng.choice([rng.randrange(8)] * 4 + [rng.randrange(8, 0x30)])
        for t_ in rng.sample(range(3136), 300):
            e.w8(rt(G_TILEMAP) + 8 * t_ + 5, rng.choice([0x40, 0x40, rng.randrange(0x30), 0x80 | rng.randrange(0x30)]))
        for k_ in range(0x30):
            sp_ = rt(G_SOLDIERS) + k_ * 0x200
            if k_ != slot and rng.random() > 0.5:
                continue
            e.w8(sp_ + 0x182, rng.choice([0, 1, 1, 1, 1]))
            e.w8(sp_ + 0x183, rng.choice([14, 15, 50, 50, 0, 0x80]))
            e.w8(sp_ + 0x17E, rng.choice([0, 0, 0, 1]))
            e.w8(sp_ + 0x1F3, rng.randrange(4))
            e.w16(sp_ + 0x1E8 + 2 * e.r8(sp_ + 0x1F3), rng.choice([0, 1, 4, 0xD, 0x20, 0x3C, 0x3D, rng.randrange(0x3D)]))
            e.w8(sp_ + 0x19C, rng.choice([3, 3, 0, 1]))
            e.w8(sp_ + 0x1D5, rng.choice([0, 1, 0xFF]))
            e.w8(sp_ + 0xBA, rng.choice([2, 0, 1, 4]))
            e.w8(sp_, rng.choice([0, 3, 0xE, 0xE, 0xF, 0x10, 0x20, 0x4D]))
            e.w8(sp_ + 1, rng.choice([0, 1, 5]))
            e.w8(sp_ + 2, rng.choice([9, 10, 50, 0xFF]))
            e.w8(sp_ + 0x1E2, rng.choice([0, 5, 20]))
            e.w8(sp_ + 0x1A5, rng.choice([0, 3, 10, 30]))
            me_ = rng.randrange(56, 3080)
            e.w16(sp_ + 0x114, me_)
            e.w16(sp_ + 0xD0, rng.choice([ai.NO_TILE, me_ + rng.choice([1, -1, 56, 57]), rng.randrange(3136)]))
            for f_ in (0x177, 0x196, 0x197, 0x1A8, 0x1FA):
                e.w8(sp_ + f_, rng.randrange(256))
            e.w32(sp_ + 0xB0, rng.randrange(1 << 32))
            for f_ in (0x116, 0x118, 0x126, 0x128, 0xD4):
                e.w16(sp_ + f_, rng.randrange(0x10000))
            if rng.random() < 0.6 and e.r16(sp_ + 0xD0) < 3136:   # a living soldier on the target tile
                o_ = rng.choice([rng.randrange(0x30), rng.randrange(0x30), rng.randrange(0x30, 0x41)])
                e.w8(rt(G_TILEMAP) + 8 * e.r16(sp_ + 0xD0) + 5, o_ | rng.choice([0, 0x80]))
                e.w8(rt(G_SOLDIERS) + 0x200 * o_ + 0x183, rng.choice([1, 50, 0, 0x80]))
        e.uc.mem_write(rt(ai.G_PATHS), bytes(rng.randrange(8) for _ in range(0x50 * 8)))
        e.uc.mem_write(rt(ai.G_PATH_SAVE), bytes(rng.randrange(8) for _ in range(0x50)))
        for a_, vals_ in GLOB_:
            e.w8(rt(a_), rng.choice(vals_))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, slot, rng.randrange(8)]))
        e.w32(rt(ai.G_WALK_AP), rng.choice([0, 3, 10, 25, rng.randrange(1 << 32)]))
        sp = rt(G_SOLDIERS) + slot * 0x200
        tile = rng.choice([rng.randrange(3136), ai.NO_TILE, e.r16(sp + 0x114)])
        args_ = [rng.choice([0, 1, 1, 0x100]) for _ in range(6)]   # marker, sneak, b177, alt, confirm, squad
        if rng.random() < 0.3:
            args_[5] = 1
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"path_ap": [rng.choice([0, 1, 5, 12, 0xFFFFFFFF, 0x100]) for _ in range(20)],
                 "confirm_click": [rng.choice([0, 1, 1, 0x100])],
                 "tile_dist": [rng.choice([1, 1, 2, 0]) for _ in range(10)],
                 **{k_: [0] * 20 for k_ in ("merc_message", "message", "stop_walk", "w_17b70", "walk_to", "w_32eb8",
                                            "w_1c464", "merc_text")}}
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        if PORT:                           # the port has no confirm; PathApCost + FindPath real on both sides
            args_[4] = 0
            unst_ = {a_: e.stubs.pop(rt(a_)) for a_ in (0x1D32C, 0x40810)}
        got = ai._i32(e.call(rt(0x1C608), sp, tile, args_[0], args_[1], *args_[2:], max_steps=200_000_000))
        if PORT:
            e.stubs.update({rt(a_): f_ for a_, f_ in unst_.items()})
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        if PORT:
            want = ai._i32(port_move_order(slot, tile, args_))
        else:
            want = ai._i32(ai.move_merc(MemRec(slot), w_, tile, *args_))
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["move"] = counts.get("move", 0) + 1
        key_ = (want, len([c_ for c_ in log if c_[0] == "walk_to"]), args_[5] & 0xFF > 0)
        results.setdefault("move", {}).setdefault(key_, 0)
        results["move"][key_] += 1
        if (got, got_g, got_log) != (want, want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [[hex(j) for j in range(len(want_g[r_])) if got_g[r_][j] != want_g[r_][j]][:8] for r_ in range(len(REG))]
                print(f"MISMATCH move slot {slot:#x} tile {tile:#x} args {args_}: {got} / {want}; regions {bad_}"
                      f"\n  calls original {got_log}\n  calls port     {log}")
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
    for a_, v_ in gsave_:
        e.w8(rt(a_), v_)
if direct("loop"):   # TacticalLoop 0x107BC, its soldier part 0x10C5B..0x10F4E (run as a code range; callees stubbed)
    LOOP_CALLS = ("frame_active", "frame_other", "itq_pop", "w_12dd4", "w_12404", "new_turn", "projectile_step")
    for c_ in LOOP_CALLS:
        e.stub(rt(CALLEES[c_][0]), make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))
    frame_ = e.alloc(16)                  # [ebp - 4] = the loop's counter (NewTurn results are added to it)
    tramp_ = e.alloc(16)
    e.uc.mem_write(tramp_, b"\xBD" + struct.pack("<I", frame_ + 8) + b"\xE9" + struct.pack("<i", rt(0x10C5B) - (tramp_ + 10)))
    e.stub(rt(0x10F4E), lambda emu_, *a_: emu_.uc.emu_stop())
    REG = [(G_SOLDIERS, 0x200 * 0x30), (ai.G_SEEN_ENEMIES, 2), (ai.G_HITS, 1), (ai.G_SELECTED, 1), (0x229E04, 4)]
    GLOB_ = [(ai.G_COMBAT, [0, 1, 1]), (0x92DC8, [0, 3, 0x40]), (0x284F0F, [0, 7, 8, 0x40, 0x40]), (0x92DDC, [0, 1, 1]),
             (0x92DDD, [0, 1]), (ai.G_ACTIVE, [0x40, 0x40, 3, 0x2A, 0x3F])]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    fxsave_ = bytes(e.uc.mem_read(rt(ai.G_FX_ANIM), 16 * 0x12))
    gsave_ = [(a_, e.r8(rt(a_))) for a_, _ in GLOB_]
    w_ = World(0)
    w_.memrun = True
    for i in range(n // 3):
        for k_ in range(0x30):
            sp_ = rt(G_SOLDIERS) + k_ * 0x200
            e.w8(sp_ + 0x182, rng.choice([0, 1, 1, 1, 1]))
            e.w8(sp_ + 0x138, rng.choice([0, 1, 1, 1, 1]))
            e.w8(sp_ + 0x188, rng.choice([0, 0, 0, 0, 1]))
            e.w8(sp_ + 0x9C, rng.choice([0, 1, 0xFF, 5]))
            e.w8(sp_ + 0x195, rng.choice([0, 0, 0, 1, 0xFF]))
            e.w32(sp_ + 0xB0, rng.choice([0, 0, 1, 0xFFFFFFF6, 10, 50, rng.randrange(1 << 32)]))
            e.w8(sp_ + 0x1B0, rng.choice([0, 0, 1]))
            e.w16(sp_ + 0xD8, rng.randrange(0x10000))
            e.w16(sp_ + 0xDA, rng.randrange(0x10000))
            e.w8(sp_ + 0x17A, k_ if rng.random() < 0.95 else rng.randrange(0x41))
            e.w32(sp_ + 0xC4, rng.choice([0, 0, 10, 0xFFFFFFFF, 3]))
            e.w8(sp_ + 0x1D2, rng.choice([0, 1, 2, 4, 5, 9, 0x80, 0x81, 0xFC, rng.randrange(256)]))
            e.w32(sp_ + 0xBC, rng.choice([0, 1, 0xFFFFFFF6, 10]))
            e.w8(sp_ + 0x189, rng.choice([0, 1, 5, 0xFF]))
            e.w32(sp_ + 0xA8, rng.choice([0, 1, 0xFFFFFFF6, 10]))
            e.w8(sp_ + 0x1AC, rng.choice([0, 1, 2, 3, 0x13, 0x14, 0x15, 0x7F, 0x80, rng.randrange(256)]))
            e.w8(sp_ + 0x194, rng.choice([0, 0, 1]))
            e.w8(sp_ + 0x1A5, rng.choice([0, 3, 3]))
            e.w8(sp_ + 0x1BF, rng.choice([0, 0, 0, 3, 2]))
        for a_, vals_ in GLOB_:
            e.w8(rt(a_), rng.choice(vals_))
        sel_ = rng.choice([rng.randrange(8), rng.randrange(8, 0x30), 7, 8, 9, 0x2E, 0x2F, 0x2F, 0x30, 0x40])
        e.w8(rt(ai.G_SELECTED), sel_)
        if sel_ < 0x30 and rng.random() < 0.5:   # the soldier in turn is done: the hand-over runs
            e.w8(rt(G_SOLDIERS) + 0x200 * sel_ + 0x17A, sel_)
            e.w8(rt(G_SOLDIERS) + 0x200 * sel_ + 0x1B0, 1)
        e.w32(rt(0x229E04), rng.choice([0xFFFFFFFF, 0xFFFFFFFF, rng.randrange(16), 0xFFFFFFFE, 0]))
        e.w16(rt(ai.G_SEEN_ENEMIES), rng.randrange(0x10000))
        e.w8(rt(ai.G_HITS), rng.randrange(256))
        e.uc.mem_write(rt(ai.G_FX_ANIM), bytes(rng.choice([0, 0, 1, 0xFF, 5]) if j_ % 0x12 == 0 else rng.randrange(256)
                                           for j_ in range(16 * 0x12)))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"Random": [rng.randrange(1 << 16) for _ in range(20)],
                 "itq_pop": [rng.choice([0x40, 0x40, 3, 7, 8, 9, 0x2A, 0x3F, 0x140]) for _ in range(60)],
                 "new_turn": [rng.choice([0, 1, 2, 0x101, 0xFF]) for _ in range(60)],
                 **{k_: [0] * 60 for k_ in ("frame_active", "frame_other", "w_12dd4", "w_12404", "projectile_step",
                                            "portrait", "turn_banner", "fire")}}
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        e.w32(frame_ + 4, 0)
        e.call(tramp_)
        got = e.r8(frame_ + 4)
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v_) for k_, v_ in base_.items()}
        log.clear()
        want = port_world("frame") & 0xFF if PORT else ai.soldier_loop(w_)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["loop"] = counts.get("loop", 0) + 1
        key_ = tuple(sorted({c_[0] for c_ in log} & {"itq_pop", "new_turn", "fire", "projectile_step", "portrait"}))
        results.setdefault("loop", {}).setdefault(key_, 0)
        results["loop"][key_] += 1
        if (got, got_g, got_log) != (want, want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [[hex(j) for j in range(len(want_g[r_])) if got_g[r_][j] != want_g[r_][j]][:8] for r_ in range(len(REG))]
                print(f"MISMATCH loop: {got} / {want}; regions {bad_}\n  calls original {got_log}\n  calls port     {log}")
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
    e.uc.mem_write(rt(ai.G_FX_ANIM), fxsave_)
    for a_, v_ in gsave_:
        e.w8(rt(a_), v_)
    for c_ in LOOP_CALLS:
        del e.stubs[rt(CALLEES[c_][0])]
    del e.stubs[rt(0x10F4E)]
REST_STUBS = {"sector_gained": (0x3FA54, "i"), "w_61688": (0x61688, "iii"), "enemies_msg": (0x1B0EC, ""),
              "music": (0x5C5EC, "ii"), "w_33768": (0x33768, "i"), "pal_cycle": (0x528AC, "xii"), "pal_set": (0x52870, "xii"),
              "weapon_text": (0x56D3C, ""), "strip": (0x2F928, "ii"), "w_46a4c": (0x46A4C, "bi")}
if PORT and direct("sector_won"):   # SectorVictory? 0x3A0B0 (library op 187), with TeamSize, FirstMerc and CombatEnd real on both sides
    sv_stub = e.stubs.pop(rt(0x3A0B0), None)
    sv_added_ = [c_ for c_ in ("w_33768", "w_46a4c", "strip") if c_ not in CALLEES]
    for c_ in sv_added_:
        CALLEES[c_] = REST_STUBS[c_]
    added_ = [c_ for c_ in ("w_33768", "merc_message", "idle", "w_46a4c", "strip") if rt(CALLEES[c_][0]) not in e.stubs]
    for c_ in added_:
        e.stub(rt(CALLEES[c_][0]), make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))
    REG = [(0x22A0C0, 0x20), (0x92D80, 0x100), (0x284F00, 0x30), (0x229D1C, 4), (G_SOLDIERS, 0x200 * 0x30)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    rs_ = random.Random(4721)
    for i in range(n // 6):
        for k_ in range(0x30):
            sp_ = rt(G_SOLDIERS) + 0x200 * k_
            e.w8(sp_ + 0x182, rs_.choice([0, 1, 1, 1]))
            e.w8(sp_ + 0x138, rs_.choice([0, 1, 1, 1]))
            e.w8(sp_ + 0x183, rs_.choice([0, 1, 14, 15, 50, 0x7F, 0x80, 0xFF]))
            e.w8(sp_ + 0x17E, rs_.choice([0, 0, 8, 3, 0xFF]))
            for o_ in (0x1A5, 0x1B0, 0x194, 0x197):
                e.w8(sp_ + o_, rs_.choice([0, 0, 1, 5]))
        for k_ in rs_.sample(range(0x30), rs_.choice([0, 0, 1, 40])):       # the last killer stays in the table
            e.w8(rt(G_SOLDIERS) + 0x200 * k_ + 0x183, 0)
        e.w8(rt(0x22A0D2), rs_.choice([0, 1, 7, 8, 0x27, 0x28, 0x2F, 0x40, 0x40, 0x41, 0xFF]))
        e.w8(rt(0x22A0CF), rs_.choice([0, 1, 1]))
        e.w8(rt(ai.G_COMBAT), rs_.choice([0, 1, 1]))
        e.w8(rt(0x92DCA), rs_.choice([0, 1]))
        e.w32(rt(0x229D1C), rs_.choice([0, 1, 0xFFFFFFFF, 123]))
        e.w8(rt(ai.G_SELECTED), rs_.choice([0, 2, 7, 8, 0x2A, 0x40]))
        for a_, v_ in ((0x92DCC, [0, 1]), (0x22A0CD, [0, 2]), (0x92DDC, [0, 1]), (0x92DCE, [0, 1])):
            e.w8(rt(a_), rs_.choice(v_))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        k_ = e.r8(rt(0x22A0D2))
        killer_ = k_ < 0x30 and ai.s8(e.r8(rt(G_SOLDIERS) + 0x200 * k_ + 0x183)) >= 15
        none_ = not any(e.r8(sp_ + 0x182) and e.r8(sp_ + 0x17E) in (0, 8) and e.r8(sp_ + 0x183)
                        for sp_ in (rt(G_SOLDIERS) + 0x200 * j_ for j_ in range(8)))    # FirstMerc 0x2AC60 gives 0x40
        script = {c_: [0] * 64 for c_ in ("w_33768", "merc_message", "idle", "w_46a4c", "strip")}
        log.clear()
        e.call(rt(0x3A0B0))
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        if got_log[:1] != [("w_33768", (1,))]:
            raise AssertionError(f"sector_won: the original does not start with BlinkUi(1): {got_log[:1]}")
        got_log = got_log[1:]                 # (BlinkUi: the port's strips draw the box each frame)
        if got_log[:1] and got_log[0][0] == "merc_message" and (got_log[0][1][0] >= 0x30 or (not killer_ and none_)):
            got_log = got_log[1:]             # no merc on duty: entry 0x40 of the soldier table = bytes 0..3 of tile 8
            #                                   (#2): the port has no quote. The value alone does not tell: tile byte
            #                                   0x25 (a22, kill40, won1, w30) reads as slot 37
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {c_: [0] * 64 for c_ in ("w_33768", "merc_message", "idle", "w_46a4c", "strip")}
        log.clear()
        port_world("sector_won", sync=True)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["sector_won"] = counts.get("sector_won", 0) + 1
        key_ = tuple(sorted({c_[0] for c_ in log}))
        results.setdefault("sector_won", {}).setdefault(key_, 0)
        results["sector_won"][key_] += 1
        if (got_g, got_log) != (want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [[hex(j) for j in range(len(want_g[r_])) if got_g[r_][j] != want_g[r_][j]][:8] for r_ in range(len(REG))]
                print(f"MISMATCH sector_won case {i}: regions {bad_}\n  calls original {got_log}\n  calls port     {log}")
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
    for c_ in added_:
        e.stubs.pop(rt(CALLEES[c_][0]), None)
    for c_ in sv_added_:
        del CALLEES[c_]
    if sv_stub:
        e.stubs[rt(0x3A0B0)] = sv_stub
if direct("loop_rest", "fuses", "loop_head"):   # TacticalLoop after the soldier loop: 0x10F4E..0x111F5 (update pass end) and
    # 0x11228..0x1130D (the timed effects' fuses, every pass), run as code ranges; own random source, memory restored
    for c_, (a_, k_) in REST_STUBS.items():
        CALLEES[c_] = (a_, k_)
        st_ = make_stub(c_, k_)
        e.stub(rt(a_), (lambda f_: lambda emu_, *a2_: (emu_.w8(rt(0x22A0CF), 1), f_(emu_, *a2_))[1])(st_)
               if c_ == "enemies_msg" else st_, 0)    # (MsgEnemiesNearby sets 0x22A0CF itself)
    added_ = [c_ for c_ in ("stat_practice", "sfx", "explode", "w_355d0", "idle", "new_turn", "w_67e18")
              if rt(CALLEES[c_][0]) not in e.stubs]
    for c_ in added_:
        e.stub(rt(CALLEES[c_][0]), make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))
    # TimeCompress 0x11D40 (time compression off) waits for the buttons up: WaitLeftUp 0x67C54 / WaitRightUp 0x67C68
    # poll the mouse driver and copy the last place 0x229D00 into 0x229CF0 (platform code; the port's front-end waits
    # on window events). A dump where the two differ (wl_post) showed 296 false loop_head mismatches at 0x229CF0.
    mwait_ = [a_ for a_ in (0x67C54, 0x67C68) if rt(a_) not in e.stubs]
    for a_ in mwait_:
        e.stub(rt(a_), lambda em_, *_: 0, 0)
    rr = random.Random(4711)
    REG = [(ai.G_SECTOR_TBL9, 9 * 80), (0x93449, 1), (0x22A0CF, 1), (0x284F00, 0x30), (0x93AAA, 2), (0x229D28, 0x28),
           (0x92EF8, 4), (0x229AFC, 4), (ai.G_TILE_ANIMS, 0xB * 10), (G_TILEMAP, 8 * 3136), (ai.G_FX, 65),
           (ai.G_GROUND_ITEMS, 12 * 200), (0x92D80, 0x100), (0x229C80, 0xA8), (0x22A400, 0x88), (0x22A0C0, 0x20),
           (G_SOLDIERS, 0x200 * 0x30), (0x9300C, 1), (0x92F60, 4)]   # (time compression + tick step: #70 F18)
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    gl_ = [(a_, e.r8(rt(a_))) for a_ in [ai.G_CUR_SECTOR, 0x1163C0, 0x9300C] + [ai.G_SPLASH + k_ for k_ in range(60)]]
    msave_ = [bytes(e.uc.mem_read(rt(G_SOLDIERS) + 0x200 * k_ + 0x138, 0x50)) for k_ in range(8)]
    water_ = [t_ for t_ in range(3136) if e.r8(rt(G_TILEMAP) + 8 * t_) >= 0x97]
    items_ = [t_ for t_ in range(3136) if e.r8(rt(G_TILEMAP) + 8 * t_ + 6) < 0xC8] or [0]
    frame_ = e.alloc(16)
    tramp_ = e.alloc(16)
    w_ = World(0)
    w_.memrun = True
    for sec_, start_, end_ in (("loop_rest", 0x10F4E, 0x111F5), ("fuses", 0x11228, 0x1130D), ("loop_head", 0x108D8, 0x10AC1)):
        if not direct(sec_):
            continue
        e.uc.mem_write(tramp_, b"\xBD" + struct.pack("<I", frame_ + 8) + b"\xE9" + struct.pack("<i", rt(start_) - (tramp_ + 10)))
        e.stub(rt(end_), lambda emu_, *a_: emu_.uc.emu_stop())
        for i in range(n // 6):
            sec = rr.choice([e.r8(rt(ai.G_CUR_SECTOR))] * 3 + [0, 0, rr.randrange(60)])
            e.w8(rt(ai.G_CUR_SECTOR), sec)
            row_ = rt(ai.G_SECTOR_TBL9) + 9 * sec
            e.w8(row_, rr.choice([0, 1, 2, 3, 0xFE, 0xFF]))
            e.w8(row_ + 1, rr.choice([0, 0, 1, 5, 0x80]))
            e.w8(rt(0x22A0CF), rr.choice([0, 0, 1]))
            for k_ in range(8):
                sp_ = rt(G_SOLDIERS) + 0x200 * k_
                e.w8(sp_ + 0x182, rr.choice([0, 1, 1, 1]))
                e.w8(sp_ + 0x183, rr.choice([0, 14, 15, 50, 0x80]))
                e.w8(sp_ + 0x138, rr.choice([0, 1, 1, 1]))
            for a_ in (0x229D48, 0x92EF8, 0x229D30, 0x229D34):
                e.w32(rt(a_), rr.choice([0, 0, 1, 10, 300, 0xFFFFFFF6, 0x80000000]))
            if PORT:                       # the UI blink is the front-end's (soldier_update.md): not due
                e.w32(rt(0x229D48), 100)
            e.w8(rt(0x284F06), rr.choice([0, 1]))
            e.w8(rt(0x284F04), rr.choice([0, 1, 2, 3, 0x80, 0xFF]))
            e.w8(rt(0x284F19), rr.choice([0, 5, 0xF8, 0xFD, 0xFE, 0xFF]))
            e.w8(rt(0x284F14), rr.choice([0, 1]))
            e.w8(rt(0x1163C0), rr.choice([0, 1, 1]))
            e.w8(rt(0x9300C), rr.choice([0, 0, 1]))
            e.w32(rt(0x92F60), 1000 if e.r8(rt(0x9300C)) else 10)   # the tick step TimeCompress sets
            e.w8(rt(ai.G_SPLASH + sec), rr.choice([0, 1, 1, 3, 10, 0x7F, 0xFF, 0x80]))
            for k_ in range(10):                         # tile animations: some free
                a_ = rt(ai.G_TILE_ANIMS) + 0xB * k_
                e.w8(a_, rr.choice([0, 0, 0, 1, 5, 0x80]))
            for k_ in range(5):                          # timed effects
                f_ = rt(ai.G_FX) + 13 * k_
                e.w32(f_, rr.choice([0, 0, 0xFFFFFFFF, 10, 1]))
                e.w8(f_ + 9, rr.choice([0, 0, 0xFF, 1, 0x80]))
                e.w16(f_ + 6, rr.choice([rr.choice(items_), rr.choice(items_), rr.randrange(3136), 0xC41, 0xC40, 0xFFFF, 0x8000]))
                if PORT and ai.s16(e.r16(f_ + 6)) < 0 or e.r16(f_ + 6) == 0xC40:   # (--port: off-map tiles below
                    e.w16(f_ + 6, 0xC41 if PORT else 0xC40)                         # 0xC41 read other memory)
                e.w8(f_ + 8, rr.choice([0, 1, 2, 3, 4, 9, 0xA, 0xA, rr.randrange(256)]))
                for o_ in (0xA, 0xB, 0xC):
                    e.w8(f_ + o_, rr.choice([0, 1, 0x40, 0xFF, rr.randrange(256)]))
                e.w16(f_ + 4, rr.randrange(0x10000))
            if sec_ == "loop_head":      # TacticalLoop head: turn timers, combat start / end
                e.w8(row_ + 2, rr.choice([0, 0, 1, 5]))
                e.w8(rt(ai.G_COMBAT), rr.choice([0, 1]))
                for a_, v_ in ((0x229D24, [0, 0, 10, 0xFFFFFFF6]), (0x229D1C, [0, 0, 1, 20000, 0xFFFFFFFF]),
                               (ai.G_BUSY, [0, 0, 1, 0xFFFFFFFF]), (0x92DB4, [0, 0, 5]), (0x92DA4, [0, 0, 1]),
                               (ai.G_TURN_BASED, [0, 1]), (ai.G_FAST_TIMER, [0, 1, 0xFFFFFFF6])):
                    e.w32(rt(a_), rr.choice(v_))
                for a_, v_ in ((0x92DCC, [0, 1]), (0x22A0CD, [0, 1, 2]), (0x229E00, [0x40, 0x40, 0x30] if PORT else [0x40, 0x40, 3, 0x30]),
                               (0x92DC7, [0, 3, 8, 0x40]), (ai.G_SELECTED, [0, 2, 8, 0x2A, 0x40]), (0x92DDF, [0, 2, 3])):
                    e.w8(rt(a_), rr.choice(v_))
                for a_ in (0x22A484, 0x22A486):
                    e.w16(rt(a_), rr.choice([0, 1, 19, 20, 0xFFFF, 0x8000, 5]))
                for k_ in range(0x28):
                    sp_ = rt(G_SOLDIERS) + 0x200 * k_
                    e.w8(sp_ + 0x182, rr.choice([0, 1, 1, 1]))
                    e.w8(sp_ + 0x138, rr.choice([0, 1, 1, 1]))
                    e.w8(sp_ + 0x183, rr.choice([0, 1, 50, 0x80]))
                    e.w8(sp_ + 0x17E, rr.choice([0, 0, 8, 3]))
                    for o_ in (0x1A5, 0x1B0, 0x194, 0x197):
                        e.w8(sp_ + o_, rr.choice([0, 0, 1, 5]))
                if i == 0:       # Force the original's 0x106F4 combat-end function through the loop head.
                    e.w8(row_ + 1, 0)
                    e.w8(rt(ai.G_COMBAT), 1)
                    e.w8(rt(ai.G_SELECTED), 0)
                    sp_ = rt(G_SOLDIERS)
                    for o_ in (0x182, 0x138):
                        e.w8(sp_ + o_, 1)
                    e.w8(sp_ + 0x183, 50)
            rnd_ = [rr.choice([rr.choice(water_) if water_ else 0, rr.randrange(3136), 0, 0, 1, rr.randrange(1 << 15)])
                    for _ in range(6)]
            tb_ = rt(G_TILEMAP) + 8 * (rnd_[0] % 3136)     # the ripple try's tile: object and occupant bytes
            e.w8(tb_ + 1, rr.choice([0, 0, 0, 1, 0x80]))
            e.w8(tb_ + 5, rr.choice([0x40, 0x40, 0x30, 0x2F, 0, 0xC0, 0xB0, 0xAF, 0x7F]))
            snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
            base_ = {"Random": rnd_,
                     **{k_: [0] * 8 for k_ in list(REST_STUBS) + ["stat_practice", "sfx", "explode", "w_355d0", "idle", "w_67e18"]},
                     "new_turn": [rr.choice([0, 0, 1, 2, 0xFF]) for _ in range(4)]}
            script = {k_: list(v_) for k_, v_ in base_.items()}
            log.clear()
            e.w32(frame_ + 4, 0)
            e.call(tramp_)
            got = e.r8(frame_ + 4)
            got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
            if sec_ == "loop_head" and i == 0 and not any(c_ == "idle" for c_, _ in got_log):
                raise AssertionError("forced combat-end case did not reach original 0x106F4 / AiIdle")
            for (a_, k_), b_ in zip(REG, snap):
                e.uc.mem_write(rt(a_), b_)
            script = {k_: list(v_) for k_, v_ in base_.items()}
            log.clear()
            if PORT:
                want = port_world(sec_, sync=True) & 0xFF if sec_ in ("loop_rest", "loop_head") else (port_world(sec_), 0)[1]
                if sec_ == "loop_rest":
                    e.w8(rt(0x284F14), 1)  # (the front-end's pass flag)
            elif sec_ == "loop_head":
                want = ai.loop_head(w_)
            elif sec_ == "loop_rest":
                want = ai.update_rest(w_)
            else:
                want = 0
                ai.fx_fuses(w_)
            want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
            counts[sec_] = counts.get(sec_, 0) + 1
            key_ = tuple(sorted({c_[0] for c_ in log} - {"stat_practice"}))
            results.setdefault(sec_, {}).setdefault(key_, 0)
            results[sec_][key_] += 1
            if (got, got_g, got_log) != (want, want_g, log):
                fails += 1
                if fails <= 10:
                    bad_ = [[f"{j:#x}: {got_g[r_][j]:#x}/{want_g[r_][j]:#x}" for j in range(len(want_g[r_]))
                             if got_g[r_][j] != want_g[r_][j]][:8] for r_ in range(len(REG))]
                    print(f"MISMATCH {sec_} case {i}: {got} / {want}; regions {bad_}\n  calls original {got_log}\n  calls port     {log}")
            for (a_, k_), b_ in zip(REG, tsave):
                e.uc.mem_write(rt(a_), b_)
        del e.stubs[rt(end_)]
    for a_, v_ in gl_:
        e.w8(rt(a_), v_)
    for k_, b_ in enumerate(msave_):
        e.uc.mem_write(rt(G_SOLDIERS) + 0x200 * k_ + 0x138, b_)
    for c_, (a_, k_) in REST_STUBS.items():
        if c_ != "w_46a4c":         # (stays: DeselectSoldier 0x17F10 runs real and calls it)
            del CALLEES[c_]
            e.stubs.pop(rt(a_), None)
    for c_ in added_:
        e.stubs.pop(rt(CALLEES[c_][0]), None)
    for a_ in mwait_:
        e.stubs.pop(rt(a_), None)
if direct("leave_tests"):   # TacticalLoop 0x1130D..0x11352 (after the fuses): home sector lost, flag 0x22A05C; code range
    added_ = [c_ for c_ in ("message",) if rt(CALLEES[c_][0]) not in e.stubs]
    for c_ in added_:
        e.stub(rt(CALLEES[c_][0]), make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))
    rl_ = random.Random(4713)
    home_ = rt(ai.G_SECTOR_TBL9) + 9 * 59
    gl_ = [(a_, e.r8(rt(a_))) for a_ in (0x2299AF, 0x93449, 0x22A05C, 0x22A04E)]
    frame_ = e.alloc(16)
    tramp_ = e.alloc(16)
    e.uc.mem_write(tramp_, b"\xBD" + struct.pack("<I", frame_ + 8) + b"\xE9" + struct.pack("<i", rt(0x1130D) - (tramp_ + 10)))
    where_ = []
    for a_ in (0x10800, 0x11358):
        e.stub(rt(a_), (lambda a2_: lambda emu_, *x_: (where_.append(a2_), emu_.uc.emu_stop(), 0)[2])(a_), 0)
    w_ = World(0)
    w_.memrun = True
    for i in range(n // 12):
        e.w8(home_, rl_.choice([0, 1, 2, 3, 0xFE, 0xFF]))
        e.w8(rt(0x93449), rl_.choice([0, 0, 1, 4]))
        e.w8(rt(0x22A05C), rl_.choice([0, 0, 1, 0x80]))
        e.w8(rt(0x22A04E), rl_.choice([0, 1, 2, 3, 0x80, 0xFF]))
        c0_ = rl_.choice([0, 0, 0, 1, 2, 0xFF])
        snap = [e.r8(rt(a_)) for a_, _ in gl_]
        script = {"message": [0]}
        log.clear()
        where_.clear()
        e.w32(frame_ + 4, c0_)
        e.call(tramp_)
        got = (e.r8(frame_ + 4), list(where_), [e.r8(rt(a_)) for a_, _ in gl_], list(log))
        for (a_, _), v_ in zip(gl_, snap):
            e.w8(rt(a_), v_)
        script = {"message": [0]}
        log.clear()
        c_ = (c0_ + (port_world("leave_tests", sync=True) if PORT else ai.leave_tests(w_))) & 0xFF
        want = (c_, [0x11358 if c_ else 0x10800], [e.r8(rt(a_)) for a_, _ in gl_], list(log))
        counts["leave_tests"] = counts.get("leave_tests", 0) + 1
        results.setdefault("leave_tests", {}).setdefault(c_ != 0, 0)
        results["leave_tests"][c_ != 0] += 1
        if got != want:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH leave_tests case {i}: original {got}\n  mirror   {want}")
    for a_ in (0x10800, 0x11358):
        del e.stubs[rt(a_)]
    for a_, v_ in gl_:
        e.w8(rt(a_), v_)
    for c_ in added_:
        e.stubs.pop(rt(CALLEES[c_][0]), None)
if direct("ticks"):   # the 10 ms timer callback 0x5EE98, its soldier part (+ the stop timer); called directly
    REG = [(G_SOLDIERS, 0x200 * 0x30), (0x95DC8, 4), (0x93D54, 4), (0x229D1C, 4), (0x229CB4, 4), (0x229C94, 4), (0x229C8C, 4),
           (0x92DB4, 4), (ai.G_BUSY, 4), (ai.G_FAST_TIMER, 4), (ai.G_TURN_BASED, 4), (ai.G_TILE_ANIMS, 0xB * 10), (ai.G_FX_ANIMS, 0x12 * 16), (ai.G_TILEMAP, 8 * 3136),
           (0x229D30, 8), (0x229D48, 4), (0x92EF8, 4), (ai.G_FX, 65), (0x229E0C, 4), (0x284F29, 1), (0x229D24, 4)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    gsave_ = [(a_, e.r32(rt(a_))) for a_ in (0x92DE1, 0x92DB8, 0x92DE0, 0x92DCC, ai.G_TICK_STEP)]
    rt_ = random.Random(4712)          # (the new fields' fuzz: own random source, the shared one stays as it was)
    w_ = World(0)
    w_.memrun = True
    for i in range(n // 3):
        for k_ in range(0x30):
            sp_ = rt(G_SOLDIERS) + k_ * 0x200
            e.w8(sp_ + 0x182, rng.choice([0, 1, 1, 1, 1]))
            e.w8(sp_ + 0x138, rng.choice([0, 1, 1, 1, 1]))
            e.w8(sp_ + 0x183, rng.choice([0, 14, 15, 50, 0x80]))
            for o_ in (0xB0, 0xCC, 0xA8, 0xC0, 0xC4, 0xB4, 0xBC, 0x1DC):
                e.w32(sp_ + o_, rng.choice([0, 1, 5, 9, 10, 11, 100, 0xFFFFFFFF, 0xFFFFFFF6, 0x80000005, rng.randrange(1 << 32)]))
            e.w8(sp_ + 0x189, rng.choice([0, 1, 0x80]))
        e.w8(rt(0x92DE1), rng.choice([0, 0, 0, 0, 1]))     # round_lock2, round_lock (turns.md New turn)
        e.w8(rt(0x92DB8), 0)
        e.w8(rt(0x92DE0), rng.choice([0, 0, 0, 0, 1]))
        e.w8(rt(0x92DCC), rng.choice([0, 1]))
        e.w32(rt(ai.G_TICK_STEP), rng.choice([10, 10, 1000, 0]))
        e.w32(rt(0x95DC8), rng.choice([0, 1, 5, 9, 10, 15, 2000, 0xFFFFFFF0]))
        e.w32(rt(ai.G_BUSY), rng.choice([0, 1, 2, 0xFFFFFFFF]))
        e.w32(rt(ai.G_TURN_BASED), rng.choice([0, 1, 2]))
        for a_ in (0x93D54, 0x229D1C, 0x92DB4, ai.G_FAST_TIMER):      # show timer, turn clock, busy timer, fast timer
            e.w32(rt(a_), rng.choice([0, 1, 5, 9, 10, 11, 1000, 5000, 20000, 0xFFFFFFFF, 0x80000005]))
        for a_ in (0x229CB4, 0x229C94, 0x229C8C):
            e.w32(rt(a_), rng.choice([0, 3, 0x40, 0xC41, 1234]))
        for k_ in range(10):                         # tile animations (front_ticks): count, timer, tile
            a_ = rt(ai.G_TILE_ANIMS) + 0xB * k_
            e.w8(a_, rng.choice([0, 0, 1, 1, 2, 5, 0x80, 0xFF]))
            e.w32(a_ + 1, rng.choice([0, 1, 9, 10, 11, 0x8C, 0xFFFFFFFF, 0x80000005]))
            t_ = rng.choice([rng.randrange(3136), 0xC41, 0xC40, 0])
            e.w16(a_ + 5, t_)
            if t_ < 3136:
                e.w8(rt(ai.G_TILEMAP) + 8 * t_ + 3, rng.choice([0, 1, 5, 9]))
        for k_ in range(16):                         # effect animations: count, timer
            a_ = rt(ai.G_FX_ANIMS) + 0x12 * k_
            e.w8(a_, rng.choice([0, 0, 1, 2, 5, 0x80, 0xFF]))
            e.w32(a_ + 1, rng.choice([0, 1, 9, 10, 11, 0x8C, 0xFFFFFFFF, 0x80000005]))
        for a_ in (0x229D30, 0x229D34, 0x229D48, 0x92EF8, 0x229E0C, 0x229D24):   # UI / ripple / fuse / turn-step timers
            e.w32(rt(a_), rt_.choice([0, 1, 9, 10, 11, 700, 0xFFFFFFFF, 0x80000005]))
        e.w8(rt(0x284F29), rt_.choice([0, 0, 1]))
        for k_ in range(5):                          # timed effects: fuse +0, timer +9, ticking +0xC
            f_ = rt(ai.G_FX) + 13 * k_
            e.w32(f_, rt_.choice([0, 1, 9, 10, 11, 500, 0xFFFFFFFF, 0x80000005]))
            e.w8(f_ + 9, rt_.choice([0, 0, 1, 0xFF, 0x80]))
            e.w8(f_ + 0xC, rt_.choice([0, 0, 1, 0x80]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        log.clear()
        e.call(rt(0x5EE98))
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        log.clear()
        if PORT:
            port_world("tick")
            ai.front_ticks(w_)                       # (the front-end's part: the harness is the host)
            if not e.r8(rt(0x92DE1)):                # (and its UI blink timer, soldier_update.md Update end)
                e.w32(rt(ai.G_BLINK_TIMER), (e.r32(rt(ai.G_BLINK_TIMER)) - 10) & 0xFFFFFFFF)
        else:
            ai.soldier_ticks(w_)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["ticks"] = counts.get("ticks", 0) + 1
        results.setdefault("ticks", {}).setdefault(e.r8(rt(0x92DCC)), 0)
        results["ticks"][e.r8(rt(0x92DCC))] += 1
        if got_g != want_g:
            fails += 1
            if fails <= 10:
                bad_ = [[hex(j) for j in range(len(want_g[r_])) if got_g[r_][j] != want_g[r_][j]][:8] for r_ in range(len(REG))]
                print(f"MISMATCH ticks: regions {bad_}")
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
    for a_, v_ in gsave_:
        e.w32(rt(a_), v_)
if direct("rebuild"):   # 0x2E730 move codes from scratch + door pass: called directly, whole table compared
    REG = [(G_TILEMAP, 8 * 3136), (combat.G_MOVECODE, 2 * 3136)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    DOORS_ = list(ai.DOOR_SIDES) + [0x1C, 0x1F, 0x3C, 0x42, 0x44, 0x5B, 0x5E, 0x61, 0x63, 0x65, 0x21, 0x23]
    for i in range(max(6, n // 600)):
        m_ = bytearray(tsave[0])
        for t_ in range(3136):
            if rng.random() < 0.3:
                m_[8 * t_ + 1] = rng.choice(DOORS_ + [0, 0, rng.randrange(256)])
            if rng.random() < 0.1:
                m_[8 * t_] = rng.randrange(256)
            if rng.random() < 0.1:
                m_[8 * t_ + 3] = rng.choice([0x73, 0, rng.randrange(256)])
        e.uc.mem_write(rt(G_TILEMAP), bytes(m_))
        e.uc.mem_write(rt(combat.G_MOVECODE), bytes(rng.randrange(256) for _ in range(2 * 3136)))
        log.clear()
        e.call(rt(0x2E730))
        got, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        e.uc.mem_write(rt(combat.G_MOVECODE), bytes(rng.randrange(256) for _ in range(2 * 3136)))
        log.clear()
        if PORT:
            e.uc.mem_write(rt(combat.G_MOVECODE), struct.pack("<3136H", *port_lib.rebuild_move_codes(
                m_, e.uc.mem_read(rt(combat.G_T0CLASS), 256), e.uc.mem_read(rt(combat.G_T1CLASS), 256))))
        else:
            ai.rebuild_move_codes(World(0))
        want = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["rebuild"] = counts.get("rebuild", 0) + 1
        key_ = min(want[1].count(b"\xd4\x62") // 50, 4)       # door marks 0x62D4 (bucketed)
        results.setdefault("rebuild", {}).setdefault(key_, 0)
        results["rebuild"][key_] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                bad_ = [hex(j // 2) for j in range(0, 6272, 2) if got[1][j:j + 2] != want[1][j:j + 2]]
                print(f"MISMATCH rebuild case {i}: tiles {bad_[:12]} ({len(bad_)})\n  calls original {got_log}\n  calls port     {log}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
if direct("doors"):   # door lookup, open / break, close, toggle by id: called directly, stubs lifted
    class DoorWorld(World):
        def ext(self, name, *args):
            if name == "door_lookup":
                return ai.door_lookup(self, *args)
            if name == "break_obj":
                return ai.break_object(self, *args[:3])
            if name == "close_door":
                return ai.close_door(self, *args)
            return super().ext(name, *args)
    lifted = {a_: e.stubs.pop(rt(a_)) for a_ in (0x3AFE0, 0x3ACB8, 0x3ADB4, 0x35774) if rt(a_) in e.stubs}
    REG = [(G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (ai.G_MOVECODE, 2 * 3136), (ai.G_SECTORS, 0xB1 * 60),
           (ai.G_CUR_SECTOR, 1), (G_SOLDIERS, 0x200 * 0x30)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    DOORS = [0x3B, 0x3D, 0x3E, 0x41, 0x43, 0x45, 0x5A, 0x5C, 0x5D, 0x60, 0x62, 0x64]
    for i in range(n // 12):
        kind = rng.choice(["lookup", "break", "break", "close", "close", "toggle"])
        if PORT and kind == "toggle":       # (toggle by id: not in the port; same rng draws)
            kind = "close"
        sec = rng.randrange(60)
        e.w8(rt(ai.G_CUR_SECTOR), sec)
        cells = [rng.randrange(57, 3136) for _ in range(rng.choice([1, 4, 12, 30]))] + [3136] * (rng.random() < 0.2 and not PORT)
        for c in cells:
            e.w8(rt(G_TILEMAP) + 8 * c + 1, rng.choice(DOORS + [0x3C, 0x42, 0x63, 0x65, 0x3A, rng.randrange(256)]))
            e.w8(rt(G_TILEMAP) + 8 * c + 5, rng.choice([0x40, 0x40, 0x40, rng.randrange(0x30), 0x2F, 0xAF, 0x30, 0xC0, 0x85]))
            e.w8(rt(G_TILEMAP) + 8 * c + 3, rng.choice([0, 0x73]))
            for nb in (c - 1, c - 56):
                e.w8(rt(G_TILEMAP) + 8 * nb + 1, rng.choice([0x42, 0x44, 0x40, 0x3C, 0, 1, 0xFF]))
                e.w8(rt(G_TILEMAP) + 8 * nb + 2, rng.randrange(256))
        rb = rt(ai.G_SECTORS) + 0xB1 * sec
        ids = [rng.choice([0, 1, 2, 3, 0xFF, 0x101, 0xFFFF]) for _ in range(12)]
        for k in range(12):
            e.w16(rb + 4 * k, rng.choice(cells + [ai.NO_TILE, rng.randrange(3136), 0xFFFF]))
            e.w16(rb + 4 * k + 2, ids[k])
        tile = rng.choice(cells + [rng.randrange(57, 3136)])
        obj = rng.choice([e.r8(rt(G_TILEMAP) + 8 * tile + 1)] * 3 + DOORS)
        snd, mode, door = rng.choice([0, 1, 0x100]), rng.choice([1, 1, 0, 2, 0x101]), rng.choice(ids[:3] + [0, 1, 0x101])
        arg = {"lookup": (0x3AFE0, (rng.choice([tile, tile, 0xFFFF, -1, tile | 0x10000]),)),
               "break": (0x3ACB8, (tile, obj, snd, tile)), "close": (0x3ADB4, (tile, obj, snd)),
               "toggle": (0x35774, (mode, door))}[kind]
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        script = {"sfx": [0] * 8}
        log.clear()
        got_r = e.call(rt(arg[0]), *[a_ & 0xFFFFFFFF for a_ in arg[1]], max_steps=5_000_000)
        got, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {"sfx": [0] * 8}
        log.clear()
        w_ = DoorWorld(0)
        a1 = arg[1]
        if PORT:                            # library ops 179 / 107 / 104 (combat.md Doors)
            want_r = port_world({"lookup": "door_lookup", "break": "break_object", "close": "close_door"}[kind],
                                tuple(ai._i32(a_) for a_ in (a1[:3] if kind == "break" else a1)))
        else:
            want_r = {"lookup": lambda: ai.door_lookup(w_, a1[0]), "break": lambda: ai.break_object(w_, *a1[:3]),
                      "close": lambda: ai.close_door(w_, *a1), "toggle": lambda: ai.toggle_doors(w_, *a1)}[kind]()
        want = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        got_r, want_r = (got_r, (want_r or 0) & 0xFFFFFFFF) if kind == "lookup" else (0, 0)
        counts["doors"] = counts.get("doors", 0) + 1
        changed = got != snap
        results.setdefault("doors", {}).setdefault(kind + ("+" if changed or got_log else ""), 0)
        results["doors"][kind + ("+" if changed or got_log else "")] += 1
        if (got_r, got, got_log) != (want_r, want, log):
            fails += 1
            if fails <= 10:
                bad = [hex(REG[k][0]) for k in range(len(REG)) if got[k] != want[k]]
                print(f"MISMATCH doors {kind}{a1}: ret {got_r:#x} / {want_r:#x}, regions differ {bad}"
                      f"\n  calls original {got_log}\n  calls port     {log}")
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    for a_, st_ in lifted.items():
        e.stubs[rt(a_)] = st_
if direct("put_item"):   # stack-argument item drop: called directly with the stub lifted
    keep = e.r8(rt(G_TILEMAP) + 8 * 100 + 1)
    for obj in range(256):              # 0x23610 over every object value
        e.w8(rt(G_TILEMAP) + 8 * 100 + 1, obj)
        got_, want_ = ai._i32(e.call(rt(0x23610), 100)), ai.object_surface(World(0), 100)
        counts["object_surface"] = counts.get("object_surface", 0) + 1
        results.setdefault("object_surface", {}).setdefault(want_, 0)
        results["object_surface"][want_] += 1
        if got_ != want_:
            fails += 1
            print(f"MISMATCH object_surface(object {obj:#x}): original {got_} port {want_}")
    e.w8(rt(G_TILEMAP) + 8 * 100 + 1, keep)
    stub = e.stubs.pop(rt(0x3FC78))
    stub2_ = e.stubs.pop(rt(0x3FC04)) if PORT else None    # port: no host call, the listed-type test has no effect
    # (port: the 3136 map tiles only; with no tile found the original also writes byte 6 of tile 0xC41)
    area = [(G_TILEMAP, 8 * (3136 if PORT else 3138)), (ai.G_GROUND_ITEMS, 12 * 200), (ai.G_NOTES_FOUND, 30)]
    F50 = bool(FAN and 50 in FAN)       # #70 F50: the swap keeps the old item under a timed effect, moves its blast mark
    if F50:
        area += [(ai.G_BLAST_LIST, 60), (ai.G_FX, 65)]
        F50_RNG = random.Random(5050)    # own random source: the plain case stream stays
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    notes = [e.r8(rt(ai.G_NOTES_FOUND) + 2 * k) for k in range(15)]
    for i in range(4000):
        tile = rng.choice([rng.randrange(3136), rng.randrange(57), rng.randrange(3080, 3136)])
        for d in range(-3, 4):                  # crowd the neighbourhood: ground items, water, objects
            for t_ in (tile + d, tile + 56 * d, tile + 57 * d):
                if 0 <= t_ < 3136 and rng.random() < 0.5:
                    e.w8(rt(G_TILEMAP) + 8 * t_ + 6, rng.choice([0xFF, 0xFF, rng.randrange(200)]))
                    e.w8(rt(G_TILEMAP) + 8 * t_ + 1, rng.choice([0, 0, 0x3B, 0x3D, 0x45, 0x5A, 0x64, 0x41, 0x60, 0x1C, 0x1F, 0x24, 0x26,
                                                                  rng.randrange(256)]))
                    e.w8(rt(G_TILEMAP) + 8 * t_, rng.choice([0x10, 0x10, 0x6C, 0x70]))
        if rng.random() < 0.15:                 # a flooded square: only ring 11 (or nothing) is left for the item
            cx, cy = tile % 56, tile // 56
            for y_ in range(max(0, cy - 11), min(56, cy + 12)):
                for x_ in range(max(0, cx - 11), min(56, cx + 12)):
                    e.w8(rt(G_TILEMAP) + 8 * (56 * y_ + x_), 0x6C)
                    e.w8(rt(G_TILEMAP) + 8 * (56 * y_ + x_) + 1, 0)
            d_ = rng.randrange(1, 9)
            t_ = tile
            for _ in range(11):
                t_ = ai.step_tile(t_, ai.dir_step(World(0), d_))
            if rng.random() < 0.6:
                e.w8(rt(G_TILEMAP) + 8 * t_, 0x10)
                e.w8(rt(G_TILEMAP) + 8 * t_ + 6, 0xFF)
        if rng.random() < 0.1:                  # a full ground item table now and then (or only the last entry free)
            for k in range(200):
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * k, rng.choice([1, 5, 0x30]))
            if rng.random() < 0.5:
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * 199, 0)
        else:
            for k in rng.sample(range(200), 30):
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * k, rng.choice([0, 1, 0x30]))
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * k + 2, rng.choice([0, 0, 1, 50, 0x80]))
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * k + 6, rng.choice([0xFF, 0xFF, 0, 1]))
        for k in range(15):
            e.w8(rt(ai.G_NOTES_FOUND) + 2 * k + 1, rng.choice([0, 1, 2]))
        if F50:                                 # the asked tile in the blast list and / or a timed effect record
            for k in range(30):
                e.w16(rt(ai.G_BLAST_LIST) + 2 * k, F50_RNG.choice([0xC41, 0xC41, tile, tile + 1, F50_RNG.randrange(3136)]))
            for k in range(5):
                e.w16(rt(ai.G_FX) + 13 * k + 6, F50_RNG.choice([0xC41, 0xC41, 0xC41, tile, F50_RNG.randrange(3136)]))
            if F50_RNG.random() < 0.5:          # an old item that must move aside (+6 != -1) on the asked tile
                g_ = F50_RNG.randrange(200)
                e.w8(rt(G_TILEMAP) + 8 * tile + 6, g_)
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_ + 6, F50_RNG.choice([0, 1, 0xFF]))
        typ = rng.choice([0, rng.randrange(1, 136), rng.randrange(1, 136), rng.choice(notes), 0x16, 0x5D])
        if PORT and typ == 135:        # its sink flag lies past the port's item_types (sector_start.md batch 3)
            typ = 134
        it = bytes([typ, rng.choice([1, 1, 2, 3, 5, 0x80]), rng.choice([1, 50, 100, 0])] +
                   [rng.choice([0, 0, rng.randrange(256)]) for _ in range(9)])
        d0, d1, d2 = (int.from_bytes(it[k:k + 4], "little") for k in (0, 4, 8))
        b6, exact, note, near = (rng.choice([0, 0xFF, 1]), rng.choice([0, 0, 1, 0x100]), rng.choice([0, 1]),
                                 rng.choice([0, 0, 1, 0xFF]))
        base_ = {} if PORT else {"w_3fc04": [rng.choice([0, 1, 0x100])] * 2}
        script = {k_: list(v) for k_, v in base_.items()}
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        log.clear()
        e.call(rt(0x3FC78), tile, rng.randrange(1 << 32), rng.randrange(1 << 32), rng.randrange(1 << 32),
               d0, d1, d2, b6, exact, note, near)
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        script = {k_: list(v) for k_, v in base_.items()}
        log.clear()
        if PORT:
            port_sector("put_item", (tile, it, b6, exact, note, near))
        else:
            ai.put_item(World(0), tile, d0, d1, d2, b6, exact, note, near)
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        counts["put_item"] = counts.get("put_item", 0) + 1
        key = "none" if got == snap else "placed"
        results.setdefault("put_item", {}).setdefault(key, 0)
        results["put_item"][key] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH put_item(tile {tile}, item {it.hex()}, b6 {b6}, exact {exact}, note {note}, near {near}): "
                      f"areas {[n for n, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log}\n  calls port     {log}")
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    e.stubs[rt(0x3FC78)] = stub
    if stub2_ is not None:
        e.stubs[rt(0x3FC04)] = stub2_
if direct("effect_hit"):   # explosion effect step: no soldier record, called directly (Blast runs for real)
    real_ = blast_real()
    if PORT:                    # the library recomputes the workable count itself (strategic.md Workable trees)
        real_["workable"] = e.stubs.pop(rt(CALLEES["workable"][0]))
        real_["connected_home"] = e.stubs.pop(rt(CALLEES["connected_home"][0]))
    tmap = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
    soldiers = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    sec = e.r8(rt(ai.G_CUR_SECTOR))
    area = [(ai.G_GAS_CLOUDS - 2, 32), (ai.G_SECTOR_TBL9, 9 * 0x40), (0x222BF6 + 0xB1 * sec, 10), (0x22A057, 1),
            (ai.G_BLASTER, 1), (ai.G_BLAST_FX, 5), (ai.G_MOVECODE, 2 * 3136)]   # (move codes: a broken object rebuilds them)
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    for i in range(3000):
        tile = rng.randrange(3136)
        e.w8(rt(G_TILEMAP) + 8 * tile + 1, rng.choice([0x86, 0x35, 0x36, 0, 0x10, rng.randrange(256)]))
        e.w8(rt(G_TILEMAP) + 8 * tile, rng.choice([e.r8(rt(G_TILEMAP) + 8 * tile)] * 3 + [rng.randrange(256)]))
        for j in range(5):                      # gas clouds: some free, sometimes none
            e.w16(rt(ai.G_GAS_CLOUDS) + 6 * j, rng.choice([ai.NO_TILE, ai.NO_TILE, rng.randrange(3136)]))
        e.w16(rt(0x222BF6 + 0xB1 * sec) + 2 * rng.randrange(5), rng.choice([tile, rng.randrange(3136)]))
        e.w8(rt(0x22A057), rng.choice([sec, sec, 0x80 | sec, 5]))
        e.w8(rt(ai.G_BLASTER), rng.choice([rng.randrange(8), 0x40]))
        kind = rng.choice([2, 2, 3, 3, 3, 4, 5, 6, 7, 8, 9, 0x63, 0x102])
        power = rng.choice([0, 1, 5, 0x7F, 0x80, 0xFF, 0x105])
        f68_bridges(tile)
        base = {"Random": [rng.choice([0, 1, 7, 25, 50, 999, 2000, rng.randrange(1 << 15)]) for _ in range(400)],
                "direction": [rng.choice([1, 4, 8])] * 200, "take_damage": [0] * 200, "rest": [0] * 200,
                "item_panel": [0] * 200, "w_17b70": [0] * 200, "w_74304": [0] * 200, "stat_practice": [0] * 200,
                "break_obj": [0] * 20, "workable": [0], "explode_art": [0], "w_3c05c": [0],
                "story_hit": [0] * 5, "replay_event": [0]}
        for k_ in BLAST_OPS:
            base.setdefault(k_, [0] * 200)
        script = {k_: list(v) for k_, v in base.items()}
        snap = ([bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)),
                bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))])
        log.clear()
        e.call(rt(0x173B4), tile, kind, power)
        got_log = list(log)
        got = ([bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)),
               bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))])
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        e.uc.mem_write(rt(G_TILEMAP), snap[-2])
        e.uc.mem_write(rt(G_SOLDIERS), snap[-1])
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        if PORT:
            port_sector("effect_hit", (tile, ai._i32(kind), power), allow=BLAST_OPS + EFFECT_OPS)
        else:
            w_ = World(0)
            w_.memrun = True
            ai.effect_hit(w_, tile, kind, power)
        want = ([bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)),
                bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))])
        counts["effect_hit"] = counts.get("effect_hit", 0) + 1
        results.setdefault("effect_hit", {}).setdefault(kind, 0)
        results["effect_hit"][kind] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH effect_hit(tile {tile}, kind {kind:#x}, power {power:#x}): areas "
                      f"{[n for n, (x, y) in enumerate(zip(got, want)) if x != y]}\n  calls original {got_log[:20]}\n"
                      f"  calls port     {log[:20]}")
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        e.uc.mem_write(rt(G_TILEMAP), snap[-2])
        e.uc.mem_write(rt(G_SOLDIERS), snap[-1])
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    e.uc.mem_write(rt(G_TILEMAP), tmap)
    e.uc.mem_write(rt(G_SOLDIERS), soldiers)
    blast_restore(real_)


EP_RNG = random.Random(3030)   # effect_pass: own stream
EPN_RNG = random.Random(3031)  # effect_pass: the nested-pass script (own stream: the case stream stays)
if PORT and direct("effect_pass"):   # (--port only) DrawProjectilePass? 0x17548 (the effect records 1..15; #70 row 30 / F69): hit, flag, fire
    FX_ = 0xEAFF0                       # record 0 (the pass starts at record 1, 0xEB002)
    sv_ = {a_: e.stubs.get(rt(a_)) for a_ in (0x1698C, 0x153C8, 0x173B4, 0x52DC8, 0x2406D)}
    # The overview copy (OverviewRedraw 0x23F63..0x2406D, view mode 0): one record per run, ESI = record, EBP = k;
    # limit 0x16 and no fire sound (0x24068 StartTileAnim only). The stub at 0x2406D (the loop's next record) returns.
    ep_cell_, ep_tramp_ = e.alloc(16), e.alloc(32)
    e.uc.mem_write(ep_tramp_, b"\x8B\x35" + struct.pack("<I", ep_cell_) + b"\x8B\x2D" + struct.pack("<I", ep_cell_ + 4)
                   + b"\x83\xEC\x20\xE9" + struct.pack("<i", rt(0x23F63) - (ep_tramp_ + 20)))

    def ep_flags(rd):
        return tuple(rd(FX_ + 0x12 * k_ + 0x10) for k_ in range(1, 16))

    # Nested pass (#70 row 30): a merc killed inside EffectHit? runs MercDies? -> TacticalRedraw(1) -> this pass again
    # before the outer pass sets the record's flag (1.13b; F69 sets it first). ep_nest = per case, one entry per hit:
    # 1 = this hit kills a merc. Original: EffectHit? 0x173B4 -> `call LOG; test eax,eax; jz; call 0x17548; ret`
    # (0x17548 = what TacticalRedraw runs; the real death path is in the merc_dies decider + live c13315).
    # Port: the hit handler runs the library pass again on the same buffers (merc_dies' op 358 EFFECT_PASS 0x13).
    ep_nest = []
    ep_log_ = e.alloc(b"\xC3" * 16)
    ep_code_ = e.alloc(16)
    e.uc.mem_write(ep_code_, b"\xE8" + struct.pack("<i", ep_log_ - (ep_code_ + 5)) + b"\x85\xC0\x74\x05\xE8"
                   + struct.pack("<i", rt(0x17548) - (ep_code_ + 14)) + b"\xC3")
    ep_old_ = bytes(e.uc.mem_read(rt(0x173B4), 5))

    def ep_hit(emu, eax, edx, ebx, ecx):    # EffectHit? as logged: tile, kind, power + every record's hit flag now
        log.append(("effect_hit", (ai.s16(eax), ai._i32(edx), ai._i32(ebx), ep_flags(lambda a_: e.r8(rt(a_))))))
        return ep_nest.pop(0) if ep_nest else 0

    def ep_hit_port(args_, bufs_, scalars_):
        fx_ = bufs_["fx_anims"]
        log.append(("effect_hit", (ai.s16(args_[0]), ai._i32(args_[1]), ai._i32(args_[2]),
                                   ep_flags(lambda a_: fx_[a_ - FX_]))))
        if ep_nest and ep_nest.pop(0):

            def call_(op, a_):
                a_ = [x_ & 0xFFFFFFFF for x_ in a_]
                if op in PORT_OP_HOOK:
                    return PORT_OP_HOOK[op](a_, bufs_, scalars_), None
                if op != "sfx":
                    raise SystemExit(f"port called host op {op} {a_} in the nested effect pass")
                log.append((op, conv(CALLEES[op][1].replace("s", "i").replace("x", ""), a_)))
                return take(op), None
            port_lib.sector_run("effect_pass", bufs_, scalars_, (0x13,), lambda n_: 0, call_)
        return 0

    def ep_anim_port(args_, bufs_, scalars_):    # op 205 kind 1: the front-end's StartTileAnim (fire table, 0x2B)
        port_lib.world_store(e, ai, bufs_, scalars_)
        ai.start_tile_anim(World(0), ai.s16(args_[0]), 0x9562E if args_[1] == 1 else 0x95659, 0x2B if args_[1] == 1 else 0x12)
        port_lib.world_load(e, ai, bufs_, scalars_)
        return 0
    e.stub(rt(0x1698C), lambda *a_: 0)      # the pictures (DrawSpriteAtWorld, DrawEffectFrame): front-end
    e.stub(rt(0x153C8), lambda *a_: 0)
    e.stub(rt(0x52DC8), lambda *a_: 0, 4)   # DrawCompiledSprite (overview kind-0 picture, y on the stack)
    e.stub(rt(0x2406D), lambda emu_, *a_: emu_.w32(emu_.uc.reg_read(UC_X86_REG_ESP), emu.RET_SENTINEL))
    e.stub(ep_log_, ep_hit)
    e.stubs.pop(rt(0x173B4), None)          # (restored from sv_ at the end) the patched entry runs, not a stub
    e.uc.mem_write(rt(0x173B4), b"\xE9" + struct.pack("<i", ep_code_ - (rt(0x173B4) + 5)))
    e.uc.ctl_flush_tb()
    PORT_OP_HOOK.update({"effect_hit": ep_hit_port, "tile_anim": ep_anim_port})
    area = [(FX_, 16 * 0x12), (ai.G_TILE_ANIMS, 10 * 0xB), (G_TILEMAP, 8 * 3136)]
    save = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in area]
    for i in range(6000):
        for k_ in range(1, 16):
            r_ = FX_ + 0x12 * k_
            e.w8(rt(r_), EP_RNG.choice([0, 0, 1, 5, 5, 0x13, 0x14, 0x16, 0x17, 0x20, 0x7F, 0x80, 0xFF]))
            e.w8(rt(r_ + 0xB), EP_RNG.choice([0, 2, 3, 3, 3, 4, 5, 0x80, 0xFF]))
            e.w8(rt(r_ + 0x10), EP_RNG.choice([0, 0, 1, 2]))
            e.w8(rt(r_ + 0x11), EP_RNG.choice([0, 1, 5, 0x7F, 0x80, 0xFF]))
            e.w32(rt(r_ + 0xC), rt(0x91000))   # (kind 0: the picture's frame table, read by the draw)
            t_ = EP_RNG.randrange(3136)
            e.w16(rt(r_ + 9), t_)
            e.w8(rt(G_TILEMAP) + 8 * t_ + 3, EP_RNG.choice([0, 0, 1, 7]))
            e.w8(rt(G_TILEMAP) + 8 * t_, EP_RNG.choice([0x6B, 0x6C, 0x10, 0xFF, EP_RNG.randrange(256)]))
        for k_ in range(10):                # tile animations: some free (count 0)
            e.w8(rt(ai.G_TILE_ANIMS) + 0xB * k_, EP_RNG.choice([0, 0, 0, 3, 0x2A]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in area]
        base = {"sfx": [0] * 40}
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        nest_ = [EPN_RNG.choice([0, 0, 0, 1]) for _ in range(6)] if EPN_RNG.random() < 0.5 else []
        ep_nest[:] = nest_
        ov_ = i % 3 == 2                    # every third case: the overview copy
        if ov_:
            for k_ in range(1, 16):
                e.w32(ep_cell_, rt(FX_ + 0x12 * k_))
                e.w32(ep_cell_ + 4, k_)
                e.call(ep_tramp_)
        else:
            e.call(rt(0x17548))
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in area]
        for (a_, k_), b_ in zip(area, snap):
            e.uc.mem_write(rt(a_), b_)
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        nested_ = len(nest_) - len(ep_nest) > 0 and any(nest_[:len(nest_) - len(ep_nest)])
        ep_nest[:] = nest_
        if PORT:
            port_sector("effect_pass", (0x16 if ov_ else 0x13,), allow=("sfx",))
        else:
            break
        want = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in area]
        counts["effect_pass"] = counts.get("effect_pass", 0) + 1
        n_ = sum(1 for c_ in got_log if c_[0] == "effect_hit")
        fire_ = any(rd_ != sn_ for rd_, sn_ in zip(got[1], snap[1]))
        key_ = ("ov " if ov_ else "") + f"hits {min(n_, 4)}" + (" fire" if fire_ else "") + (" nest" if nested_ else "")
        results.setdefault("effect_pass", {}).setdefault(key_, 0)
        results["effect_pass"][key_] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH effect_pass case {i}{' (overview)' if ov_ else ''}: areas {[n for n, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log[:8]}\n  calls port     {log[:8]}")
        for (a_, k_), b_ in zip(area, snap):
            e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(area, save):
        e.uc.mem_write(rt(a_), b_)
    for a_, v_ in sv_.items():
        if v_ is None:
            e.stubs.pop(rt(a_), None)
        else:
            e.stubs[rt(a_)] = v_
    for k_ in ("effect_hit", "tile_anim"):
        PORT_OP_HOOK.pop(k_, None)
    e.uc.mem_write(rt(0x173B4), ep_old_)
    e.uc.ctl_flush_tb()
if direct("turn_flow"):   # interrupt queue pop, NextTurn?, InterruptTurn?: called directly
    soldiers = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    area = [(ai.G_ITQ, 0x40), (ai.G_ITQ_N, 1), (0x92DD0, 3), (0x92DC7, 1), (ai.G_SELECTED, 1), (ai.G_SHOW_UI, 2),
            (0x284F0F, 2), (0x92E38, 4), (0x22A484, 4), (0x229CB4, 4), (0x93D54, 4), (0x92DCE, 1), (0x92DD2, 1), (0x92DCA, 1)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    stubs_ = {a: e.stubs.pop(rt(a), None) for a in (0x12D58, 0x12D55, 0x1304C)}
    for i in range(4000):
        which = i % 3
        nq = rng.choice([0, 1, 2, 3, 5, 8])
        e.uc.mem_write(rt(ai.G_ITQ), bytes(rng.choice([0x40, rng.randrange(16), rng.randrange(0x28, 0x30)]) for _ in range(0x40)))
        e.w8(rt(ai.G_ITQ_N), nq)
        for k in range(0x30):
            a_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(a_ + 0x182, rng.choice([1, 1, 0]))
            e.w8(a_ + 0x183, rng.choice([0, 14, 15, 40]))
            e.w8(a_ + 0x1B0, rng.choice([0, 0, 1, 3]))
            e.w8(a_ + 0x17E, rng.choice([0, 8, 8, 3]))
            e.w8(a_ + 0x194, rng.choice([0, 2]))
        e.w32(rt(0x92E38), rng.choice([0, 0, 0, 1, 0xFFFFFFFF]))
        e.w8(rt(0x92DD0), rng.choice([0, 1]))
        e.w8(rt(0x92DC7), rng.choice([0x40, 0x40, 3, 0x2A]))
        e.w8(rt(ai.G_SELECTED), rng.choice([2, 7, 8, 0x2A, 0x40]))
        e.w8(rt(ai.G_SHOW_UI), rng.choice([0, 1]))
        e.w8(rt(ai.G_SHOW_AI), rng.choice([0, 1]))         # RedrawTacticalPanel? 0x12CBC runs real in NextTurn?
        e.w8(rt(0x284F0F), rng.choice([0, 5, 7, 8, 0x40]))
        e.w16(rt(0x22A486), rng.choice([0, 20]))
        for k in range(8):
            e.w8(rt(G_SOLDIERS) + 0x200 * k + 0x138, rng.choice([1, 1, 0]))
        e.w8(rt(0x92DD2), rng.choice([0, 3, 7, 8, 0x40, 0xC0]))
        e.w8(rt(0x92DCA), rng.choice([0, 2]))
        mode = rng.choice([0, 1, 1, 2, 0x101])
        base = {"w_32828": [rng.choice([0, 3, 0x2A])], "redraw": [0], "turn_banner": [0], "w_10398": [0],
                "w_331a4": [0] * 16, "focus": [0], "w_1148c": [0], "w_67e30": [0], "w_53660": [0, 0]}
        script = {k_: list(v) for k_, v in base.items()}
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))]
        log.clear()
        addr = (0x12C14, 0x12D55 if 29 in (FAN or ()) else 0x12D58, 0x1304C)[which]   # #70 F29: 1.15m callers enter at 0x12D55
        got_r = e.call(rt(addr), mode) & 0xFF
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        e.uc.mem_write(rt(G_SOLDIERS), snap[-1])
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        w_ = World(0)
        w_.memrun = True
        if PORT:                         # turns.md: Interrupt queue, Next turn, Merc interrupt turn
            want_r = port_world(("itq_pop", "next_turn", "interrupt_turn")[which], (mode,) if which == 1 else ())
        else:
            want_r = ai.itq_pop(w_) if which == 0 else ai.next_turn(w_, mode) if which == 1 else ai.interrupt_turn(w_)
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))]
        name_ = ("itq_pop", "next_turn", "interrupt_turn")[which]
        counts[name_] = counts.get(name_, 0) + 1
        results.setdefault(name_, {}).setdefault(nq, 0)
        results[name_][nq] += 1
        if (got, got_log, got_r if which == 0 else 0) != (want, log, want_r if which == 0 else 0):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH {name_} case {i} (queue length {nq}, mode {mode}): ret {got_r} / {want_r}, areas "
                      f"{[j for j, (x, y) in enumerate(zip(got, want)) if x != y]}\n  calls original {got_log}\n  calls port     {log}")
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    e.uc.mem_write(rt(G_SOLDIERS), soldiers)
    for a, st in stubs_.items():
        if st:
            e.stubs[rt(a)] = st
if direct("turn_start"):   # ShowTurnBanner? 0x1054C (turn start) and AllyAlert? 0x74A60: called directly
    area = [(G_SOLDIERS, 0x6000), (G_TILEMAP - 8 * 64, 8 * (3136 + 128)), (combat.G_MOVECODE, 2 * 3136),
            (ai.G_GROUND_ITEMS, 12 * 256), (0x92D80, 0x100), (0x92E00, 0x80), (ai.G_BANNER_TEXT, 0x20),
            (0x284F00, 0x30), (0x22A000, 0x500), (0x229C80, 0x190)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    stubs_ = {a: e.stubs.pop(rt(a), None) for a in (0x1054C,)}
    for i in range(n // 3):
        which = i % 2                    # 0 turn start (on the selected soldier), 1 ally alert (on a soldier)
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30),
                           rng.randrange(0x28, 0x30), rng.randrange(16, 0x28)])
        if which == 0 and i % 8 == 0:    # the empty interrupt queue: "record 0x40" lies in the tile map (issue #2 row 25)
            slot = 0x40
        for k in range(0x30):
            a_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(a_ + 0x182, rng.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x138, rng.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x183, rng.choice([0, 14, 15, 40, 0x80]))
            e.w8(a_ + 0xBB, rng.choice([0, 0, 1, 1, 2, 3, 0xFF]))
            e.w8(a_ + 0x1C4, rng.choice([0, 0, 1]))
            e.w8(a_ + 0x17A, k)
            e.w8(a_ + 0x1A1, rng.randrange(1, 9))       # MercView reads stale stack bytes outside 1..8
        sp = rt(G_SOLDIERS) + 0x200 * slot
        e.w8(sp + 0x1B0, rng.choice([0, 0, 0, 0, 0, 1]) if slot < 0x30 else rng.choice([1, 4, 0x1D, 0x60]))   # ground: never 0
        e.w8(sp + 0x182, rng.choice([1] * 8 + [0]))
        e.w8(sp + 0x138, rng.choice([1] * 8 + [0]))
        e.w8(sp + 0x183, rng.choice([14, 15, 40, 40, 40, 0x80]))
        e.w8(sp + 0xBA, rng.choice([0, 1, 2, 3, 3, 3, 4, 5]))
        e.w8(sp + 0x19E, rng.choice([0, 5, 12, 20]))
        e.w8(sp + 0x197, rng.choice([0, 1]))
        e.w8(sp + 0x1C1, rng.choice([0, 1]))
        e.w8(sp + 0x1D4, rng.choice([0, 1, 2]))
        e.w8(sp + 0x1D5, rng.choice([0, 0, 1]))
        e.w8(sp + 0x18F, rng.choice([99, 50]))
        e.w8(rt(ai.G_SELECTED), slot)
        e.w8(rt(ai.G_BANNER_TEXT), 0)
        e.w8(rt(0x284F0F), rng.choice([0x40, 0x40, 3, 7, 8]))
        e.w8(rt(ai.G_ITQ_N), rng.choice([0, 0, 1, 3]))
        e.w8(rt(ai.G_ITQ), rng.choice([rng.randrange(16), rng.randrange(0x28, 0x30), 0x40]))
        e.w32(rt(ai.G_FAST_TIMER), rng.choice([0, 10, 5000]))
        e.w32(rt(ai.G_TURN_BASED), rng.choice([0, 1]))
        base = {"enemy_turned": [0], "guard_turned": [0], "share_sightings": [0], "perceive": [rng.choice([0, 1])] * 4,
                "tile_dist": [rng.choice([0, 3, 14, 15, 30, 0x10003]) for _ in range(16)],
                "sight_line": [rng.choice([0, 0, 1, 0x100]) for _ in range(16)],
                "Random": [rng.randrange(100) for _ in range(60)],
                "noise_tile": [(rng.choice([0xC41, rng.randrange(3136)]), rng.choice([0, 10, 50])) for _ in range(8)],
                **{k_: [0] * 40 for k_ in ("w_1f6e4", "quote_event", "w_197d8", "w_1148c", "stat_practice", "mine_flag",
                                           "stop_anim")}}
        script = collections.defaultdict(list, {k_: list(v) for k_, v in base.items()})
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        log.clear()
        if which == 0:
            e.call(rt(0x1054C))
        else:
            e.call(rt(0x74A60), sp)
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        script = collections.defaultdict(list, {k_: list(v) for k_, v in base.items()})
        log.clear()
        w_ = World(slot)
        w_.memrun = True
        if PORT:                         # turns.md Turn start, Ally alert
            port_world(("turn_start", "ally_alert")[which], () if which == 0 else (slot,), sync=True)
        elif which == 0:
            ai.turn_start(w_)
        else:
            ai.ally_alert(MemRec(slot), w_)
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        name_ = ("turn_start", "ally_alert")[which]
        counts[name_] = counts.get(name_, 0) + 1
        sb_ = got[0][0x200 * slot:0x200 * slot + 0x200]
        key_ = (got[6][:got[6].index(0)].decode("latin-1")[:9] if got[6] != snap[6] else "-") if which == 0 else \
            f"alert {sb_[0xBB]}"
        results.setdefault(name_, {}).setdefault(key_, 0)
        results[name_][key_] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH {name_} case {i} (slot {slot:#x}): areas "
                      f"{[hex(area[j][0]) for j, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log}\n  calls mirror   {log}")
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    for a, st in stubs_.items():
        if st:
            e.stubs[rt(a)] = st
if direct("escape_stop"):   # TacticalKeyHandler case Escape 0x13FF3..0x1409B (world_load.md "Quit"), run as a code
    # range with ESI = the selected record; EndTask, AnimStep, StopWalk and StopAnim stay stubbed
    area = [(G_SOLDIERS, 0x1000), (0x92DC0, 0x40), (0x229CB4, 4)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    tramp_, cell_ = e.alloc(16), e.alloc(16)   # (mov esi, [cell]; jmp 0x13FF3: written once, the code cache keeps it)
    e.uc.mem_write(tramp_, b"\x8B\x35" + struct.pack("<I", cell_) + b"\xE9" + struct.pack("<i", rt(0x13FF3) - (tramp_ + 11)))
    e.stub(rt(0x1409B), lambda emu_, *a_: emu_.uc.emu_stop())
    snap_stub_ = e.stubs.pop(rt(0x197D8), None)   # SnapToTile real (its StopAnim stays stubbed)
    for i in range(n // 4):
        sel_ = rng.choice([rng.randrange(8)] * 6 + [8, 0x2F, 0x40])
        sp = rt(G_SOLDIERS) + 0x200 * min(sel_, 7)
        e.w8(sp + 0x1F3, rng.randrange(3))
        for k_ in range(3):
            e.w16(sp + 0x1E8 + 2 * k_, rng.choice([0, 1, 2, 3, 4, 5, 0x10, 0x20, rng.randrange(0x40)]))
        e.w8(sp + 0x1B9, rng.choice([0, 0, 4, 5, 6, 9]))
        t_ = rng.randrange(3136)
        e.w16(sp + 0x114, t_)
        e.w16(sp + 0x124, rng.choice([t_, t_, rng.randrange(3136)]))
        for o_ in (0x116, 0x118):
            v_ = rng.randrange(0x400)
            e.w16(sp + o_, v_)
            e.w16(sp + o_ + 0x10, rng.choice([v_, v_, v_, rng.randrange(0x400)]))
        e.w16(sp + 0x120, rng.choice([ai.NO_TILE, t_, rng.randrange(3136)]))
        for o_ in (0x95, 0x1A6, 0x1A7, 0x197, 0x1F7, 0x1D8, 0x1C2, 0x1BE):
            e.w8(sp + o_, rng.randrange(256))
        for o_ in (0x96, 0x134, 0xD0, 0xDC):
            e.w16(sp + o_, rng.randrange(0x10000))
        e.w8(rt(ai.G_SELECTED), sel_)
        e.w8(rt(ai.G_COMBAT), rng.choice([0, 1]))
        e.w8(rt(ai.G_SHOW_UI), rng.choice([0, 1, 1]))
        e.w8(rt(0x92DCE), rng.choice([0, 0, 1]))
        e.w32(rt(0x229CB4), rng.choice([sel_, 0x40, rng.randrange(8)]))
        e.w32(cell_, rt(G_SOLDIERS) + 0x200 * sel_)
        script = collections.defaultdict(list)
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        log.clear()
        e.call(tramp_)
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        script = collections.defaultdict(list)
        log.clear()
        if PORT:
            port_world("escape_stop", (), sync=True)
        else:
            w_ = World(0)
            w_.memrun = True
            ai.escape_stop(w_)
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        key_ = tuple(sorted({c_[0] for c_ in log}))
        counts["escape_stop"] = counts.get("escape_stop", 0) + 1
        results.setdefault("escape_stop", {}).setdefault(key_, 0)
        results["escape_stop"][key_] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                bad_ = [[hex(j) for j in range(len(want[r_])) if got[r_][j] != want[r_][j]][:8] for r_ in range(len(area))]
                print(f"MISMATCH escape_stop case {i} sel {sel_}: {bad_}\n  calls original {got_log}\n  calls port     {log}")
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    del e.stubs[rt(0x1409B)]
    if snap_stub_:
        e.stubs[rt(0x197D8)] = snap_stub_
if direct("select_merc"):   # SelectMerc? 0x1148C (op 120) and SelectSoldier? 0x17D80: called directly
    # (select.md; PathApCost and FindPath real on both sides: the path preview marks tile byte 7)
    area = [(G_SOLDIERS, 0x6000), (0x92D80, 0x100), (0x92E00, 0x80), (0x284F00, 0x30), (0x229C80, 0x190),
            (G_TILEMAP, 8 * 3136), (ai.G_MARKS, 0x100), (ai.G_PATHS, 0x50 * 0x30), (0x1163B8, 0x10)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    stubs_ = {a: e.stubs.pop(rt(a), None) for a in (0x1148C, 0x17D80, CALLEES["path_ap"][0], CALLEES["find_path"][0])}
    for c_, (a_, k_) in SEL_STUBS.items():
        CALLEES[c_] = (a_, k_)
        e.stub(rt(a_), make_stub(c_, k_), 0)
    sel_local_ = {c_: e.stubs.get(rt(CALLEES[c_][0])) for c_ in ("w_681d0",)}
    for c_ in sel_local_:
        e.stub(rt(CALLEES[c_][0]), make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))

    class SelWorld(World):
        def ext(self, name, *args):      # Path AP: the mirror (real on the original side too)
            if name == "path_ap":
                return ai.path_ap_cost(*args[:1], self, *args[1:])
            return World.ext(self, name, *args)
    for i in range(n // 3):
        which = i % 2                    # 0 SelectMerc (op 120), 1 SelectSoldier
        slot = rng.choice([rng.randrange(8)] * 5 + [rng.randrange(8, 16)])
        for k in range(8):
            a_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(a_ + 0x1BD, rng.choice([0, 0x6A, 0x6E]))
            e.w8(a_ + 0x17A, k)
        sp = rt(G_SOLDIERS) + 0x200 * slot
        e.w8(sp + 0x183, rng.choice([0, 5, 14, 15, 50, 50, 0x80]))
        e.w8(sp + 0x182, rng.choice([1, 1, 1, 0]))
        e.w8(sp + 0x17E, rng.choice([0, 0, 0, 8]))
        e.w8(sp + 0x17A, slot)
        me_ = e.r16(sp + 0x114)
        if PORT and not 0 <= ai.s16(me_) < 3136:     # (the port's tiles hold the map only)
            me_ = rng.randrange(3136)
            e.w16(sp + 0x114, me_)
        e.w16(sp + 0x120, rng.choice([ai.NO_TILE, me_, rng.randrange(3136), rng.randrange(3136),
                                      min(3135, me_ + rng.choice([1, 2, 57, 3 * 56 + 2]))]))
        e.w8(sp + 0x16D, rng.choice([0, 1, 1, 1]))
        e.w8(sp + 0x1A5, rng.choice([0, 3, 10, 20, 0x80]))
        e.w8(sp + 0x1A8, rng.choice([0, 0, 1]))
        e.w8(sp + 0x177, rng.choice([0, 0, 1]))
        e.w8(sp + 0x196, rng.choice([0, 0, 1]))
        e.w8(rt(ai.G_SHOW_UI), rng.choice([0, 1, 1, 1, 1]))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot, rng.randrange(8), rng.randrange(8), 0x40, 8]))
        e.w32(rt(0x229CB4), rng.choice([slot, 0x40, rng.randrange(8)]))
        e.w8(rt(0x92DDF), rng.choice([0, 0, 1, 3, 3]))
        e.w8(rt(0x92DCE), rng.choice([0, 0, 0, 0, 0, 1]))
        e.w8(rt(0x92DEC), rng.choice([0x40, 0x40, 3]))
        e.w8(rt(0x92DC6), rng.choice([0x40, 0x40, 1]))
        e.w32(rt(0x229DD8), rng.choice([0, 0, 1, 0x80000000]))
        e.w32(rt(ai.G_MARKS_SHOWN), rng.choice([0, 0, 0, 0, 0, 1]))
        e.w32(rt(ai.G_MARKS_N), rng.choice([0, 0, 2]))
        e.w8(rt(0x1163BF), rng.choice([0, 1, 1]))
        e.w8(rt(0x92E58), rng.choice([slot, slot, 0x40]))
        e.w8(rt(ai.G_SPOT_STOP), rng.choice([0, 1]))
        e.w32(rt(ai.G_WALK_AP), rng.choice([0, 5, 20, 0xFFFFFFFF]))
        e.w8(rt(0x92DCB), rng.choice([0, 1]))
        e.w32(rt(0x229D04), rng.choice([0xB5, 0xB6, 0x100, 0x200]))
        e.w32(rt(ai.G_VIEW_Y), rng.choice([0, 0x100, 0x2F0]))
        e.w32(rt(0x229CC0), rng.choice([0, 0x20]))
        force, quote = rng.choice([0, 1, 0x100]), rng.choice([0, 1, 0x100])
        base = {}
        script = collections.defaultdict(list, {k_: list(v) for k_, v in base.items()})
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        log.clear()
        try:
            got_r = e.call(rt((0x1148C, 0x17D80)[which]), slot, force, quote, max_steps=400_000_000) & 0xFF
        except emu.EmuStop:
            print("CRASH", i, which, slot, force, quote, log, hex(e.r8(rt(0x92DEC))), hex(e.r8(rt(0x284F27))))
            raise
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        script = collections.defaultdict(list, {k_: list(v) for k_, v in base.items()})
        log.clear()
        name_ = ("select_merc", "select_soldier")[which]
        if PORT:                         # select.md Select merc, Select soldier
            want_r = port_world(name_, (slot, force, quote), sync=True) & 0xFF
        else:
            w_ = SelWorld(slot)
            w_.memrun = True
            want_r = (ai.select_merc, ai.select_soldier)[which](w_, slot, force, quote) & 0xFF
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        counts[name_] = counts.get(name_, 0) + 1
        key_ = (got_r, got[5] != snap[5])     # (result, tiles marked)
        results.setdefault(name_, {}).setdefault(key_, 0)
        results[name_][key_] += 1
        if (got, got_log, got_r) != (want, log, want_r):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH {name_} case {i} (slot {slot:#x}): ret {got_r} / {want_r}, areas "
                      f"{[hex(area[j][0]) for j, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log}\n  calls mirror   {log}")
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    for a, st in stubs_.items():
        if st:
            e.stubs[rt(a)] = st
    for c_, st in sel_local_.items():
        if st:
            e.stubs[rt(CALLEES[c_][0])] = st
        else:
            e.stubs.pop(rt(CALLEES[c_][0]), None)
    for c_, (a_, k_) in SEL_STUBS.items():
        if c_ != "w_46a4c":         # (stays: DeselectSoldier 0x17F10 runs real and calls it)
            del CALLEES[c_]
            e.stubs.pop(rt(a_), None)
if PORT and direct("kept_turn"):  # 0x31550 (library op 171): NewTurn? runs it after a traverse or arriving guards.
    # TurnSightings? real, its callees and NextTurn? stubbed. Enemy fields on the count's edges.
    area = [(0x92D80, 0x100), (0x229C80, 0x190), (0x22A400, 0x100), (0x22A0C0, 0x20), (0x284F00, 0x30),
            (ai.G_KNOWN, 0x30), (ai.G_REMEMBER_E, 0x1C0), (ai.G_BLED, 0x30), (G_SOLDIERS, 0x6000)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    st_ = e.stubs.pop(rt(0x31550), None)
    rk_ = random.Random(3155)
    base = {"enemy_turned": [0] * 8, "guard_turned": [0] * 16, "share_sightings": [0] * 16, "w_73d28": [0] * 64,
            "w_73f3c": [0] * 320, "next_turn": [0]}
    for i in range(max(n // 4, 600)):
        for k in range(0x30):
            a_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(a_ + 0x17A, k)
            e.w8(a_ + 0x182, rk_.choice([0, 1, 1, 1, 0x80]))
            e.w8(a_ + 0x138, rk_.choice([0, 1, 1, 1, 0x80]))
            e.w8(a_ + 0x183, rk_.choice([0, 14, 15, 16, 40, 0x7F, 0x80, 0xFF]))
            e.w8(a_ + 0x9C, rk_.choice([0, 0, 1, 3, 0x7F, 0x80, 0xFF]))
        e.w16(rt(0x22A478), rk_.choice([0, 1, 9, 0xFFFF]))
        e.w8(rt(0x92DD0), rk_.choice([0, 1, 1, 0x80]))
        e.w8(rt(ai.G_SELECTED), rk_.choice([0, 3, 7, 8, 9, 0x2A, 0x40]))
        e.w8(rt(0x284F0F), rk_.choice([0x40, 2]))
        script = collections.defaultdict(list, {k_: list(v) for k_, v in base.items()})
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        log.clear()
        e.call(rt(0x31550), max_steps=50_000_000)
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        got_seen = e.r16(rt(0x22A478))
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        script = collections.defaultdict(list, {k_: list(v) for k_, v in base.items()})
        log.clear()
        port_world("kept_turn", sync=True)
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        counts["kept_turn"] = counts.get("kept_turn", 0) + 1
        key_ = (min(got_seen, 3), any(c_[0] == "next_turn" for c_ in got_log))
        results.setdefault("kept_turn", {}).setdefault(key_, 0)
        results["kept_turn"][key_] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH kept_turn case {i}: areas "
                      f"{[(hex(area[j][0]), next(q for q in range(len(x)) if x[q] != y[q])) for j, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log[:12]}\n  calls port     {log[:12]}")
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    if st_:
        e.stubs[rt(0x31550)] = st_
if PORT and direct("swap_places"):  # SwapPlaces 0x40568 (key X): called directly on a merc's record and a tile next to
    # it; TurnSightings?, StepApCost, MsgNotEnoughAP, the tile and class reads real; its other callees stubbed
    area = [(0x92D80, 0x100), (0x229C80, 0x190), (0x22A400, 0x100), (0x22A0C0, 0x20), (0x284F00, 0x30),
            (ai.G_KNOWN, 0x30), (ai.G_REMEMBER_E, 0x1C0), (ai.G_BLED, 0x30), (G_SOLDIERS, 0x6000), (G_TILEMAP, 8 * 3136)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    st_ = e.stubs.pop(rt(0x40568), None)
    snap_ = e.stubs.pop(rt(0x197D8), None) if FAN and 13 in FAN else None   # F13: 1.15m PlaceAt calls SnapToTile
    base = {"enemy_turned": [0] * 8, "guard_turned": [0] * 16, "share_sightings": [0] * 16, "w_73d28": [0] * 64,
            "w_73f3c": [0] * 320, "next_turn": [0], "tile_dist": []}
    for i in range(max(n // 2, 600)):
        me = rng.randrange(8)
        o = rng.choice([rng.randrange(16)] * 5 + [rng.randrange(0x28, 0x30), me])
        mt = rng.randrange(57, 3136 - 57)
        t = rng.choice([mt + d for d in (1, -1, 56, -56, 57, -55)] * 3 + [mt + 2, mt + 112, mt])
        t = t if t < 3136 else mt - 112    # (the cursor tile is on the map; the port's tile reads end there)
        for k, tile_ in ((me, mt), (o, t)):
            a_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(a_ + 0x17A, k)
            e.w16(a_ + 0x114, tile_)
            e.w16(a_ + 0x116, rng.choice([(tile_ % 56) * 16 + 8, rng.randrange(896)]) & 0xFFFF)
            e.w16(a_ + 0x118, rng.choice([(tile_ // 56) * 14 + 7, rng.randrange(784)]) & 0xFFFF)
            e.w8(a_ + 0x183, rng.choice([0, 14, 15, 0x90] + [40] * 12))
            idx_ = rng.randrange(8)
            e.w8(a_ + 0x1F3, idx_)
            e.w16(a_ + 0x1E8 + 2 * idx_, rng.choice([0, 3, 4, 0xFFFF] + [9, 0x30] * 6))
            e.w8(a_ + 0x18A, rng.choice([0] + [1, 0x50] * 6))
            e.w8(a_ + 0x1A5, rng.choice([0, 1, 3, 0x90] + [10, 25] * 3))
            e.w8(a_ + 0x1E2, rng.choice([0, 0, 2, 0xFE]))
            e.w8(a_ + 0x1A8, rng.choice([0, 0, 1]))
            e.w8(a_ + 0x196, rng.choice([0, 0, 1]))
            e.w8(a_ + 0x177, rng.choice([0, 0, 1]))
            e.w8(a_ + 0x1B0, 0)
            e.w8(rt(G_TILEMAP) + 8 * tile_ + 5, rng.choice([k, k, k, k | 0x80, 0x40]))
        if FAN and 68 in FAN:      # #70 F68: the two step tiles (StepApCost) are sometimes broken bridge pieces
            for tile_ in (t, mt):
                f68_bridges(tile_, F68_RNG.choice([0, 0, 2]))
                if F68_RNG.random() < 0.6:
                    e.w16(rt(ai.G_MOVECODE) + 2 * tile_, F68_RNG.choice([25300, 25300, 25400]))
                    e.w8(rt(G_TILEMAP) + 8 * tile_, F68_RNG.choice([0x6C, 0x6C, 0x80, 0x6B, 0xFF]))
                    e.w8(rt(G_TILEMAP) + 8 * tile_ + 1, F68_RNG.choice([0x21, 0, 0x1C, 0x1F]))
        e.w8(rt(0x92DCB), rng.choice([0, 1, 1]))
        e.w8(rt(0x92DD0), rng.choice([0, 0, 1]))
        e.w8(rt(ai.G_SELECTED), rng.choice([me, me, 0x40, 3]))
        e.w8(rt(0x284F0F), rng.choice([0x40, 2]))
        dist_ = rng.choice([0, 1, 1, 1, 2])
        script = collections.defaultdict(list, {k_: list(v) for k_, v in base.items()})
        script["tile_dist"] = [dist_]
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        log.clear()
        e.call(rt(0x40568), rt(G_SOLDIERS) + 0x200 * me, t, max_steps=50_000_000)
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        script = collections.defaultdict(list, {k_: list(v) for k_, v in base.items()})
        script["tile_dist"] = [dist_]
        log.clear()
        port_world("swap_places", (me, t), sync=True)
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        counts["swap_places"] = counts.get("swap_places", 0) + 1
        key_ = tuple(c_[0] for c_ in got_log[:3])
        results.setdefault("swap_places", {}).setdefault(key_, 0)
        results["swap_places"][key_] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH swap_places case {i} (me {me}, o {o}, {mt} -> {t}): areas "
                      f"{[(hex(area[j][0]), next(q for q in range(len(x)) if x[q] != y[q])) for j, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log[:12]}\n  calls port     {log[:12]}")
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    if st_:
        e.stubs[rt(0x40568)] = st_
    if snap_:
        e.stubs[rt(0x197D8)] = snap_
if PORT and direct("next_enemy"):  # NextEnemy 0x3F7C8 (Enter on the tactical screen, in a fight): called directly on a
    # merc's record; FocusOn and StatusMessage stubbed (the port's op 125 text 15 = "No enemies visible!")
    area = [(G_SOLDIERS, 0x6000), (0x92D80, 0x100), (0x92E00, 0x80), (0x93440, 0x20)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    st_ = e.stubs.pop(rt(0x3F7C8), None)
    for i in range(max(n // 4, 200)):
        slot = rng.randrange(8)
        for k in range(0x28, 0x30):
            a_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(a_ + 0x17A, k)
            e.w8(a_ + 0x182, rng.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x138, rng.choice([1, 1, 1, 0, 5]))
            e.w8(a_ + 0x9C, rng.choice([0, 1, 1, 0x80, 3]))
            e.w8(rt(G_SOLDIERS) + 0x200 * slot + 0x112 + k, rng.choice([0, 1, 1, 2, 0xFF]))
        for k in range(0, 0x28, 7):     # (a focus_last outside 0x27..0x2F starts the search on mercs / guards)
            e.w8(rt(G_SOLDIERS) + 0x200 * k + 0x17A, k)
        e.w8(rt(G_SOLDIERS) + 0x200 * slot + 0x17A, rng.choice([slot] * 6 + [0x40]))
        e.w8(rt(0x92E53), rng.choice(list(range(0x27, 0x31)) + [0xFF, 5, 0x40]))
        e.w8(rt(0x92E52), rng.choice([0x40, 0x29]))
        e.w8(rt(0x92DC6), rng.choice([slot, slot, 0x40, 0x40, 3]))
        e.w8(rt(0x93458), rng.choice([0, 0, 0, 1]))
        script = collections.defaultdict(list)
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        log.clear()
        e.call(rt(0x3F7C8), rt(G_SOLDIERS) + 0x200 * slot, max_steps=10_000_000)
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        script = collections.defaultdict(list)
        log.clear()
        port_world("next_enemy", (slot,), sync=True)
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        counts["next_enemy"] = counts.get("next_enemy", 0) + 1
        key_ = tuple(c_[0] for c_ in got_log)
        results.setdefault("next_enemy", {}).setdefault(key_, 0)
        results["next_enemy"][key_] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH next_enemy case {i} (slot {slot}): areas "
                      f"{[hex(area[j][0]) for j, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log}\n  calls port     {log}")
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    if st_:
        e.stubs[rt(0x3F7C8)] = st_
if PORT and direct("next_merc"):   # NextMerc? 0x1315C (Space on the tactical screen): called directly; SelectMerc,
    # PathApCost and FindPath real on both sides (as in select_merc), HidePathMarks (ui_refresh) and FocusOn stubbed
    area = [(G_SOLDIERS, 0x1000), (0x92D80, 0x100), (0x92E00, 0x80), (0x284F00, 0x30), (0x229C80, 0x190),
            (G_TILEMAP, 8 * 3136), (ai.G_MARKS, 0x100), (ai.G_PATHS, 0x50 * 8), (0x1163B8, 0x10), (0x22979C, 80 * 9)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    stubs_ = {a: e.stubs.pop(rt(a), None) for a in (0x1315C, 0x1148C, CALLEES["path_ap"][0], CALLEES["find_path"][0])}
    for c_, (a_, k_) in SEL_STUBS.items():
        CALLEES[c_] = (a_, k_)
        e.stub(rt(a_), make_stub(c_, k_), 0)
    sel_local_ = {c_: e.stubs.get(rt(CALLEES[c_][0])) for c_ in ("w_681d0",)}
    for c_ in sel_local_:
        e.stub(rt(CALLEES[c_][0]), make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))
    for i in range(max(n // 6, 60)):
        for k in range(8):
            a_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(a_ + 0x1BD, rng.choice([0, 0x6A, 0x6E]))
            e.w8(a_ + 0x17A, k)
            e.w8(a_ + 0x183, rng.choice([0, 5, 15, 50, 50, 50, 0x80]))
            e.w8(a_ + 0x182, rng.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x17E, rng.choice([0, 0, 0, 8]))
            me_ = e.r16(a_ + 0x114)
            if not 0 <= ai.s16(me_) < 3136:     # (the port's tiles hold the map only)
                me_ = rng.randrange(3136)
                e.w16(a_ + 0x114, me_)
            e.w16(a_ + 0x120, rng.choice([ai.NO_TILE, me_, rng.randrange(3136), min(3135, me_ + rng.choice([1, 2, 57]))]))
            e.w8(a_ + 0x16D, rng.choice([0, 1]))
            e.w8(a_ + 0x1A5, rng.choice([0, 3, 10, 20, 0x80]))
            e.w8(a_ + 0x1A8, rng.choice([0, 0, 1]))
            e.w8(a_ + 0x177, rng.choice([0, 0, 1]))
            e.w8(a_ + 0x196, rng.choice([0, 0, 1]))
        slot = rng.randrange(8)
        sec_ = rng.choice([e.r8(rt(ai.G_CUR_SECTOR))] * 3 + [rng.randrange(80)])
        e.w8(rt(ai.G_CUR_SECTOR), sec_)
        if sec_ < 80:
            e.w8(rt(0x22979C) + 9 * sec_ + 2, rng.choice([0, 1, 3, 3]))
        e.w8(rt(ai.G_SHOW_UI), rng.choice([0, 1, 1, 1, 1]))
        e.w8(rt(ai.G_SELECTED), rng.choice([slot] * 8 + [rng.randrange(8), 0x40, 8]))
        e.w32(rt(0x229CB4), rng.choice([slot, 0x40, rng.randrange(8)]))
        e.w8(rt(0x92DDF), rng.choice([0, 0, 1, 3, 3]))
        e.w8(rt(0x92DCE), rng.choice([0, 0, 0, 0, 0, 1]))
        e.w8(rt(0x92DEC), rng.choice([0x40, 0x40, 3]))
        e.w8(rt(0x92DC6), rng.choice([0x40, 0x40, 1]))
        e.w32(rt(0x229DD8), rng.choice([0, 0, 1, 0x80000000]))
        e.w32(rt(ai.G_MARKS_SHOWN), rng.choice([0, 0, 0, 1]))
        e.w32(rt(ai.G_MARKS_N), rng.choice([0, 0, 2]))
        e.w8(rt(0x1163BF), rng.choice([0, 1, 1]))
        e.w8(rt(0x92E58), rng.choice([slot, slot, 0x40]))
        e.w8(rt(ai.G_SPOT_STOP), rng.choice([0, 1]))
        e.w32(rt(ai.G_WALK_AP), rng.choice([0, 5, 20, 0xFFFFFFFF]))
        e.w8(rt(0x92DCB), rng.choice([0, 1]))
        e.w32(rt(0x229D04), rng.choice([0xB5, 0xB6, 0x100, 0x200]))
        e.w32(rt(ai.G_VIEW_Y), rng.choice([0, 0x100, 0x2F0]))
        e.w32(rt(0x229CC0), rng.choice([0, 0x20]))
        script = collections.defaultdict(list)
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        log.clear()
        e.call(rt(0x1315C), slot, max_steps=400_000_000)
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        script = collections.defaultdict(list)
        log.clear()
        port_world("next_merc", (slot,), sync=True)
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        counts["next_merc"] = counts.get("next_merc", 0) + 1
        key_ = (e.r8(rt(ai.G_SELECTED)) != got[3][0x11], len(got_log))
        results.setdefault("next_merc", {}).setdefault(key_, 0)
        results["next_merc"][key_] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH next_merc case {i} (slot {slot}): areas "
                      f"{[hex(area[j][0]) for j, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log}\n  calls port     {log}")
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    for a, st in stubs_.items():
        if st:
            e.stubs[rt(a)] = st
    for c_, st in sel_local_.items():
        if st:
            e.stubs[rt(CALLEES[c_][0])] = st
        else:
            e.stubs.pop(rt(CALLEES[c_][0]), None)
    for c_, (a_, k_) in SEL_STUBS.items():
        if c_ != "w_46a4c":         # (stays: DeselectSoldier 0x17F10 runs real and calls it)
            del CALLEES[c_]
            e.stubs.pop(rt(a_), None)
if direct("turn_end"):   # FirstMerc?, TurnSightings?, EnemiesTurn?, EndTurn?, MercsTurn?: called directly
    area = [(0x92D80, 0x100), (0x229C80, 0x190), (0x22A400, 0x100), (0x22A0C0, 0x20), (0x284F00, 0x30),
            (ai.G_KNOWN, 0x30), (ai.G_REMEMBER_E, 0x1C0), (ai.G_BLED, 0x30), (G_SOLDIERS, 0x6000), (G_TILEMAP, 8 * 3136),
            (ai.G_GROUND_ITEMS, 12 * 200), (0x9300C, 1), (0x92F60, 4)]   # (time compression + tick step: #70 F18)
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    local_ = ("move_merc", "w_11fb4", "w_67bf0", "w_681d0", "w_67e18", "w_3f578")
    for c_ in local_:
        e.stub(rt(CALLEES[c_][0]), make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))
    mwait_te_ = [a_ for a_ in (0x67C54, 0x67C68) if rt(a_) not in e.stubs]   # TimeCompress(0)'s button-up waits
    for a_ in mwait_te_:                                                        # (F18): platform code, silent
        e.stub(rt(a_), lambda em_, *_: 0, 0)
    rr18_ = random.Random(18)              # (own stream: the cases of the other draws stay)
    real_start = ai.start_turn

    def bleed_(k_):                  # StartTurn? bleeds: g_bled of the soldier's id (step 6 of EnemiesTurn? reads it)
        if k_ in START_BLED:
            e.w8(rt(ai.G_BLED) + e.r8(rt(G_SOLDIERS) + 0x200 * k_ + 0x17A), START_BLED[k_])

    def start_(s_, w_):
        r_ = w_.ext("w_11fb4", s_)
        bleed_(s_.slot if hasattr(s_, "slot") else SP[s_.base])
        return r_
    ai.start_turn = start_
    st0_, pop0_ = e.stubs[rt(CALLEES["w_11fb4"][0])]

    def st11fb4_(emu, eax, edx, ebx, ecx):
        r_ = st0_(emu, eax, edx, ebx, ecx)
        bleed_(SP[eax])
        return r_
    e.stubs[rt(CALLEES["w_11fb4"][0])] = (st11fb4_, pop0_)
    names_ = ("first_merc", "turn_sightings", "enemies_turn", "end_turn", "mercs_turn")
    for i in range(5000):
        which = i % 5
        for k in range(0x30):
            a_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(a_ + 0x182, rng.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x138, rng.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x183, rng.choice([0, 9, 10, 14, 15, 40, 0x90]))
            e.w8(a_ + 0x188, rng.choice([0, 0, 0, 1]))
            e.w8(a_ + 0x17E, rng.choice([0, 0, 8, 8, 3]))
            e.w8(a_ + 0x1B0, rng.choice([0, 0, 1]))
            e.w8(a_ + 0x194, rng.choice([0, 2]))
            e.w8(a_ + 0x9C, rng.choice([0, 1, 0xFF, 3]))
            e.w8(a_ + 0x1A5, rng.choice([0, 3, 10, 25, 0x90]))
            e.w8(a_ + 0x95, rng.randrange(256))
            e.w8(a_ + 0x186, rng.choice([0, 0, 1, 0xFF, 2]))   # F15: a down enemy gets the slide
            e.w16(a_ + 0x96, rng.randrange(1 << 16))
            own = e.r16(a_ + 0x114)
            e.w16(a_ + 0x120, rng.choice([0xC41, 0xC41, own, rng.randrange(3136)]))
        e.uc.mem_write(rt(ai.G_ITQ), bytes(rng.choice([0x40, rng.randrange(16), rng.randrange(0x28, 0x30)]) for _ in range(0x40)))
        e.w8(rt(ai.G_ITQ_N), rng.choice([0, 0, 1, 2, 5]))
        e.uc.mem_write(rt(ai.G_REMEMBER_E), bytes(rng.choice([0, 0, 0, 1, 0xFF]) for _ in range(0x1C0)))
        e.uc.mem_write(rt(ai.G_BLED), bytes(rng.choice([0, 0, 1]) for _ in range(0x30)))
        e.uc.mem_write(rt(ai.G_KNOWN), bytes(rng.choice([0, 1, 2, 4, 0xFF, 0xFD, 0x7F]) for _ in range(0x30)))
        for f_ in range(5):
            b_ = rt(ai.G_FX) + 13 * f_
            e.w32(b_, rng.choice([0xFFFFFFFF, 0, 5]))
            e.w16(b_ + 4, rng.randrange(1 << 16))
            e.w16(b_ + 6, rng.choice([0xC41, 0xC41, rng.randrange(3136), rng.randrange(3136)]))
            for o_ in (8, 9, 0xA, 0xB, 0xC):
                e.w8(b_ + o_, rng.choice([0, 1, 2, 3, 4, 0xFF, rng.randrange(256)]))
        for a_, vals in ((0x92DDC, [0, 1]), (ai.G_SHOW_UI, [0, 1, 1]), (0x92DCD, [0, 1]), (0x22A0CD, [0, 1]),
                         (0x284F0F, [0x40, 0x40, 2, 7, 8]), (ai.G_SELECTED, [0, 3, 7, 8, 0x2A, 0x2F]), (0x92DEC, [0x40, 3]),
                         (0x22A0D0, [0, 1, 0xFF]), (0x22A0CE, [0, 1]), (0x92DA8, [0x40, 0x40, 3]), (0x92DDF, [0, 2, 3, 5]),
                         (0x284F20, [0, 1, 0xFF]), (0x284F21, [0, 1, 0xFF]), (0x92DBC, [0, 1, 2]), (0x92DC0, [0, 1, 2]),
                         (0x93458, [0, 0, 1, 0xFF])):
            e.w8(rt(a_), rng.choice(vals))
        e.w16(rt(0x92DAA), rng.choice([0xC41, 300]))
        e.w32(rt(0x229DF0), rng.choice([0, 4, 12, 30, 0xFFFFFFFF]))
        for a_, vals in ((0x229CB4, [0x40, 3, 9]), (0x229C94, [0x40, 5]), (0x229C8C, [0xC41, 100])):
            e.w32(rt(a_), rng.choice(vals))
        for a_, vals in ((0x22A486, [0, 7]), (0x22A484, [0, 7])):
            e.w16(rt(a_), rng.choice(vals))
        for a_, vals in ((0x284F26, [0, 3]), (0x92DCE, [0, 1]), (0x92DCA, [0, 1]), (0x92E52, [0x40, 2]),
                         (0x92DC7, [0x40, 2])):
            e.w8(rt(a_), rng.choice(vals))
        for t_ in [3135, 0] + [rng.randrange(3136) for _ in range(20)]:
            e.w8(rt(G_TILEMAP) + 8 * t_ + 7, rng.choice([0, 1, 0x80]))
        for f_ in range(5):                  # a timed effect on a tile with an explosive ground item
            if rng.random() < 0.4:
                b_ = rt(ai.G_FX) + 13 * f_
                t_, g_ = rng.randrange(3136), rng.randrange(200)
                typ_ = rng.choice([0x16, 0x18, 0x1B, 0x1C, 0x5D, 0x10, 0x40])
                e.w32(b_, 0)
                e.w8(b_ + 9, rng.choice([0, 0xFF, 1]))
                e.w16(b_ + 6, t_)
                e.w8(rt(G_TILEMAP) + 8 * t_ + 6, rng.choice([g_, g_, 200, 0xFF]))
                e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_, typ_)
                e.w8(b_ + 8, rng.choice([ai.explosive_kind(typ_)] * 3 + [rng.randrange(10)]))
        tc_ = rr18_.choice([0, 0, 1])      # F18: time compression on in some cases
        e.w8(rt(0x9300C), tc_)
        e.w32(rt(0x92F60), 1000 if tc_ else 10)
        flag = rng.choice([0, 1, 0x100])
        START_BLED.clear()
        START_BLED.update({k_: rng.choice([1, 1, 2]) for k_ in range(0x28, 0x30) if rng.random() < 0.4})
        base = {"w_11fb4": [0] * 8, "mission_cancel": [0], "turn_banner": [0], "w_17b70": [0] * 8, "enemy_turned": [0] * 8,
                "guard_turned": [0] * 16, "share_sightings": [0] * 16, "w_73d28": [0] * 64, "w_73f3c": [0] * 320,
                "explode": [0] * 5, "w_355d0": [0] * 5, "mouse_hide": [0], "w_46a4c": [0] * 4, "w_10454": [0],
                "w_331a4": [0] * 8, "w_67bf0": [0], "w_681d0": [0], "w_67e18": [0], "path_ap": [0], "move_merc": [0],
                "w_10398": [0], "focus": [0], "w_1148c": [rng.choice([0, 1, 0x100])], "w_3f578": [0] * 8}
        script = {k_: list(v) for k_, v in base.items()}
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        log.clear()
        addr = (0x2AC60, 0x11E2C, 0x12404, 0x121D8, 0x12DD4)[which]
        got_r = e.call(rt(addr), flag) & 0xFF
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        w_ = World(0)
        w_.memrun = True
        if PORT:
            want_r = port_world(names_[which], (flag,) if which == 1 else ())
        else:
            want_r = (ai.first_merc(w_) if which == 0 else ai.turn_sightings(w_, flag) if which == 1 else
                      ai.enemies_turn(w_) if which == 2 else ai.end_turn(w_) if which == 3 else ai.mercs_turn(w_))
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        name_ = names_[which]
        counts[name_] = counts.get(name_, 0) + 1
        key_ = got_r if which == 0 else len(got_log)
        results.setdefault(name_, {}).setdefault(key_, 0)
        results[name_][key_] += 1
        if (got, got_log, got_r if which == 0 else 0) != (want, log, want_r if which == 0 else 0):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH {name_} case {i}: ret {got_r} / {want_r}, areas "
                      f"{[(hex(area[j][0]), next(o for o in range(len(x)) if x[o] != y[o])) for j, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log[:30]}\n  calls port     {log[:30]}")
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
    ai.start_turn = real_start
    START_BLED.clear()
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    for c_ in local_:
        e.stubs.pop(rt(CALLEES[c_][0]), None)
    for a_ in mwait_te_:
        e.stubs.pop(rt(a_), None)
if direct("turn_new"):   # NewTurn? 0x126C8 called directly (turns.md New turn); its callees stubbed, FirstMerc?, AgeKnowledge?,
    # MercQuoteEvent? and MercBusy? real (library procedures in the port)
    area = [(ai.G_SECTOR_TBL9, 60 * 9), (0x92D80, 0x100), (0x93D50, 8), (0x229C80, 0x190), (0x229FA0, 0x356), (0x22A400, 0x100), (0x284F00, 0x30),
            (ai.G_KNOWN, 0x30), (ai.G_BLED, 0x30), (G_SOLDIERS, 0x6000), (G_TILEMAP, 8 * 3136)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    local_ = ("w_67e30", "w_6a960", "guards_arrive", "next_turn", "w_10398", "ai_think", "w_1583c", "gas_tick", "w_11fb4",
              "mouse_in_rect", "w_673ac", "w_124fc", "w_1259c", "w_1148c", "item_panel", "w_410c4", "w_297d4", "w_635dc",
              "stage_five", "w_17b70", "w_31550", "focus")
    for c_ in local_:
        e.stub(rt(CALLEES[c_][0]), make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))
    real_ = {k_: e.stubs.pop(rt(a_), None) for k_, a_ in (("w_2ac60", 0x2AC60), ("w_1f6e4", 0x1F6E4), ("merc_quote", 0x1B324))}
    keep_ = (ai.start_turn, ai.think, ai.gas_tick, ai.kill_stragglers)
    ai.start_turn = lambda s_, w_: w_.ext("w_11fb4", s_)
    ai.think = lambda s_, w_: w_.ext("ai_think", s_)
    ai.gas_tick = lambda w_: w_.ext("gas_tick")
    ai.kill_stragglers = lambda w_: w_.ext("stage_five")

    class NTWorld(World):       # the mirror: the callees that run for real in the original
        def ext(self, name, *args):
            if name == "w_2ac60":
                return ai.first_merc(self)
            if name == "w_741f8":
                return ai.age_knowledge(self, args[0])
            if name == "w_1f6e4":
                return int(any(self.g8(G_SOLDIERS + 0x200 * k + 0x182) and self.g8(G_SOLDIERS + 0x200 * k + 0x138) and
                               (ai.s8(self.g8(G_SOLDIERS + 0x200 * k + 0x1D5)) >= 1 or self.g8(G_SOLDIERS + 0x200 * k + 0x1C4))
                               for k in range(8)))
            if name == "merc_quote":
                return ai.merc_quote(MemRec(args[0]), self)
            return World.ext(self, name, *args)
    for i in range(3000):
        for k in range(0x30):
            a_ = rt(G_SOLDIERS) + 0x200 * k
            e.w8(a_ + 0x182, rng.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x138, rng.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x183, rng.choice([0, 9, 14, 15, 40, 0x90]))
            e.w8(a_ + 0x17E, rng.choice([0, 0, 8, 3]))
            e.w8(a_ + 0x9C, rng.choice([0, 1, 0xFF, 3]))
            e.w8(a_ + 0x1D5, rng.choice([0, 0, 0, 1, 0xFF]))
            e.w8(a_ + 0x1C4, rng.choice([0, 0, 0, 1]))
            e.w8(a_ + 0x17A, k)
        e.uc.mem_write(rt(ai.G_BLED), bytes(rng.choice([0, 0, 1]) for _ in range(0x30)))
        e.uc.mem_write(rt(ai.G_KNOWN), bytes(rng.choice([0, 1, 2, 4, 0xFF, 0xFD, 0x7F]) for _ in range(0x30)))
        for a_, vals in ((ai.G_COMBAT, [0, 1, 1]), (0x92DDD, [0, 0, 1]), (0x92DD0, [0, 0, 1]), (0x92DC7, [0, 3, 7, 0x2A, 0x30, 0x40]),
                         (ai.G_SELECTED, [0, 3, 7, 8, 9, 0x2A, 0x40]), (0x92DEC, [0x40, 0x40, 2, 9]), (0x92DD5, [0x40, 0x40, 1, 5]),
                         (0x22A1F2, [0, 3, 4, 5]), (0x22A1F3, [0, 1]), (0x284F02, [0, 0, 1, 2]), (ai.G_SHOW_UI, [0, 1]),
                         (0x22A0C2, [0, 1, 2]), (0x284F0F, [0x40, 2]), (0x92DBC, [0, 1, 2]), (0x92DC0, [0, 1, 2])):
            e.w8(rt(a_), rng.choice(vals))
        if rng.random() < 0.3:
            e.w8(rt(ai.G_CUR_SECTOR), 0)
        e.w8(rt(0x22A075) + e.r8(rt(ai.G_CUR_SECTOR)) % 60, rng.choice([0, 0, 1]))
        sec_ = rt(ai.G_SECTOR_TBL9) + 9 * e.r8(rt(ai.G_CUR_SECTOR))
        e.w8(sec_ + 2, rng.choice([0, 1, 4]))
        e.w8(sec_ + 1, rng.choice([0, 1, 6]))
        e.w32(rt(0x92DA4), rng.choice([0, 0, 1]))
        for f_ in range(5):
            e.w8(rt(ai.G_STORY_TIMERS) + 13 * f_, rng.choice([0, 1, 5, 0xFF]))
        focus = rng.choice([0, 1, 0x100])
        base = {"w_67e30": [0] * 4, "w_6a960": [0], "guards_arrive": [rng.choice([0, 0, 0, 1, 0x100])], "next_turn": [0],
                "w_10398": [0], "ai_think": [0] * 8, "w_1583c": [0], "gas_tick": [0], "w_11fb4": [0] * 0x28,
                "mouse_in_rect": [rng.choice([0, 1, 0x100])] * 2, "w_673ac": [0] * 2, "w_124fc": [0], "w_1259c": [0],
                "w_1148c": [0], "item_panel": [0], "w_410c4": [rng.choice([0, 1, 0x100])], "w_297d4": [0], "w_635dc": [0],
                "stage_five": [0], "w_17b70": [0] * 0x28, "w_31550": [0], "focus": [0] * 2,
                "merc_message": [0] * 2, "cry": [0], "voc": [0]}
        script = {k_: list(v) for k_, v in base.items()}
        snap = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        log.clear()
        got_r = e.call(rt(0x126C8), focus) & 0xFF
        got_log = list(log)
        got = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        script = {k_: list(v) for k_, v in base.items()}
        log.clear()
        w_ = NTWorld(0)
        w_.memrun = True
        if PORT:
            want_r = port_world("new_turn", (focus,)) & 0xFF
        else:
            want_r = ai.new_turn(w_, focus) & 0xFF
        want = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
        counts["new_turn"] = counts.get("new_turn", 0) + 1
        results.setdefault("new_turn", {}).setdefault(len(got_log), 0)
        results["new_turn"][len(got_log)] += 1
        if (got, got_log, got_r) != (want, log, want_r):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH new_turn case {i}: ret {got_r} / {want_r}, areas "
                      f"{[(hex(area[j][0] + next(o for o in range(len(x)) if x[o] != y[o])), x[next(o for o in range(len(x)) if x[o] != y[o])], y[next(o for o in range(len(x)) if x[o] != y[o])]) for j, (x, y) in enumerate(zip(got, want)) if x != y]}\n"
                      f"  calls original {got_log[:30]}\n  calls port     {log[:30]}")
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
    ai.start_turn, ai.think, ai.gas_tick, ai.kill_stragglers = keep_
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    for c_ in local_:
        e.stubs.pop(rt(CALLEES[c_][0]), None)
    for k_, st_ in real_.items():
        if st_:
            e.stubs[rt(CALLEES[k_][0])] = st_
if direct("use_throw"):   # item entry argument: called directly on a scratch soldier entry
    stub = e.stubs.pop(rt(0x535E8), None)
    stub2 = e.stubs.pop(rt(0x470B0), None)          # the original's DropItem runs for real here
    k_slot = 3
    base_ = G_SOLDIERS + 0x200 * k_slot
    keep = bytes(e.uc.mem_read(rt(base_), 0x90))
    throw_ = [t for t in range(136) if e.r8(rt(ai.ITEM_TBL) + 12 * t + 2) == 0x95]
    for i in range(3000):
        j_ = rng.randrange(12)
        ent = bytes([rng.choice(throw_ + [rng.randrange(136)]), rng.choice([0, 1, 2, 3, 4, 5, 0x80, 0xFF])]
                    + [rng.randrange(256) for _ in range(10)])
        e.uc.mem_write(rt(base_ + 12 * j_), ent)
        snap = bytes(e.uc.mem_read(rt(base_), 0x90))
        e.call(rt(0x535E8), rt(base_ + 12 * j_))
        got = bytes(e.uc.mem_read(rt(base_), 0x90))
        e.uc.mem_write(rt(base_), snap)
        ai.use_throw(World(0), base_ + 12 * j_)
        want = bytes(e.uc.mem_read(rt(base_), 0x90))
        counts["use_throw"] = counts.get("use_throw", 0) + 1
        results.setdefault("use_throw", {}).setdefault(ent[1], 0)
        results["use_throw"][ent[1]] += 1
        if got != want:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH use_throw(entry {ent.hex()}): original {got[12 * j_:12 * j_ + 12].hex()} port {want[12 * j_:12 * j_ + 12].hex()}")
    e.uc.mem_write(rt(base_), keep)
    if stub:
        e.stubs[rt(0x535E8)] = stub
    if stub2:
        e.stubs[rt(0x470B0)] = stub2
if direct("explode"):   # no soldier record: called directly with the stub lifted
    stub = e.stubs.pop(rt(0x3B79C))
    real_ = blast_real()
    tmap = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
    soldiers = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))
    area = [(0x229D50, 60), (ai.G_BLAST_FX, 1), (ai.G_BLAST_FX_ID, 4), (ai.G_BLASTER, 1), (ai.G_VIEW_X, 8),
            (0x22A038, 1), (0x92E41, 0x40), (ai.G_MOVECODE, 2 * 3136)]
    save = [bytes(e.uc.mem_read(rt(a), k)) for a, k in area]
    OBJS = [0, 0x10, 0x26, 0x27, 0x30, 0x37, 0x3B, 0x3D, 0x50, 0x82, 0xE1]
    for i in range(3000):
        slot = rng.choice([rng.randrange(8), rng.randrange(8, 16), rng.randrange(0x28, 0x30), 0x40, rng.randrange(0x30, 0x40)])
        tile = rng.randrange(3136)
        for d in (0, 1, -1, 56, -56):
            if 0 <= tile + d < 3136:
                e.w8(rt(G_TILEMAP) + 8 * (tile + d) + 1, rng.choice(OBJS + [rng.randrange(256)]))
        e.w8(rt(G_TILEMAP) + 8 * tile + 5, rng.choice([0x40, 0x7F, rng.randrange(0x30), 0x80 | rng.randrange(0x30)]))
        e.uc.mem_write(rt(0x229D50), b"".join(rng.choice([tile, ai.NO_TILE, rng.randrange(3136)]).to_bytes(2, "little")
                                              for _ in range(30)))
        e.w32(rt(ai.G_VIEW_X), rng.choice([0, 100, 400, ai.tile_x(tile) - 50]) & 0xFFFFFFFF)
        e.w32(rt(ai.G_VIEW_Y), rng.choice([0, 100, 400, ai.tile_y(tile) - 50, ai.tile_y(tile) + rng.randrange(-30, 40)])
              & 0xFFFFFFFF)
        e.w8(rt(0x22A038), rng.randrange(0x40))
        if PORT:                # (port fuzz clamp: no_break has one byte per tileset 0..9)
            e.w8(rt(0x22A038), e.r8(rt(0x22A038)) % 10)
        e.w8(rt(0x92E41) + e.r8(rt(0x22A038)), rng.choice([0, 0, 1]))
        kind = rng.choice([2, 2, 3, 3, 4, 5, 6, 9, 10])
        hit = rng.choice([0, 1, 0xFFFFFFFF, 0x10005, rng.randrange(1 << 32)])
        base = {"puff": [rng.choice([0, 1, 5, 0x80, 0x1FF])], "sfx": [0], "break_obj": [0], "make_noise": [0]}
        script = {k: list(v) for k, v in base.items()}
        snap = ([bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)),
                bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))])
        log.clear()
        e.call(rt(0x3B79C), slot, tile, kind, hit)
        got_log = list(log)
        got = ([bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)),
               bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))])
        for (a, k), b in zip(area, snap):
            e.uc.mem_write(rt(a), b)
        e.uc.mem_write(rt(G_TILEMAP), snap[-2])
        e.uc.mem_write(rt(G_SOLDIERS), snap[-1])
        script = {k: list(v) for k, v in base.items()}
        log.clear()
        if PORT:
            port_sector("explode", (slot, tile, kind, ai._i32(hit)), allow=("puff", "sfx", "break_obj", "make_noise"))
        else:
            ai.explode(World(0), slot, tile, kind, hit)
        want = ([bytes(e.uc.mem_read(rt(a), k)) for a, k in area] + [bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136)),
                bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x6000))])
        counts["explode"] = counts.get("explode", 0) + 1
        results.setdefault("explode", {}).setdefault(kind, 0)
        results["explode"][kind] += 1
        if (got, got_log) != (want, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH explode(slot {slot:#x}, tile {tile}, kind {kind}, hit {hit:#x}): areas "
                      f"{[n for n, (x, y) in enumerate(zip(got, want)) if x != y]}\n  calls original {got_log}\n  calls port     {log}")
    for (a, k), b in zip(area, save):
        e.uc.mem_write(rt(a), b)
    e.uc.mem_write(rt(G_TILEMAP), tmap)
    e.uc.mem_write(rt(G_SOLDIERS), soldiers)
    e.stubs[rt(0x3B79C)] = stub
    blast_restore(real_)
if direct("door_fx"):   # (last section: its fuzz changed later sections' state) PlayDoorFx? 0x3B03C (op 70): door value, lock pick, keys (0x3AE78), results 6..11: called directly
    FX_R = {"KEY": (6, 9), "PCK": (7, 10), "HAN": (8, 11)}
    FX_K = {r_: k_ for k_, rs_ in FX_R.items() for r_ in rs_}

    def door_anim_stub(emu, eax, edx, ebx, ecx):      # 0x5BFF8 (name "I_PDKEY4.FLC", x, y, 0, 1): (big, kind, letter)
        b_ = bytes(emu.uc.mem_read(eax, 32))
        nm_ = b_[:b_.index(0)].decode("latin-1")
        log.append(("door_anim", (int(nm_[2:4] == "PD"), nm_[4:7], nm_[0]) + ((ai._i32(edx), ai._i32(ebx)) if PORT else ())))
        return 0

    def door_snd_stub(which_):                        # 0x31BBC door sounds (0) / 0x31B44 overhead sounds (1)
        def fn_(emu, *_):
            log.append(("door_snd", (which_,)))
            return 0
        return fn_

    DOOR_LIB = None
    if PORT:                                          # the front-end's name stem (sound.rs door_base), for the port's op 210
        DOOR_LIB = ctypes.CDLL(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "port", "ja1front", "target", "release", "libja1front.so"))
        DOOR_LIB.ja1f_door_base.argtypes = [ctypes.c_int32] * 3 + [ctypes.c_void_p, ctypes.c_int32]

    def door_anim_log(big_, r_, ph_, *xy_):           # the port's (big, r, phase, x, y) as the original's file name shows it
        if DOOR_LIB:                                  # "I_PDKEY": (big, kind, letter) of the name the front-end asks for
            b_ = ctypes.create_string_buffer(16)
            n_ = DOOR_LIB.ja1f_door_base(big_, r_, ph_, b_, 16)
            nm_ = b_.raw[:n_].decode("latin-1")
            log.append(("door_anim", (int(nm_[2:4] == "PD"), nm_[4:7] or f"r {r_}", nm_[:1]) + tuple(xy_)))
            return
        log.append(("door_anim", (big_ & 0xFF, FX_K.get(r_, f"r {r_}"), "I" if not ph_ else "S" if r_ >= 9 else "F") + tuple(xy_)))

    class DoorWorld(World):
        def ext(self, name, *args):
            if name == "door_snd":
                log.append((name, (args[0],)))
                return 0
            if name == "door_anim":
                door_anim_log(*args)
                return 0
            return World.ext(self, name, *args)
    DOOR_HOST[:] = [door_anim_log]
    lifted_ = {a_: e.stubs.pop(rt(a_)) for a_ in (0x3AFE0, 0x470B0, 0x3B03C, 0x3AE78) if rt(a_) in e.stubs}
    e.stub(rt(0x31BBC), door_snd_stub(0))
    e.stub(rt(0x31B44), door_snd_stub(1))
    e.stub(rt(0x5CE08), lambda *_: 0)
    e.stub(rt(0x5EDE0), lambda *_: 0)
    e.stub(rt(0x5BFF8), door_anim_stub, 4)
    dr = random.Random(1000003 * 70 + 5)            # own random source: the shared rng stays as it was
    sec_ = e.r8(rt(ai.G_CUR_SECTOR))
    REC_ = (ai.G_SECTORS + 0xB1 * sec_, 0x30)
    SEL_ = (0x284F11, 1)
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in (REC_, SEL_, (0x92E36, 1), (G_SOLDIERS, 0x200 * 0x30))]
    pockets_ = [ai.s8(e.r8(rt(ai.POCKET_SLOTS + i_))) for i_ in range(7)]
    VIEW_ = [(0x92DDF, 1), (ai.G_VIEW_X, 8), (ai.G_VIEW_W, 8), (0x229CB8, 4), (0x229CC4, 4), (0x229CD8, 8)]
    vsave_ = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in VIEW_]
    for i in range(n // 3):
        if PORT:    # the film place: a view mode (SetViewWindow 0x24880 globals) and a view position at random
            md_ = dr.choice([1, 2, 3, 3, 0])
            cols_, rows_, left_, top_ = {2: (20, 12, 0, 14), 3: (20, dr.choice([13, 14]), 0, 0)}.get(md_, (14, 12, 48, 14))
            e.w8(rt(0x92DDF), md_)
            for a_, v_ in ((ai.G_VIEW_X, dr.choice([0, 16 * dr.randrange(43), dr.randrange(672)])),
                           (ai.G_VIEW_Y, dr.choice([0, 14 * dr.randrange(45), dr.randrange(630)])),
                           (ai.G_VIEW_W, 16 * cols_), (ai.G_VIEW_H, 14 * rows_), (0x229CB8, left_), (0x229CC4, top_),
                           (0x229CD8, left_ + 16 * cols_ - 1), (0x229CDC, top_ + 14 * rows_ - 1)):
                e.uc.mem_write(rt(a_), (v_ & 0xFFFFFFFF).to_bytes(4, "little"))
        slot = dr.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        tile = dr.randrange(3136)
        v = dr.choice([0, 0, 1, 5, 20, 35, 50, 90, 100, 101, 102, 130, 150, 200, 227, 228, 300, 0x7FFF, 0x8000,
                       0xFFFF, 0xFF9C, dr.randrange(0x10000), dr.randrange(1, 201), dr.randrange(1, 201)])
        vk = ai.s16(v - 100 if 101 <= ai.s16(v) else v)  # the value a key must match
        rec_ = bytearray(e.uc.mem_read(rt(REC_[0]), REC_[1]))
        k_ = dr.randrange(12)
        for j_ in range(12):
            if j_ == k_ or dr.random() < 0.1:
                ai.w16(rec_, 4 * j_, tile if j_ == k_ else dr.choice([tile, dr.randrange(3136)]))
                ai.w16(rec_, 4 * j_ + 2, v if j_ == k_ else dr.randrange(0x10000))
        if dr.random() < 0.1:
            ai.w16(rec_, 4 * k_, (tile + 1) % 3136)       # no pair for the tile: value 0
        e.uc.mem_write(rt(REC_[0]), bytes(rec_))
        idv = lambda: dr.choice([vk, vk, vk + 1, 0, dr.randrange(256)]) & 0xFF
        for p_ in pockets_ + [0x10]:               # hand + pockets (+ entry 16, not a pocket): keys, lock picks, others
            if p_ < 0 or p_ > 0x10:
                continue
            b_ = 12 * p_
            if dr.random() < 0.6:
                s[b_] = dr.choice([0x57, 0x57, 0x58, 0x58, 0x4C, 0x4C, 0, 0x76, dr.randrange(136)])
                s[b_ + 1] = dr.choice([0, 1, 1, 2, 3, 4, 5, dr.randrange(1, 6)])
                for m_ in range(2, 12):
                    s[b_ + m_] = idv()
                if s[b_] not in (0x57, 0x58):
                    s[b_ + 2] = dr.choice([0, 1, 10, 50, 99, 100, 0x7F, 0x80, dr.randrange(256)])
        for o_ in range(0x189, 0x193):
            s[o_] = dr.choice([0, 1, 10, 35, 50, 70, 90, 100, 0x7F, 0x80, dr.randrange(256), dr.randrange(101)])
        if s[0] == 0x4C and dr.random() < 0.6:      # lock pick: a door value at the skill boundary
            for _ in range(12):     # draw until the boundary is a door value 1..100 (skill bytes all differ)
                for o_ in (0x18C, 0x18D, 0x190, 0x192, 2):
                    s[o_] = dr.choice([s[o_], 100, 99, 0x7F, dr.randrange(90, 128), dr.randrange(20, 101)])
                s[0x192] = dr.choice([s[0x192], 0, 1, 0xFF, dr.randrange(10)])
                s[2] = dr.choice([s[2], 100, dr.randrange(1, 101), dr.randrange(1, 30)])
                sk_ = ai.cdiv(ai.cdiv(ai.s8(s[0x18D]) * ai.s8(s[0x190]), 100) * ai.s8(s[0x18C]), 100) + ai.s8(s[0x192]) * 5
                val_ = ai.cdiv(ai.s8(s[2]) * sk_, 100) + dr.choice([0, 1])
                if 1 <= val_ <= 100:
                    break
            if 1 <= val_ <= 100:
                v = val_ + dr.choice([0, 100])
                ai.w16(rec_, 4 * k_, tile)
                ai.w16(rec_, 4 * k_ + 2, v)
                e.uc.mem_write(rt(REC_[0]), bytes(rec_))
        s[0x1B3] = dr.randrange(256)
        task = dr.choice([0xFF, 0xFF, 0, 1, 0x46, dr.randrange(256)])
        e.w8(rt(0x284F11), dr.choice([slot, slot, 0x40, (slot + 1) % 0x30]))
        e.w8(rt(0x92E36), 1 if PORT else dr.choice([0, 1, 5, 0x7F]))     # (the port passes 1: combat.md Doors)
        e.uc.mem_write(sp, bytes(s))
        base_ = {"Random": [dr.choice([0, 1, 2, 5, 9, 10, 49, 50, 99, dr.randrange(0x7FFF)]) for _ in range(4)],
                 **{k2_: [0] * 4 for k2_ in ("stat_practice", "item_cond", "break_obj", "make_noise")}}
        script = {k2_: list(v_) for k2_, v_ in base_.items()}
        log.clear()
        e.call(rt(0x3B03C), sp, tile, task)
        got_s, got_log = bytes(e.uc.mem_read(sp, 0x200)), list(log)
        e.uc.mem_write(sp, bytes(s))
        script = {k2_: list(v_) for k2_, v_ in base_.items()}
        log.clear()
        ps = bytearray(s)
        if PORT:
            port_world("door_fx", (slot, tile, task))
            ps = bytearray(e.uc.mem_read(sp, 0x200))
        else:
            ai.door_fx(ps, DoorWorld(slot), tile, task)
        an_ = [c_[1] for c_ in log if c_[0] == "door_anim"]
        key_ = (an_[-1][1:3] if an_ else None)
        counts["door_fx"] = counts.get("door_fx", 0) + 1
        results.setdefault("door_fx", {}).setdefault(key_, 0)
        results["door_fx"][key_] += 1
        if (got_s, got_log) != (bytes(ps), log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH door_fx slot {slot:#x} tile {tile:#x} v {v:#x} task {task:#x}: record "
                      f"{[hex(o) for o in range(0x200) if got_s[o] != ps[o]]}\n  calls original {got_log}\n  calls port     {log}")
        e.uc.mem_write(sp, bytes(s))
    for (a_, k_), b_ in zip((REC_, SEL_, (0x92E36, 1), (G_SOLDIERS, 0x200 * 0x30)), tsave):   # (later sections see the
        # soldiers as before: the fuzzed inventories changed sector_start's DropItem calls)
        e.uc.mem_write(rt(a_), b_)
    for (a_, k_), b_ in zip(VIEW_, vsave_):
        e.uc.mem_write(rt(a_), b_)
    for a_ in (0x31BBC, 0x31B44, 0x5CE08, 0x5EDE0, 0x5BFF8):
        del e.stubs[rt(a_)]
    DOOR_HOST.clear()
    e.stubs.update({rt(a_): f_ for a_, f_ in lifted_.items()})
if direct("reload"):    # (end of file: own random source) MsgReloaded? 0x40094 (host op 190 "reloaded"): a clip from a pocket loads the gun
    lifted_ = {a_: e.stubs.pop(rt(a_)) for a_ in (0x40094, 0x470B0) if rt(a_) in e.stubs}
    dr = random.Random(1000003 * 190 + 7)
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in (((0x284F11, 1), (0x92E36, 1), (0x1163BD, 1),
                                                              (G_SOLDIERS, 0x200 * 0x30)))]
    pockets_ = [e.r8(rt(ai.POCKET_SLOTS + i_)) for i_ in range(7)]
    for i in range(n // 6):
        slot = dr.randrange(0x30)
        sp = rt(G_SOLDIERS) + slot * 0x200
        s = bytearray(e.uc.mem_read(sp, 0x200))
        s[0x17A] = slot
        gun = dr.choice([1, 2, 3, 5, 7, 8, 9, 11, 13, 14, dr.randrange(1, 15), dr.randrange(1, 15), 0, 0x30,
                         dr.randrange(136)])
        ammo = e.r8(rt(ai.WEAPON_TBL + 1 + 6 * gun))
        s[0] = gun
        s[1] = dr.choice([0, 0, 0, 1, 5, 0x7F, 0xFF, dr.randrange(256)])
        for p_ in pockets_[1:] + [0x10]:          # pockets (+ entry 16, not a pocket): clips of this gun, others
            b_ = 12 * p_
            if dr.random() < 0.5:
                s[b_] = dr.choice([ammo, ammo, ammo + 1, ammo - 1, 0, dr.randrange(136)]) & 0xFF
                s[b_ + 1] = dr.choice([0, 1, 1, 2, 3, 4, 5, 0x80, dr.randrange(256)])
                for m_ in range(2, 12):
                    s[b_ + m_] = dr.randrange(256)
                if PORT and 0x8D <= b_ + s[b_ + 1] + 6 <= 0x93 and s[b_ + 1] < 0x80:
                    s[b_ + 1] = 5     # (step 5 would write into +0x90, a pointer here and a script offset in the port)
            elif s[b_] == ammo and dr.random() < 0.7:
                s[b_] = 0
        s[0x1D5] = dr.choice([0, 1, 0x7F, 0x80, 0xFF, dr.randrange(256)])
        e.w8(rt(0x284F11), dr.choice([slot, slot, 0x40, (slot + 1) % 0x30]))
        e.w8(rt(0x92E36), 1 if PORT else dr.choice([0, 1, 5, 0x7F]))     # (the port passes 1, as for the doors)
        e.w8(rt(0x1163BD), dr.choice([0, 1, 5]))
        e.uc.mem_write(sp, bytes(s))
        base_ = {k2_: [0] * 4 for k2_ in ("stat_practice", "item_cond", "sfx", "merc_text")}
        script = {k2_: list(v_) for k2_, v_ in base_.items()}
        log.clear()
        want_r = e.call(rt(0x40094), sp)
        got_s, got_log = bytes(e.uc.mem_read(sp, 0x200)), list(log)
        e.uc.mem_write(sp, bytes(s))
        script = {k2_: list(v_) for k2_, v_ in base_.items()}
        log.clear()
        ps = bytearray(s)
        if PORT:
            r_ = port_world("reload", (slot,))
            ps = bytearray(e.uc.mem_read(sp, 0x200))
        else:
            r_ = ai.reload_gun(ps, World(slot))
        key_ = "reloaded" if want_r == 0 else "no clip"
        counts["reload"] = counts.get("reload", 0) + 1
        results.setdefault("reload", {}).setdefault(key_, 0)
        results["reload"][key_] += 1
        if (want_r, got_s, got_log) != (r_ & 0xFFFFFFFF, bytes(ps), log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH reload slot {slot:#x} gun {gun:#x}: result {want_r} / {r_}, record "
                      f"{[f'{o:#x}: {s[o]:#x} -> {got_s[o]:#x} / {ps[o]:#x}' for o in range(0x200) if got_s[o] != ps[o]]}\n  calls original {got_log}\n  calls port     {log}")
        e.uc.mem_write(sp, bytes(s))
    for (a_, k_), b_ in zip(((0x284F11, 1), (0x92E36, 1), (0x1163BD, 1), (G_SOLDIERS, 0x200 * 0x30)), tsave):
        e.uc.mem_write(rt(a_), b_)
    e.stubs.update({rt(a_): f_ for a_, f_ in lifted_.items()})
if direct("leave_sector"):    # (end of file: own random source) LeaveSectorCleanup? 0x30D6C (AL flag), from the leave path
    lifted_ = {a_: e.stubs.pop(rt(a_)) for a_ in (0x30D6C, 0x50EA8, 0x41178, 0x41E1C, 0x41574) if rt(a_) in e.stubs}
    if PORT:                       # RecalcTerritory 0x50EA8 runs for real (F22 calls it after SectorGained?): the port
        port_lib.LEDGER = True     # world carries the accounts; both sides' totals + sector table are compared
    CALLEES.setdefault("sector_gained", (0x3FA54, "i"))
    added_ = [c_ for c_ in ("sector_gained",) if rt(CALLEES[c_][0]) not in e.stubs]
    for c_ in added_:
        e.stub(rt(CALLEES[c_][0]), make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))
    dr = random.Random(1000003 * 77 + 3)
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in ((G_SOLDIERS, 0x200 * 0x30), (ai.G_SECTOR_TBL9, 9 * 60),
                                                             (ai.G_CUR_SECTOR, 1), (0x229FA0, 0x98))]
    w_ = World(0)
    w_.memrun = True
    for i in range(n // 6):
        for k_ in range(0x30):
            a_ = rt(G_SOLDIERS) + 0x200 * k_
            e.w8(a_ + 0x17E, dr.choice([0, 0, 8, 8, 1, 9]))
            e.w8(a_ + 0x182, dr.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x138, dr.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x188, dr.choice([0, 0, 0, 1]))
            e.w8(a_ + 0x183, dr.choice([0, 0, 1, 14, 15, 16, 60, 0x80, 0xFF] if dr.random() < 0.3 else [15, 40, 60]))
        weak_ = dr.choice([0, 0, 1, 2, 3])    # 1: no guard up (8..0x27), 2: no enemy up (0x28..0x2F), 3: neither side up
        for k_ in range(8, 0x30):
            if weak_ & (1 if k_ < 0x28 else 2):
                a_ = rt(G_SOLDIERS) + 0x200 * k_
                if dr.random() < 0.2:
                    e.w8(a_ + 0x182, 0)
                elif e.uc.mem_read(a_ + 0x183, 1)[0] in range(15, 0x80):
                    e.w8(a_ + 0x183, dr.choice([0, 1, 5, 14, 0x80, 0xFF]))
        for g_, (lo_, hi_) in ((1, (8, 0x28)), (2, (0x28, 0x30))):
            if weak_ & g_ and dr.random() < 0.5:    # one lone soldier up at the edge of the range (hp 15 / 16)
                a_ = rt(G_SOLDIERS) + 0x200 * dr.choice([lo_, hi_ - 1, dr.randrange(lo_, hi_)])
                e.w8(a_ + 0x182, 1)
                e.w8(a_ + 0x183, dr.choice([15, 16]))
        cur_ = dr.choice([dr.randrange(60), dr.randrange(60), 0, 59])
        e.w8(rt(ai.G_CUR_SECTOR), cur_)
        row_ = rt(ai.G_SECTOR_TBL9) + 9 * cur_
        for b_ in range(5):
            e.w8(row_ + b_, dr.choice([0, 0, 1, 2, 0x80, 0xFF] if b_ else [0, 1, 2, 3, 0xFE, 0xFF]))
        flag_ = dr.choice([0, 1, 1, 0x100, 5])
        pre_ = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x200 * 0x30))
        pre_tbl_ = bytes(e.uc.mem_read(rt(ai.G_SECTOR_TBL9), 9 * 60))
        pre_acc_ = bytes(e.uc.mem_read(rt(0x229FA0), 0x98))
        script = collections.defaultdict(list)
        log.clear()
        e.call(rt(0x30D6C), flag_)
        got_s, got_log = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x200 * 0x30)), list(log)
        got_t = bytes(e.uc.mem_read(rt(ai.G_SECTOR_TBL9), 9 * 60)) + bytes(e.uc.mem_read(rt(0x229FA0), 0x98))
        e.uc.mem_write(rt(G_SOLDIERS), pre_)
        e.uc.mem_write(rt(ai.G_SECTOR_TBL9), pre_tbl_)
        e.uc.mem_write(rt(0x229FA0), pre_acc_)
        script = collections.defaultdict(list)
        log.clear()
        if PORT:
            port_world("leave_sector", (flag_,))
        else:
            ai.leave_sector(w_, flag_)
        my_s = bytes(e.uc.mem_read(rt(G_SOLDIERS), 0x200 * 0x30))
        my_t = bytes(e.uc.mem_read(rt(ai.G_SECTOR_TBL9), 9 * 60)) + bytes(e.uc.mem_read(rt(0x229FA0), 0x98))
        if got_t != my_t:          # sector table + territory totals (RecalcTerritory under F22)
            got_s, my_s = got_s + got_t, my_s + my_t
        key_ = (weak_, len([c_ for c_ in log if c_[0] in ("merc_dies", "w_3a8c0")]) > 0, log[-2][0] if len(log) > 1 else "-")
        counts["leave_sector"] = counts.get("leave_sector", 0) + 1
        results.setdefault("leave_sector", {}).setdefault(key_, 0)
        results["leave_sector"][key_] += 1
        if (got_s, got_log) != (my_s, log):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH leave_sector case {i} flag {flag_:#x} sector {cur_}: soldiers "
                      f"{[hex(o) for o in range(len(my_s)) if got_s[o] != my_s[o]][:8]}\n  calls original {got_log}\n  calls mirror   {log}")
    for (a_, k_), b_ in zip(((G_SOLDIERS, 0x200 * 0x30), (ai.G_SECTOR_TBL9, 9 * 60), (ai.G_CUR_SECTOR, 1),
                             (0x229FA0, 0x98)), tsave):
        e.uc.mem_write(rt(a_), b_)
    if PORT:
        port_lib.LEDGER = False
    for c_ in added_:
        e.stubs.pop(rt(CALLEES[c_][0]), None)
    e.stubs.update({rt(a_): f_ for a_, f_ in lifted_.items()})
if direct("leave_path"):    # TacticalLoop 0x11358..0x1140F (the leave counter != 0): fade, end of day, cleanup; code range
    dr = random.Random(1000003 * 78 + 5)
    gl_ = [(0x284F13, 1), (0x96360, 2), (0x92E74, 1), (0x93449, 1), (ai.G_SANTINO_SPOKE, 1), (ai.G_CUR_SECTOR, 1),
           (ai.G_SECTOR_TBL9, 9 * 60)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in gl_]
    frame_ = e.alloc(16)
    tramp_ = e.alloc(16)
    e.uc.mem_write(tramp_, b"\xBD" + struct.pack("<I", frame_ + 8) + b"\xE9" + struct.pack("<i", rt(0x11358) - (tramp_ + 10)))
    e.stub(rt(0x1140F), lambda emu_, *x_: (emu_.uc.emu_stop(), 0)[1], 0)
    w_ = World(0)
    w_.memrun = True
    for i in range(n // 24):
        e.w8(rt(0x284F13), dr.choice([0, 1]))
        e.w16(rt(0x96360), dr.choice([0, 3, 0xFFFF]))
        e.w8(rt(0x92E74), dr.choice([0, 0, 1, 0x80]))
        e.w8(rt(0x93449), dr.choice([0, 0, 1, 4]))
        e.w8(rt(0x9344A), dr.choice([0, 1]))     # the byte after leave_reason (catches a wrong leave_reason address)
        e.w8(rt(ai.G_SANTINO_SPOKE), dr.choice([0, 1, 0x80]))
        cur_ = dr.choice([dr.randrange(60), 59, 0, 0x7F, 0x80, 0xFF] if not PORT else [dr.randrange(60), 59, 0])
        e.w8(rt(ai.G_CUR_SECTOR), cur_)
        if cur_ < 60:
            e.w8(rt(ai.G_SECTOR_TBL9) + 9 * cur_ + 2, dr.choice([0, 0, 1, 4, 0x80, 0xFF]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in gl_]
        script = collections.defaultdict(list)
        log.clear()
        e.call(tramp_)
        got = ([bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in gl_], list(log))
        for (a_, k_), b_ in zip(gl_, snap):
            e.uc.mem_write(rt(a_), b_)
        script = collections.defaultdict(list)
        log.clear()
        if PORT:
            port_world("leave_path")
        else:
            ai.leave_path(w_)
        want = ([bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in gl_], list(log))
        key_ = any(c_[0] == "end_of_day" for c_ in log)
        counts["leave_path"] = counts.get("leave_path", 0) + 1
        results.setdefault("leave_path", {}).setdefault(key_, 0)
        results["leave_path"][key_] += 1
        if got != want:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH leave_path case {i} sector {cur_}: globals "
                      f"{[hex(gl_[j][0]) for j in range(len(gl_)) if got[0][j] != want[0][j]]}\n  calls original {got[1]}\n  calls mirror   {want[1]}")
    e.stubs.pop(rt(0x1140F), None)
    for (a_, k_), b_ in zip(gl_, tsave):
        e.uc.mem_write(rt(a_), b_)
if direct("loss_msg"):   # TacticalLoop 0x107BC..0x108D8 (the day setup and every pass's stat-loss message):
    # code range with the setup callees and the message callees stubbed (entry 0x107BC runs for the coverage gate)
    CALLEES["w_61688"] = (0x61688, "iii")        # TeamQuote? (the stage-0 home warning), not in the default list
    saved_ = {rt(0x27B98): e.stubs.get(rt(0x27B98)), rt(0x75668): e.stubs.get(rt(0x75668)),
              rt(0x61688): e.stubs.get(rt(0x61688)),
              rt(0x15910): e.stubs.pop(rt(0x15910), None),    # MsgMercText runs for real: its ShowMessage is logged
              rt(0x2AD20): e.stubs.get(rt(0x2AD20)), rt(0x47020): e.stubs.get(rt(0x47020)),
              rt(0x5F2B0): e.stubs.get(rt(0x5F2B0))}
    e.stub(rt(0x27B98), make_stub("show_msg", CALLEES["show_msg"][1]), POP.get("show_msg", 0))
    e.stub_sprintf()
    e.stub(rt(0x61688), make_stub("w_61688", "iii"), 0)

    from unicorn import UC_HOOK_CODE
    from unicorn.x86_const import UC_X86_REG_EIP

    def lm_end_(uc_, addr_, size_, ud_):     # the loop head follows the block: a hook (a stub would pop the stack)
        uc_.reg_write(UC_X86_REG_EIP, emu.RET_SENTINEL)
        uc_.emu_stop()
    lm_hook_ = e.uc.hook_add(UC_HOOK_CODE, lm_end_, begin=rt(0x108D8), end=rt(0x108D8))
    e.stub(rt(0x2AD20), lambda emu_, *a_: 0, 0)     # LoadTacticalSector?, TimerStart x2, 0x5F2B0: not compared
    e.stub(rt(0x47020), lambda emu_, *a_: 0, 0)
    e.stub(rt(0x5F2B0), lambda emu_, *a_: 0, 0)
    lm_ = random.Random(4715)
    REG = [(0x92DCB, 1), (0x92EFC, 4), (ai.G_LOSS_TIMER, 4)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    names_ = [b"Nick\0", b"Hitman\0", b"Grunty\0", b"", b"Buns\0", b"A\0"]
    parts_ = [b"head\0", b"arm\0", b"leg\0", b"leg\0", b"\0", b"elbow\0"]
    stats_ = [b"wisdom\0", b"dexterity\0", b"agility\0", b"agility\0", b"\0", b"luck\0"]
    frame_ = e.alloc(16)
    tramp_ = e.alloc(16)
    e.uc.mem_write(tramp_, b"\xBD" + struct.pack("<I", frame_ + 8) + b"\xE9" + struct.pack("<i", rt(0x107BC) - (tramp_ + 10)))
    w_ = World(0)
    w_.memrun = True
    for i in range(max(200, n // 40)):
        amount = lm_.choice([0, 0, 1, 2, 5, 0x7F, 0x80, 0xFF, 0xFF, 0xFE, lm_.randrange(256)])
        slot = lm_.choice([lm_.randrange(8)] if amount == 0xFF else
                          [lm_.randrange(8), lm_.randrange(0x28, 0x40), 0x3F, 0x40, 0xFF])
        e.w32(rt(ai.G_LOSS_TIMER), lm_.choice([0, 0, 0, 500, 5, 0xFFFFFFF6]))
        e.w8(rt(ai.G_LOSS_SLOT), slot)
        e.w8(rt(ai.G_LOSS_AMOUNT), amount & 0xFF)
        e.w32(rt(0x92EFC), lm_.choice([0, 1, 0xFFFFFFFF]))
        if slot < 0x30:      # a name and the merc slot byte of the soldier record
            nm_ = lm_.choice(names_)
            e.uc.mem_write(rt(G_SOLDIERS) + 0x200 * (slot & 0x3F) + 0x16E, nm_ + b"\0" * (10 - len(nm_)))
            e.w8(rt(G_SOLDIERS) + 0x200 * (slot & 0x3F) + 0x17A, lm_.choice([slot & 0x3F, slot & 0x3F, 0, 7, 8, 0x40]))
        e.uc.mem_write(rt(ai.G_LOSS_PART), lm_.choice(parts_) + b"\0" * 6)
        e.uc.mem_write(rt(ai.G_LOSS_STAT), lm_.choice(stats_) + b"\0" * 6)
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"show_msg": [0] * 3, "w_61688": [0] * 3}
        script.clear()
        for k_, v_ in base_.items():
            script[k_] = list(v_)
        log.clear()
        e.call(tramp_)
        got_log = list(log)
        got_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        for k_, v_ in base_.items():
            script[k_] = list(v_)
        log.clear()
        ai.loss_message(w_)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["loss_msg"] = counts.get("loss_msg", 0) + 1
        key_ = tuple(sorted({c_[0] for c_ in log}))
        results.setdefault("loss_msg", {}).setdefault(key_, 0)
        results["loss_msg"][key_] += 1
        if got_log != log or got_g != want_g:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH loss_msg case {i} amount {amount & 0xFF} slot {slot}: memories {got_g != want_g}"
                      f"\n  calls original {got_log}\n  calls mirror   {log}")
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
    for a_, s_ in saved_.items():
        if s_ is None:
            e.stubs.pop(a_, None)
        else:
            e.stubs[a_] = s_
    e.uc.hook_del(lm_hook_)
    del CALLEES["w_61688"]
def eod_case(log_):
    """DOS file names: ArcSeek 0x534A4 upper-cases the name literal 0x90660 in place, so a dump taken after an
    earlier day end (day3) passes "SUNSET.PCX" where tac60 passes "sunset.pcx". Compare the names without case."""
    return [(c_[0], tuple(a_.lower() if isinstance(a_, str) else a_ for a_ in c_[1])) + tuple(c_[2:])
            if c_[0] in ("load_pcc", "load_pal") else c_ for c_ in log_]


if direct("end_of_day"):    # EndOfDayScreen 0x29F44 with the retreat 0x30B04 and its hit 0x3928C run for real
    from unicorn import UC_HOOK_CODE
    lifted_ = {a_: e.stubs.pop(rt(a_)) for a_ in (0x29F44,) if rt(a_) in e.stubs}
    new_ = {"load_main_sound": (0x31B8C, ""), "load_pal": (0x53300, "ixmx"), "pal_copy": (0x52820, "xxi"),
            "w_46bf4": (0x46BF4, ""), "pal_cycle": (0x528AC, "xii"), "pal_set": (0x52870, "xii"),
            "eod_text": (0x358D0, "mxiiiiiiii"), "w_6782c": (0x6782C, ""), "key_any": (0x67E44, ""),
            "w_5ce60": (0x5CE60, "i"), "w_5ce08": (0x5CE08, ""), "overhead_snd": (0x31B44, "i"),
            "merc_text2": (0x27B38, "sm"), "eod_wait": (0x47040, "x"), "wait": (0, "i")}
    for c_, v_ in new_.items():
        CALLEES.setdefault(c_, v_)
    POP.setdefault("eod_text", 24)
    saved_ = {}
    for c_ in [c_ for c_ in new_ if c_ != "wait"] + ["load_pcc", "sfx", "connected_home", "fade", "hit_location",
                                                    "stat_practice", "w_17b70", "w_3a8c0", "w_67bf0"]:
        a_ = rt(CALLEES[c_][0])
        saved_[a_] = e.stubs.get(a_)
        if c_ == "eod_wait":
            def eod_wait_(emu_, *x_):
                log.append(("eod_wait", ()))
                return 2
            e.stub(a_, eod_wait_, 0)
        else:
            e.stub(a_, make_stub(c_, CALLEES[c_][1]), POP.get(c_, 0))
    saved_[rt(0x47020)] = e.stubs.get(rt(0x47020))
    e.stub(rt(0x47020), lambda emu_, *x_: 0, 0)       # TimerStart: not compared

    def wait_hook_(uc_, addr_, size_, ud_):
        log.append(("wait", (ai._i32(e.r32(rt(0x92EF8))),)))
        e.w32(rt(0x92EF8), 0)
    hooks_ = [e.uc.hook_add(UC_HOOK_CODE, wait_hook_, begin=rt(a_), end=rt(a_)) for a_ in (0x30CA4, 0x30CDE, 0x30D18, 0x30D41)]
    dr = random.Random(1000003 * 79 + 7)
    gl_ = [(0x92DCC, 1), (0x22A484, 4), (0x92E4F, 1), (0x229D18, 4), (0x92EF8, 4), (ai.G_DAY, 4), (0x93449, 1),
           (ai.G_SANTINO_SPOKE, 1), (0x22A04E, 1), (0x22A05C, 1), (ai.G_CUR_SECTOR, 1), (ai.G_SECTOR_TBL9, 9 * 60),
           (G_SOLDIERS, 0x200 * 8)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in gl_]
    w_ = World(0)
    w_.memrun = True
    for i in range(max(n // 60, 4)):
        e.w32(rt(ai.G_DAY), dr.choice([1, 2, 9, 10, 99, 0x30005, 0xFFFFFFFF]))
        e.w8(rt(0x93449), dr.choice([0, 0, 0, 1, 4]))
        e.w8(rt(ai.G_SANTINO_SPOKE), dr.choice([0, 0, 0, 1]))
        e.w8(rt(0x22A05C), dr.choice([0, 0, 1]))
        e.w8(rt(0x22A04E), dr.choice([0, 1, 2, 2]))
        e.w32(rt(0x92EF8), dr.choice([0, 5, 0xFFFFFFF0]))
        cur_ = dr.choice([dr.randrange(60), 0, 9, 50, 59, 49, 10, 19])
        e.w8(rt(ai.G_CUR_SECTOR), cur_)
        e.w8(rt(ai.G_SECTOR_TBL9) + 9 * cur_, dr.choice([0, 1, dr.randrange(0x20), dr.randrange(256), 0x1F, 0x1E,
                                                                    0x1D, 0x1B, 0x17, 0x0F]))   # owned, one side open
        for k_ in range(8):
            a_ = rt(G_SOLDIERS) + 0x200 * k_
            e.w8(a_ + 0x182, dr.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x138, dr.choice([1, 1, 1, 0]))
            e.w8(a_ + 0x188, dr.choice([0, 0, 0, 1]))
            e.w8(a_ + 0x183, dr.choice([0, 1, 14, 15, 16, 20, 31, 60, 100, 0x80, 0xFF]))
            e.w8(a_ + 0x17E, dr.choice([0, 7, 8, 9, 0xFF]))
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in gl_]
        keys_ = [0] * 119
        for _ in range(dr.choice([0, 0, 1, 2])):
            keys_[dr.choice([0, 1, 29, 30, 31, dr.randrange(119)])] = 1
        sc_ = {"key_any": keys_, "sfx": [dr.randrange(1 << 16) for _ in range(40)],
               "connected_home": [dr.choice([0, 0, 1, 0x100]) for _ in range(5)],
               "Random": [dr.choice([0, 0, 5, 10, 30, 31, 60, 61, 99, dr.randrange(1000)]) for _ in range(300)],
               "hit_location": [dr.choice([0, 1, 5, 14, 20, 30, 61, 200, 0xFFFFFFFF]) for _ in range(8)],
               "load_pcc": [dr.randrange(1 << 32)], "load_pal": [dr.randrange(1 << 32)]}
        script = collections.defaultdict(list, {k_: list(v_) for k_, v_ in sc_.items()})
        log.clear()
        try:
            e.call(rt(0x29F44))
            how_ = "returned"
        except Exception as x_:
            how_ = f"stop {x_}"
        got = ([bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in gl_], eod_case(log), how_)
        for (a_, k_), b_ in zip(gl_, snap):
            e.uc.mem_write(rt(a_), b_)
        script = collections.defaultdict(list, {k_: list(v_) for k_, v_ in sc_.items()})
        log.clear()
        if PORT:
            port_world("end_of_day", sync=True)
        else:
            ai.end_of_day(w_)
        want = ([bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in gl_], eod_case(log), "returned")
        key_ = ("stranded" if any(c_[0] == "fade" and c_[1][1] == 0x38 for c_ in log) else "home",
                min(sum(c_[0] == "merc_text2" for c_ in log), 3))
        counts["end_of_day"] = counts.get("end_of_day", 0) + 1
        results.setdefault("end_of_day", {}).setdefault(key_, 0)
        results["end_of_day"][key_] += 1
        if got != want:
            fails += 1
            if fails <= 10:
                k_ = next((j for j, (a_, b_) in enumerate(zip(got[1], want[1])) if a_ != b_), min(len(got[1]), len(want[1])))
                print(f"MISMATCH end_of_day case {i} sector {cur_}: {got[2]}; globals "
                      f"{[hex(gl_[j][0]) for j in range(len(gl_)) if got[0][j] != want[0][j]]}; call {k_}: original "
                      f"{got[1][k_:k_ + 3]}, mirror {want[1][k_:k_ + 3]}")
    for h_ in hooks_:
        e.uc.hook_del(h_)
    for a_, f_ in saved_.items():
        if f_ is None:
            e.stubs.pop(a_, None)
        else:
            e.stubs[a_] = f_
    for (a_, k_), b_ in zip(gl_, tsave):
        e.uc.mem_write(rt(a_), b_)
    e.stubs.update({rt(a_): f_ for a_, f_ in lifted_.items()})
if direct("explode_dat"):    # (end of file: own random source) LoadExplodeDat 0x33BD4 (host op 336 explode_art): crater from
    # DAT\EXPLODE.DAT, ground items 0x5F / 0x6E, the sector-0 story kill; 0x33820 / 0x33B60 run real, callees stubbed
    dr = random.Random(1000003 * 336 + 11)
    lifted_ = {a_: e.stubs.pop(rt(a_)) for a_ in (0x33BD4, 0x33820, 0x33B60) if rt(a_) in e.stubs}
    cdsave_ = CD.install()
    LOCAL_ = {"forget_soldier": (0x3A148, "s"), "w_2be54": (0x2BE54, "i")}
    STUBBED_ = ("forget_soldier", "w_2be54", "merc_dies", "stage_five", "w_3c05c")
    if PORT:          # the port runs Forget soldier and the tile's move code itself: real in the original
        STUBBED_ = STUBBED_[2:]
    CALLEES.update(LOCAL_)
    held_ = {CALLEES[k_][0]: e.stubs.get(rt(CALLEES[k_][0])) for k_ in STUBBED_}
    for k_ in STUBBED_:
        e.stub(rt(CALLEES[k_][0]), make_stub(k_, CALLEES[k_][1]))
    oc_old_ = CALLEES["open_cd"]
    CALLEES["open_cd"] = (0x38304, "mx")      # (EDX is a leftover register: not an argument)
    oc_inner_ = make_stub("open_cd", "mx")

    def oc_stub_(emu_, eax, edx, ebx, ecx):
        v_ = oc_inner_(emu_, eax, edx, ebx, ecx)
        return CD.after("open_cd", (bytes(emu_.uc.mem_read(eax, 80)).split(b"\0")[0].decode("latin-1"), edx), v_)
    e.stub(rt(0x38304), oc_stub_)
    CD.exists("DAT/EXPLODE.DAT")
    real_dat_ = CD.cache["DAT/EXPLODE.DAT"]
    assert e.r32(rt(0x1009F8)) == rt(G_SOLDIERS) + 0x200 * 0x28, "g_soldiersPtr is not slot 0x28"
    TOP_ = G_TILEMAP + 8 * (3136 + 256)
    REG = [(G_SOLDIERS, TOP_ - G_SOLDIERS), (ai.G_GROUND_ITEMS, 12 * 256), (ai.G_SECTOR_TBL9, 9 * 256),
           (ai.G_MOVECODE, 2 * 3136), (0x22A039, 1), (0x22A0C2, 1), (0x22A0CC, 1), (0x22A05E, 1)]
    tsave = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
    OBJ_ = ([0, 1, 5, 6, 0xA, 0xB, 0xC, 0xD, 0xE, 0xF, 0x10, 0x1A, 0x1B, 0x1D, 0x1E, 0x20, 0x22, 0x25, 0x27, 0x36]
            + list(range(0x37, 0x3F)) + [0x41, 0x42, 0x43, 0x44, 0x45, 0x47, 0x49, 0x4A, 0x4B, 0x4E, 0x50, 0x52, 0x53]
            + list(range(0x55, 0x5E)) + [0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x6D, 0x71, 0x72]
            + list(range(0x75, 0x84)) + [0xFF])
    BIG_C_ = [0x56, 0x66, 0x37, 0x57, 0x67, 0x38, 0x52, 0x4B, 0x39, 0x4A, 0x58, 0x72, 0x49]
    BIG_O_ = ([0x67, 0x57, 0x71, 0x5B, 0x6D, 0x61, 0x58, 0x39, 0x38, 0x52, 0x3C, 0x65, 0x37, 0x56, 0x72, 0x5D, 0x5A,
               0x3D, 0x3E, 0x3B, 0x64, 0x62, 0x41, 0x43, 0x45, 0x53, 0x66, 0x47, 0x1B, 0x1E, 0x20] + list(range(0x75, 0x7F)))
    BIG_S_ = [0, 0, 6, 6, 0xD, 0x39, 0x3A, 0x3C, 0x42, 0x44, 0x4E, 0x50, 0x53, 0x58, 0x59, 0x5B, 0x63, 0x65, 0x67, 0x6D,
              0x71, 0x72]
    A_ = list(range(0x75, 0x7A))
    RULES_ = [((0x56, 0x66), 2, (0x67,), ()), ((0x56, 0x66), 5, [0x57, 0x71, 0x5B] + A_, ()),
              ((0x56, 0x66), 7, [0x6D, 0x61, 0x58] + list(range(0x7A, 0x7F)), ()), ((0x37,), 7, (0x39,), ()),
              ((0x37,), 5, (0x38, 0x52, 0x3C), ()), ((0x57,), 6, (0x67,), ()), ((0x67,), 7, (0x65,), ((54, (6,)),)),
              ((0x67,), 6, (0x57,), ((56, (0x65, 0, 6)),)), ((0x38, 0x52), 3, (0x37,), ((-2, (0,)),)),
              ((0x4B,), 3, (0x37,), ()), (None, 1, (0x56,), ((-112, (0x4E, 0x50)),)), (None, 3, (0x56,), ((-2, (0,)),)),
              ((0x56,), 5, (0x71,), ()), ((0x39,), 3, (0x71, 0x57), ((56, (0x53, 0)),)), ((0x39,), 6, (0x57,), ()),
              ((0x4A,), 5, (0x38, 0x52, 0x10), ()), (None, 3, (0x5D,), ((-2, (0x5B,)),)), (None, 3, (0x5A,), ((-2, (0x59,)),)),
              (None, 3, (0x3D, 0x3E), ((-2, (0x3C,)),)), (None, 3, (0x3B,), ((-2, (0x3A,)),)),
              (None, 1, (0x64, 0x62), ((-112, (0x63,)),)), (None, 1, (0x41,), ((-112, (0x42,)),)),
              (None, 1, (0x43, 0x45), ((-112, (0x44,)),)), (None, 0, (0x57,), ((-112, (0x39,)),)),
              ((0x58, 0x72, 0x39), None, (), ((-112, (0x58, 0x6D)), (-56, (6,)))), ((0x58,), None, (), ((-112, (0xD,)), (-56, (6,)))),
              ((0x58, 0x72), None, (), ((-112, (0x4E,)),)), ((0x49,), 7, (0x39, 0x53), ()),
              (None, 1, (0x66, 0x47), ((-112, (0,)),))]
    CACHE_ = [ai.s16(e.r16(rt(ai.G_EXPLODE_CACHE) + 7 * k_)) for k_ in range(12)]
    for i in range(n):
        for (a_, k_), b_ in zip(REG, tsave):
            e.uc.mem_write(rt(a_), b_)
        tile = dr.choice([dr.randrange(3136), dr.randrange(114, 3136 - 114), dr.randrange(114, 3136 - 114),
                          dr.choice([0, 1, 2, 55, 56, 57, 112, 113, 3022, 3079, 3135]), 0x15B,
                          dr.choice([0x121, 0x122, 0x123, 0x124, 0x125]), dr.choice(CACHE_)])
        if PORT and not 0xB0 <= tile < 3136 - 0xB0:   # the port drops tile bytes off the map (SPEC GAP): every
            tile = dr.randrange(0xB0, 3136 - 0xB0)       # tile the steps touch stays on the map
        big = dr.choice([0, 1, 1, 2, 0x100])
        e.w8(rt(0x22A039), dr.choice([0, 0, dr.randrange(1, 60)]))
        e.w8(rt(0x22A0C2), dr.choice([0, 1, 2, 3, 0x80, 0xFF]))
        e.w8(rt(0x22A0CC), dr.choice([0, 1, 2, 3, 0xFF]))
        e.w8(rt(0x22A05E), dr.choice([0, 1]))
        tb_ = rt(G_TILEMAP)
        near_ = [tile + d_ for d_ in ai.EXPLODE_OFS] + [tile + d_ - 56 for d_ in ai.EXPLODE_OFS]
        near_ += [x_ + d2_ for x_ in list(near_) for d2_ in (-2, -1, 1, 56, -56)]
        near_ += [tile + d_ for d_ in (-114, -113, -112, -111, -58, -57, -56, -2, -1, 1, 2, 56, 57, 113)]
        for x_ in set(near_):
            if not -128 <= x_ < 3136 + 128:
                continue
            a_ = tb_ + 8 * x_
            if dr.random() < 0.6:
                e.w8(a_ + 1, dr.choice(OBJ_ + [dr.randrange(256)]))
            if dr.random() < 0.4:
                e.w8(a_ + 2, dr.choice([0, 0x47, 0x48, 0x4A, 0x4A, 0x50, 0x51, dr.randrange(256)]))
            if dr.random() < 0.4:
                e.w8(a_ + 4, dr.choice([0, 0, 0x80, 1, dr.randrange(256)]))
        e.w8(tb_ + 8 * tile + 1, dr.choice(OBJ_ + [dr.randrange(256)]))
        e.w8(tb_ + 8 * tile, dr.choice([dr.randrange(256), dr.randrange(0x97, 0x100), e.r8(tb_ + 8 * tile)]))
        if dr.random() < 0.5:          # the big-blast object fixes: a centre object and cell objects they test
            for x_, l_ in ([(tile + d_, BIG_O_) for d_ in ai.EXPLODE_OFS]
                           + [(tile + d_, BIG_S_) for d_ in (-114, -113, -112, -111, -58, -57, -2, 2, 113)]
                           + [(tile + d_ + d2_, BIG_S_) for d_ in ai.EXPLODE_OFS for d2_ in (-56, -1, 1, 56)]):
                if dr.random() < 0.5:
                    e.w8(tb_ + 8 * x_ + 1, dr.choice(l_))
            e.w8(tb_ + 8 * tile + 1, dr.choice(BIG_C_))
            for _ in range(dr.choice([1, 1, 2])):      # one or two rules with their tests made true
                oc_, k_, os_, side_ = dr.choice(RULES_)
                for d_, vs_ in side_:
                    e.w8(tb_ + 8 * (tile + d_) + 1, dr.choice(vs_))
                if os_:
                    e.w8(tb_ + 8 * (tile + ai.EXPLODE_OFS[k_]) + 1, dr.choice(os_))
                if oc_:
                    e.w8(tb_ + 8 * tile + 1, dr.choice(oc_))
                if k_ == 3 and dr.random() < 0.5:
                    e.w8(tb_ + 8 * (tile - 58) + 2, 0x4A)
        g_ = dr.choice([0xFF, 0xC8, 0xE0, dr.randrange(200), dr.randrange(200)])
        e.w8(tb_ + 8 * tile + 6, g_)
        if g_ < 200:
            e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_, dr.choice([0x5F, 0x6E, 0x3F, 0x3F, 0x5E, 0x60, 0x7B, dr.randrange(256)]))
        if tile == 0x15B and dr.random() < 0.5:     # the story kill: item 0x3F on tile 0x15B of sector 0 after Santino
            g_ = dr.randrange(200)
            e.w8(tb_ + 8 * tile + 6, g_)
            e.w8(rt(ai.G_GROUND_ITEMS) + 12 * g_, 0x3F)
            e.w8(rt(0x22A039), 0)
            e.w8(rt(0x22A0C2), dr.choice([1, 2, 3, 0xFF]))
        e.w16(rt(ai.G_MOVECODE) + 2 * tile, dr.choice([0x62D4, 0x6338, dr.randrange(0x10000)]))
        if dr.random() < 0.5:          # fuzzed words in the file (0xFF stops, 0xFFFF, the 1000+ pieces)
            dat_ = bytearray(real_dat_)
            for _ in range(dr.choice([20, 200, 2000])):
                p_ = 2 * dr.randrange(len(dat_) // 2)
                dat_[p_:p_ + 2] = dr.choice([0xFF, 0xFFFF, 0xFE, dr.randrange(1000, 1020), dr.randrange(1000, 1020),
                                             dr.randrange(256), dr.randrange(0x10000)]).to_bytes(2, "little")
            for k_, d_ in enumerate(ai.EXPLODE_OFS):     # the words this case reads: the 1000+ pieces
                p_ = 4 * k_ + 0x24 * e.r8(tb_ + 8 * (tile + d_) + 1)
                if dr.random() < 0.5:
                    w_ = dr.choice([(dr.randrange(1000, 1020), 0), (dr.randrange(0xFF), dr.randrange(1000, 1020)),
                                    (dr.randrange(0xFF), 0xFF), (0xFF, 0), (0xFFFF, dr.randrange(0x10000))])
                    dat_[p_:p_ + 4] = w_[0].to_bytes(2, "little") + w_[1].to_bytes(2, "little")
                if dr.random() < 0.5:     # the neighbours the pieces test
                    for d2_ in (-1, -56, 56, -2):
                        e.w8(tb_ + 8 * (tile + d_ + d2_) + 1, dr.choice([0x57, 0xE, 0x5C, 0x5D, 0xC, 0x56, 0x58, 0x37, 0x39,
                                                                         0x7F, 0, 0x6D, 6, 0x67, 0x72] + A_))
            CD.cache["dat/explode.dat"] = bytes(dat_)
        else:
            CD.cache["dat/explode.dat"] = real_dat_
        CdFiles.names.add("dat/explode.dat")
        snap = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        base_ = {"Random": [dr.randrange(0x7FFF) for _ in range(4)]}
        script = collections.defaultdict(list, {k_: list(v_) for k_, v_ in base_.items()})
        log.clear()
        e.call(rt(0x33BD4), tile, big)
        got_g, got_log = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG], list(log)
        for (a_, k_), b_ in zip(REG, snap):
            e.uc.mem_write(rt(a_), b_)
        script = collections.defaultdict(list, {k_: list(v_) for k_, v_ in base_.items()})
        log.clear()
        if PORT:
            port_sector("explode_dat", (tile, big), allow=("merc_dies", "stage_five", "w_3c05c"))
        else:
            ai.explode_dat(CdWorld(0), tile, big)
        want_g = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in REG]
        counts["explode_dat"] = counts.get("explode_dat", 0) + 1
        keys_ = []
        if not any(c_[0] == "open_cd" for c_ in got_log):
            keys_.append("kept")
        else:
            gi_ = (ai.G_GROUND_ITEMS - REG[1][0]) + 12 * g_ if g_ < 200 else None
            if gi_ is not None and got_g[1][gi_] != snap[1][gi_]:
                keys_.append(f"item {got_g[1][gi_]:#x}")
            if any(c_[0] in ("forget_soldier", "merc_dies") for c_ in got_log):
                keys_.append("story kill")
            o0_ = snap[0][G_TILEMAP - G_SOLDIERS + 8 * tile + 1]
            keys_.append(("crater big" if big and ai.explode_class(o0_) != 9 else "crater") if o0_ >= 0x1B else "small")
            if got_g[0] != snap[0]:
                keys_.append("tiles changed")
            if any(c_[0] == "w_2be54" for c_ in got_log) or got_g[3] != snap[3]:
                keys_.append("move code")
        for k_ in keys_:
            results.setdefault("explode_dat", {}).setdefault(k_, 0)
            results["explode_dat"][k_] += 1
        if (got_g, got_log) != (want_g, log):
            fails += 1
            if fails <= 10:
                bad_ = [(hex(REG[j_][0]), [hex(REG[j_][0] + o_) for o_ in range(REG[j_][1]) if got_g[j_][o_] != want_g[j_][o_]][:8])
                        for j_ in range(len(REG)) if got_g[j_] != want_g[j_]]
                k_ = next((j for j, (a_, b_) in enumerate(zip(got_log, log)) if a_ != b_), min(len(got_log), len(log)))
                print(f"MISMATCH explode_dat case {i} tile {tile} big {big}: {bad_}; call {k_}: original "
                      f"{got_log[k_:k_ + 3]}, mirror {log[k_:k_ + 3]}")
    for (a_, k_), b_ in zip(REG, tsave):
        e.uc.mem_write(rt(a_), b_)
    CD.cache.pop("dat/explode.dat", None)
    CdFiles.names.discard("dat/explode.dat")
    CALLEES["open_cd"] = oc_old_
    CD.remove(cdsave_)
    for a_, f_ in held_.items():
        if f_ is None:
            e.stubs.pop(rt(a_), None)
        else:
            e.stubs[rt(a_)] = f_
    for k_ in LOCAL_:
        del CALLEES[k_]
    e.stubs.update({rt(a_): f_ for a_, f_ in lifted_.items()})
summary = ", ".join(f"{k} {counts[k]} (results {dict(sorted(results[k].items(), key=str)[:17])}{' ...' if len(results[k]) > 17 else ''})" for k in counts)
clock(None)
for k in counts:                     # machine-readable case counts: check.py CASE_FLOORS
    print(f"CASES {k}={counts[k]}")
if os.environ.get("AI_DIFF_TIMES"):
    print("TIMES " + " ".join(f"{k}={v:.1f}" for k, v in SPENT.most_common()))
print(f"ai_diff{f' --fan {FAN}' if FAN is not None else ''}: {summary}; {fails} mismatches" + (f" (original never returned, skipped: {hangs})" if hangs else ""))
sys.exit(1 if fails else 0)
