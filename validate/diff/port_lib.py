"""ctypes bridge to the clean-room port (port/ja1logic, Rust, C interface): the diff tests' `--port` mode.

The harness side may know the original; the port side is written from docs/spec only. Each wrapper takes the same
Python arguments as the analysis mirror it replaces in a diff test, so a test swaps one function for the other.
Build first: (cd port/ja1logic && ~/.cargo/bin/cargo build --release).
"""
import ctypes, os

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
LIB_PATH = os.environ.get("JA1_PORT_LIB") or os.path.join(ROOT, "port", "ja1logic", "target", "release",
                                                     "libja1logic.so")   # JA1_PORT_LIB: test a frozen copy
_lib = None


def lib():
    global _lib
    if _lib is None:
        if not os.path.exists(LIB_PATH):
            raise SystemExit(f"port library missing: {LIB_PATH} (build it with cargo build --release)")
        _lib = ctypes.CDLL(LIB_PATH)
        u8p, u16p = ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint16)
        _lib.ja1_move_code.argtypes = [ctypes.c_uint8] * 3 + [ctypes.c_uint16, u8p, u8p]
        _lib.ja1_move_code.restype = ctypes.c_uint16
        _lib.ja1_rebuild_move_codes.argtypes = [u8p, u8p, u8p, u16p]
        _lib.ja1_rebuild_move_codes.restype = None
        _lib.ja1_step_ap.argtypes = [ctypes.c_uint16] + [ctypes.c_uint8] * 3
        _lib.ja1_step_ap.restype = ctypes.c_int32
        _lib.ja1_find_path.argtypes = [ctypes.c_int32, ctypes.c_int32, u16p, u8p, u8p] + [ctypes.c_uint8] * 4 + [u8p, u16p]
        _lib.ja1_find_path.restype = ctypes.c_int32
    return _lib


def _u8(b):
    return (ctypes.c_uint8 * len(b)).from_buffer_copy(bytes(b))


def move_code(t0, t1, t3, old, t0class, t1class):
    return lib().ja1_move_code(t0 & 0xFF, t1 & 0xFF, t3 & 0xFF, old & 0xFFFF, _u8(t0class), _u8(t1class))


def step_ap_cost(code, shift, b177, alt):
    return lib().ja1_step_ap(code & 0xFFFF, shift & 0xFF, b177 & 0xFF, alt & 0xFF)


def rebuild_move_codes(tiles, t0class, t1class):
    """tiles = 3136 x 8 bytes -> list of 3136 move codes"""
    out = (ctypes.c_uint16 * 3136)()
    lib().ja1_rebuild_move_codes(_u8(tiles), _u8(t0class), _u8(t1class), out)
    return list(out)


def find_path(start, dest, mc, t0, occ, b177, bB9, slot, combat, fill_dirs=0xEE, fill_count=0x5555):
    """-> (found, 80 path bytes, count) with the output buffers pre-filled like the test fills the original's"""
    dirs = (ctypes.c_uint8 * 80)(*([fill_dirs] * 80))
    count = ctypes.c_uint16(fill_count)
    occ_b = bytes(occ(t) for t in range(3136)) if callable(occ) else bytes(occ)
    found = lib().ja1_find_path(start, dest, (ctypes.c_uint16 * 3136)(*mc), _u8(t0), _u8(occ_b), b177 & 0xFF,
                                bB9 & 0xFF, slot & 0xFF, combat & 0xFF, dirs, ctypes.byref(count))
    return found, bytes(dirs), count.value


# AI decision layer (docs/spec/port_api.md): world + host callbacks
u8p = ctypes.POINTER(ctypes.c_uint8)


class World(ctypes.Structure):
    _fields_ = [("soldiers", u8p), ("tiles", u8p), ("item_types", u8p), ("anim_table", u8p),
                ("sector_state", u8p), ("mission_done", u8p),
                ("cur_sector", ctypes.c_uint8), ("alarm", ctypes.c_uint8), ("combat", ctypes.c_uint8),
                ("difficulty", ctypes.c_uint8), ("mission_step", ctypes.c_int8), ("mission_slot", ctypes.c_uint8),
                ("mission_target", ctypes.c_uint16), ("mission_spot", ctypes.c_uint16),
                ("turn_based", ctypes.c_int32), ("busy", ctypes.c_int32), ("active_slot", ctypes.c_uint8),
                ("santino_spoke", ctypes.c_uint8), ("santino_timer", ctypes.c_int16), ("show_ai", ctypes.c_uint8),
                ("santino_word", ctypes.c_uint32),
                ("knowledge", u8p), ("fresher", u8p), ("last_seen_m", u8p), ("last_seen_e", u8p), ("calls", u8p),
                ("paths", u8p), ("dir_steps", u8p), ("t0_class", u8p), ("t1_class", u8p),
                ("last_spotter", ctypes.c_uint8), ("remember_m", u8p), ("remember_e", u8p),
                ("story", u8p), ("plants", u8p), ("crates", u8p), ("day", ctypes.c_int32),
                ("weapons", u8p), ("absorb", u8p), ("anim_rows", u8p),
                ("selected", ctypes.c_uint8), ("anim_scripts", u8p), ("anim_free", u8p), ("anim_sounds", u8p),
                ("path_preview", ctypes.c_int32), ("anim_steps_tile", u8p), ("anim_turn_while", u8p),
                ("opt_sound", ctypes.c_uint8), ("opt_onscreen", ctypes.c_uint8), ("view_x", ctypes.c_int32),
                ("view_y", ctypes.c_int32), ("view_w", ctypes.c_int32), ("view_h", ctypes.c_int32),
                ("anim_signal", ctypes.c_int32), ("effect_tile", ctypes.c_int16),
                ("supplies", u8p), ("equip_skill", u8p), ("attach", u8p), ("guard_skill", ctypes.c_uint8),
                ("ground_items", u8p), ("notes", u8p),
                ("move_codes", u8p), ("held", u8p), ("blast_list", u8p), ("listed", u8p),
                ("t2_class", u8p), ("enemy_watch", u8p),
                ("text_buf", u8p), ("item_buf", u8p), ("crate_state", u8p), ("crate_shown", u8p),
                ("fixed_items", u8p), ("sectors", u8p), ("bomb_count", ctypes.c_int32),
                ("scratch", u8p), ("team_on_duty", ctypes.c_int32), ("purifier", ctypes.c_uint8),
                ("safe_opened", ctypes.c_uint8),          # sector_start.md batch 11
                ("screen_drawn", ctypes.c_uint8), ("interrupt_go", ctypes.c_uint8), ("turn_moves", ctypes.c_uint8),
                ("kept_merc", ctypes.c_uint8), ("next_selected", ctypes.c_uint8), ("quote_slot", ctypes.c_uint8),
                ("quote_tile", ctypes.c_uint16), ("quote_focus", ctypes.c_uint8), ("quote_delay", ctypes.c_int8),
                ("quote_event", ctypes.c_uint8), ("opt_half_delay", ctypes.c_uint8), ("opt_follow", ctypes.c_uint8),
                ("opt_reload_msg", ctypes.c_uint8), ("alone_warned", ctypes.c_uint8), ("follow_all", ctypes.c_uint8),
                ("stop_timer", ctypes.c_int32), ("interrupted", ctypes.c_uint8), ("walk_ap", ctypes.c_int32),
                ("ap_warned", u8p), ("splash", u8p),       # soldier_update.md
                ("spotted", ctypes.c_int32), ("opt_step_text", ctypes.c_uint8),
                ("key_shift", ctypes.c_uint8), ("key_alt", ctypes.c_uint8), ("opt_move_fx", ctypes.c_uint8),
                ("seen_enemies", ctypes.c_uint16), ("in_flight", ctypes.c_uint8), ("merc_shown", ctypes.c_uint8),
                ("interrupt_turn", ctypes.c_uint8), ("fx_anims", u8p), ("tick_step", ctypes.c_int32),
                ("view_rays", u8p), ("obj_block", u8p), ("story_finder", ctypes.c_uint8),   # Merc view
                ("spot_stop", ctypes.c_uint8),
                ("itq", u8p), ("itq_len", ctypes.c_uint8), ("bled", u8p), ("timed_fx", u8p),   # turns.md
                ("new_round", ctypes.c_uint8), ("turn_timer", ctypes.c_int16), ("turn_timer2", ctypes.c_int16),
                ("shown_slot", ctypes.c_int32), ("view_tile", ctypes.c_int32), ("focus_slot", ctypes.c_uint8),
                ("turn_shown", ctypes.c_uint8), ("merc_to_show", ctypes.c_uint8), ("view_hold", ctypes.c_uint8),
                ("weather_tire", ctypes.c_int32), ("merc_tire", ctypes.c_int8),
                ("round_lock", ctypes.c_uint8), ("round_lock2", ctypes.c_uint8), ("handed_over", ctypes.c_uint8),   # turns.md New turn
                ("turn_flag", ctypes.c_uint8), ("interrupter", ctypes.c_uint8), ("speaker", ctypes.c_uint8),
                ("speech_wait", ctypes.c_uint8), ("stage", ctypes.c_uint8), ("redraw_next", ctypes.c_uint8),
                ("redraw_pending", ctypes.c_int32), ("turn_clock", ctypes.c_int32), ("show_timer", ctypes.c_int32),
                ("sector_spoken", u8p),
                ("silencer", u8p), ("shot_sound", u8p), ("impact_noise", u8p), ("shot_range", u8p),   # shooting.md
                ("busy_timer", ctypes.c_int32), ("replay_m", ctypes.c_uint8), ("first_hit", ctypes.c_uint8),
                ("fx_id", ctypes.c_uint8),
                ("dmg_text", u8p), ("marked_slot", ctypes.c_uint8), ("knockdown", ctypes.c_uint8),   # shooting.md batch 2
                ("focus_last", ctypes.c_uint8), ("hit_scale", ctypes.c_uint8),
                ("dead_mercs", ctypes.c_int32), ("dead_allies", ctypes.c_int32), ("dead_npc", ctypes.c_int32),   # death
                ("enemy_deaths", ctypes.c_int32), ("squad_lost", ctypes.c_uint8), ("last_killer", ctypes.c_uint8),
                ("posture", u8p), ("window", u8p), ("attack_sound", u8p), ("diff_mod", u8p),   # aiming (batch 5)
                ("key_ctrl", ctypes.c_uint8),   # click.md
                ("fast_timer", ctypes.c_int32),   # turns.md Turn start
                ("marks_shown", ctypes.c_int32), ("marks_n", ctypes.c_int32), ("marks", u8p),   # select.md
                ("opt_path_marks", ctypes.c_uint8), ("sees_slot", ctypes.c_uint8), ("preview_slot", ctypes.c_uint8),
                ("ripple_timer", ctypes.c_int32), ("ripple_tile", ctypes.c_int32), ("ambient_t1", ctypes.c_int32),   # Update end
                ("ambient_t2", ctypes.c_int32), ("ambient_count", ctypes.c_uint8), ("opt_ambient", ctypes.c_uint8),
                ("time_compress", ctypes.c_uint8), ("enemies_warned", ctypes.c_uint8), ("leave_reason", ctypes.c_uint8),
                ("fuse_beep_timer", ctypes.c_int32), ("fuse_beep", ctypes.c_uint8),
                ("turn_tick", ctypes.c_int32),   # Loop head
                ("quit_tactical", ctypes.c_uint8),   # soldier_update.md Leaving the sector
                ("inv_cursor", u8p), ("inv_swap", u8p), ("inv_picked", u8p), ("inv_give", u8p),   # inventory.md
                ("item_rects", u8p), ("slot_rects", u8p), ("vest_owner", ctypes.c_uint8),
                ("mouse_x", ctypes.c_int32), ("mouse_y", ctypes.c_int32), ("mouse_buttons", ctypes.c_uint8),
                ("key_flags", ctypes.c_uint16), ("tip_item", ctypes.c_uint8), ("face_left", ctypes.c_uint8),
                ("cursor_drop", ctypes.c_uint8), ("drop_answer", ctypes.c_int32), ("panel_dy", ctypes.c_int32),
                ("voice_test", ctypes.c_uint8),
                ("accounts", u8p), ("prev_balance", ctypes.c_int32), ("sector_order", u8p),   # strategic.md
                ("profiles", u8p), ("stash_counts", u8p), ("stat_progress", u8p), ("greed", u8p),   # Morning pay
                ("stat_changed", u8p),   # strategic.md Morning stat changes
                ("duty7_merc", ctypes.c_uint8),   # strategic.md Back on duty
                ("sector_replay", u8p),   # strategic.md Day start
                ("guide_queue", u8p),     # strategic.md Story events at the day start
                ("clock_minute", ctypes.c_uint8), ("clock_hour", ctypes.c_uint8), ("day_hours", ctypes.c_int8),
                ("day_minutes", ctypes.c_int32), ("day_bar", ctypes.c_int32), ("hard_day", ctypes.c_uint8),
                ("night_warned", ctypes.c_uint8), ("guard_eta", u8p), ("guard_rec", u8p),   # turns.md The day clock
                ("guides_hired", ctypes.c_uint8), ("guide_wait", ctypes.c_uint8),
                ("idle_day", ctypes.c_uint8), ("enemy_counter", ctypes.c_uint8),
                ("start_trees", ctypes.c_int32), ("start_workable", ctypes.c_int32), ("start_tapped", ctypes.c_int32),
                ("start_sectors", ctypes.c_int32), ("start_cap", ctypes.c_int32), ("start_plants", ctypes.c_int32),
                ("item_told", u8p), ("bonus_due", ctypes.c_uint8), ("bonus", ctypes.c_uint16),
                ("blaster", ctypes.c_uint8), ("gas_clouds", u8p), ("tileset", ctypes.c_uint8), ("no_break", u8p),
                ("walter_said", u8p), ("walter_next", u8p), ("walter_sector", ctypes.c_uint8), ("strip_box", ctypes.c_uint8), ("opt_replays", ctypes.c_uint8), ("replay_carry", ctypes.c_uint8), ("opt_confirm_move", ctypes.c_uint8), ("detector_slot", ctypes.c_uint8), ("detector_tile", ctypes.c_int16)]   # combat.md The listening device   # combat.md Blast, exact steps   # strategic.md The day's verdict   # world_load.md section 4 step 3   # strategic.md The guide's advice


RANDOM_FN = ctypes.CFUNCTYPE(ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32)
def _with_entry_chance(op, args, out, out_size):
    """Op 48 hit_test: the chance on entry sits in the out record. The tests compare it as a tenth argument."""
    if op == 48 and out_size >= 8:
        return args + [int.from_bytes(bytes(out[4:8]), "little", signed=True)]
    return args


CALL_FN = ctypes.CFUNCTYPE(ctypes.c_uint32, ctypes.c_void_p, ctypes.c_int32, ctypes.POINTER(ctypes.c_int32),
                           ctypes.c_int32, u8p, ctypes.c_int32)


class Host(ctypes.Structure):
    _fields_ = [("ctx", ctypes.c_void_p), ("random", RANDOM_FN), ("call", CALL_FN), ("host_ops", ctypes.c_uint64)]


ALL_OPS = (1 << 64) - 1      # host_ops: every op goes to the host (a test that scripts every callee)
SECTOR_LIB_OPS = ALL_OPS & ~(1 << 51 | 1 << 58 | 1 << 33 | 1 << 44 | 1 << 17 | 1 << 45)   # the library runs replace, reserve_ap
                                    # (sector_start.md batch 9), radio_result (10), quest_check (11), action_ap (12)


# host op number -> callee name in ai_diff.CALLEES (port_api.md table)
OPS = {1: "idle", 2: "noise_tile", 3: "find_dry_tile", 4: "patrol_tile", 5: "buddy_tile", 6: "direction",
       7: "tile_dist", 8: "news_value", 9: "sector_value", 10: "approach_tile", 11: "ally_call_tile",
       12: "search_tile", 13: "move_ap", 14: "reach_ap", 15: "attack_ready", 16: "swap_hand", 17: "action_ap",
       18: "can_stand_at", 19: "mission_cancel", 20: "opp_strength", 21: "retreat_tile", 22: "advance_tile",
       23: "target_tile", 24: "gun_option", 25: "throw_option", 26: "knife_option", 27: "busy", 28: "turn",
       29: "walk_to", 30: "click", 31: "spend_ap", 32: "wait", 33: "radio_result", 34: "share_sightings",
       35: "perceive", 36: "redraw", 37: "delay", 38: "santino_speech", 39: "fire", 40: "path_ap", 41: "find_path", 42: "fire_line", 43: "connected_home", 44: "quest_check", 45: "shot_ap",
       46: "chance_to_hit", 47: "div_zero", 48: "hit_test", 49: "sight_line", 50: "end_task", 51: "replace", 52: "set_anim", 53: "stop_anim", 54: "merc_message",
       55: "ui_refresh", 56: "anim_turn", 57: "merc_text", 58: "reserve_ap", 59: "walk_sound", 60: "sfx",
       61: "anim_frame", 62: "reveal_view", 63: "enemy_turned", 64: "guard_turned", 65: "w_17b70",
       66: "w_3f490", 67: "quote", 68: "w_58574", 69: "use_special", 70: "door_fx", 71: "popups", 72: "stop_walk",
       73: "w_3cdd4", 74: "w_3cfd0", 75: "bury_mine", 76: "w_401b4", 77: "w_3521c", 78: "w_3a8c0", 79: "w_5abe0",
       80: "w_5ad50", 81: "launch", 82: "merc_dies", 83: "anim_end", 84: "anim_step", 85: "w_53660",
       86: "open_cd", 87: "fread", 88: "fclose", 89: "replay_event", 90: "explode", 91: "fx_157bc", 92: "open_tmp",
       93: "sector_resets", 94: "keep_selected", 95: "sector_tail", 96: "fseek",
       97: "anim_next", 98: "walk_blocked", 99: "focus", 100: "cry", 101: "voc", 102: "reloaded", 103: "quote_event",
       104: "close_door", 105: "stat_practice", 106: "consume_trigger", 107: "break_obj", 108: "w_1ac68",
       109: "w_3c05c", 110: "take_damage", 111: "w_735bc", 112: "tac_redraw", 113: "w_1304c", 114: "next_turn",
       115: "sight_check", 116: "w_73f3c", 117: "throw_item", 118: "mine_flag", 119: "ai_think", 120: "w_1148c",
       121: "w_59ae8", 122: "w_5afbc", 123: "turn_banner", 124: "w_1aaf0", 125: "message", 126: "w_32eb8", 127: "make_noise", 128: "w_73d28", 129: "show_msg", 130: "tile_marker",
       131: "itq_pop", 132: "w_12dd4", 133: "w_12404", 134: "new_turn", 135: "portrait", 136: "projectile_step",
       137: "frame_active", 138: "frame_other", 139: "step_sound", 140: "move_merc", 141: "w_11fb4",
       142: "mouse_hide", 144: "w_10454", 145: "w_67bf0", 146: "w_681d0", 147: "w_67e18",
       148: "w_10398", 149: "w_331a4", 150: "w_355d0", 151: "aid_tick", 152: "w_32d04", 153: "w_323d4",
       154: "energy_bar", 155: "w_33204", 156: "w_328a4", 157: "w_67e30", 158: "w_6a960", 159: "guards_arrive",
       160: "w_1583c", 161: "gas_tick", 162: "mouse_check", 163: "cursor_reset", 164: "w_124fc", 165: "w_1259c",
       166: "item_panel", 167: "w_410c4", 168: "w_297d4", 169: "w_635dc", 170: "stage_five", 171: "w_31550", 172: "w_32828",
       173: "w_746d4", 174: "item_cond", 175: "bullet_damage", 176: "land_item", 177: "story_hit", 178: "puff",
       179: "door_lookup", 180: "hit_location", 181: "anim2", 182: "w_73ab4",
       183: "loss_msg", 184: "merc_sees", 185: "w_1c438", 186: "death_show", 187: "sector_won", 188: "squad_lost", 189: "dir_atan", 190: "reloaded",
       191: "first_aid", 192: "attack_cursor", 193: "voc", 194: "banner",     # shooting.md
       195: "w_46a4c", 196: "panel_view", 197: "w_11448", 198: "w_67c54", 199: "w_1c464", 200: "cursor_select",   # select.md
       201: "sector_gained", 202: "w_61688", 203: "enemies_msg", 204: "music", 205: "tile_anim", 206: "pal_cycle",
       207: "pal_set",   # soldier_update.md Update end
       208: "strip",     # soldier_update.md Loop head
       209: "door_snd", 210: "door_anim",   # combat.md Doors
       211: "hears", 212: "w_73c00", 213: "hears_msg",
       214: "leave_sector", 215: "fade", 216: "music_stop", 217: "end_of_day", 218: "resolve_battle", 219: "save_tmp",
       220: "timer_off",
       221: "inv_poll", 222: "key_pressed", 223: "get_key", 224: "mouse_show", 225: "inv_cursor_pic",   # inventory.md
       226: "inv_cursor_pic2", 227: "inv_click", 228: "inv_begin", 229: "inv_end", 230: "item_value", 231: "item_info",
       232: "inv_body", 233: "tip_item", 234: "tip_merc", 235: "drop_answer", 236: "text_toggle", 237: "inv_extra",
       238: "draw_slot", 239: "vestmap", 240: "no_room", 241: "rest", 242: "inv_select", 243: "inv_leave_merc",
       244: "inv_moved", 245: "inv_vest_begin", 246: "inv_vest_end", 247: "mine_spotted", 248: "disarm",
       249: "pick_up", 250: "inv_put_item", 251: "merge_prompt", 252: "merge_quote", 253: "merge_yes_no",
       254: "eod_begin", 255: "eod_wait", 256: "eod_text", 257: "eod_poll", 258: "eod_sound_stop",   # soldier_update.md End of day
       259: "overhead_sounds", 260: "show_screen", 261: "natives_quit_msg", 262: "map_redraw",
       263: "face_clear", 264: "stash_put", 265: "face_move", 266: "team_redraw", 267: "runs_off", 268: "island_map",
       269: "runs_off_end", 270: "pd_message", 271: "pd_timer_start", 272: "pd_elapsed", 273: "pd_clear",
       274: "pd_blink", 275: "pd_ask", 276: "pd_quote", 277: "pd_balloon", 278: "pd_end", 279: "pd_face_frame",
       280: "pd_panel_show", 281: "practice", 282: "xp_stat_begin",
       283: "xp_count", 284: "xp_health_bar", 285: "xp_salary_msg", 286: "xp_stat_end", 287: "xp_end",
       288: "cp_begin", 289: "cp_quote", 290: "cp_end", 291: "hate_quote",
       292: "corpse_ask", 293: "corpse_paid", 294: "corpse_end",
       295: "raise_voice", 296: "raise_ask", 297: "raise_offer", 298: "raise_answer", 299: "raise_done", 300: "raise_redraw", 301: "payroll_show", 302: "guide_say", 303: "day_ask", 304: "sunrise_show", 305: "merc_gone", 306: "map_show", 307: "quest_record", 308: "stash_take",
       309: "day_bar", 310: "enemy_pick", 311: "enemy_move", 312: "guard_enter", 313: "mode_banner", 314: "guards_msg",
       315: "mouse_release", 316: "refresh"}   # turns.md The day clock, Time compression   # strategic.md Morning pay     # soldier_update.md Leaving the sector     # ai.md Make noise, Hearing a noise, Noise interrupt
OPS.update({317: "brenda_say", 318: "panel_draw", 319: "guide_offer", 320: "merc_join"})   # strategic.md The guide's advice
OPS.update({326: "guard_sees_msg", 327: "clear_rooms"})   # ai.md Sightings as steps
OPS.update({328: "fire_quote"})   # strategic.md Firing a merc
OPS.update({329: "enemy_walk_in"})   # strategic.md Enemy moves
OPS.update({330: "home_lost_msg", 331: "rating_word"})   # strategic.md The day's verdict
OPS.update({337: "aim_remark", 338: "aim_removed"})
OPS.update({340: "w_72568", 341: "w_19eb0", 342: "w_35774", 343: "w_3c16c", 344: "w_54ec8", 345: "disarm_ask", 346: "ground_item_reset", 347: "menu_redraw", 348: "walter_say", 349: "inv_open", 350: "mouse_to", 351: "wait_up", 352: "weapon_cursor"})   # panel.md Clicks on the bottom strip   # combat.md Springing a ground item   # strategic.md The A.I.M. screen
OPS.update({353: "replay_count", 354: "replay_record", 355: "replay_anim", 356: "replay_draw"})   # strategic.md The day replay
OPS.update({332: "battle_ui", 333: "battle_wait", 334: "battle_box", 335: "battle_msg", 336: "explode_art"})   # strategic.md Sector battles
CRIES = ("BAD", "GOOD", "EMPTY", "NOTHING", "HMM")     # op 100 cry: number -> the text the original passes
# ai_diff decider name -> (C entry point, number of extra int arguments; "out16" = a u16 output pointer)
AI_ENTRY = {"think": ("ja1_ai_tick", False), "update_alert": ("ja1_ai_update_alert", False),
            "choose_action": ("ja1_ai_choose_action", True), "calm_action": ("ja1_ai_calm_action", False),
            "alert_action": ("ja1_ai_alerted_action", False), "combat_action": ("ja1_ai_combat_action", False),
            "engaged_action": ("ja1_ai_engaged_action", False),
            "action_affordable": ("ja1_ai_action_affordable", False), "do_action": ("ja1_ai_start_action", False),
            "noise_tile": ("ja1_ai_noise_tile", "out16"), "find_dry_tile": ("ja1_ai_find_dry_tile", 0),
            "patrol_tile": ("ja1_ai_patrol_tile", 0), "buddy_tile": ("ja1_ai_buddy_tile", 0),
            "news_value": ("ja1_ai_news_value", 0), "approach_tile": ("ja1_ai_approach_tile", 1),
            "ally_call_tile": ("ja1_ai_ally_call_tile", 0), "can_stand_at": ("ja1_ai_can_stand_at", 1),
            "advance_tile": ("ja1_ai_advance_tile", 1), "target_tile": ("ja1_ai_target_tile", 0),
            "opp_strength": ("ja1_ai_opp_strength", 0), "soldier_power": ("ja1_ai_soldier_power", 2),
            "retreat_tile": ("ja1_ai_retreat_tile", 0), "search_tile": ("ja1_ai_search_tile", 1),
            "sector_value": ("ja1_ai_sector_value", 1), "gun_option": ("ja1_ai_gun_option", "out10"),
            "knife_option": ("ja1_ai_knife_option", "out10"), "throw_option": ("ja1_ai_throw_option", "out10"),
            "line_of_fire": ("ja1_ai_line_of_fire", 2),
            "anim_delay": ("ja1_anim_delay", False), "anim_step": ("ja1_anim_step", False), "anim_end": ("ja1_anim_end", False), "merc_view": ("ja1_merc_view", 3),
            "set_anim": ("ja1_anim_set", 3), "anim_frame": ("ja1_anim_frame", False),
            "anim_turn": ("ja1_anim_turn", 1), "xra_enemy": ("ja1_xra_enemy", False),
            "replace": ("ja1_replace_soldier", False), "reserve_ap": ("ja1_reserve_ap", False),
            "soldier_frame": ("ja1_soldier_frame", False), "active_frame": ("ja1_active_frame", False),
            "walk_step": ("ja1_walk_step", False), "walk_blocked": ("ja1_walk_blocked", 2),
            "walk_end": ("ja1_walk_end", 1), "stand_up": ("ja1_stand_up", False),
            "frame_checks": ("ja1_frame_checks", False), "merc_quote": ("ja1_merc_quote", False),
            "enemies_see": ("ja1_enemies_see", False), "mercs_see": ("ja1_enemy_step_seen", False),
            "step_noise": ("ja1_step_noise", False), "consume_trigger": ("ja1_consume_trigger", False),
            "throw_follow": ("ja1_throw_follow", False), "throw_aim": ("ja1_throw_aim", 1),
            "throw_scatter": ("ja1_throw_scatter", 2),
            "find_path": ("ja1_find_path_op", 2), "path_ap": ("ja1_path_ap_marks", 6),
            "walk_to": ("ja1_walk_to", 1), "idle": ("ja1_idle", False),
            "move_order": ("ja1_move_order", 6), "stop_anim": ("ja1_stop_anim", 1),
            "start_turn": ("ja1_start_turn", False), "bleed": ("ja1_bleed", False), "drown": ("ja1_turn_drown", False),
            "turn_rest": ("ja1_turn_rest", False),
            "aid_tick": ("ja1_aid_tick", False), "end_task": ("ja1_end_task", 1), "move_ap_cost": ("ja1_move_ap", False), "w_59ae8": ("ja1_use_cost", 4), "swap_hand": ("ja1_swap_hand", 2), "busy": ("ja1_busy", False), "turn_to": ("ja1_turn", 2), "first_aid": ("ja1_first_aid", 3),   # combat.md First aid
            "spend_ap": ("ja1_spend_ap", 2), "rest": ("ja1_rest", 2),
            "launch": ("ja1_launch", False), "pstep": ("ja1_projectile_step", False), "fire": ("ja1_fire_now", False),
            "w_56dd4": ("ja1_shot_end", 2), "impact": ("ja1_projectile_impact", 4),
            "take_damage": ("ja1_take_damage", 5), "hit_reaction": ("ja1_hit_reaction", 3),
            "hit_alert": ("ja1_hit_alert", 2), "mark_seen": ("ja1_mark_seen", 1),
            "bullet_damage": ("ja1_bullet_damage", False), "hit_location": ("ja1_hit_location", 2),
            "shot_alert": ("ja1_shot_alert", 1), "trigger": ("ja1_trigger_item", 3), "use_on_tile": ("ja1_use_on_tile", 2), "bury_mine": ("ja1_bury_mine", 1), "pickup_here": ("ja1_pickup_here", 0), "lock_kit": ("ja1_use_special", 0), "walter": ("ja1_walter", 1), "mine_flag": ("ja1_mine_flag", 1), "land_item": ("ja1_land_item", 5), "interrupt_check": ("ja1_interrupt_check", 2), "merc_interrupt": ("ja1_ask_merc_interrupt", 1), "enemy_interrupt": ("ja1_ask_interrupt", 1),
            "kill": ("ja1_kill", 1), "merc_dies": ("ja1_merc_dies", 1), "melee_hit": ("ja1_melee_hit", 1), "revive_roll": ("ja1_revive_roll", False),
            "start_attack": ("ja1_start_attack", 1), "attack_go": ("ja1_attack_go", False),
            "fire_gun": ("ja1_fire_gun", 1), "chance_to_hit": ("ja1_chance_to_hit", 2),
            "click": ("ja1_click", 2), "click_detector": ("ja1_click_detector", 1), "attack_ready": ("ja1_attack_ready", 1), "reach_tile": ("ja1_reach_tile", 2),
            "gun_jam": ("ja1_gun_jam", False), "w_59a54": ("ja1_jam_check", False),
            "hears": ("ja1_hears", 3), "noise_int": ("ja1_noise_interrupt", False),
            "sight_check": ("ja1_sight_check", 1), "update_sightings": ("ja1_perceive", 1),
            "share_sightings": ("ja1_share_sightings", 1), "enemy_turned": ("ja1_enemy_turned", False),
            "guard_turned": ("ja1_guard_turned", False),
            "deselect": ("ja1_deselect", 1), "teleport": ("ja1_teleport", 1), "debug_heal": ("ja1_debug_heal", 0), "knockdown_anim": ("ja1_knockdown_anim", 2)}   # ai.md Sightings as steps   # ai.md Hearing a noise, Noise interrupt   # click.md   # shooting.md   # turns.md   # soldier_update.md     # sector_value: the "slot" argument is the sector   # QueueTwoAnims 0x17F5C (op 181): next, cur


VOID_ENTRY = ("deselect", "teleport", "debug_heal", "use_on_tile", "think", "swap_hand", "update_alert", "anim_delay", "anim_step", "anim_end", "merc_view", "set_anim", "anim_frame", "anim_turn", "xra_enemy",
              "replace", "reserve_ap", "soldier_frame", "active_frame", "walk_step", "walk_blocked", "walk_end",
              "stand_up", "frame_checks", "merc_quote", "enemies_see", "mercs_see", "step_noise", "consume_trigger", "throw_follow", "throw_aim", "throw_scatter",
              "walk_to", "idle", "stop_anim", "start_turn", "bleed", "drown", "turn_rest", "aid_tick", "end_task", "spend_ap", "rest",
              "launch", "pstep", "fire", "w_56dd4", "impact", "take_damage", "land_item", "lock_kit", "walter", "mine_flag", "hit_reaction", "hit_alert", "mark_seen", "shot_alert", "interrupt_check", "kill", "melee_hit", "revive_roll", "merc_dies", "start_attack", "attack_go", "fire_gun", "gun_jam", "hears", "noise_int",
              "update_sightings", "share_sightings", "enemy_turned", "guard_turned")


def ai_run(name, bufs, scalars, slot, random, call, fallback=0, sync=None, host_ops=ALL_OPS):
    """run one AI entry point. bufs = {field: bytearray} (written back in place), scalars = {field: int} (updated
    in place); random(n) -> int, call(op_name, args) -> (result, out bytes or None). Returns the int32 result
    (noise_tile: (tile, loudness)). fallback = the extra argument where the entry takes one (a tuple where it takes several). host_ops = bit
    mask of the ops the host answers although the library implements them (port_api.md; default: all).
    sync = (out, in): out(scalars) runs before each host call and random(), in(scalars) after it, with bufs and
    scalars current (a host that works on its own copy of the world syncs it there)."""
    fn, has_fb = AI_ENTRY[name]
    f = getattr(lib(), fn)
    extra = ([ctypes.POINTER(ctypes.c_uint16)] if has_fb == "out16" else [u8p] if has_fb == "out10" else
             [ctypes.c_int32] * int(has_fb))
    f.argtypes = [ctypes.POINTER(World), ctypes.POINTER(Host), ctypes.c_int32] + extra
    f.restype = None if name in VOID_ENTRY or has_fb == "out10" else ctypes.c_int32
    arrays = {k: (ctypes.c_uint8 * len(v)).from_buffer(v) for k, v in bufs.items()}
    w = World(**{k: ctypes.cast(a, u8p) for k, a in arrays.items()}, **scalars)
    errors = []

    def synced(f, *a):
        if not sync:
            return f(*a)
        for k in scalars:
            scalars[k] = getattr(w, k)
        sync[0](scalars)
        try:
            return f(*a)
        finally:
            sync[1](scalars)
            for k in scalars:
                setattr(w, k, scalars[k])

    def rnd(_ctx, n):
        try:
            return synced(random, n)
        except BaseException as x:            # an exception must not unwind through Rust (SystemExit too)
            errors.append(x)
            return 0

    def cb(_ctx, op, args, nargs, out, out_size):
        try:
            r, data = synced(call, OPS.get(op, f"op{op}"), _with_entry_chance(op, [args[i] for i in range(nargs)], out, out_size))
            if data is not None:
                ctypes.memmove(out, bytes(data)[:out_size], min(len(data), out_size))
            return r & 0xFFFFFFFF
        except BaseException as x:
            errors.append(x)
            return 0
    host = Host(None, RANDOM_FN(rnd), CALL_FN(cb), host_ops)
    loud = ctypes.c_uint16(0)
    rec = (ctypes.c_uint8 * 10)()
    if has_fb == "out16":
        extra = [ctypes.byref(loud)]
    elif has_fb == "out10":                    # option record (10 bytes)
        extra = [ctypes.cast(rec, u8p)]
    else:
        extra = list(fallback) if isinstance(fallback, (list, tuple)) else [fallback] * int(has_fb)
    r = f(ctypes.byref(w), ctypes.byref(host), slot, *extra)
    if errors:
        raise errors[0]
    for k in scalars:
        scalars[k] = getattr(w, k)
    if has_fb == "out10":
        return bytes(rec)
    return (r or 0, loud.value) if has_fb == "out16" else r or 0


_I = ctypes.c_int32
SECTOR_ENTRY = {"mission_blast": ("ja1_mission_blast", [_I]), "call_enemies": ("ja1_call_enemies", [_I, _I]),
                "toggle_doors": ("ja1_toggle_doors", [_I, _I]), "slot_reset": ("ja1_slot_reset", [_I, _I]), "item_init": ("ja1_item_init", [u8p, _I, _I]),
                "equip": ("ja1_equip", [_I, _I]), "init_tapper": ("ja1_init_tapper", [_I] * 3),
                "init_guard": ("ja1_init_guard", [_I] * 5), "guard_tile": ("ja1_guard_tile", [_I] * 3),
                "place_merc": ("ja1_place_merc", [_I] * 3), "merc_entry": ("ja1_merc_entry", [_I] * 2),
                "put_item": ("ja1_put_item", [_I, u8p] + [_I] * 4), "take_ground_item": ("ja1_take_ground_item", [_I] * 2),
                "take_ground_patch": ("ja1_take_ground_patch", [_I] * 2), "pick_up": ("ja1_pick_up", [_I] * 3), "disarm": ("ja1_disarm", [_I] * 5), "mine_spotted": ("ja1_mine_spotted", [_I] * 3), "soldier_start": ("ja1_soldier_start", [_I]),
                "tile_patch": ("ja1_tile_patch", [_I] * 5), "item_patch": ("ja1_item_patch", [_I] * 5),
                "crate_lost": ("ja1_crate_lost", [_I]), "crate_drops": ("ja1_crate_drops", []),
                "plant_sector": ("ja1_plant_sector", []), "plant_done": ("ja1_plant_done", []),
                "fixed_items": ("ja1_fixed_items", []), "tmp_file": ("ja1_tmp_file", []),
                "ground_blast": ("ja1_ground_blast", [_I] * 5), "setup_sector": ("ja1_setup_sector", [_I] * 2), "radio_result": ("ja1_radio_result", []), "mission_cancel": ("ja1_mission_cancel", []),
                "quest_check": ("ja1_quest_check", [_I]), "action_ap": ("ja1_action_ap", [_I] * 3),
                "shot_ap": ("ja1_shot_ap", [_I] * 2),
                "edge_tile": ("ja1_edge_tile", [_I]), "guard_enter": ("ja1_guard_enter", [_I]),     # strategic.md Walk-ins
                "enemy_walk_in": ("ja1_enemy_walk_in", [_I] * 2),
                "resolve_battle": ("ja1_resolve_battle", [_I] * 2), "sector_gained": ("ja1_sector_gained", [_I]),   # Sector battles
                "stat_practice": ("ja1_stat_practice", [_I] * 4),   # combat.md Stat practice
                "blast": ("ja1_blast", [_I] * 3), "blast_tile": ("ja1_blast_tile", [_I] * 4), "gas_tick": ("ja1_gas_tick", []),   # combat.md Blast, exact steps
                "sector_tick": ("ja1_sector_tick", []), "explode": ("ja1_explode", [_I] * 4), "effect_hit": ("ja1_effect_hit", [_I] * 3), "explode_dat": ("ja1_explode_dat", [_I] * 2),   # combat.md Explosion, exact steps
                "kept_turn": ("ja1_kept_turn_start", []), "end_turn": ("ja1_end_turn", []), "first_merc": ("ja1_first_merc", []), "escape_stop": ("ja1_escape_stop", []), "next_merc": ("ja1_next_merc", [_I]), "next_enemy": ("ja1_next_enemy", [_I]), "swap_places": ("ja1_swap_places", [_I, _I]),
                "turn_sightings": ("ja1_turn_sightings", [_I]), "itq_pop": ("ja1_itq_pop", []),
                "enemies_turn": ("ja1_enemies_turn", []), "mercs_turn": ("ja1_mercs_turn", []),
                "new_turn": ("ja1_new_turn", [_I]), "next_turn": ("ja1_next_turn", [_I]), "interrupt_turn": ("ja1_interrupt_turn", []),
                "enter_sector": ("ja1_enter_sector", [_I]), "frame": ("ja1_frame", []), "tick": ("ja1_tick", []),
                "loop_rest": ("ja1_update_end", []), "fuses": ("ja1_fuses", []), "loop_head": ("ja1_loop_head", []), "leave_tests": ("ja1_leave_tests", []), "sector_won": ("ja1_sector_won", []),
                "turn_start": ("ja1_turn_start", []), "ally_alert": ("ja1_ally_alert", [_I]),
                "select_merc": ("ja1_select_merc", [_I] * 3), "select_soldier": ("ja1_select_soldier", [_I] * 3), "on_screen": ("ja1_on_screen", [_I]),
                "hide_path_marks": ("ja1_hide_path_marks", [_I]),     # select.md
                "door_lookup": ("ja1_door_lookup", [_I]), "break_object": ("ja1_break_object", [_I] * 3),   # combat.md Doors
                "close_door": ("ja1_close_door", [_I] * 3), "door_fx": ("ja1_door_fx", [_I] * 3),
                "reload": ("ja1_reload", [_I]),     # combat.md Reload
                "make_noise": ("ja1_make_noise", [_I] * 5),     # ai.md Make noise
                "leave_path": ("ja1_leave_path", []), "leave_sector": ("ja1_leave_sector", [_I]),
                "end_of_day": ("ja1_end_of_day", []), "connected_home": ("ja1_connected_home", [_I]),
                "calc_ledger_all": ("ja1_calc_ledger_all", []), "apply_day_ledger": ("ja1_apply_day_ledger", []),
                "recalc_territory": ("ja1_recalc_territory", []), "worker_pool": ("ja1_worker_pool", []),
                "cash_count": ("ja1_cash_count", [_I, _I]), "collect_cash": ("ja1_collect_cash", []),
                "stash_item": ("ja1_stash_item", [_I, _I]), "merc_leaves": ("ja1_merc_leaves", [_I, _I]), "fire_merc": ("ja1_fire_merc", [_I, _I]),
                "fire_merc_core": ("ja1_fire_merc_core", [_I, _I, _I]),
                "merc_join": ("ja1_merc_join", [_I]),     # strategic.md A merc joins the team
                "team_compact": ("ja1_team_compact", [_I]), "pay_mercs": ("ja1_pay_mercs", []),   # strategic.md Morning pay
                "natives_quit": ("ja1_natives_quit", [_I]),   # strategic.md   # soldier_update.md Leaving the sector
                "hit_test": ("ja1_hit_test", [_I] * 9 + [u8p]),   # shooting.md batch 5 (out: a bytearray argument)
                "inventory_screen": ("ja1_inventory_screen", [_I, _I]),   # inventory.md
                "strip_click": ("ja1_strip_click", [_I, _I]),   # panel.md Clicks on the bottom strip
                "payday_dialog": ("ja1_payday_dialog", []),               # strategic.md Not enough funds
                "morning_duties": ("ja1_morning_duties", []),             # strategic.md Morning duties
                "stat_changes": ("ja1_stat_changes", []),                 # strategic.md Morning stat changes
                "duty_wake": ("ja1_duty_wake", []), "profile_sync": ("ja1_profile_sync", []),   # Back on duty, Profiles
                "complain_pick": ("ja1_complain_pick", []), "complain": ("ja1_complain", [_I]),   # AIM rate complaints
                "hated_check": ("ja1_hated_check", []),   # strategic.md A disliked merc in the team
                "corpse_export": ("ja1_corpse_export", []),   # strategic.md Corpse export
                "raise_salary": ("ja1_raise_salary", []),     # strategic.md Salary raise
                "morning": ("ja1_morning", []),               # strategic.md The morning
                "map_click": ("ja1_map_click", [ctypes.c_int32] * 3 + [u8p]),   # strategic.md Sector click on the island map
                "day_start": ("ja1_day_start", []),           # strategic.md Day start
                "sunrise": ("ja1_sunrise", []),               # strategic.md Sunrise
                "strategic_open": ("ja1_strategic_open", []), # strategic.md Strategic screen opening
                "story_day": ("ja1_story_day", []),           # strategic.md Story events at the day start
                "story_find": ("ja1_story_find", []),         # the story-object find scene 0x6179C
                "story_hit": ("ja1_story_hit", []),           # a hit on a story object 0x61D28 (op 177)
                "guide_advice": ("ja1_guide_advice", []),     # strategic.md The guide's advice
                "day_verdict": ("ja1_day_verdict", []), "day_replay": ("ja1_day_replay", []), "day_bonus": ("ja1_day_bonus", []), "day_end": ("ja1_day_end", []), "day_end_accounts": ("ja1_day_end_accounts", []),   # The day's verdict
                "win_vars": ("ja1_win_vars", [u8p]), "performance_rating": ("ja1_performance_rating", []),   # ending.md
                "traverse_edge": ("ja1_traverse_edge", [_I, _I]), "effect_start": ("ja1_effect_start", [_I] * 6),
                "stage_effects": ("ja1_stage_effects", []), "stage_five": ("ja1_stage_five", []), "traverse_check": ("ja1_traverse_check", [_I, _I, u8p]),
                "traverse": ("ja1_traverse", [_I, _I, u8p]), "traverse_out": ("ja1_traverse_out", [_I]),
                "traverse_in": ("ja1_traverse_in", [_I, _I]),   # soldier_update.md Traverse
                "day_clock": ("ja1_day_clock", [_I]),         # turns.md The day clock
                "time_compress": ("ja1_time_compress", [_I]),  # turns.md Time compression
                "enemy_pick": ("ja1_enemy_pick", []),         # strategic.md Enemy moves
                "enemy_move": ("ja1_enemy_move", [_I]),
                "guard_open": ("ja1_guard_open", [u8p]), "guard_hover": ("ja1_guard_hover", [u8p, _I]),   # strategic.md
                "guard_click": ("ja1_guard_click", [u8p] + [_I] * 3), "guard_exit": ("ja1_guard_exit", [u8p]),   # The guard
                "guard_close": ("ja1_guard_close", [u8p]),                                                       # screen
                "sight_line": ("ja1_sight_line", [_I] * 9),   # ai.md Sight line (library op 49)
                "aim_refusal": ("ja1_aim_refusal", [_I]), "aim_hire": ("ja1_aim_hire", [_I]),   # strategic.md
                "aim_cancel_pending": ("ja1_aim_cancel_pending", []), "aim_browse": ("ja1_aim_browse", [_I, _I]),   # The A.I.M. screen
                "aim_open": ("ja1_aim_open", []), "aim_pay": ("ja1_aim_pay", [])}
SECTOR_INT = ("disarm", "day_end", "mine_spotted", "effect_start", "traverse_edge", "traverse_check", "traverse", "traverse_in", "aim_refusal", "performance_rating", "aim_hire", "aim_browse", "aim_open", "aim_pay", "day_verdict", "edge_tile", "guard_enter", "enemy_walk_in", "enemy_pick", "enemy_move", "cash_count", "merc_join", "sight_line", "door_lookup", "reload", "loop_rest", "loop_head", "leave_tests", "select_merc", "select_soldier", "on_screen", "hit_test", "first_merc", "itq_pop", "new_turn", "frame", "guard_tile", "tile_patch", "item_patch", "tmp_file", "radio_result", "quest_check", "action_ap", "shot_ap", "inventory_screen", "connected_home", "payday_dialog", "morning", "map_click", "day_start", "day_clock", "guard_open", "guard_hover", "guard_click", "guard_exit")      # entry points with an int32 result


def sector_run(name, bufs, scalars, args, random, call=None, host_ops=ALL_OPS, sync=False):
    """run one sector start entry point (sector_start.md; void). args = its arguments after the world and the host;
    an item pointer argument is given as (buffer name, byte offset) into bufs, or as bytes (a read-only copy).
    random / call as in ai_run. Returns the int32 result for SECTOR_INT entries. sync: scalars is current during
    each host call and the host's changes to it go back into the world (a host that answers on emulator memory)."""
    fn, types = SECTOR_ENTRY[name]
    f = getattr(lib(), fn)
    f.argtypes = [ctypes.POINTER(World), ctypes.POINTER(Host)] + types
    f.restype = ctypes.c_int32 if name in SECTOR_INT else None
    arrays = {k: (ctypes.c_uint8 * len(v)).from_buffer(v) for k, v in bufs.items()}
    w = World(**{k: ctypes.cast(a, u8p) for k, a in arrays.items()}, **scalars)
    errors = []

    def rnd(_ctx, n):
        try:
            return random(n)
        except BaseException as x:            # an exception must not unwind through Rust (SystemExit too)
            errors.append(x)
            return 0

    def cb(_ctx, op, a, nargs, out, out_size):
        try:
            if sync:
                for k in scalars:
                    scalars[k] = getattr(w, k)
            r, data = call(OPS.get(op, f"op{op}"), _with_entry_chance(op, [a[i] for i in range(nargs)], out, out_size))
            if sync:
                for k in scalars:
                    setattr(w, k, scalars[k])
            if data is not None:
                ctypes.memmove(out, bytes(data)[:out_size], min(len(data), out_size))
            return r & 0xFFFFFFFF
        except BaseException as x:
            errors.append(x)
            return 0
    host = Host(None, RANDOM_FN(rnd), CALL_FN(cb), host_ops)
    copies = [(ctypes.c_uint8 * len(a)).from_buffer_copy(a) for a in args if isinstance(a, bytes)]
    outs = [(ctypes.c_uint8 * len(a)).from_buffer(a) for a in args if isinstance(a, bytearray)]   # written in place
    cargs = [ctypes.cast(ctypes.byref(arrays[a[0]], a[1]), u8p) if isinstance(a, tuple) else
             ctypes.cast(copies.pop(0), u8p) if isinstance(a, bytes) else
             ctypes.cast(outs.pop(0), u8p) if isinstance(a, bytearray) else a for a in args]
    r = f(ctypes.byref(w), ctypes.byref(host), *cargs)
    if errors:
        raise errors[0]
    for k in scalars:
        scalars[k] = getattr(w, k)
    return r


def _world_map(ai):
    """World field -> (address, size, signed) for scalars, (address, size) for buffers (harness side: addresses)"""
    scalars = {"cur_sector": (ai.G_CUR_SECTOR, 1, 0), "alarm": (ai.G_ALARM, 1, 0), "combat": (ai.G_COMBAT, 1, 0),
               "difficulty": (ai.G_DIFFICULTY, 1, 0), "mission_step": (ai.G_MISSION, 1, 1),
               "mission_slot": (ai.G_MISSION_ORDER, 1, 0), "mission_target": (ai.G_MISSION_ORDER + 2, 2, 0),
               "mission_spot": (ai.G_MISSION_SPOT, 2, 0), "turn_based": (ai.G_TURN_BASED, 4, 1),
               "busy": (ai.G_BUSY, 4, 1), "active_slot": (ai.G_ACTIVE, 1, 0),
               "santino_spoke": (ai.G_SANTINO_SPOKE, 1, 0), "santino_timer": (ai.G_SANTINO_TIMER, 2, 1),
               "show_ai": (ai.G_SHOW_AI, 1, 0), "santino_word": (0x22A008, 4, 0),
               "last_spotter": (ai.G_LAST_SPOTTER, 1, 0), "day": (ai.G_DAY, 4, 1),
               "selected": (ai.G_SELECTED, 1, 0), "path_preview": (ai.G_MARKS_ON, 4, 1),
               "opt_sound": (0x1163C3, 1, 0), "opt_onscreen": (0x92DDF, 1, 0), "view_x": (ai.G_VIEW_X, 4, 1),
               "view_y": (ai.G_VIEW_Y, 4, 1), "view_w": (ai.G_VIEW_W, 4, 1), "view_h": (ai.G_VIEW_H, 4, 1),
               "anim_signal": (0x229E04, 4, 1), "effect_tile": (0x92E4C, 2, 1), "guard_skill": (ai.G_GUARD_SKILL, 1, 0),
               "bomb_count": (ai.G_BOMB_COUNT, 4, 1), "team_on_duty": (ai.G_TEAM_ON_DUTY, 4, 1),
               "purifier": (ai.G_PURIFIER, 1, 0), "safe_opened": (ai.G_SAFE_OPENED, 1, 0),
               "screen_drawn": (0x92DCC, 1, 0), "interrupt_go": (0x92DCF, 1, 0), "turn_moves": (0x92DD0, 1, 0),
               "kept_merc": (0x284F0F, 1, 0), "next_selected": (0x284F10, 1, 0), "quote_slot": (0x92DEC, 1, 0),
               "quote_tile": (0x92DEE, 2, 0), "quote_focus": (0x284F25, 1, 0), "quote_delay": (0x284F26, 1, 1),
               "quote_event": (0x284F27, 1, 0), "opt_half_delay": (0x1163C1, 1, 0), "opt_follow": (0x1163C2, 1, 0),
               "opt_reload_msg": (0x22A02E, 1, 0), "alone_warned": (0x22A0FF, 1, 0), "follow_all": (0x93458, 1, 0),
               "stop_timer": (0x95DC8, 4, 1), "interrupted": (ai.G_INTERRUPTED, 1, 0), "walk_ap": (0x229DF0, 4, 1),
               "spotted": (0x229C94, 4, 1), "opt_step_text": (0x1163BD, 1, 0),
               "key_shift": (0x284F21, 1, 0), "key_alt": (0x284F20, 1, 0), "key_ctrl": (0x284F1F, 1, 0), "opt_move_fx": (0x1163C7, 1, 0),
               "seen_enemies": (0x22A478, 2, 0), "in_flight": (0x92DC4, 1, 0), "merc_shown": (0x92DC8, 1, 0),
               "interrupt_turn": (0x92DDC, 1, 0), "tick_step": (0x92F60, 4, 1),
               "story_finder": (0x92DE3, 1, 0), "spot_stop": (0x92E64, 1, 0),
               "itq_len": (0x92DD1, 1, 0), "new_round": (0x22A0CD, 1, 0), "turn_timer": (0x22A486, 2, 1),
               "turn_timer2": (0x22A484, 2, 1), "shown_slot": (0x229CB4, 4, 1), "view_tile": (0x229C8C, 4, 1),
               "focus_slot": (0x92E52, 1, 0), "turn_shown": (0x92DCA, 1, 0), "merc_to_show": (0x92DC7, 1, 0),
               "view_hold": (0x92DCE, 1, 0),
               "weather_tire": (0x22A100, 4, 1), "merc_tire": (0x22A059, 1, 1),
               "round_lock": (0x92DE0, 1, 0), "round_lock2": (0x92DE1, 1, 0), "handed_over": (0x92DDD, 1, 0),
               "turn_flag": (0x92DC9, 1, 0), "interrupter": (0x92DD2, 1, 0), "speaker": (0x92DD5, 1, 0),
               "speech_wait": (0x284F02, 1, 0), "stage": (0x22A1F2, 1, 0), "redraw_next": (0x22A1F3, 1, 0),
               "redraw_pending": (0x92DA4, 4, 1), "turn_clock": (0x229D1C, 4, 1), "show_timer": (0x93D54, 4, 1),
               "busy_timer": (0x92DB4, 4, 1), "replay_m": (0x22A0D1, 1, 0), "first_hit": (0x22A1F5, 1, 0),
               "fx_id": (0x229E03, 1, 0), "marked_slot": (0x95E0C, 1, 0), "knockdown": (ai.G_KNOCKDOWN, 1, 0),
               "focus_last": (0x92E53, 1, 0), "hit_scale": (ai.G_HIT_SCALE, 1, 0),
               "dead_mercs": (ai.G_DEAD_MERCS, 4, 1), "dead_allies": (ai.G_DEAD_ALLIES, 4, 1),
               "dead_npc": (ai.G_DEAD_NPC, 4, 1), "enemy_deaths": (ai.G_ENEMY_DEATHS, 4, 1),
               "squad_lost": (ai.G_SQUAD_LOST, 1, 0), "last_killer": (0x22A0D2, 1, 0),
               "fast_timer": (ai.G_FAST_TIMER, 4, 1),
               "marks_shown": (ai.G_MARKS_SHOWN, 4, 1), "marks_n": (ai.G_MARKS_N, 4, 1),   # select.md
               "opt_path_marks": (0x1163BF, 1, 0), "sees_slot": (0x92DC6, 1, 0), "preview_slot": (0x92E58, 1, 0),
               "ripple_timer": (ai.G_RIPPLE_TIMER, 4, 1), "ripple_tile": (0x229AFC, 4, 1),     # soldier_update.md Update end
               "ambient_t1": (ai.G_AMBIENT_T1, 4, 1), "ambient_t2": (ai.G_AMBIENT_T2, 4, 1), "ambient_count": (0x284F19, 1, 0),
               "opt_ambient": (0x1163C0, 1, 0), "time_compress": (0x9300C, 1, 0), "enemies_warned": (0x22A0CF, 1, 0),
               "leave_reason": (0x93449, 1, 0), "fuse_beep_timer": (0x229E0C, 4, 1), "fuse_beep": (0x284F29, 1, 0),
               "turn_tick": (0x229D24, 4, 1), "quit_tactical": (0x92E74, 1, 0)}
    if INVENTORY:
        scalars.update(INV_SCALARS)
    bufs = {"soldiers": (ai.G_SOLDIERS, 0x30 * 0x200), "tiles": (ai.G_TILEMAP, 8 * 3136),
            "item_types": (ai.G_ITEM_KIND - 2, 136 * 12), "anim_table": (ai.ANIM_CAN_ACT, 256 * 8),
            "sector_state": (ai.G_SECTOR_TBL9, 80 * 9), "mission_done": (ai.G_MISSION_DONE, 80),
            "knowledge": (ai.G_KNOWN, 64), "fresher": (ai.G_FRESHER - 1152, 2304),
            "last_seen_m": (ai.G_LAST_SEEN_M, 16 * 8 * 2), "last_seen_e": (ai.G_LAST_SEEN_E, 8 * 40 * 2),
            "calls": (ai.G_CALL_M, 8), "paths": (ai.G_PATHS, 48 * 80), "dir_steps": (ai.G_DIR_STEP, 257 * 2),
            "t0_class": (ai.G_T0_CLASS, 256), "t1_class": (ai.G_T1_CLASS, 256),
            "remember_m": (ai.G_REMEMBER_M, 16 * 8), "remember_e": (ai.G_REMEMBER_E, 8 * 40),
            "story": (0x22A040, 35), "plants": (ai.G_PLANTS, 48), "crates": (ai.G_CRATE_DROPS, 16),
            "weapons": (ai.G_WEAPON_B5 - 5, 256 * 6), "absorb": (ai.G_ARMOR_ABSORB, 288 * 2),
            "anim_rows": (ai.ANIM_AP, 256 * 8), "anim_scripts": (ai.ANIM_FRAMES, 68 * 0x50),
            "anim_free": (ai.ANIM_FREE, 256), "anim_sounds": (ai.ANIM_SOUND, 256 * 2),
            "anim_steps_tile": (ai.ANIM_STEPS_TILE, 256), "anim_turn_while": (ai.ANIM_TURN_WHILE, 256),
            "supplies": (ai.G_EQ_ITEMS, 4), "equip_skill": (ai.G_EQ_SKILL, 6), "attach": (ai.ATTACH_TBL, 82 * 4),
            "ground_items": (ai.G_GROUND_ITEMS, 200 * 12), "notes": (ai.G_NOTES_FOUND, 30),
            "move_codes": (ai.G_MOVECODE, 3136 * 2), "held": (ai.G_HELD, 12), "blast_list": (ai.G_BLAST_LIST, 60),
            "listed": (ai.G_LISTED_ITEMS, 32), "t2_class": (ai.G_T2_CLASS, 256), "enemy_watch": (ai.G_SLOT_T34, 48),
            "text_buf": (ai.G_TEXT_BUF, 128), "item_buf": (ai.G_ITEM_BUF, 12), "crate_state": (ai.G_CRATE_STATE, 4),
            "crate_shown": (ai.G_CRATE_SHOWN, 4), "fixed_items": (ai.FIXED_ITEMS, 88),
            "sectors": (ai.G_SECTORS, 60 * 0xB1), "ap_warned": (ai.G_AP_WARNED, 8), "splash": (0x92E76, 60),
            "fx_anims": (0xEAFF0, 16 * 0x12), "itq": (0x22A412, 50), "bled": (0xFA928, 48), "timed_fx": (0x22A488, 65), "sector_spoken": (0x22A075, 60), "view_rays": (0x93554, 306), "obj_block": (0x95694, 685),
            "silencer": (0x92F00, 17), "shot_sound": (0x94DDC, 34), "impact_noise": (0x92F21, 33), "shot_range": (0x9634B, 9),
            "posture": (0x9300D, 256), "window": (0x95788, 2056), "attack_sound": (0x92E66, 10), "diff_mod": (0x95C4E, 6),
            "dmg_text": (ai.G_DMG_TEXT, 240), "marks": (ai.G_MARKS, 256),
            "scratch": (lambda e: e.r32(e.rt(ai.G_SCRATCH_PTR)), 0x200)}   # (an absolute address: the record it points to)
    if WALTER:      # combat.md The listening device at a door: ai_diff walter only (the world image leaves them NULL)
        bufs.update({"walter_said": (0x22A0B1, 12), "walter_next": (0x22A0C3, 8)})
        scalars["walter_sector"] = (0x92E2D, 1, 0)
    if BLAST:       # combat.md Blast, exact steps: ai_diff blast sections only (the world image leaves them NULL)
        bufs.update({"gas_clouds": (0x22A44E, 30), "no_break": (0x92E41, 10)})
        scalars.update({"blaster": (ai.G_BLASTER, 1, 0), "tileset": (0x22A038, 1, 0)})
    if PROFILES:    # combat.md Mine spotted, "G for g of 200 or more": entry 0xFF reads profile 5 (ai_diff use_on_tile,
        bufs["profiles"] = (ai.G_PROFILES, 64 * ai.PROFILE_SIZE)     # pickup_here, mine_spotted only)
    if INVENTORY:
        bufs.update(INV_BUFS)
    if LEDGER:      # strategic.md world fields: only for ai_diff ledger (other tests and the world image leave them NULL)
        bufs.update({"accounts": (0x229FA0, 0x98), "sector_order": (ai.G_SECTOR_ORDER, 61)})
        scalars["prev_balance"] = (ai.G_PREV_BALANCE, 4, 1)
    if PAY:         # strategic.md Morning pay: only for ai_diff pay (accounts grows to 0xA8: +0xA0 = resignations)
        bufs.update({"accounts": (0x229FA0, 0xA8), "sector_order": (ai.G_SECTOR_ORDER, 61),
                     "profiles": (ai.G_PROFILES, 64 * ai.PROFILE_SIZE), "stash_counts": (ai.G_STASH_COUNTS, 136),
                     "stat_progress": (ai.G_STAT_PROGRESS, 8 * 0x12), "greed": (ai.GREED, 256)})
        scalars["prev_balance"] = (ai.G_PREV_BALANCE, 4, 1)
        scalars["voice_test"] = (0x92F8B, 1, 0)
    if STATS:       # strategic.md Morning stat changes: only for xp_diff --port
        bufs.update({"stat_progress": (ai.G_STAT_PROGRESS, 8 * 0x12), "stat_changed": (0x10DD10, 640)})
    if MORNING:     # strategic.md Back on duty, Profiles take the team's stats: only for morning_diff --port
        bufs["profiles"] = (ai.G_PROFILES, 64 * ai.PROFILE_SIZE)
        scalars["duty7_merc"] = (0x92DDA, 1, 0)
    if DAYSTART:    # strategic.md Day start: only for day_diff --port
        bufs["sector_replay"] = (0x2254F0, 540)
        scalars.update({"opt_replays": (0x1163C4, 1, 0), "replay_carry": (0x9562D, 1, 0)})   # The day replay
    if STORYDAY:    # strategic.md Story events at the day start: storyday_diff / stratopen_diff --port
        bufs["guide_queue"] = (0x22A4C9, 7)
    if CLOCK:       # turns.md The day clock: clock_diff --port
        scalars.update({"clock_minute": (0x22A0F5, 1, 0), "clock_hour": (0x22A0F6, 1, 0), "day_hours": (0x22A0F4, 1, 1),
                        "day_minutes": (0x22A0F7, 4, 1), "day_bar": (0x22A0FB, 4, 1), "hard_day": (0x22A10C, 1, 0),
                        "night_warned": (0x22A0D3, 1, 0)})
        bufs.update({"guard_eta": (0x10D2B8, 960), "guard_rec": (0x10D0D8, 480)})
    if GUIDE:       # strategic.md The guide's advice: guide_diff --port (story = 43 bytes; the port and the world image
        bufs["story"] = (0x22A040, 43)   # still use 35 until ja1_guide_advice is ported: then make 43 the default)
        scalars.update({"guides_hired": (0x22A073, 1, 0), "guide_wait": (0x22A074, 1, 0), "hard_day": (0x22A10C, 1, 0)})
    if VERDICT:     # strategic.md The day's verdict: verdict_diff --port
        bufs["story"] = (0x22A040, 43)
        bufs["item_told"] = (0x22A1F6, 3)
        scalars.update({"start_trees": (0x229AC4, 4, 1), "start_workable": (0x229ACC, 4, 1),
                        "start_tapped": (0x229AD4, 4, 1), "start_sectors": (0x229AD8, 4, 1),
                        "start_cap": (0x229AF4, 4, 1), "start_plants": (0x229AF8, 4, 1),
                        "bonus_due": (0x92DD6, 1, 0), "bonus": (0x92DD8, 2, 0), "idle_day": (0x22A10A, 1, 0)})
    if ENEMY:       # strategic.md Enemy moves: enemy_diff --port
        bufs["sector_order"] = (ai.G_SECTOR_ORDER, 61)
        scalars["enemy_counter"] = (0x22A0BD, 1, 0)
    return scalars, bufs


# inventory.md world fields: mapped only for inv_diff (INVENTORY = True); other tests leave them 0 / NULL, because
# some of these addresses are scratch the original also uses elsewhere (0x229D18 = the PCC loader result)
INVENTORY = False
BLAST = False
WALTER = False
PROFILES = False
LEDGER = False
PAY = False
STATS = False
MORNING = False
DAYSTART = False
STORYDAY = False
CLOCK = False
GUIDE = False
ENEMY = False
VERDICT = False
INV_SCALARS = {"vest_owner": (0x92E40, 1, 0), "mouse_x": (0x229D00, 4, 1), "mouse_y": (0x229D04, 4, 1),
               "mouse_buttons": (0x229CF4, 1, 0), "key_flags": (0x22A46C, 2, 0), "tip_item": (0x93EA8, 1, 0),
               "face_left": (0x284F13, 1, 0), "cursor_drop": (0x955C9, 1, 0), "drop_answer": (0x229D18, 4, 1),
               "panel_dy": (0x229C48, 4, 1), "voice_test": (0x92F8B, 1, 0), "strip_box": (0x92E73, 1, 0)}
INV_BUFS = {"inv_cursor": (0x222B50, 12), "inv_swap": (0x222B68, 12), "inv_picked": (0x28EF30, 8),
            "inv_give": (0x28EF38, 2), "item_rects": (0x949F2, 56), "slot_rects": (0x93686, 32)}


def script_pos(rec, e, ai, to_port):
    """soldier +0x90: the original's pointer into the animation script table <-> the port's byte offset into
    anim_scripts (port_api.md); 32-bit wrap both ways, so any value converts back unchanged"""
    v = int.from_bytes(rec[0x90:0x94], "little")
    base = e.rt(ai.ANIM_FRAMES)
    rec[0x90:0x94] = ((v - base if to_port else v + base) & 0xFFFFFFFF).to_bytes(4, "little")


def _script_all(bufs, e, ai, to_port):
    sol = bufs["soldiers"]
    for k in range(0x30):
        rec = sol[0x200 * k:0x200 * k + 0x200]
        script_pos(rec, e, ai, to_port)
        sol[0x200 * k:0x200 * k + 0x200] = rec


def _addr(e, a):
    """a world field's emulator address: a game address, or a function of the emulator (a pointer the game holds)"""
    return a(e) if callable(a) else e.rt(a)


def world_load(e, ai, bufs=None, scalars=None):
    """the AI world from emulator memory: (bufs, scalars); given bufs / scalars are refilled in place. Soldier
    +0x90 becomes the script offset of the port world."""
    smap, bmap = _world_map(ai)
    if bufs is None:
        bufs = {k: bytearray(n) for k, (_, n) in bmap.items()}
    for k, (a, n) in bmap.items():
        bufs[k][:] = e.uc.mem_read(_addr(e, a), n)
    _script_all(bufs, e, ai, True)
    scalars = {} if scalars is None else scalars
    for k, (a, n, sg) in smap.items():
        scalars[k] = int.from_bytes(e.uc.mem_read(_addr(e, a), n), "little", signed=bool(sg))
    return bufs, scalars


READ_ONLY = ("view_rays", "obj_block", "fx_anims", "splash", "equip_skill", "attach", "item_types", "anim_table", "anim_rows", "anim_scripts", "anim_free", "anim_sounds", "anim_steps_tile", "anim_turn_while", "fresher", "dir_steps", "t0_class", "t1_class", "t2_class", "listed", "crates", "fixed_items",
             "weapons", "absorb", "silencer", "shot_sound", "impact_noise", "shot_range", "posture", "window", "attack_sound", "diff_mod", "item_rects", "slot_rects", "no_break")


def bufs_store(e, ai, bufs):
    """the world's writable buffers to emulator memory (a host op answered by the mirror on memory)"""
    _script_all(bufs, e, ai, False)
    for k, (a, n) in _world_map(ai)[1].items():
        if k not in READ_ONLY:
            e.uc.mem_write(_addr(e, a), bytes(bufs[k]))


def bufs_load(e, ai, bufs):
    """the world's writable buffers back from emulator memory (in place)"""
    for k, (a, n) in _world_map(ai)[1].items():
        if k not in READ_ONLY:
            bufs[k][:] = e.uc.mem_read(_addr(e, a), n)
    _script_all(bufs, e, ai, True)


def world_store(e, ai, bufs, scalars):
    """write the world's writable parts back to emulator memory (soldier +0x90 back to pointers, in bufs too)"""
    smap, bmap = _world_map(ai)
    _script_all(bufs, e, ai, False)
    for k, (a, n) in bmap.items():
        if k not in READ_ONLY:
            e.uc.mem_write(_addr(e, a), bytes(bufs[k]))
    for k, (a, n, sg) in smap.items():
        e.uc.mem_write(_addr(e, a), (scalars[k] & (1 << 8 * n) - 1).to_bytes(n, "little"))


def mouse_op_place(op):
    """The mouse place that the window sets at a host op of the morning (ja1front mouselimit::place_of_op); None: no move."""
    root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
    lib = ctypes.CDLL(os.path.join(root, "port", "ja1front", "target", "release", "libja1front.so"))
    out = (ctypes.c_int32 * 2)()
    return tuple(out) if lib.ja1f_mouse_op_place(op, out) else None


def mouse_op_limit(op):
    """The mouse limit that the window sets at a host op of the morning (ja1front mouselimit::of_op), or None."""
    root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
    lib = ctypes.CDLL(os.path.join(root, "port", "ja1front", "target", "release", "libja1front.so"))
    out = (ctypes.c_int32 * 4)()
    return tuple(out) if lib.ja1f_mouse_op_limit(op, out) else None
