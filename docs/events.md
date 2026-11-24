# Fight event log (`JA1_EVENTS=1`)

Issue #25 item 5. With the environment variable `JA1_EVENTS=1`, `ja1front` (script mode and the window) prints one
line per fight event on stderr. Each line starts with `ev `. Without the variable the output does not change.

The log only reads the world (port/ja1front/src/events.rs): at each host call, each random number and each end of
a library call it compares the soldier records and the sector table with the copy of the last poll. It never
writes the world and never draws a random number. A script run gives the same world image, stdout and other
stderr lines with and without the variable (validate/gate_events.py part 4). One record comes from outside the
world: port/ja1logic/src/trace.rs keeps each won interrupt contest, because a win and its queue pop can both fall
between two polls. The game never reads this record.

## Recorder (`JA1_RECORD_DIR=DIR`, #69)

port/ja1front/src/record.rs. With `JA1_RECORD_DIR=DIR` the same lines also go to `DIR/events.log` (stderr gets
them only with `JA1_EVENTS=1`). The recorder also keeps `DIR/day<N>.SAV`, a copy of each day start's DAY.SAV
(N = game day; an existing copy is never overwritten, the new one gets `-2`, `-3`, ...), and `DIR/meta.txt`:
`port_commit`, `exe`, `exe_sha256`, `program_start` (unix seconds), then one `start <unix seconds> <mode>` line
per game start (`new game`, `continue`, `quick continue`, `save FILE`). It only reads the world:
validate/record_diff.py runs one script with and without it and compares saves, world, stdout and event lines.

## Names

- `merc K`: slot 0..7 (the player's mercs).
- `native K`: slot 8..39 (guards, tappers, natives, Santino's people).
- `enemy K`: slot 40..47.
- Tiles are 56 x 56 tile indexes (row = tile / 56, column = tile % 56).

## Lines

| Line | When |
|---|---|
| `ev combat on sector S` / `ev combat off sector S` | the combat flag of the world turns on or off (S = current sector) |
| `ev turn N side merc` / `ev turn N side enemy` | the selected soldier changes side in combat; N counts up at each merc turn (a merc interrupt in the enemy turn counts as one, so N is not the round number) |
| `ev interrupt A during B tile T` | A won an interrupt contest (combat.md "Interrupts", ai.md Noise interrupt) while B was the selected soldier; T = A's tile. One line per win: a soldier can win more than once in one moment, and a win with `busy` set hands the turn over later |
| `ev door A tile T open` / `ev door tile T open` | the move code of tile T goes from 1500 (closed) to 25400 (open). A = the soldier whose task tile is T, else the selected soldier within 2 tiles; no A = neither. One line per opening: a door that closes and opens again gives a second line. Closing gives no line |
| `ev stance A posture P` | in a fight, the posture value of a merc or an enemy changes to P = the game's table g_animPosture 0x9300D at its anim word (events.rs `posture`): 2 for anims 0..3, 0x29, 0x2F, 0x30, 0x41..0x43; 3 for 0x3A..0x3C, 0x40; 4 for 5 and 0xF; 5 for 0x11; 0 or 1 for 0x44..0x4D; 6 for the rest. The only proved meaning: 6 and up can block a shot aimed at another soldier. No stand / crouch / lie names: nothing proves them (the game has no prone mode). A knock-down and the way up from it also give lines |
| `ev shot A -> B hit D hp X->Y ap P->Q` | a shot of A at B; B lost D health (X before, Y after); A had P AP before the shot and Q after |
| `ev shot A -> B miss ap P->Q` | a shot of A at B that did not wound B (closed by the next shot of A, the end of the turn or of the fight) |
| `ev shot A -> tile T miss ap P->Q` | a shot at a tile with no soldier |
| `ev hit B by A hp X->Y` | the bullet of A wounded B, a soldier that A did not aim at |
| `ev hurt B hp X->Y` / `ev hurt B hp X->0 tile T` | a wound with no open shot (explosion, bleeding, a blow); a wound to 0 names the tile T (#11186) |
| `ev health B H M W` | health H, total health M (`hpMax`) or unbandaged wounds W (`wounds`) of B changed; M - H - W is the bandaged part (panel.md "Health bar") |
| `ev kill B by A tile T` / `ev kill B tile T` | B dies (in play goes to 0 or health to 0); A = the last shooter that wounded B; T = B's tile (#11186) |
| `ev sees A B` / `ev loses A B` | A's knowledge of B becomes "seen now" (1) or stops being it; mercs of enemies, enemies of mercs |
| `ev drop B item I tile T` | B died; an item of type I in its record (one line per non-empty slot) lies at tile T (#11186) |
| `ev pickup A item I tile T` | the number of items of type I in A's record went up (an item arrived from the ground or a stash; not a slot-to-slot move); T = A's tile (#11186) |
| `ev ammo A out item I` | the gun in A's main hand (item type I) has no rounds left |
| `ev reload A rounds N` | the empty gun in A's main hand has N rounds again (Reload: a clip from a pocket) |
| `ev walk A F -> T` / `ev walk A F -> T water` | at the end of a turn (or of the fight, or of the run): A moved from tile F to tile T; `water` = T has move code 5000 or 3600 |
| `ev wave sector S enemies N` | the enemy count of sector S (0..60) goes up to N, and no enemies left a sector in the same clock minute |
| `ev day N` | the day counter of the world changes to N (sunrise) |
| `ev clock H:MM` | the game clock (as the wristwatch shows it, 12-hour) before the first of the day clock lines below in a new clock minute |
| `ev move src S -> D enemies N` | an enemy move (strategic.md Enemy moves): N enemies left S and came into D in the same clock minute (outside a fight and a sector battle) |
| `ev battle sector S guards G tappers T enemies E mines M` | a sector battle (strategic.md Sector battles) of sector S; the counts at its start (op 332 (0)), M = the mines of the sector record (+0xAF); printed at its result message (op 335) |
| `ev battle end sector S guards G tappers T enemies E owned O` | the end of that battle (op 332 (8)): the counts after it and the owned bit |
| `ev gained sector S` / `ev lost sector S` | the owned bit (flags bit 0) of sector S (0..59) turns on or off |
| `ev quit guards N tappers M` | natives quit (strategic.md Natives quit, op 261): N guards (the guard message), M tappers (the drop of the tappers of all sectors; the game's own tapper message shows the guard count, issue #2 row 13) |
| `ev wage W` | the native wage (Day accounts +0x54) changes to W |

The day clock lines (from `ev day` on) compare the sector table of the first world buffer of the run between polls,
and read the host ops 261, 332 and 335 before the poll of the same call. The day end binds a second world buffer
that it fills from a save: its changes give no lines. An owned bit or an enemy count that a buffer only takes
over (equal to the last line) gives no second line.

A shot runs from Launch (soldier +0x1BF = 3) to its wound: the wound comes after Shot end, in the target's hit
animation. So the hit line can come after other lines of the same moment.

## Week file

`validate/week_normal.py` writes one file per seed: `build/gate/week_normal{TAG}{seed}_events.txt` (TAG = WN_TAG,
mostly empty). Each line starts with `day N ` (the game day), then an `ev` line of the port or a `plan` line of the
script. A run replays from the newest day save, so only the run that reaches the next day save (and the last run
of the seed) is written: each event once. Per day: its `ev day` line, then the `plan` lines, then the other lines in
run order.

`plan` lines (the script's choices, not the port's):
- `plan threat S:N ...`: per owned sector, the enemies in its neighbours.
- `plan attacks S:A/E ...`: WN_OUT -2..-4: per owned sector that the game's enemy move attacks with no guards, the
  attacked runs and the enemies sent per try.
- `plan target S D -> N enemies E (why)` or `plan target none (why)`: the day's attack.
- `plan duties rest [K ...]`: the mercs that rest or take PATIENT.
- `plan guards S:H+A-B ...`: per owned sector the guards it had, the guard clicks and the guards sent back.
- `plan tappers S:H+N ... wage W`: per owned sector the tappers it had and the tappers the clicks add; the wage.
- `plan fight N from S edges D squad K fit F hp [...] medics [...]`: the start of an attack.

## Readers

- `validate/week_normal.py` sets the variable and prints a report per fight: shots, hits, kills, mercs lost, turns,
  mercs out of rounds; it writes the week file.
- `validate/week_events.py SEED [TAG]` prints a per-day summary of a week file: plan, fights, battles, gains and
  losses, quits, wage, enemy moves and waves.
