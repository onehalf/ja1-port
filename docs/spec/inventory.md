# Inventory screen (tactical view, key I)

In the tactical view the key I (a merc with 15+ hp) opens the inventory screen of the chosen merc: the merc's body
with the 12 item entries, the merc slots at the sides, and the item on the mouse cursor. The player moves items
between the hands, the body places, the vest pockets, the ground and other mercs; reloads a gun by putting its ammo
on it; drinks from a canteen; uses a camouflage kit; merges two items into a new one. This page gives the rules of
that screen: what each click does to the game state, the AP costs, and the order of the calls to the front-end
(drawing, sound, text, prompts).

Verified: validate/diff/inv_diff.py runs the original screen loop in Unicorn with scripted mouse and key events and
compares it with the analysis mirror (5 dumps, 2000 fuzzed cases per run, 0 mismatches; the whole data area, the
ordered list of front-end calls and how the run ends are compared). 21 hand mutants of the mirror were caught.

## Data

**Item entry** (12 bytes, layout `item` in defs/types.toml): +0 type (0 = nothing), +1 count (rounds for a gun,
units for a stack; signed byte in the tests below), +2 condition of the first unit, +8..+11 the conditions of the
units 2..5 of a stack (unit u >= 2 at +6 + u). Entry 0 also holds an **attachment** in +3 type, +4 count,
+5 condition (a silencer, a scope; the screen shows it as its own place, rect 12).

**Item type table** (`itemType`, 12 bytes per type): +5 clip size, +6 **max stack** (1 = does not stack).
**Ammo table**: for gun types 1..14, 6 bytes per type, byte 0 = the ammo item type of that gun.

A soldier record (`soldier`) holds 12 entries at +0 (entry k at +12 k). Entry 6 is the vest; entries 7..11 are
the vest pockets. A pocket k is **missing** when the soldier is a merc (slot +0x17A < 8) and the vest type (+0x48)
is below the need of that pocket: 7 and 8 need 0x32, 9 needs 0x33, 10 needs 0x34, 11 needs 0x35. For other soldiers
no pocket is missing.

Screen state (all kept between frames; the evidence line gives the original's addresses):

| name | size | meaning |
|---|---|---|
| cursor | item entry | the item on the mouse cursor |
| swap | item entry | scratch copy of the cursor during a move |
| picked | 8 bytes | per merc slot: this merc already paid the ground pick-up AP in this screen |
| give A, give B | 2 bytes | the "gave an item" pair (0x40 = none), see Gave an item |
| vest owner | byte | slot whose vest pockets the cursor vest carries (0x40 = none) |
| text on | byte | merc remarks on (the T key toggles it through the front-end) |
| shift, ctrl | bytes | set from the keyboard flags at each poll: shift = 1 when flags & 3 is not 0, else 0; ctrl = 1 when flags & 0x104 is not 0, else 0 |
| leave flags | bytes | face leave flag (set to 1), the cursor-under-panel flag (set to 1), the tooltip item (set to 0) |
| voice test (0x92F8B, named merge flag before) | byte | set to 1 when a merge fails; SpeechWait? reads it; always 1 in play |

**Item rects**: 14 places, (x, y) of the top-left corner; a left click hits x - 4..x + 45, y - 4..y + 19, a right
click x..x + 41, y..y + 15 (inside = edges included, signed compares). First hit in the order 0..13 wins. The table
is a game variable: the screen's drawing (body, see Drawing the screen) sets it to the **inventory table** (left
columns below); the tactical view keeps the **default table** (right columns) there, so the screen reads the
inventory table once it is drawn. (The tests above compare both sides on whatever table the dump holds.)

| place | inventory table | default table |
|---|---|---|
| 0 | (59, 108) | (108, 73) |
| 1 | (154, 124) | (203, 89) |
| 2 | (112, 43) | (161, 8) |
| 3 | (59, 66) | (108, 31) |
| 4 | (151, 63) | (200, 28) |
| 5 | (59, 87) | (108, 52) |
| 6 | (208, 53) | (257, 18) |
| 7 | (208, 72) | (257, 37) |
| 8 | (208, 91) | (257, 56) |
| 9 | (208, 110) | (257, 75) |
| 10 | (208, 129) | (257, 94) |
| 11 | (208, 148) | (257, 113) |
| 12 = attachment of entry 0 | (59, 128) | (108, 92) |
| 13 = the ground | (110, 150) | (159, 115) |

**Merc slots**: slot k has its corner at (0, 14 + 42 k) for k = 0..3 and (272, 14 + 42 (k - 4)) for k = 4..7; a
click hits x..x + 47, y..y + 41.

Other rects: the **mouth** (108, 62)-(156, 86), the **body** (112, 82)-(150, 118), the **face**
(70, 28)-(102, 56); the merc text tooltip uses (72, 31)-(100, 55).

## Front-end calls

The screen calls the front-end for everything visible. The order and the arguments are part of the behaviour (the
test compares them). Names used below:

| call | arguments | result |
|---|---|---|
| show / hide | - | mouse cursor on / off |
| cursor(type) | cursor item type | set the cursor picture |
| cursor2(type) | cursor item type | the same after a use (drink, camouflage, put down) |
| click | - | click sound |
| sfx(n, a, b, c) | sound | - |
| w_4b730(0) / w_4b764 | | screen start / end hooks |
| item_info(profile, type, count, value, 1, 1, info) | | 1 = the body must be redrawn |
| item_value(entry) | the entry | the value to show (money) |
| body(soldier) | | draws the body; returns the **armour value** shown |
| tip_merc(n) / tip_item(t) | | tooltips |
| message(text) | | status line text |
| inv_extra(sound, text) | two strings | a short sound file + a text ("" = none) |
| draw_slot(soldier, place, 0, 0x140) | | redraw one item place |
| vestmap(soldier) | | redraw the vest pockets: strategic.md "The vest map", but on this screen the rectangle copied to the screen is (x7, y7) .. (271, 181) (the assignment screen: .. (319, 140)) |
| merc_text(soldier, text, time) | | a remark of the merc |
| slot_redraw(slot, 1) | | redraw a panel slot |
| spend_ap(soldier, ap, fatigue) | | the AP rule of combat.md |
| rest(soldier, amount, 1) | | energy back |
| reserve_ap(soldier) | | sector_start.md |
| select(slot, 0, 0) | | choose another merc; 0 = refused |
| w_2b6a0(soldier) | | the merc's after-screen hook |
| w_55fd8(soldier, 1, 1) | | after an item move |
| w_682ac(1, 190, 318, 197) | | the drop answer (under the panel) |
| w_467e4(0, 0x34, 0x25) / w_28698(0x25) | | before / after a vest change and a failed merge |
| text_toggle("Text ", text flag) | | the T key |
| delay(ms) | | wait |
| put_item(tile, entry bytes, 2, 0, 0, 0) | | put an item on the ground (combat.md) |
| mine_spotted(soldier, tile, ground) / disarm(soldier, tile, ground, 0, 0) / pick_up(soldier, tile, 0) | | combat.md |
| focus(slot, 1) | | centre the map on the merc |
| merge_prompt(text, picture, type a, 10, 20, 16, type b, 20, 50, 1, 1, 0) | | 1 = yes |
| quote(profile, 33 x profile, 6, 200, buffer, 1, profile, 1) / yes_no(buffer, profile, nickname, 0, 0) | | a merc quote |
| stat_practice(soldier, stat, amount, 0) | | experience for a stat |
| random(n) | | 0..n-1 |

**Not enough AP**: an action with cost c is refused when the merc is the selected one (or always, for these
actions: flag 99), combat mode is on and AP left + AP reserve (+0x1A5 + +0x1E2, signed) < c; then the merc says
"doesn't have enough Action Points!" (merc_text, 1200). Otherwise it goes on.

**inv_extra (sound, text)** (the "Empty", "Drank...", "OK" and "No Room" remarks; port_api.md op 237 gives the
numbers). Steps:

1. The sound file (for example `NOROOM.8`) is a member of the archive `INV\EXTRA` (on the hard disk). It plays on
   the speech channel at 22050 Hz, volume 70, once (nothing plays while Sound FX is off).
2. When the text is not empty and Subtitles is on (option byte 0x1163BD ≠ 0): the text shows in the message box of
   panel.md "Message box" (steps 1..4, the darkening table in use = the sector's tileset table 28, frame colour 0x68)
   with the time limit 1000 ms. The rules of step 3 apply: a key, or a button press that starts while the box shows,
   ends it sooner; a button that is already down (the click that caused the remark) does not.
3. Then the game waits while the sound plays, until a mouse button (left or right) is down.

Evidence: PlayInvExtra? 0x25AD8: 0x5C0EC opens `\INV\extra`, ArcSeek 0x534A4 (member), SpeechStream 0x5CC6C (0x5622,
0x46, 0); strlen > 0 and byte 0x1163BD ≠ 0 → ShowMessage? 0x27B98 (text, 0, 0, 1, 1, 0, 1000, 1): the 6th and 8th
arguments are not read (the 5th = 1 = put the screen back, the 7th = the time limit), so the box is the one of
StatusMessage? 0x27D24 with 1000 ms; then `while word 0x96372 ≠ 0` (speech busy): MousePoll? 0x678E8 (1), 0x5CCF8,
stop when g_mouseButtons & 1 or & 2; CloseSpeechArchiveOnce? 0x5C144.

## The loop

Start (soldier S, redraw flag R): give A = give B = 0x40; picked = 0 for all 8 slots; show; armour = the armour
rating of S (combat.md); clear the first 6 bytes of swap; w_4b730(0). Then repeat:

1. If R: v = item_value(cursor) when the cursor type is money (0x6F), else the signed cursor condition;
   if item_info(S profile, cursor type, cursor count, v, 1, 1, 0) then armour = body(S); R = 0.
2. Poll the mouse (x, y, buttons, keyboard flags); shift / ctrl from the flags (0 or 1, see Screen state).
   Only this poll sets shift / ctrl: the polls inside a wait (wait up, right button waits) leave them as they are.
3. Key waiting: read it (a 0 byte = an extended key: read once more and ignore). Lower case is made upper case.
   Esc: the cursor empty = leave; else message "Better put that down first!". T: toggle the text flag.
4. Left button up:
   - Right button down: find the place (0..12, right-click rect). If one: click; item_info for it (place 12 = the
     attachment, its value = condition x 10 for money; else item_value(entry), 0 = the condition), with info = 1;
     if that returns 1, armour = body(S). Then (whatever item_info returned) wait until the right button is up.
     After that, for a hit and for no hit alike, wait until the right button is up once more (one more poll at least). If the mouse y + the panel offset > 181 (under the panel): when the cursor holds an item, set the
     under-panel flag and store the answer of w_682ac(1, 190, 318, 197); leave. Else next poll.
   - Else the tooltip: tip_merc(selected slot + 1) over the face tooltip rect, else tip_item(tooltip item).
5. Left button down, in this order (each case ends this poll; "wait up" = poll until the left button is up):

   **Every wait (left or right button) polls at least once**: poll, then test the button, repeat while it is down.
   It does not test the button before the first poll, so a wait with the button already up still uses one poll
   (one mouse event). This holds for every "wait up" and "wait until the right button is up" in this file.
   - Mouth rect and not refused for 5 AP: Drink, wait up.
   - Body rect and not refused for 10 AP: Camouflage, wait up.
   - Face rect: click; cursor empty: wait up, tooltip item = 0, hide, face flag = 1, leave. Else message
     "Better put that down first!", wait up.
   - Mouse x < 48 or x > 271: Merc slot click.
   - Else: Item click.

After the loop: the skill bonus of S (+0x1D0) = 1 when its level (+0x192) > 3, else 0; with entry 4 type 0x2B
(extended ear) it gets + 5 - (100 - its condition) / 20 (signed, division toward 0). w_2b6a0(S); w_4b764.

### Drink (mouth rect)

click. Only cursor types 0x41 (canteen) and 0x4F (beer). A canteen must be one unit (count 1), else message "Can only
drink from one item at a time!". Then click; c = cursor condition: sips n = 20 when c > 20, else c for beer, else
c - 1 (a canteen keeps the last). n <= 1: inv_extra("empty.8", "Empty"). Else spend_ap(S, 5, 0), slot_redraw(S slot),
rest(S, (100 - energy) x n, 1), reserve_ap(S), inv_extra("hmm.8", "Drank..."), cursor condition -= n. Then, after both branches (also after n <= 1 "Empty"), if the cursor condition (signed)
is now <= 0: beer waits 1 s (delay 1000), the cursor is cleared, inv_extra("empty.8", "Empty"). At the end (also after the "one item" message): cursor2(cursor type).

### Camouflage (body rect)

click. Only cursor type 0x47 (camouflage kit), one unit, else message "Can only use one camouflage kit at a time!".
Then: click, camouflage +0x1B1 += 3, spend_ap(S, 10, 0), slot_redraw, armour = body(S), inv_extra("hmm.8", "OK"), kit
condition -= 20; at <= 0 the cursor is cleared and inv_extra("empty.8", "Empty"). At the end cursor2.

### Merc slot click

For the first slot k whose rect holds the mouse (none: wait up, nothing else), O = merc k:
- If O is out of action (+0x1BB), or k is S itself, or both O and S have hp below 15: when k is not S, wait up;
  when k is S: with an item on the cursor message "Better put that down first!" and wait up, else wait up and leave.
- With an item on the cursor and k not give A: when S has 15+ hp, it needs 6 AP (refused: wait up, stop); when O has
  15+ hp, O needs 6 AP the same way.
- select(k, 0, 0) = 0: wait up, stop.
- With an item on the cursor and give B = none: give B = S slot. The skill bonus of S as after the loop; w_2b6a0(S);
  S = merc k; armour = body(S); wait up. The screen goes on with the new merc.

### Item click

Find the place p (left-click rects). With ctrl held on a place: Unstack the 12 bytes at S + 12 x p (for p = 12
and 13 these are not items: soldier bytes +0x90 and +0x9C; the original then may shift ~130 bytes of the merc,
issue #2; the port does the same on its own record, the test leaves these two cases out) (when that changes it: hide,
draw_slot(S, p), show) and treat as no hit. No hit: wait up. p = 13: the cursor empty = Pick up, else Put down.

Reload: p < 12, the entry is a gun (type 1..14) with 0 rounds, the cursor is one unit (count 1) of that gun's ammo:
rounds = the gun's clip size; stat_practice(S, 3, 1, 0) when S +0x1D5 > 0; sfx(15 for types below 8, else 14,
0x2B11, 50, 1); the cursor is cleared; hide, draw_slot(S, p) (and place 12 for p = 0), cursor(type), show;
merc_text(S, "has reloaded.", 800) when text is on; wait up. Done.

Else copy the cursor to swap and ask **May it go** (below) with the swap item; -1 = wait up, done; 0 = inv_extra("huh.8",
""), wait up, done. Otherwise S +0xB4 (i32) = 2000, +0xD0 (i16) = 0xC41, +0x130 = 1, and:
- p = 6 and the cursor is a vest with pockets (0x32..0x35): when the vest owner is another merc (< 0x40, not S),
  swap the entries 7..11 of S and the owner. Else (the owner < 0x40 becomes none) when S's vest type is bigger
  than the cursor's: w_467e4(0, 0x34, 0x25), cursor = entry 6, entry 6 = swap, for every
  missing pocket that holds an item (type not 0; empty missing pockets are skipped, no put_item for them): put its
  item on the ground (put_item on S's tile) and clear it, w_28698(0x25), and Gave an item; the
  pocket move below is skipped.
- p = 6 otherwise: vest owner = S slot.
- p < 12: unless skipped or p is a missing pocket: Pocket move.
- p = 12 with an item in entry 0: Gave an item; the cursor gets the attachment (+3, +4, +5 to type, count,
  condition; its +3, +4, +5 and +8..+11 cleared). For the answer 1: the attachment = the first 3 bytes of swap.
  For 2 or more: inv_extra("hmm.8", ""); answer 4 clears the attachment, other answers clear the cursor's first 3 bytes.
Then swap is cleared. Redraw: for p = 6: hide, draw_slot(S, 6), cursor(type), show, vestmap(S), click. Else hide,
draw_slot(S, p) + click unless p is a missing pocket, draw_slot for places 0 and 12 when p is 0 or 12,
cursor(type), show. Then w_55fd8(S, 1, 1); when the armour rating of S changed, armour = body(S). Wait up.

### Pocket move (p = 0..11)

- Cursor type = entry type, it stacks (max stack > 1), p is not 3: click. Shift: exchange cursor and entry
  (cursor = entry, entry = swap), Gave an item. Else when max stack < entry count + cursor count (or p = 3): no
  room (inv_extra("noroom.8", "No Room") for places 2..6, else no_room(entry, cursor)). Else Stack join (entry += cursor), the
  cursor is cleared, Gave an item.
- Entry count <= 1 or the entry does not stack: on p = 3 with sun goggles (0x27) of 2+ units on the cursor:
  inv_extra("noroom.8", "No Room"), done. Else exchange, Gave an item.
- Cursor empty: shift = exchange (no Gave an item). Else the cursor = the entry with count 1 and +8..+11 cleared;
  Take one from the entry (entry count 0 = cleared).
- Else exchange, Gave an item.

**No room (entry, cursor)** (pocket move to places 0, 1 and 7..11, the stack would grow past its maximum): n = the
maximum stack of the entry's type (item table +6) - the entry count (signed). n < 1: inv_extra("noroom.8", "No
Room"), nothing moves. Else n times: take the cursor's last unit (its condition byte at cursor + 6 + cursor count;
that byte becomes 0; cursor count - 1), the entry count + 1, then that condition goes to entry + 6 + entry count. So
the entry is full and the rest stays on the cursor (no "No Room"). In play a stack holds at most 5 units (conditions at
+2 and +8..+11), so the bytes stay inside the 12-byte records; larger counts are not described.
Evidence: PlayNoRoom? 0x53A34 (EAX entry, EDX cursor; table byte 0x94DFE + 12 type; called by 0x5454C and the
assignment screen 0x499AC). validate/diff/inv_diff.py and assign_items_diff.py run it unstubbed.

Stack join (dst, src): n = src count (signed); n < 1: nothing. For i = 0 .. n - 1 (0-based): the condition is
src +2 for i = 0, else src +7 + i (so the second unit is src +8). It goes to dst +2 when i = 0 and the dst count is 0;
in every other case to dst + 7 + (dst count, signed byte), read anew each time: a negative count writes before the
entry (still inside the soldier record). Then dst count + 1.
Take one (entry): count <= 0 nothing; a non-stacking type or count 1: clear the entry. Else (n = count) +2 = +8; for k = 1 .. n - 2:
byte +7+k = byte +8+k; then byte +6+n = 0; count - 1. Exactly these bytes and no others: with n < 5 the bytes after
+6+n stay as they are, and with n > 5 (a count above the 5 condition bytes) the moves and the clear run on into the
next 12-byte entry (offsets measured from this entry).
Unstack (ctrl, entry): only a stack of 2+ (max stack not 1, count not 99, count >= 2): remember the first
condition, Take one, then count + 1 with that condition at +6 + count (it moves to the end).

### Put down (p = 13, an item on the cursor)

Gave an item; picked[S] = 1; click. A vest with pockets (0x32..0x35) on the cursor: when the vest owner < 0x40, each
of the owner's entries 7..11 goes to the ground (put_item on S's tile, then cleared); vest owner = none. The cursor
goes to the ground (put_item) and is cleared; armour = body(S); cursor2; wait up.

### Pick up (p = 13, empty cursor)

(pick_up(S, tile, 0) = sector_start.md "Take ground item" (tile, keep 1): the ground item goes to the cursor; then
for the story items 0x79, 0x86, 0x5F, 0x6E a once-per-game merc quote; evidence: tools/ai.py pick_up, mirror of
PickUpItem? 0x3C16C.)

g = the ground item index of S's tile (tile byte 6); a buried item (its +6 = 0xFF and its condition < 0) counts as
none (0xFF). The first pick-up of this merc in this screen: when g is none or the merc is refused for 4 AP: wait up,
done; else spend_ap(S, 4, 0), picked[S] = 1. Then mine = mine_spotted(S, tile, g); r = disarm(S, tile, g, 0, 0)
(signed 32-bit). Note: at the start only the exact value 0xFF is "none": a byte 6 of 200..254 (not 0xFF) is used
as g as it is (the fuzz in the test sets 0xD0), for the buried test, the 4 AP test, mine_spotted and disarm; its
bytes lie past the 200 entries as in combat.md Mine spotted, "G for g of 200 or more". r > 0: body(S), pick_up(S, tile, 0); when mine, the sector's mine count - 1; cursor(type); armour =
body(S); wait up. Else: when the tile has no ground item now (a mine went off; "no ground item" = tile byte 6 is
200 or more, not only 0xFF): the sector's mine count - 1,
focus(S slot, 1), leave the screen. Else armour = body(S); when mine, the tile's ground item condition = its
negative (it stays known as a mine); wait up.

### Gave an item (slot n)

Give B none: nothing. n is neither give A nor give B: give B pays when it has 15+ hp (spend_ap(B, 6, 20), picked[B]
= 1, slot_redraw(B)), then n the same when it has 15+ hp; give A = give B; give B = none. Else (n = give A: give A
= none); give B = none.

## May it go (item X onto place p of S)

The answer is 1 (go on), 0 (not allowed: "huh") or -1 (cancelled).

- X empty: 1.
- X a vest (0x31..0x35): p = 6: 1; p = 0 or 1 (or negative): 1 when the vest owner has nothing in the entries 7..11
  (owner none: 1), else message "Vest is not empty!" and 0; other places: 0.
- p 2 (head): 1 only for 0x23..0x26 (helmets). p 3 (eyes): only 0x27..0x28, sun goggles (0x27) only one unit.
  p 4 (ear): only 0x2A..0x2B. p 5 (body): only 0x2D..0x30. p 6: only vests (so 0 here). p 7..11: 0 for vests, else
  1. p 0, 1 and other values: 1.
- p 12 (onto the item in entry 0, type H): 0 when X has an attachment (+3 != 0), when X stacks and has 2+ units,
  or when H stacks and has 2+ units. Then the first row of the **merge table** with (X type, H type) decides:
  result type 0 = 1 (attach: the item becomes entry 0's attachment). No row: 0. Else merge_prompt("Attempt to
  PERMANENTLY merge these two items?", "merge.pcc", X type, 10, 20, 16, H type, 20, 50, 1, 1, 0): no = -1. Then the
  merge by the row's kind gives r; r < 2 is the answer. Else H's type = the result type; when the result type's
  clip size is 99 the count of entry 0 becomes 99. The answer is r.

Merge kinds (X = the cursor copy, H = entry 0; conditions as signed bytes; "/" divides toward 0):
- 0: r = 3; stat_practice(S, 4, 2, 0).
- 1 (steel + gun): skill = mechanical (+0x190) - (100 - dexterity (+0x18C)) / 2; roll = random(100). roll < skill:
  r = 2, stat_practice(S, 7, 10, 0). Else the merge fails: u = random(3), d = roll - skill; u = 0: both lose d / 2,
  u = 1: H loses d, u = 2: X loses d (condition minus the loss, byte); an item at condition <= 0 is cleared
  (type, count, condition). Then hide, cursor(0), w_467e4(0, 0x34, 0x25), show, merge flag = 1, quote(...), yes_no(
  the quote text, profile, nickname), w_28698(0x25), stat_practice(S, 7, 5, 0); r = 1.
- 2 (compound 17 + armour): roll = random(100) (not used); r = 2, stat_practice(S, 7, 10, 0).
- 3 (explosives + detonator, molotov parts): skill = explosives (+0x18F); roll = random(100); r = 2;
  stat_practice(S, 6, 10, 0); H condition = ((h + x) / 2 + h x / 100) / 2 + skill - roll, clamped to 10..100.
- 4 (kit + kit): r = 4. h + x <= 100: H condition = h + x, X condition = 0. Else X condition = h + x - 100, H
  condition = 100 and the attachment bytes of entry 0 get (H type, 99 when the type does not stack else 1, the
  rest).
- Other kinds: r = 0.

The merge table (82 rows: X type, H type, result, kind) is in the executable's data; the port gets it from the
host. The order matters: the first match wins. Summary:
silencer / sniper scope on guns, note on lab coat = attach (result 0); paper + paper / taped paper = kind 0;
chunk of steel + gun = modified gun (kind 1); compound 17 + helmets / kevlar = treated (kind 2); explosives /
plastic explosives + detonator, the molotov and Eagle grenade chains = kind 3; first aid / medical / locksmith /
tool / camouflage kit + the same kit = kind 4.

Evidence: tools/invscreen.py (mirror of UseInventoryItem? 0x53A9C, MergeItemsPrompt 0x4A838, MergeApply 0x4A4DC,
MsgVestNotEmpty 0x4A7BC and helpers), validate/diff/inv_diff.py. State addresses: cursor 0x222B50, swap 0x222B68,
picked 0x28EF30, give A / B 0x28EF38 / 0x28EF39, vest owner 0x92E40, text flag 0x1163BD, shift 0x284F21, ctrl
0x284F1F, tooltip item 0x93EA8, face flag 0x284F13, under-panel flag 0x955C9 + answer 0x229D18, panel offset
0x229C48, merge flag 0x92F8B, item rects 0x949F2, slot rects 0x93686, merge table 0x94AAA, ammo table 0x94C32.

## Drawing the screen (body)

The front-end call body(S) draws the whole inventory screen of merc S into the panel buffer (320 x 200, pitch 320;
panel.md), marks the 8 mercs for the hand-over rule, redraws their panel slots, copies the panel buffer to the screen
and returns the **armour value** (the armour rating of S, ai.md "armour"). Drawing primitives (element, text, number,
remapped draw, darken, table 0, table 29 = the darken table, the face buffers, Drawing one slot) are those of
panel.md. All drawing below goes into the panel buffer unless it says "screen". Numbers are signed 32-bit; "/"
truncates toward zero. **opt** = the world's `opt_onscreen` (the screen's start sets it to 1).

Static data (from the executable; the values are given here):

- Elements of the panel sheet (x, y, w, h): 0x2E (172, 93, 47, 17), 0x2F (276, 17, 43, 17), 0x30 (221, 93, 51, 18),
  0x31 (220, 29, 52, 19), 0x43 (220, 111, 52, 20), 0x69 (272, 17, 47, 17), 95 (131, 87, 20, 8), 99 (78, 149, 20, 8),
  100 (98, 149, 20, 8). "Draw element e at (x, y)" = panel.md Element (sheet pixel 0 not written).
- **Item pictures**: CD `DAT/INV.DAT` is an offset bank (u32 count N, then N u32 offsets relative to the end of
  the offset table). Picture p (= byte +4 of the item type) is 16 rows of w bytes (pitch w) with w = 42 when
  p ≤ 20, 30 when p ≤ 90, else 16. **Draw picture p at (x, y) with colour c**: save table 0 byte 0x17, set it to c,
  draw the picture remapped with table 0 (v = table0[pixel], written unless v = 0) at (x + dx, y) with dx = 0 / 6 /
  13 for w = 42 / 30 / 16, restore the byte.
- Item type bytes used (item type table, 12 bytes per type): +4 picture, +6 max stack, +7 and +9 (flags of a
  weapon / tool that can be "useless"), +0xA (0 = the type has no condition).
- **Condition bar** (value v, x, y): h = 10 - v / 10; if h ≠ 0, copy the 2 x h pixels at sheet (266, 65) to (x, y)
  (all pixels written). (v ≥ 110 makes h negative: the original then runs a near-endless copy; never fed.)
- **Strip boxes** k = 0..6: box x = 0, 49, 94, 139, 184, 229, 272; picture x = 1, 49, 94, 139, 184, 229, 273;
  entry shown = 0, 7, 8, 9, 10, 11, 1.
- **Buttons** i = 0..5: x = 49, 86, 123, 160, 197, 234, y = 1, labels `done`, `trav`, `inv`, `map`, `opt`, `abort`.

World values read: `opt`, the `small panel` flag (i32), the current sector (byte 0..59) and its enemies byte (byte
1 of the sector's 9-byte state record, sectorState.enemies; spec fixed 2026-10-23), the **time meter width** (i32), the **blink** byte (in/out),
`tiles` (8 bytes per tile, byte 6 = ground item index, 255 none), `ground_items` (200 x 12 bytes; byte 6 = 255 =
buried), the soldier records (merc k = record k; in/out), the face buffers.

Steps (S = the merc, slot = S +0x17A):

1. **Loader result** = load MAIN.DAT `TSTRIP.PCC` at (0, 0). (Load = PCX rows of the member into the panel buffer
   at the given corner, pitch 320; result 0 = loaded, -1 = no such member, nothing drawn. Member names in upper
   case. The loader result is a game variable that each load of this list overwrites.)
2. **Time meter**: if the width w > 0 and opt < 3: copy w x 8 pixels from sheet (131, 87) to (286, 3) (all
   pixels written).
3. **Sector box**: if opt ≤ 2: e = 100; if the sector's enemies byte ≠ 0: blink = blink xor 1, e = 100 if blink
   = 1 else 99. Draw element e at (15, 3). Number sector + 1 with 2 digits (`^` padded) at (21, 4), fg 0xFF,
   shadow 1, font 0.
4. **Buttons**, for i = 0..5 in order: text of the label centred at (x + 19, 4), fg 0x2A, shadow 3, font 1. Then
   save table 0 byte 0x1B, set it to 0x70, draw the 19 x 12 pixels at sheet (242, 2) remapped with table 0
   (written unless the result is 0) at (x, 1) and the 18 x 12 pixels at sheet (242, 2) at (x + 19, 1); restore
   the byte.
5. Item rects = the inventory table (Data). A game byte, the **inventory drawn** flag, = 1.
6. Load `INVAMB.PCC` at (48, 14); `BODY<n>.PCC` at (110, 62) with n = S +0x17C in decimal (e.g. `BODY3.PCC`);
   if S +0x1B1 ≠ 0, `CAMO<n>.PCC` at (110, 77); `BSTRIP.PCC` at (0, 182). Each sets the loader result.
7. Text: the nickname (S +0x16E, zero-terminated) at (112, 27), not centred, fg 0x0E, shadow 5, font 1.
8. If slot < 8: copy the 28 x 25 face (face buffer of slot, from (2, 2), pitch 32) to (72, 31) (all pixels);
   text `SALARY:` at (112, 36), fg 0xB5, shadow 5, font 0; number u16 S +0x112 with 5 digits at (140, 36), fg 0x28,
   shadow 1, font 0.
9. armour = armour rating of S. Number armour with 3 digits at (125, 88), fg 0xFF, shadow 1, font 0; text `%` at
   (137, 88), fg 0xFF, shadow 1, font 1.
10. For place k = 0..13: unless pocket k is missing (Data), draw **item place** k (below).
11. The stat numbers n = 2..9 of the stat popup (panel.md "Stat popup" step 2: shadow pass and colour pass) with
    the picture corner (173, 18).
12. **Marks**, for k = 0..7 (O = merc k):
    - if s8 O +0x183 (hp) < 15: O +0x1AC = 3.
    - O +0x1BB = 0. out = (O +0x182 = 0) or (O +0x17E ≠ 0) or (s8 hp < 1) or tile distance (s16 S +0x114,
      s16 O +0x114) > 2 (tile distance = the larger of |row difference| and |column difference|, row = tile / 56,
      column = tile - 56 row).
    - if not out and slot ≠ k: out = (sight line (S, s16 S +0x116 + 2, s16 S +0x118 + 4, s16 O +0x114,
      s16 O +0x116 + 2, s16 O +0x118 + 4, 1, 14, 0x40) = 0) (port_api.md op 49 sight_line).
    - if out: O +0x1BB = 1.
    - **slot redraw** k: if opt < 2: if O +0x17F = 0, draw slot O +0x17A (panel.md Drawing one slot); if it is 1,
      the switch view (panel.md "The switch view").
13. **Strip**: if slot < 8 and (opt < 3 or the small panel flag ≠ 0): strip box k for k = 0..6 (below).
14. Copy the whole panel buffer (64000 bytes) to the screen. Return armour.

**Item place** k of S (x, y = item rect k of the inventory table): shadow colour c0 = 10, background element 0x30,
bar at (x + 46, y + 4), dy = 1 for k < 7; c0 = 0xE2, element 0x31, bar at (x + 46, y + 5), dy = 2 for k ≥ 7.
buried = 0.

- k = 13 (the ground): g = tile byte 6 of tile s16 S +0x114 (a tile outside 0..3135: the original reads the bytes
  after the tile map; the port takes g = 255 and the test leaves such cases out). If g ≠ 255 and ground item g has byte 6 = 255 and
  s8 byte 2 < 0: g = 255. If g = 255: t = count = cond = 0; else t, count, cond = bytes 0, 1, 2 of ground item g
  and buried = (byte 6 = 255).
- k = 12 (the attachment): if s8 S +5 ≤ 0: S +3, +4, +5 = 0. t = S +3, count = S +4, cond = S +5.
- k ≤ 11: if s8 S +2 ≤ 0 (the condition of **entry 0**, whatever k is): S +0, +1, +2 = 0. t, count, cond = bytes
  0, 1, 2 of entry k. If t ≠ 0 and type t's byte +0xA = 0: cond = 100. With (ax, ay) = item rect 12: if k = 0,
  draw element 0x43 at (ax, ay) when t = 0, else element 0x31 at (ax, ay) and the condition bar (s8 S +5,
  ax + 46, ay + 5). Then draw the background element at (x, y).
- If t = 0: for k ≤ 11 the condition bar (s8 cond) at the bar; stop.
- If k < 13: condition bar (s8 cond) at the bar, then picture of t at (x + 1, y + dy) with colour c0. If k = 13:
  picture at (x + 5, y + dy) with colour 0.
- n = s8 count. If n < 99 and k ≠ 12: if max stack (+6) < 2 or t = 0x6F: unless k = 13 or t = 0x6F, text of |n|
  as 2 digits with a leading zero (`%02d`) at (x + 2, y + dy + 10), fg 0x29, shadow c0, font 0. Else (a stack)
  unless k = 3: text of |n| right-aligned in 2 characters (space padded, `%2d`) at (x + 35, y + dy + 10), fg 0xED,
  shadow c0, font 0.
- If n < 0: text `jammed` centred at (x + 22, y + dy + 6), fg 0xED, shadow 1, font 0.
- If buried: text `buried` (same place and colours). Else if cond as an **unsigned** byte < 10 and (type byte +9
  ≠ 0 or byte +7 ≠ 0): text `useless` (same place and colours).

**Strip box** k (y = 183, e = the entry shown, x / px = box / picture x): if slot > 7, nothing. If s8 condition
(byte 2) of entry e ≤ 0: bytes 0..2 of entry e = its bytes 3..5, bytes 3..5 = 0 (a worn-out item gives way to what
is under it). If k = 0 and entry e's type = 0: draw element 0x2E at (x, y); else draw element 0x2F (k = 1..5) or
0x69 (k = 0, 6) at (x, y), and if pocket e is missing, darken the 42 x 16 pixels at (x, y) with table 29 (each
pixel p becomes table29[p]). If type t ≠ 0 and pocket e is not missing: picture of t at (px, y) with colour 8;
n = s8 count; if n < 99 and t ≠ 0x6F: if max stack > 1, `%2d` of |n| at (px + 34, y + 11), fg 0xED, shadow 0x0A,
font 0, else `%02d` of |n| at (px + 1, y + 11), fg 0x29, shadow 0x0A, font 0; then, still only when n < 99 and
t ≠ 0x6F, if n < 0, `jammed` centred at (px + 21, y + 6), fg 0xED, shadow 1, font 0. Then (still for t ≠ 0, not missing) if s8 condition < 10 (signed
here) and (byte +9 or +7 of t ≠ 0): `useless` at the same place. Last: unless pocket e is missing, darken (the drawing
always; world_load.md op 244 only when its d ≠ 0)
(x, y) with width 46 for k = 0 and 6, 42 for the others, height 16.

All texts here use mode 300 (panel.md Text). Original bug kept: the k ≤ 11 rule clears entry 0, not entry k, when
entry 0's condition is 0 or less.

Evidence: tools/invdraw.py (mirror of DrawMercBody? 0x555DC, DrawItemSlot? 0x4AAD4, 0x55FD8 / DrawItemCondition?
0x55BE4, the picture 0x46750, the bar 0x4B064, the pull-up 0x466D0, BlinkUi? 0x33768, the meter 0x3DB3C, the
button label 0x35AB8 and shade 0x254F0), validate/diff/invdraw_diff.py: 1000 cases on 5 dumps (fuzzed entries,
attachment, vest, body number, camouflage, salary, ground item, the other mercs' hp / sector / assignment / tile,
noise in the panel), compared: panel, screen, 48 soldier records, table 0, item rects, blink, the drawn flag, the
loader result, the armour value: 0 mismatches; 3 of 4 hand mutants caught (the 4th, slot > 8, needs a non-merc).
Not covered by the fuzz: opt ≠ 1, the small panel flag, a time meter width ≤ 0, O +0x17F ≠ 0. Addresses: item
rects 0x949F2 (inventory table 0x94A3A, default 0x94A72), drawn flag 0x955CA, loader result 0x229D18, blink
0x96068, time meter width 0x22A0FB, small panel flag 0x92E30, INV.DAT bank 0x1163CC.

### Front-end interface: ja1f_draw_inventory

port/ja1front exports this for validate/diff/invdraw_diff.py `--port` (conventions of panel.md "Front-end
interface"; Ja1fPanel as there: `slot` = the record index k of S (0..7), `opt_onscreen` = opt, the soldier records
and table 0 in/out, the faces as there):

```c
typedef struct {
    Ja1fPanel panel;           /* slot = S's record k; the slot redraws of step 12 use the same values */
    int32_t tileset;           /* 0..9, as ja1f_draw_slot */
    uint8_t sector;            /* current sector 0..59 */
    uint8_t enemies;           /* that sector's enemies byte */
    uint8_t blink;             /* in */
    uint8_t sight[8];          /* the sight line answer (0 = none) for merc k, used only when step 12 asks for it */
    int32_t small_panel;
    int32_t time_w;
    const uint8_t *tiles;      /* 3136 x 8 */
    const uint8_t *ground;     /* 200 x 12 */
} Ja1fInventory;

typedef struct {
    int32_t armour;            /* the return value of body */
    int32_t load_result;
    uint8_t blink;
    uint8_t drawn;             /* the inventory drawn flag (1) */
    uint8_t item_rects[56];    /* 14 x (i16 x, i16 y), little-endian: the table step 5 sets */
} Ja1fInventoryOut;

/* Draw the inventory screen of merc v->panel.slot (steps 1..14) into panel and screen (320 x 200 each, in/out). */
int32_t ja1f_draw_inventory(const char *iso, const char *exe, const Ja1fInventory *v, uint8_t *panel,
                            uint8_t *screen, Ja1fInventoryOut *out);
```
