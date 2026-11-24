#ifndef JA1LOGIC_H
#define JA1LOGIC_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* terrain move code of one tile from its layers (spec: Movement, first list) */
uint16_t ja1_move_code(uint8_t ground, uint8_t object, uint8_t roof, uint16_t previous,
                       const uint8_t ground_class[256], const uint8_t object_class[256]);
/* rebuild the whole table (spec: Rebuilding all move codes). tiles = 3136 x 8-byte tile structs, row-major 56x56 */
void ja1_rebuild_move_codes(const uint8_t *tiles, const uint8_t ground_class[256],
                            const uint8_t object_class[256], uint16_t move_codes[3136]);
/* AP to enter one tile (spec: "AP to enter a tile"); returns the value as a signed byte widened to int32 */
int32_t ja1_step_ap(uint16_t move_code, uint8_t sneak, uint8_t byte177, uint8_t alt);
/* A* path (spec: Finding the path). occupant[t] = soldier slot on tile t, 64 = none. slot = the walker's slot,
   combat = enemies near flag. Returns 1 if a path was found, else 0. On return dirs_out holds up to the first 80
   directions (0 = north, clockwise) and *count_out the step count, as the spec describes the stored path. */
int32_t ja1_find_path(int32_t start, int32_t dest, const uint16_t move_codes[3136], const uint8_t ground[3136],
                      const uint8_t occupant[3136], uint8_t byte177, uint8_t byteB9, uint8_t slot, uint8_t combat,
                      uint8_t dirs_out[80], uint16_t *count_out);

/* ---- Slice 2a: enemy AI decisions (docs/spec/port_api.md, docs/spec/ai.md) ---- */
typedef struct Ja1World {
    uint8_t *soldiers;           /* 48 records x 512 bytes, layout "soldier" */
    uint8_t *tiles;              /* 3136 x 8 bytes, layout "tile" */
    const uint8_t *item_types;   /* 136 x 12 bytes, layout "itemType" */
    const uint8_t *anim_table;   /* 256 rows x 8 bytes: col 0 can act, 1 can attack, 2 steps on when idle */
    uint8_t *sector_state;       /* 80 x 9 bytes, layout "sectorState" */
    uint8_t *mission_done;       /* 80 bytes */
    uint8_t cur_sector;
    uint8_t alarm;
    uint8_t combat;
    uint8_t difficulty;
    int8_t  mission_step;
    uint8_t mission_slot;
    uint16_t mission_target;
    uint16_t mission_spot;
    int32_t turn_based;
    int32_t busy;
    uint8_t active_slot;
    uint8_t santino_spoke;
    int16_t santino_timer;
    uint8_t show_ai;
    uint32_t santino_word;
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
    uint8_t *remember_e;         /* 8 x 40 bytes: enemy index x opponent slot -> != 0 remembers that opponent
                                    (sector start batch 8 clears it) */
    /* slice 2b part 2, batch 2 (Sector value in ai.md): */
    uint8_t *story;              /* 43 bytes: the story block (story flags read by Sector value; sector start
                                    batch 8 writes byte 0x17) */
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
    uint8_t *text_buf;           /* 32 bytes: the patch file path text (0-terminated) */
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
    uint8_t safe_opened;         /* != 0: the safe in sector 0x0B is open (read only) */
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
    uint8_t opt_reload_msg;      /* option: the "out of ammo" quote */
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
    const uint8_t *fx_anims;     /* 16 x 0x12 bytes: effect animations, byte 0 = count left (s8) */
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
    /* aiming (shooting.md batch 5), read-only: */
    const uint8_t *posture;      /* 256 bytes, one per animation code: >= 6 = can block a shot not aimed at it */
    const uint8_t *window;       /* 2056 bytes: window[8 x object + direction] (i8): how a wall lets a projectile pass */
    const uint8_t *attack_sound; /* 5 x i16, one per weapon class 0..4: the sound argument of a shot's Spend AP */
    const uint8_t *diff_mod;     /* 3 x i16, one per difficulty: the enemies' to-hit bonus */
    /* click (click.md): */
    uint8_t key_ctrl;            /* Ctrl is held: no panel update after a click */
    /* turn start (turns.md): */
    int32_t fast_timer;          /* ms of real time left after a turn start; at 0 turn_based comes back on */
    /* selecting a merc (select.md): */
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
} Ja1World;   /* 848 bytes on x86-64 */

typedef struct Ja1Host {
    void *ctx;
    int32_t (*random)(void *ctx, int32_t n);
    uint32_t (*call)(void *ctx, int32_t op, const int32_t *args, int32_t nargs, uint8_t *out, int32_t out_size);
    /* bit n set: op n goes to the host even when the library implements it (a test that scripts that op).
       Library ops: 2, 3, 4, 5, 8, 9, 10, 11, 12, 17, 18, 20, 21, 22, 23, 24, 25, 26, 33, 42, 44, 45, 51, 52, 56, 58, 61,
       62, 84, 97, 98, 109, 119, 113, 114, 131-134, 140, 141, 31, 81, 136, 39, 110, 173, 175, 180, 182, 78, 82, 46, 48, 79, 80, 30, 15, 14, 68, 123, 120 and the others of port_api.md "Library ops";
       bit 63 covers op 63 and every higher op. */
    uint64_t host_ops;
} Ja1Host;

/* host call ops (port_api.md host table) */
enum {
    JA1_OP_IDLE = 1, JA1_OP_NOISE_TILE = 2, JA1_OP_FIND_DRY_TILE = 3, JA1_OP_PATROL_TILE = 4,
    JA1_OP_BUDDY_TILE = 5, JA1_OP_DIRECTION = 6, JA1_OP_TILE_DIST = 7, JA1_OP_NEWS_VALUE = 8,
    JA1_OP_SECTOR_VALUE = 9, JA1_OP_APPROACH_TILE = 10, JA1_OP_ALLY_CALL_TILE = 11, JA1_OP_SEARCH_TILE = 12,
    JA1_OP_MOVE_AP = 13, JA1_OP_REACH_AP = 14, JA1_OP_ATTACK_READY = 15, JA1_OP_SWAP_HAND = 16,
    JA1_OP_ACTION_AP = 17, JA1_OP_CAN_STAND_AT = 18, JA1_OP_MISSION_CANCEL = 19, JA1_OP_OPP_STRENGTH = 20,
    JA1_OP_RETREAT_TILE = 21, JA1_OP_ADVANCE_TILE = 22, JA1_OP_TARGET_TILE = 23, JA1_OP_GUN_OPTION = 24,
    JA1_OP_THROW_OPTION = 25, JA1_OP_KNIFE_OPTION = 26, JA1_OP_BUSY = 27, JA1_OP_TURN = 28,
    JA1_OP_WALK_TO = 29, JA1_OP_CLICK = 30, JA1_OP_SPEND_AP = 31, JA1_OP_WAIT = 32,
    JA1_OP_RADIO_RESULT = 33, JA1_OP_SHARE_SIGHTINGS = 34, JA1_OP_PERCEIVE = 35, JA1_OP_REDRAW = 36,
    JA1_OP_DELAY = 37, JA1_OP_SANTINO_SPEECH = 38, JA1_OP_FIRE = 39, JA1_OP_PATH_AP = 40,
    JA1_OP_FIND_PATH = 41, JA1_OP_FIRE_LINE = 42,
    JA1_OP_CONNECTED_HOME = 43, JA1_OP_QUEST_CHECK = 44, JA1_OP_SHOT_AP = 45, JA1_OP_CHANCE_TO_HIT = 46,
    JA1_OP_DIV_ZERO = 47, JA1_OP_HIT_TEST = 48, JA1_OP_SIGHT_LINE = 49,
    /* soldier update (soldier_update.md) */
    JA1_OP_ANIM_NEXT = 97, JA1_OP_WALK_BLOCKED = 98, JA1_OP_FOCUS = 99, JA1_OP_CRY = 100, JA1_OP_VOICE = 101,
    JA1_OP_RELOAD_CHECK = 102, JA1_OP_QUOTE_EVENT = 103, JA1_OP_CLOSE_DOOR = 104, JA1_OP_STAT_PRACTICE = 105,
    JA1_OP_CONSUME_TRIGGER = 106, JA1_OP_BREAK_OBJECT = 107, JA1_OP_TRIGGER_ITEM = 108,
    JA1_OP_TAKE_GROUND_ITEM = 109, JA1_OP_TAKE_DAMAGE = 110, JA1_OP_ENEMY_STEP = 111, JA1_OP_TAC_REDRAW = 112,
    JA1_OP_MERC_INTERRUPT = 113, JA1_OP_NEXT_TURN = 114, JA1_OP_SIGHT_CHECK = 115, JA1_OP_ASK_INTERRUPT = 116,
    JA1_OP_THROW_ITEM = 117, JA1_OP_MINE_FLAG = 118, JA1_OP_AI_THINK = 119, JA1_OP_LOW_AP_STOP = 120,
    JA1_OP_USE_COST = 121, JA1_OP_THROW_FOLLOW = 122, JA1_OP_TURN_BANNER = 123, JA1_OP_NEAREST_MERC = 124,
    JA1_OP_MESSAGE = 125, JA1_OP_MOVE_SOUND = 126,
    /* turns (turns.md) */
    JA1_OP_ITQ_POP = 131, JA1_OP_TURN_HAND_OVER = 132, JA1_OP_ROUND_END = 133, JA1_OP_NEW_TURN = 134,
    JA1_OP_MOVE_ORDER = 140, JA1_OP_START_TURN = 141, JA1_OP_MOUSE_HIDE = 142, JA1_OP_DESELECT = 143,
    JA1_OP_PANEL_REDRAW = 144, JA1_OP_MOUSE_OFF = 145, JA1_OP_MOUSE_MOVE = 146, JA1_OP_MOUSE_SAVE = 147,
    JA1_OP_PANEL_SLOTS = 148, JA1_OP_PORTRAIT_REFRESH = 149, JA1_OP_TIMED_EFFECT = 150, JA1_OP_AID_TICK = 151,
    JA1_OP_TURN_REST = 152, JA1_OP_WOUND_BAR = 153, JA1_OP_ENERGY_BAR = 154, JA1_OP_MERC_PANEL = 155,
    JA1_OP_PANEL_UPDATE = 156, JA1_OP_MOUSE_REFRESH = 157, JA1_OP_TURN_UI = 158, JA1_OP_GUARDS_ARRIVE = 159,
    JA1_OP_SECTOR_TICK = 160, JA1_OP_GAS_TICK = 161, JA1_OP_MOUSE_CHECK = 162, JA1_OP_CURSOR_RESET = 163,
    JA1_OP_VIEW_SAVE = 164, JA1_OP_VIEW_RESTORE = 165, JA1_OP_ITEM_PANEL = 166, JA1_OP_SPEECH_READY = 167,
    JA1_OP_SECTOR_SPEECH = 168, JA1_OP_STAGE_EFFECTS = 169, JA1_OP_STAGE_FIVE = 170, JA1_OP_REDRAW_KEPT = 171,
    JA1_OP_ENEMY_FOCUS = 172, JA1_OP_SHOT_ALERT = 173, JA1_OP_ITEM_COND = 174, JA1_OP_BULLET_DAMAGE = 175,
    JA1_OP_LAND_ITEM = 176, JA1_OP_STORY_HIT = 177, JA1_OP_PUFF = 178, JA1_OP_DOOR_LOOKUP = 179,
    JA1_OP_HIT_LOCATION = 180, JA1_OP_KNOCKDOWN_ANIM = 181, JA1_OP_INTERRUPT_CHECK = 182,
    JA1_OP_LOSS_MSG = 183, JA1_OP_MERC_SEES = 184, JA1_OP_CLICK_PANEL = 185, JA1_OP_DEATH_SHOW = 186,
    JA1_OP_SECTOR_WON = 187, JA1_OP_SQUAD_LOST = 188, JA1_OP_AIM_DIR = 189, JA1_OP_RELOADED = 190,
    JA1_OP_FIRST_AID = 191, JA1_OP_ATTACK_CURSOR = 192, JA1_OP_VOC = 193, JA1_OP_BANNER = 194,
    JA1_OP_SELECT_LIGHT = 195, JA1_OP_PANEL_VIEW = 196, JA1_OP_SEES_MSG = 197, JA1_OP_MOUSE_WAIT = 198,
    JA1_OP_CURSOR_STATE = 199, JA1_OP_CURSOR_SELECT = 200,
    JA1_OP_SECTOR_GAINED = 201, JA1_OP_WON_QUOTE = 202, JA1_OP_ENEMIES_MSG = 203, JA1_OP_MUSIC = 204,
    JA1_OP_TILE_ANIM = 205, JA1_OP_PAL_CYCLE = 206, JA1_OP_PAL_SET = 207, JA1_OP_STRIP = 208
};

void    ja1_ai_tick(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_ai_update_alert(Ja1World *w, const Ja1Host *h, int32_t slot);
int32_t ja1_ai_choose_action(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t fallback);
int32_t ja1_ai_calm_action(Ja1World *w, const Ja1Host *h, int32_t slot);
int32_t ja1_ai_alerted_action(Ja1World *w, const Ja1Host *h, int32_t slot);
int32_t ja1_ai_combat_action(Ja1World *w, const Ja1Host *h, int32_t slot);
int32_t ja1_ai_engaged_action(Ja1World *w, const Ja1Host *h, int32_t slot);
int32_t ja1_ai_action_affordable(Ja1World *w, const Ja1Host *h, int32_t slot);  /* 1 = affordable */
int32_t ja1_ai_start_action(Ja1World *w, const Ja1Host *h, int32_t slot);       /* 1 = goes on */

/* ---- Slice 2b: search procedures (ai.md "Search procedures"); host_ops applies to their inner calls too ---- */
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

/* ---- Slice 2b part 2: Situation score (ai.md); inner calls 6, 9, 42 are always host calls ---- */
int32_t ja1_ai_opp_strength(Ja1World *w, const Ja1Host *h, int32_t slot);      /* 0..4 */
int32_t ja1_ai_soldier_power(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t flag);
        /* Soldier power of the record in that slot; tile = low 16 bits as s16 (0xC41 = no tile), flag = low byte;
           full 32-bit result */

/* ---- Slice 2b part 2, batch 2: positions and sector value (ai.md) ---- */
int32_t ja1_ai_retreat_tile(Ja1World *w, const Ja1Host *h, int32_t slot);      /* tile, low 16 bits */
int32_t ja1_ai_search_tile(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t mode);  /* mode = low byte */
int32_t ja1_ai_sector_value(Ja1World *w, const Ja1Host *h, int32_t sector, int32_t flag);
        /* sector 0..79, flag = low byte; result -100, 20..100, 250 or 1000 */

/* ---- Slice 2c part 1: attack options (ai.md). The 10-byte option record is written to out. ---- */
void    ja1_ai_gun_option(Ja1World *w, const Ja1Host *h, int32_t slot, uint8_t *out);
void    ja1_ai_knife_option(Ja1World *w, const Ja1Host *h, int32_t slot, uint8_t *out);
/* slice 2c part 2: throw option (ai.md Throw damage / Throw AP / Throw chance / Throw option) */
void    ja1_ai_throw_option(Ja1World *w, const Ja1Host *h, int32_t slot, uint8_t *out);
/* slice 2c part 3: line of fire (ai.md "Line of fire"), library op 42; thrown = low byte; chance in % */
int32_t ja1_ai_line_of_fire(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t thrown);
/* animation: frame delay (ai.md "Frame delay") */
void    ja1_anim_delay(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_anim_step(Ja1World *w, const Ja1Host *h, int32_t slot);   /* ai.md "Next animation" */
void    ja1_anim_end(Ja1World *w, const Ja1Host *h, int32_t slot);    /* ai.md "Animation end" (op 83) */
void    ja1_anim_set(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t code, int32_t keep, int32_t reset);
        /* ai.md "Starting an animation" (library op 52): code = low 16 bits, keep / reset = low bytes */
void    ja1_anim_frame(Ja1World *w, const Ja1Host *h, int32_t slot);   /* ai.md "Frame routine" (library op 61) */
void    ja1_anim_turn(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);  /* "Turn done" (op 56); flag = low byte */
/* ---- Sector start batch 1 (sector_start.md); these use only random (no host call op) ---- */
void    ja1_slot_reset(Ja1World *w, const Ja1Host *h, int32_t record, int32_t slot);  /* "Slot reset" on record */
void    ja1_item_init(Ja1World *w, const Ja1Host *h, uint8_t *item, int32_t cond_mode, int32_t count_mode);
        /* "Item reset" on the 12-byte entry at item; modes = low bytes */
void    ja1_equip(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);       /* "Equipment roll"; flag = low byte */
/* ---- Sector start batch 2: placing soldiers (host ops 51 replace, 58 reserve_ap) ---- */
void    ja1_xra_enemy(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_init_tapper(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t facing);
void    ja1_init_guard(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t facing, int32_t orders,
                       int32_t anchor);
int32_t ja1_guard_tile(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t dir, int32_t dist);   /* a tile */
void    ja1_place_merc(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t facing);
void    ja1_merc_entry(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t facing);
/* ---- Sector start batch 3 (no host call) and batch 4 (host op 85 cursor_back, no arguments) ---- */
void    ja1_put_item(Ja1World *w, const Ja1Host *h, int32_t tile, const uint8_t *item, int32_t b6, int32_t exact,
                     int32_t note, int32_t near);                  /* item = a 12-byte entry, read only */
void    ja1_take_ground_item(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t keep);
void    ja1_take_ground_patch(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t type);
/* ---- Sector start batch 5: soldier start (no host call) ---- */
void    ja1_soldier_start(Ja1World *w, const Ja1Host *h, int32_t record);
/* ---- Sector start batch 6 (host ops 86 open_patch, 87 read, 88 close, 89 replay_event) ---- */
int32_t ja1_tile_patch(Ja1World *w, const Ja1Host *h, int32_t kind, int32_t number, int32_t count, int32_t bl,
                       int32_t cl);
int32_t ja1_item_patch(Ja1World *w, const Ja1Host *h, int32_t kind, int32_t number, int32_t count, int32_t bl,
                       int32_t cl);
void    ja1_crate_lost(Ja1World *w, const Ja1Host *h, int32_t drop);
void    ja1_crate_drops(Ja1World *w, const Ja1Host *h);
void    ja1_plant_sector(Ja1World *w, const Ja1Host *h);
void    ja1_plant_done(Ja1World *w, const Ja1Host *h);
/* ---- Sector start batch 7 (host ops 90 explode, 91 effect, 92 open_tmp; 87 read, 88 close) ---- */
void    ja1_fixed_items(Ja1World *w, const Ja1Host *h);
int32_t ja1_tmp_file(Ja1World *w, const Ja1Host *h);                  /* 1 = read, 0 = no tmp file */
void    ja1_ground_blast(Ja1World *w, const Ja1Host *h, int32_t type, int32_t flag, int32_t power, int32_t thrower,
                         int32_t last);                               /* type, flag, thrower, last = low bytes */
/* ---- Sector start batch 8: the whole sector start (host ops 33, 86..88, 93 sector_resets, 94 keep_selected,
        95 sector_tail, 96 seek; kind 4 of op 86 = DAT\XRA.DAT, text_buf not written) ---- */
void    ja1_setup_sector(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t facing);   /* facing = low byte */
/* ---- Entering a sector (world_load.md section 4: batch 8 host calls plus op 89 replay_event(cur, cur, 0x58)) ---- */
void    ja1_enter_sector(Ja1World *w, const Ja1Host *h, int32_t entry_tile);
/* ---- Sector start batch 9 (ops 51 replace and 58 reserve_ap as library ops; host op 17 action_ap) ---- */
void    ja1_replace_soldier(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_reserve_ap(Ja1World *w, const Ja1Host *h, int32_t slot);
/* ---- Sector start batch 10 (op 33 radio_result as a library op; host op 44 quest_check) ---- */
int32_t ja1_radio_result(Ja1World *w, const Ja1Host *h);   /* the new mission step: a byte 0..4 */
/* ---- Sector start batch 11 (op 44 quest_check as a library op; no host call) ---- */
int32_t ja1_quest_check(Ja1World *w, const Ja1Host *h, int32_t quest);   /* 1 = the condition holds, else 0 */
/* ---- Sector start batch 12 (op 17 action_ap as a library op; host ops 45, 7, 14, 40, 6) ---- */
int32_t ja1_action_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t flag);   /* a byte */
/* ---- Sector start batch 13 (op 45 shot_ap as a library op; only host call 47 div_zero) ---- */
int32_t ja1_shot_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);   /* a byte */
/* ---- Soldier update per frame (soldier_update.md; library ops 97 anim_next, 98 walk_blocked,
        106 consume_trigger, 109 take_ground_item, 111 enemy_step, 117 step_noise, 119 ai_think, 122 throw_follow;
        other host ops 99..129) ---- */
void    ja1_soldier_frame(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_active_frame(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_walk_step(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_walk_blocked(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t occupant);
        /* tile = low 16 bits as s16, occupant = low byte */
void    ja1_walk_end(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t code);   /* code: low byte */
void    ja1_stand_up(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_frame_checks(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_merc_quote(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_enemies_see(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_enemy_step_seen(Ja1World *w, const Ja1Host *h, int32_t slot);        /* op 111 enemy_step */
void    ja1_step_noise(Ja1World *w, const Ja1Host *h, int32_t slot);             /* op 117 step_noise */
void    ja1_consume_trigger(Ja1World *w, const Ja1Host *h, int32_t index);       /* op 106; index 0..9 */
void    ja1_throw_follow(Ja1World *w, const Ja1Host *h, int32_t slot);           /* op 122 throw_follow */
void    ja1_throw_aim(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);        /* tile: signed 16-bit */
void    ja1_throw_scatter(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t margin, int32_t distance);
        /* margin: low byte; distance: signed 16-bit */
int32_t ja1_find_path_op(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t flag);   /* op 41 */
int32_t ja1_path_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile,
                    int32_t sneak, int32_t byte177, int32_t alt);       /* op 40 = sneak, byte177, alt all 0 */
void    ja1_walk_to(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);                      /* op 29 */
void    ja1_idle(Ja1World *w, const Ja1Host *h, int32_t slot);                                       /* op 1 */
/* soldier_update.md Move order, Small ops, One frame, Timer tick */
int32_t ja1_move_order(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t marker,
                       int32_t sneak, int32_t byte177, int32_t alt, int32_t squad);
        /* tile: signed 16-bit; marker, sneak, byte177, alt, squad: low byte (!= 0 = set). Result 1 = walking */
int32_t ja1_direction(int32_t x1, int32_t y1, int32_t x2, int32_t y2);   /* op 6; no world */
int32_t ja1_aim_dir(int32_t x1, int32_t y1, int32_t x2, int32_t y2, uint16_t *word);   /* direction 1..8; word for +0x10A; no world */
int32_t ja1_tile_dist(int32_t a, int32_t b);                             /* op 7; no world */
void    ja1_stop_anim(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);   /* op 53; flag: low byte */
void    ja1_merc_view(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t facing, int32_t range, int32_t think);
        /* op 62 reveal_view; facing, range, think: low byte */
int32_t ja1_frame(Ja1World *w, const Ja1Host *h);    /* result: the 8-bit sum of the op 134 new_turn results */
void    ja1_tick(Ja1World *w, const Ja1Host *h);     /* Timer tick: call it every 10 ms of game time */
int32_t ja1_update_end(Ja1World *w, const Ja1Host *h);   /* Update end: result 0 or 1 (added to the leave counter) */
void    ja1_fuses(Ja1World *w, const Ja1Host *h);        /* Fuses: after every pass (update and draw) */
int32_t ja1_loop_head(Ja1World *w, const Ja1Host *h);    /* Loop head: start of every pass; result = 8-bit sum of new_turn results */
int32_t ja1_leave_tests(Ja1World *w, const Ja1Host *h);  /* Leave tests: after Fuses; result 0, 1 or 2 (added to the leave counter) */

/* turns.md (port_api.md "Turn entry points") */
void    ja1_end_turn(Ja1World *w, const Ja1Host *h);
int32_t ja1_first_merc(Ja1World *w, const Ja1Host *h);
void    ja1_turn_sightings(Ja1World *w, const Ja1Host *h, int32_t both);
int32_t ja1_itq_pop(Ja1World *w, const Ja1Host *h);                     /* = library op 131 */
void    ja1_enemies_turn(Ja1World *w, const Ja1Host *h);                /* = library op 133 */
void    ja1_mercs_turn(Ja1World *w, const Ja1Host *h);                  /* = library op 132 */
void    ja1_start_turn(Ja1World *w, const Ja1Host *h, int32_t slot);    /* = library op 141 */
void    ja1_bleed(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_turn_drown(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_turn_rest(Ja1World *w, const Ja1Host *h, int32_t slot);      /* = library op 152 */
/* combat.md First aid, exact steps */
void    ja1_aid_tick(Ja1World *w, const Ja1Host *h, int32_t slot);       /* = library op 151 */
int32_t ja1_first_aid(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t task, int32_t b);   /* = library op 191 */
void    ja1_spend_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t amount, int32_t tire);   /* = library op 31 */
void    ja1_rest(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t amount, int32_t redraw);
int32_t ja1_new_turn(Ja1World *w, const Ja1Host *h, int32_t focus);     /* = library op 134; result: a byte */
void    ja1_next_turn(Ja1World *w, const Ja1Host *h, int32_t mode);      /* = library op 114; mode: low byte */
void    ja1_interrupt_turn(Ja1World *w, const Ja1Host *h);               /* = library op 113 */
/* shooting.md */
void    ja1_launch(Ja1World *w, const Ja1Host *h, int32_t slot);            /* = library op 81 */
void    ja1_projectile_step(Ja1World *w, const Ja1Host *h, int32_t slot);   /* = library op 136 */
void    ja1_fire_now(Ja1World *w, const Ja1Host *h, int32_t slot);          /* = library op 39 */
void    ja1_shot_end(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t silent, int32_t notify);   /* low bytes */
void    ja1_projectile_impact(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t x, int32_t y,
                              int32_t hit);   /* tile: low 16 bits, signed */
void    ja1_take_damage(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t height, int32_t damage, int32_t kind,
                        int32_t attacker, int32_t tile);   /* = library op 110; kind, attacker: low byte; tile: low 16 bits */
void    ja1_hit_reaction(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t attacker, int32_t flag, int32_t damage);
void    ja1_hit_alert(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent, int32_t flag);
void    ja1_mark_seen(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent);
/* shooting.md batch 3 */
int32_t ja1_bullet_damage(Ja1World *w, const Ja1Host *h, int32_t slot);                     /* = library op 175 */
int32_t ja1_hit_location(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t height, int32_t damage);  /* op 180 */
void    ja1_shot_alert(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);           /* = library op 173 */
void    ja1_interrupt_check(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent, int32_t old);   /* op 182 */
int32_t ja1_ask_merc_interrupt(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent);   /* library op 128 (combat.md Interrupt contests) */
int32_t ja1_ask_interrupt(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent);        /* library op 116 */
/* shooting.md batch 4 */
void    ja1_kill(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t quiet);        /* = library op 78 kill */
void    ja1_melee_hit(Ja1World *w, const Ja1Host *h, int32_t slot);                          /* = library op 77 melee_hit */
void    ja1_revive_roll(Ja1World *w, const Ja1Host *h, int32_t slot);                        /* = library op 76 revive_roll */
void    ja1_merc_dies(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t quiet);   /* = library op 82 merc_dies */
void    ja1_forget_soldier(Ja1World *w, const Ja1Host *h, int32_t slot);
/* shooting.md batch 5 */
void    ja1_start_attack(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);   /* = library op 79 */
void    ja1_attack_go(Ja1World *w, const Ja1Host *h, int32_t slot);                  /* = library op 80 */
int32_t ja1_chance_to_hit(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t aim);  /* op 46 */
int32_t ja1_hit_test(Ja1World *w, const Ja1Host *h, int32_t height, int32_t x, int32_t y, int32_t flown,
                     int32_t direction, int32_t skip, int32_t cap, int32_t target, int32_t roll,
                     uint8_t *out);                                                 /* = library op 48; out: 8 B */
void    ja1_fire_gun(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);
/* click.md */
int32_t ja1_click(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t item);   /* = library op 30 */
int32_t ja1_attack_ready(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);          /* = library op 15 */
int32_t ja1_reach_tile(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t flag);  /* op 14 */
void    ja1_gun_jam(Ja1World *w, const Ja1Host *h, int32_t slot);                             /* = library op 68 */
int32_t ja1_jam_check(Ja1World *w, const Ja1Host *h, int32_t slot);
/* turns.md Turn start */
void    ja1_turn_start(Ja1World *w, const Ja1Host *h);                   /* = library op 123 turn_banner */
void    ja1_ally_alert(Ja1World *w, const Ja1Host *h, int32_t slot);
/* combat.md Doors, Reload */
int32_t ja1_door_lookup(Ja1World *w, const Ja1Host *h, int32_t tile);                 /* = library op 179 */
void    ja1_break_object(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t object, int32_t snd);  /* op 107 */
void    ja1_close_door(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t object, int32_t snd);    /* op 104 */
void    ja1_door_fx(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t task);        /* op 70 */
int32_t ja1_reload(Ja1World *w, const Ja1Host *h, int32_t slot);                      /* = library op 190 */
/* ai.md Noise */
void    ja1_make_noise(Ja1World *w, const Ja1Host *h, int32_t src, int32_t tile, int32_t cls, int32_t loud,
                       int32_t kind);                                                 /* = library op 127 */
void    ja1_hears(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t strength, int32_t kind);  /* op 211 */
void    ja1_noise_interrupt(Ja1World *w, const Ja1Host *h, int32_t slot);             /* = library op 212 */
void    ja1_escape_stop(Ja1World *w, const Ja1Host *h);   /* world_load.md "Quit": Escape stops the selected merc */
/* select.md */
int32_t ja1_select_merc(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t force, int32_t quote);   /* = library op 120 */
int32_t ja1_select_soldier(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t show, int32_t quote);
int32_t ja1_path_ap_marks(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t mark,
                          int32_t extra, int32_t sneak, int32_t byte177, int32_t alt);
void    ja1_hide_path_marks(Ja1World *w, const Ja1Host *h, int32_t force);
int32_t ja1_on_screen(Ja1World *w, const Ja1Host *h, int32_t slot);   /* OnScreen 0x1FC34: 1 = in the view */
/* strategic.md "Sector click on the island map" (host op 302 guide_say). Returns the message number (0 = none).
   warned: the screen's process warning flag (in/out). */
int32_t ja1_map_click(Ja1World *w, const Ja1Host *h, int32_t sector, int32_t mode, int32_t buttons, uint8_t *warned);
int32_t ja1_map_start(const Ja1World *w, int32_t button, uint8_t *hinted);   /* 0, 14, 15 or 16 */
/* strategic.md "Day start" (host op 303 day_ask). Returns 1 = the day starts, 0 = not. */
int32_t ja1_day_start(Ja1World *w, const Ja1Host *h);
void    ja1_sunrise(Ja1World *w, const Ja1Host *h);          /* strategic.md "Sunrise" (host op 304) */
/* strategic.md "Strategic screen opening" (host ops 302, 305 merc_gone, 306 map_show and those of the steps). */
void    ja1_strategic_open(Ja1World *w, const Ja1Host *h);
/* strategic.md "Story events at the day start" (host ops 307 quest_record, 308 stash_take). */
void    ja1_story_day(Ja1World *w, const Ja1Host *h);
/* The story-object find scene (0x6179C; TacticalLoop when story_finder < 0x40 and its hp >= 15): host ops 99,
   54, 36, 317, 145, 215, 85, 185, 126, 89, 143, 188; library ops 1, 50, 120, 33. */
void    ja1_story_find(Ja1World *w, const Ja1Host *h);
/* A hit on a story object (StoryHit? 0x61D28, library op 177 story_hit): story[0x15] = 1, 0xAE/0xAF -> 0xAC/0xAD in
   the current sector's story slots and on their tiles, mission_step = radio_result, mission_slot = 0x40. */
void    ja1_story_hit(Ja1World *w, const Ja1Host *h);
/* strategic.md "The guide's advice" (step 7 of ja1_strategic_open; host ops 302, 307, 317 brenda_say,
   318 panel_draw, 319 guide_offer, 320 merc_join; world fields story (43 bytes), guides_hired, guide_wait,
   port_api.md). */
void    ja1_guide_advice(Ja1World *w, const Ja1Host *h);
/* turns.md "The day clock" (= library op 159; host ops 309..314, 89, 112, 218; world fields clock_minute ..
   guard_rec, port_api.md). Result 1 = the day is over. */
int32_t ja1_day_clock(Ja1World *w, const Ja1Host *h, int32_t minutes);
/* turns.md "Time compression" (host ops 198, 315, 316). */
void    ja1_time_compress(Ja1World *w, const Ja1Host *h, int32_t on);
#ifdef __cplusplus
}
#endif
#endif
