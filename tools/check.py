"""Run every automated check of the project; exit 1 if any fails.

usage: ./t check [--quick] [--allow-skip]        --quick skips the slow differential tests (combat ~30 s, tiles ~55 s, move ~18 s, damage ~14 s, edt ~12 s, flight ~6 s, path ~20 s, ai ~12 s, compose ~60 s, compose --flight ~10 s, --turn ~5 s, --melee ~5 s, --blast ~5 s, --newturn ~10 s, --dies ~5 s)
Checks: defs/types.toml consistency (+ labels.csv names), labels.csv sanity (unique addresses and names, parseable),
Unicorn harness smoke test, differential tests validate/diff/*_diff.py (original code in Unicorn vs spec).
Tests that need local dumps (build/mem_*.bin, gitignored) FAIL when those are missing, so a fresh checkout cannot pass
without running the differential suite; --allow-skip turns that into a counted SKIP (the run is then not a commit gate).
The diff tests run CHECK_JOBS (see JOBS below) at a time; the line above the last one gives the total time.
Each finished run adds one line to build/check_history.jsonl (jobs, total, failed step names, slowest steps).
smoke_play runs as 4 jobs (--group K/4) and gate_speed as its parts (one "gate_speed.py" line for all of them).
"""
import csv
import glob
import json
import os
import re
import shutil
import subprocess
import sys
import threading
import time
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from common import ROOT, parse_addr
sys.path.insert(0, os.path.join(ROOT, "validate", "diff"))
import livecast                      # noqa: E402  (no-op progress hook)

PY = os.path.join(ROOT, ".venv", "bin", "python")
STEPS, TOTAL = [], [0]               # run() results so far; planned run() count, for the Live panel
TIMES, FAILED = {}, []               # run() seconds per step and failed step names, for build/check_history.jsonl
SLOW = {"serve_diff.py", "popup_diff.py", "morning_chain_diff.py", "soldier_diff.py", "combat_diff.py", "tiles_diff.py", "edt_diff.py", "damage_diff.py", "move_diff.py", "flight_diff.py", "path_diff.py", "ai_diff.py", "compose_diff.py", "walk_diff.py", "panel_diff.py", "panel_data_diff.py", "inv_diff.py", "invdraw_diff.py", "sight_diff.py"}
ARGS = {"invwrap_diff.py": ["1500"], "replayinit_diff.py": ["60"], "viewtile_diff.py": ["2000"], "render_diff.py": ["10"], "loadface_diff.py": ["50"], "stashclear_diff.py": ["50"], "stashload_diff.py": ["200"], "tilepal_diff.py": [], "mdi_diff.py": ["80", "--port"], "dbgpage_diff.py": ["200"], "tilepopup_diff.py": ["20"], "aimstats_diff.py": ["10"], "salary_diff.py": ["10"], "raisebox_diff.py": ["20"], "focus_diff.py": ["300"], "balloonview_diff.py": ["400"], "viewmode_diff.py": ["300"], "overview_diff.py": ["60"], "ovmouse_diff.py": ["300"], "msgbox_diff.py": ["50"], "balloon_diff.py": ["20"], "question_diff.py": ["30"], "iteminfo_diff.py": ["30"], "assign_diff.py": ["30"], "payroll_diff.py": ["20"], "office_diff.py": ["30"], "ledger_diff.py": ["20"], "switchbox_diff.py": ["20"], "battlebox_diff.py": ["20"], "dayreplay_diff.py": ["300"], "slots_diff.py": ["30"], "duty_diff.py": ["30"], "select_diff.py": ["30"], "assign_panel_diff.py": ["30"], "assign_hover_diff.py": ["30"], "mappanel_diff.py": ["20"], "strips_diff.py": ["300"], "island_diff.py": ["20"], "replaypic_diff.py": ["80"], "map_diff.py": ["500"], "day_diff.py": ["500"], "sunrise_diff.py": ["500"], "stratopen_diff.py": ["200"], "storyday_diff.py": ["500"], "storyfind_diff.py": ["1000", "--port"], "storyhit_diff.py": ["1000", "--port"], "clock_diff.py": ["500"], "guard_diff.py": ["300"], "guide_diff.py": ["1000"], "verdict_diff.py": ["3000"], "battle_diff.py": ["1000"], "practice_diff.py": ["1000"], "enemy_diff.py": ["1000"], "morning_chain_diff.py": ["100"], "corpse_diff.py": ["500"], "raise_diff.py": ["500"], "xp_diff.py": ["500"], "xpcount_diff.py": ["20"], "inv_diff.py": ["2000"], "popup_diff.py": ["100"], "loadpopup_diff.py": ["16", "--port"], "ctlpanel_input_diff.py": ["300", "--port"], "office_input_diff.py": ["300", "--port"], "sunrise_screen_diff.py": ["60", "--port"], "speechwait_diff.py": ["150", "--port"], "slotmouse_diff.py": ["2000", "--port"], "travprompt_diff.py": ["1500"], "allmove_diff.py": ["1500"], "confirm_diff.py": ["240", "--port"], "aimpage_diff.py": ["300", "--port"], "aimshow_diff.py": ["100", "--port"], "flipin_diff.py": ["30", "--port"], "menu_diff.py": ["500"], "dayend_diff.py": ["1500"], "tacredraw_diff.py": ["100"], "strat_input_diff.py": ["200", "--port"], "cursormode_diff.py": ["300", "--port"], "palrotate_diff.py": ["2000"], "vtile_diff.py": ["2000"], "fade_diff.py": ["200"],
        "soldier_diff.py": ["100", "build/mem_tac60.bin", "build/mem_combat1.bin", "build/mem_death1.bin"],
        "tiles_diff.py": ["build/mem_tac60.bin", "1", "--sectors", "1-60"],
        "compose_diff.py": ["25", "build/mem_a10.bin", "build/mem_a35.bin", "build/mem_combat1.bin", "build/mem_death1.bin",
                            "build/mem_f3.bin", "build/mem_fight20.bin", "build/mem_tac60.bin"]}
NEEDS = {"itemvoice_diff.py": "build/mem_tac60.bin", "keyhandler_diff.py": "build/mem_tac60.bin", "heal_diff.py": "build/mem_tac60.bin", "fullclips_diff.py": "build/mem_tac60.bin", "teleport_diff.py": "build/mem_tac60.bin", "startup_diff.py": "build/mem_tac60.bin", "sweeprows_diff.py": "build/mem_tac60.bin", "startworld_diff.py": "build/mem_tac60.bin", "aimleave_diff.py": "build/mem_tac60.bin", "font_diff.py": "build/mem_tac60.bin", "arcseek_diff.py": "build/mem_tac60.bin", "officehl_diff.py": "build/mem_tac60.bin", "confirm_diff.py": "build/mem_day3.bin", "aimpage_diff.py": "build/mem_aim_hired.bin", "aimshow_diff.py": "build/mem_aim_hired.bin", "flipin_diff.py": "build/mem_tac60.bin", "viewtile_diff.py": "build/mem_tac60.bin", "cursor_diff.py": "build/mem_fight59.bin", "loadface_diff.py": "build/mem_tac60.bin", "loadpopup_diff.py": "build/mem_tac60.bin", "stashclear_diff.py": "build/mem_tac60.bin", "stashload_diff.py": "build/mem_tac60.bin", "sectorreset_diff.py": "build/mem_tac60.bin", "tilepal_diff.py": "build/mem_tac60.bin", "tmpsave_diff.py": "build/mem_tac60.bin", "volume_diff.py": "build/mem_tac60.bin", "scroll_diff.py": "build/mem_tac60.bin", "wipe_diff.py": "build/mem_tac60.bin", "dbgpage_diff.py": "build/mem_tac60.bin", "tilepopup_diff.py": "build/mem_tac60.bin", "aimstats_diff.py": "build/mem_day3.bin", "salary_diff.py": "build/mem_day3.bin", "raisebox_diff.py": "build/mem_tac60.bin", "sanim_diff.py": "build/mem_day3.bin", "focus_diff.py": "build/mem_tac60.bin", "balloonview_diff.py": "build/mem_tac60.bin", "onscreen_diff.py": "build/mem_tac60.bin", "viewmode_diff.py": "build/mem_tac60.bin", "overview_diff.py": "build/mem_tac60.bin", "ovmouse_diff.py": "build/mem_tac60.bin", "click_diff.py": "build/mem_fight59.bin", "island_diff.py": "build/mem_tac60.bin", "replaypic_diff.py": "build/mem_tac60.bin", "msgbox_diff.py": "build/mem_tac60.bin", "balloon_diff.py": "build/mem_tac60.bin", "question_diff.py": "build/mem_tac60.bin", "iteminfo_diff.py": "build/mem_tac60.bin", "assign_diff.py": "build/mem_tac60.bin", "payroll_diff.py": "build/mem_tac60.bin", "office_diff.py": "build/mem_tac60.bin", "gameover_diff.py": "build/mem_tac60.bin", "sab_diff.py": "build/mem_tac60.bin", "credits_diff.py": "build/mem_tac60.bin", "ledger_diff.py": "build/mem_tac60.bin", "switchbox_diff.py": "build/mem_tac60.bin", "battlebox_diff.py": "build/mem_tac60.bin", "dayreplay_diff.py": "build/mem_tac60.bin", "slots_diff.py": "build/mem_tac60.bin", "newgame_diff.py": "build/mem_tac60.bin", "aim_diff.py": "build/mem_aim_hired.bin", "duty_diff.py": "build/mem_tac60.bin", "select_diff.py": "build/mem_tac60.bin", "assign_panel_diff.py": "build/mem_tac60.bin", "assign_hover_diff.py": "build/mem_tac60.bin", "assign_items_diff.py": "build/mem_tac60.bin", "mappanel_diff.py": "build/mem_tac60.bin", "strips_diff.py": "build/mem_tac60.bin", "day_diff.py": "build/mem_tac60.bin", "sunrise_diff.py": "build/mem_tac60.bin", "sunrise_screen_diff.py": "build/mem_tac60.bin", "speechwait_diff.py": "build/mem_tac60.bin", "slotmouse_diff.py": "build/mem_tac60.bin", "travprompt_diff.py": "build/mem_tac60.bin", "allmove_diff.py": "build/mem_tac60.bin", "menu_diff.py": "build/mem_tac60.bin", "dayend_diff.py": "build/mem_tac60.bin", "tacredraw_diff.py": "build/mem_tac60.bin", "stratopen_diff.py": "build/mem_tac60.bin", "storyday_diff.py": "build/mem_tac60.bin", "storyfind_diff.py": "build/mem_tac60.bin", "storyhit_diff.py": "build/mem_tac60.bin", "clock_diff.py": "build/mem_tac60.bin", "guard_diff.py": "build/mem_tac60.bin", "guide_diff.py": "build/mem_tac60.bin", "verdict_diff.py": "build/mem_tac60.bin", "rating_diff.py": "build/mem_tac60.bin", "trav_diff.py": "build/mem_tac60.bin", "stage_diff.py": "build/mem_tac60.bin", "battle_diff.py": "build/mem_tac60.bin", "practice_diff.py": "build/mem_tac60.bin", "enemy_diff.py": "build/mem_tac60.bin", "map_diff.py": "build/mem_tac60.bin", "morning_chain_diff.py": "build/mem_tac60.bin", "raise_diff.py": "build/mem_tac60.bin", "corpse_diff.py": "build/mem_tac60.bin", "morning_diff.py": "build/mem_tac60.bin", "complain_diff.py": "build/mem_tac60.bin", "xp_diff.py": "build/mem_tac60.bin", "xpcount_diff.py": "build/mem_tac60.bin", "duties_diff.py": "build/mem_tac60.bin", "payday_diff.py": "build/mem_tac60.bin", "panel_diff.py": "build/mem_tac60.bin", "invdraw_diff.py": "build/mem_tac60.bin", "sight_diff.py": "build/mem_tac60.bin", "popup_diff.py": "build/mem_tac60.bin", "rect_diff.py": "build/mem_tac60.bin", "inv_diff.py": "build/mem_tac60.bin", "panel_data_diff.py": "build/mem_tac60.bin", "soldier_diff.py": "build/mem_death1.bin", "combat_diff.py": "build/mem_*.bin", "tiles_diff.py": "build/mem_tac60.bin", "move_diff.py": "build/mem_tac60.bin", "flight_diff.py": "build/mem_tac60.bin", "path_diff.py": "build/mem_tac60.bin", "ai_diff.py": "build/mem_tac60.bin",
         "compose_diff.py": "build/mem_combat1.bin", "world_diff.py": "build/mem_wl_post.bin", "loadsector_diff.py": "build/mem_wl_pre.bin", "walk_diff.py": "build/mem_tac60.bin",
         "quicksave_diff.py": "build/mem_tac60.bin", "quickrestore_diff.py": "build/mem_tac60.bin", "pause_diff.py": "build/mem_tac60.bin", "toggle_diff.py": "build/mem_tac60.bin", "viewpose_diff.py": "build/mem_tac60.bin", "allowsave_diff.py": "build/mem_tac60.bin", "gradename_diff.py": "build/mem_tac60.bin", "clearrooms_diff.py": "build/mem_tac60.bin", "slotset_diff.py": "build/mem_tac60.bin", "terrainload_diff.py": "build/mem_tac60.bin", "picsize_diff.py": "build/mem_tac60.bin", "ctlpanel_diff.py": "build/mem_tac60.bin", "ctlpanel_input_diff.py": "build/mem_tac60.bin", "office_input_diff.py": "build/mem_tac60.bin", "strat_input_diff.py": "build/mem_tac60.bin", "cursormode_diff.py": "build/mem_tac60.bin", "palrotate_diff.py": "build/mem_tac60.bin", "vtile_diff.py": "build/mem_tac60.bin", "fade_diff.py": "build/mem_tac60.bin", "loadgame_diff.py": "build/mem_tac60.bin", "ctlshell_diff.py": "build/mem_tac60.bin"}
# second runs of a test with other arguments: (label, script, args)
EXTRA = [("compose_diff.py --flight", "compose_diff.py", ["--flight", "20", "build/mem_combat1.bin", "build/mem_death1.bin",
                                                         "build/mem_fight20.bin", "build/mem_f3.bin", "build/mem_fight7.bin"]),
         ("compose_diff.py --turn", "compose_diff.py", ["--turn", "40", "build/mem_combat1.bin", "build/mem_death1.bin",
                                                       "build/mem_tac60.bin"]),
         ("compose_diff.py --melee", "compose_diff.py", ["--melee", "40", "build/mem_combat1.bin", "build/mem_death1.bin",
                                                        "build/mem_fight7.bin"]),
         ("compose_diff.py --blast", "compose_diff.py", ["--blast", "30", "build/mem_combat1.bin", "build/mem_death1.bin",
                                                        "build/mem_fight7.bin"]),
         ("compose_diff.py --newturn", "compose_diff.py", ["--newturn", "15", "build/mem_combat1.bin", "build/mem_death1.bin",
                                                          "build/mem_f3.bin"]),
         ("compose_diff.py --dies", "compose_diff.py", ["--dies", "40", "build/mem_combat1.bin", "build/mem_death1.bin",
                                                       "build/mem_a10.bin"]),
         # the clean-room port (port/ja1logic, built first) against the original
         ("path_diff.py --port", "path_diff.py", ["300", "--port"]),
         ("move_diff.py --port", "move_diff.py", ["2000", "--port"]),
         ("damage_diff.py --port", "damage_diff.py", ["1500", "--port"]),
         ("soldier_diff.py --port", "soldier_diff.py", ["100", "build/mem_tac60.bin", "build/mem_combat1.bin",
                                                        "build/mem_death1.bin", "--port"]),
         ("ai_diff.py --port", "ai_diff.py", ["--port"]),
         ("ai_diff.py --port use_on_tile", "ai_diff.py", ["--port", "--only", "use_on_tile", "3000"]),   # combat.md Using the hand item on a tile
         ("ai_diff.py --port bury_mine", "ai_diff.py", ["--port", "--only", "bury_mine", "2000"]),   # combat.md Burying a mine
         ("ai_diff.py --port revive_roll", "ai_diff.py", ["--port", "--only", "revive_roll", "3000"]),   # combat.md Getting up after a collapse
         ("ai_diff.py --port pickup_here", "ai_diff.py", ["--port", "--only", "pickup_here", "2000"]),   # combat.md Picking up at the own tile
         ("ai_diff.py --port land_item", "ai_diff.py", ["--port", "--only", "land_item", "2000"]),   # combat.md A thrown item lands
         ("ai_diff.py --port lock_kit", "ai_diff.py", ["--port", "--only", "lock_kit", "2000"]),   # combat.md Using the lock kit on the mission safe
         ("ai_diff.py --port mine_flag", "ai_diff.py", ["--port", "--only", "mine_flag", "2000"]),   # combat.md A detected mine gets a blue flag
         ("ai_diff.py --port walter", "ai_diff.py", ["--port", "--only", "walter", "8000"]),   # combat.md The listening device at a door
         ("ai_diff.py --port throw", "ai_diff.py", ["--port", "--only", "throw_option", "1500"]),   # throw option: a fault in the aim limit shows ~1 in 400 cases, the grouped run gives ~100 (Insert #12214)
         ("ai_diff.py --port spend_ap", "ai_diff.py", ["--port", "--only", "spend_ap", "2400"]),   # fatigue scale: slot 40 with a guide fatigue value shows ~1 in 200 cases, the grouped run gives ~110 (Insert #12214)
         ("ai_diff.py --port active_frame", "ai_diff.py", ["--port", "--only", "active_frame", "3000"]),   # the energy cost of each move code at a step end (a fault shows ~1 in 80 cases; Insert #12214)
         ("ai_diff.py --port alert_action", "ai_diff.py", ["--port", "--only", "alert_action", "4000"]),   # the mood terms of the walk to a noise, a shout and an ally call (Insert #12214)
         ("ai_diff.py --port click", "ai_diff.py", ["--port", "--only", "click", "3000"]),   # tactical click: the ground index of the tile (a fault shows ~1 in 3000 cases; Insert #12214)
         ("ai_diff.py --port impact", "ai_diff.py", ["--port", "--only", "impact", "3000"]),   # shooting.md Projectile impact: a thrown explosive that lands, then goes off (impact_effect)
         ("ai_diff.py --port soldier_frame", "ai_diff.py", ["--port", "--only", "soldier_frame", "4000"]),
         ("ai_diff.py --port decide", "ai_diff.py", ["--port", "--only", "engaged_action,combat_action,alert_action,calm_action,think", "10000"]),   # the AI decision layer, 2000 cases each (the grouped run gives ~100: Santino's speech, deep-water retreat ran 0 times; ja1-cov 2026-11-06)
         ("ai_diff.py --port sight", "ai_diff.py", ["--port", "--only", "sight_check,mark_seen", "6000"]),   # sight check: a lost sighting of a remembered (0xFF) opponent comes ~1 in 500 cases; the grouped run gives ~90 (ja1-cov 2026-11-06)
         ("ai_diff.py --port turn", "ai_diff.py", ["--port", "--only", "turn_to,anim_turn,start_turn", "6000"]),   # a turn with exactly 1 AP left, the turn end of slot 16, the story sector view at a turn start: each ~1 in 100..200 cases, the grouped run gives ~95 (Insert #12271)
         ("ai_diff.py --port anim", "ai_diff.py", ["--port", "--only", "set_anim,anim_step,anim_frame", "9000"]),   # animation start chains (walk from a still pose), the step + frame scripts: the grouped run gives each ~100 cases, too few (ja1-cov 2026-11-06)   # soldier frame: the move to the waypoint, covered check (ja1-cov 2026-11-06)
         ("world_diff.py --port", "world_diff.py", ["--port"]),
         ("quicksave_diff.py", "quicksave_diff.py", ["build/mem_tac60.bin", "build/mem_combat1.bin", "build/mem_death1.bin",
                                                     "build/mem_fight20.bin", "build/mem_f3.bin"]),   # QuickSave 0x2D880 vs quicksave.rs
         ("quickrestore_diff.py", "quickrestore_diff.py", ["build/QUICK_t10.SAV", "build/mem_tac60.bin",
                                                          "build/QUICK_t2.SAV", "build/mem_combat1.bin",
                                                          "build/QUICK_s59start.SAV", "build/mem_fight20.bin"]),   # QuickRestore 0x2C898 vs quicksave.rs
         ("walk_diff.py --combat", "walk_diff.py", ["--combat"]),      # walk in combat, End turn, new round (turns.md)
         ("walk_diff.py --shoot", "walk_diff.py", ["--shoot", "40", "--frames", "120"]),   # a merc shoots an enemy (shooting.md)
         ("walk_diff.py --enemy", "walk_diff.py", ["--enemy", "20", "--frames", "300"]),   # the enemies' turn: the AI walks, aims, shoots
         ("walk_diff.py --enemy fight20", "walk_diff.py", ["--enemy", "10", "--frames", "300", "build/mem_fight20.bin"]),
         ("walk_diff.py --combat f1", "walk_diff.py", ["--combat", "20", "--frames", "120", "build/mem_f1.bin"]),   # enemies attack in the turn after End turn (JamCheck runs on both sides)
         ("walk_diff.py --enemy f5", "walk_diff.py", ["--enemy", "6", "--frames", "300", "build/mem_f5.bin"]),   # draws past the scripted Random list (CalmAction facing loop)
         ("walk_diff.py --shoot a1", "walk_diff.py", ["--shoot", "40", "--frames", "120", "build/mem_a1.bin"]),   # melee blows: MeleeHit 0x3521C (op 77) runs on both sides
         ("walk_diff.py --cross", "walk_diff.py", ["--cross", "20", "--frames", "400"]),   # two walkers to one place: blocked steps (op 98), stops short
         ("walk_diff.py --water w3", "walk_diff.py", ["--water", "150", "--frames", "300", "build/mem_w3.bin"]),   # a walk out of deep water: SoakItems 0x407B8 runs on both sides; tired swimmers drown on a step (Drown? 0x3843C: message, damage, death)
         ("walk_diff.py --water clk1", "walk_diff.py", ["--water", "20", "--frames", "300", "build/mem_clk1.bin"]),   # a tired walker steps into water: Drown? 0x3843C (step path) on both sides
         ("walk_diff.py --water --combat w3", "walk_diff.py", ["--water", "--combat", "20", "--frames", "200", "build/mem_w3.bin"]),   # swimmers in turn-based combat: drowning at the start of a turn (StartTurn -> Drown?)
         ("walk_diff.py --mine", "walk_diff.py", ["--mine", "100", "--frames", "400"]),   # mines near the walk: a merc who sees the mine stops and flags it (step start step 6); the forgetful merc stop (step 7)
        ("walk_diff.py --mine --combat", "walk_diff.py", ["--mine", "--combat", "40", "--frames", "400"]),   # the forgetful merc's stop in combat takes Random(AP) + 1 AP (step start step 7)
        ("walk_diff.py --mine clk1", "walk_diff.py", ["--mine", "20", "--frames", "400", "build/mem_clk1.bin"]),   # a mine's timed effect (EffectStart 0x157BC = op 91) on both sides
         ("walk_diff.py --cross aim_pre", "walk_diff.py", ["--cross", "30", "--frames", "600", "build/mem_aim_pre.bin"]),   # a walker waits 50 frames for another, then stops
         ("walk_diff.py --cross t12", "walk_diff.py", ["--cross", "10", "--frames", "400", "build/mem_t12.bin"]),   # an enemy shot in real time
         ("walk_diff.py --fight", "walk_diff.py", ["--fight", "1", "--frames", "60000", "--rounds", "200", "--to-end"]),   # a whole fight to the end (28 rounds with interrupts, ops 128 / 116)
         ("flight_diff.py --port", "flight_diff.py", ["5000", "--port"]),
         ("panel_diff.py --port", "panel_diff.py", ["600", "--port"]),          # merc panel slots (panel.md)
         ("panel_diff.py --port --switch", "panel_diff.py", ["300", "--port", "--switch"]),   # switch view (panel.md)
         ("panel_data_diff.py --port", "panel_data_diff.py", ["--port"]),       # face buffers + eye rows (panel.md)   # Hit test (shooting.md batch 5)
         ("inv_diff.py --port", "inv_diff.py", ["2000", "--port"]),            # inventory screen (inventory.md)
         ("inv_diff.py --port --strip", "inv_diff.py", ["1000", "--port", "--strip"]),   # bottom strip clicks (panel.md)
         ("popup_diff.py --port", "popup_diff.py", ["200", "--port"]),         # merc stat popup (panel.md)
         ("rect_diff.py", "rect_diff.py", ["2000"]),                          # rect visible (shooting.md Explode)
         ("rng_diff.py --port", "rng_diff.py", ["3000", "--port"]),           # Random 0x53538 over the Watcom seed (front-end Rng)
         ("invdraw_diff.py --port", "invdraw_diff.py", ["300", "--port"]),   # inventory screen drawing (inventory.md)
         ("payday_diff.py --port", "payday_diff.py", ["1000", "--port"]),     # "not enough funds" dialog (strategic.md)
         ("duties_diff.py --port", "duties_diff.py", ["3000", "--port"]),     # morning duties (strategic.md)
         ("xp_diff.py --port", "xp_diff.py", ["500", "--port"]),             # morning stat changes (strategic.md)
         ("morning_diff.py --port", "morning_diff.py", ["2000", "--port"]),  # back on duty, profile sync (strategic.md)
         ("complain_diff.py --port", "complain_diff.py", ["2000", "--port"]),  # AIM rate complaints, disliked merc
         ("corpse_diff.py --port", "corpse_diff.py", ["1000", "--port"]),   # corpse export (strategic.md)
         ("raise_diff.py --port", "raise_diff.py", ["1000", "--port"]),     # salary raise (strategic.md)
         ("mapopen_diff.py --port", "mapopen_diff.py", ["40", "--port"]),   # InitStrategicMap 0x43ECC: the whole opening picture
         ("island_diff.py --port", "island_diff.py", ["300", "--port"]),   # the island map picture (strategic.md)
         ("replaypic_diff.py --port", "replaypic_diff.py", ["600", "--port"]),   # the replay map pictures (strategic.md)
         ("map_diff.py --port", "map_diff.py", ["3000", "--port"]),       # sector click, start checks (strategic.md)
         ("strat_input_diff.py --port", "strat_input_diff.py", ["200", "--port"]),   # strategic screen input loop (strategic.md)
         ("day_diff.py --port", "day_diff.py", ["3000", "--port"]),       # day start (strategic.md)
         ("sunrise_diff.py --port", "sunrise_diff.py", ["2000", "--port"]),   # sunrise (strategic.md)
         ("stratopen_diff.py --port", "stratopen_diff.py", ["1000", "--port"]),   # strategic screen opening (strategic.md)
         ("storyday_diff.py --port", "storyday_diff.py", ["3000", "--port"]),   # story events at the day start (strategic.md)
         ("clock_diff.py --port", "clock_diff.py", ["3000", "--port"]),     # the day clock, time compression (turns.md)
         ("guard_diff.py --port", "guard_diff.py", ["3000", "--port"]),     # the guard screen (strategic.md)
         ("guide_diff.py --port", "guide_diff.py", ["3000", "--port"]),     # the guide's advice (strategic.md)
         ("verdict_diff.py --port", "verdict_diff.py", ["3000", "--port"]),   # the day's verdict (strategic.md)
         ("rating_diff.py --port", "rating_diff.py", ["2000", "--port"]),   # the performance rating (ending.md)
         ("trav_diff.py --port", "trav_diff.py", ["3000", "--port"]),   # the traverse (soldier_update.md)
         ("stage_diff.py --port", "stage_diff.py", ["3000", "--port"]),   # timed effects + home-sector stages (turns.md)
         ("battle_diff.py --port", "battle_diff.py", ["2000", "--port"]),   # sector battles (strategic.md)
         ("practice_diff.py --port", "practice_diff.py", ["3000", "--port"]),   # stat practice (combat.md)
         ("enemy_diff.py --port", "enemy_diff.py", ["3000", "--port"]),     # enemy moves (strategic.md)
         ("strips_diff.py --port", "strips_diff.py", ["500", "--port"]),   # the interface strips (panel.md)
         ("mappanel_diff.py --port", "mappanel_diff.py", ["300", "--port"]),   # the map panel (strategic.md)
         ("msgbox_diff.py --port", "msgbox_diff.py", ["1000", "--port"]),   # the message box (panel.md)
         ("balloon_diff.py --port", "balloon_diff.py", ["200", "--port"]),   # the speech balloon (panel.md)
         ("tilepopup_diff.py --port", "tilepopup_diff.py", ["200", "--port"]),   # the tile popups (cursor.md)
         ("aimstats_diff.py --port", "aimstats_diff.py", ["300", "--port"]),   # the A.I.M. statistics page (strategic.md)
         ("salary_diff.py --port", "salary_diff.py", ["200", "--port"]),   # the A.I.M. salary panel (strategic.md)
         ("itemvoice_diff.py --port", "itemvoice_diff.py", ["1500", "--port"]),   # the item's voice (panel.md)
         ("raisebox_diff.py --port", "raisebox_diff.py", ["300", "--port"]),   # the salary box of the raise (strategic.md)
         ("loadface_diff.py --port", "loadface_diff.py", ["400", "--port"]),   # a merc face into a slot's face buffer (panel.md)
         ("loadpopup_diff.py --port", "loadpopup_diff.py", ["16", "--port"]),   # the drown / injection popup frames (panel.md)
         ("arcseek_diff.py", "arcseek_diff.py", ["2000"]),   # ArcSeek 0x534A4: the member lookup of every archive read
         ("startworld_diff.py", "startworld_diff.py", []),   # InitProgramWorld? 0x377B0 + SrandTime? 0x53564
         ("stashclear_diff.py --port", "stashclear_diff.py", ["300", "--port"]),   # the item store emptied at a load (strategic.md)
         ("tilepal_diff.py --port", "tilepal_diff.py", ["--port"]),   # the tileset palettes (world_load.md)
         ("sanim_diff.py --port", "sanim_diff.py", ["--port"]),   # the travel animation rows (panel.md)
         ("question_diff.py --port", "question_diff.py", ["300", "--port"]),   # the question box (panel.md)
         ("question_diff.py --mouse --port", "question_diff.py", ["200", "--mouse", "--port"]),   # YesNoPrompt? mouse move
         ("question_diff.py --face --port", "question_diff.py", ["150", "--face", "--port"]),   # the merc-face question box
         ("iteminfo_diff.py --port", "iteminfo_diff.py", ["400", "--port"]),   # the item info box (panel.md)
         ("balloonview_diff.py", "balloonview_diff.py", ["1500"]),   # the view move at the end of a merc message
         ("focus_diff.py --port", "focus_diff.py", ["3000", "--port"]),   # host op 99 focus (world_load.md section 8)
         ("onscreen_diff.py --port", "onscreen_diff.py", ["3000", "--port"]),   # OnScreen 0x1FC34, the on-screen test (select.md)
         ("ovmouse_diff.py --port", "ovmouse_diff.py", ["3000", "--port"]),   # overview mouse step 0x24630 (panel.md)
         ("keyhandler_diff.py --port", "keyhandler_diff.py", ["6000", "--port"]),   # key part of 0x13490 (tactkeys.rs)
         ("viewmode_diff.py --port", "viewmode_diff.py", ["4000", "--port"]),   # view modes 0x248E4 + keys (panel.md View modes)
         ("assign_diff.py --port", "assign_diff.py", ["300", "--port"]),   # the assignment picture (strategic.md)
         ("payroll_diff.py --port", "payroll_diff.py", ["200", "--port"]),   # the payroll screen (strategic.md)
         ("xpcount_diff.py --port", "xpcount_diff.py", ["120", "--port"]),   # the experience count (strategic.md)
         ("office_diff.py --port", "office_diff.py", ["300", "--port"]),   # the office picture (strategic.md The office)
         ("ledger_diff.py --port", "ledger_diff.py", ["300", "--port"]),   # the team ledger (strategic.md The team ledger)
         ("officehl_diff.py", "officehl_diff.py", ["60"]),   # the office highlight 0x4C0C8 on hot spots 1..7
         ("switchbox_diff.py --port", "switchbox_diff.py", ["300", "--port"]),   # the switch box (strategic.md The switch box (Control Panel))
         ("ctlpanel_diff.py --port", "ctlpanel_diff.py", ["100", "--port"]),   # the tactical Control Panel's opening picture (panel.md)
         ("cursormode_diff.py --port", "cursormode_diff.py", ["300", "--port"]),   # CursorModeSet 0x1C438 (cursor.md "Mode cursor")
         ("battlebox_diff.py --port", "battlebox_diff.py", ["300", "--port"]),   # the battle box (strategic.md The battle box)
         ("dayreplay_diff.py --port", "dayreplay_diff.py", ["1000", "--port"]),   # the day replay (strategic.md The day replay)
         ("gameover_diff.py --port", "gameover_diff.py", ["--port"]),   # the game-over screen (ending.md)
         ("sab_diff.py --port", "sab_diff.py", ["--port"]),   # the SAB end scripts (ending.md)
         ("sab_diff.py --port --subtitles", "sab_diff.py", ["--port", "--subtitles"]),   # the same with Subtitles on (op 29 texts)
         ("sab_diff.py --port intro", "sab_diff.py", ["INTRO\\SAT.DAT", "ISLAND.SAB", "--port", "--subtitles"]),   # the new-game intro (ending.md "The new-game intro")
         ("sab_diff.py --port --op35", "sab_diff.py", ["--port", "--op35"]),   # SAB op 0x35 (MovePaletteEntry 0x528DC + PaletteSet of the DAC copy)
         ("tmpsave_diff.py --port", "tmpsave_diff.py", ["--port"]),   # the sector tmp file (op 219 save_tmp)
         ("flc_diff.py --port", "flc_diff.py", ["--port"]),   # the FLC film decoder, all 138 films
         ("confirmclick_diff.py", "confirmclick_diff.py", []),   # the second click of a confirm: answer, cursor tile, item timer
         ("slotheaders_diff.py", "slotheaders_diff.py", []),   # the save list headers: count, days, names
         ("teamquote_diff.py", "teamquote_diff.py", []),   # the team quote: which mercs speak, order, seed
         ("smallrows_diff.py", "smallrows_diff.py", ["500"]),   # small functions, see its head
         ("talkbox_diff.py", "talkbox_diff.py", ["60"]),   # a merc talks, 0x50908: balloon or text box
         ("puff_diff.py", "puff_diff.py", ["500"]),   # 0x156B8 a free effect record, play::puff
         ("mouseshape_diff.py", "mouseshape_diff.py", []),     # SetMouseShape? 0x674B0 + draw, turn mouse 0x67E18 / 0x67E30 / 0x681D0
         ("pointer_diff.py", "pointer_diff.py", ["600"]),         # top-row rule of ShowMouse? 0x673AC + MousePoll? 0x678E8
         ("pressbtn_diff.py", "pressbtn_diff.py", ["1000"]),      # the hold of a pressed button: 0x35D44, 0x35ED4, mouse back at the release
         ("mouselimit_diff.py", "mouselimit_diff.py", ["400"]),   # SetMouseLimit? 0x682AC with the driver call stubbed
         ("facelight_diff.py", "facelight_diff.py", ["300"]),   # the light frame of a merc face: 0x46A4C, buffers after a load
         ("stripbuttons_diff.py", "stripbuttons_diff.py", []),   # the top strip buttons up and shaded: panel, screen, table
         ("invextra_diff.py", "invextra_diff.py", []),   # an inventory remark: sound, box wait, sound wait, keys left
         ("cursorback_diff.py", "cursorback_diff.py", []),   # the cursor item back to its strip box
         ("guardopen_diff.py", "guardopen_diff.py", []),   # key M: the way to the guard screen and back
         ("aimtalk_diff.py", "aimtalk_diff.py", []),   # the A.I.M. quote box: box or no box, face, point, time
         ("aimquote_diff.py", "aimquote_diff.py", []),   # the text of an A.I.M. refusal box: StrReplace bug, g_textBuf, last quote
         ("raiseask_diff.py", "raiseask_diff.py", []),   # the quote boxes of a salary raise: reason, grudge name, answers, buffers
         ("exitdos_diff.py", "exitdos_diff.py", []),   # Exit to DOS: box, ends or not, Quick Save, files, quit line, seed
         ("continue_diff.py", "continue_diff.py", []),   # the prompt at the program start: answer, steps, palette, boxes, files
         ("officeinit_diff.py", "officeinit_diff.py", []),   # the office entry: pixels, palette, theme, call order
         ("mainmenu_diff.py", "mainmenu_diff.py", []),   # the main menu picture: pixels, palettes, mouse place
         ("offsetbank_diff.py", "offsetbank_diff.py", []),   # the picture bank loader + tile bank name and size rule
         ("complainquote_diff.py", "complainquote_diff.py", []),   # a merc's morning quotes: boxes, name, last quote, buffers
         ("flcplay_diff.py", "flcplay_diff.py", []),   # the film player: frame order, passes, ring frame, keys, cue index
         ("volume_diff.py --port", "volume_diff.py", ["4000", "--port"]),   # the volume keys + - . , (TacticalKeyHandler)
        ("scroll_diff.py --port", "scroll_diff.py", ["4000", "--port"]),   # the arrow-key view scroll (TacticalKeyHandler + 0x1EDAC)
        ("wipe_diff.py --port", "wipe_diff.py", ["60", "--port"]),   # the view wipes of NewTurn? (ops 164 / 165: 0x124FC / 0x1259C)
         ("credits_diff.py --port", "credits_diff.py", ["--port"]),   # the credits after the win (ending.md)
         ("credits_diff.py --title --port", "credits_diff.py", ["--title", "--port"]),   # the title part at the program start
         ("slots_diff.py --port", "slots_diff.py", ["300", "--port"]),     # the save / restore slot list (strategic.md)
         ("newgame_diff.py --port", "newgame_diff.py", ["30", "--port"]),  # a new game (world_load.md section 11)
         ("aim_diff.py --port", "aim_diff.py", ["1200", "--port"]),  # strategic.md The A.I.M. screen (refusal, hire, cancel, browse)
         ("duty_diff.py --port", "duty_diff.py", ["300", "--port"]),   # the duty menu (strategic.md)
         ("select_diff.py --port", "select_diff.py", ["300", "--port"]),   # merc selection, item panel (strategic.md)
         ("assign_panel_diff.py --port", "assign_panel_diff.py", ["500", "--port"]),   # panel buttons (strategic.md)
         ("assign_hover_diff.py --port", "assign_hover_diff.py", ["300", "--port"]),   # hover + tooltip box (strategic.md, panel.md)
         ("assign_items_diff.py --port", "assign_items_diff.py", ["1000", "--port"]),   # item moves on the assignment screen (strategic.md)
         # more cases than the split run gives these sections: the difficulty bonus and each word of the rest table
         # decide only a few cases (Insert #11820: ledger rows ja1logic:9946, ja1logic:8501).
         ("ai_diff.py --port contest", "ai_diff.py", ["--port", "--only", "merc_interrupt,enemy_interrupt,turn_rest", "18000"]),
         ("sight_diff.py", "sight_diff.py", ["3000"]),                       # sight line, library op 49 (ai.md)
         ("pure_diff.py --port", "pure_diff.py", ["20000", "--port"]),
         ("compose_diff.py --port", "compose_diff.py", ["--port", "25", "build/mem_a10.bin", "build/mem_a35.bin",
                                                       "build/mem_combat1.bin", "build/mem_death1.bin", "build/mem_f3.bin",
                                                       "build/mem_fight20.bin", "build/mem_tac60.bin"]),   # AI decision layer (9 entry points) + move-code rebuild
         ("compose_diff.py --port --windows", "compose_diff.py", ["--port", "--windows", "60", "build/mem_a39.bin",
                                                                 "build/mem_adv.bin", "build/mem_a29.bin", "build/mem_a45.bin",
                                                                 "build/mem_fight21.bin"])]   # enemy line of fire across window walls
CARGO = os.path.join(os.path.expanduser("~"), ".cargo", "bin", "cargo")
PORT_CRATES = ("ja1logic", "ja1front")


def labels_check():
    rows = [r for r in csv.reader(open(os.path.join(ROOT, "labels.csv"))) if r and r[0] != "addr" and not r[0].startswith("#")]
    errs = []
    for r in rows:
        try:
            parse_addr(r[0])
        except ValueError as e:
            errs.append(f"bad address {r[0]}: {e}")
    for what, col in (("address", 0), ("name", 1)):
        dup = [k for k, n in Counter(r[col].upper() if col == 0 else r[col] for r in rows).items() if n > 1]
        if dup:
            errs.append(f"duplicate {what}s: {', '.join(dup)}")
    return errs


def dict_keys_check():
    """duplicate keys in a dict literal of the diff harnesses silently replace a stub (0x15260 once hid 0x3D750)"""
    import ast
    errs = []
    for f in sorted(glob.glob(os.path.join(ROOT, "validate", "diff", "*.py"))) + [os.path.join(ROOT, "tools", "ai.py")]:
        for node in ast.walk(ast.parse(open(f).read())):
            if isinstance(node, ast.Dict):
                keys = [k.value for k in node.keys if isinstance(k, ast.Constant)]
                dup = sorted({k for k in keys if keys.count(k) > 1}, key=str)
                if dup:
                    errs.append(f"{os.path.basename(f)}:{node.lineno}: duplicate keys {dup}")
    return errs


# a long test split into N parallel jobs ("LABEL K/N", script option --group K/N); same sections and case counts
SPLIT = {"ai_diff.py": 4, "ai_diff.py --port": 4}   # (smoke_play: the play-test steps over 4 jobs, --group K/4)
# gate_speed's parts as their own jobs (Insert #11198 step 4): each part is a window on its own display; the parts run
# side by side today, so the wall time is the longest part. The new-game intro part (~290 s) is out of the check
# (the maintainer 2026-11-02: most of the game needs the test time, the intro can be fixed last if it ever breaks):
# run it by hand with validate/gate_speed.py intro.
GATE_SPEED_PARTS = ["day", "walk", "slide", "msgbox"]
# minimum case count per section (the script's "CASES section=N" lines, summed over a split test's jobs), about 80 %
# of the counts on 2026-10-25: a section that is missing or below its floor fails the check. Never lower a floor to
# make a check green (write the section, old and new floor and why in the sticky update). A family with no CASES line
# in a run fails too, unless that run recorded a skip for that family's label (Insert #11623). A rename of a test in
# the check renames its CASE_FLOORS key in the same commit.
CASE_FLOORS = {
    "font_diff.py": {"font": 2000},
    "aimleave_diff.py": {"aimleave": 1600},
    "slots_diff.py --port": {"slots": 280},
    "newgame_diff.py --port": {"newgame": 30},
    "aim_diff.py --port": {"aim": 1200},
    "ai_diff.py --port use_on_tile": {"use_on_tile": 2400},
    "ai_diff.py --port soldier_frame": {"soldier_frame": 3200},
    "ai_diff.py --port decide": {"engaged_action": 1600, "combat_action": 1600, "alert_action": 1600, "calm_action": 1600, "think": 1600},
    "ai_diff.py --port sight": {"sight_check": 2400, "mark_seen": 2400},
    "ai_diff.py --port turn": {"turn_to": 1600, "anim_turn": 1600, "start_turn": 1600},
    "ai_diff.py --port anim": {"set_anim": 2400, "anim_step": 2400, "anim_frame": 2400},
    "ai_diff.py --port bury_mine": {"bury_mine": 1600},
    "ai_diff.py --port pickup_here": {"pickup_here": 1600},
    "ai_diff.py --port land_item": {"land_item": 1600},
    "ai_diff.py --port lock_kit": {"lock_kit": 1600},
    "ai_diff.py --port mine_flag": {"mine_flag": 1600},
    "ai_diff.py --port walter": {"walter": 6400},
    "ai_diff.py --port throw": {"throw_option": 1200},
    "ai_diff.py --port active_frame": {"active_frame": 2400},
    "ai_diff.py --port alert_action": {"alert_action": 3200},
    "ai_diff.py --port click": {"click": 2400},
    "ai_diff.py --port impact": {"impact": 2400},
    "ai_diff.py --port contest": {"merc_interrupt": 4800, "enemy_interrupt": 4800, "turn_rest": 4800},
    "ai_diff.py --port": {
        "think": 95, "update_alert": 95, "choose_action": 95, "calm_action": 95, "alert_action": 95,
        "combat_action": 95, "engaged_action": 95, "action_affordable": 95, "do_action": 95, "noise_tile": 95,
        "find_dry_tile": 95, "patrol_tile": 95, "buddy_tile": 95, "news_value": 95, "approach_tile": 95,
        "ally_call_tile": 95, "can_stand_at": 95, "advance_tile": 95, "target_tile": 95, "opp_strength": 95,
        "soldier_power": 95, "retreat_tile": 95, "search_tile": 95, "gun_option": 95, "knife_option": 95,
        "throw_option": 94, "line_of_fire": 95, "anim_delay": 95, "anim_step": 95, "anim_end": 95, "set_anim": 95,
        "anim_frame": 95, "anim_turn": 95, "xra_enemy": 95, "replace": 95, "reserve_ap": 95, "soldier_frame": 95,
        "active_frame": 95, "walk_step": 95, "walk_blocked": 95, "walk_end": 95, "stand_up": 95, "frame_checks": 95,
        "merc_quote": 95, "enemies_see": 95, "mercs_see": 95, "step_noise": 95, "consume_trigger": 95,
        "throw_follow": 94, "throw_aim": 95, "throw_scatter": 95, "find_path": 95, "path_ap": 95, "walk_to": 95,
        "idle": 95, "stop_anim": 95, "start_turn": 95, "bleed": 95, "drown": 95, "turn_rest": 95, "aid_tick": 95,
        "end_task": 95, "move_ap_cost": 95, "w_59ae8": 95, "swap_hand": 95, "busy": 95, "turn_to": 95,
        "first_aid": 95, "spend_ap": 95, "rest": 95, "launch": 95, "pstep": 95, "fire": 95, "w_56dd4": 95,
        "impact": 95, "take_damage": 95, "hit_reaction": 95, "hit_alert": 95, "mark_seen": 95, "shot_alert": 95,
        "interrupt_check": 95, "merc_interrupt": 95, "enemy_interrupt": 94, "kill": 94, "merc_dies": 94,
        "start_attack": 94, "attack_go": 94, "fire_gun": 94, "chance_to_hit": 94, "click": 1870, "attack_ready": 94,
        "reach_tile": 94, "gun_jam": 94, "use_on_tile": 92, "w_59a54": 94, "hears": 94, "noise_int": 94, "sight_check": 94,
        "update_sightings": 94, "share_sightings": 94, "enemy_turned": 94, "guard_turned": 94, "sector_value": 4800,
        "make_noise": 3200, "hide_path_marks": 2400, "blast_tile": 3200, "blast": 1200, "sector_tick": 2400,
        "gas_tick": 480, "connected": 4800, "ledger": 4800, "pay": 2400, "pickup": 2936, "disarm": 3200, "mine_spotted": 1600, "merc_entry": 1600,
        "slot_reset": 1600, "sector_start": 144, "tapper": 533, "guard": 533, "guard_tile": 532, "equip": 1600,
        "init_soldier": 1600, "edge_tile": 267, "guard_enter": 266, "enemy_walk_in": 266, "crate_drops": 320,
        "crate_lost": 320, "fixed_items": 320, "plant_sector": 320, "plant_done": 320, "tile_patch": 800,
        "item_patch": 800, "ground_blast": 1600, "tmp_file": 160, "shot_ap": 2400, "action_ap": 2400,
        "quest_check": 1600, "radio_result": 1600, "view": 3200, "move": 3200, "loop": 3200, "loop_rest": 1600,
        "kept_turn": 600, "sector_won": 1600, "fuses": 1600, "loop_head": 1600, "leave_tests": 800, "ticks": 3200, "rebuild": 16, "doors": 800,
        "object_surface": 204, "put_item": 3200, "effect_hit": 2400, "itq_pop": 1067, "next_turn": 1066,
        "interrupt_turn": 1066, "turn_start": 1600, "ally_alert": 1600, "escape_stop": 2400, "select_merc": 1600,
        "select_soldier": 1600, "first_merc": 800, "turn_sightings": 800, "enemies_turn": 800, "end_turn": 800,
        "mercs_turn": 800, "new_turn": 2400, "explode": 2400, "door_fx": 3200, "reload": 1600, "leave_sector": 1600,
        "leave_path": 400, "end_of_day": 160, "explode_dat": 9600},
    "ai_diff.py": {
        "update_alert": 83, "calm_action": 83, "alert_action": 83, "combat_action": 83, "engaged_action": 83,
        "choose_action": 83, "action_affordable": 83, "do_action": 83, "think": 83, "opp_strength": 83,
        "soldier_power": 83, "target_tile": 83, "update_sightings": 83, "move_ap_cost": 83, "ally_call_tile": 83,
        "news_value": 83, "noise_tile": 83, "patrol_tile": 83, "approach_tile": 83, "advance_tile": 83,
        "buddy_tile": 83, "can_stand_at": 83, "find_dry_tile": 83, "retreat_tile": 83, "search_tile": 83,
        "gun_option": 83, "knife_option": 83, "throw_option": 81, "line_of_fire": 83, "sight_check": 83,
        "mark_seen": 83, "no_ally_sees": 83, "share_sightings": 83, "idle": 83, "enemy_turned": 83,
        "guard_turned": 83, "reach_tile": 83, "attack_ready": 83, "turn_to": 83, "walk_to": 83, "anim_step": 83,
        "anim_end": 83, "set_anim": 83, "anim_delay": 83, "msg_not_enough_ap": 83, "anim_frame": 83, "anim_turn": 83,
        "end_task": 83, "replace": 83, "xra_enemy": 83, "stop_anim": 83, "w_197d8": 83, "stop_walk": 82,
        "walk_step": 82, "walk_blocked": 82, "stand_up": 82, "walk_end": 82, "enemies_see": 82, "mercs_see": 82,
        "step_noise": 82, "nearest_merc": 82, "frame_checks": 82, "merc_quote": 82, "soldier_frame": 82,
        "active_frame": 82, "consume_trigger": 82, "throw_scatter": 82, "throw_aim": 81, "throw_follow": 82,
        "find_path": 82, "path_ap": 82, "click": 1650, "w_59a54": 82, "w_5aa08": 82, "w_59ae8": 82, "fire": 82,
        "launch": 82, "w_56dd4": 82, "pstep": 82, "impact": 82, "take_damage": 82, "hit_reaction": 82,
        "hit_alert": 82, "interrupt_check": 82, "hears": 82, "rest": 82, "land_item": 82, "revive_roll": 82,
        "melee_hit": 82, "kill": 82, "merc_dies": 82, "noise_int": 82, "shot_alert": 82, "trigger": 82,
        "reserve_ap": 82, "gun_jam": 82, "bury_mine": 82, "pickup_here": 82, "use_on_tile": 82, "start_attack": 82,
        "fire_gun": 82, "chance_to_hit": 82, "attack_go": 82, "spend_ap": 82, "stat_practice": 82, "turn_rest": 82,
        "aid_tick": 82, "apply_aid": 82, "first_aid": 82, "start_turn": 82, "bleed": 82, "drown": 82,
        "merc_interrupt": 82, "enemy_interrupt": 82, "swap_hand": 82, "busy": 82, "tile_standable": 2400,
        "sector_value": 4800, "sight_line": 3200, "throw_chance": 2400, "tile_distance": 1600, "make_noise": 3200,
        "hide_path_marks": 2400, "blast_tile": 3200, "blast": 1200, "sector_tick": 2400, "gas_tick": 480,
        "workable": 3200, "connected": 4800, "ledger": 4800, "pay": 2400, "pickup": 3080, "stage": 1600, "age": 1600,
        "story": 800, "forget": 1600, "stragglers": 1600, "crowbar": 3200, "disarm": 3200, "mine_spotted": 1600, "merc_entry": 1599,
        "slot_reset": 1600, "sector_start": 136, "tapper": 533, "guard": 533, "guard_tile": 532, "equip": 1600,
        "init_soldier": 1600, "edge_tile": 267, "guard_enter": 266, "enemy_walk_in": 266, "crate_drops": 320,
        "crate_lost": 320, "fixed_items": 320, "plant_sector": 320, "plant_done": 320, "tile_patch": 800,
        "item_patch": 800, "ground_blast": 1600, "tmp_file": 160, "shot_ap": 2400, "action_ap": 2400,
        "quest_check": 1600, "radio_result": 1600, "view": 3200, "move": 3200, "loop": 3200, "loop_rest": 1600,
        "fuses": 1600, "loop_head": 1600, "leave_tests": 800, "ticks": 3200, "rebuild": 16, "doors": 800,
        "object_surface": 204, "put_item": 3200, "effect_hit": 2400, "itq_pop": 1067, "next_turn": 1066,
        "interrupt_turn": 1066, "turn_start": 1600, "ally_alert": 1600, "escape_stop": 2400, "select_merc": 1600,
        "select_soldier": 1600, "first_merc": 800, "turn_sightings": 800, "enemies_turn": 800, "end_turn": 800,
        "mercs_turn": 800, "new_turn": 2400, "use_throw": 2400, "explode": 2400, "door_fx": 3200, "reload": 1600,
        "leave_sector": 1600, "leave_path": 400, "end_of_day": 160, "explode_dat": 9600},
}
CASES_SEEN = {}                      # family label -> {section: cases}, filled by run()


def split(label, cmd):
    """the jobs of one test: itself, or its SPLIT parts"""
    k = SPLIT.get(label)
    return [(label, cmd)] if not k else [(f"{label} {g}/{k}", cmd + ["--group", f"{g}/{k}"]) for g in range(k)]


def floors_check(skipped=()):
    """skipped: the labels this run recorded as SKIP (--quick, --allow-skip with a missing dump)"""
    errs = []
    for fam, fl in CASE_FLOORS.items():
        seen = CASES_SEEN.get(fam)
        if seen is None:
            if fam not in skipped:   # renamed, off the list, or no CASES line
                errs.append(f"{fam}: no CASES line in this run and no skip")
            continue
        for sec, lo in fl.items():
            if seen.get(sec, 0) < lo:
                errs.append(f"{fam}: {sec} {seen.get(sec, 0)} < {lo}")
    return errs


LOCK, RUNNING = threading.Lock(), set()
# Diff tests run in parallel (each is one single-threaded process). Insert #11198: measurements 2026-10-31 --
# 8 -> 527 s, 12 -> 380 s, 16 -> 332 s with smoke_play split into 4 jobs (--group) and gate_speed into its 5 parts;
# the pool is work-bound (4705 s of test time), so 16 is the default. 16 with the OLD single smoke_play job flaked
# 122_window_squad_lost (the death morph) under the load; the split keeps the process count below that run.
# check-trial 7 (2026-11-02): with the intro part out, a queue of tests sets the end at 16 jobs (simulation 303 s).
JOBS = int(os.environ.get("CHECK_JOBS", "20"))


COV_DIR = os.path.join(ROOT, "build", "emu_cov")
COV_GATES = ("gate_diff_tested.py", "gate_diff_tested_third.py", "gate_diff_tested_half.py")   # run after the diff tests: they read their COV_DIR files


def run(name, cmd):
    t = time.monotonic()
    with LOCK:
        RUNNING.add(name)
        livecast.suite(len(STEPS), TOTAL[0], ", ".join(sorted(RUNNING)), sum(STEPS), len(STEPS) - sum(STEPS))
    env = dict(os.environ)
    if any("validate/diff/" in c for c in cmd):    # which original functions each diff test runs (gate_diff_tested.py)
        env["JA1_EMU_COV"] = os.path.join(COV_DIR, re.sub(r"[^\w.-]+", "_", name) + ".txt")
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=env)
    ok = r.returncode == 0
    tail = (r.stdout + r.stderr).strip().splitlines()[-1:] or [""]
    with LOCK:                       # one test's lines stay together
        RUNNING.discard(name)
        fam = re.sub(r" \d+/\d+$", "", name)
        for l in r.stdout.splitlines():
            if l.startswith("CASES ") and "=" in l:
                sec, v = l[6:].rsplit("=", 1)
                CASES_SEEN.setdefault(fam, {})
                CASES_SEEN[fam][sec] = CASES_SEEN[fam].get(sec, 0) + int(v)
        print(f"{'PASS' if ok else 'FAIL'}  {name:28s} {time.monotonic() - t:5.1f}s  {tail[0][:90]}", flush=True)
        TIMES[name] = round(time.monotonic() - t, 1)
        if not ok:
            FAILED.append(name)
            out = (r.stdout + r.stderr).strip().splitlines()
            early = [l for l in out[:-15] if l.startswith("FAIL")][:10]     # e.g. smoke_play's failed step lines
            print("\n".join("      " + l for l in early + out[-15:]), flush=True)
        STEPS.append(ok)
    return ok


def write_history(total, passed, failed, skipped, quick):
    """one line per finished run: the janitor compares CHECK_JOBS trials with it; a failed write never fails the check"""
    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    dirty = bool(subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True).stdout.strip())
    row = {"time": time.strftime("%F %T"), "head": head, "dirty": dirty, "jobs": JOBS, "quick": quick, "total_s": round(total),
           "passed": passed, "failed": failed, "skipped": skipped, "failed_steps": FAILED,
           "slowest": sorted(TIMES.items(), key=lambda kv: -kv[1])[:8], "times": TIMES}
    try:
        with open(os.path.join(ROOT, "build", "check_history.jsonl"), "a") as f:
            f.write(json.dumps(row) + "\n")
    except OSError as x:
        print(f"check_history.jsonl: {x}")


def main():
    T0 = time.monotonic()
    quick = "--quick" in sys.argv
    allow_skip = "--allow-skip" in sys.argv
    results = []
    skips = []
    shutil.rmtree(COV_DIR, ignore_errors=True)
    os.makedirs(COV_DIR)

    def missing(label, n):
        """A needed dump is absent: FAIL, or a counted SKIP with --allow-skip."""
        why = f"needs {NEEDS[n]} (validate/to_tactical.py or jactl load tac60, + ./t jactl dump)"
        if allow_skip:
            print(f"SKIP  {label:28s}        {why}")
            skips.append(label)
        else:
            print(f"FAIL  {label:28s}        {why}; --allow-skip to skip")
            results.append(False)

    TOTAL[0] = 3 + 2 * len(PORT_CRATES) + len(glob.glob(os.path.join(ROOT, "validate", "diff", "*_diff.py"))) + len(EXTRA) + \
        sum(k - 1 for k in SPLIT.values())
    errs = labels_check()
    print(f"{'PASS' if not errs else 'FAIL'}  {'labels.csv':28s}        " + ("; ".join(errs) if errs else "ok"))
    results.append(not errs)
    errs = dict_keys_check()
    print(f"{'PASS' if not errs else 'FAIL'}  {'dict keys (harnesses)':28s}        " + ("; ".join(errs) if errs else "ok"))
    results.append(not errs)
    results.append(run("1.13b game folders", [os.path.join(ROOT, "validate", "patch113b.sh")]))   # #24, before every use
    results.append(run("defs/types.toml", [PY, "tools/jatypes.py"]))
    results.append(run("emu_smoke", [PY, "validate/emu_smoke.py"]))
    results.append(run("story_refs", [PY, "tools/story_refs.py", "--check"]))   # every story byte the port touches has a row in docs/story_map.md
    results.append(run("quests", [PY, "tools/quests.py", "--check"]))   # ./t quests: a status rule for every chain and byte of docs/story_map.md
    for crate in PORT_CRATES:        # the port libraries the --port tests and render_diff load, built first
        for what in ("build", "test"):
            results.append(run(f"{crate} cargo {what}", [CARGO, what, "--release", "--quiet", "--manifest-path",
                                                         os.path.join(ROOT, "port", crate, "Cargo.toml")]))
    jobs = []   # the weekend play-test steps (front-end built above), in the pool with the diff tests
    gate_parts = []                  # gate_speed part job labels: one "gate_speed.py" line below is the milestone's proof
    for t in sorted(glob.glob(os.path.join(ROOT, "validate", "gate_*.py"))):   # milestone gate tests (one per milestone)
        n = os.path.basename(t)
        if n in COV_GATES:
            continue
        if n == "gate_speed.py" and not quick:      # each part its own job; the aggregate line prints "gate_speed.py"
            gate_parts = [f"gate_speed.py {part}" for part in GATE_SPEED_PARTS]
            jobs += [(lbl, [PY, t, lbl.split()[-1]]) for lbl in gate_parts]
            continue
        jobs.append((n, [PY, t]))
    for t in sorted(glob.glob(os.path.join(ROOT, "validate", "diff", "*_diff.py"))):
        n = os.path.basename(t)
        if quick and n in SLOW:
            print(f"SKIP  {n:28s}        --quick")
            skips.append(n)
            continue
        if n in NEEDS and not glob.glob(os.path.join(ROOT, NEEDS[n])):
            missing(n, n)
            continue
        jobs += split(n, [PY, t] + ARGS.get(n, []))
    for label, n, args in EXTRA:
        if quick and n in SLOW:
            print(f"SKIP  {label:28s}        --quick")
            skips.append(label)
        elif n in NEEDS and not glob.glob(os.path.join(ROOT, NEEDS[n])):
            missing(label, n)
        else:
            jobs += split(label, [PY, os.path.join(ROOT, "validate", "diff", n)] + args)
    def tier(j):     # the longest tests first: the pool ends with its longest job, not with a queue of long jobs
        n = j[0]
        if n.startswith("smoke_play"):
            return 0
        if n.startswith("gate_speed.py"):
            return 1
        if n in ("gate_campaign.py", "serve_diff.py"):     # 210 s and 198 s: longer than a split ai_diff job
            return 2
        if n.startswith("ai_diff"):
            return 3
        return 4
    jobs.sort(key=tier)
    part_ok = {}

    def run_part(j):
        ok = run(*j)
        part_ok[j[0]] = ok
        return ok
    with ThreadPoolExecutor(JOBS) as ex:
        results += list(ex.map(run_part, jobs))
    if gate_parts:               # the milestone's [test: gate_speed.py] reads this line: one result for all parts
        ok = all(part_ok.get(lbl, False) for lbl in gate_parts)
        print(f"{'PASS' if ok else 'FAIL'}  {'gate_speed.py':28s}        all {len(gate_parts)} parts above: " +
              ", ".join(p.split()[-1] for p in gate_parts), flush=True)
    for g in COV_GATES:
        if quick:
            print(f"SKIP  {g:28s}        --quick (needs every diff test)")
            skips.append(g)
        else:
            results.append(run(g, [PY, os.path.join(ROOT, "validate", g)]))
    errs = floors_check(skips)
    print(f"{'PASS' if not errs else 'FAIL'}  {'case floors':28s}        " + ("; ".join(errs) if errs else
                                                                             f"{sum(map(len, CASE_FLOORS.values()))} sections"))
    results.append(not errs)
    print(f"\ntotal {time.monotonic() - T0:.0f} s, {JOBS} parallel jobs")
    write_history(time.monotonic() - T0, sum(results), len(results) - sum(results), len(skips), quick)
    print(f"{sum(results)} passed, {len(results) - sum(results)} failed, {len(skips)} skipped")
    livecast.suite(len(STEPS), TOTAL[0], "finished", sum(results), len(results) - sum(results), done=True)
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
