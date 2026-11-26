# Strategic map: territory

The island map is a grid of 60 sectors, 10 columns by 6 rows, numbered 0..59 row by row from the top (sector
*s* is in column *s* mod 10, row *s* / 10). The player's home base is sector 59 (bottom right). Each sector has a
9-byte state record (`g_sectorTbl9` in `defs/types.toml`); byte 0 holds the flags used here:

- bit 0: owned by the player;
- bits 1, 2, 3, 4 (values 2, 4, 8, 0x10): a barrier on the north, east, south, west side. Barriers are set in
  pairs: the north bit of a sector and the south bit of the sector above it, and so on.

All arithmetic is on bytes unless stated.

## Connected to home

A sector is **connected to home** when a walk over owned sectors leads from it to sector 59. The answer is yes for
sector 59 itself and no for an unowned start. The game does not use a plain search: it uses a wall-follow walk,
and the port must reproduce it exactly, because the walk can give "no" for some owned sectors that a plain search
would join to home.

The walk keeps, for each of the 60 sectors, a copy of byte 0 (the "tried" marks go into this copy, never into the
record), a "done" flag (all clear at the start), and a stack of junction sectors (the start sector first). It also
keeps the current sector (the start) and a direction *d* = 0. Directions: 0 east (+1), 1 south (+10), 2 west (−1),
3 north (−10). The side mark of direction 0, 1, 2, 3 is 4, 8, 0x10, 2.

Repeat:

1. **Pick a side.** Loop: if the copy of the current sector does not have the side mark of *d*, set it and stop
   with "move". Then, in every pass of the loop: if the copy is 0x1E or more (all four marks set, or any of the
   high bits 5..7 set in the record), then
   - if the current sector is not done, mark it done;
   - else remove the top of the stack: if the stack is now empty and the start is done, the answer is **no**; if
     the stack is now empty, the current sector becomes the start and the stack holds the start again; else the
     current sector becomes the junction now on the top (the one just removed is still read from the stack
     storage: the walk goes back to the entry it removed).
   If no side was set in this pass, *d* goes to the next direction (after 3 back to 0). Loop until "move".
2. **Try the step** in direction *d* (the direction of the mark just set). The step is refused when the current
   sector is in column 0 and *d* is west, in column 9 and *d* is east, when the target number is outside 0..59
   (north of row 0 wraps to 246..255 as a byte), or when the target is not owned (record bit 0). Otherwise the
   current sector becomes the target, even if the target is done; if it is not done:
   - set in its copy the side mark of the side it was entered from (moving east marks 0x10, south 2, west 4,
     north 8);
   - count its owned neighbours in the four directions in the real records, **without** edge or range checks (a
     sector in column 9 counts sector + 1 in the next row; numbers outside 0..59 read the bytes just before or
     after the table). With more than 2 owned, push it on the stack and set *d* = 0.
3. If the current sector is 59, the answer is **yes**.

A sector number of 60 or more (the office screen uses 0xFF) has no defined answer in the original (it reads
unrelated stack bytes); the port answers no. The stack holds at most 60 entries in the original's frame; a deeper stack overwrites the done flags. Real maps
never come near this. The walk may also run forever on some inputs that are not real game states; real game
states always end.

## Workable trees

The resource count of a sector (byte 6, trees) is worked only if the sector is owned and connected to home.
Security is 25 per safe side: a side is safe when it is the map edge, or the neighbour on that side is owned, or
the barrier bit of that side is set. The workable count (byte 7, also written) is the lower of byte 6 − byte 8
(trees used or lost) and byte 6 × security / 100 (truncated). An unowned or unconnected sector gets 0.

Evidence: SectorConnectedToHome and SectorWorkable diff-tested in Unicorn against the original
(validate/diff/ai_diff.py `connected`, `workable`; the walk on random tables with high bits and out-of-table
bytes), mutation-tested; SectorWorkable also checked live for sectors 58 and 59 before and after a capture.

## Day accounts

The money and worker totals of the office screen and of the day end. In the port the globals live in the world field `accounts`
(port_api.md), at these offsets: g_teamCount 0x08, g_treePrice 0x10, g_treesWorkable 0x14,
g_treesOwned 0x18, g_treesTapped 0x1C, g_dayIncome 0x20, g_balance 0x24, g_balanceAfterDay 0x28, g_dayNet 0x2C,
g_teamHalfSalaries 0x30, g_teamSalaries 0x34, g_teamPay 0x38, g_workerPool 0x3C, g_tappers 0x40, g_guards 0x44,
g_workers 0x48, g_idlePool 0x4C, g_nativeDeathsTotal 0x50, g_nativeWage 0x54, g_tappersCost 0x58, g_guardsCost
0x5C, g_nativesCost 0x60, g_dayExpenses 0x64, g_sectorsOwned 0x74, g_sectorsGained 0x78, g_sectorsLost 0x7C,
g_processCap 0x84, g_plantsOwned 0x88 (all i32), and the counters "byte at 0x22A031" = u8 at 0x91, "16-bit value
at 0x22A032" = u16 at 0x92, "16-bit value at 0x22A034" = u16 at 0x94. g_teamOnDuty is the world field
`team_on_duty` (not offset 0x0C), g_prevBalance the field `prev_balance`, `g_plants` the field `plants`, `g_sectorOrder` the field `sector_order`. All values are signed 32-bit with
wrap-around unless stated; "connected" means Connected to home (above) for that sector number, called with the
full table each time. Globals are typed in `defs/types.toml`; sector bytes are `sectorState` fields.

**Plants** (`calc_process_cap`). g_plantsOwned = 0, g_processCap = 0. For each of the 4 entries of `g_plants`, in
order: let c = the low byte of `sector`. If `flags` bit 0 of sector c is set (c may be 60..255: it then reads past
the 60-entry table, and the port's Connected to home answers 0; SPEC GAP (not seen: the plant sectors are 59, 26, 53 and 30 from the new game (EXE data, world_load.md) and no step writes them): the original's answer for such a c
is undefined, its walk reads the caller's stack): g_plantsOwned + 1; and if sector c is connected and `state`
= 2: g_processCap += `capacity`.

**Territory** (`recalc_territory`). g_sectorsOwned, g_tappers, g_guards, g_treesOwned, g_treesWorkable,
g_treesTapped = 0. For each sector s = 0..59:
- not owned (`flags` bit 0 clear): `workable` = 0, `guards` = 0, `tappers` = 0;
- owned: g_sectorsOwned + 1, g_tappers += `tappers`, g_guards += `guards`, g_treesOwned += `trees`. If s is not
  connected: `workable` = 0. Else g_treesWorkable += Workable trees (above; it also writes `workable`), and
  g_treesTapped += the lower of `tappers` / 3 (integer) and (`trees` − `unhealthy`) as a byte.

Then if g_processCap < g_treesTapped: g_treesTapped = g_processCap. g_workers = g_tappers + g_guards. g_idlePool =
g_workerPool − g_workers, or 0 when that is below 0.

**Natives cost** (`calc_natives_cost`). g_tappersCost = g_nativeWage × g_tappers, g_guardsCost = g_nativeWage ×
g_guards, g_nativesCost = their sum.

**Team pay** (`calc_team_pay`). Walk soldiers 0..7 in slot order and stop at the first with `inCurSector` = 0. For
each: half = (`salary` + 1) / 2 (integer, from the 16-bit salary); the charge is half when soldier `duty` is 1
or 4, else `salary`; `salary2` = the charge (16 bits). Totals: g_teamCount = soldiers walked, g_teamSalaries = sum
of `salary`, g_teamHalfSalaries = sum of half, g_teamPay = sum of charges, g_teamOnDuty = soldiers with `duty` = 0.

**Ledger** (`calc_ledger`). g_dayIncome = g_treePrice × g_treesTapped, g_dayExpenses = g_nativesCost + g_teamPay,
g_dayNet = g_dayIncome − g_dayExpenses.

**All accounts** (`calc_ledger_all`, the office screen): Plants, Territory, Natives cost, Team pay, Ledger, then
g_balanceAfterDay = g_balance + g_dayNet.

**Pay the day** (`apply_day_ledger`, the day end): Team pay, Ledger, then g_balance = g_prevBalance + g_dayNet.

Evidence (2026-10-23): CalcProcessCap 0x41574, RecalcTerritory 0x50EA8, CalcNativesCost 0x5102C, CalcTeamPay
0x51068, CalcLedger 0x5113C, CalcLedgerAll 0x51178, ApplyDayLedger 0x51210 mirrored in tools/ai.py; ai_diff
`ledger` runs the three entry points (all, apply, territory) with every callee real on both sides over random
sector tables, plants, teams and globals, comparing the globals 0x229FA0..0x22A02F, g_prevBalance, the sector
table, the plants and the 8 soldiers; 0 mismatches; hand mutants (min / max, the half-pay codes, state = 2)
killed.

## Worker pool and natives who quit

**Sectors-gained bonus** (`gained_bonus`). With G = g_sectorsGained (signed): the sum over i = 0, 10, 20, ...,
140 of min(G − i, 10) × (15 − i / 10) for each i < G. (15 per sector for the first 10 gains, 14 for the next 10,
..., 1 for gains 141..150; nothing after that; 0 when G ≤ 0.)

**Worker pool** (`worker_pool`; at new game, in the morning and from the strategic screen). k = `g_difficulty`
as a signed byte. All of the sum below is taken in 16 bits (wrap-around), then read as signed:
- base = 20, 15, 10 for k = 0, 1, 2;
- loss = 15, 20, 25 for k = 0, 1, 2 (SPEC GAP (not seen: the Set Difficulty screen (world_load.md) gives 0, 1 or 2 only; another value needs an edited save): for k outside 0..2 the original reads other data; the port uses
  k = 0);
- W = (the low 16 bits of g_nativeWage) − 35, as signed 16-bit;
- L = (the byte at 0x22A031 + the low 16 bits of g_sectorsLost), as signed 16-bit;
- Q = W × (L + g_sectorsGained) in 32 bits, divided by 5 (truncated toward 0);
- v = base + W + bonus − loss × L + Q + (16-bit value at 0x22A034) − (16-bit value at 0x22A032) − (low 16 bits of
  g_nativeDeathsTotal).

If v < 0: v = 0; if v > 500: v = 500. g_workerPool = v. Then Territory (above). g_idlePool = g_workerPool −
g_workers (signed, this time without the floor). If it is below 0: Natives quit with n = (−g_idlePool) as a byte,
g_idlePool = 0, All accounts (above), then host op 262 map_redraw.

**Natives quit** (`natives_quit`, n = a byte). Guards go first, then tappers; sectors are visited in the order of
the 61 bytes of `g_sectorOrder` (a value may repeat; a value 60..255 is SPEC GAP (not seen: the 61 bytes come from the new game (EXE data, all 0..59) and no step writes them) as under Plants: the port treats it as not connected), and only sectors connected to home count.
1. If n ≠ 0: for each order entry s that is connected: while `guards` of s ≠ 0 and n ≠ 0: `guards` − 1, n − 1,
   G + 1 (G = guards who quit, a byte).
2. If G ≠ 0: host op 261 natives_quit_msg (G, 0 when G = 1 else 1): "G guard(s) quit this morning!".
3. If n ≠ 0: for each order entry s that is connected: while `tappers` of s ≠ 0 and n ≠ 0: `tappers` − 1, n − 1,
   T + 1. Then r = `tappers` mod 3; if r ≠ 0: `tappers` − r, T + r, and n = n − r, or 0 when n ≤ r. (This step
   runs for every connected sector, also after n reaches 0: the tappers of a sector stay a multiple of 3.)
4. If T ≠ 0: host op 261 natives_quit_msg (X, 2 when X ≤ 1 else 3): "X tapper(s) quit this morning!", where
   X is **G**, not T (original bug, issue #2 row 13: the game passes the guard counter). The number register keeps G across
   the guard message: sprintf_ and the status message both save and restore it (checked in the asm, 2026-10-28).

Evidence (2026-10-23): 0x50CF4, 0x50BC4 and MercsQuitMsg? 0x50D2C mirrored in tools/ai.py (gained_bonus,
worker_pool, natives_quit); ai_diff `ledger` kinds pool and quit (random sector tables, orders, difficulty,
counters; the message text in g_textBuf compared), 0 mismatches; 5 of 6 hand mutants killed (the sixth, n > r
-> n >= r, is equal behaviour).

## Morning pay

In the morning, after the day end, the game collects the money the mercs carry, and a merc who carries too much
runs off with it. World fields: `soldiers` (team slots 0..7), `profiles` (64 records, layout "profile"),
`item_types`, `listed`, `stash_counts` (136 bytes, one per item type: records in the item store), `stat_progress`
(8 rows of 0x12 bytes, one per team slot), `greed` (256 bytes, one per profile number: the days of pay a merc
runs off with), `voice_test` (set to 1 here; its meaning in this place is not known) and `accounts` (Day accounts; offset 0xA0 = the
AIM "resignations" counter, u8). "Item" = a 12-byte item entry (layout "item"); "place i" of a soldier = its item
entry i (0..11). Bytes are unsigned unless stated.

**Money in an item** (`cash_value`). 0 unless the type is 0x6F (money). n = byte +1 as a signed byte. v = 10 ×
(byte +2 as signed); for j = 1 .. n − 1: v += 10 × (the byte at +7 + j, as signed). For n > 5 this reads past the
12 bytes: the next bytes of the soldier records (read them from `soldiers` as they lie).

**Cash carried** (`cash_count`, slot, drop). 0 when `inCurSector` = 0. Else, for the places 0, 7, 8, 9, 10, 11, 1
in this order: v = Money in the item; total += v (32-bit); when v ≠ 0 and the low byte of drop ≠ 0, the item is
cleared (all 12 bytes 0). The result is total.

**Collect the cash** (`collect_cash`). total = the sum of Cash carried (drop 1) over slots 0..7 (each slot, also
after an empty one). If total > 0 (signed): g_dayIncome, g_balance and g_dayNet each += total.

**Store an item** (`stash_item`, item e). t = type of e; t = 0: nothing. c = byte 6 of item_types[t] (the stack
class). k = 1 when c < 2, else byte +1 of e as a signed byte. If k + stash_counts[t] ≥ 100: nothing. Records:
- c < 2: (t, byte +1, byte +2), and when byte +3 (an attachment type) ≠ 0 also (byte +3, byte +4, byte +5);
- else, for j = 0 .. k − 1: (t, 1, byte +2 for j = 0, else the byte at +7 + j) (k ≤ 0: no record).

For each record in order: host op 264 stash_put (type, qty, cond); r = the low byte of the result; when r ≠ 0,
stash_counts[record type] + 1 (byte). After the last record: when its r ≠ 0, bytes 0..5 of e are set to 0.

**Merc leaves** (`merc_leaves`, slot, status). p = the profile of the soldier's `profileId`. p.teamStatus =
status (byte), p.salary = the soldier's `salary`, and p takes these soldier bytes: health ← `hpMax`, agility,
marksmanship, wisdom, medical, mechanical, dexterity, explosives, level. Kit (profile bytes +0x50 .. +0x59, one per
place 0..9): for place i = 0..11 with t = its type:
- t = 0 or t = 0x6F: kit[i] = 0 (only for i < 10);
- else when i ≥ 10, or t is listed, or byte +3 of the item is listed ("listed" = not 0 and one of the 32 `listed`
  bytes): Store an item with a **copy** of the 12 bytes (the soldier's item stays as it is; only the copy's bytes
  0..5 are cleared), then kit[i] = 0 (only for i < 10);
- else kit[i] = t.

Then if profile byte +0x3C ≠ 99 and it equals profile byte +0x3A: byte +0x3A = 99. Host op 263 face_clear (the
soldier's `slot` byte). Then the soldier: nickname byte 0 = 0, `inCurSector` = 0, `profileId` = 99, byte +0x19D =
0. (A stack of more than 5 in an item that goes to the store: SPEC GAP (not seen: the max stack, item type byte 6, is
1..5 for all 136 types in the EXE table; the pocket moves keep a stack at or below it (inventory.md "Pocket move");
the only stacking type with clip size 99, 0x52, is the result of none of the 82 merge table rows, so no merge sets
its count to 99), the original reads its own stack
beyond the copy; the port reads 0.)

**Team shift** (`team_shift`, to, from; team slots 0..7). stat_progress row to = row from, then row from = 0.
Soldier to = a copy of the 512 bytes of soldier from; its `slot` = to. Host op 265 face_move (to, from). Soldier
from: `salary` = 0, `salary2` = 0, `duty` = 0, `profileId` = 99, nickname byte 0 = 0, `inCurSector`, `energy`,
`hpMax`, `wounds`, `agility`, `marksmanship`, `hp`, `wisdom` = 0, bytes 0..5 of each of the 12 items = 0, `slot`
= 99, byte +0x19D = 0. Host op 263 face_clear (from).

**Close up the team** (`team_compact`, redraw). For j = 0..6 in order: when soldier j has `inCurSector` = 0, Team
shift (j, j + 1). Then when the low byte of redraw ≠ 0: host op 266 team_redraw.

**Pay the mercs** (`pay_mercs`). k = 0. While k < 8 and soldier k has `inCurSector` ≠ 0: when its `hp` ≠ 0, its
byte +0x19C is not 1 and not 7, and Cash carried (k, drop 0) ≥ greed[`profileId`] × `salary` (signed 32-bit
compare), the merc runs off:
1. `voice_test` = 1; host op 267 runs_off (k, the soldier's `slot` byte, profile number): the merc's quote, its
   speech balloon and the message "Perhaps you should've given the money to a merc you can trust?";
2. Merc leaves (k, status 0x0E); `profileId` = 99; `inCurSector` = 0; resignations (accounts +0xA0) + 1 (byte);
3. Close up the team (redraw 0); host op 268 island_map; Pay the day (Day accounts); host op 269 runs_off_end
   (the panel is drawn again);
4. slot k is tested again (the next merc moved into it).

Otherwise k + 1. (A merc with salary 0, or greed 0, always runs off: 0 ≥ 0. All AIM greed bytes are above 0.)

Evidence (2026-10-23): 0x25B84, 0x4458C, 0x445DC, AddStashItem? 0x51D50, 0x50618, 0x50790, 0x50750 and PayMercs?
0x44740 mirrored in tools/ai.py; ai_diff `pay` (kinds cash, collect, stash, leaves, compact, pay; random teams,
items with stacks of at most 5, store counts, profiles, accounts), 12000 cases, 0 mismatches; `ai_diff --port --only pay`
(port/ja1logic src/pay.rs, clean-room agent): 0 mismatches. PayDayDialog? 0x45100 (the "not enough funds" dialog) is mirrored in
tools/payday.py, validate/diff/payday_diff.py 400 cases 0 mismatches; its spec is the next section.

## Not enough funds

In the morning, while the balance is below 0, the player must choose mercs to dismiss without pay (each one leaves
the team) until the balance is 0 or more, the team is empty, or only a native guide (who works for free) is left.
Entry point `payday_dialog` (port_api.md); result 0 = the dialog ended, 1 = the poll (host op 221) said stop (the
library returns at once, with no more host calls or writes). World fields: `soldiers` (team slots 0..7),
`profiles`, `accounts` (balance = i32 at +0x24, signed; terminations = u8 at +0x9C; non-payments = u8 at +0xA1),
`opt_step_text`, `voice_test`, `tip_item`, `mouse_x`, `mouse_y` (signed), `mouse_buttons` (bit 0 = left), and
the fields of Merc leaves, Close up the team and Pay the day. "Mouse in (x0, y0, x1, y1)" = x0 ≤ mouse_x ≤ x1 and
y0 ≤ mouse_y ≤ y1 (signed, edges included).

**Team size**: the team slots in use, counted from slot 0 up to the first slot with `inCurSector` = 0 (at most 8).
**First guide**: the first team slot k of 0..7 (all 8, also after an empty slot) with `inCurSector` ≠ 0, `hp` ≠ 0
and `profileId` 60..64 (both ends included); 64 = none.

**The dialog.** blink = 0, first = yes. Repeat:
1. Stop (go to the end) when the balance ≥ 0 or Team size = 0, or when Team size = 1 and First guide = 0.
2. When first: host op 270 pd_message(0) ("NOT ENOUGH FUNDS TO PAY ALL MERCENARIES!"), op 270 pd_message(1)
   ("Choose someone to ... be the charitable one"), first = no, op 271 pd_timer_start.
3. Op 273 pd_clear; op 226 inv_cursor_pic2(0).
4. The click loop, until a merc is dismissed:
   a. Op 221 inv_poll (stop → return 1).
   b. Tooltips: for k = 0..7, x0 = 12 + 37·k: when Mouse in (x0, 20, x0 + 30, 46) and soldier k has `inCurSector`
      ≠ 0: op 234 tip_merc(k + 1) (more than one can come). When none came: op 233 tip_item(`tip_item`).
   c. The blink: when op 272 pd_elapsed (signed) > 200: when mouse_y > 145 op 145 (mouse off); blink = 1 − blink;
      op 274 pd_blink(blink); op 271 pd_timer_start; when mouse_y > 145 op 224 (mouse on).
   d. When bit 0 of mouse_buttons is 0: next poll (a).
   e. The faces: for k = 0..7: stop at the first soldier with `inCurSector` = 0; x0 = 13 + 37·k; skip k unless
      Mouse in (x0, 20, x0 + 27, 44). When its `profileId` ≥ 60 (a native guide; also 99): op 145, op 270
      pd_message(2) ("The native guides help you for free!"), op 224, next k. Else op 275 pd_ask(k, family) with
      family = 1 when `hp` as a signed byte ≤ 0 ("Don't pay MEMBER's family?"), else 0 ("Don't pay MEMBER?");
      the low byte of the result = 0 (no): op 246 (int 0x25), op 224, next k. Yes: Dismiss (k, x0), the click
      loop ends after this step (no more faces).
   f. The funds box: when Mouse in (279, 166, 304, 181): op 270 pd_message(0). (Also in the poll that dismissed.)
5. Back to 1.

**Dismiss** (team slot k, face x x0). S = soldier k, p = its `profileId`.
- When `hp` as a signed byte > 0: `voice_test` = 1; op 276 pd_quote(p); when `opt_step_text` = 0: op 246
  (0x25) and op 224; op 277 pd_balloon(k, x0).
- Else (dead): profile p byte +0x3E (u16) = S `salary2`; terminations + 1 (byte).

Then Merc leaves (k, status 1); `inCurSector` = 0; non-payments + 1 (byte); `profileId` = 99. Only when
`opt_step_text` ≠ 0: op 246 (0x25). Then always: op 224; op 233 tip_item(`tip_item`); Close up the team (redraw
0); Pay the day; op 268 island_map; op 269 runs_off_end.

**The end** (step 1 stopped): op 278 pd_end (the funds box and the panel are drawn again). When the balance ≥ 0:
for k = 0..7, stop at the first soldier with `inCurSector` = 0; when its `profileId` < 60: op 279 pd_face_frame(k);
then op 280 pd_panel_show. Return 0.

**What the player sees** (the host side of the ops; screen = the payroll screen as "The payroll screen" left it,
panel buffer P = its payroll picture):
- Op 234 tip_merc(n) shows the Tooltip box (panel.md "Tooltip box") number n at (x0, 20, x0 + 28, 45) with
  x0 = 13 + 37 (n − 1), the one text "fire", font 0; op 233 hides it.
- Op 274 pd_blink(b) draws number fields on the screen (as "The payroll screen", mode 300): b = 1 fields 52 and
  53, b = 0 fields 47..51. Field 52 = g_teamPay +0x38 at (150, 174), field 53 = g_balance +0x24 at (241, 174), both
  6 digits, fg 41, shadow 0. So every 200 ms the two numbers TEAM and NEW BAL. turn colour 41 and back.
- Op 276 pd_quote(p) starts PST quote 0 of speaker p (speech.md "Quote categories": text record 15 p, voice
  `PST\p`, member `<p>PST0.8`). Op 277 pd_balloon(k, x0): with Subtitles (`opt_step_text`) off, the face of p slides
  in and out as panel.md "Speech balloon" steps 1, 4, 5, 7 and 8 (no hurt face, no text) at the face point
  (x0, 20) (so the balloon rectangle is at (max(1, x0 − 15), 6)); the wait without a voice is 80 ms per byte of
  the quote text. With Subtitles on: the merc-face box with the text only (panel.md "Merc-face question box", ECX
  0, `PANEL_P.PCC`) for the same wait.
- Op 269 (after a dismissal) copies P to the screen after POSTFIN.PCC and payroll fields 43..51 went into P again
  (the new numbers); op 268 built P's payroll picture again (the team closed up).
- Op 278 pd_end: POSTFIN.PCC and fields 43..51 into P again. Op 279 pd_face_frame(k): panel element 103 (the
  panel sheet at (272, 132), 28 x 25, colour 0 left out: the word PAID over the face) into P at (13 + 37 k, 20).
  Op 280: P to the whole screen (the message box, the tooltip and the blink go away). With money enough the
  dialog ends at once, so every morning with a balance ≥ 0 stamps PAID on the faces of the mercs (not the
  guides) on the payroll screen.
- Ops 145 / 224 (mouse off / on), 226 (mouse cursor picture 0) and 246 (put back the screen part under the question
  box) change nothing that stays on the screen.

Evidence (2026-10-27, asm 0x45100..0x4555C): tooltip 0x23264 (EAX k + 1, EDX esi + 1, EBX 0x14, ECX esi + 0x1D,
stack 0x2D, "fire" 0x99E61, "" 0x90A60, "", font 0), hide 0x23420; blink 0x45266..0x45293: byte [esp + 0x1C] xor
1, ≠ 0 → 0x36010(0x90A60 "", *0x95678 (the screen), 320, 0x34, 0x35), else (…, 0x2F, 0x33); field table 0x93FD8
entries 52 / 53 (x 0x96 / 0xF1, y 0xAE, digits 6, fg 0x29, shadow 0, mode 300, values 0x229FD8 / 0x229FC4);
0x50908 (byte 0x1163BD = 0: face loader 0x28898 (profile), SpeechBalloon? 0x29544 (0x2299BC, x0, 0x14, ECX
profile, kind 5); else YesNoPrompt? 0x28C84 (0x2299BC, profile, nickname, 0, 0)); 0x4508C (DrawPanelElem 0x67 at
(13 + 37 k, 0x14) into g_panelBuf for each team slot up to the first empty one with profile < 0x3C, 0x46BF4); element
table 0x93732 entry 103 = (0x110, 0x84, 27, 24); 0x28698 = 0x468B0(0) (save area 0 back).

Evidence (2026-10-23): PayDayDialog? 0x45100 mirrored in tools/payday.py, validate/diff/payday_diff.py (random
teams with guides and dead mercs, balances, script clicks on the faces, the funds box and elsewhere, the yes / no
answers and the timer): 400 cases, 0 mismatches. The funds box position (279, 166) is a constant table entry
only this dialog reads.

## Morning duties

In the morning, after the pay, each merc does the work of its duty: resting and wounded mercs heal, mercs in
training practise, doctors heal the patients with the medical supplies, and mechanics repair the items. Entry point
`morning_duties` (port_api.md). World fields: `soldiers` (team slots 0..7) and `item_types` (byte +6 = the stack
class, byte +9 = repairable). Host op 281 practice (slot, stat, amount, flag) gives a merc practice in a stat
(amount = a signed 16-bit value; flag 1 = practice from training). random(n) = the host's random (n is at most
25 here, or the repair value below). Bytes are unsigned unless stated; "signed" = the byte as a signed byte.
Divisions truncate toward 0. S = soldier k; "place i" = S's item entry i (0..11, 12 bytes each, layout "item").

**Need** of S = signed `hpMax` − (signed `hp` + `healed`).

**Medical units** of an item: type 0x49 (first aid kit) weight 1, type 0x4A (medical kit) weight 2, any other type
weight 0. The item holds n = signed byte +1 units; unit 0 has the condition signed byte +2, unit j ≥ 1 the signed
byte at +7 + j. **Med points** of S = the sum, over the places 0..11 with weight m > 0, of (condition × m) for each
unit j = 0 .. n − 1 (n ≤ 0: none).

**Doctor budget** of S: t = (100 − signed dexterity) × 20 / 100; cap = ((signed wisdom − t) × signed medical / 100)
× 2; budget = min(Med points, cap).

**Doctor** (S with duty 3; returns the points used):
1. a = the budget. a = 0: return 0.
2. own = 0. While Need of S ≠ 0 and a ≠ 0: a − 1, own + 1, S `healed` + 1. When own ≠ 0: op 281 practice (k, 5,
   own / 3, 0). When a = 0 now: return own.
3. The patients = the soldiers j = 0..7 with j ≠ S `slot` (the byte, not k) and duty 3 or 4 (so other doctors
   count too). None: return own. per = a / (the number of patients).
4. done = 0. For each patient P in order: n = 0; while a ≠ 0 and Need of P ≠ 0 and n < per: a − 1, done + 1,
   n + 1, P `healed` + 1.
5. When a ≠ 0: for each patient P in order: when a ≠ 0 and Need of P ≠ 0: a − 1, done + 1, P `healed` + 1 (one
   point each, one pass).
6. When done ≠ 0: op 281 practice (k, 5, done / 3, 0). Return own + done.

A negative budget (wisdom below t) never reaches 0, so the loop of step 2 ends only when Need of S = 0. `healed` is
an unsigned byte, so that takes at most 256 rounds when `hp` ≤ `hpMax`; step 4 then does nothing (per < 0) and step
5 gives one point to each patient with a Need. With a negative budget and `hp` above `hpMax` the Need never reaches
0 and the original runs about 2^32 rounds: a **stop point** (port_api.md "Division by zero"): the library makes host
call 47 and returns 0 before step 2.

**Use the supplies** (S, used, total): used = min(used, total). For each place 0..11 with weight m > 0: for c = n −
1 down to 0 (n = signed byte +1 read once at the start; c means the unit c):
- c = 0: v = signed byte +2. When v × m ≤ used: all 12 bytes of the item = 0, used − v × m. Else byte +2 = v −
  used / m, used = 0.
- c ≥ 1: v = the signed byte at +7 + c. When v × m ≤ used: byte +1 − 1, used − v × m, the byte at +7 + c = 0.
  Else that byte = v − used / m, used = 0.

(A unit with condition 0 goes even when used = 0: 0 ≤ 0.)

**Repair value** of S: v = ((signed mechanical × signed dexterity + (signed level − 1) × 5) × 3) / 200. t = the
signed byte +2 of place 0 when the type of place 0 is 0x4B (toolkit), else 0. Points = t × v / 100.

**Repair** (S with duty 5):
1. p = the points. p = 0: done (a negative p goes on).
2. done = 0. For the places 1, 7, 8, 9, 10, 11 in order, when byte +9 of item_types[type] ≠ 0:
   a. q = signed byte +1. When q < 0 and p ≥ 2: byte +1 = −q (the item is unjammed), done + 2, p − 2.
   b. n = 100 − signed byte +2. While n ≠ 0 and p ≠ 0: n − 1, done + 1, p − 1, byte +2 + 1.
   c. When byte +6 of item_types[type] > 1 and signed byte +1 > 1: for c = 0 .. (signed byte +1) − 2: n = 100 − the
      signed byte at +8 + c; while n ≠ 0 and p ≠ 0: that byte + 1, n − 1, done + 1, p − 1.
3. done = 0: stop. Op 281 practice (k, 7, done, 0).
4. The toolkit wears: when random(100) < done and the signed byte +2 of place 0 > 10: x = random((done / 15) + 1);
   c = (byte +2 of place 0) − (x + 1), as a byte; byte +2 of place 0 = c; when c as a signed byte < 0: all 12
   bytes of place 0 = 0.

(A condition above 100 makes n negative and the loops of steps 2b / 2c run until p = 0. A negative p (a toolkit
with a negative condition, or a negative v) ends each loop when n = 0, so each item comes to condition 100. When
both n and p are negative, the original runs about 2^32 rounds: a **stop point** (port_api.md "Division by zero"):
the library makes host call 47 there and the step does nothing more (no practice, no toolkit wear).)

**Training** (S with duty 2), by `trainWhat`; each line is op 281 practice (k, stat, (100 − signed field) × m, 1):
- 0: stat 1 with `hpMax`, stat 2 with agility, stat 3 with dexterity, each m = 4 (three calls in this order);
- 1: stat 5 with medical; 3: stat 7 with mechanical; 4: stat 8 with marksmanship; each m = 12;
- 2: stat 6 with the **level** (not explosives), m = 12 (an original bug, #2 row 14);
- other values: nothing.

The amount is the low 16 bits of the product as a signed value.

**Morning duties** (`morning_duties`):
1. Healing at rest. For k = 0..7: d = signed `hpMax` − signed `hp`. Skip S when d = 0 or its duty is not 1 or 4.
   n = (signed `hpMax` / 10) as a byte; n = 0: skip. r = (random(n) + 1 + random(n) + 1) as a byte; when r > d:
   r = d as a byte. S `healed` + r (byte).
2. The duties. For k = 0..7, by S's duty:
   - 0 or 7: op 281 practice (k, 1, 100 − signed `hpMax`, 0);
   - 2: Training; 3: total = Med points of S, used = Doctor, then Use the supplies (S, used, total); 5: Repair;
   - other duties: nothing.
   Then, for every duty: when `inCurSector` ≠ 0, byte +0x188 = 0 and signed `hp` ≥ 15: op 281 practice (k, 9,
   0x19, 0).

Evidence (2026-10-23): MorningDuties? 0x4B48C with MedPoints? 0x48318, DoctorCap? 0x483AC, DoctorHeal? 0x48418,
UseMedSupplies? 0x48224, RepairDuty? 0x48684, RepairCap? 0x4882C mirrored in tools/duties.py;
validate/diff/duties_diff.py (random teams, duties, training choices, stats, kits, toolkits, repairable and
stacked items, jammed items; Random and StatPractice 0x62A68 stubbed and logged): 3 × 5000 cases, 0 mismatches.
The practice amounts here reach about 1200 and carry flag 1 for training, so the byte-sized op 105 does not fit:
op 281 takes the full amount.

## Morning stat changes

In the morning, after the "not enough funds" dialog, the practice each merc collected (stat_progress) becomes stat
points. The team panel counts each change up or down; a new experience level changes the salary. Entry point
`stat_changes` (port_api.md). World fields: `soldiers` (team slots 0..7), `stat_progress` (row k = 9 signed 16-bit
values for team slot k: stats 1..9 in this order) and `stat_changed` (640 bytes: byte profile number × 10 + stat is
set to 1 when that stat changed; stat 0 = the pay). Divisions truncate toward 0; "signed" = the byte as a signed
byte; d and the progress values are signed 16-bit (store the low 16 bits).

The stats 1..9 and their **field** (what the panel shows and what changes) and **lowest** value:

| stat | field | lowest |
|---|---|---|
| 1 (health) | `hp` (the current health, not `hpMax`) | 15 |
| 2, 3, 4 | agility, dexterity, wisdom | 1 |
| 5, 6, 7, 8 | medical, explosives, mechanical, marksmanship | 0 |
| 9 | level | 1 |

**Count** (k, stat, d): stat 0 means `salary2` (u16), stats 1..9 the field above as a signed byte. value = that
number of soldier k; op 283 xp_count (k, stat, value, d); then the field = value + d (a 16-bit store for stat 0,
else the low byte).

**Salary step** (salary, up): x = salary × 1.5 when up, else salary × 0.66666666666666663 (the double nearest
below 2/3), computed in double precision and truncated; u = the low 16 bits of x. g = 5 when u ≤ 500, 10 when u ≤
1000, 25 when u ≤ 1500, 50 when u ≤ 2500, 100 when u ≤ 5000, 500 when u ≤ 10000, else 1000. r = u mod g. The
result = the low 16 bits of ((x − r) mod 65536 + (2r / g) × g) (so 72 × 2/3 = 48 → 50, half up).

**Level steps** (p = the level progress, v = signed level): d = 0.
- p < 0: while (v + d) × −1000 ≥ p: s = v + d; d − 1; p = p + s × 1000.
- else: while p ≥ (v + d) × 1000: s = v + d; d + 1; p = p − s × 1000.
- Then when v + d > 10: d = 10 − v, p = 0. When v + d < 1: d = 1 − v, p = 0.

(The compares use full integers; p and d wrap as 16-bit values. A level progress of −1000 × v(v + 1)/2 or lower
takes the first loop below level 1 until p wraps to a positive value; the loop always ends, after at most 32,769
rounds (checked for every signed byte v and every 16-bit p), and the last line then gives level 1. Evidence:
xp_diff fuzzes progress values down to −32768 against the original.)

**Morning stat changes** (`stat_changes`): for k = 0..7, stop at the first soldier with `inCurSector` = 0. Skip
soldier k (S) when its `hp` = 0. Else for stat = 1..9 in order:
1. Op 282 xp_stat_begin. p = stat_progress row k, value stat − 1; v = the field as a signed byte.
2. Stat 9: Level steps. Stats 1..8: d = p / 100, p = p − 100 × (p / 100) (the remainder keeps the sign of p);
   when v + d > 100: d = 100 − v, p = 0; when v + d < lowest: d = lowest − v, p = 0. Store p.
3. Stat 1: S `hpMax` + d (the low byte); d = d + S `healed`; `healed` = 0. (So the health grows by the gain plus
   the healed points, and a merc with `hp` below 15 gains up to 14 `hpMax`: an original bug, #2 row 15.)
4. When d ≠ 0: stat_changed[profileId × 10 + stat] = 1.
5. Count (k, stat, d). Stat 1: op 284 xp_health_bar (k).
6. Stat 9 and d ≠ 0:
   a. Repeat |d| times (only this line): `salary` = Salary step (`salary`, up = d > 0).
   b. Then once: op 285 xp_salary_msg (k, 1 when d > 0 else 0, the new level as a signed byte, `salary`).
   c. e = (`salary` + 1) / 2 − `salary2` when `duty` is 1 or 4, else `salary` − `salary2` (16-bit). When e ≠ 0:
      stat_changed[profileId × 10 + 0] = 1. Count (k, 0, e) (so `salary2` becomes the pay of tomorrow).
7. Op 286 xp_stat_end.

At the end: op 287 xp_end.

Evidence (2026-10-23): ExperienceChangeMsg? 0x45BBC, StatCountUp? 0x45624 (its store), StatValue? 0x47350 and
SalaryStep? 0x62DA8 mirrored in tools/xpchange.py; validate/diff/xp_diff.py: the salary step for 6000 salaries,
the store of 0x45624 run for real (1500 cases) and 3 × 1500 random teams (progress rows, levels, salaries, duties):
0 mismatches; 5 hand mutants of the mirror (lowest 15 → 14, a byte store for the pay, the level top 10 → 11, no
pay flag, duty 3 instead of 4) all fail. The original reads the value of Count from the soldier its slot byte
names; the test keeps the slot byte = k.

### The experience count (host ops 282..287)

The stat changes run on the payroll screen: panel buffer P (320 x 200, pitch 320) holds the payroll picture ("The
payroll screen": BASSMAP.PCC and the merc columns, as drawn again by "Not enough funds"), and B is BASSMAP.PCC
(MAIN.DAT) on its own (320 wide). **Show** = copy P's rows 0..145 to the screen rows 0..145 (rows 146..199 of the
screen do not change). A **fast** flag is 0 when `stat_changes` starts.

- Op 282 xp_stat_begin: when an Escape key press is waiting (it is taken) and fast = 0: fast = 1. Start the stat timer.
- Op 283 xp_count (k, stat, value, d): the **count** below, for the soldier S = team slot k.
- Op 284 xp_health_bar (k): the assignment health bar of S at (37 × S `slot` + 44, 20) in P, with the gap rows
  ("The assignment picture" 2g, from the world as it is now); then copy P's 3 × 25 pixels at that place to the
  screen.
- Op 286 xp_stat_end: wait until the stat timer reaches 120 ms (25 ms when fast = 1).
- Op 287 xp_end: Show.

**Count** (stat i, value v, change d; v and d as the op gives them): X = 27 + 37 × S `slot`, Y = 59 + 8 i (the
stat's place of "The assignment picture" 2d). t = v + d. a = |d| (16-bit). The step g: 1000 when a ≥ 10000 and a
mod 1000 = 0; else 500 when a ≥ 5000 and a mod 500 = 0; else 100 when a ≥ 1000 and a mod 100 = 0; else 50 when
a ≥ 500 and a mod 50 = 0; else 25 when a ≥ 250 and a mod 25 = 0; else 10 when a ≥ 100 and a mod 10 = 0; else 5
when a ≥ 50 and a mod 5 = 0; else 2 when a ≥ 20 and a mod 2 = 0; else 1. When d < 0: g = −g.
For n = v, v + g, v + 2g, ... while n ≤ t (g > 0) or n ≥ t (g < 0) (so d = 0 still gives one round, n = v):
1. When an Escape key press is waiting (it is taken) and fast = 0: fast = 1.
2. Frames f: 1 when n = v, else 4. Width W and left x by n: n < 100 (also below 0): W = 12, x = X + 2;
   n < 1000: W = 14, x = X − 2; n < 10000: W = 16, x = X; else W = 21, x = X − 1. y = Y, h = 8.
3. For j = 0 .. f − 1:
   a. W = W + 2, x = x − 1, y = y − 1.
   b. Save P's rectangle (x, y) W × h into P at (0, 146) (row by row, pitch 320: P's rows from 146 are not on the
      screen and are not drawn again here).
   c. Frame (panel.md "Drawing primitives") from (x + 1, y) to (x + W0 + 1 + 2 j, y + 7 + 2 j), colour 0x79,
      with W0 = the width of step 2 (before 3a). Show.
   d. When n = v, d ≠ 0 and fast = 0: wait 800 ms.
   e. Put the saved rectangle back into P.
   f. When j = 3: copy B's 18 × 8 pixels at (X, Y) to P at (X, Y); n with 3 digits (as 2d) at (X + 1, Y + 1)
      with fg 7, then at (X, Y) with fg 0xFF (font 0, shadow 0, mode 300). Show.
   g. When fast = 0: the click sound (sound.md "Click sound"), then wait 30 ms.
   h. h = h + 2.
4. At the end the library stores t (the field as before).

(So the frame of round 0 of a number below 100 runs from X + 2 to X + 14, one pixel inside the saved rectangle,
and each next round grows it by one pixel on each side.)

So a count of d ≠ 0 shows the old number with a small frame for 800 ms, then each next number with a frame that
grows four times (four clicks) and the new number in white; Escape drops the waits and the clicks. The waits of
800 ms (a busy loop) and 30 ms (the game clock with mouse polls) and the stat timer are real time.

Evidence (2026-10-28): StatCountUp? 0x45624 (fast flag = ExperienceChangeMsg? local, 0 at its start; EscPressed?
0x67300; jump table 0x455FC for the widths; BlitCopy 0x52AE0 with the save area g_panelBuf + 0xB680; DrawFrame
0x46C44 colour 0x79; 0x46C1C = 0xB680 bytes of g_panelBuf to the screen *0x95678; busy delay 0x67250(800);
ClickSound 0x5C880; wait 0x67EEC(30) (10 after Escape, but no wait runs then); background *0x93E5C = BASSMAP.PCC
loaded with pitch 640 at 0x4455C, read with pitch 640; DrawNumber 0x36090 with the table 0x93F10 (x 27, y 59 + 8 i,
fg 7, mode 300); ExperienceChangeMsg? 0x45BBC: timer 0x22A404 (0x78 ms, 0x19 after Escape), HealthBar? 0x47144
(ECX 1, stack 1) after the stat 1 count, 0x46C1C at the end. Test: validate/diff/xpcount_diff.py.

## Back on duty

After the morning stat changes (entry point `duty_wake`). World fields: `soldiers` (records 0..63), `duty7_merc`
(u8: the soldier record whose duty 7 ends this morning, 0x40 = none) and `team_on_duty` (Day accounts). A soldier is "Healed" when (signed `hpMax`) − (signed `hp`) equals the signed byte +0x185 (`wounds`): an equality test of
three bytes, not a test of the sign of `wounds`.
1. g = duty7_merc. When g < 0x40 and soldier g has `duty` 7: its `duty` = 0 and duty7_merc = 0x40.
   (The world holds records 0..0x2F only: for g = 0x30..0x3F the original reads past them; the library does
   nothing then. The test keeps g < 0x30 or g ≥ 0x40.)
2. For k = 0..7: when soldier k has `duty` 4 (patient) and is Healed: `salary2` = `salary` (full pay again),
   `team_on_duty` + 1, `duty` = 0.
3. When no soldier 0..7 has `duty` 4 now: for k = 0..7: when soldier k has `duty` 3 (doctor) and is Healed:
   `team_on_duty` + 1, `duty` = 0 (`salary2` stays).

## Profiles take the team's stats

The last step of the morning (entry point `profile_sync`). World fields: `soldiers`, `profiles`. For k = 0..7 with
`inCurSector` ≠ 0 (the others are skipped; the loop does not stop at them): profile p = `profileId` (< 64); p.salary (u16) = `salary`; p.level, health, agility, dexterity, wisdom,
medical, mechanical, marksmanship, explosives = the soldier's level, `hpMax`, agility, dexterity, wisdom, medical,
mechanical, marksmanship, explosives.

Evidence (2026-10-23): 0x42378 and 0x65B40 mirrored in tools/morning.py; validate/diff/morning_diff.py (random
duties, health, wounds bytes, the duty 7 record, salaries, profiles): 2000 cases, 0 mismatches. Both walk the
team pointer table 0x100958, which the test checks equals the team records 0..7.

## A disliked merc in the team

After the salary prompts and before "AIM rate complaints" (entry point `hated_check`). World fields: `soldiers`
(records 0..7), `profiles`. Host op 291.
For k = 0..7: stop the loop at the first soldier k with `inCurSector` = 0. Skip soldier k when `hp` = 0 or its
`profileId` p ≥ 60. Else d = profile p's byte +0x3A (the merc it dislikes). n = the number of soldiers 0..7 with
`inCurSector` ≠ 0 and `profileId` = d (the soldier k itself counts too). When n ≠ 0: profile p's byte +0x3C = d,
then op 291 hate_quote (k, d).

Evidence (2026-10-23): 0x44C10 (and its count 0x50AC0) mirrored in tools/morning.py hated_check; complain_diff.py
section hate (the name, PlayQuote, StrReplace, 0x50908, 0x67BF0, 0x467E4, 0x28698 stubbed and logged): 0 mismatches.

## AIM rate complaints

After the salary prompts and before "Profiles take the team's stats" (entry point `complain_pick`; the inner step
is entry point `complain` with a speaker slot). World fields: `soldiers` (records 0..7), `profiles`, `accounts`
(byte offsets: +0x00 mercs killed (low byte), +0x04 the day (low byte), +0x9B bad days in a row, +0x9C terminations,
+0x9D hirings, +0xA0 resignations, +0xA1 non-payments; all u8). Host ops 288..290 (port_api.md), random = the host's
random. All arithmetic in this section is on bytes (0..255, wrapping) unless the text says otherwise.

**AIM rate** (kind 0 = fatality, 1 = turnover, 2 = non-payment), always computed "with one more" here:
1. d = (day + 1) mod 256. o = d for kind 0, else hirings. t = (d div 2 + o) mod 256. When t = 0: the rate is 0.
2. When t < 10: t = 10.
3. n = mercs killed (kind 0), (resignations + terminations) mod 256 (kind 1), non-payments (kind 2).
   When n = 0: the rate is 0 (no "one more" then). Else n = (n + 1) mod 256.
4. q = (n × 100) div t (integer); r = q mod 256; the rate is 100 when r > 100, else r.

**complain_pick**: a list "picked" of 8 flags starts all clear. Repeat 8 times: draw r = random(8) again and again
until picked[r] is clear; set picked[r]. When soldier r has `inCurSector` ≠ 0, `hp` ≠ 0, the byte +0x19C = 1 and
`profileId` ≤ 59: run complain with speaker = soldier r's `slot` byte (+0x17A) and stop.

**complain** (speaker slot S, a byte):
1. Op 288 cp_begin (S). "said" = 0.
2. For k = 0..7: stop the loop at the first soldier k with `inCurSector` = 0 or `hp` = 0 (the soldiers after it are
   not looked at). Skip soldier k when its `slot` byte = S or its `profileId` p ≥ 60. Else, with profile p:
   - when the fatality rate ≥ p.maxFatalityRate (+0x36): op 289 cp_quote (S, 8, p, first = 1 − said); said = 1;
   - when the turnover rate ≥ p.maxTurnoverRate (+0x37): the same with quote 9;
   - when the non-payment rate ≥ p.maxNonPaymentRate (+0x39): the same with quote 11;
   - when bad days ≠ 0 and bad days + 1 ≥ p.maxBadDayStreak (+0x38) (integers, no wrap): the same with quote 10.
   (The rates are compared unsigned, byte to byte.)
3. When said = 1: op 290 cp_end.

Evidence (2026-10-23): 0x45010, 0x44E7C and AimRate 0x1FDEC mirrored in tools/morning.py (complain_pick,
complain, aim_rate); validate/diff/complain_diff.py (pick and speak sections, AimRate runs for real; random,
0x67BF0, 0x467E4, 0x673AC, 0x28698 and the quote 0x44D28 stubbed and logged): 2000 cases, 0 mismatches, every quote
reached; 5 hand mutants caught. The quote itself (0x44D28: the merc's voice, the intro quote 7 the first time, the
text with NFNCFS replaced by the other merc's name, its text box) is the host's job (op 289).
Evidence (2026-11-01): complainquote_diff compares 0x44D28 and the quote of 0x44C10 with `balloon::complain_texts` / `hate_text`.
Op 289: speaker = the slot S in AL, member byte 0x22A1F0 = p (the port had them reversed until this test).

## Corpse export

After the morning stat changes and Back on duty, before the salary prompts (entry point `corpse_export`, host ops
292 corpse_ask, 293 corpse_paid, 294 corpse_end). World fields: `soldiers` (records 0..7), `profiles`, `accounts` (+0x24 balance i32,
+0x2C day net i32), `difficulty`, `stash_counts`, `item_types`, `listed`. "Dead" = signed `hp` < 1.
1. Look for k = 0..7, stopping at the first soldier with `inCurSector` = 0: is soldier k Dead with `profileId` < 60?
2. When one is found: cost = 600, 800 or 1000 for difficulty 0, 1, 2. The host asks "Dish out $cost for corpse
   export?" (op 292 corpse_ask with cost and can_pay = 1 when cost <= balance, signed). When cost > balance the same
   box asks, but its answer does not count (no payment; panel.md "The travel animation": the yes row then says
   "Sorry, you don't have the funds"). Else, when the answer ≠ 0: balance −=
   cost, day net −= cost (32-bit), and the host draws the funds again (op 293 corpse_paid); no grudge. In every other case (no money or
   answer 0) the export is refused: for k = 0..7 (all, no stop): when soldier k is Dead, `inCurSector` ≠ 0 and
   `profileId` < 60: **Corpse grudge** (k).
3. k = 0; while k < 8: when soldier k is Dead and `inCurSector` ≠ 0: Merc leaves (k, status 0) for `profileId` < 60,
   else **Native leaves** (k); then Close up the team (redraw 0) and test slot k again. Then the host draws the
   island map and the team panel again (op 294 corpse_end; always, also when no soldier was found in step 1).

**Corpse grudge** (k): c = random(2) + 1 (a byte; c = 0: nothing). Repeat c times: draw u = random(60) again until u ≠
soldier k's `profileId`. Profile u: when all four tests hold (+0x3C ≠ that `profileId`, +0x3B is equal to 99,
+0x4D is equal to 99, +0x2F ≠ 3): when no
soldier 0..7 with `inCurSector` ≠ 0 has `profileId` = u, +0x3B = soldier k's `profileId`, else +0x4D = it.

**Native leaves** (k): for place i = 0..11 with type t ≠ 0 (money too): when t or the item's byte +3 is listed: Store
an item with a copy of the 12 bytes. Then face_clear (the `slot` byte), nickname byte 0 = 0, `inCurSector` = 0,
`profileId` = 99 (byte +0x19D stays).

Evidence (2026-10-23): CorpseExportPrompt? 0x46328 (with 0x462E0, 0x46548, 0x50AC0, 0x505A0, 0x514A4) mirrored in
tools/morning.py corpse_export; validate/diff/corpse_diff.py (the parts run for real, UI stubbed and logged):
1000 cases, 0 mismatches; 5 hand mutants caught.

## Salary raise

After Corpse export (entry point `raise_salary`). A team merc with a reason asks for a raise; when no deal is made,
the merc leaves. World fields: `soldiers` (records 0..7), `profiles`, `accounts` (same bytes as in AIM rate
complaints) and the fields of Merc leaves and Close up the team. Host ops 295..300 (port_api.md) and 263..266.
Soldier bytes: +0x110 salary (u16), +0x112 old salary (u16), +0x18D, +0x19C, +0x1D9. Profile bytes: +0x28 salary
(u16), +0x30 accept, +0x31 refuse, +0x32 tries, +0x36 fatality limit, +0x37 turnover limit, +0x38 bad days limit,
+0x39 non-payment limit, +0x4D grudge (all u8 except the salaries). The AIM rate here is the one of AIM rate
complaints **without** "one more": d = the day (not + 1), and n is not increased.

1. k = 0. While k < 8 and soldier k has `inCurSector` ≠ 0 (test k first: soldier 8 ends the loop):
   skip soldier k (k = k + 1) when its `hp` byte is equal to 0 or its `profileId` ≥ 60. Else get the **Reason** of
   soldier k; reason 0: skip. Else:
2. Op 296 raise_ask (k, reason, g), g = the grudge profile for reason 5, else −1. The start offer o = soldier k's
   salary. flag "leaves" = 1. n = 0.
3. While the profile's tries byte (+0x32, unsigned) > n: o = op 297 raise_offer (k, o) (a signed 16-bit offer;
   the host shows the dialog and the player types an amount). a = the **Answer** to o.
   - a = 2 (accepted): when soldier k's old salary < its salary (unsigned): old salary = o div 2 (signed, toward
     zero), else old salary = o. Salary = o, profile +0x28 = o. leaves = 0. Then by the reason: 1: +0x36 = 101;
     2: +0x37 = 101; 3: +0x38 = 10; 4: +0x39 = 101; 5: +0x4D = 99.
   - When a ≥ 0: op 298 raise_answer (k, a) (the merc's quote and the redraw).
   - When a = 1: n = n + 1 and repeat step 3; else stop step 3 (a = −1, 0, 2).
4. Op 299 raise_done. When leaves = 1: when soldier k's +0x19C is equal to 8 and +0x1D9 ≠ 0: +0x18D = 0. Then Merc
   leaves (k, status 1), `profileId` = 99, `inCurSector` = 0, accounts resignations (+0xA0) = +1 (byte, wraps),
   Close up the team (redraw 0), and k = k − 1 (the merc that moved into slot k is tested next).
5. Op 300 raise_redraw. k = k + 1, back to step 1.

**Reason** (soldier k, profile p of its `profileId`), the first that holds:
1. +0x19C is equal to 8 and +0x1D9 ≠ 0: 3.
2. AIM rate (kind 0) ≥ p+0x36: 1. AIM rate (kind 1) ≥ p+0x37: 2. AIM rate (kind 2) ≥ p+0x39: 4. (unsigned bytes)
3. accounts bad days in a row (+0x9B) ≥ p+0x38: 3.
4. g = p+0x4D. g ≥ 60: 0. Else op 295 raise_voice (k, g) and the reason is 5.

**Answer** (soldier k, offer o as a signed 16-bit number): o < 1: −1. Else s = the salary of the soldier whose
record number is soldier k's `slot` byte (+0x17A; normally k itself). pct = (o − s) × 100 div s (32-bit signed,
toward zero), then cut to a signed 16-bit number. A = p+0x30, R = p+0x31 (unsigned). When A is equal to 1 and
pct < 1: pct = 1. When pct ≥ A and A ≠ 0: 2. Else when pct < R: 0, else 1.
(Bug kept on purpose: with A = 1 every positive offer is accepted, also a lower salary. s = 0 crashes the
original with a division by zero; the port then returns 0.)

Evidence (2026-10-23): RaiseSalaryPrompt? 0x448A8, RaiseReason? 0x44634 and the answer rule of SalaryDialog?
0x29074 mirrored in tools/morning.py (raise_salary, raise_reason, salary_answer); validate/diff/raise_diff.py (the
dialog runs for real with its drawing stubbed and the typed number scripted): 3000 cases, 0 mismatches; 7 hand
mutants caught.

## The morning

The morning steps run in this order (entry point `morning`; each step is the entry point of its section, on the
same world):

1. host op 301 payroll_show (the payroll screen; only drawing);
2. Pay the mercs (`pay_mercs`, Morning pay);
3. Collect the cash (`collect_cash`, Morning pay);
4. Morning duties (`morning_duties`);
5. Not enough funds (`payday_dialog`): when its result is 1 (op 221 inv_poll said stop), `morning` stops here
   and returns 1;
6. Morning stat changes (`stat_changes`);
7. Back on duty (`duty_wake`);
8. Corpse export (`corpse_export`);
9. Salary raise (`raise_salary`);
10. A disliked merc in the team (`hated_check`);
11. AIM rate complaints (`complain_pick`);
12. Profiles take the team's stats (`profile_sync`).

Then `morning` returns 0. World: the fields of all these sections.

Evidence (2026-10-23): the original calls 0x44380, PayMercs? 0x44740, 0x445DC, MorningDuties? 0x4B48C,
PayDayDialog? 0x45100, ExperienceChangeMsg? 0x45BBC, DutyWake? 0x42378, CorpseExportPrompt? 0x46328,
RaiseSalaryPrompt? 0x448A8, HatedCheck? 0x44C10, ComplainPick? 0x45010 and ProfileSync? 0x65B40 one after the other
with no arguments (DayEndScreen? 0x65BD4, at 0x65CE2..0x65D19). PayrollScreen? 0x44380 only draws (map, PFLIP
animation, ARPOSTFIN.PCC panel, a slide of the panel) and loads BASSMAP.PCC.

### The payroll screen (host op 301)

All waits of this screen (60 ms, 1 ms) and of the office below (90 ms frames, the palette cycle) read the game
clock (world_load.md "Game speed", The game clock): a wait ends on a clock step, ~55 ms apart.

Step 1 of the morning shows the **payroll screen**: one column per merc in the sector with the face, the duty, the
stats and the health bar (as the assignment screen, but nobody can change anything), and a panel with the day's
money. It only draws; nothing waits for the player. Screen S and panel buffer P are 320 x 200 (pitch 320). Before
the screen, S shows the day replay screen as it is at the end of "The day replay" (the map of R with the flags of
the last replay picture, the verdict's rating word; when the replay showed no picture, its start picture): the
morning runs inside DayEndScreen? 0x65BD4 after DayReplay and DayEndVerdict, and the payroll starts at the OK
(Enter or button 15) of that screen. Evidence (2026-10-28, DOSBox-X tac60, key C to the day end): the screen at the
DayEndScreen? wait shows the island map with the day's markers and the green flag of sector 60 over the verdict panel.

**The payroll picture** (built in P, as "The assignment picture" with these changes): step 1 draws BASSMAP.PCC
(MAIN.DAT) instead of ASSMAP.PCC; steps 2b (duty choices: no duty changes) and 2c (the darkened face and the status
field) are left out; step 3 (ASSPAN.PCC and its fields) is left out. Steps 2a, 2d, 2e, 2f and 2g are the same.

**Payroll fields** 43..51 (as the map panel's Number fields: all without background, mode 300, shadow 0; money
colour = 0x79 when the value is below 0, else 0x32, no g_balanceAfterDay test):

| k | value | at (x, y) | d | fg |
|---|---|---|---|---|
| 43 | g_treesTapped `accounts` +0x1C | (78, 163) | 3 | 50 |
| 44 | g_treePrice +0x10 | (66, 174) | 6 | 50 |
| 45 | g_dayIncome +0x20 | (66, 185) | 6 | 50 |
| 46 | g_nativesCost +0x60 | (150, 163) | 6 | 121 |
| 47 | g_teamPay +0x38 | (150, 174) | 6 | 121 |
| 48 | g_dayExpenses +0x64 | (150, 185) | 6 | 121 |
| 49 | `prev_balance` | (241, 163) | 6 | money colour |
| 50 | g_balance +0x24 | (241, 174) | 6 | money colour |
| 51 | g_dayNet +0x2C | (241, 185) | 6 | money colour |

The steps:
1. Keep a copy B of the screen rows 0..142.
2. Build the payroll picture in P.
3. Picture steps `PFLIP1.PCC`, `PFLIP2.PCC`, `PFLIP3.PCC` (as the finance page's Open step 1: each picture
   (MAIN.DAT, 320 x 54) goes into P rows 146..199 and to the screen's panel rows; with the first picture (after 60 ms of the old screen) the
   **page sound**: main-bank effect 4 (ROT), rate 0x2B11, volume 20, once).
4. POSTFIN.PCC (MAIN.DAT, 320 x 54) into P rows 146..199, then payroll fields 43..51 into P; wait until 60 ms
   passed since the last picture step; copy P rows 146..199 to the screen.
5. The mouse pointer becomes shape 201 (panel.md "The mouse pointer": it stays until the end of the morning stat
   changes, which set shape 0).
6. The **slide sound**: main-bank effect 5 (SCR), rate 0x2B11, volume 0x58, 2 times.
7. The **slide**: 24 steps j = 0..23, each at least 1 ms after the previous one: when 136 − 6 j ≥ 1, screen rows
   3 .. 3 + (136 − 6 j) − 1 = B rows 7 + 6 j and on (the map moves up); then screen rows 139 − 6 j .. 142 = P's
   same rows (the picture comes up from below). (The original hides the mouse while it draws when the mouse is
   above row 142.)

At the end the screen holds P's rows 1..142 and 146..199; rows 0 and 143..145 keep what they showed before. The
screen stays so for the next morning steps ("Not enough funds" draws over it). The original then loads BASSMAP.PCC
once more into another picture buffer (0x93E5C); nothing visible comes of it here.

Evidence (2026-10-26): PayrollScreen? 0x44380 (0xB2C0 bytes = 320 x 143 to g_backBuffer; DrawBassMap? 0x4420C;
PlayFlipAnim? 0x51264("pflip", first 1, count 3, step 1; PlaySfxSlot 0x5CB34(4, 0x2B11, 20, 1) at frame index 1);
0x36010("postfin.pcc" 0x90B18, g_panelBuf, 320, 43, 51); TimerElapsedMs 60; BlitCopy 0x52AE0 320 x 54 at +0xB680;
SetMouseShape? 0x674B0(0xC9); PlaySfxSlot(5, 0x2B11, 0x58, 2); the slide loop 0x44485..0x44556 (BlitCopy to
screen + 0x3C0 from back + 320 (7 + 6 j), BlitCopy at 320 (139 − 6 j) for 4 + 6 j rows); 0x532BC(bassmap.pcc,
0x93E5C)); DrawBassMap? 0x4420C = DrawAssignMap? without 0x4811C / 0x47700 / RemapRect (face BlitCopy 28 x 25 at
P + 0x1900, stats 0x47504, duty text 0x9662A + 8 duty at the table 0x93F72 x / y, nickname, HealthBar? 0x47144);
field table 0x93FD8 entries 43..51 (field 49 = g_prevBalance 0x229AC0). Test: validate/diff/payroll_diff.py.

### The day-end accounts

Before the day replay (and so before the day's verdict) the day end settles the day. Entry point `day_end_accounts`
(port_api.md), in this order:
1. **Death totals**: g_mercDeathsTotal (accounts +0x00) += `dead_mercs`; g_nativeDeathsTotal (+0x50) += `dead_allies`
   + `dead_npc`; g_enemyDeathsTotal (+0x6C) += `enemy_deaths` (all i32, wrap-around).
2. Plants (`calc_process_cap`), Territory (`recalc_territory`), then **Pay the day** (Day accounts): g_balance =
   `prev_balance` + g_dayNet. This is the only place where a day's pay and the natives' wages leave the balance;
   the morning's "Not enough funds" reads that balance.
3. **Rest of the team**: for each soldier record k = 0..7 (all 8, also empty ones): `wounds` (+0x185) = 0; byte
   +0x1B1 − 1 when it is not 0; `energy` (+0x18A) = 100; the word `fatigue` (+0x10C) = 0.
4. **Tappers on lost trees**: for each sector s = 0..59 with `flags` bit 0 set, Connected to home and `tappers`
   (byte 3) ≠ 0: a = (3 × `workable` (byte 7)) as a byte (low 8 bits); when a < `tappers` (unsigned): e = `tappers`
   − a; `tappers` = a; g_tappers (+0x40) − e and g_workers (+0x48) − e. (No other total is computed again.)

The replay does not read or write these fields (it works on `sector_replay`), so the port runs the four steps
together before the replay; the original runs 1 and 2 when the replay screen opens, 3 after it, 4 after the replay.
Then (both): the guards on the way (`sector_state` byte 5 of s = 0..59) = 0, and the day's verdict.

The replay screen's panel (POSTPAN.PCC) also shows numbers in the original (DrawPanelPccFields 0x36010 with the
field table 0x91234: among them g_sectorsOwned − `start_sectors`, g_treesOwned − `start_trees`, g_treesWorkable −
`start_workable`, `dead_allies` + `dead_npc`). These are the number fields 31..42 (field table 0x93FD8, 19 bytes
each: x, y, box x, box y, digits, fg, flag, shadow, background, value address); all 3 digits, shadow 0, no background:

| k | value | at (x, y) | fg |
|---|---|---|---|
| 31, 32, 33 | `dead_mercs`, `dead_allies` + `dead_npc`, `enemy_deaths` | (78, 161), (78, 173), (78, 185) | 50 |
| 34, 35, 36 | g_mercDeathsTotal +0x00, g_nativeDeathsTotal +0x50, g_enemyDeathsTotal +0x6C | (108, 161 / 173 / 185) | 50 |
| 37, 38, 39 | g_sectorsOwned +0x74, g_treesOwned +0x18, g_treesWorkable +0x14 | (189, 161 / 173 / 185) | 50 |
| 40, 41, 42 | g_sectorsOwned − `start_sectors`, g_treesOwned − `start_trees`, g_treesWorkable − `start_workable` | (219, 161 / 173 / 185) | money colour (0x79 below 0, else 0x32) |

The values are taken after steps 1 and 2 above. Evidence: mappanel_diff --port replay cases (0x36010 (31, 42)).

Evidence (2026-10-28, asm): DayEndScreen? 0x65BD4: byte 0x22A043 (accounts +0xA3, the A.I.M. link) = 1,
PlayXmiMusic, ReplayScreenInit? 0x64274, SetMusic, 0x455B4 (step 3), DayReplay 0x652B8, ShowMouse?, LoadMainSound,
0x65AB0 (step 4), the byte-5 clear 0x65C27..0x65C37, DayEndVerdict. ReplayScreenInit? 0x642EE..0x64341: the three
death totals (0x229FA0, 0x229FF0, 0x22A00C from 0x229AE0, 0x229AE4 + 0x229AE8, 0x229AF0), CalcProcessCap 0x41574,
RecalcTerritory 0x50EA8, ApplyDayLedger 0x51210; then 0x229ADC / 0x229AC8 / 0x229AD0 (the panel differences)
and 0x229AEC (allies + NPCs). 0x455B4: per record +0x185 = 0, +0x1B1 dec when ≠ 0, +0x18A = 0x64, word +0x10C = 0.
0x65AB0: owned (g_sectorTbl9 b0 bit 0), SectorConnectedToHome 0x41178, b3 ≠ 0, `mul` of b7 by 3 (AL), compare
unsigned, b3 = AL, g_tappers 0x229FE0 and g_workers 0x229FE8 − (b3 − AL). Found 2026-10-28: the port had no Pay
the day at the day end (the balance never paid the day).

### The day-end screen

After the day replay and the day's verdict (result 0) and its bonus (`day_bonus`), the screen waits for the
player in two parts. The same loop runs in both parts; each pass: read the mouse, then at most one key.

**Keys** (both parts; the key byte is taken as upper case): `L` = panel.md "Say it again (key L)"; `T` = "Key T"
(the Subtitles switch and its message box, as on the A.I.M. screen); Alt-X (the extended key 0x2D) = panel.md
"Exit to DOS" with argument 0. All other keys do nothing (Enter too).

**The OK buttons.** Two screen buttons with the same faces as the finance page's back button, but with the
elements 0x66 (up) and 0x65 (pressed) and the overlay element 0x3A at (dx, dy) = (5, 4) (up) or (4, 3) (pressed):
button 15 at (266, 174) on the replay screen and button 16 at (279, 166) on the payroll screen. The hit area is
(x, y) .. (x + 25, y + 15) (the size of element 0x65). The up face is already in both pictures. A left press in the
hit area: the press sound, the pressed face, then nothing until the left button is up (also when the mouse leaves
the button), then the up face, the release sound, and the button acts (strategic.md "Button faces" of the A.I.M.
screen: the same button routine).

Front-end (2026-10-29): the window waits for both OK buttons (stderr `day-end wait B` when the wait starts,
`day-end ok B` when the button acts; smoke window_day grabs `window_day_ok15.png` / `_ok16.png` and clicks them); a
play script does not wait. The window also takes the keys L / T / Alt-X (stderr `day-end again`, `day-end msg
Text  on|off`, `day-end ask exit`; yes = the quit line, no office) and on the payroll screen the face tooltip
"fire" (`day-end tip N`) and the "Fire NICK?" question (`day-end ask fire k`, `day-end fire k [no]`; yes shows the
fired merc's balloon at the face, then the payroll picture again with the team slot marks) (2026-10-29).

**Part 1 (the replay screen):** only button 15 acts (other presses do nothing). It starts "The morning" (all 12
steps, the payroll screen first). Then part 2.

**Part 2 (the payroll screen):** in each pass, in this order:
1. Button 16 acts: part 2 ends (the day-end steps below).
2. Else, when the left or the right button is down: for team slot k = 0..7 with the mouse in (11 + 37 k, 20) ..
   (47 + 37 k, 46) (inclusive) and soldier k `inCurSector` (+0x182) ≠ 0: the tooltip goes (Tooltip box Hide); the
   screen part 0 is saved; the merc-face question box (panel.md) with the text "Fire <nickname of soldier k>?", the
   profile and the nickname of soldier k. Yes: when Subtitles is off, the screen part 0 comes back; then **Fire the
   merc** ("Firing a merc", below) with soldier k, x = 11 + 37 k, y = 20 (the quote balloon at the face); the host
   loads the panel sheet again; Close up the team (redraw 0). No: the screen part 0 comes back. Both answers: the
   payroll picture is drawn again (step 2 of "The payroll screen": the team as it is now) and the team slot marks
   (DrawTeamSlotMarks 0x4508C: for k = 0, 1, .. while soldier k `inCurSector` ≠ 0 and k < 8: element 0x67 at
   (13 + 37 k, 20) when its profile is below 60). The steps 5 and 8 of "Firing a merc" (the
   hire-day salary, CalcLedgerAll) do not run here: the day's money on the payroll panel does not change.
3. Else (no button down): for k = 0..7 with the mouse in (12 + 37 k, 20) .. (42 + 37 k, 46) and the soldier of team
   slot k `inCurSector` ≠ 0: the Tooltip box number k + 1 at (13 + 37 k, 20, 41 + 37 k, 45), text "fire", font 0.
   When the mouse is on no such face: the tooltip goes.

**The day-end steps** (after button 16):
1. When the balance (g_balance) is below 0: say q (guide_say (q, 3)) with q = 0xB4 when s8 S[5] < 2, else 0xB3;
   the result is 3 (the game ends as with the day's verdict result 3, the same end screens). Else the result is 0.
2. When the signed byte `first_hit` (the story flag that the first hit on an enemy sets to 2, combat.md) is below 2:
   the accounts byte +0x91 (the "byte at 0x22A031" of the Worker pool) + 1 (8 bits). So each day that ends before
   the first hit on an enemy lowers the next worker pools.
3. The palette fades out (24 steps); then the office (result 0) or the end (result 3).

Entry point `day_end` (port_api.md): steps 1 and 2; returns the result. World: accounts, the story bytes S,
`first_hit`. Host op 302 guide_say.

Evidence (2026-10-28, asm): DayEndScreen? 0x65BD4: the loop 0x65C57..0x65F58 (MousePoll? 0x678E8, KeyPressed
0x5616C, GetKey 0x560F4, toupper 0x754D7: 0x4C → SpecialNpcSpeech? 0x29D54, 0x54 → ToggleMessage 0x3F994(0x9ADD1
"Text ", 0x1163BD), 0 then GetKey 0x2D → ExitToDosPrompt 0x38084(0); 0x0D falls through); part flag [esp + 0x8C]:
0 → 0x67CD8(15, 1) → the 12 morning calls 0x65CE2..0x65D19, part 1; 1 → 0x67CD8(16, 1) → part 2 ends; else
g_mouseButtons 0x229CF4 & 3: faces via MouseInRect? 0x67C0C(11 + 37 k, 20, 47 + 37 k, 46) over g_soldiers + 0x200 k,
HideTooltip 0x23420, 0x67BF0, SaveScreenArea 0x467E4(0), sprintf "%s%s?" 0x91260 with "Fire " 0x99EAF and +0x16E,
YesNoPrompt? 0x28C84(text, +0x179, ECX 1, 0); yes: Subtitles 0x1163BD = 0 → RestoreSaveArea0 0x28698; FireMerc
0x5097C(+0x17A, EDX 11 + 37 k, EBX 0x14); LoadPanels 0x46EC4; TeamCompact? 0x50750(0); no: RestoreSaveArea0;
both: DrawBassMap 0x4420C, DrawTeamSlotMarks? 0x4508C. No button: MouseInRect?(12 + 37 k, 20, 42 + 37 k, 46) over
the team pointers 0x100958, ShowTooltip 0x23264(k + 1, 13 + 37 k, 20, 41 + 37 k, 45, "fire" 0x99E61, "", "", 0),
none → HideTooltip(byte 0x93EA8). After the loop: Subtitles = 0 → LoadPanels; 0x62304 (g_balance 0x229FC4 < 0:
GuideSpeech? 0x5F5AC(0xB4 when byte 0x22A045 < 2 else 0xB3, 3), AL = 3; else 0) → end code 0x93449; byte
0x22A1F5 (`first_hit`) < 2 → inc byte 0x22A031; FadePalette? 0x52908(0xEDA08, 0, 24, 256, 0). Button table
0x9345A entries 15 (0x934F0: 266, 174, 0x66, 0x65, kind 1, 0x3A, 5, 4) and 16 (0x934FA: 279, 166, same); element
0x65 = 26 x 16 (element table 0x93732).

## The office

After the morning the original shows the office (a room with a bed, a calendar, a desk and a door) and waits until
the player clicks the bed (Sleep). The front-end draws it from these rules (world_load.md section 9 step 2).

**Files.** All pictures are members of `DAT/OFFICE.DAT` (a Sir-tech archive, as MAIN.DAT): ROOM.PCX and ROOMDARK.PCX
(320 x 200, palette = the last 768 bytes of the file, each value shifted right by 2 = 6-bit VGA values), CASE1.PCC,
CASE1DRK.PCC, CASE2.PCC, CASE2DRK.PCC and OFFBRUSH.PCX (640 pixels wide, the highlight frames). Every picture is
copied with all its pixels (0 too).

**The room picture** (dark d = 0 or 1): R = ROOM.PCX (d = 0) or ROOMDARK.PCX (d = 1). The A.I.M. link is the
`accounts` byte at +0xA3 (not 0 = the merc agency can be reached from the desk). Link ≠ 0: CASE1.PCC (d = 1:
CASE1DRK.PCC) at (152, 91); link = 0: CASE2.PCC (d = 1: CASE2DRK.PCC) at (127, 137). Then, when `day` ≠ 0: Text
(panel.md Drawing primitives) of the decimal `day`, centred at x 195, y 46, fg 0xFF (d = 1: 0x1A), shadow 1, font 2,
mode 300 (the calendar's number). The palette is the palette of R.

**Hot spots** k = 0..7, rectangles (x0, y0, x1, y1), the mouse is inside when x0 ≤ x ≤ x1 and y0 ≤ y ≤ y1; the
hot spot under the mouse is the first k that holds it (none = 8). Label, highlight sizes w x h at (x, y), and the
four highlight frames (sheet positions in OFFBRUSH.PCX):

| k | rectangle | label | w x h | at | frames 0..3 |
|---|-----------|-------|-------|----|-------------|
| 0 | 178, 40, 217, 60 | Save Game | (none) | | |
| 1 | 178, 62, 217, 82 | Restore Game | 60 x 24 | 169, 60 | (100,103) (160,103) (220,103) (280,103) |
| 2 | 152, 113, 202, 123 | Contact A.I.M. (link ≠ 0) | 72 x 33 | 152, 91 | (200,66) (272,66) (552,0) (552,33) |
| 3 | 152, 91, 202, 123 | Contact A.I.M. (link ≠ 0 and highlight = 2) | (as 2) | | |
| 4 | 0, 122, 102, 162 | Sleep | 100 x 39 | 0, 130 | (0,0) (0,39) (0,78) (100,0) |
| 5 | 53, 60, 112, 80 | View Team | 50 x 32 | 53, 61 | (100,39) (150,39) (100,71) (150,71) |
| 6 | 0, 21, 24, 70 | Control Panel | 38 x 66 | 0, 18 | (200,0) (238,0) (276,0) (314,0) |
| 7 | 270, 25, 319, 166 | Leave | 50 x 147 | 270, 21 | (352,0) (402,0) (452,0) (502,0) |

A label that does not apply (k = 2, 3 outside their condition, k = 8) is empty.

**The office loop.** Start: the room picture (d = 0) on the screen, highlight H = 0 (none). On every mouse move
(and after a highlight change or a closed box): k = the hot spot under the mouse; the label of k is drawn with Text
at x = min(mouse x, 295) (a mouse x above 296 gives 295), y = mouse y - 10, not centred, fg 0xFE, shadow 1, font 1,
mode 300, over the picture (the label of the last move is removed first). Before the label is drawn:
- when H ≠ 0, H ≠ k, and not (H = 2, k = 3, link ≠ 0): the highlight goes off: frames 3, 2, 1, 0 of H are copied
  to its place (w x h from OFFBRUSH.PCX, all pixels), then the saved rectangle under it is put back; H = 0.
- when H = 0, 1 ≤ k ≤ 7, k ≠ 3 and (k ≠ 2 or link ≠ 0): the rectangle under the highlight of k is saved, frames 0,
  1, 2, 3 are copied to its place one after the other; H = k (frame 3 stays).
Each frame is on the screen at least 90 ms (the loop waits until more than 89 ms passed since the last frame).
Every more than 80 ms the palette entries 0xE1..0xEF rotate by one (as host op 206 pal_cycle, port_api.md).

A left button press on hot spot k:
- 4 (Sleep): when Team size (Not enough funds) is 0, the message box (panel.md "Message box", darkening table 31,
  frame colour 0xFF) shows "You try to sleep, but the worry of not having any men on duty tomorrow keeps you
  awake..." until a key or a click; the office goes on. Else the room picture with d = 1 is drawn (dark palette),
  shown for 700 ms (a key or a click ends it early; Fades of the office), and the office ends with result 4 (Sleep).
- 5 (View Team): the team ledger (below, "The team ledger").
- 0, 1: Save Game and Restore Game (below, "Save Game and Restore Game"). After a Restore Game the office uses the
  loaded world (faces, the A.I.M. link, the calendar and the team size for Sleep).
- 7 (Leave): the office ends with result 7: back to the main menu (world_load.md section 12).
- 2, 3: the A.I.M. screen (k = 2, or k = 3 when H = 2; link ≠ 0 only; "The A.I.M. screen").
- 6 (Control Panel): the switch box (below, "The switch box (Control Panel)").
Key V: the message box with the text at 0x90E04 ("Release  v1.13b  Sep 07-95" in 1.13b; A.I.M. key V uses 0x903F4, which 1.13b left at "Release  v1.13  Sep 05-95"). Alt+X: the game ends at once (no question, no file deleted, see
Fades), with the line "Normal exit." on the text screen (panel.md "Exit to DOS" step 4 with that line; OfficeScreen
0x4F4F1: 0x382AC(0x90DF4)).

**Highlight sounds.** The office loads the main bank when it opens (sound.md "Main bank"). Each hot spot k has a
sound for on and one for off (main bank effects; - = none):

| k | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| on | - | 12 PA1 | 9 LAP | 9 LAP | 8 CVR | 14 SL1 | 10 ME1 | 6 CR1 |
| off | - | 13 PA2 | 9 LAP | 9 LAP | 8 CVR | 15 SL2 | 11 ME2 | 7 CR2 |

A highlight going on starts its on sound just before frame 0. A highlight going off starts its off sound just before
frame 3 for k = 1, 4, 5, 7, and just after frame 0 for k = 2, 3, 6. Each plays as op 60 sfx (effect, rate 8000,
volume 118 for k = 4 (on sound 8), else 88, once). Script output: `screen sound NAME` per sound (main bank member
name), in order.

The music is in music.md. The mouse pointer is shape 202, the grey arrow (panel.md "The mouse pointer"; shape 0 on the
View Team page).

**Fades of the office.** Each fade is the fade of ending.md op 24 / 25 (n steps, one level per vertical retrace) on
the colours shown at that moment:
- The office opens (and opens again after the A.I.M. screen): after the room picture, fade in (24 steps, 0 → 256).
- Sleep (with a team): the dark room picture shows for 700 ms (a key or a click ends this wait early), then fade
  out (56 steps, 256 → 0).
- Leave (hot spot 7): fade out (56 steps, 256 → 0).
- The A.I.M. screen (hot spots 2, 3): fade out (24 steps, 256 → 0) before it opens.
- Alt+X: fade out (24 steps, 256 → 0), then the program ends.
The team ledger has its own fades (below). Evidence (2026-10-27, asm; the fade 0x52908 takes EAX = colour slot
0xEDA08, EBX = n, ECX = from, one stack argument = to): InitOfficeScreen? 0x4EDE3 (24, 0, 256; only with argument 0,
which OfficeScreen passes at the start and after AimScreen), OfficeScreen 0x4F2B0 / 0x4F307 (A.I.M.: 24, 256, 0),
0x4F38E (Sleep: 56 = EBX of DrawRoomScreen (1, 0x960, 0x38), 256, 0; after WaitMs? 0x67F48 (700): the wait ends at
700 ms or at KeyOrClick? 0x67E44), 0x4F494 (Leave: 56, 256, 0), 0x4F4EC (Alt+X: 24, 256, 0).

Evidence (2026-10-25): OfficeScreen 0x4EE64 (hot spots g_officeHotspots 0x960D2, labels 0x9A3DD.., label x clamp at
0x4F13D, DrawText 0x358D0 (label, screen, 320, x, y - 10, 0, 0xFE, 1, 1, 300), Sleep 0x4F334: TeamSize 0x513B8,
StatusMessage 0x989DF, DrawRoomScreen(1), wait 700 ms, result 4; Leave result 7, PlayLoop 0x4FB94 then leaves),
InitOfficeScreen 0x4ED1C (OFFICE.DAT, OFFBRUSH.PCX at pitch 640, ROOM.PCX palette), DrawRoomScreen 0x4C304 (names
0x4B6E0, link byte 0x22A043 = accounts +0xA3, case at panel offsets 0x7258 / 0xABBF, darkening byte 0x284F08 = 31),
0x4C2B4 (calendar number), highlight 0x4C0C8 (sounds: on table bytes 0x9609D + k, off 0x960A5 + k, 99 = none; volume 0x76 when the on byte is 8, else 0x58; PlaySfxSlot(s, 8000, v, 1); off: before the frames, except k = 2, 3, 6 after frame 0) / frame 0x4BFFC (tables 0x960AE sizes, 0x96196 places, 0x961BA
frames, 90 ms timer 0x22A3FC), palette cycle 0x4C26C (80 ms, 0xE1..0xEF), MouseInRect 0x67C0C (inclusive),
PCX reader 0x52F9C (all pixels). A live run (daysave_diff --orig) showed the office with the bed highlight and
the label "Sleep" at the mouse.

### The team ledger (View Team)

A left press on hot spot 5 (View Team) shows the team ledger: a page with the team on the left and the accounts on
the right. The office waits until the page closes and then goes on (the label is drawn again; the highlight state H
does not change, but the room picture under it is drawn again without the highlight).

**The count.** After step 3 (the mercs) and before step 4, all accounts are counted again (Day accounts "All
accounts", CalcLedgerAll); step 4 shows the new values and the world keeps them. (Step 3 shows `salary2` as it was
before the count.)

**The page** (on the screen, 320 x 200, all text with mode 300; Text and Number as in panel.md "Drawing
primitives"; "Number v with d digits" pads with `^` on the left):
1. VIEWISLE.PCX (DAT/OFFICE.DAT, 320 x 200, all pixels) at (0, 0); the palette is the palette of VIEWISLE.PCX.
2. "Current Team" centred at (95, 8), fg 0xFF, shadow 1, font 2. "Day #" at (203, 8), fg 0xFF, shadow 1, font 2.
   The `day` (the i32 at `accounts` +4) as a number with 3 digits at (233, 8), fg 0xFF, shadow 8, font 2.
3. The mercs: team slots k = 0, 1, ... while slot k is `inCurSector` and k ≤ 7 (the first slot not in the sector
   ends the list). x = 12 for k < 4, else 100; y = 45 × (k mod 4). S = soldier k.
   a. The **health bar**: the Assignment health bar (Assignment picture) of S at (x + 30, y + 22), drawn in the
      office's room buffer (the room picture with the highlight of View Team, as on the screen before the press),
      but without the gap rows (element 88 is not drawn; y still moves down by the gap). Then the drawn rows, 3
      pixels wide from (x + 30, y + 22 + gap) to row y + 46, are copied from the room buffer to the same place on
      the screen (so the gap shows VIEWISLE.PCX). The row count is nh + nw + ne
      (Assignment health bar): nh + nw when `hpMax` − `hp` ≤ `wounds`, else the larger of nh + nw and nm. It is below
      0 only with `wounds` ≤ −4 (`hp` never goes below 0: every damage step stops at 0; `wounds` go below 0 by
      bleeding with the bleeding mark, combat.md "Bleeding", or by the wound formula of a hit, combat.md "Taking
      damage"). Then the original's copy loop counts down from the
      negative count (about 4 G rows, BlitCopy 0x52AE0 `dec`/`jne`) and runs off the end of memory: a **stop point**
      (port_api.md "Division by zero"); the front-end copies nothing.
   b. The **face**: the 28 x 25 pixels of face buffer k from column 2, row 2 to (x, y + 22), all pixels written.
   c. A Frame (x − 1, y + 21, x + 28, y + 47) with colour 0x5B.
   d. The nickname (up to its 0 byte) centred at (x + 14, y + 49), fg 0x0E, shadow 5, font 0.
   e. The texts "Sal", "Hea", "Lvl" and "Kills" at (x + 34, y + 23), (x + 34, y + 29), (x + 34, y + 35) and
      (x + 34, y + 41), fg 0x0E, shadow 5, font 0.
   f. Numbers, fg 0x28, shadow 8, font 0: `salary2` (unsigned 16-bit) with 5 digits at (x + 55, y + 23); signed
      `hp` with 4 digits at (x + 59, y + 29); signed `level` with 4 digits at (x + 59, y + 35); `kills` (soldier
      +0x1D9, unsigned byte) with 4 digits at (x + 59, y + 41).
4. The accounts (i32 values of `accounts` at the given offsets). Labels with fg 0xE5, shadow 1, font 0 at x 203;
   values with fg 0xFF, shadow 8, font 0 (4 digits at x 293 unless stated):

| y | label | value |
|---|-------|-------|
| 24 | "DIFFICULTY:" | text at x 285: `difficulty` 0 = "  EASY", 1 = "NORMAL", 2 = "  HARD", other = "UNKNOWN" (unsigned) |
| 32 | "Sectors Won:" | +0x78 |
| 40 | "Sectors Lost:" | +0x7C |
| 48 | "Sectors Owned:" | +0x74 |
| 56 | "Last Sector Gained:" | +0x80 = 59: text "  NONE" at x 285; else the number +0x80 plus 1 |
| 64 | "Factory Capacity:" | +0x84 |
| 72 | "Total Trees Owned:" | +0x18 |

   Then, all with fg 0xFF; labels with shadow 1, values with shadow 8:
   - "Expenses" at (203, 87), font 1. "Guards" (208, 97), "Tappers" (208, 104), "Team" (208, 111), font 0;
     their values +0x5C, +0x58, +0x38 with 5 digits at x 289, font 0.
   - "TOTAL" at (203, 121), font 1; the sum +0x5C + +0x58 + +0x38 with 5 digits at (279, 121), font 1.
   - "Revenue" at (203, 137), font 1. "Trees" (208, 147), font 0; +0x20 with 5 digits at (289, 147), font 0.
   - "TOTAL" at (203, 157), font 1; +0x20 with 5 digits at (279, 157), font 1.
   - "FUNDS" at (203, 177), font 1; +0x24 with 6 digits at (273, 177), font 1.
   (The texts are not centred.)

**Closing.** The page polls the mouse and the keys. A press of the left or the right button closes it. A key k
(letters a..z as A..Z) closes it when k is Return (13), Escape (27) or Space (32); a key with the value 0 is an
extended key: its second value is read and nothing happens. Key L shows the last quote again
(panel.md "Say it again (key L)": the box shows over the page, and the page stays open). After the page: the
room picture (dark 0, Office "The room picture") is drawn again with the palette of ROOM.PCX, and the office goes
on.

**Fades.** Each fade is the fade of ending.md op 24 / 25 with n = 24 steps (25 levels, one per vertical retrace)
on the colours shown at that moment:
1. Before the page is drawn: fade out (256 → 0) of the office colours (the room picture stays on the screen).
2. After the page is drawn with the palette of VIEWISLE.PCX: fade in (0 → 256).
3. After the closing press or key: fade out (256 → 0) of the page.
4. After the room picture is drawn again with the palette of ROOM.PCX: fade in (0 → 256).
Evidence (2026-10-27, asm): the four calls of the fade 0x52908 (EAX = colour slot 0xEDA08, EDX = 0, EBX = 24,
ECX = from, one stack argument = to) at 0x4C44A (256, 0), 0x4CE0C (0, 256), 0x4CE9B (256, 0) and 0x4CF00
(0, 256; EBX stays 24 from the call of DrawRoomScreen 0x4C304).

Evidence (2026-10-25): DrawOfficeLedger 0x4C41C (VIEWISLE.PCX "xviewisle.pcx" + 1 at 0x90C5C, loader 0x532BC at
screen pointer 0x95678 = 0xA0000, palette 0x53300 / 0x52820; texts 0x9A16D "Current Team", 0x9A187 "Day #",
DrawNumber 0x36090 of the dword 0x229FA4; merc loop from g_soldiers, +0x182 test, columns 12 / 100, rows 45 apart;
HealthBar? 0x47144 (ECX 0 = no gap, stack 1 = copy 3 x 25 from g_panelBuf 0x93E60 to the screen), face buffers
[0x93428 + 4 k] + 0x42 BlitCopy 0x52AE0 28 x 25, DrawFrame 0x46C44 colour 0x5B, nickname +0x16E, labels 0x9A1A1..
0x9A1EF, numbers +0x112 word, +0x183, +0x192 (dword +0x18F >> 24), +0x1D9; CalcLedgerAll 0x51178 after the merc
loop and before the accounts are drawn; g_difficulty 0x22A0CC unsigned compare; key loop 0x4CE2x, toupper
0x754D7, SpecialNpcSpeech? 0x29D54 on L; exit: DrawRoomScreen 0x4C304(0)); OfficeScreen 0x4EE64 case 5.
validate/diff/ledger_diff.py.

### Save Game and Restore Game (the slot list)

The office hot spots 0 (Save Game) and 1 (Restore Game) open the **slot list**: ten game slots 0..9, each one a file
`GAMEn.SAV` (n = 0..9) in the save directory (world_load.md section 10). A slot file is a day save (savegame.md),
written and read as in world_load.md sections 3 and 10. Mode M = 1 for Save Game, 0 for Restore Game.

**Slot headers.** For n = 0..9: when `GAMEn.SAV` can be opened, its bytes 0..1 (u16, the save's `day`) and 2..27
(the 26-byte save name) are the slot's day and name, and the slot is in use; else day = 0xFFFF and the name is
empty. C = the number of slots in use. The **list** L holds the numbers of the slots in use in rising order
(L[0], L[1], ...; the rest are none). In mode 0 with C = 0 the list does not open: the message box (panel.md,
darkening table 31, frame colour 0xFF) shows "No saved games exist!" and the office goes on.

**Rows.** Row r = 1..10 shows (mode 1) slot r - 1, or (mode 0) slot L[r - 1]; in mode 0 a row with no list entry
does not exist. Row r lies at y_r = 26 + 14 r (text) and its picture at y_r - 3. Its hit rectangle is
(43, y_r - 3, 276, y_r + 9) (inclusive). "Day text" of a slot = "Day " followed by its day as a signed 16-bit
number in decimal, right-aligned in 2 characters (a space first for 0..9), and ":" (for example "Day  3:"). The
name text = the name bytes up to the first 0 (names without a 0 in their 26 bytes are not described).

**The list picture**, drawn on the screen as it is (the office picture under it); TEXTSLAB.PCC is a member of
`DAT/OFFICE.DAT` (a PCX picture 233 x 12; all pixels copied):
1. TEXTSLAB.PCC at (43, 19); Text (panel.md "Drawing primitives") "Select Game Slot For Save" at x 93 (mode 1) or
   "Select Game to Restore" at x 99 (mode 0), y 19, fg 0xFF, shadow 8, font 2, mode 300.
2. For each row r that exists: TEXTSLAB.PCC at (43, y_r - 3); when the slot is in use (day ≠ 0xFFFF in mode 1;
   always in mode 0): the day text at (47, y_r) and the name text at (103, y_r), both fg 0x28, shadow 1, font 1,
   mode 300; when it is not in use (mode 1 only): "Empty" at (47, y_r), fg 0xCA, shadow 0, font 1, mode 300.
B44 = 44 spaces, B32 = 32 spaces.
- **Lit row** r (r ≥ 1, the row exists): B44 at (47, y_r) with fg 0xFF, shadow 8, font 1, mode 0xF3 (Text with a
  background, colour 0xF3); then for a slot in use the day text at (47, y_r) and the name text at (103, y_r), fg
  0xFE, shadow 8, font 1, mode 300; for an unused slot (mode 1) "Empty" at (47, y_r) with the same colours.
- **Plain row** r (r ≥ 1, the row exists): B44 as for a lit row; then for a slot in use the day text and the name
  text as in step 2 (fg 0x28, shadow 1, font 1, mode 300); for an unused slot "Empty" as in step 2 (fg 0xCA, shadow 0).
Row 0 (no row) draws nothing in both.

**Input.** The list reads the mouse in **polls** (position and the buttons held: bit 0 left, bit 1 right) and the
keyboard as a stream of **key bytes**: a normal key gives its ASCII byte; an extended key (arrows, Home, End,
Delete ...) gives 0 and then its scan code (Home 0x47, Left 0x4B, Right 0x4D, End 0x4F, Delete 0x53). "Read a key
byte" takes the next byte of the stream and waits for one when there is none. The **key reader** K: read a key
byte c; when c = 0, read a second byte s: s = 0x4F, 0x47, 0x4B, 0x4D, 0x53 give ']', '[', '<', '>', 0x7F as the
result, any other s gives 0. When c ≠ 0: the result is c when c is one of the **allowed keys**, else 0. Allowed
keys in mode 1: a..z, A..Z, space, 0..9, `: ; ? , . ! - & ' /`, 8 (Backspace), 13 (Enter), 27 (Escape); in mode 0
only 27. "Wait for the left (right) button": polls until a poll has bit 0 (bit 1) clear.

**The list loop.** First wait for the left button (the click that opened the list), then read the slot headers
(mode 0 with C = 0: the message box above, no list). Then the list picture. Then, with
the lit row H = 0:
1. Poll. When a key byte is waiting: R = K; R = 27 cancels (the list ends, nothing chosen).
2. h = the first existing row r = 1..10 whose hit rectangle holds the mouse (0 = none). When h ≠ H: the plain row
   H, then the lit row h; H = h.
3. Bit 0 set: the chosen row = the first existing row whose hit rectangle holds the mouse (none: no choice). Else
   bit 1 set: cancel. Neither a choice nor a cancel: go on at 1.
4. When bit 0 is set: wait for the left button. A cancel (also with a chosen row in the same poll) ends the
   list. Else, with the chosen row c: mode 0: the chosen slot is L[c - 1]; the list ends. Mode 1: the slot is c - 1 and the **name input** (below) runs on row c. When it ends with Enter, the list ends. When it is
   cancelled: h = the row under the mouse; when h ≠ H: the plain row H, the lit row h; then H = 0 and go on at 1
   (so the next poll lights the row under the mouse again).
At the end the screen gets the office picture back (the front-end draws the office again).

**The name input** on row c (mode 1). The save name S (26 bytes) becomes all 0, then the name of slot c - 1 (up
to its first 0). The cursor p = (the length of S) - 1, but 24 when that is above 23. Draw: B44 at (47, y_c) (fg
0xFF, shadow 8, font 1, mode 0xF3); the day text of the current `day` (the
full value) at (47, y_c), fg 0x28, shadow 1, font 1, mode 0xF3; S at (103, y_c), fg 0xFE, shadow 8, font 1, mode 0xF3; the
**text cursor** at p + 1. The text cursor at q: the character = S[q] when q < length of S, else a space; x = 103 +
the sum over S[0..q-1] of 5 for a space, '-', '.' or ':' and 6 for any other byte; Text of that one character at
(x, y_c), fg 0xFE, shadow 8, font 1, mode 0xCA. Then the loop:
1. Poll. Bit 1 set: wait for the right button, the input is cancelled (the steps below still run for a waiting key).
2. When a key byte is waiting: R = K, then:
   - R = 0: read one more key byte k (raw, not through K). k = 0x47 (Home): p = -1; 0x4B (Left): p = p - 1 when
     p > -1; 0x4D (Right): p = p + 1 when p < 24 and p + 1 < the length of S; 0x4F (End): p = (the length of S) - 1,
     but 24 when above 24; 0x53 (Delete): S[i] = S[i + 1] for i = p + 1 .. 24. Other k: nothing.
   - R = 8 (Backspace): when p > -1: S[i] = S[i + 1] for i = p .. 24, then p = p - 1.
   - R = 13 (Enter): the input ends (accepted).
   - R = 27 (Escape): the input is cancelled.
   - Any other R (also ']', '[', '<', '>' and 0x7F from the extended keys): when the length of S is below 24:
     S[i] = S[i - 1] for i = 25 down to p + 2, S[p + 1] = R, p = p + 1.
   Then, when the input is not cancelled: B32 at (103, y_c) (fg 0xFF, shadow 8, font 1, mode 0xF3), S at
   (103, y_c) (fg 0xFE, shadow 8, font 1, mode 0xF3) and the text cursor at p + 1.
3. The loop ends after Enter or a cancel.
S stays the game's save name also after a cancel (the next day save writes it as its bytes 2..27).

The character 0x7F (Delete, below) is a **stop point** (port_api.md "Division by zero"): fonts 0 and 1 give it
the width 0 (also 0x7B..0x8E; font 2: 7), and the glyph copy of the text draw counts the columns down with "decrement, stop at 0"
(BlitRemap and BlitRemapKey alike), so a width of 0 runs 2^32 columns and writes past all memory. The original
crashes at the first draw of a name that holds 0x7F (the redraw after the key, a save slot row, or the text cursor
on it). The front-end writes "stop point" to stderr there and draws the glyph with no columns.
Result: the arrow keys, Home, End and Delete do not move the cursor; they put the characters '<', '>', '[', ']'
or 0x7F into the name. The cursor moves only when a key that is not allowed (for example '#', or F1) is followed by
the letter G, K, M, O or S, which is then taken as Home, Left, Right, End or Delete (a bug of the original, issue #2).

**After the list.** Save Game, a slot n chosen (not cancelled): the game is saved to `GAMEn.SAV` as a day save
(world_load.md section 10) with bytes 2..27 = S; first the message box (panel.md "Message box", darkening table 31,
frame colour 0xFF; steps 1 and 2 only: no wait, no sound, the screen is not put back) shows "Saving..." over the
office picture. Restore Game, a slot n chosen: the world is loaded from `GAMEn.SAV` as in
world_load.md section 3, but the save name S and the first two Control Panel switches (Sound FX and Music, save
bytes 21538 and 21539) keep their values from before; faces are loaded again; before the load the same box shows "Restoring game...". In both cases the office picture is drawn again and the office goes on.

Evidence (2026-10-25): OfficeScreen 0x4EE64 cases 0 (SelectSaveSlot? 0x4D798(1), then SaveGame 0x4EBC8(name, 0)
unless cancel byte 0x28ECDC) and 1 (RestoreGameDialog? 0x4E700: SelectSaveSlot(0), 0 = StatusMessage 0x9A375,
LoadGame 0x4E964(name)); the boxes: SaveGame 0x4EBDE / LoadGame 0x4E98E, when EDX = 0: ShowMessage? 0x27B98 ("Saving..." 0x9AD35 / "Restoring game..." 0x9ADEB, 0, 0, CL 1 = save area, stack 0 = no put back, 0, 0 ms; front-end: Office::busy_picture, script line `slots box TEXT`); SelectSaveSlot 0x4D798 (TEXTSLAB.PCC "textslab.pcc" 0x90CB0 via loader 0x532BC at pitch
0xE9, wait 0x67C54, ReadSaveSlotHeaders 0x4DB34 (game%d.sav 0x90CCC, u16 to 0x28ECE4, names 26 B at 0x28ECF8, list
0x28E060), allowed keys 0x98877 / 0x90CC0 into 0x28E0A8, BlitCopy 0x52AE0 233 x 12, texts 0x98967 / 0x988EF,
"%s%2d:" 0x90CC4 + "Day " 0x99AD3, "Empty" 0x99EC9, DrawText 0x358D0; screen back from g_panelBuf at the end);
SlotListLoop? 0x4DCCC (hit rows 0x4E24C, MouseInRect 0x67C0C (43, 37 + 14 k, 276, 49 + 14 k), plain row 0x4E4B0,
lit row 0x4E2C8, spaces 0x90D34 / 0x90D64 / 0x90D94); key reader SaveSlotKeyHandler? 0x4DC1C (tables 0x96090
"OGKMS", 0x96096 "][<>\x7F"; GetKey 0x560F4); SaveSlotNameInput? 0x4DEF4 (g_saveName 0x28EDFE, B44 0x90CDC, B32
0x90D10, cursor 0x4E660, right-button wait 0x67C68); LoadGame keeps g_options 0x1163B8 and 0x1163B9 and skips the
28-byte header. validate/diff/slots_diff.py.

### The switch box (Control Panel)

A left press on hot spot 6 (Control Panel) opens the switch box: a picture with lever switches for the 16 switches of
the tactical Control Panel (panel.md "The Control Panel (key F10, `opt` button)"; the same 16 bytes, save offsets
21538 + k, and the same names) and one close lever. The office waits until the box closes.

**Files** (DAT/OFFICE.DAT): SWICHBOX.PCC (282 x 200, the box) and PANBRUSH.PCX (398 x 130, the parts: the open
frames, the levers, the lamps and the close frames). All pixels are copied (0 too). The palette is ROOM.PCX's.
The screen picture S below starts as the office picture without the label (the highlight of hot spot 6 is on).

**Switch places.** Switch i = 0..19: x = 28, y = 26 + 15 i for i < 10; x = 137, y = 26 + 15 (i − 10) for i ≥ 10.
Switches 16..19 are the four bytes after the 16 (save offsets 21554..21557): they get a lever only (no lamp, no
name, no click).

**One switch** i with value v (0 or 1) on a picture: the lever, 26 x 7 from PANBRUSH (26, 74) when v = 0, (0, 74)
when v = 1, at (x, y); for i < 16 also the lamp, 6 x 4 from PANBRUSH (52, 74) when v = 0, (58, 74) when v = 1, at
(x + 10, y + 7), and the name (Text, panel.md Drawing primitives) at (x + 30, y + 2), not centred, fg 0x44 when
v = 0, 0x28 when v = 1, shadow 3, font 1, mode 300.

**Opening.**
1. The open frames of the box door: the 50 x 74 rectangle of S at (0, 9) is saved (U); then frames j = 0..3, 50 x 74
   from PANBRUSH (50 j, 0), are copied to S at (0, 9); S is shown after each frame, each at least 90 ms (no sound; game clock).
2. R = a copy of S (the room, columns 0..317 kept).
3. The box picture B: S with SWICHBOX.PCC copied to (0, 0) (columns 282..319 keep S). Each of the 20 bytes that is
   neither 0 nor 1 is set to 0 first; then every switch i = 0..19 is drawn on B with its value.
4. Zoom: 16 steps s = 1..16, each on R (the picture of step 2; steps draw over each other): B's 282 x 200 area is
   scaled to w x h = (4 + 17 s) x (20 + 11 s) at (0, y), y = 24 − 4 (s − 1), at least 0; nearest scaling as the
   speech balloon (panel.md "Speech balloon" step 5, source 282 x 200): sx = (282 · 2^22) / w, sy = (200 · 2^22) / h,
   source row of row j = (sy / 2 + j · sy) >> 22, source column of column i = (sx / 2 + i · sx) >> 22. Each step is
   shown at once (no wait).
5. The box: the 282 x 200 area of B copied onto the screen picture (columns 282..319: those of R); the ROOM.PCX
   palette (the palette cycle starts again from it); shown. This picture is P.

**Clicks** (left button press only; no keys, no Escape; a press acts once). The first switch k = 0..15 with the mouse
in (x, y) .. (x + 26, y + 7) (inclusive):
- k = 1 (Music) needs a music driver, k = 0 (Sound FX) a sound driver; else nothing.
- k = 0 while Sound FX is on and Subtitles (5) are off: Subtitles toggle first (they go on), then Sound FX.
- k = 5 while Sound FX is off: nothing (at least one of the two stays on).
- else switch k toggles.
A toggle of k: the switch sound (main bank effect 2 SWI, 11025 Hz, volume 20, once; it is not heard when Sound FX
is off); the byte flips; switch k is drawn again on P with the new value; then Music turned off while a theme
plays (playing theme > 0): the music stops; Music turned on with a wanted theme > 0: the wanted theme plays from the
start; Sound FX turned on: the switch sound once more (now heard). P is shown.

**Close lever**: a press with the mouse in (244, 31) .. (266, 96): the lever sound (main bank effect 3 LEV, 11025
Hz, volume 20, once); 9 frames f = 0..8, 22 x 130 from PANBRUSH (200 + 22 f, 0), copied to the screen at (244, 31),
each shown at least 90 ms. Then:
1. The screen gets R (318 x 200 from the left edge) by the **dissolve copy**: the pixels go straight to the display
   memory in a scattered order, with no pause and no wait for the display between them, so the copy lasts only as
   long as 63,600 single-pixel copies take and the end picture is R. A port shows R at once. The order, for
   completeness: a 256-byte table T is made once at program start (T[i] = i, then 1024 swaps of T[i] and T[j], i
   and j each = (rand() x 256 / 32767) mod 256 with the C library's rand); then for a = 0..255 and, inside, for
   i = 0..255: n = T[i] x 256 + T[(a + i) mod 256], y = n / 318, x = n mod 318, and the pixel (x, y) is copied when
   y < 200. Every n in 0..65535 comes once, so every pixel of R is copied.
2. Q = the office picture (the room, dark = 0, no highlight). The off sound of the door (main bank effect 0 IN,
   rate 8000, volume 88, once), then the open frames 3, 2, 1, 0 copied to Q at (0, 9), Q shown after each (at least
   90 ms each); then U is put back on Q at (0, 9) and Q is shown.
3. The office goes on with the plain room as its picture (the highlight state H stays 6, the saved rectangle under
   it is the plain room); the next pass acts as a mouse move (the mouse is not on hot spot 6, so the highlight of
   6 goes off).

Script line (port/ja1front): `office X Y 1` on hot spot 6 opens the box (output `office switchbox`); while it is
open, `office X Y 1` is a press on the box: output `switchbox k v` per toggle (k, new value), `switchbox closed` at
the close lever. JA1_SWITCHBOX_PNG = the box picture P after the last press.

Evidence (2026-10-26): OfficeScreen 0x4EE64 case 6 (0x468B0 label off, SwitchBoxScreen? 0x4D310, 0x467E4 label,
redraw flag local_28 = as a mouse move); SwitchBoxScreen? 0x4D310 (PANBRUSH "panbrush.pcx" 0x90C80 to the sheet
0x93E5C pitch 640; OfficeHighlight 0x4C0C8 (8, 1): place 0x961B6 (0, 9), size 0x960CE (50 x 74), frames 0x9623A
(0, 0) (50, 0) (100, 0) (150, 0), on sound byte 0x960A5 = 99, off sound byte 0x960AD = 0; BlitCopy 0x52AE0 (dst,
src, w, h, dpitch, spitch) of g_panelBuf 318 x 200 to 0x22A521 = g_backBuffer + 50 pitch 0x17C (U at
g_backBuffer + 0); "swichbox.pcc" 0x90C90 to g_panelBuf; values ≠ 0/1 cleared at 0x4D3EC; lever tables 0x9611A
(off) / 0x9611E (on), size 0x9612E (26 x 7), lamps 0x96122 / 0x96126 size 0x96136 (6 x 4), places 0x96142 + 4 i,
DrawText 0x358D0 (name 0x9675F + 12 i, x + 30, y + 2, fg 0x44 / 0x28, shadow 3, font 1, 300); zoom at
0x4D5B8..0x4D662: ScaledBlit? 0x52B7C (dst g_panelBuf + 320 y, sheet, 282, 200, 320, 320, w, h), y from 0x18
down by 4, w from 4 by 0x11, h from 0x14 by 0xB, 0x46BF4 (g_panelBuf to the screen) per step; PaletteSet 0x52870
(ROOM.PCX palette via 0x53300 / 0x52820)); input loop 0x4D114 (MousePoll 0x678E8, left bit of 0x229CF4,
MouseInRect 0x67C0C (x, y, x + 26, y + 7), drivers 0x9639A music / 0x96399 sound, Sound FX 0x1163B8 / Subtitles
0x1163BD rule, close lever 0x96192 (244, 31) hit 22 x 65 (half of the frame height 0x96140 = 130), PlaySfxSlot
0x5CB34 (3, 0x2B11, 0x14, 1), 9 frames from (0x9612A = 200 + 22 f, 0x9612C = 0) to the screen 0x95678, timer
0x22A3FC > 89 ms, wait for release 0x67C54, DrawRoomScreen 0x4C304 (0)); ToggleSwitch? 0x4CF10 (PlaySfxSlot(2,
0x2B11, 0x14, 1), xor 1, lever + lamp + name, StopMusic 0x5C82C when the playing theme word 0x9635E > 0, SetMusic
0x5C5EC (word 0x96360, 0) when it is > 0, Sound FX on: PlaySfxSlot(2, ...) again; 0x46BF4); back in 0x4D310: 0x52C98
(screen, R, 318, 200, 320, 0x17C, 0: pixel order from the table 0x28EE30, made by 0x52C10 from main_ 0x102C8; the third
stack argument is not read; screen = 0x95678 = 0xA0000, no wait inside), OfficeHighlight(8, 0), DrawRoomScreen(0),
BlitCopy of hot spot 6's rectangle (0, 18) 38 x 66 from g_panelBuf to g_backBuffer, OFFBRUSH back to the sheet.
validate/diff/switchbox_diff.py.

## The island map picture

The strategic screen shows the island as a grid of 60 sector **cells**; a sector the player owns shows the bright
map, every other sector the dark map, and small markers in each cell count its trees, tappers, guards and mercs
(front-end work; the world gives the sector table).

Pictures (MAIN.DAT members, PCX as the panel sheet, panel.md): `MAPDARK.PCC` and `MAP.PCC`, 320 x 146 each. The
**map buffer** is 640 x 200 (pitch 640): MAPDARK.PCC in columns 0..319, MAP.PCC in columns 320..639, rows 0..145
(rows 146..199 are 0; no cell reaches them: the lowest cell ends at row 141). The palette is
the one of MAP.PCC (6-bit values, as in world_load.md End-of-day screen). The **screen** is 320 x 200 (pitch 320).

**Drawing the map** (sectors = the 60 records of 9 bytes, `sectorState` in defs/types.toml; marks = 60 signed
bytes, one per sector: guards marked on the guard screen, normally 0):
1. The screen is cleared to 0.
2. For each sector s = 0..59 in order: draw its cell, then its markers (so a later cell can cover markers of an
   earlier sector that run too far).

**Cell** of sector s: cx = (s mod 10) × 31 + 6, cy = (s div 10) × 23 + 4. Copy the 30 x 22 block of the map buffer
at (cx, cy) when bit 0 of `flags` is clear, else at (cx + 320, cy), to the screen at (cx, cy) (every pixel, also 0).

**Markers** of sector s: x0 = (s mod 10) × 31 + 9, y0 = (s div 10) × 23. A marker is a panel sheet element (panel.md
"Element": sheet pixel 0 is not written; here w = h = 3) drawn at screen offset y × 320 + x (no clipping at the
right edge: a long row goes on in the next screen row; with byte counts the last marker pixel stays below screen offset
(115 + 20 + 2) × 320 + 288 + 3 × 255 + 2 = 44,895, and in the guards row below (115 + 16 + 2) × 320 + 288 + 3 × 637
+ 2 = 44,761, so no marker reaches the end of the 64000-byte screen). Marker i of a row is at x = x0 + 3 × i. Counts are unsigned bytes unless stated.
- Trees, y = y0 + 8: for i = 0 .. `trees` − 1: h = `trees` − `unhealthy` (signed, may be below 0); element 30 when
  i < `workable` and i < h, 113 when i < `workable` only, 29 when i < h only, else 112.
- Tappers, y = y0 + 12: (`tappers` div 3) markers of element 31.
- Guards, y = y0 + 16: n = `guards` + `b5` + the sector's mark (signed); for i = 0 .. n − 1: element 32 when
  i < `guards`, 118 when i < `guards` + `b5`, else 119.
- Mercs, y = y0 + 20: `mercs` markers of element 33.

Elements (sheet x, y; all 3 x 3): 29 (251, 74), 30 (254, 74), 31 (257, 74), 32 (260, 74), 33 (263, 74),
112 (245, 74), 113 (248, 74), 118 (236, 74), 119 (239, 74).

Front-end test entry point (port/ja1front; `iso`, `exe` as in panel.md "Front-end interface"):

```c
/* Drawing the map: sectors = 60 x 9 bytes, marks = 60 signed bytes, screen = 320 x 200 (out), palette = 768
   bytes (out, 6-bit values). Return 0 = done, < 0 = an error (bad argument, missing or short file). */
int32_t ja1f_island_map(const char *iso, const char *exe, const uint8_t *sectors, const int8_t *marks,
                        uint8_t *screen, uint8_t *palette);
```

Evidence (2026-10-23): InitStrategicMap 0x43ECC (memset, DrawStrategicMap 0x41FA0 loads MAPDARK.PCC and MAP.PCC
into the 640-wide buffer *0x93E5C, then per sector 0x41640 (cell) and 0x419BC (markers; element draw 0x46920 =
BlitKey from g_panelSheet with the rectangle table 0x93732: x, y, w − 1, h − 1 as i16); marks = 0x229D8C, used by
GuardAssignScreen?). The map palette: 0x53300 MAP.PCC into 0xFA608.

### Water on the island map

While the island map waits for input, its water moves: on each round of the screen's input loop, when more than
500 ms passed on the game clock since the last rotation (or since the screen opened), the palette colours
0xD7..0xDA and 0xDB..0xDE each rotate by one (entries first + 1 .. last move down by one, the first entry goes to
the last place; as on the guard screen, "The guard screen" step 4) and the changed colours are written to the
display. No Control Panel switch stops it (Pal Cycling does not). Nothing rotates while the screen waits inside a
box (message box, question box), the finance page, the processing plants (tab 3 held), a held number field box,
the wage box, a map press until its release (WaitLeftUp + WaitRightUp; the sector frame does not follow the mouse
and no second press counts), or the assignment screen (its own loop has no rotation); the next loop round after
such a wait rotates at once when more than 500 ms passed. After a flip animation (finance page, assignment screen)
and after the wage question's picture row a full 500 ms wait starts, because these use the same timer 0x22A3FC. A
redraw of the map keeps the rotated colours.

Evidence (2026-10-29, asm StrategicScreen 0x432F4 at 0x43E07..0x43E59: TimerElapsedMs(0x22A3FC) > 500 →
PaletteRotate? 0x528AC(0xEDA08, 0xD7, 0xDA), (0xEDA08, 0xDB, 0xDE), PaletteSet? 0x52870(0xEDA08, 0xD7, 0xDE),
TimerStart(0x22A3FC); the same at GuardAssignScreen? 0x42F4F; the timer is also restarted by the flip animation and
the panel waits). Live (DOSBox-X, state estore_asg): on the assignment screen 0xEDA08 + 4 × 0xD7 stays the same;
after OK (the island map) the 8 entries rotate about every 0.5 s, the entry at 0xD7 moving to 0xDA. Live 2026-11-22 (timer
0x22A3FC and the palette read every 2 ms, build/probe/w5b): a status box after a map press stops the water and its
closing press rotates at once; the finance flip restarts the timer on each picture (about 110 ms apart), the page
stops the water, and the first rotation comes about 550 ms after the closing flip; the wage box stops the water and
its closing press rotates at once; the wage question's picture row restarts the timer every 220 ms, so the first
rotation comes about 530 ms after the answer, and a flip at once after the answer gives a full wait after the flip.

## Sector click on the island map

The player clicks a sector cell of the island map with the left or the right mouse button. What the click does
depends on the **map mode** (the raised tab of the strategic panel): 0 = place the team, 1 = tappers, 2 = guards;
any other mode does nothing. Buttons: bit 0 = left, bit 1 = right; when both are set, "left" holds. A click with
neither bit does nothing (answer 0). World fields: `sector_state` (records 0..59), `cur_sector` (0xFF = no
starting sector yet), `team_on_duty`, `accounts` (Day accounts offsets: g_idlePool 0x4C, g_tappers 0x40,
g_processCap 0x84). Sector bytes are `sectorState` fields; adds and subtracts on them are 8-bit with wrap-around.

The answer is a **message number** (0 = none); the front-end shows its text on the status line:

| n | text |
|---|------|
| 1 | This sector is in enemy hands! |
| 2 | This sector is not linked! |
| 3 | No one on duty! |
| 4 | No Mercenaries there! |
| 5 | Starting sector already specified! |
| 6 | No Tappers there! |
| 7 | Not enough workers available! |
| 8 | There are no trees there! |
| 9 | Tappers are too scared to work there! |
| 10 | No more tappers will work there! |
| 11 | No Guards there! |
| 12 | No workers available! |
| 13 | Guard capacity reached! |
| 14 | Starting sector not specified! |
| 15 | Costs are higher than you can afford! |
| 16 | You may want to visit the ASSIGNMENT screen where you can outfit your mercs. |
| 17 | To do this, click on the RAISED team cost button in the lower left area of the screen. |

Click on sector s (0..59), in this order:
1. `flags` bit 0 of s is clear: message 1, nothing else.
2. s is not connected (Connected to home, full table): message 2, nothing else.
3. Mode 0 (place the team):
   - Left and `team_on_duty` ≠ 0: when `cur_sector` = 0xFF: `mercs` of s + the low byte of `team_on_duty`,
     `cur_sector` = s; else message 5.
   - Else (right, or nobody on duty): when `cur_sector` = s: `mercs` of s = 0, `cur_sector` = 0xFF; else message 4
     when the right bit is set, else message 3.
4. Mode 1 (tappers):
   - Left: when `tappers` div 3 < `workable` (unsigned): when g_idlePool ≥ 3: `tappers` + 3, else message 7.
     Else when `tappers` ≠ 0: message 10; else when `trees` = 0: message 8; else message 9.
   - Right: when `tappers` ≥ 3: `tappers` − 3, else message 6.
   - Then always: All accounts (Day accounts). Then the **process warning**: q = g_tappers div 3 (signed). When
     `warned` ≠ 0: `warned` = 0 when q ≤ g_processCap. Else when q > g_processCap: host op 302 guide_say (quote
     0x2F, category 6), then `warned` = 1.
5. Mode 2 (guards):
   - Left: when `guards` < 8: when g_idlePool ≥ 1: `guards` + 1, else message 12. Else message 13.
   - Right: when `guards` ≠ 0: `guards` − 1, else message 11.
   - Then always: All accounts. (No process warning.)

After the click the front-end redraws the cell and markers of s (The island map picture), in every mode and also
after a message; in modes 1 and 2 it also redraws the accounts panel.

**Start the day** (the start key of the map, or the start button): message 14 when `cur_sector` = 0xFF and
`team_on_duty` ≠ 0; else message 15 when g_balanceAfterDay (accounts 0x28) < 0 (signed); else, from the button only:
when `hinted` = 0, the front-end shows messages 16 and 17, `hinted` = 1 and the day does not start yet (answer 16);
else (and always from the key) answer 0 = the day may start. `hinted` is a game flag (0 at a new game; in the day
save, world_load.md section 3). The start button is button 7 of the button table (281..306 x 176..191): a left
press on it counts when the button goes up (wherever the mouse is then).

```c
/* strategic.md "Sector click on the island map". warned: the screen's process warning flag (in/out, 0 when the
   screen opens). Returns the message number. */
int32_t ja1_map_click(Ja1World *w, const Ja1Host *h, int32_t sector, int32_t mode, int32_t buttons, uint8_t *warned);
int32_t ja1_map_start(const Ja1World *w, int32_t button, uint8_t *hinted);   /* 0, 14, 15 or 16 */
```

Evidence (2026-10-23): StrategicScreen 0x432F4, click block 0x43987..0x43C32 (mode byte 0x93DA0, warning flag =
local [esp+0x1C], StatusMessage? 0x27D24, guide quote 0x5F5AC(0x2F, 6)); start checks after MapKeyHandler? 0x426D0
and the start button (0x67CD8(7, 1); hint flag 0x22A1F4).

## Mouse over the island map

While the mouse is in the map part of the strategic screen (mouse y < 141), the sector under the mouse at (x, y) is
s = ((y − 3) div 23) × 10 + ((x − 3) div 31), with div rounding toward zero and the sum taken as a signed byte;
when s > 59, s = 59. (Quirk of the original, keep it: x = 313..319 gives column 10, so the right edge of the east
column selects the first sector of the next row; the right edge of the last row selects sector 59.)

When s changes: the old sector (when it is 0..59) gets a frame of colour 0x6E, then s gets a frame of colour 0xFF
when `flags` bit 0 of s is set (owned), else 0x6B. The frame of sector s is the panel.md **Frame** drawn on the
screen (pitch 320) with x0 = (s mod 10) × 31 + 5, y0 = (s div 10) × 23 + 3, x1 = x0 + 31, y1 = y0 + 23. Each frame
also plays the faint release sound (sound.md "Island map sounds"). When the mouse leaves the map part (y ≥ 141), the frame of the last sector
gets colour 0x6E and no sector is current. A left or right click while a sector is current is the Sector click on
the island map (above) with the current map mode; then the front-end redraws the cell and markers of s.

Evidence (2026-10-23): StrategicScreen 0x432F4 loop (mouse y 0x229D04 < 0x8D), sector under the mouse 0x41410
(it also stores s + 1 at 0x22A010 and draws that number on the panel: not specified yet), frame 0x4147C
(DrawFrame 0x46C44, sound 0x5622).

## The map panel

Below the island map (screen rows 146..199) the strategic screen shows the **map panel**: four tabs, the team,
worker, tree and money numbers, the native wage box and the number of the sector under the mouse. The world gives
the numbers (`accounts`, Day accounts; `team_on_duty`); the **map mode** (0..2, the raised tab, also used by Sector
click on the island map) is front-end state and starts at 0. Drawing uses the panel sheet (panel.md "Element": sheet
pixel 0 is not written) and panel.md "Text" / "Number" on the screen (pitch 320) with font 0.

**Tabs** (x0, y0, x1, y1; the mouse is in a tab when x0 ≤ x ≤ x1 and y0 ≤ y ≤ y1):

| tab | rectangle | raised element (x, y, w x h in the sheet) | use |
|---|---|---|---|
| 0 | (22, 155, 57, 164) | 34 (42, 114, 36 x 10) | map mode 0: place the team |
| 1 | (76, 168, 111, 177) | 35 (42, 104, 36 x 10) | map mode 1: tappers |
| 2 | (76, 155, 111, 164) | 36 (42, 94, 36 x 10) | map mode 2: guards |
| 3 | (190, 155, 225, 164) | 37 (162, 140, 36 x 10) | the processing plants (shown while the button is held) |

**Raise** tab m: keep a copy of the 36 x 10 screen pixels at the tab's (x0, y0), then draw element 34 + m at
(x0, y0). **Lower** tab m: put that copy back.

**Number fields** k (value = an i32 of `accounts` at the offset given, else as stated; d = digits; fg, shadow,
background b as in panel.md "Text with a background"; all with b = 13):

| k | value | at (x, y) | d | fg | shadow | box at |
|---|---|---|---|---|---|---|
| 0 | g_teamPay +0x38 | (30, 182) | 6 | 121 | 12 | (22, 180) |
| 1 | g_nativeWage +0x54 | (146, 164) | 6 | 121 | 12 | (138, 162) |
| 2 | g_nativesCost +0x60 | (146, 182) | 6 | 121 | 12 | (138, 180) |
| 3 | g_dayIncome +0x20 | (241, 164) | 6 | 50 | 12 | (233, 162) |
| 4 | g_balanceAfterDay +0x28 | (241, 182) | 6 | money colour | 12 | (233, 180) |
| 5 | g_workers +0x48 | (78, 182) | 3 | 39 | 9 | |
| 6 | g_workerPool +0x3C | (97, 182) | 3 | 39 | 9 | |
| 7 | g_treesWorkable +0x14 | (192, 182) | 3 | 39 | 9 | |
| 8 | g_treesOwned +0x18 | (211, 182) | 3 | 39 | 9 | |
| 9 | `team_on_duty` | (32, 169) | 1 | 39 | 9 | |
| 10 | g_teamCount +0x08 | (43, 169) | 1 | 39 | 9 | |
| 11 | g_tappers +0x40 | (120, 170) | 3 | 39 | 9 | |
| 12 | g_guards +0x44 | (120, 157) | 3 | 39 | 9 | |
| 13 | g_treesTapped +0x1C | (192, 169) | 3 | 39 | 9 | |
| 14 | g_processCap +0x84 | (211, 169) | 3 | 39 | 9 | |
| 15 | the mouse sector + 1, i32 at +0x70 | (290, 165) | 2 | 29 | 50 | |

**Money colour**: 0x79 when the value is below 0 or g_balanceAfterDay < g_balance (+0x24, signed); else 0x32.

**Drawing the panel** (Strategic screen opening step 4e, after the island map; the screen row 199 stays 0):
1. MAIN.DAT `MAPPAN.PCC` (320 x 53) at screen (0, 146), every pixel.
2. Raise the tab of the map mode.
3. Number fields 0..14 in order.

**Mouse over the map**: in every pass of the strategic screen with the mouse in the map part (Mouse over the island
map: y < 141), +0x70 = s + 1 for the sector s under the mouse, then number field 15. When the mouse leaves the map
part, the number stays. +0x70 is a game field: it goes into the day save with the rest of `accounts`.

**Tab click** (left button; tabs tested 0..3 in order, the first tab with the mouse in it counts): tab 3: the click
sound, then The processing plants (below). Tab m ≤ 2 other than the map mode: the click sound, lower the
tab of the map mode, map mode = m, raise tab m. The tab of the map mode itself: nothing.

**Native wage box** (rectangle (138, 162, 172, 170)): a left click in it:
1. Element 43 (42, 134, 36 x 10; the pressed box) at (138, 162); the native wage (+0x54) as number field 1 but at
   (145, 164) with fg 114, shadow 89, background 56; the click sound. Keep w0 = the native wage.
2. Wait for the left button to come up. Then, until the next left click: each right click (the button going down)
   plays the click sound and, when the wage < 100: wage + 1, Worker pool (worker_pool), All accounts, the pressed
   wage number again, number fields 2..14; else the status line shows "Natives' salary is at its maximum!".
3. At the left click: the faint release sound (sound.md "Island map sounds"), element 43 and the pressed wage number again. When the wage ≠ w0: the
   question "Raise natives' salary? ± (don't expect to lower it later)" (yes / no; panel.md "Question box", with
   an animation: ConfirmDialog? 0x275E4); at "no": wage = w0, Worker pool,
   All accounts, number fields 0..14.
4. Element 39 (42, 84, 36 x 10; the box) at (138, 162), then number field 1.

Front-end test entry points (port/ja1front; `iso`, `exe` as in panel.md "Front-end interface"; accounts = 0xA8
bytes as in port_api.md; screen = 320 x 200, only the parts named are written):

```c
/* Drawing the panel (steps 1..3) with map mode `mode` (0..2). Return 0 = done, < 0 = an error. */
int32_t ja1f_map_panel(const char *iso, const char *exe, const uint8_t *accounts, int32_t team_on_duty,
                       int32_t mode, uint8_t *screen);
/* Number field k (0..15) alone; k = 16 = the pressed wage number (step 1); k = 17 = step 4 of the wage box. */
int32_t ja1f_map_field(const char *iso, const char *exe, const uint8_t *accounts, int32_t team_on_duty,
                       int32_t k, uint8_t *screen);
```

Evidence (2026-10-23): DayEndMercUpdate? 0x43F54 (MAPPAN.PCC load to screen + 0xB680, 0x42020 = save area 10
(0x936BA, back buffer pitch 380) + element 0x22 + mode at the tab table 0x93EDA (x0, y0, x1, y1 as i16, 8 bytes per
tab), 0x36010 = fields 0..14); number field table 0x93FD8 (19 bytes: x, y, box x, box y as i16, digits u8, fg,
shadow, mode as i16, value pointer), drawn by 0x35F64 (fg < 0 = money colour, the g_balanceAfterDay test for
fields 4 and 30) and DrawNumber 0x36090 / DrawText 0x358D0 (mode ≠ 300: table 0 byte 0 = mode, BlitRemap); mouse
sector 0x41410 (field 15); StrategicScreen 0x432F4: tab loop 0x43421..0x434DE (MouseInRect 0x67C0C, restore 0x468B0,
mode byte 0x93DA0), wage box 0x43512..0x43728 (0x35E84 = element 0x27 + field), tests validate/diff/mappanel_diff.py.
The finance page (field 4 box) and the processing tab: next sections.

### Number field boxes

Number fields 0..4 sit in a **box** (the "box at" column). A left click with the mouse in the box (box x ≤ x ≤ box
x + 34 and box y ≤ y ≤ box y + 8) **presses** it: the click sound; element 38 (42, 124, 36 x 10; the pressed box) at
the box; the field drawn as usual but without the background (mode 300, panel.md Text); wait until the left button is
up; element 39 at the box; the field again (with its background 13). Then:
- field 0 box (team pay): the inventory screen (inventory.md) opens; after it the island map is drawn again (front-end
  of the port: not done, the click only presses the box);
- field 4 box (balance after the day): The finance page (below).
The strategic screen tests, in one pass with the left button down: the tabs, the field 0 box, the native wage box,
the field 4 box, in this order.

### The finance page

Number fields 16..25 (all without background, mode 300; money colour as above but without the g_balanceAfterDay
test: 0x79 only when the value is below 0):

| k | value (i32 of `accounts`) | at (x, y) | d | fg | shadow |
|---|---|---|---|---|---|
| 16 | g_treePrice +0x10 | (77, 173) | 6 | 50 | 0 |
| 17 | g_dayIncome +0x20 | (77, 184) | 6 | 50 | 0 |
| 18 | g_teamPay +0x38 | (177, 153) | 6 | 121 | 12 |
| 19 | g_tappersCost +0x58 | (177, 164) | 6 | 121 | 12 |
| 20 | g_guardsCost +0x5C | (177, 175) | 6 | 121 | 12 |
| 21 | g_dayExpenses +0x64 | (177, 186) | 6 | 121 | 12 |
| 22 | +0x04 (the day number u16 and the u16 after it) | (280, 153) | 6 | 50 | 0 |
| 23 | g_balance +0x24 | (280, 164) | 6 | 50 | 0 |
| 24 | g_balanceAfterDay +0x28 | (280, 175) | 6 | money colour | 0 |
| 25 | g_dayNet +0x2C | (280, 186) | 6 | money colour | 0 |

The **panel rows** are screen rows 146..199 (320 x 54). All pictures below are MAIN.DAT members of 320 x 54 and fill
the panel rows; a **picture step** = show the picture in the panel rows, then wait until 60 ms passed on the game clock since the
previous picture step (the first step of a run counts from its own start). The mouse stays in the panel rows while
the page is open.

**Open** (after the field 4 box press):
1. Picture steps `MAPFLIP1.PCC`, `MAPFLIP2.PCC`, `MAPFLIP3.PCC` (the page sound 0x2B11 with the first picture, after 60 ms of the old screen).
2. Picture step: `FINANCE.PCC` with number fields 16..25 on it.

**Back button** (on the page): x = 74, y = 153; the mouse is in it when 74 ≤ x ≤ 99 and 153 ≤ y ≤ 168. Up:
element 20 (260, 49, 26 x 16) at (74, 153), then element 58 (245, 65, 16 x 9) at (79, 157). Pressed: element 21
(286, 49, 26 x 16) at (74, 153), then element 58 at (78, 156). A left click in it: the click sound, pressed, wait
until the left button is up, up. Then **Close**:
1. Picture steps `MAPFLIP3.PCC`, `MAPFLIP2.PCC`, `MAPFLIP1.PCC` (the sound with the first picture).
2. Picture step: `MAPPAN.PCC` (320 x 53, rows 146..198; row 199 keeps the last row of MAPFLIP1.PCC), number fields
   0..14, the tab of the map mode raised. Number field 15 is not drawn again (it comes back with the next mouse
   move over the map).
Nothing else on the screen reacts while the page is open (no keys, no other clicks).

### The processing plants

Tab 3 shows, while the left button stays down, the processing plants on the island map:
1. Keep a copy of the screen rows 0..142. Raise tab 3 (element 37, as Raise above).
2. remaining = the low byte of g_treesTapped (u8). For each of the 4 `plants` entries in order, with s = its
   `sector` (i32), col = s mod 10, row = s div 10:
   1. Draw the cell of s again (The island map picture "Cell": the markers of s are covered).
   2. Element 114 (238, 131, 14 x 7) when `state` = 2, else element 115 (238, 138, 14 x 7) at (col × 31 + 7,
      row × 23 + 5).
   3. used = 0 when the sector of the low byte of `sector` is not connected (Connected to home) or `state` ≠ 2;
      else used = `capacity` when `capacity` ≤ remaining (signed compare), else remaining; then remaining =
      remaining − used (8-bit wrap).
   4. With y = row × 23 + 17, font 1, fg 0xFF, shadow 1 (panel.md Text, mode 300): Number used, 2 digits, at
      (col × 31 + 5, y); the text "/" at (col × 31 + 17, y); Number `capacity`, 2 digits, at (col × 31 + 23, y).
3. The text "Total processing plant usage is  %d/%d" (two spaces before the numbers; g_treesTapped, g_processCap as signed decimals) centred at
   (159, 18), font 2, fg 0xFF, shadow 1.
4. Wait until the left button is up. The sound 0x2B11 (slot 1).
5. Lower tab 3; put the copy of rows 0..142 back.
SPEC GAP (not seen: the plant sectors are 59, 26, 53 and 30 from the new game (EXE data, world_load.md) and no step writes them): a `sector` outside 0..59 draws outside its cell in the original; the port draws nothing for that plant
(used is still computed as above).

Front-end test entry points (as above; `plants` = 4 x 12 bytes, `sectors` = 60 x 9 bytes):

```c
/* k = 18: the finance page (FINANCE.PCC at (0, 146) + fields 16..25); 19 / 20: the back button up / pressed;
   21 / 22: the field k box pressed / up for k = 4 (with 23 / 24 the same for k = 0); 25..27: MAPFLIP1..3.PCC at
   (0, 146). Through ja1f_map_field. */
/* The processing plants, steps 1..3 on `screen` (in/out; the copy and step 5 are the caller's). */
int32_t ja1f_processing(const char *iso, const char *exe, const uint8_t *accounts, const uint8_t *plants,
                        const uint8_t *sectors, uint8_t *screen);
```

Evidence (2026-10-23): StrategicScreen 0x432F4: field box test 0x67CD8(k, 0) (number field table box x, y, 34 x 8
inclusive; press 0x35ED4 = 0x35F0C element 0x26 + field with mode 300, wait 0x67C54, 0x35E84 element 0x27 + field);
0x67CD8(0, 0) -> AssignScreen? (0x434E8), 0x67CD8(4, 0) -> finance (0x43734): PlayFlipAnim? 0x51264 ("mapflip",
first 1, count 3, step 1; then 3, 3, -1: each frame loads `%s%d.pcc` into the panel buffer + 0xB680, waits 60 ms on
timer 0x22A3FC, 0x41608 copies 320 x 54 to the screen; sound PlaySfxSlot(4, 0x2B11) at frame index 1), 0x36010
(finance.pcc, panel buffer, 320, 16, 25), wait 60 ms, BlitCopy 320 x 54; button 8 of the button table 0x9345A (x, y
i16, up element, pressed element, kind 1, overlay element, dx, dy; the hit size = the pressed element's w − 1, h − 1;
press 0x35DA4 overlay at dx − 1, dy − 1, up 0x35C0C kind 1) via 0x67CD8(8, 1); back: 0x36010(mappan.pcc, panel
buffer, 0, 14) + 0x42090 (tab into the panel buffer). DrawProcessingUsage? 0x42148 (0x2CB0 dwords = 320 x 143 to
g_panelBuf and back, save area 14 + element 0x25 at tab 3, cell 0x41640, elements 0x72 + (state ≠ 2), DrawNumber /
DrawText font 1, sprintf "%s %d/%d" font 2 centred; the remaining count is a byte local). Tests:
validate/diff/mappanel_diff.py (finance, box, button, flip and processing cases).

## Day start

After the start checks give 0 (Sector click on the island map, **Start the day**), the game asks the player and
then starts the day. World fields: `team_on_duty`, `cur_sector`, `accounts` (offset 0x9F = **sorties**, u8: the
AIM statistic "Mercenary Sorties"), `soldiers` (records 0..7), `speaker`, `sector_state` (records 0..59) and
`sector_replay` (60 x 9 = 540 bytes: the copy of the sector table at the start of the day).

1. t = `team_on_duty` (signed 32-bit). When t = 0: `cur_sector` = 59 (this stays also when the player says no).
2. The question, host op 303 day_ask(kind, wait_row, go_row, number); the host loads the travel animations
   (`dat\sanim2.dat`) first in both kinds:
   - t = 0: kind 0 = the yes/no box "Start day without anyone assigned to duty?"; wait_row = go_row = number = 0.
   - Else: kind 1 = the "Get to work?" box over the travel animation. (wait_row, go_row) by t: 1 → (6, 7),
     2 → (12, 13), 3 → (14, 15), any other value (also negative) → (16, 17). number = `cur_sector` + 1 (unsigned
     byte plus 1, so 1..256). The host writes number as `%2d` and keeps only its first 2 characters (256 gives
     "25") into characters 22 and 23 of the text line of the go animation ("Destination: ± Sector NN").
   The answer of the host is "yes" when it is not 0.
3. When the answer is "no": nothing more; the strategic screen goes on (answer 0 of `ja1_day_start`).
4. When "yes":
   - sorties = sorties + (the low byte of t), 8-bit with wrap-around.
   - The **guide merc**: the first record k of 0..7 with `inCurSector` ≠ 0, `hp` ≠ 0 and `profileId` 60..64;
     none = 0x40. `speaker` = k.
   - When k < 0x40 and its `duty` = 0 (on duty): sorties = sorties − 1 (8-bit).
   - `sector_replay` = a copy of the 540 bytes of the sector table (records 0..59, 9 bytes each).
   - Answer 1: the day starts (the front-end leaves the strategic screen; the music and the fade-out are front-end).

```c
/* strategic.md Day start. World: team_on_duty, cur_sector, accounts, soldiers, speaker, sector_state,
   sector_replay. Host op 303. Return = 1 when the day starts, 0 when the player says no. */
int32_t ja1_day_start(Ja1World *w, const Ja1Host *h);
```

Evidence (2026-10-23): StrategicScreen 0x432F4, block 0x43C3D..0x43E07 (sanim load 0x5E8E0, jump table 0x432E4,
sprintf "%2d" into 0x9694D / 0x96A3D / 0x96A8D / 0x96ADD = sanim text 0x9681F + 40 x row id, ConfirmDialog?
0x275E4 with rows of table 0x9443A, yes/no 0x278A0 text 0x98007; sorties 0x22A03F; guide merc 0x62A00 into
0x92DD5) and 0x43E5E..0x43E97 (copy of g_sectorTbl9 0x22979C to 0x2254F0).

## Sunrise

When the player goes to sleep in the office, the next day begins with the sunrise screen, before the island map
(Day start). Entry point `sunrise`. World fields: `day`, `accounts`, `soldiers` (records 0..7), `stat_changed`.

1. `day` = `day` + 1 (signed 32-bit); the 4 bytes at `accounts` +0x04 get the same new value (they are the same
   game value; the morning sections read it there).
2. `stat_changed` = all 0 (640 bytes).
3. host op 304 sunrise_show (the sunrise picture with "Day N", "Current Funds" and the faces of the new arrivals:
   only drawing and sound; the arrivals count is the i16 at `accounts` +0x8C).
4. The 2 bytes at `accounts` +0x8C (the new arrivals count) = 0.
5. **Overnight healing**, for each record k = 0..7 with `inCurSector` ≠ 0: m = `hpMax` and h = `hp`, both as
   signed bytes; d = m − h. When d ≠ 0: q = m div 15 (rounding toward zero); r = random(q) when q > 0, else 0
   (random(n) = the game's random number 0..n−1, one draw); when r > d (signed): r = d. Then `hp` = `hp` + r
   (8-bit). (So a merc with more health than maximum, h > m, drops to m: d is negative and r becomes d.)

```c
/* strategic.md Sunrise. World: day, accounts, soldiers, stat_changed. Host op 304. */
void ja1_sunrise(Ja1World *w, const Ja1Host *h);
```

Evidence (2026-10-23): PlayLoop 0x4FB94 (g_day + 1 at 0x4FBF8, then SunriseScreen? 0x2A268); SunriseScreen?
memset of g_statChanged 0x10DD10 (640), the i16 at 0x22A02C cleared at its end, then 0x4B3F0 (the healing loop;
Random 0x53538). The screen's own bird calls draw more random numbers during op 304 (see "The sunrise screen"
below); the front-end draws them from the library's random sequence, so the healing gets the original's numbers.

### The sunrise screen (host op 304)

What the original shows for host op 304 (front-end only: no world data changes, but the bird sounds draw game
random numbers, so the front-end draws them from the same random sequence as the library). Inputs: `day` (already
+ 1), the balance (`accounts` +0x24, signed 32-bit) and the new arrivals count A (the i16 at `accounts` +0x8C, not
yet cleared). Pictures are members of `DAT/MAIN.DAT`: `SUNRISE.PCX` (320 x 200) and `HELIN6.PCX` (1280 x 40: 32
helicopter frames of 40 x 40 side by side, frame i at x = 40 i). Palettes are the PCX palettes (last 769 bytes:
0x0C and 256 x RGB, each value shifted right by 2), 6-bit, as the sunset screen (world_load.md section 8).

1. The main sound bank loads; the music stops. P = the SUNRISE.PCX pixels (a 320 x 200 buffer), W = its palette.
   S (the screen) = P. The effect 0x12 (sound.md main bank) starts at rate 11025, volume 50, loops 0 (until
   stopped): the **morning sound**. When A > 0 (signed), the effect 0x10 starts at rate 10000, volume 50, loops 0:
   the **helicopter sound**.
2. **Fade in**: the screen shows S while the palette goes from black to W in 57 levels l = 0..56: every entry e
   shows (R · l / 56, G · l / 56, B · l / 56) (truncated), one level per vertical retrace (70 per second).
3. **Three texts** (each built once): T1 = "Day " + `day` in decimal; T2 = "Current Funds : $ " + the balance in
   decimal (minus sign when negative); T3 = A in decimal + " New Arrival" when A = 1, A + " New Arrivals" when
   A > 1, the empty text when A ≤ 0. A text is drawn with panel.md "Text" (centred, x 159, fg 0xFF, shadow 1,
   font 2 = JA_BIG.FNT, table 0 of the tactical screen) at y 64 (T1), 79 (T2), 94 (T3).
4. **The colour cycle**, frames f = 0..89: first a clock wait of 1 ms (world_load.md "Game speed", The game
   clock; the first frame does not wait: its timer started before the fade in), so one frame per clock step
   (~55 ms, 90 frames ~4.9 s). Then the entries 3..254 of W rotate by one toward the lower index (as op
   206 of the sunset screen), the entries 3..254 are shown from W (one vertical retrace, after the clock wait). At f
   = 10, 35 and 75 the text T1, T2 resp. T3 is drawn on S. Then the input is polled: a key press or a mouse button
   press **skips**: T1, T2 and T3 are drawn on S, W = the palette of HELIN6.PCX (which equals the sunrise palette
   after its 90 rotations), all 256 entries are shown, and the cycle ends at once (no bird call in that frame).
   Else, when f mod 4 = 0: r = random(5); when r = 0, a **bird call**: loops = random(2) + 1, volume = random(15) +
   10, effect = random(4) + 0x13 (four draws in this order: 5, 2, 15, 4); it plays at rate 11025.
5. **The helicopter** (only when A > 0), frames i = 0..31, with the x table X = 0, 0, 0, 0, 10, 20, 30, 40, 50,
   60, 70, 80, 90, 95, 100, 105, 109, 112, 115, 117, 119, 120, 121, 123, 124, 125, 126, 127, 128, 129, 129, 129 and
   a saved 40 x 40 block B:
   - when i > 0: the block B goes back into P at (X[i − 1], 89);
   - B = the 40 x 40 block of P at (X[i], 89); then HELIN6 frame i is copied into P at (X[i], 89) (every pixel,
     colour 0 too);
   - T1, T2 and T3 are drawn on P (same places); rows 29..131 of P (all 320 columns) are copied to S and shown;
   - the screen waits on the game clock (world_load.md "Game speed"; counted from the end of the previous frame) 105 ms when i < 15, 110 ms when i < 20, 115 ms
     when i < 25, 120 ms when i < 29, else 125 ms; a key or a mouse button press during the wait ends the wait
     (skip);
   - when i mod 4 = 0: r = random(5) and, when r = 0, a bird call as in step 4 (also in the frame of a skip);
   - a skip ends the helicopter here.
6. The morning sound and the helicopter sound stop. **Fade out**: from W to black in 25 levels l = 24, 23, .. 0
   (entry e shows (R · l / 24, ...)), one level per vertical retrace. The screen ends (the bird calls play out).

Evidence (2026-10-26): SunriseScreen? 0x2A268: TimerStart 0x47020, LoadMainSound 0x31B8C, StopMusic 0x5C82C,
0x532BC loads `helin6.pcx` (into g_panelSheet, pitch 0x500, only when g_newArrivals 0x22A02C > 0) and
`xsunrise.pcx` + 1 (into g_panelBuf, pitch 0x140), palette 0x53300 + 0x52820 (RGB into RGBX 0xEDA08), 0x46BF4 panel
buffer to screen, PlaySfxSlot 0x5CB34 (0x12, 0x2B11, 0x32, 0) and (0x10, 10000, 0x32, 0), fade 0x52908 (EAX
0xEDA08, EDX 0, EBX 0x38, ECX 0, stack 0x100) in / (EBX 0x18, ECX 0x100, stack 0) out; frame loop 0x2A4FC..0x2A6EE
(TimerElapsedMs 0x47040 ≥ 1, PaletteRotate? 0x528AC (3, 0xFE), PaletteSet? 0x52870 (3, 0xFE), DrawText 0x358D0 (x
0x9F, y 0x40 / 0x4F / 0x5E, centred, 0xFF, shadow 1, font 2, screen pointer 0x95678), poll 0x6782C + 0x67E44, skip
0x2A5F0..0x2A695 (0x53300 `helin6.pcx`, PaletteSet (0, 0xFF)), Random 0x53538 at 0x2A6A9..0x2A6DD); helicopter
loop 0x2A70B..0x2A932 (table 0x95F70, 32 bytes; BlitCopy 0x52AE0 with g_backBuffer pitch 0x17C as B, sheet
pitch 0x500; copy 0x2030 dwords from g_panelBuf + 0x2440 to the screen; waits 0x69 / 0x6E / 0x73 / 0x78 / 0x7D);
LoadPanels 0x46EC4 after it; 0x5CE60 (AIL_end_sample) on both handles; strings 0x99AD3 "Day ", 0x99AED "Current
Funds : $ ", 0x99B07 " New Arrival", 0x99B21 " New Arrivals", empty 0x90684. The HELIN6 palette equals the
SUNRISE palette with the entries 3..254 rotated 90 times (checked on the files).

## Strategic screen opening

When the island map opens at the start of a day (after Sunrise, before the player places the team; Day start
comes at its end), the game runs these steps in this order (entry point `strategic_open`). World fields: those of
the steps (`sector_state`, `soldiers` records 0..7, `profiles`, `accounts` and the fields of All accounts, Collect
the cash and Worker pool).

1. **Story events** (`story_day`, Story events at the day start, below).
2. **Tapper limit**: for each sector s = 0..59 with `flags` bit 0 set and connected to home (Connected to home):
   Workable trees of s (which writes `workable`); a = `trees` − `unhealthy` (8-bit); when a ≥ `workable`
   (unsigned): a = `workable`. When `tappers` > 3 × a: `tappers` = 3 × a (low byte); else `tappers` = `tappers` −
   (`tappers` mod 3).
3. **Canteens refilled**: for each soldier k = 0..7 with `inCurSector` ≠ 0, for each item place i = 0..11 (12 bytes
   each from record offset 0) whose byte 0 (type) = 0x41 (canteen): byte 2 = 100; c = byte 1 as a signed byte;
   when c > 1: bytes 8 .. c + 6 of that place (counted from the place's byte 0) = 100 each. (Quirk: with c > 5
   this runs past the 12 bytes of the place into the next places; keep it.)
4. **Overnight departures**:
   a. For each profile p = 0..59: when `teamStatus` (+0x4B) as a signed byte is 1..49: `teamStatus` − 1.
   b. gone[0..7] = 0x40, picked[0..7] = clear. Repeat 8 times: draw k = random(8) again and again until picked[k]
      is clear; set picked[k]. When soldier k has `inCurSector` ≠ 0: p = its profile (`profileId`), d = p byte
      +0x3A (the merc it dislikes), w = p byte +0x3C, leave = no, n(x) = the number of soldiers 0..7 with
      `inCurSector` ≠ 0 and `profileId` = x (as in A disliked merc in the team):
      - d = 99: when w ≠ 99 and n(w) ≠ 0: p byte +0x3A = w.
      - else when d = w and n(d) ≠ 0 (the morning ultimatum was ignored; personality.md Hate and the night): when p
        byte +0x2F = 3 (relentless, guide OverEnth: the merc kills the hated merc): for each soldier j = 0..7 in order with
        `inCurSector` ≠ 0 and `profileId` = d: gone[k] = d, `hpMax` of j = 0, Merc leaves (j, status 0), `inCurSector`
        of j = 0, the i32 at `accounts` +0x00 (mercs killed) + 1, `profileId` of j = 99. Else leave = yes.
      - else nothing.
      When leave: gone[k] = `profileId` of k, Merc leaves (k, status 0), `inCurSector` of k = 0, the byte at
      `accounts` +0xA0 (resignations) + 1 (8-bit), `profileId` of k = 99.
   c. Close up the team (redraw 0), 8 times.
   d. All accounts.
   e. host op 306 map_show (the front-end draws the island map and the map panel; only drawing).
   f. When at least one soldier left in b (either way): host op 302 guide_say (0x98, 3); then for k = 0..7 in order
      with gone[k] < 0x40: host op 305 merc_gone (gone[k]): the status line shows "<nickname of profile gone[k]>
      seems to have disappeared!" (nickname = profile bytes from +0x00 up to the first 0).
5. **Collect the cash** (`collect_cash`, Morning pay).
6. **Worker pool** (`worker_pool`, Worker pool and natives who quit).
7. **The guide** (`guide_advice`, The guide's advice, below).

```c
/* strategic.md Strategic screen opening. Host ops 302, 305, 306 and those of the steps. */
void ja1_strategic_open(Ja1World *w, const Ja1Host *h);
```

Evidence (2026-10-23): StrategicScreen 0x432F4 with argument 1 (PlayLoop): 0x61DF4 (story events), 0x4660C, 0x43264,
DayEndMercUpdate? 0x43F54 (Random 0x53538 shuffle; TeamHasProfile? 0x50AC0, 0x50618, 0x50750 x 8, CalcLedgerAll,
InitStrategicMap 0x43ECC and drawing, 0x5F5AC(0x98, 3), StatusMessage? 0x27D24 with "%s %s"), 0x445DC, then
0x4247C (SetMusic, 0x50BC4 = Worker pool, drawing, NativeGuidePrompt? 0x5F94C).

## Story events at the day start

The tree blight story (entry point `story_day`; the first step of Strategic screen opening). World fields: `story`
(35 bytes; S[i] = byte i), `guide_queue` (7 bytes; Q[i] = byte i: the guide's messages of the day, read by the
guide's advice, not specified yet), `sector_state` (entry s: flags = byte 0, trees = byte 6, unhealthy = byte 8),
`sectors` (record s = 177 bytes from 177 s; R(s, o) = byte o of record s), `soldiers` records 0..7, `stash_counts`,
`difficulty`, `accounts` (sectors owned = the i32 at +0x74). Host ops 307 quest_record and 308 stash_take; Workable
trees (above). "s8 x" = x read as a signed byte; every byte change is 8-bit (wraps).

1. Q[0] = 0; Q[1] = Q[3] = Q[5] = Q[6] = 0xFF (Q[2] and Q[4] keep their values).
2. When s8 S[0x0A] = 1 (the blight runs):
   a. When S[0x20] ≠ 0: when s8 S[0x0B] = 1 or S[0x15] ≠ 0: Q[1] = 0x4D, Q[2] = 6; else Q[5] = 0x11. Then for each
      s = 0..59 in order: R(s, 0xAE) = unhealthy(s); unhealthy(s) = 0; R(s, 0xAB) = 0. Then S[0x0A] = 2.
   b. Else, when S[0x21] ≠ 0 and s8 S[0x0B] ≠ 1 and S[0x15] = 0: S[0x22] − 1; when s8 S[0x22] ≤ 0: S[0x20] = 1,
      Q[5] = 0x12.
   c. When s8 S[0x0A] < 2: when Blight spread gives yes: Q[0] = 1.
3. Else, when sectors owned ≥ 30 (signed) and S[0x0A] = 0: when Blight start gives yes:
   a. When s8 S[0x0B] = 1 or S[0x15] ≠ 0: Q[1] = 0x18, Q[2] = 6; when Squad search (0x51, take no) ≠ −1 and
      S[0x15] ≠ 0: Q[3] = 0xB0, Q[4] = 3.
   b. Else: Q[5] = 4; when Squad search (0x51, take yes) ≠ −1: S[0x21] = 1, and when s8 S[0x0E] < 2 or S[0x1E] ≠ 0:
      Q[6] = 0x0F, S[0x22] = 3; else S[0x22] = 1, Q[6] = 0x0D.
   c. S[0x0A] = 1.

**Blight start** (yes / no). Host op 307 quest_record (5): q = the 250 bytes it gives. For i = 0..24 in order: a =
the s16 at q + 10 i, b = the s16 at q + 10 i + 6; the first i where flags(a) bit 0 is set, trees(a) ≠ 0 and
flags(b) bit 0 is clear: unhealthy(a) = 1, R(a, 0xAD) = 1, R(a, 0xAB) = 0, R(b, 0xA4) = 0x5E (story object slot
4), the u16 at R(b, 0x8C) = the s16 at q + 10 i + 8 (its tile); the result is yes. No such i: no. (a or b outside
0..59: SPEC GAP (not seen: QUESTS.DAT records 5 and 6 hold only sectors 4..55 in the 1.13b file, 5..55 on the CD).)

**Blight spread** (yes / no). seen[0..59] = no, changed = no, limit = 4, 3, 2 for `difficulty` 0, 1, 2 (other
values: SPEC GAP (not seen: the Set Difficulty screen (world_load.md) gives 0, 1 or 2 only; another value needs an edited save)). For s = 0..59 in order:
1. When seen[s]: next s. seen[s] = yes. When unhealthy(s) = 0: next s.
2. R(s, 0xAB) + 1. When R(s, 0xAB) ≥ limit: trees(s) − 1, unhealthy(s) − 1, R(s, 0xAC) + 1, Workable trees of s.
3. When unhealthy(s) < trees(s) (unsigned): unhealthy(s) + 1, R(s, 0xAD) + 1, changed = yes; next s.
4. Else the neighbours in this order, d = −10 (not when s < 10), +1 (not when s mod 10 = 9), +10 (not when s ≥ 50),
   −1 (not when s mod 10 = 0), −11 (not when s < 10 or s mod 10 = 0), −9 (not when s < 10 or s mod 10 = 9), +11 (not
   when s ≥ 50 or s mod 10 = 9), +9 (not when s ≥ 50 or s mod 10 = 0): n = s + d; when trees(n) ≠ 0, unhealthy(n) =
   0 and flags(n) bit 0 is set: R(n, 0xAB) = 0, seen[n] = yes, unhealthy(n) + 1, R(n, 0xAD) + 1, changed = yes, and
   no more neighbours.
The result is changed.

**Squad search (type, take)**: for k = 0..7 with `inCurSector` ≠ 0, the item places i = 0..11 in order: the first
place whose byte 0 = type: when take, its 12 bytes = 0; the result is k. No such place: host op 308 stash_take
(type, take): when the low byte of its result ≠ 0: the result is 0x63, and when take, stash_counts[type] − 1. Else
−1.

So: once the player owns 30 sectors, a blight starts in the first listed sector with trees, and spreads every day
to more trees and to the owned neighbours; sick trees die after 2 to 4 days. The cure (item 0x51) stops it.

```c
/* strategic.md Story events at the day start. Host ops 307 quest_record, 308 stash_take; Workable trees. */
void ja1_story_day(Ja1World *w, const Ja1Host *h);
```

Evidence (2026-10-23): StoryDayEvents? 0x61DF4, 0x61FF4 (LoadQuestRecord 0x614D4: DAT\QUESTS.DAT record 0 to
0x22A302; OpenCDFile 0x38304: the game-folder file (1.13b patch) first, else the CD), 0x620EC (neighbour offsets 0x5F4BC, jump table 0x620CC, limits 0x95CAE), FindSquadItem? 0x61528 with
the store probe 0x51A24; S[0x20..0x22] = 0x22A060..0x22A062, Q = 0x22A4C9..0x22A4CF. Port test validate/diff/storyday_diff.py --port
(the original in Unicorn, quest record and store probe as host ops): 3000 cases, 0 mismatches; stratopen_diff.py
runs it as step 1 (1000 cases, 0 mismatches).

## The guide's advice

The native guide's (and Brenda's) messages when the island map opens (entry point `guide_advice`; the last step
of Strategic screen opening). World fields: `story` (43 bytes; S[i] = byte i), `guide_queue` (Q[i] = byte i,
written by Story events at the day start), `plants` (entry i = 12 bytes from 12 i: `capacity` i32 +0, `sector` i32
+4, `state` +8, countdown = byte +0xB), `mission_done`, `sector_state` (flags, enemies, tappers, guards,
unhealthy), `sectors` (R(s, o) = byte o of record s, 177 bytes each), `accounts` (Day accounts offsets:
g_treePrice, g_treesOwned, g_treesTapped, g_sectorsOwned, g_processCap, g_teamCount), `team_on_duty`, `merc_tire`,
`day`, `difficulty` (d), `purifier`, `hard_day`, `guides_hired`, `guide_wait`, `soldiers` records 0..7,
`profiles`. Host ops 302 guide_say (quote, category), 317 brenda_say (quote), 318 panel_draw (a, b), 307
quest_record, 319 guide_offer, 320 merc_join. "s8 x" = x read as a signed byte; every byte change is 8-bit
(wraps); comparisons are signed unless stated. "Say q" = guide_say (q, 6). "Redraw" = All accounts (Day
accounts), then panel_draw (0, 14). flags(s) / enemies(s) / ... = the `sector_state` byte of sector s (s may be
any i32 for the first loops; SPEC GAP (not seen: every sector here is a constant of the steps or the tip table, a plant sector (EXE data) or a sector of QUESTS.DAT records 5 and 6, all in 0..59) outside 0..59). Connected(s) = Connected to home of the low byte of s
(0 / 1).

1. **Plant nobody owns.** best = 99 (16-bit), pick = none. For i = 1, 2, 3 in order: when flags(`sector` of i)
   bit 0 is clear, s8 `state` = 1, countdown ≠ 0 and s8 countdown < best: pick = i, best = s8 countdown. When a
   pick: its countdown − 1; when it is then 0: `state` = 2, `mission_done`[`sector`] = 0.
2. **Owned plant.** best = 99, pick2 = none. For i = 1, 2, 3 in order: when s8 `state` = 1, Connected(`sector`) =
   1, countdown ≠ 0, s8 countdown < best, and (s8 S[5] = 2, or s8 S[5] < 2 and s8 countdown > 1): pick2 = i,
   best = s8 countdown. When pick2: its countdown − 1; when it is then 0: `state` = 2, `mission_done`[`sector`] =
   0, All accounts, panel_draw (0, 14), then say q with q from g_processCap (as unsigned): 45 → 0x31, 50 → 0x32,
   55 → 0x30, 75 → 0x34, 80 → 0x33, 85 → 0x35, 110 → 0x36; other values: q = the register EBP, which PlayLoop
   0x4FBAD sets to the run-time address of the string `DAT\ENDSAB.DAT` (image 0x90E60) and which StrategicScreen
   and the guide keep. That address depends on where DOS/4GW loads the data object (DOSBox-X, state tac60: delta
   0x1F1000, so q = 0x281E60); every possible value is above 0x90000. The table assumes the home plant's 20 counts, but plant 0 (sector 59) has state 0 in the EXE
   data and in all 783 saves of build/, and no step sets it, so the reachable capacities are the sums of the plants
   1..3 (35, 25, 30): only 55 is in the table, and the first repaired plant always gives that q (issue #2 row 35).
   With that q, PlayQuote seeks to record q × 200 of `EDT\NATQUOTE.EDT`, past its end: fread_ reads 0 bytes,
   so the text buffer keeps the text of the guide's previous quote; the voice members `64NAT<q>…` do not exist,
   so no voice plays. Seen live 2026-10-28 (DAY_day3 restored through the office Restore Game, plant 2 under
   repair on owned sector 58, story byte 5 = 2, Sleep; capacity 25): with Subtitles off the guide's face slides in
   with no words; with Subtitles on the text-only box shows his face, `Jack` and an empty text (the buffer was
   still empty: no quote before it in that program run). In both forms the wait does not end by itself (more than
   80 s); Enter, Escape, Space or a click ends it, and the advice goes on. That run came after a DOSBox-X state
   load, where the speech stream stays stuck at position 0. From a COLD start (2026-11-06, #2 comment 12040,
   Subtitles off) the same quote plays: no member means no seek, so PlayQuote's mode 1 streams the size of the
   last member found (0x2338, MAINSND R_L) from the place behind the archive's directory (0xA00), then the cry
   sample; the face goes by itself after 0.38 s with no key. JAC 999 at "The day's verdict" step 13 does the same
   (#2 row 62). The port does both (sound::QuotePart::Kept); with Subtitles on the box still waits for the player.
   The port uses q = 0x281E60 (the DOSBox-X value), so its record is past the end too. With an earlier quote in the
   same program run, the Subtitles-on box shows that earlier text: the guide, Brenda, Santino, merc messages,
   A.I.M., Walter and the sector signs all read their 200-byte EDT record into one text buffer (0x2299BC), and
   showing a text does not change that buffer (Unicorn 2026-10-28: after SpeechBalloon? 0x299B8 the buffer still
   holds the +1-encoded record). The port keeps one such buffer for these speakers (a record read past the end of
   its file leaves it as it is) and shows its text.
3. said = 999; gap = 0. When g_processCap < 110, pick2 = none: gap = g_processCap − g_treesTapped; when gap < 6
   and s8 S[5] = 2: q = 0x2D − gap for gap 0..5, 0x2E for gap < 0; say q; said = q.
4. **Tree price story** (S[0x27] = stage, S[0x28] = counter, S[0x29], S[0x2A] = said once). By stage (stage ≥ 5:
   nothing):
   - 0: when g_treesTapped > 16 or g_treesOwned > 32: say 8, said = 8, stage + 1.
   - 1: when g_treesTapped > 20 or g_treesOwned > 40: say 10, said = 10, S[0x28] = random(3) + 1, stage + 1.
   - 2: S[0x28] − 1; when it is then 0: say 12, said = 12, g_treePrice − 100, Redraw, stage + 1.
   - 3: t = (g_treesTapped / 10) × 20 + 300 (signed division, truncated); L = 4, 3, 2 for d = 0, 1, 2 (other d:
     SPEC GAP (not seen: the Set Difficulty screen (world_load.md) gives 0, 1 or 2 only; another value needs an edited save)).
     - When t < g_treePrice: S[0x28] − 1; when s8 S[0x28] ≠ −L: nothing more. Else S[0x28] = 0, g_treePrice − 20,
       q = 0x0E when S[0x29] = 0 (then S[0x29] = 1), else 0x47.
     - When t > g_treePrice: S[0x28] + 1; when s8 S[0x28] ≠ L: nothing more. Else g_treePrice + 20, S[0x28] = 0;
       q = 0x49 when g_treePrice is now 500 (then stage + 1), else 0x16 when S[0x2A] = 0 (then S[0x2A] = 1), else
       0x48.
     - When t = g_treePrice: nothing.
     With a q: say q, said = q, Redraw.
   - 4: nothing.
5. When s8 S[6] = 1 and S[0x25] = 0: brenda_say (10), said = 10, S[0x25] = 1.
6. When S[2] ≠ 0: S[2] − 1; when it is then 0: q = 0x37 when s8 S[7] = 2, else 4; say q, said = q; `merc_tire` =
   0.
7. Plant 2 is p = plant entry 2. When S[0x0D] = 0: when flags(p `sector`) bit 0 is set, Connected(p `sector`) = 1,
   s8 p `state` = 2, gap × 2 < 25, (enemies(43) > 6 or enemies(52) > 6 or enemies(54) > 6, unsigned) and
   unhealthy(p `sector`) = 0: say 0x4A, said = 0x4A, S[0x0D] = 1. Else (S[0x0D] ≠ 0): when s8 S[0x0D] = 1:
   S[0x0D] = 2.
8. **The queued messages**: when Q[1] ≠ 0xFF: guide_say (Q[1], Q[2]), said = Q[1]. When Q[3] ≠ 0xFF: guide_say
   (Q[3], Q[4]), said = Q[3]. When Q[5] ≠ 0xFF: brenda_say (Q[5]), said = Q[5]. When Q[6] ≠ 0xFF: brenda_say
   (Q[6]), said = Q[6].
9. a. When s8 S[9] = 2, s8 S[6] = 2, s8 S[0x0B] < 1, flags(27) bit 0 clear, Q[5] = 0xFF and Q[6] = 0xFF:
      quest_record (6) gives the 250 bytes q; a(i) = the s16 at q + 10 i, b(i) = the s16 at q + 10 i + 6. The
      first i of 0..24 with flags(a(i)) bit 0 clear and flags(b(i)) bit 0 clear: S[0x0B] = 1; with a = a(i), b =
      b(i), w(o) = the s16 at q + 10 i + o: R(a, 0x9F) = 0xAE, R(a, 0xA4) = 0x5C, R(a, 0xA0) = 0xAF, the u16 at
      R(a, 0x82) = w(2), the u16 at R(a, 0x84) = w(2) + 1, the u16 at R(a, 0x8C) = w(4); S[0x17] = the byte at q +
      10 i; R(b, 0xA4) = 0x7A, the u16 at R(b, 0x8C) = w(8); the u16 at R(27, 0x8C) = 0x55C, R(27, 0xA4) = 0x86;
      say 0x14, said = 0x14. No such i: nothing.
   b. When s8 S[0x0B] = 1 and 0 < S[0x14] < 11 (unsigned): say S[0x14] + 0x38, said = that.
   c. When s8 S[0x0C] < 1 and (guards(5) ≠ 0 or tappers(5) ≠ 0): quest_record (7) gives q; the first i of 0..3
      with flags(a(i)) bit 0 clear: R(a(i), 0xA4) = 0x77, the u16 at R(a(i), 0x8C) = w(2), S[0x0C] = 1, say 0x12,
      said = 0x12. No such i: S[0x0C] = 2.
10. When S[0x0E] = 0: S[0x0E] = 1. Then when s8 S[0x0E] ≥ 2, S[0x1E] = 0 and S[0x1F] = 0: when s8 S[0x0B] = 1 or
    S[0x15] ≠ 0: say 0x1A, said = 0x1A; else brenda_say (6), said = 6. Then S[0x1F] = 1.
11. **Five tips** (table below; entry k, read e0 .. e11 as signed bytes, e12 = the quote). For k = 0..4 in order,
    n = the sum of:
    - 1 when e0 > `day` (signed 32-bit);
    - 1 when e1 ≠ 100 and s8 S[5 + e1] < 2;
    - 1 for each of e2, e3 that is ≠ 100 with S[5 + it] ≠ 0;
    - for k = 1 and 4: 1 when s8 S[0x0B] = 1 or S[0x15] ≠ 0;
    - 1 when all five sectors e4 .. e8 have flags bit 0 set;
    - Connected(e9);
    - 1 when said ≠ 999 and e10 ≠ 0;
    - 1 when e11 > g_sectorsOwned;
    - 1 when s8 S[5 + k] = 1;
    - for k = 2, when the sum so far is 0: 1 when random(100) ≥ 20, 50, 80 for d = 0, 1, 2.
    When n = 0: q = e12; when q < 500: say q, else brenda_say (q − 500); said = q; S[5 + k] = 1; then for k = 0:
    when s8 d ≤ 1: `purifier` = 1, else R(c, 0x9E) = 0x79 with c = 0x30, 0x31, 0x3A for random(3) = 0, 1, 2; for
    k = 1 and 2: S[2] = 0, `merc_tire` = 30, 50, 70 for d = 0, 1, 2.

    | k | e0 day | e1 | e2 | e3 | e4 .. e8 sectors | e9 | e10 | e11 | quote |
    |---|---|---|---|---|---|---|---|---|---|
    | 0 | 1 | 100 | 0 | 100 | 0 0 0 0 0 | 0 | 0 | 1 | 0 |
    | 1 | 2 | 0 | 1 | 100 | 28 29 38 48 49 | 0 | 0 | 1 | 2 |
    | 2 | 3 | 1 | 100 | 100 | 28 29 38 48 49 | 0 | 1 | 1 | 6 |
    | 3 | 12 | 100 | 7 | 3 | 5 59 59 59 59 | 5 | 1 | 24 | 0x10 |
    | 4 | 1 | 100 | 4 | 100 | 0 0 0 0 0 | 27 | 1 | 7 | 500 (Brenda 0) |

12. a. When s8 S[9] = 1: S[0x26] + 1; when it is then 3 (unsigned): say 0x4B, said = 0x4B.
    b. When s8 S[8] = 1, Connected(5) = 0 and S[0x24] = 3: say 0x4C, said = 0x4C.
    c. When s8 S[8] = 2, Connected(5) = 0, S[0x12] ≠ 0 and said = 999: say 0x38, said = 0x38, S[0x12] = 0.
    d. When Q[0] ≠ 0 and (said = 999 or S[0x23] = 0): say 0x4E (said stays), S[0x23] = 1.
13. **Hire offer.** When the guide merc (Day start: the first record k of 0..7 with `inCurSector` ≠ 0, `hp` ≠ 0
    and `profileId` 60..64) is none: `guide_wait` + 1; then when s8 `guide_wait` > 1, s8 `guides_hired` < 4, m =
    the number of records 0..7 with `profileId` < 60, `inCurSector` ≠ 0 and s8 `hp` > 0 is 1..7, and random(100)
    < 70 − 20 × s8 `guides_hired` + 5 × s8 `guide_wait`: say `guides_hired` + 0x43 (as a signed byte; said
    stays); p = the low byte of `guides_hired` + 60; guide_offer (p) (the host asks "Accept this native guide?"
    with the face and nickname of profile p); when its answer ≠ 0: merc_join (p) (the host adds profile p to
    the team: A merc joins the team), `guides_hired` + 1,
    g_teamCount + 1, `team_on_duty` + 1, panel_draw (9, 10). Then (either answer) `guide_wait` = 0.
14. **Weather.** When random(100) < 10: say 0x4F, `hard_day` = 1; else `hard_day` = 0.

So: the guide talks about new plants, the tapping limit, the falling and rising tree price, the next tips of the
story, and offers to bring one more native guide (up to four) when the team has no guide; one day in ten he warns
of hard weather.

```c
/* strategic.md The guide's advice. Host ops 302, 307, 317..320; All accounts. */
void ja1_guide_advice(Ja1World *w, const Ja1Host *h);
```

Evidence (2026-10-23): NativeGuidePrompt? 0x5F94C..0x6067C (jump tables 0x5F90C gap quotes, 0x5F924 price stages,
0x5F938 tips; tip table 0x95BD4 (16 B per entry; helpers 0x61440 e2/e3, 0x61488 e4..e8); per-difficulty s16
0x95C8A random limits, u8 0x95C90 merc_tire, s16 0x95C96 L; local sectors 0x5F4B0); S = 0x22A040 +i (dword reads
at X with sar 24 read byte X + 3); guide_say 0x5F5AC (EAX quote, EDX category), BrendaSpeech? 0x5F76C (EAX),
0x36010 (ECX a, stack b) = panel_draw, CalcLedgerAll 0x51178; guide merc 0x62A00, count 0x410C4; hire: 0x67BF0,
0x467E4, YesNoPrompt? 0x28C84 ("Accept this native guide?" 0x97977), 0x28698, 0x673AC = guide_offer, 0x21FC0
(0x21B74 fills the record, LoadMercFace) = merc_join; guides_hired 0x22A073, guide_wait 0x22A074, hard_day
0x22A10C, purifier 0x22A0CB, d = 0x22A0CC. Harness validate/diff/guide_diff.py.

## A merc joins the team

The entry point `merc_join` (port_api.md) adds profile p (0..63) to the team. The original calls it when you accept
a native guide (host op 320 merc_join, The guide's advice step 13) and when you hire an AIM merc.

1. k = the team size: the number of records from slot 0 up to the first with `inCurSector` = 0 (at most 8). S =
   soldier record k, P = profile p. (The callers never call it with 8 mercs on the team; the port does nothing
   then and returns −1.)
2. The nickname of P (profile +0x00) is copied into S `nickname` (+0x16E) up to and with its 0 byte.
3. S `slot` = k, S `profileId` = p; bytes +0x16A, +0x16B, +0x16C = 0; `inCurSector` = 1; byte +0x194 = 0.
4. S `salary2` and `salary` = the profile word +0x28. S `hp` = P byte +0x2A, then `hpMax` = `hp`.
5. Bytes from P into S: `energy` (+0x18A) = +0x2B, `agility` (+0x18B) = +0x2C, `dexterity` (+0x18C) = +0x42,
   `wisdom` (+0x18D) = +0x2E, `medical` (+0x18E) = +0x40, `explosives` (+0x18F) = +0x43, `mechanical` (+0x190) =
   +0x41, `marksmanship` (+0x191) = +0x2D, `level` (+0x192) = +0x44, `status` (+0x19B) = +0x4A, +0x19C = +0x2F,
   +0x17C = +0x49, `colour` (+0x17B) = +0x48.
6. S `killerSlot` (+0x1DA) = 0x40; bytes +0x1D9, +0x1B1, `duty` (+0x17E), `wounds` (+0x185), +0x188 and `healed`
   (+0x19A) = 0; the word `fatigue` (+0x10C) = 0; `energy` = 100.
7. **Practice start** (stat_progress row k, 9 signed 16-bit values): value 8 = 0. Then for i = 0..7 with the stat
   field f = `hpMax`, `agility`, `dexterity`, `wisdom`, `medical`, `explosives`, `mechanical`, `marksmanship` and
   L = the signed byte `level` (read each time):
   - `status` 0: value i = 0.
   - `status` 1: f ≥ 100: value i = 0. Else r = random(100), value i = r, and when L < 10: value 8 + r.
   - `status` 2: f = 0: value i = 0. Else r = random(100), value i = −r, and when L > 1: value 8 − r.
   - Any other `status`: stop the practice start here (values i..7 stay as they were).
   (16-bit values: the sums wrap.)
8. Items i = 0..11 (12-byte entries at S + 12 i): type t = P byte +0x50 + i for i < 10, else 0. t = 0: count (+1)
   = 0, condition (+2) = 0. Else count = item_types[12 t + 5] when item_types[12 t + 6] = 1, else 1; condition =
   20 when t = 0x6F, else 100. Then Item reset (sector_start.md) of the entry with cond_mode 0, count_mode 0.
9. When item 0 has type 0 and item 1 does not: bytes +0..+5 of item 1 move to item 0 (item 1 bytes +0..+5 = 0).
10. The host loads the face of profile p for team slot k (the original sets g_facesLoaded = 0, then LoadMercFace
    (k, p)). The entry point returns k.

The caller counts the team itself (the guide offer: g_teamCount + 1 and `team_on_duty` + 1).

```c
/* strategic.md A merc joins the team. Host random; returns the team slot or -1. */
int32_t ja1_merc_join(Ja1World *w, const Ja1Host *h, int32_t profile);
```

Evidence (2026-10-24): 0x21FC0 (TeamSize 0x513B8, soldier pointer 0x100958[k], 0x21B74(S, profile, k, p),
g_facesLoaded = 0, LoadMercFace); 0x21B74 (string copy to 0x16E, memset_ 0x16A 3, the field copies, 0x21DB8, the
item loop with item table +5 / +6 at 0x94DFD / 0x94DFE and 0x6F, ItemInit? 0x53708(entry, 0, 0), the hand move at
0x21D4E); 0x21DB8 (row 0x22A15A + 18 k, jump table 0x21D98, status compare at 0x21F09, Random 0x53538, sar 24 of the
dword at +0x18F = level); also called by the AIM hire 0x22014. Harness: validate/diff/ai_diff.py section pay, kind
"join".

## The assignment screen

A click in the box of number field 0 (the team pay) on the map panel opens the **assignment screen**: one column
per merc in the sector with the face, the duty, the stats and the health bar, and a panel with the pay numbers.
The player gives each merc a duty here. Duties (soldier `duty`) and their names:

| duty | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| name | ON DUTY | REST | TRAIN | DOCTOR | PATIENT | REPAIR | FIRE | ESCORT |

Training subjects (soldier `trainWhat` 0..4): PHYS. / MED. / EXPL. / MECH. / MARKS. A duty above 7 or a
`trainWhat` above 4 has no name here (SPEC GAP (not seen: the game writes duty 0..5 only, from the duty menu and the day steps, and `trainWhat` 0..4 from the subject menu): the original reads the next bytes of its text table).

### The assignment picture

The picture is built in the panel buffer (pitch 320; panel.md "Drawing primitives"; all text with mode 300). The
mercs are the team slots k = 0, 1, ... while slot k is `inCurSector` and k ≤ 7 (the first slot not in the sector
ends the list). Column x of merc k: X = 37 k.

1. ASSMAP.PCC (MAIN.DAT, 320 x 146) at (0, 0).
2. For each merc k (S = soldier k), in order:
   a. **Face**: the 28 x 25 pixels of face buffer k (32 x 32, panel.md "Buffers and data") from column 2, row 2
      to (X + 13, 20), all pixels written.
   b. **Duty choices**: doctor-able = place 0 holds type 0x49 or 0x4A and `medical` ≠ 0; repair-able = place 0
      holds type 0x4B and `mechanical` ≠ 0. When `duty` = 3 and S is not doctor-able, or `duty` = 5 and S is
      not repair-able: `duty` = 0 (the world's soldier changes: drawing the picture puts such a merc on duty).
   c. When `duty` ≠ 0: every pixel p of the 28 x 25 face box at (X + 13, 20) becomes remap table 30 [p]; then the
      **status field** at (x, y) = (X + 11, 38), font 0, fg 0x27, shadow 9:
      - duty 2: the training subject, centred at x + 17.
      - duty 3: b = Doctor budget, c = its cap (Morning duties "Doctor budget": cap before the min).
      - duty 4: the number signed `hp` − signed `hpMax` with 3 digits at x + 8 (zero or below).
      - duty 5: b = Points, c = the Repair value v (Morning duties "Repair value").
      - duty 3 or 5: b as a number with 3 digits at x, "/" centred at x + 15, c as a number at x + 18 with d digits: d = 1
        when c < 10, 2 when c < 100, else 3.
      - other duties: nothing.
   d. **Stats** i = 0..9 at sx = 27 + 37 × S `slot` (the slot byte, not k), sy = 59 + 8 i. Value: i = 0 pay
      (`salary2`, unsigned 16-bit), 1 signed `hp`, 2 `agility`, 3 `dexterity`, 4 `wisdom`, 5 `medical`, 6
      `explosives`, 7 `mechanical`, 8 `marksmanship`, 9 `level` (2..9 signed bytes). Colour: 237 for even i, 41
      for odd i; 0xFF instead when stat_changed[`profileId` × 10 + i] ≠ 0 (Morning stat changes). Draw the number
      with 3 digits at (sx + 1, sy + 1) with fg 7, then at (sx, sy) with the colour; both font 0, shadow 0.
   e. The duty name centred at (X + 29, 9), font 0, fg 0xB5, shadow 8.
   f. The nickname (up to its 0 byte) centred at (X + 28, 48), font 0, fg 0x0E, shadow 5 (SPEC GAP (not seen: the
      nicknames are those of EDT\NEWPROF.EDT, letters only, highest byte 0x7A, and no screen of the game edits
      them): a nickname byte { | } ~ or above; the original writes outside the buffer with font 0).
   g. The **assignment health bar** at (x, y) = (X + 44, 20) (below).
3. ASSPAN.PCC (MAIN.DAT) at (0, 146); then the number fields 27..30 (as the map panel's Number fields, b = 13):

| k | value (i32 of `accounts`) | at (x, y) | d | fg | shadow |
|---|---|---|---|---|---|
| 27 | g_teamHalfSalaries +0x30 | (161, 164) | 6 | 121 | 12 |
| 28 | g_teamSalaries +0x34 | (161, 182) | 6 | 121 | 12 |
| 29 | g_teamPay +0x38 | (217, 164) | 6 | 121 | 12 |
| 30 | g_balanceAfterDay +0x28 | (217, 182) | 6 | money colour (as field 4) | 0 |

**Assignment health bar** (signed bytes, "/" truncates toward zero): nh = `hp` / 4; nm = `hpMax` / 4; nw = 0
when `wounds` = 0, else `wounds` / 4 or 1 when that is 0. ne = 0 when `hpMax` − `hp` ≤ `wounds`, else nm − nh − nw
or 0 when that is below 0. gap = 25 − nw − ne − nh. When gap > 0: element 88 at (x, y) with gap rows. y += gap
(also when gap is below 0). When nw > 0: nw bar rows of element 87 at (x, y). When ne > 0: ne bar rows of element
86 at (x, y + nw). y += nw + ne. When nh > 0: element 81 at (x, y), then nh − 1 bar rows of element 80 at
(x, y + 1). (This is not the tactical panel's health bar: other rounding, no selected colour.)

Evidence (2026-10-24): DrawAssignMap? 0x478A4 (argument 99 = no mouse move; < 8 = mouse to the merc's nickname),
duty choices 0x4811C (also sets the patient-able flag hp < hpMax for the click loop), status field 0x47700
(DoctorCap? 0x483AC, RepairCap? 0x4882C), stat row 0x47504 (table 0x93F10: x, y, fg, colour 7, mode per stat;
StatValue? 0x47350), HealthBar? 0x47144 (ECX 1 = draw the gap; stack 0 = no screen copy), RemapRect 0x52481 with
the table pointer 0xE8CC0, texts 0x9662A / 0x966A1 / "/" 0x90B88, 0x36010 (ASSPAN.PCC at g_panelBuf + 0xB680,
fields 27..30); validate/diff/assign_diff.py.

### Opening and leaving the assignment screen

**Open** (at the release of the field 0 box press, strategic.md "Number field boxes"):
1. Picture steps `ASSFLIP1.PCC`, `ASSFLIP2.PCC`, `ASSFLIP3.PCC` (as the finance page's Open step 1).
2. Build the assignment picture (above; this can change duties).
   Then bytes 0..5 of the cursor item and of the swap item (inventory.md "Data") = 0; bytes 6..11 keep their
   values. No item is on the cursor while the screen is open at the start.
3. The picture slides in from the bottom while the island map moves up, as the payroll screen's steps 4, 6 and 7
   ("The payroll screen") with P = the assignment picture and B = the screen before the flips (the island map):
   a picture step shows P's rows 146..199 (ASSPAN.PCC and its number fields, as drawn in step 2); the slide sound
   is main-bank effect 5 (SCR), rate 0x2B11, volume 0x32 (not 0x58), 2 times; then the 24 slide steps, each a
   clock wait of 1 ms (one game clock step, ~55 ms; world_load.md "Game speed"). At the end the screen holds the
   picture's rows 1..142 and 146..199; screen rows 0 and 143..145 keep what they showed before the screen opened.

**Exit button** (button 9): x = 269, y = 166; the mouse is in it when 269 ≤ x ≤ 294 and 166 ≤ y ≤ 181. Faces as
the finance page's back button at this place: up = element 20 at (269, 166), then element 58 at (274, 170);
pressed = element 21 at (269, 166), then element 58 at (273, 169). A left click in it: the click sound, pressed,
wait until the left button is up, up; then **Leave**. The Enter key also leaves.

**Leave**: when `team_on_duty` > 0 (the value as it was before the screen opened: the duty changes of the picture
do not count it again), or else when the question box (panel.md "Question box") "Hmmm. No one assigned to duty?"
with Q_MARK.PCC answers yes:
1. When the current sector is not 0xFF: its `mercs` byte (sector state +2) = `team_on_duty`.
2. The screen closes with the slide back: keep a copy M of the island map view (rows 0..143, the map picture
   as the strategic screen shows it); the slide sound (main-bank effect 5, rate 0x2B11, volume 0x32, 2 times);
   then 24 steps j = 0..23, each after a clock wait of 1 ms (one game clock step): screen rows 4 .. 4 + 6 j =
   M rows 142 − 6 j .. 142 (the map comes down from the top over the assignment picture, which does not move).
   Right after the last step the picture steps `ASSFLIP3.PCC`, `ASSFLIP2.PCC`, `ASSFLIP1.PCC` (rows 146..199,
   each shown and then 60 ms on the game clock; the page sound with the second, as the finance page's Close
   step 1), then `MAPPAN.PCC` and its number fields as the finance page's Close step 2. (The original leaves
   screen rows 0..3 and 143..145 as the assignment picture had them; the front-end draws the whole island map
   view again.)
When the question answers no, the assignment screen stays open.
After Leave the strategic screen (0x434F6) clears the current sector (0xFF) when `team_on_duty` is 0, and draws the
map again. Open sets the flag `hinted` (0x4893A): after a visit the start button shows no hint messages.

Evidence (2026-10-27, Leave step 2, asm 0x4A2C8..0x4A437): BlitCopy(g_panelBuf, *0x93E5C, 320, 144, pitch 320, 640); PlaySfxSlot(5, 0x2B11, 0x32, 2); loop esi = 0xB180 − 0x780 j, edi = 1 + 6 j: TimerElapsedMs(0x22A3FC) ≥ 1, TimerStart, BlitCopy(*0x95678 + 0x500, g_panelBuf + esi, 320, edi) until esi = −0x280; PlayFlipAnim?(0x90B98 "assflip", 3, 3, −1); 0x36010(mappan.pcc, g_panelBuf, 320, 0, 14); 0x42090; TimerElapsedMs ≥ 60; 0x41608. A.I.M. right button: AimScreen 0x2011C at 0x20934..0x20989 (TimerStart 0x22A3FC, AimBrowse(+1), wait TimerElapsedMs ≥ 200 or button 2 up).
Evidence (2026-10-24): AssignScreen? 0x4891C at 0x48940..0x489A1 (cursor 0x222B50 and swap 0x222B68 bytes 0..5 = 0). 0x47AE4 (saves the screen, DrawAssignMap?(99), AssignPanelFlipIn? 0x47B68 =
PlayFlipAnim?("assflip", 1, 3) + ASSPAN.PCC 0x36010(asspan.pcc, g_panelBuf, 320, 27, 30) + TimerElapsedMs
60 + 0x41608 + PlaySfxSlot(5, 0x2B11, 0x32, 2) + 24 slide steps of 6 rows: the same BlitCopy pairs as the payroll
slide, back + 320 (7 + 6 j), panel + 320 (139 − 6 j) for 142 − (138 − 6 j) rows, each after TimerElapsedMs > 0); run in Unicorn on tac60 with the timers
stubbed, the screen afterwards differs from the panel buffer only in rows 0 and 143..145 (the noise before).
AssignScreen? 0x4891C: Enter or 0x67CD8(9, 1) (button table 0x9345A entry 9 = entry 8 at another place), the
g_teamOnDuty test, 0x278A0("Hmmm. No one assigned to duty?" 0x9834F, "q_mark.pcc" 0x90BD4), g_sectorTbl9[cur].mercs.

### The duty menu

All drawing of the menu goes to the screen (pitch 320); "the whole picture" means: build the assignment picture
again (above) and copy all 200 rows of the panel buffer to the screen (rows 0 and 143..145 then show the picture's
rows too). Duty choice flags F[0..6] of a merc S: F0 = F1 = F2 = F6 = 1 (ON DUTY, REST, TRAIN, FIRE), F3 =
doctor-able, F4 = signed `hp` < signed `hpMax`, F5 = repair-able (doctor-able / repair-able as in the picture's
step 2b). Duty 7 (ESCORT) is never in the menu.

**Open**: while no merc is selected, a left button that is down with the mouse in the duty name box of team slot k
(k = 0..7, X = 11 + 37 k: X ≤ x ≤ X + 36, 7 ≤ y ≤ 15) opens the menu when slot k is `inCurSector` (the slot is
checked alone: the picture's list end does not count); when it is not, wait until the left button is up and do
nothing. Open, with S = soldier k and x0 = X − 3:
1. The **merc pay panel** into the panel buffer: ASSPAN.PCC and the number fields 27..30 (as the picture's step
   3), then for i = 0..9 the stat value i of S (as the picture's step 2d) as a number with 4 digits at
   (px + 1, py + 1) with fg 7 and then at (px, py) with fg 237 for even i, 41 for odd i (never 0xFF), font 0,
   shadow 0. (px, py) = (50, 156 + 7 i) for i ≤ 4, (112, 156 + 7 (i − 5)) for i ≥ 5.
2. Copy the whole panel buffer to the screen. The click sound.
3. Element 0x41 (the pressed duty name; sheet (78, 107), 37 x 9) at (X, 7); the duty name of S centred at
   (X + 17, 9), font 0, fg 0xB5, shadow 8.
4. The menu box: element 0x3B (sheet (0, 84), 42 x 62, no pixel 0) at (x0, 12).
5. For each i = 0..6 with F[i] = 1: the label = duty name i at (x0 + 7, 19 + 7 i), font 0, fg 8, shadow 0x0E.
6. The mouse moves to (its x, 24 + 7 `duty`). No label is lit (h = none). The left button counts as not
   released yet.

**Menu loop** (each input poll, mouse (x, y)):
- Mouse outside the box (x < x0, x > x0 + 41, y ≤ 12 or y ≥ 73): when a label h is lit, draw label h again
  normally (fg 8, shadow 0x0E); h = none.
- Inside: for the row i (0..6) with 19 + 7 i < y < 27 + 7 i (no row: nothing changes): when F[i] = 0, unlight h
  as above and h = none; when F[i] = 1 and i ≠ h, unlight h, h = i and draw label i at its place with fg 0xFF,
  shadow 8 (over the normal label).
- Then the buttons: left down and released before: the click sound, **choose** h. Left down, not released yet:
  go on. Left up and right down: h = none, **choose** none. Both up: released.

**Choose** c:
- c = 6 (FIRE): **Firing a merc** (below) with S and X; then wait until the left button is up (the steps below
  replace the last line).
- c = 2 (TRAIN): the **subject menu** opens at (sx, 30) with sx = x0 + 10 for k < 7, x0 − 8 for k = 7; its answer
  none makes c none, else S `trainWhat` = the answer.
- c = 0..5 (after TRAIN still not none): S `duty` = c; CalcLedgerAll (Day accounts); ASSPAN.PCC and number fields
  27..30 again into the panel buffer, then the panel buffer rows 146..199 to the screen.
- Then (every c): the whole picture. No merc is selected; wait until the left button is up.

**Subject menu** at (sx, sy), names PHYSICAL / MEDICAL / EXPLOSIVES / MECHANICAL / MRKSMNSHIP (subjects 0..4):
element 0x28 at (sx, sy), element 0x29 at (sx, sy + 20), element 0x2A at (sx, sy + 30); the names i = 0..4 at
(sx + 7, sy + 9 + 7 i), font 0, fg 8, shadow 0x0E. h = none, not released. Loop: mouse outside (x < sx,
x > sx + 56, y ≤ sy or y ≥ sy + 60): unlight h (fg 8, shadow 0x0E), h = none. Inside: for each i with
sy + 9 + 7 i < y < sy + 17 + 7 i and i ≠ h: unlight h, h = i, name i with fg 0xFF, shadow 8. Then left down and
released: the click sound, close, answer h; left down, not released: go on; left up and right down: close, answer
none; both up: released. Close = the screen part under the menu comes back (the whole picture follows anyway).

Evidence (2026-10-24): AssignScreen? 0x4891C at 0x48B86..0x48C9D (open), 0x48CA9..0x48EDD (loop), 0x48EE5..
(choose); 0x4811C (flags 0x22A4E8..0x22A4EE), DrawAssignPanel 0x476B8 (0x36010 ASSPAN fields 27..30, 0x47600 with
table 0x93F88 (x, y) and the colours of 0x93F10), 0x46BF4 (panel buffer to screen), 0x46920 (BlitKey of
g_panelElems 0x93732 (x, y, w − 1, h − 1), key 0), 0x467E4 / 0x468B0 (save / restore a screen part of table
0x936BA into g_backBuffer, pitch 380), DrawOnDutyLabels 0x481C0, 0x681D0 (mouse to (x, y)), 0x5C8A0 (click
sound), 0x41608 (panel rows 146..199 to the screen), PhysicalPopup? 0x4B148 (texts 0x9666A, 11 bytes each),
"%s%s?" 0x90BE0 + "Fire " 0x99EAF, YesNoPrompt? 0x28C84; validate/diff/duty_diff.py.

### Firing a merc

FIRE in the duty menu of merc S (team slot k, duty name box X = 11 + 37 k):
1. Save the screen part 0 (216 x 80 at (52, 37), as the question box). The text "Fire <nickname of S>?".
2. The **merc-face question box** (panel.md) with that text, the profile of S and the nickname of S.
3. No: put the screen part back; the whole picture; no merc is selected. End.
4. Yes: put the screen part back, then the screen part under the duty menu. Then the entry point `fire_merc`
   (port_api.md, slot k, x X), steps 5..9.
5. When g_day − (signed byte S +0x1E5, the day the merc was hired) = 0: the profile of S word +0x3E = S
   `salary2` (+0x112), and the accounts byte +0xA1 (non-payments) + 1.
6. **Fire the merc** (below) with S, X and y 0x14.
7. The host loads the panel sheet again (the quote balloon draws over it).
8. Close up the team (redraw 0); CalcLedgerAll (Day accounts).
9. The whole picture; no merc is selected.

**Fire the merc** (S, x, y), p = S `profileId`:
- p ≥ 60 (a native): all items of S into the store as key E does (no item panel redraw), then Native leaves (S).
  End.
- S byte +0x19D ≠ 0: Merc leaves (S, status 0). End. (The items stay with the merc: the listed ones go to the
  store as Merc leaves does.)
- Else (an AIM merc): the profile of p byte +0x3D (`firedByYou`) = 1; the world byte `voice_test` = 1; the host
  plays quote 9 of p and shows it in the speech balloon of S at (x, y) (host op 328 `fire_quote`, below). Then
  d = g_day − (signed byte S +0x1E5), a 32-bit difference. When d > 0: r = random(d as a signed 16-bit number);
  when r = 0: **Grudge** (p, random(3) + 1). When d ≤ 0 (fired on the day of the hire): Grudge (p, 4). Then the
  accounts byte +0x9C (terminations) + 1; all items of S into the store as key E does (no redraw); Merc leaves
  (S, status 1). So the gear of a fired AIM merc stays with you, in the store.

**Grudge** (p, n): n times: u = random(60). When u is not the `profileId` of a team soldier 0..7 with
`inCurSector` ≠ 0 (TeamHasProfile), u ≠ p, profile u byte +0x3C ≠ p, profile u byte +0x4C = 99, profile u byte
+0x2F ≠ 3 and u ≠ 7: profile u byte +0x4C = p (a merc who will not join now). A try that fails is not repeated.

Host op 328 `fire_quote` (k, x, y, p): the original plays PlayQuote (p, 14 p, category 9 (AIM), 200, the text
buffer, 0, p, 1), shows the balloon 0x50908 (S, x, y, 5) and, when the option Subtitles is on, puts the screen part
0 back (0x28698). The balloon 0x50908 is the one of the Not enough funds dialog (above, "Dismiss", ops 276 / 277):
Subtitles off: the face of p (MAIN.DAT member p) slides in at the face point (x, y = 0x14) and out as "Speech
balloon" steps 1 .. 8 (panel.md), no text; Subtitles on: the merc-face box with the text only (YesNoPrompt? 0x28C84
with ECX 0, the nickname S +0x16E). The text is AIMQUOTE.EDT record 14 p + 9 (the text buffer 0x2299BC that
PlayQuote fills); the wait ends with the voice (or after 80 ms per text byte without one) or at a key or click.
The screen under the balloon is the assignment screen without the question box (0x28698 put it back), with the team
not yet compacted. Front-end: the window shows it (stderr `balloon fire K X P`, smoke window_day).

Evidence (2026-10-24): AssignScreen? 0x4891C at 0x48FD2..0x490D9 (0x467E4(0, 0x34, 0x25); sprintf "%s%s?"
0x90BE0 with "Fire " 0x99EAF and S +0x16E; YesNoPrompt? 0x28C84(text, DL profile, EBX nickname, ECX 1, stack 0);
no: 0x28698(0x25) and DrawAssignMap? 0x478A4(0x63); yes: 0x28698, 0x468B0(0xB) (the duty menu part), the +0x1E5 test
at 0x4904A, 0x5097C(AL slot +0x17A, EDX X ([esp + 0x98] = esi of the slot loop at 0x48BAE), EBX 0x14), LoadPanels
0x46EC4, 0x50750(0), CalcLedgerAll 0x51178, 0x478A4(0x63, 0x63)); fire merc 0x5097C (key E store 0x52150(S, 0),
NativeLeaves? 0x505A0, Merc leaves 0x50618, PlayQuote 0x24F08, balloon 0x50908, 0x28698 under byte 0x1163BD, Random
0x53538 with movsx of the difference, grudge 0x50B50, g_aimTerminations 0x22A03C); grudge 0x50B50 (Random(0x3C),
TeamHasProfile? 0x50AC0); validate/diff/ai_diff.py section pay, kinds "fire" (0x5097C) and "fire_yes" (0x4904A..
0x49095).

### Selecting a merc

While no merc is selected, a left button that is down with the mouse in the **face box** of team slot k (k = 0..7,
X = 11 + 37 k: X ≤ x ≤ X + 30, 20 ≤ y ≤ 46; the duty name boxes are tested first, slot by slot) selects merc k when
slot k is `inCurSector` (checked alone, as for the duty menu); when it is not, wait until the left button is up and
do nothing. Select, S = soldier k: the click sound; the selected slot = k; the **merc pay panel** (The duty menu,
Open step 1); the **merc's item panel** (below, with the mouse move); wait until the left button is up. (The
original also tests "the press is on the selected merc's face: deselect", but that test is reached only while no
merc is selected, so it never holds.)

While a merc is selected, the item panel covers the face row: the face, duty and stat boxes of the picture do not
answer. **Leave** (Enter or the exit button) with a selected merc S first checks S's duty: when `duty` = 3 and S is
not doctor-able, or `duty` = 5 and S is not repair-able (the picture's step 2b), `duty` = 0.

**The merc's item panel** of S, into the panel buffer (pitch 320, all texts mode 300 unless stated):
1. `INVMAP.PCC` (MAIN.DAT, 320 x 143) at (0, 0); `BODY<n>.PCC` at (159, 27) with n = S +0x17C in decimal (as the
   inventory screen's step 6); when S +0x1B1 ≠ 0, `CAMO<n>.PCC` at (159, 42). Each load sets the **loader result**
   (inventory.md "Drawing the screen (body)" step 1; a missing member draws nothing and gives −1).
2. armour = the armour rating of S (ai.md "armour"): number armour with 3 digits at (175, 53), fg 0xFF, shadow 1,
   font 0; text `%` at (187, 53), not centred, fg 0xFF, shadow 1, font 1.
3. The 28 x 25 face of face buffer S `slot` from column 2, row 2 (pitch 32) to (34, 12), all pixels written.
4. The duty name of S at (72, 12), not centred, fg 0x0E, shadow 5, font 1; the nickname (up to its 0 byte)
   centred at (48, 42), fg 0x0E, shadow 5, font 0.
5. Button 14: element 0x2D (sheet (240, 49), 20 x 16) at (79, 90), then element V (The panel buttons: 0x38 =
   sheet (219, 65), 13 x 9 at the game start; 0x39 = sheet (232, 65), 12 x 8) at (82, 93).
6. The **store tab**: element 0x36 (sheet (78, 132), 13 x 7) when the store view is 0 (records), element 0x37
   (sheet (91, 132), 13 x 7) when it is 1 (counts), at (11, 52).
7. For k = 0..12: unless pocket k of S is missing (inventory.md "Data"), the **item place** k of S
   (inventory.md "Drawing the screen (body)", with the **item places table of the assignment screen**:
   0 (108, 73), 1 (203, 89), 2 (161, 8), 3 (108, 31), 4 (200, 28), 5 (108, 52), 6 (257, 18), 7 (257, 37),
   8 (257, 56), 9 (257, 75), 10 (257, 94), 11 (257, 113), 12 (108, 92); the opening of the assignment screen
   makes this the current table; the ground place 13 (159, 115) is not drawn here).
8. The **store list** (below).
9. Copy the whole panel buffer (200 rows) to the screen; the mouse moves to (50, 26), and the loader result = 0
   (the mouse move gives 0, and the original stores that in the loader result).

**The store list** shows 4 store places j = 0..3 at (xj, yj) = (29, 59 + 19 j) (item places 14..17 of the
table). The **store view** (a game value, 1 at the game start, kept between openings of the screen) selects:
- View 0 (records): the item store holds item records (type, quantity, condition; strategic.md "Store an item");
  the list shows 4 records from the **current record** on (the host keeps the store; the opening does not change
  the current record). First for each j: element 0x32 (sheet (272, 0), 44 x 17) at (xj, yj) and element 0x34
  (sheet (78, 119), 13 x 13) at (12, 61 + 19 j). Then for each j: when there is no record j, the condition bar
  (inventory.md, value 0) at (xj − 11, yj + 4). Else with (t, q, c) = record j: picture of t at (xj + 1, yj + 1)
  with colour 8; when signed c < 10 and (type byte +9 ≠ 0 or byte +7 ≠ 0): `useless` centred at (xj + 22, yj + 7),
  fg 0xED, shadow 1, font 0; else when signed q < 0: `jammed` there (same colours); then the condition bar
  (signed c) at (xj − 11, yj + 4). (Signed c from 110 to 127 makes the bar height negative and the original's
  bar copy runs off memory, as in the inventory screen: a **stop point** (port_api.md "Division by zero"); the
  front-end draws no bar then.)
- View 1 (counts, `stash_counts` per item type 0..135): the **top type** T (a game value; the opening of the
  screen sets it to 0) becomes Fix(T, T) (below). Then v = T and for each j: element 0x32 at (xj, yj); picture of
  type v at (xj + 1, yj + 1) with colour 8; when stash_counts[v] ≠ 0 the number stash_counts[v] with 2 digits at
  (xj − 14, yj + 7), else the text of two spaces there; both not centred, fg 0x27, shadow 0x32, font 0, with the
  background 11 (panel.md "Text with a background"). The **shown type** j = v (a game byte). Then, when v ≠ 0:
  v = the next type u > v (u ≤ 135) with stash_counts[u] ≠ 0, or 0 when there is none. (So after the last
  stored type the places show type 0 with no number.)
- **Fix(T, p)** (p = a wanted top type): L = the first 4 types u ≥ T (ascending, u ≤ 135) with stash_counts[u]
  ≠ 0. When p is not in L: L = the first 4 such types u ≥ p. When L has 4 types: the result is the first of L.
  Else the result is the 4th type counted down from 135 among the types 1..135 with a count (with fewer than 4
  such types: the lowest of them; with none: 0). (Type 0 is in L when it has a count, but the count-down does not
  reach it.)

Evidence (2026-10-24): AssignScreen? 0x4891C at 0x48B86.. (the duty name and face box loop: MouseInRect_
(X, 20, X + 30, 46), 0x5C880 click, the dead equal test, 0x67C54 wait), the Enter path (0x4811C + duty 3 / 5
check of the selected merc), DrawAssignPanel 0x476B8, DrawInvMap? 0x47CF4 (0x532BC loads "invmap.pcc", "body" +
itoa + ".pcc" 0x90BAC / 0x90BB4, "camo" 0x90BBC; ArmorRating 0x55500; 0x35B60(14) = button table 0x9345A entry
14; the tab element table 0x93F0C [view × 2]; DrawItemSlot? 0x4AAD4 unless 0x4A464; 0x51648(0); 0x46BF4;
0x681D0(50, 26)), the item places copy 0x47AE4 (0x94A72 → 0x949F2, 14 places; places 14..17 at 0x94A2A are
fixed), 0x51F80 (view 0, records: type, quantity, condition, next pointer at +3; ItemStateText? 0x51E78), 0x516D8
(Fix), 0x518DC / 0x517DC (view 1; shown types 0x93DD0, top type 0x229C88, the view 0x93EA4, the current record
0x229C7C, "  " 0x90FC4); validate/diff/select_diff.py.

### The panel buttons

While a merc S (team slot k) is selected, a left button that is down is tested in this order (the first that holds
acts; then, unless stated, wait until the left button is up):
1. **Face** of the item panel (34 ≤ x ≤ 62, 12 ≤ y ≤ 36): the click sound; build the assignment picture again
   (it can change duties) and copy all 200 rows of the panel buffer to the screen; the mouse moves to
   (28 + 37 k, 33) and the loader result = 0 (the mouse move gives 0); no merc is selected.
2. **Body** (161 ≤ x ≤ 199, 48 ≤ y ≤ 84): the click sound (with an item on the cursor more happens: see the item
   moves).
3. Else the **panel buttons** 10..14, in this order, each tested at its own turn with the mouse and button state of
   that moment. Button i is pressed when the left button is down and bx ≤ x ≤ bx + 19, by ≤ y ≤ by + 15:

   | button | (bx, by) | overlay element | (dx, dy) | action |
   |---|---|---|---|---|
   | 10 | (8, 25) | 0x19 (sheet (192, 67), 14 x 10) | (4, 3) | previous merc |
   | 11 | (69, 25) | 0x1A (sheet (206, 67), 12 x 10) | (4, 3) | next merc |
   | 12 | (79, 72) | 0x1B (sheet (192, 56), 10 x 11) | (4, 3) | store up |
   | 13 | (79, 108) | 0x1C (sheet (202, 56), 9 x 11) | (4, 3) | store down |
   | 14 | (79, 90) | V | (3, 3) | store view |

   A press: the click sound; on the screen element 0x2C (sheet (220, 49), 20 x 16) at (bx, by) and the overlay
   at (bx + dx − 1, by + dy − 1); wait until the left button is up; for button 14 the store view = 1 − view and
   V = 0x38 when the new view is 1, 0x39 when it is 0 (V is a game byte, 0x38 at the game start; the item panel's
   step 5 draws V too); then on the screen element 0x2D (sheet (240, 49), 20 x 16) at (bx, by) and the overlay at
   (bx + dx, by + dy); then the action. The following buttons are tested after that (the left button is up now, so
   they do not act).
4. Else nothing (the item places and the store list: see the item moves). Wait until the left button is up.

**Previous / next merc**: L = the last slot of the assignment picture's list (set by each build of the picture;
a build with no merc in the list keeps the old value). Previous: j = k − 1; next: j = k + 1. The new slot is j when
0 ≤ j ≤ L, L when j < 0, 0 when j > L. (The new slot is not checked for `inCurSector`.) The new merc S: the merc pay panel (The duty menu,
Open step 1), then the merc's item panel steps 1..9 without the mouse move (the loader result stays the one of the
last picture load of step 1).

**Store up / store down / store view** use the store list (above). With view 0 the store is a chain of records
(record 0 first); R = the current record (none when the store is empty).
- prev(R): R is none or the first record: R. Else the record before R; the top type = the type of that record.
- next(R): R is none: none. Else when the record after R exists and at least 3 more records follow it: that
  record; else R. Then the top type = the type of the result.
- **Store up**, view 0: up to 4 times R = prev(R), stop when R does not change. View 1: with T0 = the top type and
  n = 0, repeat: u = the highest type with 1 ≤ u < T (T = the top type) whose count ≠ 0 (counts[u] is read for
  u = T − 1 down to 1); none: the top = Fix(T, T) and stop; else the top = Fix(u, u), and when it differs from
  the top before this step, n + 1; stop when n ≥ 4, else go on from that top.
- **Store down**, view 0: up to 4 times R = next(R), stop when R does not change. View 1: with T0 = the top type
  and n = 0, repeat: u = the lowest type with T < u ≤ 135 whose count ≠ 0; none: the top = T0, else the top = u;
  then the top = Fix(top, top); when it equals T0: stop; else n + 1, stop when n > 3, else go on from that top.
- After store up / down: view 0: when R changed, the store list (view 0) again; view 1: when the top differs from
  T0, the store list (view 1) again. "Again" = draw it into the panel buffer, then copy the panel buffer part
  x 0..72, y 52..140 to the screen.
- **Store view** (after the toggle): into the panel buffer the store tab (element 0x36 for view 0, 0x37 for view 1)
  at (11, 52) and four times element 0x34 (view 0) or 0x35 (view 1, sheet (91, 119), 13 x 13) at
  (12, 61 + 19 j). View 0: R = the first record whose type is the shown type 0 (none when no record has it), then
  the store list (view 0) again. View 1: the store list (view 1) again.

Evidence (2026-10-24): AssignScreen? 0x4891C at 0x492B9.. (MouseInRect_(0x22, 0xC, 0x3E, 0x24) → 0x5C880,
0x93EA8 = 0, DrawAssignMap?(selected) (0x681D0(0x1C + 37 k, 0x30 − 15)), 0x46BF4, selected = 99; body
MouseInRect_(0xA1, 0x30, 199, 0x54); 0x67CD8(10 + i) for i = 0..4 with the switch; 0x229C20 = the last listed
slot, set in DrawAssignMap? 0x478A4; DrawAssignPanel + DrawInvMap?(S, 0)); 0x67CD8 (button table 0x9345A, area
w − 1 / h − 1 of the pressed element), 0x35D44 (press 0x35DA4, wait 0x6782C, button 14: 0x93EA4 ^= 1 and 0x934ED =
0x38 / 0x39, up 0x35C0C type 1), 0x52098 / 0x520CC (prev / next record, top 0x229C88), 0x516D8 (Fix), 0x519C8 /
0x51F80 (redraw + copy 0x49 x 0x59 at offset 0x4100), 0x4B0B0 (tab table 0x93F0C), 0x52078 (find by type);
validate/diff/assign_panel_diff.py.

### Item moves on the item panel

While a merc S (team slot k) is selected, a left button that is down and that the face and the panel buttons do not
answer (The panel buttons, steps 1 and 3) is an **item press** (entry point `assign_item_press`, docs/spec/port_api.md).
The cursor item, the swap item, the vest owner, shift and ctrl are those of inventory.md "Data". "Wait up" = poll
until the left button is up (at least one poll, inventory.md "The loop"). Draws go to the screen (not the panel
buffer): "draw place p" = the item place p of S (inventory.md "Drawing the screen (body)", with the item places table
of the assignment screen), between hide and show. In order (the first that holds acts):

1. **Body** (161 ≤ x ≤ 199, 48 ≤ y ≤ 84): the click sound. When the cursor type is 0x47 (camouflage kit; the count
   is not checked): the click sound again; camouflage S +0x1B1 + 3 (byte); **the item panel again** (the merc's item
   panel steps 1..9 without the mouse move); inv_extra("hmm.8", ""); the cursor condition − 20 (byte); when it is
   now ≤ 0 (signed): all 12 cursor bytes 0 and inv_extra("empty.8", ""); cursor(cursor type). Then wait up. Done.
2. **Item places**: p = the first place 0..13 of the table whose rectangle (px − 4, py − 4) .. (px + 45, py + 19)
   holds the mouse. With ctrl held and a place found: Unstack the 12 bytes at S + 12 p (inventory.md "Pocket
   move"; for p = 12 and 13 these are soldier bytes +0x90 and +0x9C, as in the inventory screen); when that changes
   them: draw place p. Then no place counts as found.
3. **p = 13** (ground place) with an item on the cursor: the click sound; ask the **discard question**: the question
   box (panel.md "Question box") with the text "Get rid of this item PERMANENTLY?", the picture `GARBAGE.PCC` and the
   picture of the cursor type (item type byte 4) at (26, 18) of the box, colour 0. Yes: when the cursor is a vest
   with pockets (0x32..0x35) and the vest owner < 0x40: the owner's entries 7..11 are cleared (12 bytes 0 each)
   and the vest owner = 0x40; then the cursor is cleared (12 bytes);
   cursor(cursor type). No: nothing. (p = 13 with an empty cursor does nothing.) Then step 5.
4. **p = 0..12**, P = entry p of S:
   - **Reload**: p < 12, P count = 0, 1 ≤ P type ≤ 14, the cursor count (signed) = 1 and the cursor type = the
     ammo type of the gun P (weapons byte 1): P count + the clip size of P (item type byte 5); sfx(0x19 when the P
     type < 8, else 0x1A; 0x2B11, 50, 1); the cursor is cleared (12 bytes); hide, draw place p, for p = 0 also place 12,
     cursor(cursor type), show; when the text flag is on: merc_text(S, "has reloaded.", 800). Step 5 is skipped.
   - Else swap = the cursor (12 bytes); a = **May it go** (inventory.md) with the swap item onto p. a = 0:
     inv_extra("huh.8", ""). a ≤ 0: then step 5. a ≥ 1 (no action points, no give, no AP fields):
     - p = 6 and the cursor type is 0x32..0x35: when the vest owner < 0x40 and ≠ S's slot (+0x17A): exchange the
       entries 7..11 of S and of the owner (12 bytes each). Else: the vest owner = 0x40 when it is < 0x40; when the
       cursor type < the type of entry 6 of S (unsigned): cursor = entry 6, entry 6 = swap (12 bytes each); then for
       each pocket j = 7..11 that is missing now (inventory.md "Data") and whose type ≠ 0: **Store an item** (above,
       "Store an item", with the host op), no store redraw; then the **store list again** (below); the pocket move
       is skipped.
     - p = 6 otherwise: the vest owner = S's slot.
     - p < 12, unless skipped, and pocket p not missing: the **pocket move** of the assignment screen (below).
     - p = 12 with the type of entry 0 (E) ≠ 0: cursor type, count, condition = E +3, +4, +5; cursor +3, +4, +5 and
       +8..+11 = 0. a = 1: E +3, +4, +5 = swap bytes 0..2. a ≥ 2: inv_extra("hmm.8", ""); a = 4: E +3, +4, +5 = 0;
       else cursor bytes 0..2 = 0.
     - Then swap = 0 (12 bytes). Redraw: p = 6: hide, draw place 6, cursor(cursor type), show, vestmap(S), the
       click sound. Else: hide; unless pocket p is missing: draw place p and the click sound; for p = 0 or 12: draw
       places 0 and 12; cursor(cursor type); show. Then when the armour rating of S (ai.md "armour") differs from
       the **armour value** (the armour rating of S taken when S was selected or shown by previous / next merc, and
       only updated here): the armour value = the
       new rating and the item panel again. Step 5 is skipped.
5. **Store** (25 ≤ x ≤ 72, 60 ≤ y ≤ 132), only when step 1 and 4 did not act:
   - An item on the cursor: the click sound. A vest with pockets (0x32..0x35) on the cursor with the vest owner
     < 0x40: Store an item for each of the owner's entries 7..11 (no redraw), the vest owner = 0x40. Then Store an
     item with the cursor; when it was stored (Store an item did not stop at its first test: the cursor type ≠ 0 and
     k + count < 100): cursor(cursor type, now 0 when stored) and the store list again. Wait up. Done.
   - Empty cursor: for store place j = 0..3 (x = 29, y = 59 + 19 j): when x ≤ mouse x ≤ x + 41 and y ≤ mouse y ≤ y + 15:
     the click sound; then
     - store view 0: R = the record j places after the current record (the walk stops when a record has no next
       one: then nothing is taken, go to "cursor" below). The cursor type, count, condition = R. When the cursor
       type ≠ 0: **take** the first record equal to R in type, count and condition.
       **No current record** (an empty store): the original walks from memory address 0, which is not game data
       (the DOS interrupt table), so the result depends on the machine. Seen live in DOSBox-X (2026-10-28, empty
       store, view 0): j = 0 puts a phantom item (type 0x60, count 0xCA, condition 0) on the cursor, the take then
       unlinks the "record" at address 0 and its search for the new current record never ends: the game hangs.
       j = 1 puts type 0xFF (count 0xFF, condition 0xFF) on the cursor, the store count drops to −1 and a byte
       after the 136 stash counts drops by 1. This is a **stop point** (as a division by zero, port_api.md): the
       library makes host call 47 and the press ends with no cursor change and no take.
     - store view 1: t = shown type j; t = 0: nothing; else the cursor type = t (count and condition stay) and
       **take by type** t.
     - "cursor": cursor(cursor type). Wait up. Done.
     No place found: wait up.
6. Else wait up.

**Pocket move of the assignment screen** (p = 0..11, P = entry p): as inventory.md "Pocket move" with these
differences: no click sound in the stack case, no Gave an item, and
- cursor type = P type, its stack class (item type byte 6) > 1, p ≠ 3: shift = exchange; else when the stack class
  < P count (signed) + cursor count (signed): "no room": inv_extra("noroom.8", "No Room") for p = 2..6, else
  no_room(S, p); else Stack join (P += cursor), the cursor is cleared (12 bytes).
- else P count ≤ 1 (signed) or P does not stack: p = 3 with cursor type 0x27 and cursor count > 1: no room
  (inv_extra); else exchange.
- else empty cursor and no shift: cursor = P with count 1 and +8..+11 = 0; Take one from P; P count = 0 then: P is
  cleared (12 bytes).
- else exchange. ("Exchange" = cursor = P, P = swap.)

**Take** (record R exact, or by type t; then the store changed). Exact: the first record equal to R in type, count
and condition; by type: the first record of type t. None: nothing more (the cursor keeps what it got). Else n = 1;
by type with shift held: n = the stack class of t, at most stash_counts[t]. The first record r0: cursor count =
r0 count, cursor condition = r0 condition, cursor +3, +4, +5, +8..+11 = 0. Each further record r_i (i = 1 .. n − 1,
the next record of type t): cursor count + 1, cursor byte 7 + i = r_i condition. Each of the n records leaves the
store. stash_counts[t] − n. Then the **top type** = Fix(top, top) (The assignment picture's store list), the current
record = the first record whose type is the top type (none when there is none), and the store list again.

**The store list again** = the store list (view 0 from the current record, view 1 with its Fix) into the panel
buffer, then its part x 0..72, y 52..140 to the screen.

**The vest map** (host op vestmap, S): the picture `VESTMAP.PCC` goes into the panel buffer at (x7, y7 + 1), where
(x7, y7) = item place 7 of the current item places table ((257, 37) on the assignment screen); its loader result
is kept as for the other pictures of the panel. Then for each pocket j = 7..11 of S that is not missing (inventory.md
"Data"): draw place j into the panel buffer. Then hide, the rectangle (x7, y7) .. (319, 140) of the panel buffer to
the screen, show. (The item press is the only caller here. The right and bottom edges are the routine's register
arguments, which its two callers leave from their mouse-shape call: (319, 140) here, (271, 181) on the inventory
screen, so there the whole pocket 11 is copied.)

**The item store** (kept by the host; the host op stash_put adds a record): the records (type, count, condition)
form a list sorted by type ascending, then condition descending (signed); a new record goes after the records
with the same type and condition. A record of type 0x41 (canteen) gets condition 100 when it is stored (a stored
canteen is full again). The day save holds stash_counts (136 bytes), then the records in list order (3 bytes
each); loading reads, for each type i = 0..135 in turn, as many records as stash_counts[i] shows, and stores each
record as Store an item does (so the list is sorted again, and stash_counts stays the same).

Evidence (2026-10-24): AssignScreen? 0x4891C at 0x492B9.. (body, camo kit 'G') and 0x495BD..0x49EC7 (places:
MouseInRect_(x − 4, y − 4, x + 45, y + 19) over 0x949F2; ctrl 0x53580 + DrawItemSlot? 0x4AAD4(S, p, *0x95678,
320); reload: ammo table 0x94C32, clip 0x94DFD, PlaySfxSlot 0x5CB34(0x19 / 0x1A, 0x2B11, 0x32, 1), MsgMercText
0x15910(0x9A083, 800); MergeItemsPrompt 0x4A838; vest 0x470E8, 0x5193C (AddStashItem?(pocket, 0) for missing
pockets 7..11), 0x51648(1); pocket move 0x538A0, PlayNoRoom? 0x53A34, UseThrow? 0x535E8, PlayInvExtra? 0x25AD8;
attachment, swap clear 0x470B0, redraw (0x674B0, DrawVestMap? 0x48000, 0x5C880), ArmorRating 0x55500 vs the
local set by DrawInvMap?, DrawInvMap?(S, 0); discard 0x278A0("Get rid of this item PERMANENTLY?", "garbage.pcc",
cursor type, 0, 0x1A, 0x12, ...) and item picture 0x46750; store rect MouseInRect_(0x19, 0x3C, 0x48, 0x84),
AddStashItem? 0x51D50(.., 0 / 1), places 14..17 MouseInRect_(x, y, x + 0x29, y + 0xF), take 0x51A24(item, exact,
redraw 1, take 1): shift count 0x284F21, unlink + free_, 0x229C78 head / 0x229C60 tail, 0x516D8, 0x51F80 /
0x519C8), stash_put 0x51CB0 (malloc 7, canteen 100, sorted insert 0x51C44), LoadItemStash 0x51508, SaveItemStash
0x51590; validate/diff/assign_items_diff.py. (The original also keeps a tail pointer that a removal of the only
record leaves on a freed record; when the allocator later gives that address to a record in the middle, removing
that record cuts the list after it, so the items after it vanish from the store (issue #2). Accepted difference
(the maintainer ruling 2026-10-29, issue #19): the port keeps a plain list and never loses an item. Copying the vanish would
need the original's whole memory manager and every file read in the same order; the bug stays listed in #2 for the
fix pass after parity.)

### Key E: all items into the store

While the assignment screen is open and a merc S is selected, the key E (either case; entry point `assign_key_e`,
docs/spec/port_api.md) puts the items of S into the item store. With no merc selected the key does nothing. For
each entry p = 11, 10, .. 0 of S (12 bytes at S + 12 p; entries 12 and 13 are not touched) whose type is not 0:
**Store an item** (above, "Store an item", with the host op; no store redraw) with a copy of the 12 bytes; then
bytes 0..5 of the entry = 0, also when Store an item stored nothing (the type had no room: k + stash_counts[t] ≥
100). Bytes 6..11 keep their values; the vest owner does not change. No click sound, no wait. After the last
entry: **the item panel again** (as in step 1 of "Item moves on the item panel"; it shows the store list too).

An item whose type has no room in the store is lost (original game behaviour; the port does the same).

Evidence (2026-10-24): AssignScreen? 0x4891C at 0x48A41..0x48A89 (GetKey 0x560F4 after 0x5616C, toupper
0x754D7, 'E' = 0x45 and the selected slot 0x229C38 ≠ 99: 0x52150(S, 1)); 0x52150 (entries from S + 0x84 down to
S + 0 in steps of 12, type byte > 0: copy 0x47090 to a stack buffer, AddStashItem? 0x51D50(buffer, 0), bytes 0..5 of
the entry = 0; DL ≠ 0: DrawInvMap? 0x47CF4(S, 0)); validate/diff/assign_items_diff.py ("key" presses).

### Right button on the item panel

While the assignment screen is open, the left button is up and the right button is down (entry point
`assign_right_press`, docs/spec/port_api.md). "Wait right" = poll until the right button is up (at least one poll,
inventory.md "The loop"). "Info" = the host op item_info (inventory.md, op 231: profile, type, count, value, a, b,
info) with the profile = byte +0x179 of S; when it returns ≠ 0: **the item panel again** (as in "Item moves on the
item panel" step 1). With no merc selected: wait right. Else, with S = the selected merc, in order:
1. **Item places**: p = the first place 0..12 (not 13) of the item places table with px ≤ x ≤ px + 41 and py ≤ y ≤
   py + 15 (a smaller rectangle than the left button's). Found: the click sound; info with (type, count, value) =
   (+3, +4, +5) of entry 0 for p = 12 (the attachment; count and value as signed bytes), else (type, count,
   condition) of entry p (count and condition as signed bytes), a = b = 1, info = 1. Then wait right twice. Done.
2. **Store places** j = 0..3 (x = 29, y = 59 + 19 j; x ≤ mouse x ≤ x + 41, y ≤ mouse y ≤ y + 15), the first that
   holds the mouse; no click sound:
   - store view 1: info with (shown type j, 99, 20), a = b = 0, info = 1 (also for shown type 0).
   - store view 0: R = the current record, then j times R = its next record while R has a next one (the walk stays
     on the last record: unlike the left button's walk). Info with (type, count, condition) of R (count and
     condition as signed bytes), a = b = 1, info = 1. (With no current record the original reads its record from
     memory address 0, as for the left button: a stop point; the library makes host call 47 and shows no info.)
   Then wait right once. Done.
3. Else wait right.

Evidence (2026-10-24): AssignScreen? 0x4891C at 0x49ED1..0x4A0C4 (selected slot 0x229C38 = 99 → 0x67C68;
MouseInRect? 0x67C0C(0x22, 0xC, 0x3E, 0x24) result unused; places over 0x949F2 with (x, y, x + 0x29, y + 0xF), 13
of them; 0x5C880; item info 0x25D20 (ItemInfoBox) with EAX = S +0x179, EDX type, EBX count, ECX value
and three stack arguments (1, 1, 1 / view 1: 0, 0, 1 and EBX = 0x63, ECX = 0x14); DrawInvMap? 0x47CF4(S, 0);
0x67C68 = wait right (poll 0x678E8 while [0x229CF4] & 2); store places 14..17 at 0x949F2 + 0x38; view 0x93EA4,
shown types 0x93DD0; 0x52108(current record 0x229C7C, j): walk with the stop at the last record, info with the
profile of soldier [0x229C38]); validate/diff/assign_items_diff.py ("rpress" presses).

### Hover on the assignment screen

Each poll of the click loop with no button down (after the keys) runs this; the lit stat row L (99 = none) is set to
99 when the screen opens and is kept while it is open (selecting or deselecting a merc does not change it). Opening
the item panel (a merc selected) and closing it (the picture again) set the tooltip number T to 0 without a hide
(the new drawing covers the tooltip). Tooltip = panel.md "Tooltip box".

**No merc selected:**
1. Faces: for k = 0..7, x = 12 + 37 k: when Mouse in (x, 20, x + 30, 46) and soldier k has `inCurSector` ≠ 0: show
   tooltip k + 1 at (x + 1, 20, x + 29, 45) with t1 `gear`, t2 and t3 empty, font 0. When no face gave a tooltip:
   hide.
2. The stat rows: row i = 0..9 lies at y_i = 59 + 8 i. When mouse_y < 59 or mouse_y ≥ 137: when L ≠ 99, light 99.
   Else r = the first i of 1..9 with y_i > mouse_y, minus 1 (9 when there is none); when r ≠ L: light r. (The row
   depends on mouse_y only, whatever mouse_x is.)

**A merc selected:** when Mouse in (34, 12, 62, 36) (the item panel's face): show tooltip (selected slot + 1) at
(34, 12, 62, 37) with t1 `exit`, font 0; else hide.

**Light r**: for each team slot k = 0..7 up to the first soldier with `inCurSector` = 0, S = soldier k, X = 27 +
37 × S `slot`:
- When L ≠ 99: the value of stat L of S (as "The assignment picture" step d) as a Number with 3 digits at (X, 59 +
  8 L), fg 237 when L is even, else 41, shadow 0, font 0 (only the number, no shadow pixels at (X + 1, y + 1)).
  Note: a stat that the picture drew in 0xFF (a stat change of the morning) goes back to its normal colour here.
- When r ≠ 99: the value of stat r of S at (X, 59 + 8 r), fg 0xFF, shadow 0, font 0.
Then L = r.

Evidence: AssignScreen? 0x4891C hover part 0x4A0F0..0x4A2C3 (tooltips 0x4A14A, 0x4A2AF), light 0x473BC (table
0x93F10: x, y, fg, colour 7, mode per row), L = 0x229C64 (99 at 0x4894B), T = 0 at 0x4917D (select) and 0x49211
(deselect). Diff-tested: validate/diff/assign_hover_diff.py (scripts of moves, select and deselect presses into
0x4891C; a press uses its poll and the next one, hover runs on every other poll).

## The A.I.M. screen

(Status 2026-10-25: logic and layout read from the asm/pseudocode and checked live in the original from a new game.
The logic (Opening steps 1 and 2, Browsing, Refusal reason, Hire, leaving: remove the pending mercs, pay) is in the
port (port_api.md "A.I.M. entry points"); Browsing, Refusal reason, Hire and the removal are diff-tested
(validate/diff/aim_diff.py --port). Live reference states: `./t jactl load aim_newgame` (day 0, NORMAL, the A.I.M. screen open on
Needle) and `aim_hired` (Jimmy hired, not yet paid).)

The office's hot spot 2 (or 3, see The office) opens it while the A.I.M. **link** (`accounts` +0xA3) is not 0. The
new game sets the link to 1; paying the helicopter charge sets it to 0 (no second visit that day); the day end
(world_load.md section 9 step 0, before the verdict) sets it to 1 again (evidence: DayEndScreen? 0x65BD4, first store at
0x65BF0, the only other writer of the byte besides the new game, main_ and the game-over path of PlayLoop).

**State.** P = the profile on the page (`accounts` +0xA4, kept for the whole game). Mode M (one byte): 0 = can be
asked, 1 = not available (on assignment / on your team / waiting), 2 = "Hire at double salary" offer, 3 = "transfer
$X now" offer, 4 = dead. Interview counts: 60 bytes, all 0 at each opening. View V: 0 = bio text, 1 = gear (a game
byte; the GEAR / BIO button flips it).

**Opening** (in this order):
1. Interview counts = 0. t = random(1000) + 500 (the face-animation eye time te, panel.md "Face animation").
2. Three times: r = random(80); when r < 60 and no team record (slots 0..7 with `inCurSector` ≠ 0) has `profileId`
   r: profile r `teamStatus` (+0x4B) = random(5).
3. Draw the screen (below), then show the page of P' = the next profile from P − 1 going up (Browsing).
4. When the balance < the helicopter charge H: the message "WARNING! You do not have enough funds to authorize
   helicopter transportation." H = 600 / 800 / 1000 for difficulty 0 / 1 / 2 (`accounts` +0x12C).

**Browsing** (from profile q, step d = +1 or −1): q = q + d, wrapping 59 → 0 and −1 → 59, until profile q has
`wisdom` (+0x2E) ≠ 0. P = q; show the page of P.

**The picture.** AIMSCRN.PCX (320 x 200 with its palette) holds the frames and the button faces EXIT (11, 178),
INFO (59, 178), GEAR (108, 178) (37 x 15 each) and the arrows (158, 177) and (198, 177) (elements 0x16 / 0x17, 25 x
18; their labels are not drawn: kind 2 in the button table). Every message box of the screen is the message box of
panel.md with darkening table 30 (frame colour 0x68); every question ("Terminate ...", the helicopter charge) is the
question box of panel.md without a picture (ConfirmDialog), except "Fire <nickname>?" with the merc's face. Kept background
rectangles (redrawn from AIMSCRN.PCX before each change): stats (80, 15) 150 x 59, text panel (4, 92) 228 x 78,
nickname (16, 78) 44 x 10, funds (280, 185) 32 x 8.
- **Funds**: the balance as a 5-digit number (DrawNumber) at x 2, y 0 of a 32 x 7 box (pitch 228), fg 0x28, shadow
  0x30, font 2; the box's rows 2..8 (x 0..31) go to the screen at (280, 185), so the digits start at x 282
  (0x513E4: DrawNumber (…, 2, 0, …), BlitCopy scratch + 0x1C8 → screen + 0xE858; the maintainer play test #29 D).
- **Team faces**, slot k = 0..7: x = 240 for even k, 284 for odd k; y = 21 + 40 (k / 2). The 28 x 25 face of the
  merc at (x, y); its nickname centred at (x + 14, y + 30), fg 0x2A, shadow 0xFE, font 0, only when the
  record's `inCurSector` ≠ 0. A merc hired in this visit (`b19D` ≠ 0) gets the blue overlay element 0x11 over the face.
- **Page of P**: the face (MAIN.DAT member named by P in decimal, top-left 56 x 50) at (9, 21). Then by state:
  `health` (+0x2A) = 0: M = 4, the face darkened, DECEASED.PCC over it. `teamStatus` = 0: M = 0. Else M = 1 and the
  face darkened with the stamp element 0x3C ("ON ASSIGNMENT"). Stats panel: full name (profile +0x0A) at (81, 17),
  fg 0xFF, shadow 0x32, font 2; numbers fg 0x27, shadow 0x32, font 2: salary (+0x28, 5 digits) at (119, 29); health
  (+0x2A), agility, dexterity, wisdom (4 digits) at x 125, y 38 / 47 / 56 / 65; medical, explosives, mechanical,
  marksmanship, level (3 digits) at x 204, y 29 / 38 / 47 / 56 / 65. Nickname centred at (38, 76), fg 0x28, shadow
  0x32, font 2. Text panel: V = 0: the BIOQUOTE.EDT text of P wrapped at (4, 1) in the panel, width 218, fg 0x29,
  shadow 0xFE, font 2, line height − 2 (not for a dead merc); V = 1: the kit items (profile +0x50 .. +0x59, type ≠ 0;
  shown only while P is not on the team, or is a merc hired in this visit), 5 per row: picture at (4 + 44 i, 3 + 38
  row) (colour 0 clear, colour 0x17 = its shadow darkens the pixel under it with remap table 30, other colours
  through table 0: RemapBlit 0x52425 in 0x23468), name = the short item name (JA.EXE table 0x96AEF, 15 bytes per
  item type, space-padded, not encoded; ".38 Revolver") wrapped centred at (x + 20, 22 + 38 row), width 40, fg 0x2A,
  shadow 0xFE, font 0, line height byte 0x949E4 − 1 = 7 (DrawWrappedText? 0x3625C in AimDrawMercItems? 0x2186C;
  the maintainer play test #29 B, C).

**Mouse on P's face** (9..64, 21..70): the tooltip box (panel.md Tooltip box) number 9 at (9, 21, 65, 71), font 1,
texts by M: 0 = "hire" (or "repeat" / "interview" when P's interview count ≠ 0); 1, 4 = none; 2 = "Hire at" /
"double" / "salary"; 3 = "transfer" / "$X" / "now". Mouse elsewhere: the tooltip is hidden.
- **Right button** (the left one up, the mouse on P's face): browse +1; then the screen waits until 200 ms passed
  on the game clock (world_load.md "Game speed") or the right button is up; while it stays down the next pass
  browses again (so about every 200 ms).
- **Left button** (nothing when the balance < 1):
  - M = 1: the message "<nickname> is <text>.", shown 4000 ms or until a click (`teamStatus` 50 and > 50: centred
    as every message box; 0..49, on assignment: the lines start at x 66, not centred): `teamStatus`
    = 50: "waiting to join your team"; > 50: "already on your team"; 2..49: "currently on assignment and is expected
    to return in <n> days" (n = `teamStatus`, in colour 0x1D: the control word 0xB3 c changes the text colour to
    c, 0xB3 alone back); 0, 1: "currently on assignment, but is scheduled to return tomorrow" ("tomorrow" in colour
    0x1D).
  - M = 2: P `b4C` = 100, P salary × 2, **hire** P with quote 10.
  - M = 3: funds −= X (X = 3 × P word +0x3E; AddFunds? 0x514A4 mode 1 counts the shown number down first, in steps of 10 when X / 10 as a signed 16-bit number is 30 or less, else 100, with a 30 ms wait after each step but the last; the helicopter charge counts down in the same way), P +0x3E = 0, then the refusal reason of P: 0 = hire P with quote 10;
    else P says quote 13, P `firedByYou` = 1, show the page again.
  - M = 0: team size (leading run of `inCurSector`) > 7: the message "Your team is complete. If you wish to hire
    another member, you must get rid of s..." (check the full text). Else the **interview**: reason r = refusal
    reason of P (below). r = 0: hire P with quote 10. Else P says quote r − 1 (AIMQUOTE.EDT row 14 P + r − 1;
    "MEMBER" in it = the nickname of: P `b4C` for r = 6, `hatesMerc` +0x3B for r = 7, `b4D` for r = 13). r = 6:
    M = 2. r = 8 and P byte +0x2F = 3: hire P anyway (quote: check). r = 12: when the balance < 3 × P word +0x3E:
    the message "Save your mouse finger - you have insufficient funds."; else M = 3 and X = 3 × that word. Then P's
    interview count + 1.
- "P says quote q": with the Subtitles option the merc-face box with the text only (panel.md Merc-face question box,
  ECX 0: text at (75, 7), fg 0xED, shadow 8, wrap 130, line height − 2); its face (face point (59, 43), the face at
  screen (60, 44)) blinks and talks with the voice in the same wait as below (SpeechWait? 0x28034 (0x3B, 0x2B,
  80 × len(text), 0, 1) in YesNoPrompt? 0x28C84 when byte 0x92EF0 = 0; the maintainer play test #29 A); without it no box: the merc's face on the page (face point (9, 21), mode 0) waits for its speech as panel.md
  "Speech balloon" step 7 and "Face animation" (blinks, the mouth moves with the voice; Enter, Escape, Space or a
  click end it, a click outside the face only after 1000 ms; else the voice's end, without a voice 80 ms per text
  byte); then the plain face. Evidence: quote box 0x28FB8 (Subtitles byte 0x1163BD = 0: face PCX by 0x532BC,
  SpeechWait? 0x28034 with ECX 8, stack 20, EBX = text length x 0x50, CL 0; callers AimScreen 0x20638 / 0x206EF /
  0x207B6 / 0x20902, AimHire 0x220B5). the maintainer play test #20 G.

**Refusal reason** of profile p (first that holds; p's fields): word +0x3E ≠ 0: 12; `firedByYou` +0x3D ≠ 0: 9
(overrides 12); else `b4D` +0x4D < 60: 13; `hatesMerc` +0x3B < 60: 7; `availDay` +0x35 ≤ 100 and > day: 1;
`availDay` > 100 and `availDay` − 100 > success points (`accounts` +0x9A): 1; AimRate(0) ≥ +0x36: 2; AimRate(1) ≥
+0x37: 3; AimRate(2) ≥ +0x39: 5; `maxBadDayStreak` +0x38 ≤ bad-day streak (`accounts` +0x9B): 4; a team record
(0..7) with `inCurSector` ≠ 0 and `profileId` = +0x3C: 8; `b4C` +0x4C < 60: 6; else 0. When reason 13, 7 or 6 named a profile m < 64: m's voice
remark is loaded and m is stored in the byte at 0x22A1F0 (game state). The remark is the sample `NAMES\<p>` member
`<m>.VOC` (m in two digits): it only goes into the VOC buffer (PlayMercVoice 0x24D14 = LoadVocSample, no play).
The refusal quotes 6, 7 and 13 (q = 5, 6, 12) have voices in two halves A and B (speech.md); the speech wait
(SpeechWait? 0x28269..0x2833D) plays half A, then the VOC buffer at rate 0x5622, volume 0x68, then half B at
volume 0x58: the merc says the other merc's name between the halves. AimRate here is the one **without** "one
more" (as in Salary raise): d = the day mod 256, n not increased.

**Hire** of profile p with quote q: k = team size; P says quote q; p `teamStatus` = 50; "A merc joins the team"
steps 2..9 into record k (without the face reload of step 10); record k `b19D` = 1, `dayHired` (+0x1E5) = day + 1;
the page shows the ON ASSIGNMENT stamp, team face k is drawn; M = 1.

**Team face k** (mouse on (x, y) .. (x + 27, y + 24) of a record with `inCurSector` ≠ 0): tooltip number k + 1 at
(x, y, x + 28, y + 25), text "remove" when `b19D` ≠ 0, else "fire", font 0 (check). Right button (held, the left one up): the salary panel
(below). Left button: `b19D` ≠ 0: Firing a merc (FireMerc core: the merc leaves, no grudge) at
(x, y), clear the face, close up the team, show P's page again. Else the merc-face question "Fire <nickname>?"; yes:
the question box and the tooltip go, then **Fire the merc** (S, x, y) of "Firing a merc" (so the quote balloon of
host op 328 shows at the team face (x, y)), clear the face, close up the team, show P's page again. Steps 5 and 8 of
"Firing a merc" (the hire-day salary and non-payment, CalcLedgerAll) do not run here (AimScreen 0x20B95..0x20C1C:
RestoreSaveArea0 0x28698, HideTooltip 0x23420, FireMerc 0x5097C with EBX = the face y, FillRect, TeamCompact? 1).

**The salary panel** of team record k (S = soldier k). Its place: x = 170 when k is even, else 214;
y = 40 × (k / 2) − 13 (integer division), limited to 5 .. 99. The screen part under it is saved. The picture
PANEL_S.PCC (MAIN.DAT, 96 x 99) goes to a buffer P (pitch 228; its palette is not used). Ten numbers are drawn in P
at x 55 as the Number rule (5 digits, left padding '^'), font 2, fg 0x27, shadow 0x32, mode 300:

| y | value |
|---|---|
| 8 | `salary` (u16) |
| 16 | `hp` (signed) |
| 24 | agility |
| 32 | dexterity |
| 40 | wisdom |
| 48 | medical |
| 56 | explosives |
| 64 | mechanical |
| 72 | marksmanship |
| 80 | `level` |

(stats +0x18B .. +0x192 as signed bytes). Then the 96 x 99 pixels of P go to the screen at (x, y), all pixels
written. The panel stays until the right button is up; then the saved screen part comes back. No sound, no game
time. The tooltip of the face (above) is drawn before the panel and stays after it.
Evidence (2026-10-28): ShowSalaryPanel 0x221E8 (asm: x 0xF0 / 0x11C − 0x46 by k mod 2, y (k / 2) × 0x28 − 0xD
clamped 5 .. 0x63; SaveScreenArea 0x467E4 (1, x, y); 0x532BC (MAIN.DAT 0x92F44, palette 0x10AD58,
"panel_s.pcc" 0x90434, buffer 0x93E6C, pitch 0xE4); DrawNumber 0x36090 (v, 5, buffer, 0xE4, 0x37, y, 0x27, 0x32, 2,
300) for +0x110 u16, +0x183 movsx, then dword +0x188 .. +0x18F sar 24 = bytes +0x18B .. +0x192; BlitCopy 0x52AE0
of (s16 0x936C6 + 1) x (s16 0x936C8 + 1) to 0x95678 + y × 320 + x; WaitRightUp 0x67C68; RestoreScreenArea
0x468B0 (1, x, y)); caller AimScreen 0x2011C (mouse buttons bit 1 with bit 0 clear, after the face tooltip).

**Button labels**: EXIT, INFO and GEAR (BIO while the gear view shows) centred at (x + 19, 183) of the button
(x = 11, 59, 108), fg 0x2A, shadow 3, font 1 (the button table's text kind; evidence 0x35AB8, strings 0x966C9; the view
byte 0x93D9F is 1 = bio at the start of the game and is not reset by the screen: each visit opens with the view of
the last visit, bio after the program start).
**Buttons** (left press on the button rectangle): EXIT leaves; INFO = the A.I.M. statistics page (below), then the
screen again; GEAR / BIO flips V and redraws the text panel; the arrows browse −1 / +1.
**Button faces.** The rectangles (button table entries 0..4): EXIT x 11 .. 48, INFO 59 .. 96, GEAR 108 .. 145, each
y 178 .. 193; left arrow x 158 .. 182, right arrow 198 .. 222, each y 177 .. 194. A left press there: the press
sound (sound.md "Button sounds"), then the **pressed face**: for EXIT, INFO and GEAR element 0x13 (sheet (230, 77),
38 x 16, colour 0 not drawn) at (x, y), then the label centred at (x + 18, y + 4), fg 0x2B, shadow 2, font 1 (for
GEAR the label of the view after the flip: BIO when the gear view comes, else GEAR); for the arrows element 0x16
(sheet (268, 65), 25 x 18) at (158, 177) or 0x17 (sheet (293, 65), 25 x 18) at (198, 177). Nothing else happens
until the left button is up (also when the mouse leaves the button; keys typed meanwhile wait). Then the up face
comes back (EXIT, INFO, GEAR: element 0x12 and the label as in Button labels; the arrows: the screen part under the
face as it was before the press), the release sound plays, and then the button acts.
Evidence (2026-10-28): AimScreen 0x2011C calls 0x67CD8 (b, 1) for b = 0..4; 0x67CD8: hit area = entry (x, y) of the
button table 0x9345A (10 bytes: x, y, up element, pressed element, kind, -, label dx, dy) + the pressed element's
w − 1, h − 1 (element table 0x93732: x, y, w − 1, h − 1), left button down; b = 2 flips byte 0x93D9F and copies
"GEAR" 0x99D43 / "BIO" 0x99D5D to the label 0x966D5 first; press sound 0x5C8A0, 0x35D44 (0x35DA4 pressed face: save
area 7, DrawPanelElemAt 0x46920 (pressed element), kind 3 .. 6: label 0x35AB8 (b, 1); wait 0x6782C until the left
button is up; 0x35C0C up face: kind 5 / 6 element + label 0x35AB8 (b, 0), kind 2 restore area 7), release sound
0x5C900. Table: EXIT (11, 178, 0x12, 0x13, kind 6), INFO (59, 178, 0x12, 0x13, 6), GEAR (108, 178, 0x12, 0x13, 5),
arrows (158, 177, -, 0x16, 2), (198, 177, -, 0x17, 2). Diff-tested: salary_diff.py (0x35DA4 for b = 0..4).
**Keys**: Escape = EXIT; Left, Up, PgUp = browse −1; Right, Down, PgDn = browse +1; L = say again; T = the
Subtitles option; V = the version message; Alt-X = the quit question.

**Key T** (the Subtitles switch): the Subtitles option (Control Panel switch 5) flips (0 ↔ 1), then the message box
(as V) shows "Text  on" when it is now 1, else "Text  off" (the word "Text ", then " on" / " off": two spaces).
The same switch rule serves other screens with other words (ToggleMessage 0x3F994: switch byte, word).

**Key Alt-X** (the quit question, "Exit to DOS" with argument 0, panel.md "Exit to DOS"): the question box
"Exit to DOS?" with the picture `Q_MARK.PCC` in the A.I.M. box frame (PANEL_B.PCC). No: the screen goes on. Yes:
no Quick Save; when `QUICK.SAV` does not exist, `OFFICE.SAV`, `DAY.SAV` (and `QUICK.SAV`) are deleted; then the
program ends with a quit line (panel.md "Exit to DOS" step 4).
Evidence (2026-10-28, asm): AimScreen 0x201FB GetKey, extended 0x2D → 0x2026E ExitToDosPrompt 0x38084(0); key
0x754D7 toupper 'T' → 0x202B9 0x3F994(EAX 0x9ADD1 "Text ", EDX 0x1163BD Subtitles); 0x3F994: byte [EDX] xor 1, copy
of the word, append 0x9A04F " on" (byte ≠ 0) or 0x9A069 " off", StatusMessage? 0x27D24.

**Fades** (ending.md op 24 / 25 rule, 24 steps each, one level per vertical retrace): drawing the screen (Opening
step 3, and again after the INFO page) ends with a fade in (0 → 256) of the AIMSCRN.PCX palette; the INFO button
fades out first (the statistics page below); leaving fades out (256 → 0) after the end of the A.I.M. music starts,
then the music stops. Evidence (2026-10-27, asm): ShowAimScreen 0x1FEB0 ends with fade 0x52908 (slot 0xEDA08, 0,
24, 0, 256) at 0x2004C; AimScreen 0x20E76 (24, 256, 0) before AimStatsScreen? 0x224C4, 0x2112A (24, 256, 0) after
0x5C858 and before StopMusic 0x5C82C.

**Leaving.** n = the number of records with `b19D` = 1.
- n = 0: the question "Terminate communication with A.I.M.?"; yes = leave, no = stay.
- balance < H: the message "You can't afford the helicopter charge...", then the terminate question; yes = remove
  every merc with `b19D` = 1 (FireMerc core, close up; 500 ms each) and leave.
- else the question box "There is a $H helicopter charge. Authorize payment?" with the helicopter animation (panel.md "The travel animation": rows 0 / 1;
  the payment below comes after the yes row ends). No:
  the terminate question as above. Yes: new arrivals (`accounts` +0x8C) = n; funds −= H (counted down); link = 0;
  ("On the way. E.T.A.: Daybreak..." is the yes row's text, before the payment); for each record with `b19D` = 1: `b19D` = 0, its profile
  `teamStatus` = 100, A.I.M. hirings (`accounts` +0x9D) + 1. Leave.
Checked live 2026-10-25 (new game, NORMAL): Jimmy (5) hired; after OK: funds 20000 → 19200, link 0, arrivals 1,
`b19D` 0, `teamStatus` 100, hirings 1.

Original bug: before an interview the screen means to warn "WARNING! If you interview this person, you will not have
enough funds to authorize helicopter transportation." when the balance is below H, but its test (balance < H and
H < balance) is never true, so the warning never shows (0x204E0: jle and jge to the same place). The port does not
show it either.

Port notes: Hire with a full team (8 records) writes record 8 in the original; the screen tests the team size first,
so the port refuses it (result −1). Browsing with no profile that has `wisdom` ≠ 0 loops forever in the original;
the port stops after 60 steps (−1). The removal loop stops after 64 passes.

The face blinks while the mouse is off the face: panel.md "Face animation", A.I.M. screen.
The INFO page is "The A.I.M. statistics page", the key L is panel.md "Say it again (key L)", the music is music.md.
**The gear view item click.** In the gear view (V = gear), each pass of the screen with the mouse off the face
rect and the left or the right button held (held, not only pressed): the kit items of P (profile +0x50, 10 places)
that are not 0 are counted n = 0, 1, ... in order; item n has the rect (4 + 44 (n mod 5), 95 + 38 (n / 5)) ..
(+42, +36). The first item whose rect holds the mouse: eye frame 0 of the face goes to the screen at (14, 26)
(panel.md "Face animation"); when P is on the team (a record with `inCurSector` ≠ 0 and profile P) and no such
record has `b19D` ≠ 0, nothing more. Else the item info box (panel.md "Item info box") of profile P, item t, count
n = the clip byte (item table byte 5, signed) when the stack class (byte 6) = 1, else 1; value 20, numbers flag 0,
voice flag 0, mouse-back flag 1. Its result is not used: a picture item stays on the screen until the page is
drawn again. Evidence (asm 2026-10-28): AimScreen 0x20CF2 .. 0x20E2B (view byte 0x93D9F = 0, buttons 0x229CF4 bit 1
or 0, MouseInRect? 0x67C0C (x, y, x + 0x2A, y + 0x24), FaceEyeFrame 0x27D64 (0, 1), item table 0x94DF8 + 12 t,
TeamHasProfile? 0x50AC0 then 0x50B04, 0x25D20 (P, t, n, 20; pushes 1, 0, 0)); the branch is reached only when the
mouse is off the face rect (0x202FF). ORIGINAL BUG (#2 row 65): the eye frame goes to the screen also when the face is
dark (DECEASED, or the stamp 0x3C of a merc on the team or on assignment), so the open eyes show as a light band over
the stamp; port fix P23 draws no eye frame on a dark face.

Evidence (2026-10-25): AimScreen 0x2011C (interview table copy 0x1FDB0, opening randoms, key table, face rect
0x67C0C(9, 0x15, 0x40, 0x46), tooltip 0x23264 texts 0x9A57D.., team loop, mode switch byte 0x95F6C, exit block),
ShowAimScreen 0x1FEB0 (save areas 2 (0x50, 0xF), 3 (4, 0x5C), 5 (0x10, 0x4E), 4 (0x118, 0xB9) of table 0x936BA;
funds 0x513E4; team 0x21B3C → 0x21198), browse 0x221B0, page 0x21648 (stamp 0x3C, DECEASED.PCC, 0x212D8 stats,
0x215BC nickname + text panel to screen + 0x7304), text panel 0x214E8 / gear 0x2186C, hire 0x22014 (0x21B74, b19D,
dayHired), MercRefusalReason 0x219CC, AimAlreadyHiredMsg? 0x2306C (ShowMessage? 0x27B98 with x 0x42, 4000 ms),
quote box 0x28FB8 (YesNoPrompt? 0x28C84 ECX 0 or SpeechWait? 0x28034), cancel pending 0x20060, button table
0x93458 entries 0..4, helicopter charge i16 table 0x95C2A, WrapText? 0x362AC control word 0xB3 (0x364EF).

### The A.I.M. statistics page

The INFO button. The mouse pointer hides, the A.I.M. palette fades out (24 steps, 256 to 0; ending.md op 25 rule),
then:
1. The picture `statbgnd.pcx` of MAIN.DAT fills the screen (320 x 200) with its own palette (the last 768 bytes
   of the member, each value >> 2).
2. Values (`accounts` offsets; u8 unless named): success rate, fatality rate, turnover rate and non-payment rate are
   0, except when hirings (+0x9D) ≠ 0 and the day (+0x04, i32) > 0: success rate = (success points (+0x9A) × 100)
   div (day × 2) (signed 32-bit, truncated); the three others = AIM rate kinds 0, 1, 2 **without** "one more"
   (d = the day mod 256, n not increased; Salary raise section).
3. Team: for k = 0..7, stop at the first record with `inCurSector` (+0x182) = 0; n = the records counted. Sums over
   them: signed bytes `hpMax` (+0x184), `agility` (+0x18B), `marksmanship` (+0x191), `wisdom` (+0x18D); salary
   (u16 +0x110): S = the sum, M = the sum of (salary + 1) div 2. When n > 0 the four averages and the average
   salary are the sums div n (signed, truncated); else all are 0.
4. The accounts are recalculated (CalcLedgerAll, as in the team ledger).
5. Labels: font 1, fg 0x29, shadow 7, not centred (panel.md "Text"). Numbers: the value in decimal (a minus sign
   when negative), left padded with '^' to D characters, font 1, fg 0x1D, shadow 9, not centred.

| x, y (label) | Label | Number (x 125, D = 4) |
|---|---|---|
| 9, 33 | Completed Days | day (+0x04, i32) |
| 9, 43 | Mercenary Sorties | sorties (+0x9F) |
| 9, 53 | Success Points | success points (+0x9A) |
| 9, 73 | # Hirings | hirings (+0x9D) |
| 9, 83 | # Fatalities | mercs killed (+0x00, i32) |
| 9, 93 | # Terminations | terminations (+0x9C) |
| 9, 103 | # Resignations | resignations (+0xA0) |
| 9, 113 | # Non-Payments | non-payments (+0xA1) |
| 9, 133 | Success Rate | success rate |
| 9, 143 | Fatality Rate | fatality rate |
| 9, 153 | Turnover Rate | turnover rate |
| 9, 163 | Non-Payment Rate | non-payment rate |

| x, y (label) | Label | Number (x 274, D = 6) |
|---|---|---|
| 171, 33 | Current Team Size | n |
| 171, 43 | Avg. Health | average hpMax |
| 171, 53 | Avg. Agility | average agility |
| 171, 63 | Avg. Marksmanship | average marksmanship |
| 171, 73 | Avg. Wisdom | average wisdom |
| 171, 93 | Avg. Salary | average salary |
| 171, 103 | Minimum Team Cost | M |
| 171, 113 | Maximum Team Cost | S |
| 171, 133 | Current Balance | balance (+0x24, i32) |
| 171, 143 | Estimated Costs | day expenses (+0x64, i32) |
| 171, 153 | Estimated Income | day income (+0x20, i32) |
| 171, 163 | Estimated Balance | balance after the day (+0x28, i32) |

   Then "Overall Rating:" at (107, 185); the performance rating score s (ending.md "The performance rating", 0..100;
   it recalculates the accounts again) as a number with D = 3 at (195, 185); "%" at (213, 185) with the number's
   colours.
6. The palette fades in (24 steps, 0 to 256); the pointer shows.
7. Wait: each poll reads the mouse, then the waiting keys one by one: a key byte 13 (Enter) ends the wait (other keys
   are dropped; an extended key gives 0 and then its scan code). Else the wait ends when the left button is down
   (held, not only a new press).
8. Wait until the left button and the right button are up; the pointer hides; the palette fades out (24 steps, 256 to
   0). The A.I.M. screen is drawn again (with the fade in of its opening).

Evidence (2026-10-27): AimStatsScreen? 0x224C4 (asm: sar 0x18 on the four stat dwords = signed bytes, salary
movzx-like into a cleared EBX, idiv), called from AimScreen 0x20E7B after 0x67BF0 (pointer off) and fade 0x52908
(0xEDA08, 0, 24, 256, 0); then ShowAimScreen 0x1FEB0 (0). AimRate 0x1FDEC with DL = 0; PerformanceRating 0x62EB4
(low byte); DrawNumber 0x36090, DrawText 0x358D0 mode 300; strings 0x9A681.. (26 bytes apart), 0x9A98D, "%" 0x90450.

## The guard screen

Key M on the tactical screen opens the **guard screen**: the island map with the GUARDPAN.PCC panel. The player
takes guards from one owned sector and sends them to other sectors (or puts them back). When the screen closes, the
guards set off; each one reaches its new sector later in the day (turns.md, The day clock, step 5).

The screen keeps its own state (`Ja1GuardScreen`, below). Moves are kept per **destination** sector d: `moves_n[d]`
entries (signed bytes, 0..8) in `moves[8 d + j]`, each the source sector of one guard (99 = none). `pick` = the
sector the picked guards come from (99 = none), `pick_n` = the number of picked guards ("in the hand"). Sector bytes
are `sectorState` fields (`guards` = byte 4, `b5` = byte 5: guards on the way to the sector); adds and subtracts on
them are 8-bit with wrap-around. **Distance**(a, b) = |a mod 10 − b mod 10| + |a div 10 − b div 10| (sector cells).

Messages (the front-end shows the text on the status line; numbers continue the table of Sector click on the island
map): 1 "This sector is in enemy hands!", 2 "This sector is not linked!", 18 "Can't mix sectors!", 19 "Not enough
time!", 20 "There are Guards without destinations!".

**Totals** (the panel numbers): `here` = the sum of `guards` over the 60 sectors; `moving` = the sum of `b5` +
`moves_n[s]` (signed) over the 60 sectors; `all` = `here` + `moving` (all 32-bit).

**Open** (`ja1_guard_open`): `pick_n` = 0, `pick` = 99, `travel` = 30, `moves` all 99, `moves_n` all 0, Totals,
`hover` = `last` = 99, `changed` = 0. Result 1 when `cur_sector` ≥ 90 (the screen does not open: the original
skips its loop and Close), else 0. (The original first puts the mouse on the cell of `cur_sector`, see Picture,
and opens only when that point is on the screen, y ≤ 199: so `cur_sector` 60..89 opens with the mouse below the map.)

**Each pass** of the screen (the front-end polls mouse and keys, then in this order):
1. The exit key (Enter) was pressed: **Exit test**.
2. When the mouse is in the map part (mouse y < 141): s = the sector under the mouse (Mouse over the island map,
   the same formula). **Hover** (s). Then, when the left or the right button is down: **Click** (s); the front-end
   waits until the buttons are up.
3. Else: **Hover** (60). Then when the start button (button 7 of the button table: 281..306 × 176..191, the
   start button of the strategic screen) is clicked: **Exit test**.
4. On the first loop pass (the screen does not start the timer 0x22A3FC; live tac60: the timer stays old up to the first rotation), then when more than 500 ms passed, the palette colours 0xD7..0xDA and 0xDB..0xDE rotate (front-end): in each range, entries first + 1
   .. last move down by one and the first entry goes to the last place.

**Exit test** (`ja1_guard_exit`): `pick_n` = 0: result 0 = the screen closes (**Close**) at the end of this pass;
the rest of the pass still runs (so a right click in the same pass can take a guard into the hand, and that guard
is lost: keep it). Else message 20, the screen stays.

**Hover** (`ja1_guard_hover`, s = 0..59, or 60 = the mouse below the map):
- s = 60: when `hover` < 60: the frame of `last` gets colour 0x6E, the sector number is cleared ("  " drawn at
  (290, 165), the place of number field 15; front-end), `last` = 60, `hover` = 60. Result 0.
- Else `hover` = s. When s = `last`: when `changed` ≠ 0 and `pick_n` ≠ 0: when s = `pick`: `travel` = 0,
  `changed` = 0; else when s is connected to home (Connected to home): `travel` = 30 × Distance(s, `pick`),
  `changed` = 0 and result 1 = the front-end shows the travel time (else `changed` stays 1). Result 0 otherwise.
- When s ≠ `last`: `changed` = 1; the frames as in Mouse over the island map (the old sector `last` when < 60:
  0x6E; s: 0xFF when owned, else 0x6B; the travel text is cleared: "   " at (219, 185)); when `pick_n` ≠ 0 the
  front-end draws the picked number on s; `last` = s. Result 0.
- In every pass with the mouse on the map the front-end also shows s + 1 as number field 15 (the map panel).
  (So `travel` follows the mouse one pass late: a click in the first pass on a new sector uses the old value.
  Keep it.)

The **travel time** text: hours "." tenths, h = `travel` div 60, t = (`travel` mod 60) × 100 div 600, printed as
"%d.%1d" at (219, 185) (C division). The travel text, its "   " clear and the "  " clear of number field 15 are
DrawText with fg 29, shadow 50, font 0, background 13; number field 15 itself is the map panel's (2 digits, the same
colours).

**Click** (`ja1_guard_click`, s = 0..59; buttons bit 0 = left, bit 1 = right; ctrl = the Ctrl key is held; the
result is a message number, 0 = none):
1. s not connected to home: message 1 when `flags` bit 0 of s is clear, else message 2. Nothing else.
2. **Left** (bit 0 set; also with bit 1): n = `moves_n[s]` + `b5` + `guards` of s. When n < 8 and `pick_n` ≠ 0:
   - when `day_minutes` + `travel` > 719 (signed 32-bit): message 19, nothing else;
   - else when s = `pick`: `guards` of s + 1 (the guard goes back); else `moves[8 s + moves_n[s]]` = `pick`,
     `moves_n[s]` + 1;
   - `pick_n` − 1; when it is 0: `pick` = 99. Totals.
   (n ≥ 8 or nothing in the hand: nothing.)
3. **Right**, Ctrl not held: when `pick` ≠ 99 and `pick` ≠ s: message 18. Else when `guards` of s ≠ 0: `pick` =
   s, `pick_n` + 1, `pick_no` = s + 1, `guards` of s − 1, Totals. (No guards there: nothing.)
4. **Right** with Ctrl: when `moves_n[s]` ≠ 0: **Undo** (s), Totals.

**Undo** (d): `moves_n[d]` − 1; src = `moves[8 d + moves_n[d]]`; `guards` of src + 1. When `b5` + `guards` +
`moves_n` of src > 8 (signed sum): Undo (src). (A guard that goes back home can push out a guard that was sent to
his home; the moves table is not cleared.) The front-end redraws the cell of src.

After a click that changed the state, the front-end redraws the cell and markers of s (The island map picture with
marks = `moves_n`), the number `pick_n` at the cursor, and the panel numbers.

**Close** (`ja1_guard_close`): left[0..59] = arrive[0..59] = 0 (8-bit counters).
1. For each destination s = 0..59 and j = 0 .. `moves_n[s]` − 1: src = `moves[8 s + j]`, e = 8 s + j:
   `guard_eta[e]` = 30 × Distance(s, src) + `day_minutes` (16-bit); `guard_rec[e]` = **Side** (s, src); `b5` of s
   + 1; left[src] + 1; arrive[s] + 1.
   Note (a bug of the original, keep it): entries are written from j = 0, so guards still on the way to s from an
   earlier guard screen of the same day lose their entry: they never arrive and `b5` of s stays raised until the
   day ends.
2. For s = 0..59: when left[s] ≠ 0: host op 89 replay_event (s, left[s], 0x5B); then when s = `cur_sector`,
   left[s] times: the first soldier record k from 15 down to 8 with +0x182 ≠ 0, `hp` (+0x183) ≠ 0 and +0x188 = 0
   leaves: Forget the soldier (k), then Merc dies (k, quiet 0) (shooting.md); when no record is found, stop. Then
   when arrive[s] ≠ 0: host op 89 replay_event (s, arrive[s], 0x5C).
3. `moves` all 99, `moves_n` all 0.
The front-end then restores the tactical screen (the original clears the screen, loads EXPLODE.PCX and
SMOKBOMB.PCC again, then the tactical palette, strips and view).

**Side** (d, src) = the side of d that the guards come in from: dx = d mod 10 − src mod 10, dy = d div 10 − src div
10. When |dx| < |dy|: 1 when dy < 0, else 5. Else 7 when dx < 0, else 3.

**At the day end** (after the day replay, before the day's verdict): `b5` of every sector = 0. Guards still on the
way are lost.

**Picture** (front-end): the island map (The island map picture, marks = `moves_n`), MAIN.DAT `GUARDPAN.PCC` at
screen (0, 146), then the panel numbers as number fields with 3 digits, fg 50, shadow 12, background 13: `here`
at (112, 163), `moving` at (112, 174), `all` at (112, 185), `pick_n` at (220, 163). The mouse is put at
((cur mod 10) × 31 + 3, (cur div 10) × 23 + 3) for cur = `cur_sector`. The **picked number** on sector s: element
0x75 (panel sheet) at (x0, y0) with x0 = (s mod 10) × 31 + 20, y0 = (s div 10) × 23 + 10, then `pick_n` as a
1-digit Number at (x0 + 7, y0 + 8), fg 1, shadow 0, mode 300 (no background). It is drawn when the mouse moves
onto s with `pick_n` ≠ 0, and after a click on s when `pick_n` ≠ 0 (after the cell redraw).

```c
typedef struct Ja1GuardScreen {
    uint8_t moves[480];      /* 60 x 8: the source sector of each guard on its way to sector d (99 = none) */
    int8_t  moves_n[60];     /* entries per destination sector */
    int32_t pick;            /* the sector of the guards in the hand, 99 = none */
    int32_t pick_n;          /* guards in the hand */
    int32_t travel;          /* minutes from pick to the sector under the mouse */
    int32_t pick_no;         /* pick + 1 (the last right click) */
    int32_t here, moving, all;   /* Totals */
    int32_t hover, last;     /* the sector under the mouse / of the last frame (60 = below the map, 99 = none) */
    uint8_t changed;         /* the mouse moved onto a new sector and travel is not set yet */
} Ja1GuardScreen;            /* 580 bytes */

int32_t ja1_guard_open(Ja1World *w, const Ja1Host *h, Ja1GuardScreen *g);           /* 0 = open, 1 = not */
int32_t ja1_guard_hover(Ja1World *w, const Ja1Host *h, Ja1GuardScreen *g, int32_t sector); /* 1 = show travel */
int32_t ja1_guard_click(Ja1World *w, const Ja1Host *h, Ja1GuardScreen *g, int32_t sector, int32_t buttons,
                        int32_t ctrl);                                             /* message number */
int32_t ja1_guard_exit(Ja1World *w, const Ja1Host *h, Ja1GuardScreen *g);           /* 0 = close, 20 = message */
void    ja1_guard_close(Ja1World *w, const Ja1Host *h, Ja1GuardScreen *g);
```

Evidence (2026-10-24): GuardAssignScreen? 0x427B4 (called by 0x1328C from TacticalKeyHandler key M; state 0x22A11D
pick_n, 0x22A121 pick, 0x22A119 travel, 0x22A125 pick_no, moves 0x10D680, moves_n 0x229D8C; the loop 0x42940..;
exit test after MapKeyHandler? 0x426D0 and 0x67CD8(7, 1); left 0x42E3A.., right 0x42D50.., Ctrl undo 0x4257C;
travel 0x41104 (Distance) × 30 and sprintf "%d.%1d"; close 0x42FBC..0x431B3: 0x41104, Side 0x30570, eta 0x10D2B8,
rec 0x10D0D8, b5 0x2297A1, replay 0x5B / 0x5C, ForgetSoldier? 0x3A148, MercDies? 0x3A4A4); Totals 0x42664 (0x22A10D,
0x22A111, 0x22A115; number fields 54..57 of the table 0x93FD8); cell redraw 0x42500 (0x41640, 0x419BC, number
0x424BC: element 0x75 + DrawNumber); the b5 clear at the day end: DayEndScreen? 0x65BD4 (after DayReplay, before
DayEndVerdict). Harness: validate/diff/guard_diff.py.

## Enemy moves

The day clock (turns.md, The day clock, step 4) asks the enemy commander for a sector (op 310 enemy_pick) and then
moves enemy troops out of it (op 311 enemy_move). With the library op (the host leaves bit 63 of `host_ops` clear)
the library does both as below; a host that answers the ops itself keeps the old behaviour.

World fields used: `sector_state` (9 bytes per sector: byte 0 flags, 1 enemies, 2 mercs, 3 tappers, 4 guards, 8
unhealthy), `sectors` (0xB1 bytes per sector: +0x96 the 8 enemy slots, signed bytes: > 0 an enemy is there, < 0 a
free slot; +0xAA), `sector_order` (61 bytes), `santino_word` (signed 32-bit: the enemies Santino can still send to
sector 0), `guard_skill`, `supplies` (byte 0), `story` (bytes 6, 9, 0x0B, 0x0D), `splash` (60 signed bytes),
`time_compress`. New world field: `enemy_counter` (byte; the day save's byte 20969, the original's 0x22A0BD).
Sector bytes are unsigned unless marked; adds and subtracts on them are 8-bit with wrap-around. s8 = the byte as a
signed number.

New host op 329 enemy_walk_in (int slot, int edge): an enemy (soldier slot 0x28..0x2F) walks into the current
sector's tactical map at the edge `edge` (3, 5, 1, 7 by the direction of the move, 9 for Santino's troops); result
(byte) ≠ 0 = he came in, 0 = he could not (a library op: Walk-ins below). Existing ops used: 89 replay_event, 218 resolve_battle, 9 sector_value (ai.md Sector value),
and Time compression (0) (turns.md).

**Story sector test** (sector n): story byte 9 = 2, story byte 6 = 2, s8 story byte 0x0B < 1 and n is 0x11 or
0x1B. (The same test as in ai.md Sector value.)

**Defence weights**: merc weight = guard_skill / 2 + 50, guard weight = guard_skill / 2 + 40 (truncated).
**Attack weight** (sector n, flag f): a = supplies[0] / 2 + 60 (truncated). If the story sector test holds for n,
or n = 0x35 and s8 story byte 0x0D = 1: 4 a. Else a if f ≠ 0, else a / 2 (truncated).

**Take an enemy** (sector s): the highest k of 7..0 whose enemy slot `sectors[s] + 0x96 + k` is > 0 (signed): the
slot becomes its negative (8-bit), enemies of s − 1; result 1. No such slot: result 0.

**Put an enemy** (sector s, edge): the lowest k of 0..7 whose slot is < 0 (signed). No such slot: result 0. If mercs
of s ≠ 0: op 329 enemy_walk_in (k + 0x28, edge); a low byte 0 ends with result 0. Then the slot becomes its negative
(8-bit), enemies of s + 1; result 1. (A free slot holds 0xFF or 0xFE; it becomes 1 or 2.)

**Enemy pick** (op 310; result = a sector): enemy_counter = enemy_counter + 1 (8-bit). If it is above 60: the
shuffle, then enemy_counter = 0. Result sector_order[enemy_counter].
The **shuffle**: for i = 60 down to 1: j = Random(i) for i > 1, j = 0 for i = 1 (low byte); swap sector_order[i] and
sector_order[j].

**Enemy move** (op 311, byte src; result = an 8-bit count of enemies who came into a sector with mercs, start 0):
1. If bit 0 of the flags of src is set (the player owns it): result 0, done.
2. If mercs of src = 0:
   a. keep = for src = 0: 8 − santino_word (low byte) if santino_word < 8 (signed), else 0; for another sector: 3 if
      `sectors[src] + 0xAA` = 0, else 1. surplus = s8 (enemies of src − keep). If surplus ≤ 0: go to step 3.
   b. v0 = sector_value (src, 1) as a signed 16-bit number.
   c. **A pass**: best = −100, last = −100, no choice. For d = 0..3 in order (offset 1, 10, −10, −1; bonus 15, 10,
      −10, −15; wall bit 4, 8, 2, 0x10; edge 3, 5, 1, 7), skip d when the neighbour is off the map (d = 0: src mod
      10 = 9; d = 1: src > 49; d = 2: src < 10; d = 3: src mod 10 = 0); else nb = src + offset:
      - room = s8 (8 − enemies of nb). Skip d if room < 1.
      - f = 1 if the wall bit of d is clear in the flags of **src**, or the story sector test holds for nb; else 0.
      - sv = sector_value (nb, f) as signed 16-bit. Skip d if sv = −100. val = sv + bonus (signed 16-bit).
      - cnt = the smaller of room and surplus.
      - **Attack** (bit 0 of the flags of nb set, or mercs of nb ≠ 0): skip d if surplus < 2. cnt = at most 7.
        def = (mercs of nb × merc weight + guards of nb × guard weight + 2 × tappers of nb), low 16 bits signed. If
        def = 0: n = Share (enemies of src); skip d if n < 2; cnt = the smaller of cnt and n. Then att = (attack
        weight (src, f) × cnt + attack weight (nb, 1) × enemies of nb), low 16 bits signed. score = (val / v0) ×
        (att × 100 / (def + att)), see Arithmetic. If def + att = 0: skip d when att = 0 or the signs of att and v0
        differ; else score = +infinity (it is taken below and best becomes 0).
      - **Fill** (else): n = Share (enemies of nb + enemies of src); n = 8 if n > 8; n = s8 (n − enemies of nb); skip
        d if n < 1; cnt = the smaller of cnt and n. score = (cnt − 1) × 10 + 50.
      - Skip d if score < 50 or score ≤ best. Else last = best, best = score truncated toward zero (low 16 bits,
        signed), the choice = (nb, f, cnt, d).
   d. If best > −100 (a choice was made): moved = 0. cnt times: if f = 0: the troops sneak through the wall only
      when Random(6) < Random(s8 splash[src]) and then Random(6) < Random(s8 splash[nb]) (the second pair is drawn
      only after the first one is true); when not: Random(100); if that is < 75 the soldier stays (op 89
      replay_event (src, src, 0x54)) and the next soldier follows. A soldier who goes: Take an enemy (src); if that
      gives 1: Put an enemy (nb, edge of d); if that gives 1: moved + 1.
      If moved > 0: op 89 (src, nb, moved + 100). If mercs of nb ≠ 0 and time_compress ≠ 0: Time compression (0).
      If bit 0 of the flags of nb is clear and mercs of nb = 0: op 89 (src, nb, 0x4B). Else op 89 (src, nb, 0x4C),
      and then if mercs of nb = 0: op 218 resolve_battle (nb, f) (a battle with the guards), else result + moved.
      Then surplus = s8 (surplus − cnt) (cnt of the choice, also for soldiers who did not go).
   e. Another pass (step c) only if last > −100 and surplus > keep. (Quirk of the original: last is the best value
      *before* the final choice of the pass, so a pass whose first improvement was also its last ends the moves.
      Troops leave a sector in a second group only when two directions of the pass improved the score one after
      the other.)
3. **Santino** (src = 0 only): n = s8 (8 − enemies of sector 0); if mercs of sector 0 ≠ 0 and n > 2: n = 2. If n >
   santino_word (signed): n = the low byte of santino_word (signed). If n ≠ 0: if n > 0, n times: Put an enemy (0, 9);
   on 1: santino_word − 1. Then if mercs of sector 0 ≠ 0: Time compression (0) when time_compress ≠ 0, result + n
   (n, not the number placed). op 89 (0, 0, s8 (n + 100)); op 89 (0, 0, 0x57).
   In player terms: Santino's sector refills. Each enemy move of sector 0 brings it back up to 8 enemies (at most 2
   at a time while mercs are there), out of a reserve of 100 / 200 / 300 by difficulty (new game, world_load.md
   table 0x68), until Santino speaks (ai.md) or a merc reaches tile 0x279 of sector 0 (soldier_update.md); then the
   reserve is 0. (Evidence: NewGameInit 0x37A0C, 0x3EDE8 step 3, AiThink 0x6D1A8, MercView 0x1491C.)
4. Result.

**Share** (tot): x = tot × (val / (val + v0)) + 0.5, truncated toward zero; the low byte as s8. When val + v0 = 0
the result is 0.

**Arithmetic.** Each step of a share or of the attack score is rounded to 64 bits of mantissa (x87 extended
precision, round to nearest even); the attack score is then rounded to a 32-bit float (24-bit mantissa) before the
compares; the truncations go toward zero. The fill score is an exact integer.

Original: enemy_pick 0x3F45C, shuffle 0x3ED94, enemy_move 0x3EDE8 (local tables 0x383D0..0x383DF; constants 100.0f
0x90A09, 0.5 0x90A0D, 50.0f 0x90A15, −100.0f 0x90A19), take 0x3DFDC, put 0x3DF4C (walk-in 0x306BC), weights 0x3EC9C,
0x3ED7C, 0x3ECB4, story test 0x3ED3C, RecordReplayEvent 0x3FA04, ResolveSectorBattle 0x3E508, TimeCompress 0x11D40;
counter 0x22A0BD, Santino 0x22A008, guard_skill 0x22A030, supplies 0x22A0BE, splash 0x92E76. Mirror tools/enemy.py;
harness validate/diff/enemy_diff.py.

## Walk-ins: hired guards and enemies come in at the map edge

```c
int32_t ja1_edge_tile(Ja1World *w, const Ja1Host *h, int32_t edge);
int32_t ja1_guard_enter(Ja1World *w, const Ja1Host *h, int32_t edge);
int32_t ja1_enemy_walk_in(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t edge);
```

The day clock's op 312 guard_enter (turns.md, The day clock, step 5) and the enemy move's op 329 enemy_walk_in
(Enemy moves above) are library ops: the library runs them itself unless the host sets bit 63 of `host_ops`. The
three entry points above run the same procedures directly. They use the sector start procedures of
sector_start.md: Slot reset, Guard (`ja1_init_guard`), Equipment roll (`ja1_equip`), Soldier start
(`ja1_soldier_start`), Occupy and the host op 51 replace. "Record k" = soldier record k; a record is 0x200 bytes.

**Edge tile** (edge): e = the low byte of edge, read as a signed byte. d = (e − 1) low 8 bits. t = −1; tries = 0;
deep = 0. Repeat while tries < 1000:

1. Pick t by d: 0 → 0xC08 + random(56) (the bottom row); 2 → 56 × random(56) (the left column); 4 → random(56)
   (the top row); 6 → 56 × random(56) + 55 (the right column); 8 → one of the 7 tiles 0x240, 0x241, 0x242, 0x278,
   0x2B0, 0x2B1, 0x2B2 by random(7) (sector 0's gate). Any other d keeps t. (SPEC GAP (not seen: no caller passes another edge): the callers pass only edges
   1, 3, 5, 7 and 9; for any other edge the library gives −1 at once and draws no random number.)
2. If tile_standable(t) > 0: if t0 of t < 0x6C or deep ≥ 10: when occupant(t) ≥ 0x40 the result is t. Else
   (t0 ≥ 0x6C and deep < 10): deep = deep + 1.
3. tries = tries + 1.

After 1000 tries the result is −1.

**Guard enters** (edge, op 312): k = the first record 8..15 whose +0x182 is 0. No such record: result 0. t = Edge
tile (edge); t = −1: result 0. Else: all 0x200 bytes of record k = 0; Slot reset (record k, slot k); f = random(8)
+ 1 (a byte); Guard (slot k, tile t, facing f, orders 4, anchor 0xC41); Soldier start (record k); +0x1AB of record
k = 1. Result 1.

**Enemy walks in** (slot, edge; op 329): slot = its low byte, e = the low byte of edge. t = Edge tile (e); t = −1:
result 0 (nothing changed). Else, on record s = slot:

1. All 0x200 bytes = 0; Slot reset (record s, slot s); +0x1E0 = random(4).
2. +0x1A1 = random(8) + 1 if e = 9, else e. +0xBB = 2; u16 +0x124, +0x114, +0x12E = t; +0x182 = 1; +0x17B =
   random(6) + 0x11; +0x17C = random(8) + 2; r = random(6).
3. +0x1D4 = 4; +0x17E = 0; +0x1F3 = 0; u16 +0x1E8 = 4; u16 +0x134 = 0xC41; +0x186, +0x188, +0x189, +0x19C = 0;
   u16 +0x12A = 0xC41; +0x185 = 0; +0x1BD = 0x6D; +0x9C = 0xFF; +0x1D3 = r; +0x1A2 = +0x1A1.
4. Level: n = 500 if day > 50, else 1000 − 10 × (day − 1) (day = the world's `day`, signed 32-bit). r =
   random(s16 of the low 16 bits of n). i = 8; while i ≥ 0 and i × i × i > r: i = i − 1. +0x192 = 9 − i (so 1..10).
   If cur_sector = 0: if s8 +0x192 < 6: +0x192 = 6. Else q = (day + 2) / 3 (truncated toward 0); if s8 +0x192 >
   (q low 8 bits, unsigned): +0x192 = q low 8 bits.
5. Stats, in this order: +0x183, +0x18B, +0x18C, +0x18D, +0x18E, +0x18F, +0x190, +0x191; each = (+0x192 × 5 +
   0x23 + random(11) + random(10)) low 8 bits (the two random numbers drawn in that order for each stat).
6. u16 +0x10C = 0; +0x18A = 100; +0x184 = +0x183. +0x1B4 = (s8 +0x192 / 2 truncated toward 0 + random(2)) low 8
   bits; if +0x1B4 > 4 (unsigned): +0x1B4 = 4.
7. Equipment roll (slot s, flag 1); Occupy(t, +0x17A); host op 51 replace (slot s); Soldier start (record s).
   Result 1.

Original: edge tile 0x30490 (gate tiles at 0x29F36), guard enters 0x30600, enemy walks in 0x306BC. Mirror
tools/ai.py edge_tile / guard_enter / enemy_walk_in; harness validate/diff/ai_diff.py section walk_in (also
--port).

## Sector battles

When enemies come into a sector that the player owns or where his guards or tappers are, and no merc is there, the
strategic layer settles the battle by itself (op 218 resolve_battle (byte s, byte f); a library op: the host leaves
bit 63 of `host_ops` clear). Callers: Enemy moves (above, f = the flag f of the choice), Leaving the sector and the
end of a fight (soldier_update.md, f = 1). World fields: `sector_state` (T = the 9 bytes of s: T1 enemies, T3
tappers, T4 guards; byte 0 bit 0 = owned), `sectors` (R = the 0xB1-byte record of s: +0x96 the 8 enemy slots as in
Enemy moves, +0xAA, +0xAF mines, +0xB0 mines used), `guard_skill`, `supplies`, `story`, `accounts` (Day accounts:
g_sectorsGained 0x78, g_sectorsLost 0x7C, and g_lastSectorGained = i32 at 0x80), `dead_npc`, `dead_allies`,
`enemy_deaths`, `time_compress`. Sector bytes are unsigned and change with 8-bit wrap-around; the i32 counters wrap.

New host ops (the battle show; none of them changes the world):
- 332 battle_ui (int k, int n): k = 0 the show starts (busy pointer), 1 open the battle sounds (folder FX\battle),
  2 play "attack.8", 3 play "die<n>.8", 4 play "mine<n>.8", 5 wait until the sound ends, 6 stop the sound, 7 close
  the battle sounds, 8 the show ends.
- (2026-11-02, asm WaitMs? 0x67F48: only Escape skips the rest of the show; another key or a click ends that wait alone. 0x5CCF8 k 5 is one stream refill, not a wait.)
- 333 battle_wait (int ms): result byte ≠ 0 = the player skipped the rest of the show (a key or a click during the
  wait); 0 = the time went by.
- 334 battle_box (int s, int enemies, int guards, int tappers, int kind): the box "ENEMY ATTACKING SECTOR <s + 1>!"
  with the three counts; kind 99 = the first picture, 0 = an enemy fell, 1 = a guard fell, 2 = a tapper fell.
- 335 battle_msg (int t, int n): a status message with the sector number n = s + 1: t = 0 "Mine explodes, killing
  enemy in sector n" (no time limit), and with a 1500 ms box: 1 "GUARDS REGAIN SECTOR n", 2 "ENEMY DEFEATED IN SECTOR
  n", 3 "ENEMIES GAIN SECTOR n", 4 "NATIVES LOSE SECTOR n", 5 "GUARDS DEFEATED IN SECTOR n".
Existing ops used: 89 replay_event (s, s, code), 316 refresh.

**Set owned** (s, v): `sector_state` byte 0 bit 0 of s = v; then Workable trees (above) of s and of its neighbours,
in this order: s + 10 if s / 10 < 5, s − 10 if s / 10 > 0, s + 1 if s mod 10 < 9, s − 1 if s mod 10 > 0.

**An enemy falls**: `enemy_deaths` + 1; T1 − 1; op 89 (s, s, 0x54); then for k = 7 down to 0: the first slot
R[0x96 + k] that is > 0 (signed) becomes its negative (8-bit); none: nothing.

**Pick a sound** (m, last): n = Random(m), drawn again while n = last; last = n.

Values: fought = 0, told = 0, sounds = 0, skip = 0, last_die = 0xFF, last_mine = 0xFF. "Wait (ms)" = skip =
(op 333 battle_wait (ms)) low byte ≠ 0. s16(x) = the low 16 bits of x as a signed number.
1. op 332 (0, 0). If `time_compress` ≠ 0: Time compression (0). op 316.
2. **Guards fight** (only if T4 ≠ 0 or T3 ≠ 0):
   a. fought = 1, told = 1; op 89 (s, s, 0x4D); op 332 (1, 0); sounds = 1; op 332 (2, 0); op 334 (s, T1, T4, T3,
      99); op 332 (5, 0).
   b. a = Attack weight (s, f) (Enemy moves); w = `guard_skill` / 2 + 40.
   c. Rounds, only if T1 ≠ 0; each round:
      - Leave the rounds if T4 = 0 and T3 = 0.
      - G = w × T4, P = 2 × T3, M = R[0xAF], X = M × T1 × 5, D = P + G + X, K = M × (T4 + T3), A = s16(T1 × a + K).
        Leave the rounds if A = 0 or s16(D) = 0.
      - r = s16(Random(s16(D + A))).
      - If r < A (a defender falls): mine = (r < s16(K)). If Random(s16(5 P + G)) < s16(G): T4 − 1, `dead_npc` + 1,
        op 89 (s, s, 0x52), kind = 1; else T3 − 1, `dead_allies` + 1, op 89 (s, s, 0x53), kind = 2.
      - Else (an enemy falls): mine = (r < A + s16(X)) (a 32-bit compare); An enemy falls; kind = 0.
      - If skip = 0: Wait (Random(2000) + 800).
      - If skip = 0: when mine, Pick a sound (3, last_mine) and op 332 (4, n); else Pick a sound (5, last_die) and
        op 332 (3, n).
      - If mine: R[0xAF] − 1, R[0xB0] + 1.
      - If skip = 0: Wait (500); else op 332 (6, 0).
      - If skip = 0: op 334 (s, T1, T4, T3, kind), op 332 (5, 0); else op 332 (6, 0).
      - Leave the rounds if T1 = 0.
   d. If skip = 0: Wait (1000). If skip = 0: op 332 (6, 0). op 316.
3. **Mines** (also without guards): while T1 ≠ 0 and R[0xAF] ≠ 0:
   - If sounds = 0: op 332 (1, 0), sounds = 1.
   - Stop if Random(100) ≥ s16(T1 × R[0xAF] × 5).
   - op 335 (0, s + 1).
   - If skip = 0: Pick a sound (3, last_mine), op 332 (4, n), op 332 (5, 0), Wait (Random(2000) + 800). Else op 332
     (6, 0).
   - If told = 0: told = 1, op 89 (s, s, 0x4D).
   - An enemy falls; R[0xAF] − 1; R[0xB0] + 1; op 316.
   - If skip = 0: Wait (500).
4. If sounds ≠ 0: op 332 (7, 0).
5. The outcome (n = s + 1):
   - T1 = 0, not owned: op 335 (1, n); Set owned (s, 1); g_lastSectorGained = s; g_sectorsGained + 1; op 89 (s, s,
     0x4F).
   - T1 = 0, owned: op 335 (2, n); op 89 (s, s, 0x50).
   - T1 ≠ 0, owned: op 335 (fought ≠ 0: 4, else 3; n); Set owned (s, 0); g_sectorsLost + 1; op 89 (s, s, 0x4E);
     R[0xAA] = 1.
   - T1 ≠ 0, not owned: op 335 (5, n); op 89 (s, s, 0x4E).
6. Territory (Day accounts, `recalc_territory`); op 332 (8, 0); op 316.

Notes: a guard or tapper can fall on a mine (r < K): the mine is used up. In the mines step a sector without mines
(R[0xAF] = 0) does nothing; the step runs also after a won guards fight only while enemies are left.

**Sector gained** (op 201 sector_gained (s8 s); a library op): `sector_state` byte 0 bit 0 of s = 1; Workable trees
of s and its neighbours (as in Set owned); g_sectorsGained + 1; g_lastSectorGained = s; op 89 (s, s, 0x4F).

Original: ResolveSectorBattle 0x3E508 (show: 0x4B730 / 0x4B764 busy pointer, FormatFxPath 0x24C90, 0x5C16C +
SpeechStream 0x5CC6C play, 0x5CCF8 sound wait, 0x5CE08 sound stop, 0x5C144 close, 0x67F48 timed wait, battle box
0x3E2D4, ShowMessage 0x27B98, refresh 0x1CB78), SetSectorOwned 0x3DDE4 + neighbours 0x404B4, SectorGained 0x3FA54,
attack weight 0x3ECB4. Harness validate/diff/battle_diff.py.

**The battle show (what the player sees and hears).** The battle runs on the tactical screen (the day's clock); the
show ops do this:
- op 332 k = 1 opens the archive `FX\BATTLE` (members ATTACK.8, DIE0.8..DIE4.8, MINE0.8..MINE2.8: raw 8-bit
  unsigned mono samples); k = 2 / 3 / 4: the speech channel plays ATTACK.8 / DIE<n>.8 / MINE<n>.8 at rate 11025
  (0x2B11), volume 88 (0x58), once (a new one replaces the one that plays); k = 5: wait until the speech channel is
  silent; k = 6: the speech channel stops; k = 7: the archive closes; k = 0 / 8: the busy pointer on / off.
- op 333 battle_wait (ms): the program waits ms milliseconds; a key or a mouse button in that time ends the wait
  and answers "skipped".
- op 334 battle_box: **The battle box** (below), straight on the screen; it stays until the refresh (op 316).
- **At the end of a sector** (soldier_update.md "Leaving the sector", Leave path step 2c: the sector the squad
  leaves at the day's end still holds enemies and has guards, tappers or is owned): the Leave path faded the screen
  out to a black palette (step 1) and the end-of-day screen closed with a fade out too, and nothing sets a palette
  before the battle. So the boxes of the show and its result message box (op 335) are drawn but the player sees
  a black screen; the sounds, the waits and the skips (a key or a click) are the same. No day step (the verdict,
  the morning, the office, the sunrise, the island map) runs a battle: the only callers of the battle are the day
  clock and enemy moves (tactical turns), the Leave path, a traverse and Squad lost (both of these before their
  fade out, so their shows are seen over the tactical screen).

  *Evidence (2026-10-27): callers of ResolveSectorBattle 0x3E508 = GuardsArriveMsg 0x1F724 (called by NewTurn?),
  FUN_0003EDE8 (enemy moves, from 0x1F724), FUN_00030D6C (leave sector; from TacticalLoop, TraverseMove? 0x315DC,
  SquadLost? 0x3A32C). Live, original: tac60 with sector 60 enemies 3 (slots +0x96 = 1, 1, 1) and the day clock at
  715 minutes: EndOfDayScreen, then ResolveSectorBattle with a black window (grabs during the battle and its
  ShowMessage? result box), guards 8 -> 7, enemies 3 -> 0.*

**The battle box** (s, enemies E, guards G, tappers T, kind): the picture comes from the tactical view as it is on
the screen (the back buffer, not the screen: the status line, the cursor and other boxes are not in it). Take the
area A = screen x 52..267, y 37..116 (216 x 80); every pixel p of A becomes D[p] (D = darkening table of byte
0x284F08: 28 on the tactical screen, the tileset's table). Then, with (x, y) relative to A's top left corner:
1. Frame (12, 6, 205, 74, colour 1), then Frame (11, 5, 204, 73, colour 0x68) (panel.md "Drawing primitives").
2. Text "ENEMY ATTACKING SECTOR n!" with n = (s as a byte) + 1 at (108, 18), centred, fg 0xFF, shadow 1, font 2,
   mode 300; then the text "Enemies       Guards      Tappers" (7 and 6 spaces) the same way at (108, 40).
3. The three counts as numbers of at least 2 digits (left padded with the glyph '^'), not centred, shadow 1, font 2,
   mode 300, y = 50: E at x = 40, G at x = 98, T at x = 155. A count's fg is 0x16 when kind names its side (0 = E,
   1 = G, 2 = T), else 0xFF (kind 99 or any other value: all 0xFF).
4. Only the part x 11..205, y 5..74 of A (screen x 63..257, y 42..111, 195 x 70) goes to the screen; the rest of
   the screen does not change.

Evidence (2026-10-26): BattleBox 0x3E2D4 (sprintf "%s %d!" 0x909CC with 0x99FE7, BlitCopy 216 x 80 from the back
buffer 0x22A4EF + (0x41 − *0x229CC4) × 384 + 0x54 − *0x229CB8 (the tactical copy 0x17AE8 puts back buffer (32, 28)
at screen (*0x229CB8, *0x229CC4)) into *0x93E6C pitch 228, RemapRect with g_remapTables[0x284F08], DrawFrame ×2,
DrawText ×2, DrawNumber ×3 (colours ebp / esi / edi), BlitCopy 195 x 70 from +0x47F to screen +0x34BF);
SpeechStream 0x5CC6C (0x2B11, 0x58, 0) after 0x5C16C at 0x3E5C1 / 0x3E82C / 0x3E9A9; names 0x909DC "attack.8",
0x909E8 "mine%d.8", 0x909F4 "die%d.8", folder 0x909D4 "battle". validate/diff/battlebox_diff.py --port: 300 cases
(noise back buffer and screen, view offsets, darkening tables except 28, any kind), 0 mismatches.

## The day replay

After the sunset and before the day's verdict the game replays the day on the island map: each event of the day
(squads that move, battles, sectors won and lost, deaths) plays again as a short picture show with sounds, on a
copy of the sector table taken at the day start (`sector_replay`, Day start). Library op `day_replay`
(`ja1_day_replay`). World fields: `sector_replay` (R(s, o) = byte o of the 9-byte entry s), `opt_replays` (the
Control Panel switch 12 "Day Replays", save byte 21550), `replay_carry` (u8, a count kept from one event to the
next; the original keeps it in a game byte that no other code writes, so no save holds it and no load or new game
changes it; 0 at program start). Host ops 353
replay_count, 354 replay_record, 355 replay_anim, 356 replay_draw.

**The day's events.** Each host op 89 replay_event (a, b, code) of the library (and of the front-end's own
strategic steps) adds the event (from = low byte of a, to = low byte of b, code = low byte of code) to the end of
the day's list. The list is emptied when a tactical day starts: at the first sector entry of a day, and again when
"Restart this day" enters the sector anew (the events of the given-up try are gone). Evidence (2026-10-28): the
original's sector loader LoadTacticalSector? (called only by TacticalLoop 0x107D1, once per tactical day and once
per restart) sets g_replayCount = 0 at 0x2AF83 on every path and deletes `replay.rec` (0x2AF8B..0x2AF9E).
Host op 353 replay_count () answers the number of events (as i16); host op 354 replay_record (i) answers event i as
from | to << 8 | code << 16, plus 1 << 24 when Escape was pressed since the last poll (the original polls Escape here).

**Day replay** (`ja1_day_replay`): show = `opt_replays` ≠ 0 at the start; "shown" below means show = 1 at that
moment (show only goes from 1 to 0). An answer of host op 355 with bit 0 set (Escape during the animation) makes
show = 0. Owned(s) = bit 0 of R(s, 0). Table work on R uses the routines of the real table on `sector_replay`
instead: Set owned (Sector battles), Workable trees (Day accounts; Connected to home also reads R); "workable all"
= Workable trees of the replay table for s = 0..59 in order.

1. n = replay_count. If n = 0: nothing more (no picture either).
2. Host op 356 replay_draw (0, 0) (the whole map from R).
3. For i = 0 .. n − 1: e = replay_record (i); f = from, t = to (signed byte), c = code. If bit 24 of e is set: show = 0.
   s0 = show (the value at this moment). Then by c:
   - `J` (a squad moves f -> t): if s0: replay_anim (1, f, t, 1) (march). If shown and not owned(f): replay_anim (0,
     f, 2, 0). If shown and not owned(t): replay_anim (0, t, 2, 1). R(t, 2) = R(f, 2); R(f, 2) = 0.
   - `K`: R(t, 1) += carry; R(f, 1) −= carry; carry = 0.
   - `L` (enemies move): if s0: replay_anim (0, f, 2, 1), then if shown: replay_anim (1, f, t, 2). R(t, 1) += carry;
     R(f, 1) −= carry; carry = 0.
   - `M` (a battle in f): if s0: the battle show (below).
   - `N` (f is lost): Set owned (f, 0); R(f, 4) = 0; R(f, 3) = 0; R(f, 2) = 0; workable all. If shown:
     replay_draw (1, 1), replay_draw (2, f), replay_anim (0, f, 2, 0).
   - `O` and `P` (f is won): if s0: replay_anim (3, f, 2, 0) (burn). Set owned (f, 1); R(f, 1) = 0; workable all. If
     shown: replay_draw (1, 1), replay_draw (2, f).
   - `Q`: if s0: replay_anim (4, f, 0, 0) (explosion).
   - `R`: R(f, 4) −1 when ≠ 0. `S`: R(f, 3) −1 when ≠ 0. `Z`: R(f, 2) −1 when ≠ 0. `U`: R(f, 6) −1 when ≠ 0, then
     Workable trees (f). `V`: R(f, 6) −1 when ≠ 0, R(f, 8) −1 when ≠ 0, then Workable trees (f). Each of these
     five, then if shown: replay_draw (3, f).
   - `T` (an enemy dies): R(f, 1) −1 when ≠ 0.
   - `W`: R(f, 1) += carry; carry = 0.
   - `X` (the squad enters f): if s0: replay_anim (0, f, 1, 1).
   - `Y` (the day ends in f): if s0: replay_anim (0, f, 1, 0). R(f, 2) = 0. If shown and not owned(f): replay_anim
     (0, f, 2, 0).
   - `[`: R(f, 4) = 0 when R(f, 4) − t < 0 (R unsigned, t signed), else R(f, 4) −= t. If shown: replay_draw (3, f).
   - `\`: R(f, 5) += t; when R(f, 4) + R(f, 5) > 8 (unsigned): R(f, 5) = 8 − R(f, 4). If shown: replay_draw (3, f).
   - `]`: R(f, 4) += t; when R(f, 4) > 8 (unsigned): R(f, 4) = 8. R(f, 5) = 0 when R(f, 5) − t < 0, else R(f, 5)
     −= t. If shown: replay_draw (3, f).
   - c = 0..63 (a merc dies; c = his picture): if s0: replay_anim (5, c, f, t) (t = the merc's team slot). If
     R(f, 2) ≠ 0: R(f, 2) −1, and when it became 0 and shown: replay_anim (3, f, 1, 0). If shown: replay_draw (1, 1).
   - c > 100 (signed byte): carry = c − 100.
   - Any other code: nothing.
   All byte changes wrap.
4. If show = 0: workable all, then replay_draw (0, 0).

**The battle show** (code `M` in sector f; uses the game's random numbers): three bursts k = 0, 1, 2, each with a
size z (0 small, 1 big, 2 done) = 0 and a count m = 99. For frame j = 0 .. 99 (r = 99 − j): if show = 0, stop. For
each k: L = 3 when z = 0, else 8. If m < L: m + 1. Else: x = Random(25) + 3, y = Random(18) + 3, z = 1 when Random(10)
> 4 else 0; L = 3 or 8 by that z; when r ≤ L: z = 2, m = 99; else m = 0. Then host op 355 replay_anim (2, f, j,
z0 | m0 << 8 | x0 << 16 | y0 << 24, (the same for burst 1), (burst 2)) (x, y of a burst = its last values, 0 at
the start); bit 0 of the answer set: show = 0 (the frame still ends; the next frame stops).

**Host op 356 replay_draw (k, a)**: k = 0 the whole island map from R (the owner colours and the counts of each
sector), 1 the map overlays from R, 2 the sector a's border, 3 the sector a's cell again from R (pictures: "The
replay pictures" below).

**Host op 355 replay_anim (k, ...)** plays one animation and answers 1 when Escape was pressed in it (it then stops
early), else 0. Frame times are the original's busy-wait counts in ms (the pictures of the frames: "The replay pictures" below). The
sounds are the effects of the **replay bank**: effects 0..5 = `FX\REPLAY` members `WHOOSH MARCH2 BURN ASHES
BATTLE EXPLODE` (`.8` added; loaded when the replay screen opens; effects 6 and up keep the tactical bank), each
started as op 60 sfx (e, 11025, v, 1):
- k = 0 (s, mode, whoosh): 8 frames of 90 ms; whoosh ≠ 0: effect 0 (WHOOSH, volume 88) at frame 3. Then one more
  frame (90 ms).
- k = 1 (f, t, kind) march: no frames when f and t are the same cell. Else 9 steps of 150 ms, effect 1 (MARCH2,
  volume 88) at each step.
- k = 2 battle frame (s, j, ...): effect 4 (BATTLE, volume 88) at frame j = 0; each frame 30 ms.
- k = 3 (s, mode) burn: 22 frames of 90 ms: effect 2 (BURN, volume 88) at frame 0, effect 3 (ASHES, volume 88)
  at frame 11; then one more frame (90 ms).
- k = 4 (s) explosion: 3 times: effect 5 (EXPLODE, volume 104), wait 400 ms (the first time) or 200 ms, wait 600 ms
  (the third time) or 400 ms.
- k = 5 (c, s, slot) the merc's death picture: effect 0x30 + 4 × slot (the merc's DIE.8 of the tactical bank,
  volume 50) at the start; the zoom in: 10 steps of 20 ms; 10 frames: 450 ms more for the first (30 × 15 ms), 30 ms
  each; then 600 ms (60 × 10 ms); the zoom out: 10 steps of 20 ms.
Escape stops each animation at its next frame. The library calls host op 355 only while show = 1 (an animation of
the original that starts with show = 0 does nothing but draw).

**The replay pictures** (front-end). The replay screen is 320 x 200 with the palette of `POSTPAN.PCC` (ReplayScreenInit? 0x64274, replayinit_diff;
the live shot has the blue-grey panel; the map art looks a little different in it):
The original draws each replay step straight into the VGA memory (screen pointer 0x95678 = 0xA0000); only a
busy wait keeps a picture on the screen. A step with the next step right behind it (the clean cell before an explosion
frame) stays there only for the drawing time. The port shows only the pictures before a wait and the last one
(`play::replay_shown`, the window and replaypic_diff); before 2026-11-05 the window showed each in-between picture
for ~0.7 ms (measured), and a compositor could keep it for one display frame.
rows 0..145 hold the island map (`MAP.PCC`, the gaps between the cells keep its pixels), rows 146..199 the picture
`POSTPAN.PCC` (MAIN.DAT, 320 x 54). Besides the screen there is the **clean map**, a second 320 x 200 picture (the map without flags), and the
flag sheet `REPLAY.PCX` (MAIN.DAT, 330 x 48, pitch 330 here; the original keeps it at pitch 384). Flags are 15 x 15
blocks of the sheet; sheet pixel 0 is not written. Sheet blocks (x, y): green flag burns (15 j, 0), red flag burns
(15 j, 15) for j = 0..21; green flag rises (15 j, 30) and red flag rises (120 + 15 j, 30) for j = 0..7; green flag
waves (240 + 15 i, 30) and red flag waves (285 + 15 i, 30) for i = 0..2. Green is the mercs (R(s, 2) ≠ 0), red the
enemies (R(s, 1) ≠ 0). At the replay start the screen shows the map of R (cells and markers as in "Drawing the
map", marks 0) and the clean map is the same picture.

- **Box** of sector s: bx = (s mod 10) × 31 + 5, by = (s div 10) × 23 + 3, 32 x 24 pixels.
- **Flag frame** (s, red, green): take the box of s from the clean map; when red is a block, draw it at (bx + 2,
  by + 3); when green is a block, draw it at (bx + 14, by + 9); put the box on the screen; then wait 90 ms.
- **Clean cell** (s): the cell and the markers of s from R (as in "Drawing the map") into the clean map (the
  screen is not changed).
- replay_draw (0, 0): the cell and the markers of each sector s = 0..59 from R onto the screen (the clean map is
  not changed).
- replay_draw (1, 1): for s = 0..59: clean cell (s); when R(s, 2) ≠ 0: flag frame (s, red waves 300 when any sector
  up to s with R(·, 2) ≠ 0 had R(·, 1) ≠ 0, green waves 255). (Original bug, issue #2: the red flag of one such sector
  stays on for every later sector with mercs; the port keeps it.)
- replay_draw (2, a): for each s ≠ a with R(s, 2) = 0: copy the 30 x 22 cell rectangle of s ((s mod 10) × 31 + 6,
  (s div 10) × 23 + 4) from the clean map to the screen.
- replay_draw (3, a): clean cell (a); when R(a, 2) = 0 copy the cell rectangle of a from the clean map to the
  screen, else flag frame (a, red waves 300 when R(a, 1) ≠ 0, green waves 255).
- replay_anim (0, s, mode, whoosh), 8 flag frames n = 0..7 (w = n mod 3; the sound at frame 3 when whoosh ≠ 0),
  with j = n when whoosh ≠ 0, else j = 7 − n: mode = 2: red rises (j), and green waves (w) when R(s, 2) ≠ 0; else
  green rises (j), and red waves (w) when R(s, 1) ≠ 0. Then one more flag frame: whoosh ≠ 0: the risen flag now waves, the other waving flag
  as before, both with w = 2 (mode 2: red waves, green waves when R(s, 2) ≠ 0; else green waves, red waves when
  R(s, 1) ≠ 0); whoosh = 0: frame 7 again without the rising flag.
- replay_anim (3, s, mode) burn, 22 flag frames n = 0..21 (w = n mod 3; sounds as in the timeline): mode = 2: red
  burns (n), green waves (w) when R(s, 2) ≠ 0; else green burns (n), red waves (w) when R(s, 1) ≠ 0. Before
  frame 21: when mode = 2, Set owned (s, 1) (with the Workable trees of s and its neighbours, on R) and R(s, 1) = 0 on the picture's R; then replay_draw (1, 1) and
  replay_draw (2, s). Then one more flag frame as frame 21 without the burning flag.
- replay_anim (4, s) explosion, 3 times: replay_draw (3, s), panel element 114 at (bx + 9, by + 9) on the screen,
  wait 400 ms (first time) or 200 ms; replay_draw (3, s), element 115 at the same place, wait 600 ms (third time) or
  400 ms. Then replay_draw (3, s). The sound of each time starts before its first replay_draw.
- replay_anim (1, f, t, kind) march (kind 1 = the squad, a green flag; kind 2 = enemies, a red flag). Box x, y of
  f = (fx, fy), of t = (tx, ty). The **area**: when fx ≠ tx: 64 x 24 at (min (fx, tx), fy), the flag moves dx = +4
  (fx < tx) or −4 per step, dy = 0, change step c = 6; else when fy < ty: 32 x 48 at (fx, fy), dy = +3, c = 4; fy >
  ty: 32 x 48 at (fx, ty), dy = −3, c = 7; else nothing at all. The flag offset (ox, oy) = (2, 3) for kind 2, (14, 9)
  for kind 1; the moving flag starts at (fx + ox, fy + oy) on the screen. At the start: stay = kind 1 and R(f, 1) ≠ 0
  (the enemies stay in f); there = kind 2 and R(t, 2) ≠ 0; both = there and R(t, 1) ≠ 0. Steps n = 0..8 (w = n mod
  3): effect 1 (MARCH2); at n = c: the change of R (kind 1: R(t, 2) = R(f, 2), R(f, 2) = 0; kind 2: R(t, 1) += carry,
  R(f, 1) −= carry (carry of the moment, the world's `replay_carry`)), then replay_draw 3 of f and of t, but for kind
  1 always the cell copy (no flag frame); then the area from the clean map, with: the moving flag (kind 1 green waves
  w, kind 2 red waves w) at its place, red waves (w) at (fx + 2, fy + 3) when stay, red waves (w) at (tx + 2, ty + 3)
  when both, green waves (w) at (tx + 14, ty + 9) when there; the area onto the screen; the flag moves by (dx, dy); a
  wait of 150 ms (none when Escape came in this step: the march ends). Flag pixels outside the area are not shown
  (only a march between sectors that are not neighbours has them; the original writes them into its scratch buffer).
- replay_anim (2, s, j, b0, b1, b2) battle frame j of sector s. The **scratch** is a picture of pitch 384 (at least
  200 rows; it keeps its pixels from frame to frame). Burst k: size z = bits 0..7 of bk, count m = bits 8..15, x =
  bits 16..23, y = bits 24..31. At frame j = 0, first: the cell and the markers of s from R onto the screen (as in
  "Drawing the map"), then the box of s (32 x 24) from the screen into the scratch at (64, 0). Each frame: the 32 x
  24 rectangle at (64, 0) of the scratch to (0, 0) of the scratch; for k = 0, 1, 2 with z ≠ 2: the 3 x 3 burst block
  of the flag sheet at ((z = 1: 0, else 24) + 3 m, 45) at (x, y) of the scratch (sheet pixel 0 is not written); the
  markers of s from R into the scratch at their **screen** places (x0, y0 as in "Drawing the map", rows of pitch 384;
  pixels past row 199 are not written); red waves (w) at (2, 3) of the scratch; when R(s, 2) ≠ 0 green waves (w) at
  (14, 9), with w = ((j + 1) div 4) mod 3; then the rectangle at (0, 0) of the scratch onto the box of s on the
  screen. (Original bug, issue #2 row 32: the markers of each frame land in the shown rectangle only for sector 0 (moved 5
  right, 3 down) and, through the copy at (64, 0), for sectors 1 and 2 from the next frame on, as a second, moved
  copy beside the markers that the box held at frame 0; for every other sector they are not seen.)
- replay_anim (5, c, s, slot) the merc's death picture. The **morph strip** = `MORFc.PCC` (280 x 25: 10 frames of
  28 x 25 side by side, frame i at x = 28 i) from the archive `DAT\MF1.DAT` when c < 30, else `DAT\MF2.DAT` (its
  palette is not used). The face place: x = bx + 2, y = by (box of s). Steps:
  1. Save the 30 x 27 rectangle of the screen at (x − 1, y − 1) (the **background**). Sound as in the timeline.
  2. Zoom in: for n = 1..10: the size (w, h) = (2, 2), (5, 5), (8, 7), (11, 10), (14, 12), (16, 15), (19, 17),
     (22, 20), (25, 22), (27, 25); a 28 x 25 picture = the background part at (1, 1), with frame 0 scaled to w x h
     (nearest scaling as in "The switch box (Control Panel)" Opening step 4, source 28 x 25) at ((28 − w) div 2, (25 − h) div 2);
     the picture onto the screen at (x, y); wait 20 ms.
  3. A frame (panel.md "Frame") (x − 1, y − 1, x + 28, y + 25) in colour 0x27.
  4. For i = 0..9: frame i of the strip onto the screen at (x, y) (all pixels); waits as in the timeline.
  5. Wait 600 ms. The background back onto the screen; frame 9 onto the screen at (x, y).
  6. Zoom out as step 2 with frame 9 and the sizes (25, 22), (22, 20), (19, 17), (16, 15), (14, 12), (11, 10),
     (8, 7), (5, 5), (2, 2), (1, 1).
  7. The background back onto the screen (the face is gone).
  The sizes come from float32 steps: step = float32(28 × 0.1f) (and 25 × 0.1f), size = float32 sum of the steps
  from 0 (zoom in) or from 28 / 25 minus the steps (zoom out), cut to an integer, at least 1.
- The 90 ms of each flag frame that replay_draw (1 or 3) makes inside an animation add to the waits of the timeline
  above.
- **Fade in**: before the first record the screen (the map of R over POSTPAN.PCC) fades in from black with the POSTPAN.PCC
  palette: 25 levels l = 0..24, each colour (6-bit) × l / 24 (cut), each level after one vertical retrace (the fade
  of ending.md op 24 with n = 24). (The original screen init fades in also with Day Replays off; the front-end shows
  the replay screen only when the replay has pictures, but the verdict's rating word is drawn on the start picture.)
  With no record at all (no merc in the sector at entry and at sunset, no enemy move) DayReplay does nothing, and the word
  still goes on the start picture.

Evidence (2026-10-26): DayReplay 0x652B8 (count i16 0x22A46E, records read from REPLAY.REC into 0x22A4E5 (from, to,
code); show byte 0x963BC = Day Replays 0x1163C4, done byte 0x963BD, carry 0x9562D; EscPressed? 0x67300 per record;
switch on code − 0x4A < 0x14; register arguments of each call read in asm: 0x64658 (s, EDX mode, EBX whoosh),
0x64D20 (f, t, EBX kind 1 / 2), 0x643F8 (s, EDX), 0x41C60 (tbl, s, BL 1)); the animations: 0x64658, 0x64D20,
0x6494C (battle; Random 25 / 18 / 10; pictures 2026-10-26 asm read: scratch 0x22ECEF pitch 0x180, box saved at
+0x40, bursts from sheet row 45 (+0x4380), DrawMarkersTo (R, s, scratch, 0x180), flags at [esp + 0x4c] / [esp + 0x58], wave
counter [esp + 0x74] / [esp + 0x78]), 0x643F8, 0x664FC, ReplayDeathAnim 0x65FB0 (PlaySfxSlot(BL × 4 + 0x30, 0x2B11,
0x32, 1); pictures 2026-10-26 asm read: LoadSanimDat? 0x5E8E0 (0x91268 / 0x91274), sprintf 0x91280, 0x532BC into the sheet
buffer + 0x9000 pitch 0x180, background at + 0xB580, ReplayZoom 0x6630C (src, 28, 25, scratch + 0x118, out flag, background + 0x181,
screen place; 0.1f at 0x9128B, __CHP, ScaledBlit? 0x52B7C, wait 0x67250 (20)), DrawFrame 0x46C44 (colour 0x27)); frame wait 0x67250 (busy loop, n × calibration), 0x67EEC; the animations change R themselves at a
frame and set the done byte, and DayReplay applies the same change when done = 0, so R ends the same with and
without them. ReplayScreenInit? 0x64274 loads the six names at 0x963BE (FormatFxPath "replay"). RecordReplayEvent
0x3FA04 (AL from, DL to, BL code). Pictures (2026-10-26 asm read): ReplayScreenInit? 0x64274 (fade 0x52908 (palette 0xEDA08, 0, EBX 24, ECX 0, 256) last; POSTPAN.PCC at row 146,
DrawStrategicMap loads MAP.PCC's palette last), DayReplay loads REPLAY.PCX at pitch 0x180 into 0x22A4EF and copies
the screen into the clean map *0x93E60; flag frame 0x6416C (box 0x413D4 / 0x413F0, scratch 0x22ECEF pitch 0x180,
BlitKey key 0 at scratch + 0x482 / + 0xD8E, wait 0x67250 (0x5A)); replay_draw 0 = 0x41C3C, 1 = 0x41B24 (red flag
flag in [esp + 4], set and never cleared), 2 = 0x41BB8, 3 = 0x41C60 (clean cell 0x416E4 + 0x41788); 0x64658 flags,
0x643F8 burn, 0x664FC explosion (DrawPanelElem 0x46920 0x72 / 0x73, wait 0x67EEC), 0x64D20 march (area at
[esp + 0x68] / [esp + 0x20], flag at [esp + 0x28] / [esp + 0x2c], step [esp + 0x30] vs change [esp + 0x40], wait 0x96).
validate/diff/replaypic_diff.py --port (2026-10-27): replay_draw 0..3 with the flag frames on fuzzed R, noise screen
and clean map, 300 cases: screen, clean map and wait total equal (the game calls replay_draw 1 with EDX = 1 only).
Same test, cases 4..7 (2026-10-27): replay_anim 0 (flags, mode 1 / 2, whoosh 0 / 1), 1 (march to a neighbour, kind
1 / 2, carry 0..255 in 0x9562D), 3 (burn, mode 1 / 2), 4 (explosion) in Unicorn (PlaySfxSlot and Escape stubs; the
waits 0x67250 and 0x67EEC summed): equal. The burn's Set owned changes byte 7 (workable) of s and its neighbours,
which the redraw at frame 21 shows; a port that only set bit 0 failed 12 of 200 cases. Not covered: the battle
frames (Random in 0x6494C) and the death picture (a file strip).

## The day's verdict

After the end-of-day replay (and after `sector_replay` flags bit 0 are cleared) the game judges the day: it may end
the game, it draws the day's rating word and the guide says what he thinks. Entry points `day_verdict` (result: 0 =
the game goes on; 2, 3, 4 = the game ends) and, only after a result 0, `day_bonus`. World fields: `story` (S[i] =
byte i, 43 bytes), `accounts` (Day accounts offsets; also A16 0x92 and A16 0x94 = u16 at 0x92
/ 0x94, success points = u8 at 0x9A, bad-day streak = u8 at 0x9B), `sector_state`, `sectors` (R(s, o) = byte o of
record s), `plants` (as in The guide's advice: `capacity`, `sector`, `state` +8, countdown +0xB), `crates`,
`crate_state`, `supplies`, `guard_skill`, `soldiers` 0..7, `stash_counts`, `day`, `difficulty` (d; SPEC GAP (not seen: the Set Difficulty screen (world_load.md) gives 0, 1 or 2 only; another value needs an edited save): d > 2 reads as 2),
`team_on_duty`, `idle_day`, `dead_mercs`, `dead_allies`, `dead_npc`, `enemy_deaths`, `prev_balance`, and the new
fields `start_trees`, `start_workable`, `start_tapped`, `start_sectors`, `start_cap`, `start_plants` (the day-start
copies, see "Day-start copies" below), `item_told` (3 bytes), `bonus_due` (u8), `bonus` (u16). Host ops 302
guide_say, 317 brenda_say, 308 stash_take, 330 home_lost_msg, 331 rating_word. "Say q" = guide_say (q, 3).
Comparisons are signed unless stated; byte changes wrap. owned(s) = `sector_state` flags bit 0 of s. Connected(s) =
Connected to home of s. Search (t, take, store) = Squad search (Story events at the day start) over all 8 records,
where the item store (host op 308) is asked only when store = 1. said = 0, late = 0, q = 999 at the start.

**Day-start copies.** At each sector entry (world_load.md section 4 step 2), before `ja1_enter_sector`, the front-end
sets `start_trees` = g_treesOwned, `start_workable` = g_treesWorkable, `start_tapped` = g_treesTapped,
`start_sectors` = g_sectorsOwned, `start_plants` = g_plantsOwned, `start_cap` = g_processCap, and `dead_mercs`,
`dead_allies`, `dead_npc`, `enemy_deaths` = 0. (So the death counts are those of the day.)

**Crate check.** `guard_skill` = 0 (the original's byte 0x22A030 = accounts +0x90). For k = 0..3: c = the 4 bytes of `crates` from 4 k (sector = s8 c[0], drop day =
s8 c[1], kept flag = c[3]); st = `crate_state`[k].
1. When drop day = `day`: (a) when st = 0: if owned(sector): st = 5; else if kept flag ≠ 0: st = 4; else Crate lost
   (k) (sector_start.md). (b) Then, with st read again: when st = 1: if owned(sector): st = 2; else if kept flag ≠ 0:
   st = 4; else Crate lost (k).
2. (Every k.) When st (read again) = 2 and kept flag = 0: `guard_skill` + 10.

**The day's score** (a value −3..3). All sums are 16-bit (wrap; read as signed 16-bit): x = (g_treesOwned −
`start_trees`) + (g_treesWorkable − `start_workable`) + (g_treesTapped − `start_tapped`) + 5 (g_sectorsOwned −
`start_sectors`) + 10 (g_plantsOwned − `start_plants`) + `enemy_deaths` − 5 `dead_mercs` − `dead_npc` −
`dead_allies` (each term from the low 16 bits). v = 0 when |x| < 5; else v = (x + 5) / 10 for x > 0 and (x − 5) /
10 for x < 0 (division truncated toward zero; v as 16 bits). v = v − `day` / 50 (truncated; v stays 16-bit). When
`day` > 250 and v ≥ 0: v = −1. When `idle_day` ≠ 0 and v > 0: v = 0. When v < 0: streak + 1; when streak ≥ 3
(unsigned): v − 1; when streak = 4: v − 1. Else (v ≥ 0): streak = 0 and success points + (v low 8 bits) (the value
before the clamp). Then v < −3 → −3, v > 3 → 3: that is the score.

**`day_verdict`**, in this order:
1. Crate check; score = the day's score.
2. When not owned(0x3B) (home): host op 330 home_lost_msg; result 4.
3. When s8 S[0xA] = 1 and no sector s = 0..59 is owned with `trees` ≠ 0: say 0xB2; result 3.
4. When streak > 4 (unsigned): say 0x95; result 2.
5. Host op 331 rating_word (score) (the word TERRIBLE, LOUSY, POOR, FAIR, GOOD, EXCELLENT, OUTSTANDING for −3..3).
   The original draws the word straight on the screen (the day replay screen, under the panel label RATING):
   centred at x 276, y 161, font 0, colour 0xFF, shadow 1, mode 300 (DayEndVerdict 0x60756: DrawText of the
   12-byte word table 0x965D6 + 12 × (score + 3)). Nothing redraws it before the guide speaks.
6. **Ransom** (s8 S[0xB] = 1): said = 1.
   - S[0x16] = 0: S[0x14] + 1; told = 0. For (j, item, quote) = (0, 0x86, 0xAB), (1, 0x7A, 0xA0), (2, 0x5C, 0x9F)
     in order: when `item_told`[j] = 0 and Search (item, 0, 0) ≠ −1: say quote, `item_told`[j] = 1, told = 1. When
     told = 0: if S[0x14] < 11 (unsigned): say S[0x14] + 0xA0; else said = 0. Then when S[0x14] (unsigned) ≥ L1(d)
     (L1 = 10, 8, 6 for d = 0, 1, 2) and S[0x15] = 0: S[0x15] = 1, and for i = 0..4: with n = s8 S[0x17], b =
     R(n, 0x9F + i): b = 0xAE → 0xAC; then (read again) b = 0xAF → 0xAD.
   - S[0x16] ≠ 0: if S[0x15] = 0: say 0xAC, `bonus_due` = 1, `bonus` + 15000; then when s8 S[0xA] = 1, S[0x20] = 0,
     S[0x21] = 0 and Search (0x51, 1, 1) ≠ −1: S[0x21] = 1; when s8 S[0xE] < 2 or S[0x1E] ≠ 0: brenda_say 0xF,
     S[0x22] = 3; else brenda_say 0xD, S[0x22] = 1. If S[0x15] ≠ 0: say 0xAD, late = 1. Both: S[0xB] = 2.
7. When streak = 4: say 0x94, said = 1.
8. **Losses** (said = 0 only), with m = `dead_mercs`, a = `dead_allies` > 5 or `dead_npc` > 15: m ≤ 0: when
   `idle_day` ≠ 0: q = score + 0x42. m = 1, 2: when a: q = score + 0x34. m ≥ 3: `team_on_duty` = 0: q = score + 0x2D;
   else q = score + 0x3B when a, else score + 0x26. Then (whatever said) when q ≠ 999: say q, said = 1.
9. **The purifier part** (s8 S[5] = 1): when Search (0x79, 1, 0) ≠ −1: S[5] = 2; q = 0x4B, 0x4A, 0x49 for score > 0,
   = 0, < 0; said = 1; say q; plant 0 `state` = 2; Plants (Day accounts); when `day` ≤ L2(d) (3, 2, 1): say 0x91,
   `bonus` + 1000, `bonus_due` = 1. Else (not found) when `day` = 1: q = 0x48, 0x47, 0x46 for score > 0, = 0, < 0;
   say q; said = 1.
10. **Blight** (s8 S[0xA] = 1): when some sector s = 0..59 is owned with `unhealthy` ≠ 0:
    - Search (0x5E, 1, 0) ≠ −1: say 0xAF, said = 1, S[0x20] = 1.
    - else when S[0x21] = 0 and Search (0x51, 0, 1) ≠ −1: said = 1; if s8 S[0xB] = 1 or S[0x15] ≠ 0: when S[0x15] ≠
      0: say 0xB0. Else: Search (0x51, 1, 1) (result unused), S[0x21] = 1; when s8 S[0xE] < 2 or S[0x1E] ≠ 0:
      brenda_say 0x10, S[0x22] = 3; else brenda_say 0xE, S[0x22] = 1.
    No such sector: say 0xB1, said = 1, S[0xA] = 2.
11. **Seeds** (s8 S[0xE] = 1): when Search (0x6E, 1, 0) ≠ −1: if s8 S[0xB] = 1 or S[0x15] ≠ 0: say 0xB7; else
    brenda_say 0x13, and when s8 S[0xA] = 1, S[0x21] ≠ 0 and s8 S[0x22] > 1: S[0x22] = 1. said = 1, S[0xE] = 2. Else
    when Search (0x7C, 1, 0) ≠ −1: if s8 S[0xB] = 1 or S[0x15] ≠ 0: say 0xB8; else brenda_say 0x14. said = 1, S[0xE]
    = 2.
12. For plants i = 1..3 with s8 `state` = 3: not owned(low byte of `sector`): `state` = 2; else when Connected(low
    byte of `sector`): `state` = 2, Plants.
13. **Capacity** (g_processCap after step 12): when g_processCap > `start_cap` and g_processCap > 20: q = 0x50,
    0x53, 0x4D, 0x59, 0x56, 0x5C, 0x5F for g_processCap 45, 50, 55, 75, 80, 85, 110 (other values: q stays as it is:
    999, the loss quote of step 8 or the quote of step 9); say q + 1, q, q − 1 for score > 0, = 0, < 0; said = 1. Else
    when g_processCap < `start_cap`: p = 0x62 when g_plantsOwned < `start_plants`, else 0x65; say p + 1, p, p − 1 by
    the score in the same way; said = 1.
14. When said = 0: r = random(5); say 7 r + score + 3.
15. When s8 S[5] = 1 and `day` > 1: say 0x9D.
16. When s8 S[6] = 1 and List(1) is all owned: S[6] = 2, say 0x92, S[2] = 2. When s8 S[7] = 1 and List(2) is all
    owned: S[7] = 2, say 0x9B, S[2] = 2. List(1) = List(2) = the sectors 28, 29, 38, 48, 49.
17. When not Connected(5): when s8 S[8] ≥ 1: A16 0x92 + L3(d) (10, 15, 20), S[0x24] + 1. Else (connected) when
    S[0x13] = 0: S[0x13] = 1, A16 0x94 + 100 when s8 S[8] = 1, else + 50; say 0x9E, S[8] = 2, S[0x12] = 1.
18. When s8 S[0xC] = 1: when Search (0x77, 1, 0) ≠ −1: say 0xB6, A16 0x94 + 150, S[0xC] = 2. Else S[0x18] + 1, A16
    0x92 + L4(d) (20, 25, 30), and when S[0x18] = 3: say 0xB5.
19. For plants i = 1..3 with s8 `state` = 3: say 0x9A, `state` = 2.
20. For plants i = 1..3 with `state` = 0: when owned(low byte of `sector`): say 0x93 when Connected(low byte of
    `sector`), else 0x99. Then (owned or not) `state` = 1, countdown = random(3) + 2.
21. When s8 S[9] < 2 and Connected(0x1B): unless s8 S[0xB] = 1 or S[0x15] ≠ 0: brenda_say 0xB. S[9] = 2; when `day`
    ≤ L5(d) (12, 10, 8): S[0xF] = 2.
22. When late: say 0xAE. Result 0.

**`day_bonus`**: when S[0xF] ≠ 0: S[0xF] − 1; when it is then 0: say 0x9C, `bonus_due` = 1, `bonus` + 5000. Then
when `bonus_due` ≠ 0: if `bonus` ≠ 0: g_dayIncome + s16 `bonus`, g_balance = `prev_balance` + g_dayIncome −
g_dayExpenses, g_dayNet = g_dayIncome − g_dayExpenses, `bonus` = 0. `bonus_due` = 0.

A result other than 0 ends the game (the caller shows the end; result 4 = home lost, 3 = the blight killed every
tree, 2 = five bad days in a row). Evidence (2026-10-24): DayEndVerdict 0x60680 (called from DayEndScreen? 0x65C39
after DayReplay and the b5 clear; result to 0x93449), crate check 0x62698 (CrateLost 0x62780), DayScore 0x61264,
0x61488 (List n = 5 bytes at 0x95BD8 + 16 n), the bonus 0x615DC (0x65C4B, only after result 0), the day-start
copies 0x2AFA3..0x2B016 in LoadTacticalSector? (0x229AC4, 0x229ACC, 0x229AD4, 0x229AD8, 0x229AF8, 0x229AF4;
`item_told` = 0x22A1F6..0x22A1F8, `bonus_due` = 0x92DD6, `bonus` = 0x92DD8, `idle_day` = 0x22A10A); tables by d:
L1 0x95CA8, L2 0x95C7E, L3 0x95C9C, L4 0x95CA2, L5 0x95C84 (i16). Rating words 0x965D6 (12 bytes each), drawn with
DrawText at (276, 161).
