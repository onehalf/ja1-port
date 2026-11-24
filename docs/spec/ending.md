# End of the game and the SAB script player

When the game ends, the original plays a short scripted show: pictures, moving sprites, a voice with subtitles,
sound effects, music and two helicopter films. The show is a **SAB script**: a list of 44-byte records that a small
player runs one by one. This page gives the game-end rules and the player, for the three end scripts
`WINGAME.SAB`, `LOS1GAME.SAB` and `LOS2GAME.SAB` (members of `DAT\ENDSAB.DAT`). Only the record kinds ("ops") that
these three scripts use are specified; the other ops are listed at the end without a description.

Conventions. All record words are signed 16-bit (s16). Arithmetic is 32-bit signed unless stated; "/" truncates
toward zero. A buffer pixel (x, y) of a buffer with width W is the byte at offset y · W + x of the buffer memory
(no clipping anywhere: the original writes wherever the offset points). "Low byte" = the value modulo 256.

## Game end

Read from the asm (the win variables of step 1 and the rating are diff-tested: rating_diff.py). The world field `leave_reason` (the end code) decides how the game ends. The
play loop, after the tactical loop of a day:

1. When the day was aborted, the end code is not looked at (other path).
2. When the end code is 0, the day's end screens run first (the day replay prompt and "The day's verdict",
   strategic.md); the verdict can set the end code to 2, 3 or 4. When the end code is still 0, the game goes on.
3. By the end code:
   - **1** (won: Santino spoke in sector 0 and the squad left it, soldier_update.md): a wait of 2000 ms; the **win
     sequence** (below); the performance rating text ("Your overall performance rating is ..." into the text
     buffer); then the program copies that text as its exit message and **ends** (exit code 0). The program does
     not come back (the code after it, the LOS1 show, is not reached after a win).
   - **2** (more than four bad days in a row): play script `DAT\ENDSAB.DAT` / `LOS1GAME.SAB` (the "Playing a
     script" entry below), then stop all sounds, then stop the music (music.md "Stop the music").
   - **3** (the tree blight) and **any value above 4**: the same with `LOS2GAME.SAB`.
   - **4** (the home sector was lost): no show.
4. For 2, 3, 4 and above: the Game Over screen ("The game-over screen"), a new game is
   prepared (NewGameInit with the current difficulty), the accounts' A.I.M. link byte = 0, the main sound bank is
   loaded again, the end code = 0, and the game goes back to the main menu.

**Win sequence.** With S[i] = byte i of the world field `story` (strategic.md):

1. Script variables 0..3 (see "Variables"): v0 = 1 when s8 S[0x0B] = 1 or S[0x15] ≠ 0, else 0. v1 = 1 when
   s8 S[0x0B] > 0, else 0. v2 = 1 when s8 S[0x0E] = 2 and S[0x1E] = 0, else 0. v3 = 1 when Squad search (0x5F,
   take no) ≠ −1 and Squad search (0x7B, take no) = −1, else 0 (strategic.md "Story events at the day start": all
   8 records, no item store). The searches run in this order; the second only when the first found the item.
2. When a MIDI driver is installed: end the playing sequence, playing theme = −1 (no bar check). Then load the
   music file `MISC\intro` (music.md "Load a file"; no theme is started here).
3. Abort flag = 0, start the timer, open `DAT\ENDSAB.DAT` / `WINGAME.SAB` and play it ("Opening a script",
   "The step loop"). The result is 1 when the script ran to its end, 0 when it was stopped (Escape or a film
   abort).
4. Only when the result is 1: the film flag = 1 (during films any key stops the film); the quote text of op 29
   with quote 79, colour 0x50, shadow 7 (always drawn: 79 ≥ 77); the film cue reset (op 59 with 0, 0, empty name);
   the film `INTRO05.FLC` with the arguments (name, 0, 15, 1, 0) (see op 10).
5. Film flag = 0; the credits ("The credits (after the win)"); when music is on (driver
   and the Music option): sequence volume 0 over 3000 ms; then the speech stop, the 8 effect samples end, the cry
   stop (as in "Stops").

## Files

All archives are Sir-tech archives (speech.md "Archives"); a member is found by its name. An archive name is
opened from the game's hard-disk folder when the file is there, else from the CD.

- `DAT\ENDSAB.DAT`: the scripts. Member data = a whole number of 44-byte records.
- `DAT\ENDGRAPH.DAT` (opened by op 54, the **graphics archive**): the pictures (PCX) and the films (FLC).
- `DAT\ENDSOUND.DAT` (opened by op 56, the **sound archive**): voice and effect samples (`.8`, raw 8-bit, sound.md).
- `EDT\INTQUOTE.EDT` on the CD (the INT quote category, speech.md): the subtitle texts, 200 bytes per record,
  encoded (speech.md "Text encoding").

## Script records

| Offset | Size | Field |
|---|---|---|
| 0 | u16 | op |
| 2 | s16 | a0 |
| 4 | 18 | name: a zero-terminated text (file or member name) |
| 0x16 | s16 | w0 (never read) |
| 0x18 + 2 (k − 1) | s16 | wk, k = 1..10 (w1 at 0x18 ... w10 at 0x2A) |

The player keeps one record copy, the **current record**. "Read a record" = read the next 44 bytes of the archive
file at its current position into the current record. There is no end-of-member check: reading past the member's
end reads the bytes of the archive that follow; at the end of the file the read gets nothing and the current record
stays as it was. The end scripts do not reach either case.

## Player state

**Variables.** 26 s16 values v0..v25. Opening a script sets v4..v25 = 0; v0..v3 keep their values (the win
sequence sets them before WINGAME; the LOS scripts do not read them). At program start all are 0.

**Screen.** The visible 320 × 200 picture, 64000 bytes, width (pitch) 320. Its colours are the VGA colours (the
DAC), which only the fades and the films change in the end scripts.

**Work buffers 0..3.** Each has fixed memory and a width W and height H (u16). The memories do not overlap.

| Buffer | Memory (bytes) | W × H at program start | Use in the end scripts |
|---|---|---|---|
| 0 | 64000 | 320 × 200 | the background picture (op 42); "buf0" |
| 1 | 96960 | 320 × 303 | the frame being built (sprites are drawn here, ops 15/16); copied to the screen by ops 17 and 23; "buf1" |
| 2 | 51200 | 320 × 160 | not used |
| 3 | 96000 | 640 × 150 | the sprite sheet (op 50 loads a picture, op 26 cuts sprites from it) |

Op 52 changes only W and H (not the memory). H is never read by the player. Nothing else in the game changes this
table, so the values are those of program start or of the last op 52 of an earlier script. Several operations work
on exactly 64000 bytes of a buffer whatever its W and H ("64000-byte copy").

**Draw target** (only op 29 uses it): the screen with pitch 320; set to the screen at each open. (Op 35 can switch
it to buffer 1; the end scripts do not use op 35.)

**Sprites.** A table of sprite records, index = a0 (the end scripts use 0..49; the table is all zero at program
start and keeps its values between scripts). A sprite has: a **source** (a place in a buffer's memory: buffer k and
offset o), w, h (s16), a source pitch p, a **has-place** flag and a **place** (px, py) (s16). Sprite pixel (i, j) is
the byte at memory offset o + j · p + i of buffer k, read at the time of drawing (so a new picture in the buffer
changes the sprite).

**Palette slots.** 4 slots of 256 entries, 4 bytes per entry: R, G, B (6-bit values 0..63) and a fourth byte X. At
program start all bytes are 0. The player never writes X. Slot s = the 1024 bytes of entry 0..255 of slot s.

**Flags and saved values** (start values = program start; the player keeps them between scripts):

| Name | Start | Set by |
|---|---|---|
| key colour | 0 | op 51 (not used by the end scripts) |
| centre flag | 0 | op 27 (not used) |
| copy flag | 1 | op 48 (not used) |
| saved rectangle (x, y, w, h) | (0, 0, 320, 200) | op 17 |
| condition | 1 | set to 1 at each play start; ops 18, 19, 38, 39, step loop |
| saved condition | 1 | op 40 |
| in-call flag | 0 | ops 38/39/40 |
| return position | — | op 40 |
| last key | 0 | set to 0 at each play start; key polls |
| abort flag | — | set to 0 by both callers before the open; set by Escape and by the film player |
| loop flag | 0 | 0 at each play start; op 20 (not used) |
| speech volume | 90 | op 70 (w1 ≠ 0) |

**Timer.** One millisecond timer. It is started before the open (both callers) and again at the end of each
completed wait (ops 17, 23, 36). A wait of t ms ends when the time since the last start is ≥ t (a wait of t ≤ 0 ends
at once).

## Opening a script

Entry "Play a script" (archive A, member M): abort flag = 0; start the timer; open; play (next section).
The win sequence does the same steps itself.

Open (A, M):
1. Draw target = the screen, pitch 320.
2. Open the archive file A (hard disk, else CD) as the **script archive**; find member M: its start offset and size
   are kept, the file position is at the member start. Keep the name M.
3. Read a record (the first record).
4. v4..v25 = 0.

## The step loop

Play:
1. Condition = 1, last key = 0, loop flag = 0.
2. **Step** (the harness logs a `step` line here, with the op and the a0 of the current record as read, before
   step 3):
3. When the op, read as s16, is ≤ 16: resolve the operands (next section).
4. Run the op (section "Ops"). Ops 0, 14 and every op above 71 (also 99) do nothing.
5. **Key poll**: when a key is waiting, last key = the key's character code (GetKey) as a signed byte.
6. When the abort flag = 1: **stop A** (section "Stops"), the result is 0, play ends.
7. Else when last key ≠ 0: when last key = 27 (Escape): abort flag = 1, **stop B**, result 0, play ends. Any other
   key: last key = 0 (the key does nothing).
8. **Skip**: when condition = 0: while the current record's op is neither 19 nor 43, read a record. (The current
   record, the one just run, is tested first. No other op stops the skip, not 99 and not 18: there is no nesting.)
9. Condition = 1. Read a record. Speech feed (an outside call that keeps a streamed voice going; not logged).
10. When the op of the record just read is 99: find member M again in the script archive (the file position goes
    back to the member start) and read a record (the first record). When the loop flag ≠ 0, go to step 2 with it.
    Else **end**: speech stop, end the 8 effect samples, cry stop, close the script archive; the result is 1.
11. Go to step 2.

So a record with op 99 ends the script when it is read by step 9; it is not run. A record with op 0 (LOS2GAME has
one, all 44 bytes 0) is an empty step: only steps 5..11 happen.

## Operands

Resolving (step 3, only for ops ≤ 16; the end scripts use ops 1, 10, 15, 16 here) changes the current record:

- a0: when a0 > 9999: for ops ≤ 7, a0 = a0 − 10000 (a variable number); for ops 8..16, a0 = v[a0 − 10000] (the
  value of that variable).
- w1 .. w10, each: when wk > 9999, wk = v[wk − 10000]. (w0 is not resolved. Values ≤ 9999, also negative ones,
  stay.)

Ops above 16 resolve nothing in step 3; ops 17 and 18 resolve some words themselves (see there). Every other op
uses the raw words: for example op 70 a0 = 22050 is a sample rate, not a variable.

## Ops

"Read" names the record words the op uses. Outside calls (sound, music, film, fade, file, key) are named in
*italics* and listed with their exact arguments in "Outside calls and the test log".

**1..6 (arithmetic).** Read a0 (the variable number n after resolving; a0 = n or a0 = 10000 + n both mean v[n])
and w1 (resolved). All values s16, results kept as their low 16 bits:
1: v[n] = w1. 2: v[n] = v[n] + w1. 3: v[n] = v[n] − w1. 4: v[n] = v[n] · w1. 5: v[n] = v[n] / w1 (signed 16-bit
division, truncated toward zero). 6: v[n] = the remainder of that division (its sign is the sign of v[n]). For
w1 = 0 (ops 5, 6) and for v[n] = −32768 with w1 = −1, the original stops with a CPU fault; the end scripts use
only op 1.

**7.** v[n] = random(w1) (the game's random number, 0..w1 − 1; not used by the end scripts).

**10 (film).** Read name, a0, w1, w2. *Film* (name, a0, w1, 1, w2). The film member is read from the graphics
archive and drawn on the screen (320 × 200), with its own colours. During the film a key stops it: Escape, or any
key when the film flag ≠ 0 (the film flag is 0 while the end scripts run); then the abort flag = 1, so step 6 does
stop A. The film also plays its cue sounds from the cue table (op 59 empties it). The film player: "Films".

**15 (sprite copy) and 16 (sprite with key colour).** Read a0 (= sprite number after resolving), w1, w2 (resolved).
Let s = sprite a0, W1 = the width of buffer 1.
- When w1 = 9999: when s has no place: nothing (an error value is set; nothing is drawn). Else draw s at
  (px, py) of s.
- Else: when the centre flag ≠ 0: x = w1 − w / 2, y = w2 − h / 2 (w, h of s; "/" truncated; x and y kept as s16).
  Else x = w1, y = w2. Draw s at (x, y).
- "Draw s at (x, y)": for j = 0..h − 1, i = 0..w − 1 in this order: c = sprite pixel (i, j); op 15 writes c to
  buffer 1 at offset (y + j) · W1 + x + i; op 16 writes it only when c ≠ the low byte of the key colour.

**17 (show a rectangle after a wait).** Read a0 = t (ms, raw), w1..w4.
1. When w1 = 9999: (w1, w2, w3, w4) = the saved rectangle. Else each of w1..w4 that is > 9999 becomes
   v[wk − 10000]; then saved rectangle = (w1, w2, w3, w4).
2. Wait t ms ("Waits"). When Escape ended the wait: stop here (nothing below; step 7 then stops the script).
3. Copy w3 × w4 pixels: for each row j = 0..w4 − 1, screen offset (w2 + j) · 320 + w1 + i = buffer 1 offset
   (w2 + j) · W1 + w1 + i, for i = 0..w3 − 1.
4. Start the timer.
5. When the copy flag ≠ 0: 64000-byte copy buffer 0 → buffer 1 (the first 64000 bytes of buffer 1 = those of
   buffer 0).

**18 (if).** Read a0, w1, w2, w3, w4, w5, w6. L = a0, R = w2, L2 = w4, R2 = w6; each of these that is > 9999 is
replaced by v[value − 10000] (the record itself is not changed). Codes c1 = w1, c2 = w5 (raw).
- r1 by c1: 1: L = R; 2: L > R; 3: L < R; 4: L ≠ R; 5: L ≥ R; 6: L ≤ R; 7: L mod R = 0 (signed remainder;
  R = 0 faults); any other code: false.
- When w3 = 0: result = r1.
- Else r2 by c2 with L2, R2: codes 1..6 as above; any other code (also 7): false. When w3 = 1: result = r1 and
  r2; any other w3: result = r1 or r2. (r2 is always computed.)
- Condition = result (1 or 0). When 0, step 8 skips to the next record with op 19 or 43.

**19 (end if).** Condition = 1 (no other effect).

**43 (else).** When condition ≠ 0 (always the case when op 43 is run, see below): while the current record's op is
not 19, read a record (op 43 itself is tested first; nothing else stops this loop, not 99). Then step 5 goes on;
step 9 reads the record after that op 19.

How if / else / end if work together: op 18 false → step 8 skips to the first following 19 or 43 (19: the next
record runs; 43: the else part runs, up to its 19, which does nothing). Op 18 true → the then part runs; reaching
43 skips to the first 19 after it. Condition is set back to 1 after every step, so a 43 that is run always skips.
There is **no nesting**: an inner 18 .. 19 inside a skipped part ends the skip at the inner 19 (the end scripts
have no nested ifs).

**23 (show the frame after a wait).** Read a0 = t (ms).
1. Wait t ms. When Escape ended the wait: stop here.
2. 64000-byte copy buffer 1 → screen.
3. Start the timer.
4. When the copy flag ≠ 0: 64000-byte copy buffer 0 → buffer 1.

**24 (fade in) / 25 (fade out).** Read a0 = slot, w1 = n (steps). 24: *fade* (slot a0, 0, n, 0, 256);
25: *fade* (slot a0, 0, n, 256, 0). The slot is not changed. The fade (outside, VGA): l0 = n · from / 256,
l1 = n · to / 256 (truncated); for level l = l0, l0 ± 1, .. l1 (|l1 − l0| + 1 levels, toward l1): every colour
entry e = 0..255 is shown as (R · l / n, G · l / n, B · l / n) (R, G, B of slot entry e; truncated), each level after
one vertical retrace. So after op 24 the screen colours are the slot's colours, after op 25 all black. (n ≤ 0
faults.)

**26 (define a sprite).** Read a0 = sprite number, w1, w2, w3, w4, w5 = buffer k, w6, w7 (all raw). Wk = the
width of buffer k now. Sprite a0: source = buffer k at offset w2 · Wk + w1; w = w3; h = w4; pitch p = Wk; has-place
= (w6 ≠ −1 and w7 ≠ −1); place px = w6, py = w7 (stored also when has-place is false). (The end scripts take
every sprite from buffer 3, all with a place.)

**28 (clear the subtitle rows).** The 11200 screen bytes from offset 48636 to 59835 (= rows 152..185 entirely, plus
x 316..319 of row 151 and x 0..315 of row 186) = 0.

**29 (subtitle text).** Read a0 = quote q, w1 = colour f, w2 = shadow colour b.
1. When the Subtitles option is off and q < 77 (signed): nothing.
2. Else read record q of `EDT\INTQUOTE.EDT` (200 bytes at offset 200 · q; the text ends at its first 0 byte).
   (The original reads it with PlayQuote (category INT, row 0, quote q, size 200, play no): no voice, and the
   "last quote" values are not changed for this category.)
3. Draw the text on the draw target as the wrapped text of the speech balloon (panel.md "Speech balloon" step 6
   and "Message box" line rule): every byte but the space taken − 1 (encoded text), lines by the message box rule
   with font 2 and the wrap width 195 (a word goes on a new line when L + u > 194); line i (0, 1, ..) drawn with
   Text (panel.md "Drawing primitives") centred at x = 159, y = 152 + 11 i, fg = low byte of f, shadow = low byte of
   b, font 2, mode 300, clip width 320. An empty line draws nothing. Nothing is cleared first (op 28 does that).
   The end scripts use f = 127 or 28 and b = 25; the win sequence f = 0x50, b = 7.

**36 (wait).** Read a0 = t. Wait t ms (with the Escape rule of op 36, "Waits"); when the wait ended by time: start
the timer.

**8 (palette set).** Read a0 = slot, w1, w2. When 0 ≤ w1 ≤ w2: the w2 − w1 + 1 colours from colour w1 of slot a0
go to the display colours w1, w1 + 1, ... (the display colour number wraps at 256). Colours past slot a0's end
come from the next slots, past slot 3 from 4 bytes that are 0 (the island intro uses w1 = 0, w2 = 256 on slot 3:
colour 0 gets the 0 bytes). The test log writes `palette w1 w2 <checksum of the 4 (w2 − w1 + 1) source bytes>`.

**30 (intro caption).** The caption count (8-bit, 0 at program start) + 1. Count 1: draw "Metavira Island" at y 85;
count 2: draw "Years Later..." at y 100, then count = 0; other counts draw nothing. Both texts: font 2, centred at
x 159, colour 0xFF, shadow 1, normal mode, on the screen with pitch 320 (the panel text drawing).

**37 (member seek).** Read name. Find member name in the script archive: its start offset becomes the member
start (ops 40 and 99 count from it) and the file position is at that start; the next record read is its first
record. A missing member changes nothing. The island intro goes on this way into INTRO.SAB and SANTINO.SAB.

**38 (sub-script start).** When the in-call flag = 0 (the script reaches the sub-script's text in its normal run):
read records until the current op is 39 or 99 (the op 38 record itself is tested first: it is neither; after each
read the new op is tested). Both cases: condition = saved condition (1 in practice, see op 40). Step 9 then reads the
record after the op 39. When the in-call flag ≠ 0 (op 40 jumped here): nothing but condition = saved condition; the
sub-script's records run next.

**39 (sub-script end).** When the in-call flag ≠ 0: set the script archive's file position to the return
position; condition = saved condition. Always: in-call flag = 0. Step 9 then reads the record after the op 40.

**40 (call).** Read a0 = byte offset of the sub-script's op 38 record in the member (raw, a multiple of 44).
Saved condition = condition (always 1 when an op runs); in-call flag = 1; return position = the current file
position (just after the op 40 record); file position = member start + a0. Step 9 reads the record there.
Only one level: an op 40 inside a sub-script overwrites the return position (the end scripts do not do that).

**41 (picture palette → slot).** Read name, a0 = slot. Find the member name in the graphics archive (size N). The
byte at member offset N − 769 must be 0x0C; then the next 768 bytes are 256 × (R, G, B); each byte is divided by 4
(shifted right by 2). Slot a0 entry e: bytes 0, 1, 2 = R, G, B of colour e; byte 3 unchanged. The screen colours are
not changed. (When the member is missing or the 0x0C byte is not there, the slot gets the 768 bytes the palette
reader read last; not used by the end scripts.)

**42 (background picture).** Read name. Load the PCX member name of the graphics archive into buffer 0 at offset 0,
pitch W0 ("PCX pictures"). Then, when the copy flag ≠ 0: 64000-byte copy buffer 0 → buffer 1 (also when the member
was missing).

**46 (clear the buffers).** Read a0. The first 64000 bytes of buffer 0, then of buffer 1, = low byte of a0.

**47 (clear the screen).** The 64000 screen bytes = 0.

**50 (picture into a buffer).** Read name, a0 = buffer k, w1, w2. When name is not `TEMPSTR`, the text buffer =
name; the member name is the text buffer (so `TEMPSTR` takes the text that op 11 left there; the end scripts give
real names). Load that PCX member of the graphics archive into buffer k at offset w2 · Wk + w1, pitch Wk (Wk = the
width now).

**52 (buffer size).** Read a0 = buffer k, w1, w2. Wk = w1, Hk = w2 (only these two values). The end scripts set
buffer 3 to 312 × 307 before loading `SLEAV01.PCX` (320 × 200, so each row's last 8 pixels are overwritten by the
next row's first 8) and to 640 × 0 before `J_B_BRUS.PCX` (640 × 150).

**54 (open the graphics archive).** Read name. When a graphics archive is open, close it. Open name (hard disk, else
CD) as the graphics archive. **55**: when a graphics archive is open, close it.

**56 (open the sound archive).** Read name. When a sound archive is open, close it. Open name as the sound archive.
**57**: when one is open, close it.

**58 (choose the voice member).** Read name. When the speech-playing flag ≠ 0: *speech stop*. Find member name in
the sound archive (its start and size become the voice that op 70 streams; the file position is at its start).
When no sound archive is open, the find runs on the closed archive (an error value is set; not used by the end
scripts).

**59 (film cue table).** Read a0, w1, name. When a0 ≠ 0: the film cue rate = a0 (at the program start 11025). When
w1 ≠ 0: the film cue voice volume = w1 (at the program start 0). Both cue counters (the cue index and the cue voice
index) = 0. When name is empty (every use in the end scripts and in the win sequence): the cue table = 50 entries of
(frame 9999, action 0) plus the 10 cue voice names all zero bytes (no cues). With a name: the 200 bytes of the cue
table and the 120 bytes of the cue voice names are read from that CD file (when it cannot be opened the program
ends). At the program start the 320 bytes are all 0. The film player uses them: "Films", "Film cues".

**60 (load music).** Read name. *Music load* (name) (music.md "Load a file").

**61 (play a theme).** Read a0. *Play theme* (a0, stop-first 0) (music.md "Play theme t").

**62 (music volume).** Read a0, w1. When a MIDI driver is installed and the Music option is on: *sequence volume*
(a0, over w1 ms).

**63 (stop the music).** When a MIDI driver is installed: end the playing sequence, playing theme = −1 (the same as
music.md "Stop the music", no bar check).

**67 (load an effect).** Read name, a0. *Load effect* (name, slot a0) (sound.md; the member is in the sound
archive).

**68 (play an effect).** Read a0, w1, w2, w3. *Play effect* (slot a0, rate w1, volume = low byte of w2 as a signed
byte, loops w3). (w2 = 9935 gives the byte 0xCF.)

**70 (stream the voice).** Read a0, w1. When w1 ≠ 0: speech volume = w1. *Speech* (rate a0, speech volume, 0):
streams the member that op 58 found, through the speech channel (sound.md).

**71 (stop all sound).** *Speech stop*; end the 8 effect samples; *cry stop*. (Not used by the end scripts.)

## Waits

A wait of t ms (ops 17, 23, 36): repeat while the time since the last timer start < t:
- when a key is waiting: last key = its code as a signed byte; when it is 27 (Escape): the **Escape-in-wait**
  action, and the wait (and the op) ends at once. Any other key: last key = 0.
- speech feed.

Escape-in-wait for ops 17 and 23: speech-playing flag = 0; end the speech sample (not logged); when the mouth-timer
flag ≠ 0: release that timer (mouth-timer flag = 0; not logged). For op 36 the same, but only when the
speech-playing flag ≠ 0 (else nothing). In all cases last key stays 27, so step 7 of the step loop does stop B
(unless another key is waiting at step 5 and replaces it).

## Stops

- **Stop A** (abort flag = 1, set by a film): when a MIDI driver is installed: end the playing sequence, playing
  theme = −1. Then *speech stop*, end the 8 effect samples, *cry stop*, close the script archive.
- **Stop B** (Escape): *speech stop*, end the 8 effect samples, *cry stop*, close the script archive (the music
  goes on).
- **End** (op 99 read, loop flag 0): as stop B; the result is 1.

The graphics and sound archives are not closed by a stop.

## PCX pictures

Load (archive, member, destination offset o in a buffer memory, pitch P): find the member (size N). When it is
missing: nothing is written (result −1). Else:
1. The first 128 bytes are the header: xmin = u16 at 4, ymin = u16 at 6, xmax = u16 at 8, ymax = u16 at 10, bytes
   per line B = u16 at 0x42. Other header fields are not read (8-bit, one plane is assumed). Rows R = ymax − ymin
   + 1; last column L = xmax − xmin. The picture is placed at o; xmin and ymin do not move it.
2. The data are the member bytes from offset 128 on, read as one stream (the original reads them in pieces of up to
   100000 bytes; all end pictures are smaller).
3. Decode: x = 0, row r = 0. Take the next byte b. When b ≥ 0xC0: count n = b − 0xC0, value v = the next byte. Else
   n = 1, v = b. Then repeat: when x ≤ L, write v at o + r · P + x; x = x + 1; when x = B: r = r + 1, x = 0, and
   when r = R the picture is done, else **the rest of the run is dropped** and decoding goes on with the next
   byte; else n = n − 1, and the repeat ends when n = 0. (So a run that crosses a line end is cut there, and
   n = 0 (byte 0xC0) runs to the end of the line. The end pictures have neither case.)
4. Bytes after x = L in a line (padding) are not written. Nothing is clipped to the buffer.

The end pictures (ENDGRAPH.DAT): SLEAV01.PCX, LEAVBAK1.PCX, LEAVBAK2.PCX, NWBRNDAN.PCX, NWJACKN.PCX (320 × 200),
J_B_BRUS.PCX (640 × 150), HELIPAL.PCX (10 × 5, used for its palette only); all with B = width and the 0x0C byte.

## Outside calls and the test log

The test validate/diff/sab_diff.py runs the original player in Unicorn with stubs and logs these calls; the port
must make the same calls in the same order with the same arguments. Its docstring is the authority for the exact
line format (it logs a `step` line with op, a0 and a checksum of the screen and buffers 1 and 2 at each step, see
"The step loop" step 2).

| Op / place | Call | Arguments | Logged as |
|---|---|---|---|
| open, 54, 56 | open archive | file name | `open NAME` |
| 10 | film | name, a0, w1, 1, w2 | `flc NAME a0 w1 1` (w2 not logged) |
| 24 | fade | slot a0, 0, w1, 0, 256 | `fade 0 w1 0 256 <checksum of the slot's 1024 bytes>` |
| 25 | fade | slot a0, 0, w1, 256, 0 | `fade 0 w1 256 0 <checksum>` |
| 60 | music load | name | `music_load NAME` |
| 61 | play theme | a0, 0 | `music a0 0` |
| 62 | sequence volume | a0, w1 ms (only with music on) | not logged |
| 63, stop A | end sequence | — | not logged |
| 67 | load effect | name, slot a0 | `sfx_load NAME a0` |
| 68 | play effect | slot a0, rate w1, volume (signed low byte of w2), loops w3 | `sfx_play a0 w1 <w2 & 0xFF> w3` |
| 70 | speech | rate a0, volume, 0 | `speech a0 VOLUME` |
| 58 (when speaking), 71, stops, end | speech stop | — | `speech_stop` |
| 71, stops, end | end the 8 effect samples | — | not logged |
| 71, stops, end | cry stop | — | `voc_stop` |
| 29 | read the INTQUOTE record | file, offset 200 q | not logged |
| step 9, waits | speech feed | — | not logged |

Because the harness's speech stub does not set the speech-playing flag, op 58 never logs `speech_stop` there and
the Escape-in-wait of op 36 does nothing. The harness's timer makes every wait end at once and it reports no key,
so the Escape paths and the film abort are not run by it. Its default state has the Subtitles option off: op 29
then draws nothing (the end scripts use quotes 38..71 only), and the INTQUOTE read is not stubbed (a run with
Subtitles on needs a stub for the CD text file).

## Inputs

Values that the player reads but the end scripts do not set; a test must fix them:

- v0..v3 (WINGAME only; the win sequence sets them).
- The Subtitles option (op 29), the MIDI-driver-installed flag and the Music option (ops 62, 63, stop A).
- The speech-playing flag (op 58, op 36 Escape; the host's speech channel sets it while a voice streams) and the
  speech volume (op 70 with w1 = 0; 90 at program start).
- The mouth-timer flag (Escape in waits; 0 in the tac60 dump).
- The abort flag (both callers clear it) and the film flag (0 during the scripts).
- The player state of the table in "Player state" (key colour, centre flag, copy flag, saved rectangle, saved
  condition, in-call flag, buffer W and H, sprites, palette slots): the program-start values, or what an earlier
  script left. In the game the first SAB of a session is an end script, unless the hard disk has `INTRO\SAT.DAT`
  (then Start New Game plays its `island.SAB` first; the GOG install has the file: see "The new-game intro"). The tac60 dump has the
  program-start values for all of them.
- The pixel contents of the screen and of buffers 0..3 at the start (in the game: what the last screen left; the
  harness sets them all to 0 now). Buffer 1 rows 200..302 and buffer 2 are never written by the end scripts; the step
  checksum covers only the screen and the first 64000 bytes of buffer 1.
- The film cue rate and the film cue voice volume (op 59 keeps them when a0 / w1 = 0; only the film player reads
  them: "Film cues").
- The font 2 sheet, its widths and the line height 11 (panel.md "Fonts"), remap table 0 (the Text primitive).
- Keys (KeyPressed / GetKey) and the timer.

## The new-game intro

Main menu choice 0 (Start New Game), before Set Difficulty: when the playing theme is not −1, stop the music; when
the hard-disk file `INTRO\SAT.DAT` exists: play the script (archive `INTRO\SAT.DAT`, member `island.SAB`), then
load the panels again; then the steps of world_load.md section 12 choice 0. The script (25854 steps) opens
INTRO\ISLAND.DAT, ISLAND2.DAT, INTROG.DAT, plays the films INTRO01..05, NUKEX, SNTLK and others, the INTRO music,
voice and effects, the captions (op 30), and with Subtitles on the INTQUOTE texts (op 29). Its films load cue
tables (op 59 with a name: INTRO\HELI.BIN and others) whose cue actions run during the films ("Film cues"); the
test log stubs the film player. Evidence: validate/diff/sab_diff.py `INTRO\SAT.DAT ISLAND.SAB
--port [--subtitles]` (2026-10-27, asm GameMain 0x502A8..0x502D8): every step equal.

## Films (op 10 and the win film)

`film(name, x, y, flag, loops)` plays the FLC member `name` of the open graphics archive into the screen (320 x 200)
at the offset (x, y). The frame time is the FLC header speed (ms per frame; an old FLI speed is in 1/70 s). One
loop = every frame of the film once; loops 0 = repeat until a key. After each frame a callback runs: a key press
stops the film (with the "any key" byte set: every key, and the Escape byte is set; else only Escape sets the
Escape byte and stops; the film ends there); then the film cue step runs ("Film cues"). Timing (FlcPlay 0x5BC58): a periodic
timer with the frame time as its period starts before the first frame; each later frame waits for the next tick
(the tick flag is cleared after the callback), so frame i shows at i x frame time from the film start, and the film
ends right after its last frame with no wait. (#31: title1 cut at 3524 ms = sirtech 48 x 71 + 114.) The end scripts reset the
cue table to 50 x (9999, 0) before each film, so no cue runs.

Frame numbers of the cue step: the first frame of the film shows with no callback. Then each pass of the film shows
the frames 2 .. N (N = the film's frame count) with the frame counter c = 0, 1, .., N − 2 (the callback runs after
each, with that c); a pass that is not the last one then shows the ring frame (frame 1 again) with c = N − 1, and
after it the cue index = 0 (the cue voice index is not reset). With loops 0 every pass shows the ring frame.

## Film cues

The cue table = the 200 bytes of op 59, read as 50 entries of 4 bytes: frame F (16 bits, signed) and action A (16
bits, signed); the 120 bytes after it = 10 cue voice names of 12 bytes (a name ends at its first 0 byte). The cue
index i and the cue voice index j start at 0 (op 59). The cue step with the frame counter c reads entry i and does
one of these (at most one entry per callback):
1. c = F − 1:
   - A < 99: *Play effect* (slot A, rate = the film cue rate, volume 50, loops 1) (as op 68). i = i + 1.
   - A = 99: clear the subtitle rows (as op 28: the 11200 screen bytes from offset 48636 = 0). i = i + 1.
   - 100 ≤ A < 200 (the value A is not used further): when the first byte of voice name j is 0: i = i + 1 (j stays).
     Else: find member (voice name j) in the sound archive (as op 58, but no speech stop first: the found member is
     the voice that op 70 streams), j = j + 1; when it is found: *Speech* (rate = the film cue rate, volume = the
     film cue voice volume, 0) streams it (as op 70). i = i + 1.
   - A ≥ 200: the subtitle text of op 29 with quote A − 200, shadow colour 0 and the colour by A: 231, 233, 235 →
     0x83; 232, 234 → 0x30; 236, 237 → 0x49; else 0xFF. i = i + 1.
2. Else, when F = 9998: *Play theme* (A, stop-first 0) (as op 61). i = i + 1.
3. Else nothing (the entry waits for its frame). An entry (9999, x) never runs, so it ends the cues; entries whose
   frame passes without a match block every later entry of the pass.
The callback then pumps the speech stream and lets the film go on. SPEC GAP (not seen: every cue file of the game (FX\*.BIN on the hard disk, INTRO\*.BIN) ends its cues with (9999, x) by entry 14 and names at most 2 voices): the cue index past entry 79 or the
voice index past name 9 reads other memory (the front-end treats such an entry as (9999, 0) and such a name as
empty). Evidence: validate/diff/filmcue_diff.py (asm FilmCallback 0x5BE18, FlcPlay 0x5BC58 (frame counter +0x9A,
ring frame after +0x54 rewind), op 59 0x5EDE0, tables 0xA5448 / 0xA5510, counters 0x1163B4 / 0x1163B0, rate word
0x96370, volume word 0x337948).

FLC format: the public Autodesk FLIC format (header magic 0xAF12; frame chunks 0xF1FA with COLOR_256, BYTE_RUN,
DELTA_FLC and DELTA_FLI sub-chunks in the end films; colours are 8-bit and are shown as 6-bit DAC values (each
byte / 4)). The three end films: INTRO05.FLC (41 frames, 320 x 125, 71 ms per frame), HELILIFT.FLC (200 frames,
320 x 200), HELILEAV.FLC (245 frames, 320 x 125). The end scripts play HELILIFT at (0, 0) and HELILEAV at (0, 15),
each once (w2 = 1); the win sequence plays INTRO05 at (0, 15) with loops 0 (until a key). The film pixels go to the
screen; the screen keeps the last frame after the film.

## The game-over screen

1. Music stops; every sound stops; the mouse hides; the mouse cursor shape is set.
2. The archive `DAT\OFFICE.DAT` is opened; the picture `viewisle.pcx` is drawn to the screen (pitch 320); its
   palette is read and converted (6-bit RGBX) into the main palette.
3. The text `GAME OVER!` is drawn on the screen at x 135, y 50: not centred, colour 0xFF, shadow 1, font 2, mode
   300 (normal).
4. The palette fades in over 24 steps (0 to 256); the mouse shows.
5. The game waits until a key or a mouse click; then the palette fades out over 56 steps (256 to 0).

## The credits (after the win)

Read from the asm (diff-tested: validate/diff/credits_diff.py). The title pictures routine with argument 0 (the
argument 1 path: "The title part (program start)") shows the credits:

1. Stop flag = 0. The sound archive `FX_TITLE` is opened (no sound of it plays in this path; it is closed at the
   end). The member `credits.bin` of `DAT\TITLE.DAT` is read: 15 s16 numbers (n[0..14], the line count of each
   page), then zero-ended text lines. A line `*` ends a page; a line `@` ends the pages and, the second time, the
   scroll part.
2. When no music theme plays, theme 1 starts (music.md; after a win the file `MISC\intro` is loaded). The screen
   is cleared to 0. The palette is that of `fadepal.pcx` of the same archive (6-bit), set at once. The
   **brightness buffer** B (320 x 200 bytes, signed) is cleared to 0. The screen shows B as it is: the palette
   gives colours 0..15 (and above) the brightness levels.
3. Pages. Page number k = 0. While the current line is not `@`:
   - P (the 320 x 200 page buffer) is cleared to 0; y = (195 - 10 x n[k]) / 2 (signed, rounds toward 0).
   - Each line up to `*` or `@` is drawn into P (pitch 320): centred on x 159, at y, colour 1, shadow 0, font 2,
     mode 300 (normal); y += 10. An empty line draws nothing but takes its 10 rows.
   - A `*` line is passed. k += 1.
   - **Fade to P** (16 frames): for every byte i of B: when P[i] ≠ 0 and B[i] < 15: B[i] += 1; when P[i] = 0 and
     B[i] > 0: B[i] -= 1. Then a wait for the vertical retrace (the start of the next one) and B is copied to the
     screen.
   - A hold of 260 x 5 ms (1.3 s; a calibrated busy loop).
4. Then P is cleared and Fade to P runs once more (the last page fades out). The `@` is passed; B is cleared to 0.
5. The scroll. For each line until a line whose first byte is `@`: rows 0..10 of P are cleared; the line is drawn at y 0 (as in step 3). Then
   for r = 0..10, one **scroll step** with row r of P:
   - a wait for the vertical retrace; B rows 1..199 move up one row (B row 0 is lost); row r of P (320 bytes)
     goes into B row 199;
   - for j = 0..4479: the top byte B[j] -= 1 when B[j] > 1 (signed); the byte B[63680 - j] += 1 when it is not
     0 (no limit, 8-bit wrap). (The top 14 rows fade down to 1, the bottom 14 rows (and B[63680], the first byte
     of row 199) grow brighter as they rise.)
   - a wait for the vertical retrace; B is copied to the screen.
6. Row 0 of P is cleared; 100 scroll steps with row 0 of P.
7. The music volume goes to 0 over 2000 ms; a wait of 2000 ms that a key stops; the palette fades out over 56
   steps (256 to 0).
8. The end (also after a stop): the sound archive closes, the speech and the effect samples stop, the music stops,
   the screen is cleared to 0.

**Stops.** Before step 3, and before each byte of a fade, at each hold step and before each scroll step, an
Escape key (a waiting key that is Escape; it is taken) sets the stop flag. With the stop flag set every loop
ends at once. A stop before step 4 skips steps 4..7 (after an Escape found at the end of a page's lines: the music
volume goes to 0 over 3000 ms and the palette fades out over 56 steps first). A stop in steps 5 or 6 ends only
their scroll steps; step 7 still runs. Other keys stay waiting.

## The title part (program start)

Read from the asm (diff-tested: validate/diff/credits_diff.py --title, the films stubbed; the pictures and the
music checked live). At the program start, after the music file INTRO is loaded and (GM drivers only) theme 0 has
started, the screen is cleared and the title pictures routine runs with argument 1:

1. Stop flag = 0; the sound archive `FX\TITLE` (the cue voices JUNGLE.8 and BATTLE.8) and the graphics archive
   `DAT\TITLE.DAT` open.
2. Film cue reset (op 59 with 0, 0, empty name); film `sirtech.flc` at (0, 0), flag 1, loops 1 (49 frames, 71 ms).
3. The screen is cleared to 0. Film cues from the CD file `FX\TITLE.BIN` with rate 22050 and voice volume 80; film
   `title1.flc` at (0, 0), flag 1, loops 1 (144 frames, 114 ms: the island, then the logo comes in). Its cue
   table: (9998, 1) = play theme 1 at the first cue step; frame 2 = voice JUNGLE.8; frame 50 = voice BATTLE.8.
4. Film cue reset; film `title2.flc` at (0, 0), flag 1, loops 1 (19 frames, 114 ms: the logo, red and orange on
   black).
5. The credits, as "The credits (after the win)" from its step 2 (theme 1 starts there only when no theme plays:
   after title1's cue it plays already).

The "any key" byte is 0 at the program start, so only Escape stops a film (see "Films"); the Escape stops that
film only, the next film and the credits still start (the films do not set the credits' stop flag). The argument
changes nothing else: the credits and their end (the music stops, the screen is cleared) are the same. Then the
main menu opens with no music. The front-end window plays this part at `--menu` (JA1_TITLE=0 skips it); script mode
plays it with JA1_TITLE=1 (films decoded with no waits, line `title ...`).

## The performance rating (end code 1 only)

The score s (s16) is the sum of these terms (bytes are the story bytes from 0x22A045 on, see Inputs):

1. +10 when byte 0x22A045 == 2; -5 when byte 0x22A046 == 1 or byte 0x22A047 == 1;
2. byte 0x22A048: == 2 -> +10, == 1 -> - u8 0x22A064;
3. byte 0x22A049: == 1 -> -10, == 2 -> +10; byte 0x22A04A == 1 -> -10;
4. when byte 0x22A055 != 0: -25; else byte 0x22A04B: 0 -> +10 when island sector 0x1B has flag bit 0; 1 -> -10;
   2 -> +25;
5. byte 0x22A04C: == 2 -> +10, == 1 -> - u8 0x22A058;
6. +25 when byte 0x22A04E == 2 and byte 0x22A05E == 0;
7. +25 when the squad holds item 0x5F; else -25 when it holds item 0x7B;
8. the accounts are recalculated (CalcLedgerAll); + trees tapped / 10; + process capacity / 10;
   + (sum of the trees of the 60 sectors (u8, wraps) - 100); - (day - 50); - (60 - sectors owned);
   - (A.I.M. resignations + merc deaths); - native deaths / 10; + enemy deaths / 8 (signed, rounds to 0);
   + A.I.M. success points / 10; - 10 x bad-day streak; + 2 x sectors gained; - 3 x sectors lost;
9. from day 3 on: balance <= 50000 -> + balance / 5000 (signed), else +10.

Then s is clamped to 0..100; when `leave_reason` > 1 it is halved (never at a win, where it is 1). Word index: s < 30 -> 0, < 50 -> 1,
< 60 -> 2, < 70 -> 3, < 80 -> 4, < 90 -> 5, else 6, into TERRIBLE, LOUSY, POOR, FAIR, GOOD, EXCELLENT, OUTSTANDING.
Text = sprintf("%s %s (%d%c)", "Your overall performance rating is", word, s, '%').
Widths (asm, diff-tested by validate/diff/rating_diff.py --port): s is s16 and every term is added with 16-bit
wrap; u32 / 10, / 8 and / 5000 are signed 32-bit divisions (toward zero) of: trees tapped (+0x1C), process capacity
(+0x84), native deaths (+0x50), enemy deaths (+0x6C), balance (+0x24); the day (+0x04) is compared as 32-bit (> 2).

The program then ends: the text and a line end are kept as the exit message; the exit handler sets text mode 3 and
prints it on the DOS screen.

## Not specified here

- Ops used neither by the end scripts nor by the island intro: 9 sprite place pointer, 11 number text,
  12 and 13 sprite blits, 14 nothing, 20 loop flag, 21 / 22 loops, 27 centre flag, 31 / 32 / 33 /
  34 flags, 35 draw target, 44 palette rotate, 45, 48 copy flag, 49 copy buffer 1 → buffer 0, 51
  key colour, 53, 64 / 65 cry sample load / play, 66 sample rate, 69 sample volume.
- PlayTitleIntro with argument 1 (the films at the program start).
- The Santino scene in sector 0 is specified: the stage blasts (ops 91, 169, 170) in turns.md "Start a timed
  effect", "Home-sector stages"; stage 0 starts when Santino uses tile 291 after his quote (world_load.md section 8
  "Frames"; op 74 use_on_tile springs the trigger item 0x2C there, combat.md "Springing a ground item"), and the
  first blast on his own tile 0x15B removes him (combat.md "Blast"). His speech balloon is in panel.md "Santino's balloon"; its Subtitles branch (the face text box 0x28C84) is not drawn
  by the port.

## Evidence

Read from the asm 2026-10-25 (`./t disas 0x5CF74 2000`, jump table bytes `./t peek 0x5D1B8 0x11C`); the three
scripts dumped record by record from rom/DAT/ENDSAB.DAT; validate/diff/sab_diff.py plays all three to the end in
Unicorn on build/mem_tac60.bin (LOS1 2367 steps, LOS2 1652, WINGAME 5451, result 1); with the Subtitles byte set
and an OpenCDFile stub (a scratch copy of the harness) LOS1 reads INTQUOTE records 62.. at offset 200 q and the
`--png` picture shows the text centred under the picture.

- PlayLoop 0x4FB94: 0x4FC3A..0x4FCFA, jump table 0x4FB84 (1 → 0x4FC78, 2 → 0x4FC96, 3 → 0x4FCAE, 4 → 0x4FCC4; above
  4 → 0x4FCAE); member names 0x90E50 / 0x90E70, archive 0x90E60; delay 0x67250(2000); PerformanceRating?
  0x62EB4; 0x382AC = copy the text to 0x10AAD8, append, exit_(0); stop all 0x5CE08; StopMusic 0x5C82C;
  GameOverScreen 0x504B4; NewGameInit 0x37A0C(byte 0x22A0CC); 0x22A043 = 0; LoadMainSound 0x31B8C; end code byte
  0x93449. GameMain 0x502C0: FileExists("INTRO\SAT.DAT") → 0x5D0C8(that, "island.SAB").
- WinGameSequence? 0x5F2F8: dword reads with sar 24 read byte X + 3 (0x22A048 → S[0x0B] at 0x22A04B, 0x22A04B → S[0x0E]
  at 0x22A04E; REVERSE.md had 0x22A048 / 0x22A04D); S[0x15] = 0x22A055, S[0x1E] = 0x22A05E; FindSquadItem? 0x61528
  (0x5F / 0x7B, DL 0, BL 0x40, CL 0); PlayXmiMusic 0x5C4C8("MISC\intro"); 0x95DC4 = 0; TimerStart 0x47020(0x22A3FC);
  0x5D0EC + 0x5D2D4; on 1: byte 0x93D58 = 1, 0x5EBE8(0x4F, 0x50, 7), 0x5EDE0(0, 0, ""), 0x5BFF8("INTRO05.FLC", 0,
  0xF, ECX 1, 0); then 0x93D58 = 0, PlayTitleIntro? 0x66604(0), AIL_set_sequence_volume(*0x337774, 0, 3000),
  0x5CDD8, AIL_end_sample ×8 (0x32EEB8), 0x5CE7C.
- Play entry 0x5D0C8 (0x95DC4 = 0, TimerStart, 0x5D0EC, 0x5D2D4). Open 0x5D0EC: draw target 0x337794 = *0x95678
  (= 0xA0000), 0x96384 = 175 (not read by these ops), pitch 0x96382 = 320; OpenFileOrCD? 0x53338(name, handle
  0x92F50, dir 0x10AB58, 16, 0); ArcSeek 0x534A4(start 0x337784, size 0x33777C); member name copy 0x33794A; fread
  0x758AD(0x337914, *0x96388 = 44, 1); vars 0x337798 + 2 i, i = 4..25 = 0.
- Loop 0x5D2D4: 0x96354 = 1, 0x96362 = 0, locals [esp+4] (ops 21/22) and [esp+8] (loop flag) = 0; top 0x5D2FF;
  resolver 0x5CF74 when s16 op ≤ 16 (EAX = op; op ≤ 7: a0 − 10000 at 0x337916; else var; w1..w10 = 0x33792C..
  0x33793E); dispatch `dec ax; cmp ax, 0x46; ja` → table 0x5D1B8; post-op 0x5DE28: KeyPressed 0x5616C / GetKey
  0x560F4 (cbw → 0x96362), abort byte 0x95DC4 = 1 → 0x5DE4B (music end when 0x9639A: AIL_end_sequence(*0x337774),
  0x9635E = −1), key 0x1B → 0x5DEC3, skip 0x5DF17 (ops 0x13 / 0x2B), read + 0x5CCF8, op 0x63 → 0x5DF8A (ArcSeek again,
  read, loop flag test, end 0x5DFD1 → 1); speech stop 0x5CDD8, cry stop 0x5CE7C, close 0x53498.
- Op addresses (table 0x5D1B8, op k at entry k − 1): 1 0x5D32F, 2 0x5D34B, 3 0x5D367, 4 0x5D383 (imul), 5 0x5D3AA
  (cwd; idiv cx), 6 0x5D3D4 (remainder), 7 0x5D3FE (Random 0x53538 = (rand()·n / 0x7FFF) mod n), 10 0x5D48C
  (0x5BFF8: EAX name 0x337918, EDX a0, EBX w1, ECX 1, stack w2), 15 0x5E018 (BlitCopy 0x52AE0 (dst, src, w, h, dpitch,
  spitch)), 16 0x5E178 (BlitKey 0x522A0 cdecl (dst, src, w, h, dpitch, spitch, key = s16 0x9637E, compared as a
  byte)), 17 0x5E2EC (rect 0x96366..0x9636C, start 0, 0, 320, 200), 18 0x5E514 (tables 0x5E4E0 / 0x5E4FC), 19
  0x5D82E, 23 0x5E960, 24 0x5D6BA / 25 0x5D6E9 (0x52908: EAX palette, EDX copy-to (0 = none), EBX steps, ECX from,
  stack to; retrace + DAC per level), 26 0x5EA78 (sprites 0xED300 + 0x12 i: +0 source ptr, +4 w, +6 h, +8 pitch, +0xA
  place ptr = w6 · W + w7 + buffer or 0, +0xE px = w6, +0x10 py = w7), 28 0x5D746 (FillRect 0x52AB4(screen + 0xBDFC,
  0, 320, 35, pitch 320)), 29 0x5D76B → 0x5EBE8 (byte 0x1163BD = Subtitles; PlayQuote 0x24F08(AL 0x40, EDX 0, EBX q,
  ECX 200, buf 0x2299BC, cat 10, speaker 0x64, play 0); 0x3625C(text, EDX fg, EBX shadow, ECX *0x337794, pitch 320,
  x 0x9F, y 0x98, wrap 0xC3, centred 1, font 2, line height byte 0x949EC = 11, encoded 1) = WrapText 0x362AC without
  box), 36 0x5D814 → 0x5E748, 38 0x5D865, 39 0x5D8B6, 40 0x5D8EC (cond 0x96354, saved 0x96356, in-call 0x96364,
  return 0x337778 by ftell 0x7C689, fseek 0x757B1 from 0x337784), 41 0x5D994 (0x53300 → 0x53204: fseek(size −
  0x301, SEEK_CUR), fgetc = 0x0C, fread 0x300, >> 2, into 0x32EED8; 0x52820 RGB → RGBX bytes 0..2 into 0x336758 +
  0x400 a0), 42 0x5D9D3 (0x532BC(handle 0x92F48, dir 0x10C158, name, *0x93E80, pitch s16 0x93E84) + 0x5E7DC copy
  flag 0x96380), 43 0x5DA05, 46 0x5DB69, 47 0x5DBAF, 50 0x5DAD1 → 0x5ED24 ("TEMPSTR" 0x910C4, text buffer 0xEBAC0),
  52 0x5DAEC, 54 0x5D934 (0x5E938, OpenFileOrCD(name, 0x92F48, 0x10C158, 0x2C, 0), flag 0x92F90), 55 0x5D966, 56
  0x5DBC5 (0x5C144, OpenFileOrCD(name, 0x92F4C, 0x10C6D8, 0x50, 1), flag 0x92F8F), 57 0x5DBF6, 58 0x5DC23 (speaking
  0x96372; ArcSeek(*0x92F4C, 0x10C6D8, name, 0x96394, 0x337940)), 59 0x5DC6E → 0x5EDE0 (0x96370, 0x337948,
  0x1163B0 / 0x1163B4 = 0, table 0xA5448 (200 B) + 0xA5510 (120 B), OpenCDFile 0x38304 else exit_), 60 0x5DC8E
  (PlayXmiMusic 0x5C4C8), 61 0x5DC9D (SetMusic 0x5C5EC(a0, 0)), 62 0x5DCB1 (0x9639A, 0x1163B9,
  AIL_set_sequence_volume(*0x337774, a0, w1)), 63 0x5DCF3, 67 0x5DD6F (LoadSfxSlot 0x5CA98(name, a0)), 68 0x5DD87
  (PlaySfxSlot 0x5CB34(a0, w1, BL = byte 0x33792E, w3)), 70 0x5DDD7 (volume dword 0x9639C, SpeechStream
  0x5CC6C(a0, vol, 0)), 71 0x5DE05.
- Waits 0x5E3D5 / 0x5E755 / 0x5E975: TimerElapsedMs 0x47040(0x22A3FC) ≥ t (signed); Escape: 0x96372 = 0,
  AIL_end_sample(*0x33776C), byte 0x284F1E ≠ 0 → 0x66D94 (AIL_release_timer_handle(*0x229DFC), 0x284F1E = 0); op 36
  tests 0x96372 first. 64000-byte copies = `rep movsd` of 0xFA00 bytes (encoded f2 a5).
- Buffers 0x93E80: 4 × (ptr, u16 W, u16 H), static values in JA.EXE (0x25CDB8 320 × 200, 0x22A4EF 320 × 303 (the
  tactical back buffer), 0xEDE08 320 × 160, 0x241EF0 640 × 150); every reference is in the player (`./t xref 0x93E80
  --range 0x20`). Static initial player values (JA.EXE bytes 0x96354..0x96389): 0x96354 = 1, 0x96356 = 1, 0x96364 =
  0, 0x96370 = 0x2B11, 0x96374 = 0, 0x9637E = 0, 0x96380 = 1, 0x9639C = 90; build/mem_tac60.bin has the same except
  0x96370 = 22050, 0x337948 = 80, 0x95DC4 = 1 (cleared by the callers). Palette slots 0x336758 (4 × 0x400) and
  sprites 0xED300 are bss, referenced only by the player.
- Film key rule: 0x5BE18 (film frame callback: any key when byte 0x93D58 ≠ 0, else Escape → 0x95DC4 = 1, film ends).
- PCX reader 0x52F9C (EDX file, EBX member size, ECX destination, stack pitch; buffer *0x93E64, pieces of 100000
  = 0x186A0 bytes, data size N − 0x381; run test `b >> 6 = 3`; write when x ≤ xmax − xmin (signed); row end `x = B`
  jumps to the next byte).
- Credits: PlayTitleIntro 0x66604 (argument AL; 0x33796C stop flag, 0x337960 counts = credits.bin at 0x22A4EF,
  0x337964 / 0x337968 text pointers, brightness buffer 0x22ACEF, page buffer *0x93E60), fade to page 0x66B18 (copy
  at 0x66BCD), scroll step 0x66BE4 (copy at 0x66CB6), busy delay 0x67250 (n ms by the loop count 0x933C8 from
  0x67294), key wait 0x67F48, EscPressed 0x67300, music volume 0x5C858, fade 0x52908; strings `*` 0x91300, `@`
  0x912FC; DrawText 0x358D0 (s, P, 320, 159, y, 1, 1, 0, 2, 300).
