# Port interface: the library's C API

The library (`port/ja1logic`) holds the game rules. It never draws, plays sound or reads input. Everything it does
not own yet is a **host call**: the library asks the caller (a front-end, or a test harness) and continues with
the answer. As more of the game moves into the library, host calls are replaced by library code; the C signatures
of the entry points stay.

Movement entry points (move codes, step AP, path search) are in `include/ja1logic.h` and need no world. The AI
entry points below work on a shared **world**.

## World

```c
typedef struct Ja1World {
    uint8_t *soldiers;           /* 48 records x 512 bytes, layout "soldier" in defs/types.toml (slot = index) */
    uint8_t *tiles;              /* 3136 x 8 bytes, layout "tile" (56 x 56, row-major) */
    const uint8_t *item_types;   /* 136 x 12 bytes, layout "itemType" (byte 2 = kind) */
    const uint8_t *anim_table;   /* 256 rows x 8 bytes, row = animation code: column 0 = can act,
                                    1 = can attack, 2 = steps on when the soldier goes idle */
    uint8_t *sector_state;       /* 80 x 9 bytes, layout "sectorState" (byte 1 = enemies left) */
    uint8_t *mission_done;       /* 80 bytes, one per sector: the scripted mission of that sector is done */
                                 /* the AI runs only in a tactical sector (cur_sector < 60) */
    uint8_t cur_sector;          /* the sector being played */
    uint8_t alarm;               /* an enemy raised the alarm */
    uint8_t combat;              /* combat mode (enemies near) */
    uint8_t difficulty;          /* 0..2 */
    int8_t  mission_step;        /* scripted mission step, > 0 = active, 1 = use the item on the target */
    uint8_t mission_slot;        /* slot ordered to the mission target, 0x40 = none */
    uint16_t mission_target;     /* the mission target tile, 0xC41 = none */
    uint16_t mission_spot;       /* the tile to stand on for it ("Reaching the target") */
    int32_t turn_based;          /* != 0: turn-based mode */
    int32_t busy;                /* > 0: something is animating (real time: the AI waits) */
    uint8_t active_slot;         /* slot whose action runs, 0x40 = none (real time: < 0x40 = the AI waits) */
    uint8_t santino_spoke;       /* Santino's quote was spoken (once per game) */
    int16_t santino_timer;       /* not read (2026-10-26): Santino's timer is turn_timer2 (same counter) */
    uint8_t show_ai;             /* redraw the screen before each AI action */
    uint32_t santino_word;       /* Santino's reserve: the enemies he can still send to sector 0 (new game: 100 / 200 / 300 by difficulty, world_load.md table 0x68); 0 when Santino speaks or a merc reaches tile 0x279 of sector 0 */
    /* slice 2b (Search procedures in ai.md): */
    uint8_t *knowledge;          /* 64 bytes, i8 per slot: what the other side knows of that soldier */
    const uint8_t *fresher;      /* 2304 bytes: freshness of (older b, newer a) = fresher[1152 + 8 * b + a] */
    uint8_t *last_seen_m;        /* 16 x 8 u16: merc / guard slot x enemy index -> tile */
    uint8_t *last_seen_e;        /* 8 x 40 u16: enemy index x opponent slot -> tile */
    uint8_t *calls;              /* 8 bytes: +0 mercs' side call tile (u16), +2 its level (u8);
                                    +4 enemies' side call tile, +6 its level */
    uint8_t *paths;              /* 48 x 80 bytes: planned path directions (0..7) per slot; soldier pathLen = count */
    const uint8_t *dir_steps;    /* 257 x i16: tile step per direction index (1..8 = N..NW) */
    const uint8_t *t0_class;     /* 256 bytes: class per ground value t0 (3 = water a calm type-1 soldier avoids) */
    const uint8_t *t1_class;     /* 256 bytes: class per object value t1 (<= 5 = can be stood on) */
    uint8_t last_spotter;        /* enemy slot that last called out a sighting */
    /* slice 2b part 2 (Situation score in ai.md): */
    uint8_t *remember_m;         /* 16 x 8 bytes: merc / guard slot x enemy index -> != 0 remembers that enemy
                                    (sector start batch 8 clears it) */
    uint8_t *remember_e;         /* 8 x 40 bytes: enemy index x opponent slot -> != 0 remembers that opponent */
    /* slice 2b part 2, batch 2 (Sector value in ai.md): */
    uint8_t *story;              /* 43 bytes: the story block (story flags read by Sector value; batch 8 writes
                                    byte 0x17) */
    uint8_t *plants;             /* 4 x 12 bytes: processing plants, i32 sector at +4, state at +8 (sector start
                                    batch 6 writes the state) */
    const uint8_t *crates;       /* 4 x 4 bytes: supply crate drops, +0 sector (i8), +1 day (i8) */
    int32_t day;                 /* the day number (as a signed 32-bit value) */
    /* slice 2c part 1 (Attack options in ai.md: expected damage, gun option): */
    const uint8_t *weapons;      /* 256 x 6 bytes per item type: byte 5 = damage base (b5) */
    const uint8_t *absorb;       /* 288 x i16: armour absorb of item t = [t], helmet absorb of item t = [t + 15] */
    /* animation (Animation script in ai.md): */
    const uint8_t *anim_rows;    /* 256 rows x 8 bytes, row = animation code: column 0 = AP cost (99 = free),
                                    2 = fixed ticks per frame (0 = computed), 4 = can act, 5 = can attack,
                                    6 = steps on when the soldier goes idle (columns 4..6 = anim_table columns 0..2) */
    uint8_t selected;            /* slot of the selected merc */
    /* animation slice 3 (Starting an animation in ai.md): */
    const uint8_t *anim_scripts; /* 68 x 0x50 bytes: animation scripts, u16 words; soldier +0x90 (u32) = byte
                                    offset into this table (the script position) */
    const uint8_t *anim_free;    /* 256 bytes, one per animation code: != 0 = no AP check */
    const uint8_t *anim_sounds;  /* 256 x i16, one per animation code: start sound argument */
    int32_t path_preview;        /* != 0: path preview marks are on */
    /* animation slice 4 (Frame routine in ai.md): */
    const uint8_t *anim_steps_tile; /* 256 bytes, one per animation code: != 0 = a step into a new tile aims at
                                       that tile's point */
    const uint8_t *anim_turn_while; /* 256 bytes, one per animation code: != 0 = turns one step per word */
    uint8_t opt_sound;           /* option: frame sounds 5 / 0xA on */
    uint8_t opt_onscreen;        /* option: some sounds only for soldiers on screen */
    int32_t view_x, view_y;      /* the visible part of the map: pixel origin ... */
    int32_t view_w, view_h;      /* ... and size */
    int32_t anim_signal;         /* set to -1 by script word 0x1B6 */
    int16_t effect_tile;         /* tile for script word 0x1B7 */
    /* sector start batch 1 (sector_start.md): */
    uint8_t *supplies;           /* 4 bytes: the enemy's supply bonuses (u8), [0] items, [1] guns, [2] medical, [3] thrown */
    const uint8_t *equip_skill;  /* 6 bytes: equipment chance base per difficulty d = equip_skill[2 d] (u8) */
    const uint8_t *attach;       /* 82 x 4 bytes: +0 kind ('B' 0x42 / '[' 0x5B), +1 item type the attachment fits */
    /* sector start batch 2: */
    uint8_t guard_skill;         /* added to the guards' equipment chance (u8) */
    /* sector start batches 3 and 4 (ground items): */
    uint8_t *ground_items;       /* 200 x 12 bytes: ground item entries (item entry layout; byte 0 = 0: free) */
    uint8_t *notes;              /* 15 x 2 bytes: {item type, placed flag} of the one-off notes */
    uint8_t *move_codes;         /* 3136 x u16: move code per tile (combat.md "Rebuilding all move codes") */
    uint8_t *held;               /* 12 bytes: the item entry on the mouse cursor */
    uint8_t *blast_list;         /* 30 x u16 tiles (0xC41 = empty) */
    const uint8_t *listed;       /* 32 bytes: item types that stay where they are when taken away */
    /* sector start batch 5 (soldier start): */
    const uint8_t *t2_class;     /* 256 bytes: class per tile byte 2 value */
    uint8_t *enemy_watch;        /* 48 bytes, one per slot: an enemy whose +0x1D4 is 3 or 4 */
    /* sector start batch 6 (map patches, supply drops, plants): */
    uint8_t *text_buf;           /* 128 bytes: a text buffer: the patch file path, the step noise text (0-terminated); a ground entry of 200+ reads it */
    uint8_t *item_buf;           /* 12 bytes: the item entry a patch record reads */
    uint8_t *crate_state;        /* 4 bytes, one per supply drop */
    uint8_t *crate_shown;        /* 4 bytes, one per supply drop: 0 / 1, 0xFF = not yet */
    /* sector start batch 7 (fixed items, tmp file, ground blast): */
    const uint8_t *fixed_items;  /* 11 x 8 bytes: +0 tile (i16), +2 item type, +4 count, +6 condition */
    uint8_t *sectors;            /* 60 x 0xB1 bytes, one row per sector: byte 0xB0 = set mines left */
    int32_t bomb_count;          /* type 0x3F ground items set off so far */
    /* sector start batch 8 (the whole sector start): */
    uint8_t *scratch;            /* 0x200 bytes: the soldier templates of absent enemies go here */
    int32_t team_on_duty;        /* mercs on duty (read only) */
    uint8_t purifier;            /* != 0: the next sector the player does not own gets the Micro-Purifier */
    /* sector start batch 11 (quest check): */
    uint8_t safe_opened;         /* != 0: the safe in sector 0x0B is open (op 74 use_on_tile sets it) */
    /* soldier update (soldier_update.md): */
    uint8_t screen_drawn;        /* != 0: the tactical screen is drawn (Stand up: who stands up) */
    uint8_t interrupt_go;        /* != 0: a merc's interrupt is granted (host op 113 merc_interrupt) */
    uint8_t turn_moves;          /* != 0: the turn moves on (combat movement ends) */
    uint8_t kept_merc;           /* the selected merc kept when the turn moves on */
    uint8_t next_selected;       /* the slot that becomes selected with the turn banner, 0x40 = none */
    uint8_t quote_slot;          /* the queued merc quote: slot (0x40 = none) ... */
    uint16_t quote_tile;         /* ... its tile (NONE) ... */
    uint8_t quote_focus;         /* ... the view goes to the merc for it ... */
    int8_t  quote_delay;         /* ... frames to wait first ... */
    uint8_t quote_event;         /* ... and the event (message number or cry 0xF9..0xFD) */
    uint8_t opt_half_delay;      /* option: walking animations run at half the frame delay */
    uint8_t opt_follow;          /* option: the view follows the selected merc too (not only others) */
    uint8_t opt_reload_msg;      /* != 0: the tactical screen was drawn (every full draw sets 1; the "out of ammo" quote waits for it) */
    uint8_t alone_warned;        /* message 4 of a +0x19C = 4 merc (alone: nearest merc more than 5 tiles away) was shown (once) */
    uint8_t follow_all;          /* != 0: the view follows soldiers that are not seen too */
    int32_t stop_timer;          /* 2000 after a low-AP stop */
    uint8_t interrupted;         /* set by Enemies see */
    int32_t walk_ap;             /* the walking AP the host's path_ap (op 40) stored last */
    uint8_t *ap_warned;          /* 8 bytes, one per merc: the low-AP stop happened this turn */
    const uint8_t *splash;       /* 60 bytes, one per sector: n of the random(n) = 0 splash chance (i8) */
    int32_t spotted;             /* the enemy slot the view was sent to when it was spotted (0x40 = none) */
    uint8_t opt_step_text;       /* option: a merc's own loud steps show as text ("splash", "clunk", "snap") */
    /* move order (soldier_update.md): */
    uint8_t key_shift;           /* Shift is held (sneak): the squad order passes it on */
    uint8_t key_alt;             /* Alt is held: the squad order passes it on */
    uint8_t opt_move_fx;         /* option: a merc's move order with a marker also makes host op 65 */
    /* one frame and the timer tick (soldier_update.md): */
    uint16_t seen_enemies;       /* enemies in play that are seen (counted each frame) */
    uint8_t in_flight;           /* the sum of the soldiers' +0x195 this frame (an attack fires only at 0) */
    uint8_t merc_shown;          /* the merc whose portrait shows (no idle portrait redraw for it) */
    uint8_t interrupt_turn;      /* != 0: a queued interrupt turn runs (the end of the enemies' turn starts a new turn) */
    const uint8_t *fx_anims;     /* 16 x 0x12 bytes: effect animations, byte 0 = count left (s8), i32 +1 = the front-end timer (the library reads only byte 0) */
    int32_t tick_step;           /* ms the 10 ms tick takes off the soldier timers +0xB0 / +0xCC (10) */
    /* merc view (soldier_update.md "Merc view"): */
    const uint8_t *view_rays;    /* 306 bytes: 17 rays x 9 steps for odd facings, then the same for even facings */
    const uint8_t *obj_block;    /* 685 bytes: how objects 0x37.. block, 9 bytes per object */
    uint8_t story_finder;        /* the merc that found a story object (0x40 = none) */
    uint8_t spot_stop;           /* a merc stopped because it spotted an item */
    /* turns (turns.md): */
    uint8_t *itq;                /* 50 bytes: the interrupt queue, soldier slots (0x40 = free) */
    uint8_t itq_len;             /* entries in use in itq */
    uint8_t *bled;               /* 48 bytes, one per slot: the soldier bled at the start of this turn */
    uint8_t *timed_fx;           /* 5 x 13 bytes: timed effects (turns.md) */
    uint8_t new_round;           /* != 0: set by a new turn, cleared by End turn */
    int16_t turn_timer;          /* set to 20 when the mercs' turn starts (front-end) */
    int16_t turn_timer2;         /* set to 0 with it */
    int32_t shown_slot;          /* the soldier the view and panel go to (0x40 = none) */
    int32_t view_tile;           /* a tile the view goes to (NONE = none) */
    uint8_t focus_slot;          /* the soldier the view follows (0x40 = none) */
    uint8_t turn_shown;          /* set when the mercs' turn starts */
    uint8_t merc_to_show;        /* the merc the next turn shows first (0x40 = none) */
    uint8_t view_hold;           /* cleared when a turn starts */
    int32_t weather_tire;        /* fatigue modifier of the conditions (turns.md, Spend AP) */
    int8_t  merc_tire;           /* fatigue modifier for mercs and guards */
    /* new turn (turns.md, New turn): */
    uint8_t round_lock;          /* set while a new round runs */
    uint8_t round_lock2;
    uint8_t handed_over;         /* != 0: the last round change handed the turn over to an interrupt */
    uint8_t turn_flag;
    uint8_t interrupter;         /* the soldier whose interrupt is granted (0x40 = none) */
    uint8_t speaker;             /* a merc waiting to say the sector speech (0x40 = none) */
    uint8_t speech_wait;         /* turns before a sector speech may start */
    uint8_t stage;               /* home-sector event stage */
    uint8_t redraw_next;
    int32_t redraw_pending;
    int32_t turn_clock;
    int32_t show_timer;
    uint8_t *sector_spoken;      /* 60 bytes, one per sector: its speech was said */
    /* shooting (shooting.md): */
    const uint8_t *silencer;     /* 17 bytes: loudness of gun 0..16 */
    const uint8_t *shot_sound;   /* 34 bytes: sound effect, i8 volume per gun 0..16 */
    const uint8_t *impact_noise; /* 33 bytes: loudness of the impact of item 0..32 */
    const uint8_t *shot_range;   /* 9 bytes: per facing 0..8, pixels before a missing bullet shows its shadow */
    int32_t busy_timer;          /* at 0 nobody is busy any more (soldier_update.md Timer tick) */
    uint8_t replay_m;            /* the 'M' replay event of this sector is recorded */
    uint8_t first_hit;           /* story stage: 1 -> 2 when an enemy is first hit */
    uint8_t fx_id;               /* the id of the last impact effect */
    /* shooting batch 2 (shooting.md): */
    uint8_t *dmg_text;           /* 240 bytes, 5 per slot: the damage number shown over the soldier (text, 0 byte) */
    uint8_t marked_slot;         /* a soldier the front-end marks (0x40 = none) */
    uint8_t knockdown;           /* the next bullet hit on a standing soldier knocks it down */
    uint8_t focus_last;          /* the soldier the view went to last */
    uint8_t hit_scale;           /* damage / 10 while a hit reaction runs, else 0 */
    /* death (shooting.md batch 4): */
    int32_t dead_mercs;          /* mercs that died today */
    int32_t dead_allies;         /* other allies (slots 16..0x27) that died today */
    int32_t dead_npc;            /* guards and native mercs that died today */
    int32_t enemy_deaths;        /* enemies that died today */
    uint8_t squad_lost;          /* no merc of the squad is left in the sector */
    uint8_t last_killer;         /* the merc that killed last */
    /* aiming (shooting.md batch 5) */
    const uint8_t *posture;      /* 256 bytes, one per animation code: >= 6 = can block a shot not aimed at it */
    const uint8_t *window;       /* 2056 bytes: window[8 x object + direction] (i8): how a wall lets a projectile pass */
    const uint8_t *attack_sound; /* 5 x i16, one per weapon class 0..4: the sound argument of a shot's Spend AP */
    const uint8_t *diff_mod;     /* 3 x i16, one per difficulty: the enemies' to-hit bonus */
    /* click (click.md) */
    uint8_t key_ctrl;            /* Ctrl is held: no panel update after a click */
    /* turn start (turns.md) */
    int32_t fast_timer;          /* ms of real time left after a turn start; at 0 turn_based comes back on */
    /* selecting a merc (select.md) */
    int32_t marks_shown;         /* != 0: path preview marks are on the map (tile byte 7) */
    int32_t marks_n;             /* number of tiles in marks */
    uint8_t *marks;              /* 128 x u16: the marked tiles, in order */
    uint8_t opt_path_marks;      /* an option: != 0 = the path preview draws its marks */
    uint8_t sees_slot;           /* a merc whose "sees enemies" message waits (0x40 = none) */
    uint8_t preview_slot;        /* the merc whose next selection shows its path preview (0x40 = none) */
    /* the rest of the update pass (soldier_update.md Update end, Fuses, Timer tick) */
    int32_t ripple_timer;        /* ms to the next water-ripple try */
    int32_t ripple_tile;         /* the tile of the last ripple try */
    int32_t ambient_t1;          /* ms to the next water-colour cycle (200 ms steps) */
    int32_t ambient_t2;          /* ms to the next second cycle and ambient-sound try (300 ms steps) */
    uint8_t ambient_count;       /* counts the second cycles (wraps to 0xF8 after 0xFE) */
    uint8_t opt_ambient;         /* an option: != 0 = water colours cycle and ambient sounds play */
    uint8_t time_compress;       /* != 0: time compression is on (no ambient sounds) */
    uint8_t enemies_warned;      /* the "enemies nearby" message was shown in this sector */
    uint8_t leave_reason;        /* why the tactical loop ends (1 = the home sector was won) */
    int32_t fuse_beep_timer;     /* ms to the next tick sound of a ticking fuse */
    uint8_t fuse_beep;           /* set to 1 when a ticking fuse's tick sound is due (the front-end clears it) */
    /* the tactical loop head (soldier_update.md Loop head) */
    int32_t turn_tick;           /* ms to the next 100 ms step of the turn timers */
    /* leaving the sector (soldier_update.md Leaving the sector) */
    uint8_t quit_tactical;       /* != 0: the tactical screen ends with no end of day (quit, load a game) */
    /* the inventory screen (inventory.md) */
    uint8_t *inv_cursor;         /* 12 bytes: the item on the mouse cursor (layout "item") */
    uint8_t *inv_swap;           /* 12 bytes: the swap item */
    uint8_t *inv_picked;         /* 8 bytes: per merc slot, the ground pick-up AP is paid */
    uint8_t *inv_give;           /* 2 bytes: give A, give B (0x40 = none) */
    const uint8_t *item_rects;   /* 14 x (i16 x, i16 y): places 0..13 */
    const uint8_t *slot_rects;   /* 8 x (i16 x, i16 y): merc slots */
    uint8_t vest_owner;          /* slot whose vest pockets the cursor vest carries, 0x40 = none */
    int32_t mouse_x, mouse_y;    /* the mouse position (the host writes them at each poll) */
    uint8_t mouse_buttons;       /* bit 0 left, bit 1 right (the host writes it at each poll) */
    uint16_t key_flags;          /* keyboard flags (the host writes them at each poll): key_shift = 1 when & 3, key_ctrl = 1 when & 0x104 (else 0) */
    uint8_t tip_item;            /* the item tooltip number (the face click sets it to 0) */
    uint8_t face_left;           /* set to 1 when the face click leaves the screen */
    uint8_t cursor_drop;         /* set to 1 when a right click under the panel leaves with an item on the cursor */
    int32_t drop_answer;         /* the answer of op 235 at that time */
    int32_t panel_dy;            /* added to mouse_y for the "under the panel" test */
    uint8_t voice_test;        /* set to 1 when a merge fails */
    /* the day accounts (strategic.md Day accounts, Worker pool) */
    uint8_t *accounts;           /* 0xA8 bytes (0x98 before Morning pay): the start of the game state block (savegame.md), same offsets */
    int32_t prev_balance;        /* the funds before the day-end accounting */
    uint8_t *sector_order;       /* 61 sector numbers: the order in which natives quit; the enemy pick shuffles it */
    /* morning pay (strategic.md Morning pay); accounts is 0xA8 bytes for it (+0xA0 = resignations) */
    uint8_t *profiles;           /* 64 x 0x5A bytes, layout "profile" */
    uint8_t *stash_counts;       /* 136 bytes, one per item type: records in the item store */
    uint8_t *stat_progress;      /* 8 x 0x12 bytes, one row per team slot */
    const uint8_t *greed;        /* 256 bytes, one per profile number */
    /* morning stat changes (strategic.md Morning stat changes) */
    uint8_t *stat_changed;       /* 640 bytes: profile number x 10 + stat (0 = the pay) */
    /* back on duty (strategic.md Back on duty) */
    uint8_t duty7_merc;          /* the soldier record whose duty 7 ends this morning, 0x40 = none */
    /* day start (strategic.md Day start) */
    uint8_t *sector_replay;      /* 540 bytes: copy of the sector table (60 x 9) made when the day starts */
    uint8_t *guide_queue;        /* 7 bytes: the guide's messages of the day (strategic.md Story events at the day start) */
    /* day clock (turns.md The day clock) */
    uint8_t clock_minute;        /* the minutes on the clock, 0..59 */
    uint8_t clock_hour;          /* the hour on the clock, 1..12 (the day starts at 7) */
    int8_t  day_hours;           /* full hours since the day began; 12 ends the day */
    int32_t day_minutes;         /* minutes since the day began */
    int32_t day_bar;             /* the day bar's length, 0..20 */
    uint8_t hard_day;            /* != 0: a day of hard weather (tiredness grows faster) */
    uint8_t night_warned;        /* the merc's evening warning came today */
    uint8_t *guard_eta;          /* 960 bytes: 60 x 8 s16, the minute a hired guard reaches the sector (9999 = none) */
    uint8_t *guard_rec;          /* 480 bytes: 60 x 8, the side the guard comes in from (1, 3, 5, 7) per guard_eta entry */
    /* the guide's advice (strategic.md The guide's advice) */
    uint8_t guides_hired;        /* native guides hired by the guide's offer (0..4) */
    uint8_t guide_wait;          /* days in a row without a guide merc since the last offer */
    uint8_t idle_day;            /* 1 = no merc could be chosen at the sector entry (world_load.md section 4) */
    uint8_t enemy_counter;       /* the enemy commander's place in sector_order (strategic.md Enemy moves) */
    /* the day's verdict (strategic.md The day's verdict) */
    int32_t start_trees;         /* g_treesOwned at the sector entry */
    int32_t start_workable;      /* g_treesWorkable at the sector entry */
    int32_t start_tapped;        /* g_treesTapped at the sector entry */
    int32_t start_sectors;       /* g_sectorsOwned at the sector entry */
    int32_t start_cap;           /* g_processCap at the sector entry */
    int32_t start_plants;        /* g_plantsOwned at the sector entry */
    uint8_t *item_told;          /* 3 bytes: the guide spoke of the ransom items 0x86, 0x7A, 0x5C (1 = yes) */
    uint8_t bonus_due;           /* != 0: the bonus is paid by day_bonus */
    uint16_t bonus;              /* money added to the day's income by day_bonus */
    /* combat.md Blast, exact steps: */
    uint8_t blaster;             /* slot of the last explosion's thrower (0x40 = none) */
    uint8_t *gas_clouds;         /* 5 x 6 bytes: u16 tile (0xC41 = free), s16 turns left, s16 radius */
    uint8_t tileset;             /* tileset of the current map */
    const uint8_t *no_break;     /* 10 bytes from JA.EXE, one per tileset: != 0 = objects do not break in blasts */
    /* combat.md The listening device at a door, exact steps: */
    uint8_t *walter_said;        /* 12 bytes: the sector where each overheard story line was said (99 = not yet; day save 20957) */
    uint8_t *walter_next;        /* 8 bytes: the next caption numbers of the plain sounds (day save 20975) */
    uint8_t walter_sector;       /* the sector of the last story line (99 at program start, not saved) */
    /* panel.md Clicks on the bottom strip: */
    uint8_t strip_box;           /* the box of the last strip click that left an item on the cursor */
    /* strategic.md The day replay: */
    uint8_t opt_replays;         /* Control Panel switch 12 "Day Replays" (save byte 21550) */
    uint8_t replay_carry;        /* the count kept from one replay event to the next (0 at program start) */
    uint8_t opt_confirm_move;    /* option byte 0x1163C6: a merc move waits for a confirm click (MoveMercTo) */
    uint8_t detector_slot;       /* set by a library click with the detector 0x4D when opt_confirm_move != 0 and slot < 8: */
    int16_t detector_tile;       /* the move order (confirm 1) is the caller's: it opens the confirm wait; tile < 0 = none */
} Ja1World;
```

The library reads and writes the world in place, field by field. **A host call may change any world data**
(soldier records included): read again after every host call, never keep a copy across one.

## Host

```c
typedef struct Ja1Host {
    void *ctx;
    /* random number 0..n-1 (n <= 0: 0). "Roll c" in the spec = random(100) < c. */
    int32_t (*random)(void *ctx, int32_t n);
    /* one host call: op from the table below, args as listed; out (out_size bytes) for ops with an output */
    uint32_t (*call)(void *ctx, int32_t op, const int32_t *args, int32_t nargs, uint8_t *out, int32_t out_size);
    /* bit n set: op n goes to the host even when the library implements it (a test that scripts that op);
       bit 63 covers op 63 and every higher op */
    uint64_t host_ops;
} Ja1Host;
```

**Library ops.** The library implements these ops itself (ai.md, Search procedures): 2 noise_tile, 3
find_dry_tile, 4 patrol_tile, 5 buddy_tile, 8 news_value, 10 approach_tile, 11 ally_call_tile, 18 can_stand_at,
9 sector_value, 12 search_tile, 20 opp_strength, 21 retreat_tile (ai.md: Sector value, Situation score,
Positions), 22 advance_tile, 23 target_tile, 24 gun_option, 25 throw_option, 26 knife_option (ai.md, Attack options),
42 fire_line (ai.md, Line of fire), 49 sight_line (ai.md, Sightings: Sight line), 35 perceive, 34 share_sightings, 115 sight_check, 63 enemy_turned, 64 guard_turned, 327 clear_rooms (ai.md, Sightings as steps), 52 set_anim (ai.md, Starting an animation), 56 anim_turn, 61 anim_frame, 84 anim_step, 83 anim_end (ai.md, Turn done, Frame
routine, Next animation, Animation end), 51 replace, 58 reserve_ap (sector_start.md, batch 9),
33 radio_result (sector_start.md, batch 10), 44 quest_check (sector_start.md, batch 11), 17 action_ap
(sector_start.md, batch 12), 45 shot_ap (sector_start.md, batch 13), 97 anim_next (ai.md, Next word), 98
walk_blocked (soldier_update.md), 106 consume_trigger, 111 enemy_step, 117 step_noise, 122 throw_follow (soldier_update.md), 109
take_ground_item (sector_start.md batch 4), 119 ai_think (ai.md, One tick), 1 idle, 29 walk_to, 40 path_ap, 41 find_path (soldier_update.md, Path ops), 137 active_frame, 138 soldier_frame (soldier_update.md, One frame), 6 direction, 7 tile_dist, 53 stop_anim (soldier_update.md, Small ops), 62 reveal_view (soldier_update.md, Merc view), 123 turn_banner, 131 itq_pop, 132 turn_hand_over, 133 round_end, 141 start_turn, 31 spend_ap, 134 new_turn, 114 next_turn, 113 merc_interrupt (turns.md), 81 launch, 136 projectile_step, 39 fire, 110 take_damage, 46 chance_to_hit, 48 hit_test, 79 start_attack, 80 attack_go, 30 click, 15 attack_ready, 14 reach_ap, 68 gun_jam (click.md), 173 shot_alert, 175 bullet_damage, 180 hit_location, 182 interrupt_check, 78 kill, 82 merc_dies (shooting.md), 140 move_order (soldier_update.md, Move order), 70 door_fx, 104 close_door, 107 break_object, 179 door_lookup (combat.md, Doors), 190 reloaded and 102 reload_check (combat.md, Reload; the same procedure), 127 make_noise, 211 hears, 212 noise_int (ai.md, Make noise, Hearing a noise, Noise interrupt), 214 leave_sector (soldier_update.md, Leaving the sector), 217 end_of_day, 43 connected_home (soldier_update.md, End of day, Joined to home). Wherever the library's code (a decision or another search) needs one of them, it
218 resolve_battle, 201 sector_gained (strategic.md, Sector battles), runs its own procedure, unless the op's bit is set in host_ops: then it makes the host call as listed in the table.
Every other op is always a host call.

Argument kinds: **slot** = soldier slot 0..47; **tile** = tile number as a signed 16-bit value (0xC41 "none" is
3137); **byte** = 0..255; **int** = the plain value. The result is a raw 32-bit value; the "result" column says
which part the AI reads. Every call is part of the observable behaviour: the order, the op and the arguments must
match the original exactly (a test compares the whole call list).

| op | name | args | result |
|---|---|---|---|
| 1 | idle | slot | none. "goes idle" (Alert state) |
| 2 | noise_tile | slot | tile (low 16 bits, signed) of the noise heard, none = 0xC41; out = 4 bytes: loudness (i32) |
| 3 | find_dry_tile | slot | tile (low 16 bits) |
| 4 | patrol_tile | slot | tile (low 16 bits) |
| 5 | buddy_tile | slot | low byte != 0: found (the host stores the target tile in the record itself) |
| 6 | direction | pixel x, y (from), pixel x, y (to): 4 ints, x = (tile % 56) × 16 + 5, y = (tile / 56) × 14 + 5 on the signed tile, both truncated toward zero | direction 1..8 (low byte) |
| 7 | tile_dist | tile, tile | distance in tiles (int) |
| 8 | news_value | slot | low byte |
| 9 | sector_value | byte sector, int 1 | value (low 16 bits, signed) |
| 10 | approach_tile | slot, tile | tile (low 16 bits) |
| 11 | ally_call_tile | slot | tile (low 16 bits) |
| 12 | search_tile | slot, byte mode | tile (low 16 bits) |
| 13 | move_ap | slot | walking cost (int; the AP check of a chosen action reads the low byte as signed; library op: combat.md Walking cost) |
| 14 | reach_ap | slot, tile, int use flag | int |
| 15 | attack_ready | slot, tile (0xC41 = any) | low byte, signed (1 ready, else −1..−6) |
| 16 | swap_hand | slot, int 0, int 6 | none. Swaps the two hands. Library op (ai.md Swap hands) |
| 17 | action_ap | slot, tile, byte flag | AP cost of using the hand item at the tile (see each use) |
| 18 | can_stand_at | slot, tile | low byte != 0: standable |
| 19 | mission_cancel | — | none |
| 20 | opp_strength | slot | situation score (low byte) |
| 21 | retreat_tile | slot | tile (low 16 bits) |
| 22 | advance_tile | slot, byte mode | tile (low 16 bits) |
| 23 | target_tile | slot | tile of the current opponent (low 16 bits) |
| 24 | gun_option | slot | none; out = 10-byte option record |
| 25 | throw_option | slot | none; out = 10-byte option record |
| 26 | knife_option | slot | none; out = 10-byte option record |
| 27 | busy | slot | int != 0: the soldier is busy. Library op (ai.md Soldier busy) |
| 28 | turn | slot, byte direction, int 30 | int != 0: still turning (library op: ai.md Turning toward a direction) |
| 29 | walk_to | slot, tile | none (the host moves the soldier, updating its record) |
| 30 | click | slot, tile, byte item | 1 = a walk to a use spot started, else 0 (start the attack / use with that item; click.md) |
| 31 | spend_ap | slot, byte amount, int sound (0 from the AI) | none |
| 32 | mission_blast | int power | none (library op: combat.md "Mission blast"; the mission bombs go off) |
| 33 | radio_result | — | new mission step (low byte) |
| 34 | share_sightings | slot, int 0 | none |
| 35 | perceive | slot, int 0 | none (update what the soldier sees) |
| 36 | redraw | — | none |
| 37 | delay | int ms | none |
| 38 | santino_speech | — | none |
| 39 | fire | int slot | none (the timed-out attack is fired) |
| 40 | path_ap | slot, tile | walking AP to the tile (low 16 bits, signed; 0 = unreachable) |
| 41 | find_path | slot, tile, int flag | int != 0 (all 32 bits): a path exists; the host stores it in `paths` and pathLen |
| 42 | fire_line | slot, tile, byte thrown | line-of-fire chance in % (int, all 32 bits) from the soldier to the tile |
| 43 | connected_home | byte sector | library op: Joined to home (soldier_update.md); 1 = a chain of player-owned sectors joins the sector to home, else 0 |
| 44 | quest_check | int quest | low byte != 0: the quest condition holds (Sector value passes 0x1C9) |
| 45 | shot_ap | slot, tile (0xC41 = no target) | AP of one shot with the main-hand gun (int) |
| 46 | chance_to_hit | slot, tile, byte aim clicks | hit chance % (low byte) |
| 47 | div_zero | — | none. The original divides by zero here and the game stops (see below) |
| 48 | hit_test | int height, int x, int y, int flown, byte direction, byte skip, int cap, tile target, int roll (≠ 0: the to-hit roll succeeded) | int != 0: the projectile stops here; out = 8 bytes: +0 new skip (u8), +4 pass-through chance % (i32) |
| 49 | sight_line | slot, int x, int y, tile, int tx, int ty, int flag, int range, int seen | library op: Sight line (ai.md); int != 0: a sight line exists |
| 50 | end_task | slot, byte flag | none (the soldier's task step ends; flag 1 = the whole task; library op: combat.md End of a task) |
| 51 | replace | slot | none (the host places the soldier again) |
| 52 | set_anim | slot, int code (low 16 bits), byte keep, byte reset | none (start animation code) |
| 53 | stop_anim | slot, int flag (the AI passes 1) | none (the soldier stops where it is; flag 1 also ends its walk plan) |
| 54 | merc_message | slot, byte message, byte p (0x40; the quote queue passes its tile), int 0, int 1 | none (show merc message number `message`: soldier_update.md "Merc message") |
| 55 | ui_refresh | int force | none (the path preview marks go away: the front-end calls Hide path marks (force), select.md) |
| 56 | anim_turn | slot, int 0 | none (the soldier turned: the host updates what it sees) |
| 57 | merc_text | slot, int text, int ms | none (show text number `text` over the merc for `ms`; 1 = "doesn't have enough Action Points!", 2 = "has reloaded.", 3 = "is shot retreating!", 4 = "dies retreating!", 5..9 = the first aid texts (combat.md), 10 = "doesn't have enough Action Points to traverse!", 11 = "not healthy enough!") |
| 58 | reserve_ap | slot | none (AP held back for the item in hand) |
| 59 | walk_sound | byte slot, int 1 | none (despite the name: no sound; the front-end redraws the frame of team slot `slot` in the merc panel, sound.md) |
| 60 | sfx | int effect, int, int, int | none (sound effect) |
| 61 | anim_frame | slot | none (the frame routine runs: Animation script) |
| 62 | reveal_view | slot, byte facing, byte range, byte think | none (Merc view, soldier_update.md) |
| 63 | enemy_turned | slot | none (an enemy finished a turn: its reaction) |
| 64 | guard_turned | slot | none (a merc or guard finished a turn: its reaction) |
| 65 | effect_slot | byte slot, int 3 | none |
| 66 | tile_effect | int tile, int 0 | none |
| 67 | quote | slot, tile | none (library op: combat.md "The listening device at a door, exact steps") |
| 68 | gun_jam | slot | none |
| 69 | use_special | slot | library op: combat.md "Using the lock kit on the mission safe, exact steps"; none (use the special item on the target) |
| 70 | door_fx | slot, tile, byte task | none (library op: the door effect, combat.md Doors) |
| 71 | popups | slot, int flag | none |
| 72 | stop_walk | slot, int flag (0: the walk ends; 1: a walk starts) | none (the busy display) |
| 73 | pickup_here | slot | none (library op: combat.md "Picking up at the own tile, exact steps") |
| 74 | use_on_tile | slot, tile, byte task | none (library op: combat.md "Using the hand item on a tile, exact steps") |
| 75 | bury_mine | slot, tile | none (library op: combat.md "Burying a mine, exact steps") |
| 76 | revive_roll | slot, byte 0 | none (library op: combat.md "Getting up after a collapse"; before 2026-11-12 the port had no handler, so the script stepped past word 0x1EA to word 999 = SET_ANIM 499 and the soldier froze, #52 c12347) |
| 77 | melee_hit | slot | none |
| 78 | kill | slot, int quiet | none (the soldier dies or falls; library op, shooting.md batch 4) |
| 79 | start_attack | slot, tile | none |
| 80 | attack_go | slot, tile | none |
| 81 | launch | slot | none (a projectile starts) |
| 82 | merc_dies | slot, int quiet | none (library op, shooting.md batch 4) |
| 83 | anim_end | slot, byte 0 | none (Animation end: back to the queued animation, or Next animation) |
| 84 | anim_step | slot | none (Next animation) |
| 85 | cursor_back | (none) | none (the host puts back the item the cursor holds: panel.md "Cursor item back") |
| 86 | open_patch | int kind, int number, int count | int: −1 = no such file in the game folder or on the CD (the game-folder file first: the 1.13b DAT\XRA.DAT; sector_start.md batch 6; kind 4 = DAT\XRA.DAT, kind 5 = DAT\EXPLODE.DAT: combat.md Craters) |
| 87 | read | int n | int: bytes read; the bytes in out |
| 88 | close | (none) | none |
| 89 | replay_event | int sector, int b, int event | none (b = 0, except int cur in ja1_enter_sector: world_load.md) |
| 90 | explode | byte thrower, int tile, int kind, int hit | none (library op: combat.md Explosion, exact steps; also sector_start.md batch 7) |
| 91 | effect | int tile, byte kind, byte timer, int power, byte owner, byte flag | 1 = taken (library op: turns.md Start a timed effect) |
| 92 | open_tmp | int sector | int: −1 = no such file, else 0 (the sector's tmp file; read with 87, close with 88) |
| 93 | sector_resets | (none) | none (the host resets its per-sector state; batch 8; no random draw since turns.md New turn) |
| 94 | keep_selected | byte slot | none (no longer called: sector_start.md batch 8 step 12 writes merc_to_show) |
| 95 | sector_tail | (none) | none (the host resets the rest of its per-sector state) |
| 96 | seek | int pos | none (the file opened last goes to byte pos) |
| 97 | anim_next | slot | none (ai.md Next word: the animation moves on one word) |
| 98 | walk_blocked | slot, tile, byte occupant | none (soldier_update.md Walk blocked) |
| 99 | focus | byte slot, int flag | none (the view goes to the soldier) |
| 100 | cry | byte speaker (+0x179), int −1, int cry (0..4 = "BAD", "GOOD", "EMPTY", "NOTHING", "HMM"), int 1, int 1 | none (a short voice line) |
| 101 | voice | int sample, int volume, int 1 | none (a sound sample) |
| 102 | reload_check | slot | library op: the same procedure as op 190, Reload (combat.md); int != 0 (no clip): the "out of ammo" quote applies |
| 103 | quote_event | byte slot, byte event, byte delay, tile, byte focus | none (queue a merc quote: soldier_update.md "Queue a merc quote") |
| 104 | close_door | tile, byte object, int seen | none (library op: closing a door, combat.md Doors) |
| 105 | stat_practice | slot, byte stat, byte amount, byte 0 | none (stat experience) |
| 106 | consume_trigger | int index | none (soldier_update.md Consume trigger) |
| 107 | break_object | tile, byte object, int seen | none (library op: opening or breaking an object, combat.md Doors) |
| 108 | trigger_item | slot, tile, byte ground index, byte armed | int: 1 = something happened (library op: combat.md "Springing a ground item, exact steps") |
| 109 | take_ground_item | int tile, int keep | none (sector_start.md Take ground item) |
| 110 | take_damage | slot, int height, int damage, int kind, int attacker (0x40 = none), tile | none |
| 111 | enemy_step | slot | none (soldier_update.md Enemy step seen) |
| 112 | tac_redraw | int 1 | none |
| 113 | merc_interrupt | (none) | none (the granted merc interrupt starts) |
| 114 | next_turn | int flag | none (the turn moves to the next soldier) |
| 115 | sight_check | slot (viewer), slot (seen) | low byte != 0: the viewer sees the soldier |
| 116 | ask_interrupt | slot (enemy), slot (seen soldier) | low byte ≠ 0: the enemy wins the interrupt (shooting.md Interrupt check). Library op (combat.md Interrupt contests) |
| 117 | step_noise | slot | none (soldier_update.md Step noise) |
| 118 | mine_flag | slot, tile | library op: combat.md "A detected mine gets a blue flag, exact steps"; none (a detected mine on the tile) |
| 119 | ai_think | slot | none (ai.md One tick) |
| 120 | low_ap_stop | int slot, int force, int quote | library op: Select merc (select.md); result (32 bits) 0 = not selected |
| 121 | use_cost | slot, tile (spot), tile (target), byte g (byte 6 of the target tile), byte kind | AP of walking to the spot and the use (low byte). Library op (combat.md Use cost, exact steps) |
| 122 | throw_follow | slot | none (soldier_update.md Throw follow-up) |
| 123 | turn_banner | (none) | none (library op: Turn start, turns.md) |
| 124 | nearest_merc | slot | int (a distance) |
| 125 | message | int text (1 = "Can't move there!", 2 "No one there", 3 "Can't get there!", 4 "Can't bury mine there!", 5 "drink", 6 "Home sector LOST!", 7 "Better put that down first!", 8 "Can only drink from one item at a time!", 9 "Can only use one camouflage kit at a time!", 10 "Vest is not empty!", 11 "Can't traverse here!", 12 "Can't traverse during an interrupt!", 13 "Crowbar breaks!", 14 "Mine buried", 15 "No enemies visible!", 16 "Recipient too far!", 17 "Recipient not healthy enough!", 18 "Recipient doesn't have enough Action Points!") | none |
| 126 | move_sound | byte slot, int 1 (turns.md, Spend AP: int 0) | none (despite the name: no sound; the front-end redraws team slot `slot` of the merc panel, sound.md) |
| 127 | make_noise | byte slot, tile, byte ground class, int loudness, byte kind (3 in water, else 1) | library op: Make noise (ai.md); none (the soldiers that hear the noise react) |
| 128 | ask_merc_interrupt | slot (merc or guard), slot (enemy it sees) | low byte ≠ 0: the merc wins the interrupt. Library op (combat.md Interrupt contests) |
| 129 | step_text | int text (0..5: "splash", "SPLASH!", "clunk", "CLUNK!", "snap", "SNAP!"; 6: "click" of PlayEmptyClick 0x1ABAC; also in text_buf) | none (the text shows for 500 ms) |
| 130 | tile_marker | byte kind, tile | none (the front-end shows marker `kind` on the tile: soldier_update.md Move order) |
| 131 | itq_pop | (none) | slot (low byte) of the next soldier in the interrupt queue, 0x40 = none (turns.md, Interrupt queue) |
| 132 | turn_hand_over | (none) | none (turns.md, Mercs' turn) |
| 133 | round_end | (none) | none (turns.md, Enemies' turn) |
| 134 | new_turn | int focus | low byte (added to the result of ja1_frame) (turns.md, New turn) |
| 135 | portrait | byte slot, int 1 | none (the merc's portrait panel is drawn) |
| 136 | projectile_step | slot | none (the soldier's projectile flies one step) |
| 137 | active_frame | slot | none (soldier_update.md Active frame) |
| 138 | soldier_frame | slot | none (soldier_update.md Soldier frame) |
| 139 | step_sound_stop | byte slot, int 1 | none (despite the name: no sound; RedrawMercPortrait? 0x336B4: the front-end redraws the portrait panel of team slot `slot` and, with opt_onscreen < 2 and `slot` = the selected one, clears the cursor tile 0x229D2C = 0xC41, not the walk marker 0x229CAC; sound.md) |
| 140 | move_order | slot, tile, byte marker, byte sneak, byte byte177, byte alt, byte confirm (0, or 1 from Tactical click item 0x4D), byte squad | result 1 = walking (soldier_update.md, Move order; the library ignores confirm: the window runs the wait, cursor.rs detector_move) |
| 141 | start_turn | slot | none (turns.md, Start of a soldier's turn) |
| 142 | mouse_hide | (none) | none |
| 143 | deselect | int 0 | none |
| 144 | panel_redraw | int 0x18 | none |
| 145 | mouse_off | (none) | none |
| 146 | mouse_move | int 0x50, int 8 | none |
| 147 | mouse_save | (none) | none |
| 148 | panel_slots | int 1 | none |
| 149 | portrait_refresh | byte slot, int 1 | none |
| 150 | ground_blast | byte type, int flag, int power, byte thrower, int last | none (library op: sector_start.md "Ground blast"; a timed effect off the map goes off: turns.md, End turn) |
| 151 | aid_tick | slot | none (the soldier's first aid goes on for one turn: turns.md, Start of a soldier's turn; library op: combat.md First aid, exact steps) |
| 152 | turn_rest | slot | none (the soldier rests on its spare AP; library op: turns.md Rest on spare AP) |
| 153 | wound_bar | slot, int 1 | none (the soldier's wound display changes) |
| 154 | energy_bar | slot, int redraw | none (the merc's energy bar changes: turns.md, Rest) |
| 155 | merc_panel | byte slot | none (the merc's panel is drawn) |
| 156 | panel_update | slot | none (the soldier's panel values change: turns.md, Spend AP) |
| 157 | mouse_refresh | (none) | none (turns.md, New turn) |
| 158 | turn_ui | (none) | none |
| 159 | guards_arrive | int minutes (5) | library op: The day clock (turns.md); int (32 bits) != 0: the day is over |
| 160 | sector_tick | (none) | none (library op: turns.md Sector tick) |
| 161 | gas_tick | (none) | none (library op: the gas clouds act once, combat.md "Blast, exact steps") |
| 162 | mouse_check | (none) | none (the front-end tests the mouse against its panel; outside it restores the pointer) |
| 163 | cursor_reset | (none) | none (the front-end resets its cursor) |
| 164 | view_save | (none) | none |
| 165 | view_restore | (none) | none |
| 166 | item_panel | slot, int 0, int 1 | none |
| 167 | speech_ready | (none) | int (32 bits) != 0: a sector speech may start |
| 168 | sector_speech | slot, int sector (signed) | none |
| 169 | stage_effects | (none) | none (library op: turns.md Home-sector stages) |
| 170 | stage_five | (none) | none (library op: turns.md Home-sector stages, the enemies nobody sees leave) |
| 171 | redraw_kept | (none) | none |
| 172 | enemy_focus | (none) | the merc to show during an enemy turn (low byte) (turns.md, Next turn) |
| 173 | shot_alert | slot, byte flag | none (the target of the shot reacts; shooting.md) |
| 174 | item_cond | int 0, slot, int 0, int 1 | none (the merc's item panel shows the new condition) |
| 175 | bullet_damage | slot | damage of the soldier's bullet (low 16 bits, signed) |
| 176 | land_item | slot, tile, byte item, int condition, int x, int y | library op: combat.md "A thrown item lands, exact steps"; none (a thrown item lands) |
| 177 | story_hit | (none) | library op: StoryHit? 0x61D28 (ja1_story_hit; a shot, a melee hit or a blast hit the story object); none |
| 178 | puff | tile, int x, int y, int look (0 splash, 1 puff, 2 explosion, 3 gas cloud), int count, int kind, int power | effect id (low byte; the front-end's effect record, combat.md The explosion effect record) |
| 179 | door_lookup | tile | library op: the door value of the tile (i32, combat.md Doors) |
| 180 | hit_location | slot, int height, int damage | damage after the hit location and armour (i32) (shooting.md batch 2) |
| 181 | knockdown_anim | slot, byte 0x28, byte 0x2B, byte 4 | none (the knockdown animation starts) |
| 182 | interrupt_check | slot, slot opponent, byte old sight flag | none (the soldier may take an interrupt turn) |
| 183 | loss_msg | slot, byte amount, int part (0 none, 1 head, 2 arm, 3 leg, 4 the home-sector warning of stage 0) | none (the stat-loss message; shooting.md batch 3) |
| 184 | merc_sees | byte slot, byte how, byte near | none (the merc reacts to what it sees at the next loop head: soldier_update.md "Merc sighting") |
| 185 | click_panel | slot, int 0 | none (the merc panel updates; shooting.md batch 4) |
| 186 | death_show | slot, int quiet | none (the merc's death portrait, sound, voice and text) |
| 187 | sector_won | (none) | none (the last enemy of the sector died) |
| 188 | squad_lost | (none) | none (the squad in the sector is gone) |
| 189 | aim_dir | int x1, int y1, int x2, int y2 | none (no longer called: the library computes it, soldier_update.md Aim direction) |
| 190 | reloaded | slot | library op: Reload (combat.md); int 0 = the gun was reloaded from a pocket clip, 1 = no clip |
| 191 | first_aid | slot, tile, byte a, byte b | low byte != 0: first aid starts (click.md; library op: combat.md First aid, exact steps) |
| 192 | attack_cursor | (none) | none (the attack cursor timer of the front-end) |
| 193 | voc | int 0x2B11, int 0x32, int 1 | none (the gun click sound) |
| 194 | banner | int kind (0..4) | none (the turn banner, turns.md Turn start) |
| 195 | select_light | byte slot, int kind | none (the soldier's selection light: 5 off, 0xC on; select.md) |
| 196 | panel_view | int view_y | the new view_y (i32) (select.md, Panel view) |
| 197 | sees_msg | slot | none (the "sees N enemies" line; the host sets `sees_slot` = the soldier's +0x17A and makes the line text: panel.md "Tactical messages") |
| 198 | mouse_wait | (none) | none (the front-end waits until the left mouse button is up) |
| 199 | cursor_state | slot | none (the cursor for the soldier; the front-end first calls Hide path marks (0)) |
| 200 | cursor_select | (none) | none (the cursor of "a merc is selected"; select.md) |
| 201 | sector_gained | int sector | none (the strategic layer marks the sector won: sector_state byte 0 bit 0 and its own records; soldier_update.md Update end) |
| 202 | won_quote | int 0x1D, int 0x63, int 1 | none (the mercs near Santino's tile say the "home sector won" quote: soldier_update.md "Team quote") |
| 203 | enemies_msg | (none) | none (the "Enemies nearby!" message box, 1500 ms; panel.md "Tactical messages") |
| 204 | music | int 2, int 0 | none (the music changes to the combat theme) |
| 205 | tile_anim | int tile, int kind | none (the front-end starts tile animation kind 0 = water ripple on the tile; soldier_update.md) |
| 206 | pal_cycle | int first, int last | none (the palette colours first..last rotate by one) |
| 207 | pal_set | int first, int last | none (the palette colours first..last go to the screen) |
| 208 | strip | int a, int b | none (the front-end loads the panel strip pictures; soldier_update.md Loop head) |
| 209 | door_snd | int which | none (0: stop all sound, load the door sounds; 1: stop them, load the overhead sounds; combat.md Doors) |
| 210 | door_anim | int big, int r, int phase, int x, int y | none (the door film and sound: kind, result r 6..11, phase 0 try / 1 outcome, film place x, y; combat.md Doors) |
| 211 | hears | slot, tile, byte strength, byte kind | library op: Hearing a noise (ai.md); none |
| 212 | noise_int | slot | library op: Noise interrupt (ai.md); none |
| 213 | hears_msg | slot, int level, int kind, int direction | none (the front-end shows "<name> hears a  <LEVEL> <KIND> noise coming from the <DIRECTION>": levels 0..3 FAINT, DEFINITE, LOUD, VERY LOUD; kinds 0..9 UNKNOWN, MOVEMENT, CREAKING, SPLASHING, IMPACT, GUNFIRE, EXPLOSION, SCREAM, IMPACT, IMPACT; direction 1..8 as op 6) |
| 214 | leave_sector | int flag | library op: Leaving the sector (soldier_update.md); none |
| 215 | fade | int 0, int 0x38, int 0x100, int 0 | none (the screen fades out with the game palette; soldier_update.md Leaving the sector) |
| 216 | music_stop | (none) | none (the music stops; the front-end clears its mouse hover flag and its next-music number) |
| 217 | end_of_day | (none) | library op: End of day (soldier_update.md); none |
| 218 | resolve_battle | int sector, int f | none (the strategic layer settles the battle in the sector; f = 1, or the flag f of strategic.md Enemy moves) |
| 219 | save_tmp | (none) | none (the sector's temporary save) |
| 220 | timer_off | (none) | none (the game timer stops) |
| 221 | inv_poll | (none) | 1 = stop now (the test script is over: the library returns at once, with no more calls or writes), else 0; before it returns the host writes mouse_x, mouse_y, mouse_buttons, key_flags (inventory.md) |
| 222 | key_pressed | (none) | != 0: a key waits |
| 223 | get_key | (none) | the key (low byte) |
| 224 | mouse_show | (none) | none |
| 225 | inv_cursor_pic | byte type | none (the cursor picture) |
| 226 | inv_cursor_pic2 | byte type | none (the cursor picture after a use) |
| 227 | inv_click | (none) | none (click sound) |
| 228 | inv_begin | int 0 | none |
| 229 | inv_end | (none) | none |
| 230 | item_value | int slot, int entry (slot -1 = the cursor item) | the value to show (low 16 bits) |
| 231 | item_info | byte profile, byte type, int count, int value, int 1, int 1, int info | 1 = redraw the body |
| 232 | inv_body | slot | the armour value shown |
| 233 | tip_item | byte n | none |
| 234 | tip_merc | int n | none |
| 235 | drop_answer | int 1, int 190, int 318, int 197 | a value the library stores in drop_answer |
| 236 | text_toggle | (none) | none (the front-end toggles opt_step_text and shows "Text ") |
| 237 | inv_extra | int sound, int text: sound 0 none, 1 "empty.8", 2 "hmm.8", 3 "huh.8", 4 "noroom.8"; text 0 "", 1 "Empty", 2 "Drank...", 3 "OK", 4 "No Room" | none |
| 238 | draw_slot | slot, int place | none (redraw one item place) |
| 239 | vestmap | slot | none |
| 240 | no_room | (not called since 2026-10-29: the library fills the stack itself, inventory.md "No room") | none |
| 241 | rest | slot, int amount, byte 1 | none (energy back) |
| 242 | inv_select | byte slot, byte 0, byte 0 | 0 = refused, else the merc is chosen |
| 243 | inv_leave_merc | slot | none |
| 244 | inv_moved | slot, int 1, int 1 | none |
| 245 | inv_vest_begin | int 0, int 0x34, int 0x25 | none |
| 246 | inv_vest_end | int 0x25 | none |
| 247 | mine_spotted | slot, tile, byte ground | byte: != 0 = the merc knows it is a mine (library op: combat.md "Mine spotted, exact steps") |
| 248 | disarm | slot, tile, byte ground, int armed, int menu (0, 0 from the inventory screen; 1, 1 and 0, 1 from op 74 use_on_tile) | int (signed): > 0 = take it, −1 = it went off, else not (library op: combat.md "Disarming an explosive, exact steps") |
| 249 | pick_up | slot, tile, byte 0 | none (the item goes to the cursor) |
| 250 | inv_put_item | tile, int d0, int d1, int d2, int 2, int 0, int 0, int 0 (d0..d2 = the 12 entry bytes as 3 little-endian words) | none (combat.md, put an item on the ground) |
| 251 | merge_prompt | byte type a (the cursor copy), byte type b (entry 0) | 1 = yes |
| 252 | merge_quote | byte profile | none |
| 253 | merge_yes_no | slot | none |
| 254 | eod_begin | int day | none (sunset picture, palette and sound; soldier_update.md End of day) |
| 255 | eod_wait | (none) | none (wait for the next end-of-day frame) |
| 256 | eod_text | int day | none ("End of Day " + day in decimal, centred) |
| 257 | eod_poll | (none) | != 0: a key or a mouse button came (the screen is shown first) |
| 258 | eod_sound_stop | (none) | none (the sunset sound stops) |
| 259 | overhead_sounds | (none) | none (all sound stops; the tactical sounds are loaded again) |
| 260 | show_screen | (none) | none (the picture without the texts is shown again) |
| 261 | natives_quit_msg | int count, int word (0 "guard", 1 "guards", 2 "tapper", 3 "tappers") | none (the status message "count word quit this morning!"; strategic.md Natives quit) |
| 262 | map_redraw | (none) | none (the island map and its panel are drawn again; strategic.md Worker pool) |
| 263 | face_clear | byte team slot | none (the merc's face picture is cleared; strategic.md Morning pay) |
| 264 | stash_put | byte type, byte qty, byte cond | byte: != 0 = the record went into the item store |
| 265 | face_move | byte to, byte from | none (the face picture of team slot from is copied to slot to) |
| 266 | team_redraw | (none) | none (the team panel is drawn again) |
| 267 | runs_off | int team slot k, byte soldier slot byte, byte profile | none (the merc's quote, balloon and the "Perhaps you should've given the money to a merc you can trust?" message) |
| 268 | island_map | (none) | none (the island map is drawn again) |
| 269 | runs_off_end | (none) | none (the money panel is drawn again, the mouse pointer shown) |
| 270 | pd_message | int text (0 "NOT ENOUGH FUNDS TO PAY ALL MERCENARIES!", 1 "Choose someone to be the charitable one", 2 "The native guides help you for free!") | none (strategic.md Not enough funds) |
| 271 | pd_timer_start | (none) | none (the blink timer starts again) |
| 272 | pd_elapsed | (none) | int (signed): ms since op 271 |
| 273 | pd_clear | (none) | none (the screen area for the dialog is prepared) |
| 274 | pd_blink | int on (0 / 1) | none (the funds text is drawn in one of its two colours) |
| 275 | pd_ask | int team slot, int family (1 = "Don't pay MEMBER's family?", 0 = "Don't pay MEMBER?") | low byte != 0 = yes (the host hides the mouse pointer and shows the merc's yes / no prompt) |
| 276 | pd_quote | byte profile | none (the merc's "not paid" quote) |
| 277 | pd_balloon | int team slot, int face x | none (the merc's speech balloon) |
| 278 | pd_end | (none) | none (the funds box and the panel are drawn again, the mouse pointer shown) |
| 279 | pd_face_frame | int team slot | none (the face frame of that merc) |
| 280 | pd_panel_show | (none) | none (the team panel is shown again) |
| 281 | practice | int team slot, int stat 1..9, int amount (signed 16-bit), int flag (1 = training) | none (stat experience; strategic.md Morning duties) |
| 282 | xp_stat_begin | (none) | none (one stat of one merc starts: the host checks for Escape, which makes the rest faster, and starts its timer; strategic.md Morning stat changes) |
| 283 | xp_count | int team slot, int stat 0..9, int value, int delta | none (the host counts the stat's panel number from value to value + delta: strategic.md "The experience count"; the library stores the result after the op) |
| 284 | xp_health_bar | int team slot | none (the merc's health bar is drawn again) |
| 285 | xp_salary_msg | int team slot, int up (1 = the level rose, 0 = it fell), int level, int salary | none (the message "NAME's experience level has risen to L! The A.I.M. ... $S per day, effective tomorrow.") |
| 286 | xp_stat_end | (none) | none (the host waits until its timer reaches the pause: 120 ms, 25 ms after Escape) |
| 287 | xp_end | (none) | none (the screen area of the panel is drawn again) |
| 288 | cp_begin | int team slot S | none (the screen gets ready for merc S's quotes; strategic.md AIM rate complaints) |
| 289 | cp_quote | int team slot S, int quote (8 fatality, 9 turnover, 10 bad days, 11 non-payment), int profile number of the other merc, int first (1 = the first quote of this call) | none (merc S says the quote about the other merc: when first, the quote 7 comes before it; the quote text names the other merc) |
| 290 | cp_end | (none) | none (after the last quote) |
| 291 | hate_quote | int soldier k (record 0..7), int profile number d | none (merc k says quote 6: it will not work with merc d, named in the text; strategic.md A disliked merc in the team) |
| 292 | corpse_ask | int cost, int can_pay (1 = cost <= balance) | int answer (!= 0 = yes; ignored when can_pay = 0): the host writes "Dish out $cost for corpse export?" into its text buffer and shows it (can_pay = 0: the same box, its yes row differs; panel.md "The travel animation"); strategic.md Corpse export |
| 293 | corpse_paid | (none) | none (the host draws the funds again after the payment) |
| 294 | corpse_end | (none) | none (the host draws the island map and the team panel again) |
| 295 | raise_voice | int soldier k, int profile g | none (merc k's voice line about the grudge merc g; the host remembers g for the quote; strategic.md Salary raise) |
| 296 | raise_ask | int soldier k, int reason 1..5, int g (grudge profile for reason 5, else -1) | none (merc k says the reason quote, with g's nickname for reason 5, in its text box) |
| 297 | raise_offer | int soldier k, int start offer (signed 16-bit) | int offer (signed 16-bit; 0 = NO): the host shows the salary box (SalaryDialog? 0x29074: up / down steps from the start offer, OK only above it; port/ja1front salary.rs) |
| 298 | raise_answer | int soldier k, int answer 0..2 | none (merc k's quote: 14 for 0 and 1, 13 for 2; then the text box again) |
| 299 | raise_done | (none) | none (after the dialogs of one merc) |
| 300 | raise_redraw | (none) | none (the host draws the island map and the team panel again) |
| 301 | payroll_show | (none) | none (the host shows the morning payroll screen: the island map, the flip animation and the money panel; strategic.md The morning) |
| 302 | guide_say | int quote, int category (mostly 6) | none (the native guide says the quote, e.g. 0x2F: more tappers than the plants can process; strategic.md Sector click on the island map, The guide's advice) |
| 303 | day_ask | int kind (0 = nobody on duty, 1 = Get to work?), int wait_row, int go_row, int number | answer (≠ 0 = yes; strategic.md Day start) |
| 304 | sunrise_show | (none) | none (the host shows the sunrise screen; strategic.md Sunrise) |
| 305 | merc_gone | int profile | none (the status line: "<nickname> seems to have disappeared!"; strategic.md Strategic screen opening) |
| 306 | map_show | (none) | none (the host draws the island map and its panel; strategic.md Strategic screen opening) |
| 307 | quest_record | int n | none; out = 250 bytes: DAT\QUESTS.DAT bytes 250 (n − 5) .. + 249 (the game-folder file first, else the CD: the 1.13b patch file; strategic.md Story events at the day start) |
| 308 | stash_take | int type, int take | byte: ≠ 0 = the item store holds a record of that type; with take ≠ 0 the host takes one such record out of the store (strategic.md Story events at the day start; the original misses the record when it is the last one of its store list and take = 0, and takes a whole stack when Shift is held: host matters) |
| 309 | day_bar | int time_compress | none (the front-end draws the day bar, length day_bar: turns.md The day clock) |
| 310 | enemy_pick | (none) | byte: the sector where the enemy sends troops next (library op: strategic.md Enemy moves) |
| 311 | enemy_move | byte sector | byte: soldiers who come into the current sector because of the move (0 = none; library op: strategic.md Enemy moves) |
| 312 | guard_enter | byte record | byte: != 0 = the hired guard walked into the current sector (turns.md The day clock); library op: strategic.md Walk-ins |
| 313 | mode_banner | (none) | none (the front-end draws the tactical screen again with its current panel) |
| 314 | guards_msg | int count, int number | none (status line: "1 Guard arrives in sector N" or "COUNT Guards arrive in sector") |
| 315 | mouse_release | (none) | none (the front-end waits until the right mouse button is up: turns.md Time compression) |
| 316 | refresh | (none) | none (the front-end draws the tactical screen and its panel again) |
| 317 | brenda_say | int quote | none (Brenda, profile 0x41, says the quote; strategic.md The guide's advice) |
| 318 | panel_draw | int a, int b | none (the front-end draws the island map panel again: (0, 14) after a money change, (9, 10) after a hire) |
| 319 | guide_offer | int profile | answer (≠ 0 = yes): the host asks "Accept this native guide?" with the face and nickname of the profile |
| 320 | merc_join | int profile | none (the host calls `ja1_merc_join` (strategic.md "A merc joins the team") and loads the face of the new team slot) |
| 321 | assign_panel | int team slot | none (the host draws the merc's item panel of the assignment screen again: strategic.md "Selecting a merc", steps 1..9 without the mouse move) |
| 322 | discard_ask | int item type | answer (≠ 0 = yes): the host asks "Get rid of this item PERMANENTLY?" in the question box with GARBAGE.PCC and the item's picture (strategic.md "Item moves on the item panel") |
| 323 | stash_take_rec | int type, int count, int condition, int exact | 0 = no such record; else 0x10000 + count × 256 + condition of the first record equal to (type, count, condition) (exact ≠ 0) or of type `type` (exact = 0), which leaves the item store |
| 324 | store_place | int j (0..3), int last | what store place j of the assignment screen shows (last ≠ 0: in view 0 a walk past the last record stays on it, strategic.md "Right button on the item panel"; bit 26 = store view 0 with no current record, a stop point: host call 47): 0 = nothing to take; else bit 24 set, bits 0..7 type, 8..15 count, 16..23 condition, bit 25 = a record (store view 0; take it exactly), bit 25 clear = a type (view 1; count and condition unused) |
| 325 | store_again | int took | none: when took ≠ 0 the host sets the top type = Fix(top, top) and the current record = the first record of the top type; then the store list again (strategic.md "Item moves on the item panel") |
| 326 | guard_sees_msg | (none) | none (the status message "Guard sees a new enemy!"; ai.md Share sightings; a message box of 25 000 ms, panel.md "Tactical messages") |
| 327 | clear_rooms | (none) | none (library op: tile byte 3 cleared in two fixed rectangles; ai.md Clear rooms) |
| 328 | fire_quote | int team slot, int x, int y, int profile | none (the fired AIM merc says quote 9 of category AIM in its speech balloon at (x, y); strategic.md Firing a merc) |
| 329 | enemy_walk_in | int slot, int edge | byte: ≠ 0 = the enemy (slot 0x28..0x2F) walked into the current sector at the edge (3, 5, 1, 7; 9 = Santino's troops); library op: strategic.md Walk-ins |
| 330 | home_lost_msg | (none) | none (the status message "Home sector LOST!"; strategic.md The day's verdict) |
| 331 | rating_word | int score | none (the host draws the day's rating word for score −3..3: TERRIBLE, LOUSY, POOR, FAIR, GOOD, EXCELLENT, OUTSTANDING; strategic.md The day's verdict) |
| 332 | battle_ui | int k, int n | none (the battle show: k = 0 start, 1 open the battle sounds, 2 play attack, 3 play die n, 4 play mine n, 5 wait for the sound end, 6 stop the sound, 7 close the battle sounds, 8 end; strategic.md Sector battles) |
| 333 | battle_wait | int ms | byte: ≠ 0 = the player skipped the rest of the battle show during the wait of ms milliseconds |
| 334 | battle_box | int s, int enemies, int guards, int tappers, int kind | none (the box "ENEMY ATTACKING SECTOR s + 1!" with the counts; kind 99 first picture, 0 an enemy fell, 1 a guard fell, 2 a tapper fell) |
| 335 | battle_msg | int t, int n | none (status message t with the sector number n: 0 Mine explodes, killing enemy in sector n; 1 GUARDS REGAIN SECTOR n; 2 ENEMY DEFEATED IN SECTOR n; 3 ENEMIES GAIN SECTOR n; 4 NATIVES LOSE SECTOR n; 5 GUARDS DEFEATED IN SECTOR n; 1..5 as a 1500 ms box) |
| 336 | explode_art | tile, int big | none (game logic, not a picture: the crater, ground items, the sector-0 story kill; big = 1 for kind 3; combat.md Craters and blast leftovers) |
| 340 | call_enemies | tile, int kind | none (library op: combat.md "Enemy call") |
| 341 | take_patch | tile, byte type | none (library op: sector_start.md "Take ground patch") |
| 342 | toggle_doors | byte mode, byte value | none (library op: combat.md "Toggling doors by value") |
| 343 | pick_up | slot, tile, byte cursor | none (library op: combat.md "Picking up a ground item, exact steps") |
| 344 | cursor_to_soldier | slot | none (when `held` byte 0 ≠ 0: the cursor shows the item, then the inventory screen of the soldier runs as key I does (not for a soldier with +0x188 = 1); when the cursor still holds an item after the screen and the tactical cursor-item flag is 0: the mouse is limited to (1, 190)..(318, 197) and the flag = 1. A front-end may run the screen after the current pass) |
| 345 | disarm_ask | slot, int kind (0 = "Attempt to disarm explosive?", 1 = "Remove blue flag?") | low byte ≠ 0 = yes (the host shows the merc's yes / no prompt with the face of S[0x179]; combat.md "Disarming an explosive, exact steps") |
| 346 | ground_item_reset | byte g | none (library op: sector_start.md "Item reset" on ground entry g with condition mode 0, count mode 0) |
| 347 | menu_redraw | (none) | none (the tactical screen is drawn again, the original's refresh from the menu) |
| 348 | walter_say | int q | none (the listening device's clip wp<q>.8 and caption q: combat.md "walter_say") |
| 353 | replay_count | (none) | the number of the day's events (strategic.md "The day replay") |
| 354 | replay_record | int i | event i: from \| to << 8 \| code << 16, + 1 << 24 when Escape was pressed since the last poll |
| 355 | replay_anim | int k, ... (strategic.md "The day replay") | bit 0 = Escape was pressed in the animation |
| 356 | replay_draw | int k, int a | none (the island map from `sector_replay`) |

**Division by zero.** Where a spec step divides by a value that is 0, the original game crashes. The library
then makes host call 47 div_zero (a test host treats it as the expected crash). If the host returns, the
quotient is 0 and the procedure goes on; nothing after that point is part of the observable behaviour. This
applies to every division in the spec whose divisor can be 0 (for example Max AP and the melee skill with a
maximum health of 0, the expected damage with a range of 0, the aim value with a + sap = 0, the melee hit chance
with a + d = 0). The same host call marks a **stop point**: a step where the original reads memory that is not
game data, so its result depends on the machine (the store press with no current record, strategic.md "Item
moves on the item panel") — or writes past all memory (the View Team health bar with a negative row count,
strategic.md "The team ledger") — or runs about 2^32 rounds (the doctor with a negative budget and hp above
hpMax, strategic.md "Doctor budget"; a repair with a negative point count on a condition above 100, strategic.md
"Repair"; a text glyph of width 0, panel.md "Text") — or writes past a game array into other
game data, whose layout the port does not copy (a push onto the full interrupt queue, ai.md "Noise interrupt"). There the library makes host call 47 and the step does nothing more.

**Option record** (10 bytes): +0 valid, +1 target slot, +2 aim clicks, +3 hit chance %, +4 score (i32,
little-endian), +8 target tile (u16).

## Strategic entry points

strategic.md "Day accounts" and "Worker pool and natives who quit". They use the world fields `accounts`,
`prev_balance`, `sector_order`, `sector_state`, `plants`, `soldiers` (slots 0..7) and `difficulty`.

```c
void ja1_calc_ledger_all(Ja1World *w, const Ja1Host *h);    /* All accounts */
void ja1_apply_day_ledger(Ja1World *w, const Ja1Host *h);   /* Pay the day */
void ja1_recalc_territory(Ja1World *w, const Ja1Host *h);   /* Territory */
void ja1_worker_pool(Ja1World *w, const Ja1Host *h);        /* Worker pool (host ops 261, 262) */
void ja1_natives_quit(Ja1World *w, const Ja1Host *h, int32_t n);   /* Natives quit, n = a byte (host op 261) */
/* Morning pay (strategic.md; also profiles, stash_counts, stat_progress, greed, voice_test, item_types, listed) */
int32_t ja1_cash_count(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t drop);  /* Cash carried */
void ja1_collect_cash(Ja1World *w, const Ja1Host *h);                              /* Collect the cash */
void ja1_stash_item(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t place);   /* Store an item: the soldier's item itself */
void ja1_merc_leaves(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t status); /* Merc leaves (status = a byte) */
void ja1_team_compact(Ja1World *w, const Ja1Host *h, int32_t redraw);              /* Close up the team */
void ja1_pay_mercs(Ja1World *w, const Ja1Host *h);                                 /* Pay the mercs */
/* The day's verdict (strategic.md; also story, crates, crate_state, supplies, the start_* fields, item_told, bonus) */
int32_t ja1_day_verdict(Ja1World *w, const Ja1Host *h);   /* 0 = the game goes on; 2, 3, 4 = the game ends */
void ja1_day_bonus(Ja1World *w, const Ja1Host *h);        /* after a verdict of 0 */
void ja1_day_end_accounts(Ja1World *w, const Ja1Host *h); /* strategic.md "The day-end accounts", steps 1..4 */
int32_t ja1_day_end(Ja1World *w, const Ja1Host *h);       /* strategic.md "The day-end screen", day-end steps 1, 2: 0 or 3 (the game ends) */
/* The day replay (strategic.md; sector_replay, opt_replays, replay_carry; host ops 353..356) */
void ja1_day_replay(Ja1World *w, const Ja1Host *h);
/* The end of the game (ending.md "Game end" win step 1, "The performance rating"; story, accounts, sector_state) */
void ja1_win_vars(Ja1World *w, const Ja1Host *h, int16_t *out);   /* v0..v3 of WINGAME.SAB into out[4] */
int32_t ja1_performance_rating(Ja1World *w, const Ja1Host *h);   /* the score 0..100; recalculates the accounts */
```

## AI entry points

All take the world, the host and the soldier's slot. Return values are bytes (action codes, 0/1 flags) widened
to int32.

```c
void    ja1_ai_tick(Ja1World *w, const Ja1Host *h, int32_t slot);            /* ai.md "One tick" */
void    ja1_ai_update_alert(Ja1World *w, const Ja1Host *h, int32_t slot);    /* "Alert state" */
int32_t ja1_ai_choose_action(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t fallback);
        /* "Choosing an action": calm / alerted / combat / engaged by state; any other state returns fallback */
int32_t ja1_ai_calm_action(Ja1World *w, const Ja1Host *h, int32_t slot);
int32_t ja1_ai_alerted_action(Ja1World *w, const Ja1Host *h, int32_t slot);
int32_t ja1_ai_combat_action(Ja1World *w, const Ja1Host *h, int32_t slot);
int32_t ja1_ai_engaged_action(Ja1World *w, const Ja1Host *h, int32_t slot);
int32_t ja1_ai_action_affordable(Ja1World *w, const Ja1Host *h, int32_t slot);  /* "AP cost": 1 = affordable */
int32_t ja1_ai_start_action(Ja1World *w, const Ja1Host *h, int32_t slot);
        /* "Starting an action": 1 while it goes on, 0 when finished or failed */

/* Search procedures (ai.md). Tiles in and out as described there; host_ops applies to their inner calls too. */
int32_t ja1_ai_noise_tile(Ja1World *w, const Ja1Host *h, int32_t slot, uint16_t *loudness); /* tile, low 16 bits */
int32_t ja1_ai_find_dry_tile(Ja1World *w, const Ja1Host *h, int32_t slot);
int32_t ja1_ai_patrol_tile(Ja1World *w, const Ja1Host *h, int32_t slot);       /* full 32-bit value */
int32_t ja1_ai_buddy_tile(Ja1World *w, const Ja1Host *h, int32_t slot);        /* 1 found (aiTarget set) / 0 */
int32_t ja1_ai_news_value(Ja1World *w, const Ja1Host *h, int32_t slot);        /* 0..255 */
int32_t ja1_ai_approach_tile(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);
int32_t ja1_ai_ally_call_tile(Ja1World *w, const Ja1Host *h, int32_t slot);    /* low 16 bits defined */
int32_t ja1_ai_can_stand_at(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);  /* 1 / 0 */
int32_t ja1_ai_advance_tile(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t weak);
int32_t ja1_ai_target_tile(Ja1World *w, const Ja1Host *h, int32_t slot);

/* Situation score (ai.md). */
int32_t ja1_ai_opp_strength(Ja1World *w, const Ja1Host *h, int32_t slot);      /* 0..4 */
int32_t ja1_ai_soldier_power(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t flag);
        /* Soldier power of the record in that slot; tile = low 16 bits as s16 (0xC41 = no tile), flag = low byte;
           full 32-bit result */

/* Positions and sector value (ai.md). */
int32_t ja1_ai_retreat_tile(Ja1World *w, const Ja1Host *h, int32_t slot);      /* tile, low 16 bits */
int32_t ja1_ai_search_tile(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t mode);  /* mode = low byte */
int32_t ja1_ai_sector_value(Ja1World *w, const Ja1Host *h, int32_t sector, int32_t flag);
        /* sector 0..79, flag = low byte; result -100, 20..100, 250 or 1000 */

/* Attack options (ai.md). The 10-byte option record is written to out. */
void    ja1_ai_gun_option(Ja1World *w, const Ja1Host *h, int32_t slot, uint8_t *out);
void    ja1_ai_knife_option(Ja1World *w, const Ja1Host *h, int32_t slot, uint8_t *out);
void    ja1_ai_throw_option(Ja1World *w, const Ja1Host *h, int32_t slot, uint8_t *out);
int32_t ja1_ai_line_of_fire(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t thrown);
        /* tile = low 16 bits as s16, thrown = low byte; result = the chance in % (full 32-bit value) */
int32_t ja1_sight_line(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t x, int32_t y, int32_t tile,
                       int32_t tx, int32_t ty, int32_t flag, int32_t range, int32_t seen);
        /* = library op 49 (ai.md Sight line): slot = the looker S, tile = low 16 bits as s16, flag / seen = low
           bytes, x, y, tx, ty, range 32 bits; result 1 = seen, 0 = not seen */
void    ja1_anim_delay(Ja1World *w, const Ja1Host *h, int32_t slot);         /* ai.md "Frame delay" */
void    ja1_anim_step(Ja1World *w, const Ja1Host *h, int32_t slot);          /* ai.md "Next animation" */
void    ja1_anim_set(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t code, int32_t keep, int32_t reset);
        /* ai.md "Starting an animation": code = low 16 bits, keep / reset = low bytes */
void    ja1_anim_frame(Ja1World *w, const Ja1Host *h, int32_t slot);         /* ai.md "Frame routine" */
void    ja1_anim_end(Ja1World *w, const Ja1Host *h, int32_t slot);           /* ai.md "Animation end" (op 83) */
void    ja1_anim_turn(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);  /* "Turn done"; flag = low byte */
```

The decision procedures store their target (tile or direction) in the soldier record and return the action code;
`ja1_ai_tick` stores the chosen code in the record and calls the others. The record offsets are in ai.md.

## Sector start entry points

```c
/* sector_start.md batch 1 */
void ja1_slot_reset(Ja1World *w, const Ja1Host *h, int32_t record, int32_t slot);  /* "Slot reset" on record */
void ja1_item_init(Ja1World *w, const Ja1Host *h, uint8_t *item, int32_t cond_mode, int32_t count_mode);
        /* "Item reset" on the 12-byte entry at item; modes = low bytes */
void ja1_equip(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);       /* "Equipment roll"; flag = low byte */
/* batch 2 (host ops 51 replace, 58 reserve_ap) */
void    ja1_xra_enemy(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_init_tapper(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t facing);
void    ja1_init_guard(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t facing, int32_t orders,
                       int32_t anchor);
int32_t ja1_guard_tile(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t dir, int32_t dist);   /* a tile */
void    ja1_place_merc(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t facing);
void    ja1_merc_entry(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t facing);
/* batch 3 (no host call) and batch 4 (host op 85 cursor_back) */
void    ja1_put_item(Ja1World *w, const Ja1Host *h, int32_t tile, const uint8_t *item, int32_t b6, int32_t exact,
                     int32_t note, int32_t near);                  /* item = a 12-byte entry, read only */
void    ja1_take_ground_item(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t keep);
void    ja1_take_ground_patch(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t type);
void    ja1_pick_up(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t cursor);   /* op 343: combat.md
                     "Picking up a ground item, exact steps" (host ops 85, 344 cursor_to_soldier, 103 quote_event) */
/* batch 5 (no host call) */
void    ja1_soldier_start(Ja1World *w, const Ja1Host *h, int32_t record);
/* batch 6 (host ops 86 open_patch, 87 read, 88 close, 89 replay_event) */
int32_t ja1_tile_patch(Ja1World *w, const Ja1Host *h, int32_t kind, int32_t number, int32_t count, int32_t bl,
                       int32_t cl);
int32_t ja1_item_patch(Ja1World *w, const Ja1Host *h, int32_t kind, int32_t number, int32_t count, int32_t bl,
                       int32_t cl);
void    ja1_crate_lost(Ja1World *w, const Ja1Host *h, int32_t drop);
void    ja1_crate_drops(Ja1World *w, const Ja1Host *h);
void    ja1_plant_sector(Ja1World *w, const Ja1Host *h);
void    ja1_plant_done(Ja1World *w, const Ja1Host *h);
/* batch 9 (ops 51 and 58 as library ops; host op 17 action_ap) */
void    ja1_replace_soldier(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_reserve_ap(Ja1World *w, const Ja1Host *h, int32_t slot);
/* batch 10 (op 33 as a library op; host op 44 quest_check) */
int32_t ja1_radio_result(Ja1World *w, const Ja1Host *h);
/* batch 11 (op 44 as a library op) */
int32_t ja1_quest_check(Ja1World *w, const Ja1Host *h, int32_t quest);
/* batch 12 (op 17 as a library op; host ops 45, 7, 14, 40, 6) */
int32_t ja1_action_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t flag);
/* batch 13 (op 45 as a library op) */
int32_t ja1_shot_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);
```

## Turn entry points

```c
/* turns.md */
void    ja1_end_turn(Ja1World *w, const Ja1Host *h);
int32_t ja1_effect_start(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t kind, int32_t timer, int32_t power,
                         int32_t owner, int32_t flag);   /* turns.md Start a timed effect (op 91) */
void    ja1_stage_effects(Ja1World *w, const Ja1Host *h);  /* turns.md Home-sector stages (op 169) */
void    ja1_stage_five(Ja1World *w, const Ja1Host *h);     /* turns.md Home-sector stages (op 170) */
int32_t ja1_first_merc(Ja1World *w, const Ja1Host *h);
void    ja1_turn_sightings(Ja1World *w, const Ja1Host *h, int32_t both);
int32_t ja1_itq_pop(Ja1World *w, const Ja1Host *h);
void    ja1_enemies_turn(Ja1World *w, const Ja1Host *h);
void    ja1_mercs_turn(Ja1World *w, const Ja1Host *h);
void    ja1_start_turn(Ja1World *w, const Ja1Host *h, int32_t slot);    /* = library op 141 */
void    ja1_bleed(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_turn_drown(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_turn_rest(Ja1World *w, const Ja1Host *h, int32_t slot);      /* = library op 152 */
/* combat.md First aid, exact steps */
void    ja1_aid_tick(Ja1World *w, const Ja1Host *h, int32_t slot);       /* = library op 151 */
int32_t ja1_first_aid(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t task, int32_t b);   /* = library op 191 */
/* combat.md Blast, exact steps */
void    ja1_blast(Ja1World *w, const Ja1Host *h, int32_t centre, int32_t radius, int32_t kind);
void    ja1_blast_tile(Ja1World *w, const Ja1Host *h, int32_t centre, int32_t tile, int32_t ring, int32_t kind);
void    ja1_gas_tick(Ja1World *w, const Ja1Host *h);                     /* = library op 161 */
int32_t ja1_turn(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t d, int32_t ticks);   /* = library op 28 (ai.md Turning toward a direction) */
int32_t ja1_direction(int32_t x1, int32_t y1, int32_t x2, int32_t y2);   /* soldier_update.md Direction 1..8 (pixel points) */
void    ja1_end_task(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);   /* = library op 50 (combat.md End of a task) */
int32_t ja1_move_ap(Ja1World *w, const Ja1Host *h, int32_t slot);                 /* = library op 13 (combat.md Walking cost) */
void    ja1_sector_tick(Ja1World *w, const Ja1Host *h);                  /* = library op 160 (turns.md Sector tick) */
void    ja1_explode(Ja1World *w, const Ja1Host *h, int32_t thrower, int32_t tile, int32_t kind, int32_t hit);  /* = library op 90 */
void    ja1_effect_hit(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t kind, int32_t power);  /* combat.md Effect hit: the front-end calls it */
void    ja1_explode_dat(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t big);         /* = library op 336 explode_art (combat.md Craters and blast leftovers) */
void    ja1_spend_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t amount, int32_t tire);   /* = library op 31 */
void    ja1_rest(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t amount, int32_t redraw);
int32_t ja1_new_turn(Ja1World *w, const Ja1Host *h, int32_t focus);     /* = library op 134 */
void    ja1_next_turn(Ja1World *w, const Ja1Host *h, int32_t mode);      /* = library op 114 */
void    ja1_interrupt_turn(Ja1World *w, const Ja1Host *h);               /* = library op 113 */
/* shooting.md */
void    ja1_launch(Ja1World *w, const Ja1Host *h, int32_t slot);            /* = library op 81 */
void    ja1_projectile_step(Ja1World *w, const Ja1Host *h, int32_t slot);   /* = library op 136 */
void    ja1_fire_now(Ja1World *w, const Ja1Host *h, int32_t slot);          /* = library op 39 */
void    ja1_shot_end(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t silent, int32_t notify);
void    ja1_projectile_impact(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t x, int32_t y,
                              int32_t hit);
void    ja1_take_damage(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t height, int32_t damage, int32_t kind,
                        int32_t attacker, int32_t tile);                 /* = library op 110 */
void    ja1_hit_reaction(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t attacker, int32_t flag, int32_t damage);
void    ja1_hit_alert(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent, int32_t flag);
void    ja1_mark_seen(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent);
int32_t ja1_bullet_damage(Ja1World *w, const Ja1Host *h, int32_t slot);                          /* = library op 175 */
int32_t ja1_hit_location(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t height, int32_t damage);   /* op 180 */
void    ja1_shot_alert(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);                /* = library op 173 */
void    ja1_interrupt_check(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent, int32_t old);   /* op 182 */
int32_t ja1_ask_merc_interrupt(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent);   /* library op 128 (combat.md Interrupt contests) */
void    ja1_swap_hand(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t a, int32_t b);   /* library op 16 (ai.md Swap hands) */
int32_t ja1_busy(Ja1World *w, const Ja1Host *h, int32_t slot);   /* library op 27 (ai.md Soldier busy) */
int32_t ja1_use_cost(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t spot, int32_t tile, int32_t g, int32_t kind);   /* library op 121 (combat.md Use cost, exact steps) */
int32_t ja1_ask_interrupt(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent);        /* library op 116 */
void    ja1_kill(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t quiet);             /* = library op 78 */
void    ja1_merc_dies(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t quiet);        /* = library op 82 */
void    ja1_forget_soldier(Ja1World *w, const Ja1Host *h, int32_t slot);
/* combat.md Doors */
int32_t ja1_door_lookup(Ja1World *w, const Ja1Host *h, int32_t tile);                              /* = library op 179 */
void    ja1_break_object(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t object, int32_t sound);  /* = library op 107 */
void    ja1_close_door(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t object, int32_t sound);    /* = library op 104 */
void    ja1_door_fx(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t task);          /* = library op 70 */
/* combat.md Reload */
int32_t ja1_reload(Ja1World *w, const Ja1Host *h, int32_t slot);                                   /* = library op 190 */
/* ai.md Noise */
void    ja1_make_noise(Ja1World *w, const Ja1Host *h, int32_t src, int32_t tile, int32_t cls, int32_t loud,
                       int32_t kind);                                                              /* = library op 127 */
void    ja1_hears(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t strength, int32_t kind);  /* op 211 */
void    ja1_noise_interrupt(Ja1World *w, const Ja1Host *h, int32_t slot);                          /* = library op 212 */
```

## Soldier update entry points

```c
/* soldier_update.md */
void ja1_soldier_frame(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_active_frame(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_walk_step(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_walk_blocked(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t occupant);
void ja1_walk_end(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t code);
void ja1_stand_up(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_frame_checks(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_merc_quote(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_enemies_see(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_leave_path(Ja1World *w, const Ja1Host *h);
void ja1_leave_sector(Ja1World *w, const Ja1Host *h, int32_t flag);
void ja1_end_of_day(Ja1World *w, const Ja1Host *h);
int32_t ja1_connected_home(Ja1World *w, const Ja1Host *h, int32_t sector);
/* soldier_update.md Traverse to a neighbour sector: dir 10 N, 11 E, 12 S, 13 W; out = 3 int16 (entry tile, new
   sector, facing). Host ops 54, 57, 125, 214, 89, 215. */
int32_t ja1_traverse_edge(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t dir);
int32_t ja1_traverse_check(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t dir, int16_t *out);
int32_t ja1_traverse(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t dir, int16_t *out);
void ja1_traverse_out(Ja1World *w, const Ja1Host *h, int32_t dest);
int32_t ja1_traverse_in(Ja1World *w, const Ja1Host *h, int32_t theme, int32_t playing);
```

Batch 1 uses only `random` (no host call op).

## Inventory screen entry point

```c
/* inventory.md: the screen loop for the soldier in `slot` 0..47 (a merc; any soldier with cursor.md "Debug keys
   (level 4)" Shift + left click) (redraw != 0: item_info + body first).
   0 = the player left the screen, 1 = op 221 inv_poll said stop. */
int32_t ja1_inventory_screen(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t redraw);
```

Host calls of inventory.md -> ops: show 224, hide 145, cursor 225, cursor2 226, click 227, sfx 60, w_4b730 228,
w_4b764 229, item_value 230, item_info 231, body 232, tip_item 233, tip_merc 234, w_682ac 235, text_toggle 236,
message 125, inv_extra 237, draw_slot 238, vestmap 239, merc_text 57 (1 = "doesn't have enough Action Points!",
2 = "has reloaded."), slot_redraw 149, no_room 240, spend_ap 31, rest 241, reserve_ap 58, select 242, w_2b6a0
243, w_55fd8 244, w_467e4 245, w_28698 246, mine_spotted 247, disarm 248, pick_up 249, focus 99, put_item 250,
merge_prompt 251, quote 252, yes_no 253, stat_practice 105, delay 37, random = the host's random. Every poll of
inventory.md ("poll", "wait up") is op 221. Other world data used: soldiers, tiles (byte 6 = ground item index),
ground_items, item_types, weapons (byte 1 of a gun type = its ammo type), attach (the merge table: +0 cursor type,
+1 hand type, +2 result, +3 kind), sectors (+0xAF = mine count of a sector, 0xB1 bytes each), cur_sector,
selected, combat, key_shift, key_ctrl, opt_step_text (the text flag).

## Assignment screen item press entry point

```c
/* strategic.md "Item moves on the item panel": one left-button press on the item panel of merc `slot` that the face
   and the panel buttons did not answer. *armour = the armour value of the selection (in / out).
   0 = done (the left button is up again), 1 = op 221 inv_poll said stop. */
int32_t ja1_assign_item_press(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t *armour);
/* strategic.md "Key E: all items into the store": the key E on the assignment screen with merc `slot` selected.
   Host ops stash_put 264 and assign_panel 321; world data soldiers, item_types, stash_counts. */
void ja1_assign_key_e(Ja1World *w, const Ja1Host *h, int32_t slot);
/* strategic.md "Firing a merc", steps 5 and 6: the answer yes to "Fire <nickname>?" for the merc of team `slot`
   (the duty menu's box x). Host op fire_quote 328 and those of Merc leaves / Native leaves / Store an item; random =
   the host's random. World data soldiers, profiles, accounts (0xA8 bytes), item_types, stash_counts, listed.
   The host then reloads the panel sheet and calls ja1_team_compact(0) and ja1_calc_ledger_all. */
void ja1_fire_merc(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t x);
/* Test entry point: Fire the merc alone (soldier `slot`, balloon place (x, y)). */
void ja1_fire_merc_core(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t x, int32_t y);
/* strategic.md "Right button on the item panel": one right-button press (left button up) with merc `slot`
   selected. Host ops as ja1_assign_item_press (item_info 231 too). 0 = done, 1 = op 221 inv_poll said stop. */
int32_t ja1_assign_right_press(Ja1World *w, const Ja1Host *h, int32_t slot);
/* strategic.md "The guard screen" (key M on the tactical screen): Ja1GuardScreen is defined there. Host ops 89
   replay_event, 43 connected_home (library op), 82 merc_dies (library op); world data sector_state, soldiers,
   cur_sector, day_minutes, guard_eta, guard_rec. */
int32_t ja1_guard_open(Ja1World *w, const Ja1Host *h, Ja1GuardScreen *g);
int32_t ja1_guard_hover(Ja1World *w, const Ja1Host *h, Ja1GuardScreen *g, int32_t sector);
int32_t ja1_guard_click(Ja1World *w, const Ja1Host *h, Ja1GuardScreen *g, int32_t sector, int32_t buttons, int32_t ctrl);
int32_t ja1_guard_exit(Ja1World *w, const Ja1Host *h, Ja1GuardScreen *g);
void    ja1_guard_close(Ja1World *w, const Ja1Host *h, Ja1GuardScreen *g);
```

Host ops as for the inventory screen (poll 221, hide 145, show 224, cursor 225, click 227, sfx 60, merc_text 57,
inv_extra 237, draw_slot 238 (on the screen, with the current item places table), vestmap 239, no_room 240,
message 125, merge_prompt 251, quote 252, yes_no 253, stat_practice 105, w_467e4 245, w_28698 246), stash_put 264,
and 321..325. The host sets key_shift / key_ctrl before the call; the polls inside do not change them. World data:
soldiers, inv_cursor, inv_swap, vest_owner, item_rects (the assignment screen's places 0..13), item_types, weapons,
attach, stash_counts, opt_step_text, mouse_x / mouse_y / mouse_buttons.

## Not enough funds entry point

```c
/* strategic.md Not enough funds: 0 = the dialog ended, 1 = op 221 inv_poll said stop.
   World: soldiers, profiles, accounts (0xA8), opt_step_text, voice_test, tip_item, mouse_x, mouse_y,
   mouse_buttons and the fields of Merc leaves, Close up the team and Pay the day. */
int32_t ja1_payday_dialog(Ja1World *w, const Ja1Host *h);
```

Host ops: inv_poll 221, mouse_off 145, mouse_show 224, inv_cursor_pic2 226, tip_item 233, tip_merc 234, 246 (int
0x25: the end of the quote's wait), island_map 268, runs_off_end 269, the ops of Merc leaves / Close up the team
(263, 264, 265) and 270..280.

## Morning duties entry point

```c
/* strategic.md Morning duties. World: soldiers, item_types. Host op 281 practice; random = the host's random. */
void ja1_morning_duties(Ja1World *w, const Ja1Host *h);
```

## Morning stat changes entry point

```c
/* strategic.md Morning stat changes. World: soldiers, stat_progress, stat_changed. Host ops 282..287. */
void ja1_stat_changes(Ja1World *w, const Ja1Host *h);
```

## Stat practice entry point

```c
/* combat.md Stat practice (exact steps): the work of host op 105 stat_practice (slot, stat, n, flag). World:
   soldiers, stat_progress. No host op; random = the host's Random. Op 105 stays a host call: a front-end answers it
   with this function on the world of the running call. */
void ja1_stat_practice(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t stat, int32_t n, int32_t flag);
```

## AIM rate complaints entry points

```c
/* strategic.md AIM rate complaints. World: soldiers, profiles, accounts. Host ops 288..290; random = the host's random. */
void ja1_complain_pick(Ja1World *w, const Ja1Host *h);
/* the same for speaker slot `slot` (no random) */
void ja1_complain(Ja1World *w, const Ja1Host *h, int32_t slot);
/* strategic.md A disliked merc in the team. World: soldiers, profiles. Host op 291. */
void ja1_hated_check(Ja1World *w, const Ja1Host *h);
```

## A.I.M. entry points

```c
/* strategic.md The A.I.M. screen. World: soldiers, profiles, accounts (0xA8), difficulty, and the fields of A merc
   joins the team, Firing a merc and Close up the team. Random = the host's random. Host ops: 337 aim_remark
   (profile, m: the named merc's name sample into the VOC buffer; the host keeps m for "MEMBER" in the quote text),
   338 aim_removed (slot: the tooltip goes, then 500 ms), and the ops of Merc leaves / Close up the team (263, 264, 265, 266). */
int32_t ja1_aim_open(Ja1World *w, const Ja1Host *h);              /* Opening steps 1, 2; result t */
int32_t ja1_aim_browse(Ja1World *w, const Ja1Host *h, int32_t q, int32_t d);   /* P (also accounts +0xA4) or -1 */
int32_t ja1_aim_refusal(Ja1World *w, const Ja1Host *h, int32_t p);  /* the refusal reason (0 = signs) */
int32_t ja1_aim_hire(Ja1World *w, const Ja1Host *h, int32_t p);     /* Hire without the quote: team slot or -1 */
void ja1_aim_cancel_pending(Ja1World *w, const Ja1Host *h);         /* remove every merc with b19D = 1 */
int32_t ja1_aim_pay(Ja1World *w, const Ja1Host *h);  /* the charge paid: arrivals, funds, link, b19D, hirings; n */
```

## Back on duty and profile sync entry points

```c
/* strategic.md Back on duty. World: soldiers, duty7_merc, team_on_duty. No host ops. */
void ja1_duty_wake(Ja1World *w, const Ja1Host *h);
/* strategic.md Profiles take the team's stats. World: soldiers, profiles. No host ops. */
void ja1_profile_sync(Ja1World *w, const Ja1Host *h);
```

## Corpse export entry point

```c
/* strategic.md Corpse export. World: soldiers, profiles, accounts, difficulty, stash_counts, item_types, listed.
   Host ops 292..294 and the ops of Merc leaves / Close up the team (263, 264, 265, 266); random = the host's random. */
void ja1_corpse_export(Ja1World *w, const Ja1Host *h);
```

## Salary raise entry point

```c
/* strategic.md Salary raise. World: soldiers, profiles, accounts and the fields of Merc leaves / Close up the team.
   Host ops 295..300 and 263..266. */
void ja1_raise_salary(Ja1World *w, const Ja1Host *h);
```

## The morning entry point

```c
/* strategic.md The morning: 0 = done, 1 = the Not enough funds dialog was stopped (op 221 inv_poll).
   World: the fields of every morning section. Host op 301 and the ops of those sections. */
int32_t ja1_morning(Ja1World *w, const Ja1Host *h);
/* strategic.md Sector click on the island map. World: sector_state, cur_sector, team_on_duty, accounts and the
   fields of All accounts. Host op 302. Return = the message number (0 = none). */
int32_t ja1_map_click(Ja1World *w, const Ja1Host *h, int32_t sector, int32_t mode, int32_t buttons, uint8_t *warned);
int32_t ja1_map_start(const Ja1World *w, int32_t button, uint8_t *hinted);   /* 0, 14, 15 or 16 */
/* strategic.md Day start. Host op 303. Return = 1 when the day starts, 0 when the player says no. */
int32_t ja1_day_start(Ja1World *w, const Ja1Host *h);
/* strategic.md Sunrise: day + 1, stat_changed cleared, overnight healing. Host op 304. */
void ja1_sunrise(Ja1World *w, const Ja1Host *h);
/* strategic.md Strategic screen opening (tapper limit, canteens, overnight departures, cash, worker pool).
   Host ops 302, 305, 306 and those of the steps. */
void ja1_strategic_open(Ja1World *w, const Ja1Host *h);
/* strategic.md Story events at the day start (the first step of ja1_strategic_open). Host ops 307, 308. */
void ja1_story_day(Ja1World *w, const Ja1Host *h);
/* The story-object find scene (0x6179C, port/ja1logic/src/storyfind.rs): a merc saw a story tile. The front-end
   calls it when story_finder < 0x40 and that soldier's hp >= 15, then sets story_finder = 0x40. */
void ja1_story_find(Ja1World *w, const Ja1Host *h);
/* A hit on a story object (StoryHit? 0x61D28, port/ja1logic/src/storyfind.rs; library op 177): story[0x15] = 1;
   in the current sector's 5 story slots 0xAE -> 0xAC and 0xAF -> 0xAD, and a slot with 0xAC / 0xAD puts that object
   on its tile (byte 1); then mission_step = radio_result (op 33), mission_slot = 0x40. */
void ja1_story_hit(Ja1World *w, const Ja1Host *h);
/* strategic.md The guide's advice (the last step of ja1_strategic_open). Host ops 302, 307, 317..320. */
void ja1_guide_advice(Ja1World *w, const Ja1Host *h);
/* turns.md The day clock (library op 159; host ops 309..314, 89, 112, 218). Result 1 = the day is over. */
int32_t ja1_day_clock(Ja1World *w, const Ja1Host *h, int32_t minutes);
/* turns.md Time compression (host ops 198, 315, 316). */
void ja1_time_compress(Ja1World *w, const Ja1Host *h, int32_t on);
/* strategic.md Enemy moves: Enemy pick (library op 310; result = a sector) and Enemy move (library op 311; host ops
   9, 89, 218, 329 (a library op); result = the enemies who came into a sector with mercs). */
int32_t ja1_enemy_pick(Ja1World *w, const Ja1Host *h);
int32_t ja1_enemy_move(Ja1World *w, const Ja1Host *h, int32_t src);
/* strategic.md Walk-ins: Edge tile (result a tile or -1), Guard enters (library op 312) and Enemy walks in
   (library op 329); results 1 = he came in, 0 = not (host op 51). */
int32_t ja1_edge_tile(Ja1World *w, const Ja1Host *h, int32_t edge);
int32_t ja1_guard_enter(Ja1World *w, const Ja1Host *h, int32_t edge);
int32_t ja1_enemy_walk_in(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t edge);
/* strategic.md Sector battles: the battle in sector s (library op 218, f as there; host ops 332..335, 89, 316) and
   Sector gained (library op 201, s = the s8 sector; host op 89). */
void ja1_resolve_battle(Ja1World *w, const Ja1Host *h, int32_t s, int32_t f);
void ja1_sector_gained(Ja1World *w, const Ja1Host *h, int32_t s);
```
