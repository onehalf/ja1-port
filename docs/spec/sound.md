# Sound effects, cries and voices (front-end)

The library never plays sound. It asks the front-end with host calls (port_api.md "Host"): op 60 sfx plays a
loaded effect, ops 100 / 101 / 193 load and play a soldier's short cry, ops 209 / 210 / 259 swap the effect bank
around a door. Merc quotes (speech.md) play when a speech balloon shows (panel.md "Speech balloon"). This page
says which files fill which effect number, and how a request becomes sound. Music is not on this page.

## Files

The effect files are Sir-tech archives (speech.md "Archives") in the game's hard-disk folder, beside JA.EXE (not
on the CD): `FX\OVERHEAD`, `FX\DOORS`, `FX\ENEMIES`, `FX\MAINSND`, `FX\BATTLE`, `FX\OFFICE`, `FX\REPLAY`,
`FX\TITLE`, and one archive per profile `SNDS\<p>` (p in decimal, 0..63). A member named `NAME.8` is raw unsigned
8-bit mono PCM (silence 0x80, no header). A member `NAME.VOC` is a Creative Voice file. Member names are looked up
in upper case. A member that is missing loads nothing (SPEC GAP (not seen: every member of the tactical bank, FX\OVERHEAD
45 names, FX\ENEMIES 8, SNDS\0..63 ATT/OK/HIT/DIE, and of the main bank, FX\MAINSND 27 names, is in the GOG files,
checked 2026-10-28): the original reads whatever its archive seek gives; the front-end leaves the effect empty).

## The effect bank

The front-end keeps an effect bank: effect numbers 0..127, each one sample (bytes) or empty.

**Tactical bank** (loaded when a sector is entered, and again at op 259 overhead_sounds):
1. Effects 0..44 = `FX\OVERHEAD`, members (in effect order) `IN OUT 1 2 3 4 8 9 10 11 12 15 EXP FAL R_L P_L ROK
   WAT KNE GRD R0 R1 R2 SIL OPN WAD S_L S_R TUR FIR D_1 D_2 IMP BUB CRT SWI B0 B1 B2 B3 F1 F2 STU SNA TCK`,
   each with `.8` added (effect 12 = EXP, 26 / 27 = S_L / S_R footsteps, 29 = FIR).
2. For each team soldier k = 0..7 with "in the current sector" (record +0x182) ≠ 0 and (in play (+0x138) ≠ 0 or
   record byte +0x188 ≠ 0, set after a death): from `SNDS\<p>` (p = profile, record +0x179) the members `ATT.8`, `OK.8`, `HIT.8`,
   `DIE.8` go to effects 0x2D + 4k, 0x2E + 4k, 0x2F + 4k, 0x30 + 4k.
3. From `FX\ENEMIES`, for i = 1..4: `<i>_HIT.8` to effect 0x4D + 2 (i − 1) and `<i>_DIE.8` to effect
   0x4E + 2 (i − 1).

**Door bank** (op 209 door_snd (0)): every sound stops; effects 0..7 = `FX\DOORS` members `KEY L_0 PUL NOB K_I
LOK P_I PCK` (`.8` added). The other effects keep their samples.

**Back from the door** (op 209 door_snd (1)): every sound stops; effects 0..16 are loaded again from
`FX\OVERHEAD` (the first 17 names of the list above). Effects 17 and up are not touched.

**Op 259 overhead_sounds**: every sound stops; the whole tactical bank is loaded again (steps 1..3).

**Main bank** (the office, the main menu, the sunset and sunrise screens and the day replay load it when they
open; nothing stops): effects 0..26 = `FX\MAINSND`, members (in effect order) `IN OUT SWI LEV ROT SCR CR1 CR2 CVR
LAP ME1 ME2 PA1 PA2 SL1 SL2 HEL NAT INS B_1 B_2 B_3 F2 IDL LIF P_L R_L`, each with `.8` added (guessed from the names and the callers: effect 0 / 1 = the
button press / release sounds, 2 = switch, 3 = lever, 4 = rotate, 5 = screen slide, 0x10 = helicopter, 0x12 = night
insects, 0x13..0x16 = birds and frogs). Effects 27 and up keep their samples (SPEC GAP (not seen: the main bank screens play only effects 0..26, and the
sector entry loads the tactical bank before any tactical sound): the original packs the
samples of a bank one after the other in one buffer, so after the main bank the old samples of effects 27 and up
point into overwritten bytes; nothing plays them before the next tactical bank).

**Menu sound** (the main menu, Set Difficulty, Quick-Save; world_load.md section 12): main-bank effect 0 (IN) as
op 60 sfx (0, 11025, 6, 1). The original plays it from the same routine (0x5C880) for a few other screen clicks.

**Button sounds** (the original's button routine 0x67CD8, used by the screen buttons of the tactical panel, A.I.M.,
the island map, the assignment and inventory screens and the menu boxes): when the left button is down on a
button, the press sound, then the button's action (its pressed picture and the wait for the button up), then the
release sound. Press = effect 0 (IN), release = effect 1 (OUT), both of the bank loaded now (the tactical and the
main bank both hold IN and OUT there), rate 11025, volume 20, once. The front-end keeps the channel of the last
button sound: when that channel still plays, it stops before the next button sound starts.

"Every sound stops" = all effect channels, the cry channel and the speech channel stop at once.

## Channels

The front-end mixes 10 channels: 8 effect channels (numbered 0..7), one cry channel, one speech channel. A
channel plays one sample from its start at a playback rate (samples per second) and a volume v (0..127; the
sample value (b − 128) is scaled by v / 127), a given number of times (loops: 1 = once, n = n times, 0 = until it
is stopped). A channel is free when it plays nothing (never started, ended, or stopped).

## Requests

- **Op 60 sfx (effect, rate, volume, loops)**: the first free effect channel (lowest number) starts the effect's
  sample with that rate, volume (the low byte, as a signed number; a negative volume is taken as 0) and loops.
  No free channel: nothing plays. An empty effect starts nothing (SPEC GAP (not seen: checked 2026-10-28, the library's op 60 effects are constants or table values 0x0C..0x2C (shot sounds below 0x63: 2..10; footsteps, water, doors, blasts), the leave sounds 0x13..0x16 and the screens' own sounds lie in the main bank 0..26, the door sounds in 0..7; op 65 names 0x2D + 4k + c only for soldiers that the sector entry loaded (mercs join on the island map, not in a sector) and 0x4D..0x54 for the others (+0x1E0 is random(4) for every guard, tapper and enemy): no request names an effect that no bank loaded). The original has no such test: it hands the effect's last pointer and size (zero before the first load, else the bytes of an older bank) to AIL. The usual rate is 11025 (0x2B11).
- **Op 65 effect_slot (slot k, kind c)**: a soldier's own sound (c: 0 ATT, 1 OK, 2 HIT, 3 DIE; the library sends
  c = 1 for a merc's move order when the option `Merc OKs` is on, the other kinds at selection, hits, deaths and
  coughs). S = record k (k = record +0x17A). Effect and volume:
  - k ≥ 8 (guards, enemies): c < 2 plays nothing. Else effect 0x4D + 2 × (u8 +0x1E0) + (c − 2), volume 0x58.
  - k < 8 (mercs): c < 2 and s32 +0xC0 > 0 (the voice timer of the cries runs) plays nothing. Else when byte
    +0x16C ≠ 0 nothing plays (and nothing changes). Else u32 +0xC0 = 400 and effect 0x2D + 4k + c, volume 0x32.

  Rate: with h = s8 +0x183 (health) and e = s8 +0x18A (energy): when h < 50 or e < 60 the rate is
  11025 − 5 × (50 − h) − 5 × (60 − e) (one term may be negative), else 11025. Then as op 60 sfx (effect, rate,
  volume, 1). Evidence: 0x17B70 (diff-tested by effslot_diff).
- **Op 100 cry (speaker p, −1, cry c, 1, 1)**: loads `SNDS\<p>` member `<NAME>.VOC` with NAME by c: 0 BAD, 1 GOOD,
  2 EMPTY, 3 NOTHING, 4 HMM, as the cry sample. Nothing plays yet.
- **Op 186 death_show (slot, quiet)**: the cry sample becomes `FX\OVERHEAD` member `DEATH.VOC` (as in op 100: from
  offset 0x21, without the last byte) and the cry channel starts it at rate 11025, volume 0x6E, loops 1, for a loud
  and a quiet death alike (the merc's own voice files are not used). Evidence: MercDies? 0x3A4A4 opens FX\OVERHEAD
  (0x5C0EC), PlayMercVoice 0x24D14 with ECX = 0 only loads the member DEATH from that open archive, PlayVocSample
  0x5C9C0 (0x2B11, 0x6E, 1). The text and the face of op 186 are in shooting.md "Merc dies".
- **Op 101 voice (rate, volume, loops)** and **op 193 voc (rate, volume, loops)**: the cry channel starts the cry
  sample (it restarts if it still plays) with that rate, volume and loops. The cry sample is the bytes of the VOC
  member from offset 0x21 up to the last byte, without the last byte (the original skips the VOC header and the
  end block; the rate in the VOC file is not used, the request's rate is: 11025).
- **Op 254 eod_begin (day)**: the main bank loads; the first free effect channel starts effect 0x12 (night insects)
  at rate 11025, volume 50, loops 0 (until stopped); the front-end keeps that channel as the sunset channel (none
  when no channel was free). The bird calls of the sunset are op 60 requests of the library (effects 0x13..0x16)
  and play from the main bank.
- **Op 258 eod_sound_stop**: the sunset channel stops (nothing else). The library then sends op 259, which loads
  the tactical bank again.
- **Op 210 door_anim (big, r, phase, x, y)**: the original plays a short film here, and the film starts door effects at
  given frames. The cue file is `FX\<P>_<D><A>.BIN` (hard-disk folder) with P = `I` for phase 0, else `S` when
  r ≥ 9 and `F` when r < 9; D = `PD` when big ≠ 0, else `DR`; A by r: 6 or 9 `KEY`, 7 or 10 `PCK`, 8 or 11 `HAN`
  (another r: no cue file, nothing plays). The file holds 50 cues of 4 bytes, (i16 frame f, i16 sound s); f = 9999
  is no cue. When the film shows frame f − 1, a cue with s < 99 starts effect s as op 60 sfx (s, 11025, 50, 1).
  Values s ≥ 99 do not occur in the door cue files. The cues start at T × (f − 1) ms after the film start, with
  T = the film's frame time (its FLC header speed, ending.md "Films"); the try film starts at the call, the outcome
  film when the try film ended ((N − 1) × T after it, N = its frame count: the original's PlayFilm returns only
  then). The film is the member `<P>_<D><A><n>.FLC` of the tileset's CD archive `DAT\<n>.DAT` (n = the tileset,
  but 5 for tileset 6: FormatSectorDatName 0x2F7C8), drawn at the (x, y) of the op (combat.md "Doors") with the film
  flag 0 (the palette stays); Escape stops a film; I_PDHAN films have 114 ms, all
  other door films 71 ms (read from every door film in the archives 0..5 and 7..9; there is no 6.DAT). The cues
  play in the order of the file, each with the effect sample as it was at the call (a later op 209 neither stops
  nor changes them). Archive 8 holds only the DRHAN films (SPEC GAP (not seen: the tileset 8 sectors 6, 7 and 16
  have the door value 0 on every tile (JAGGED.DAT, no game write; 23 dumps), so their doors are never big and never
  locked and only I_DRHAN and S_DRHAN are asked for) what the original does when the film member is missing:
  the archive lookup fails and the film player reads a header at the archive's current file position; the
  front-end plays the cues all the same).

**Switch view** (panel.md "The switch view"): a switch press plays effect 0 (IN), its release effect 1 (OUT),
rate 11025, volume 0x32, once.

**Island map sounds** (strategic.md "Mouse over the island map", "Native wage box"; main bank): every sector frame
(the old sector's and the new one's) plays effect 1 (OUT) at rate 22050, volume 1, once (FlashSectorFrame 0x4147C;
also on the guard screen). The map tab clicks (a new map mode, the plants tab), the left click on the native wage
box and each right click while the wage is edited play the press sound (effect 0, the button sound of "Button
sounds", routine 0x5C8A0). The left click that ends the wage edit plays effect 1 (OUT) at rate 11025, volume 1,
once (0x43637).

**Click sound** (the original's routine 0x5C880): effect 0 (IN) of the bank loaded now, rate 11025, volume 6,
once (the menu sound above is the same routine). Unlike the press sound it does not stop an earlier sound. It plays:
- op 227 inv_click (the library's inventory screen and the item moves of the assignment screen);
- on the assignment screen: selecting a merc (a face box), the press on the item panel's face (no merc is
  selected after it), and the press on the ground place with an item on the cursor, before the discard question;
- in the tactical aim (cursor.md "Clicks", left step 6): when the aim starts (before the wait) and at each right
  click of the aim.
- in the morning stat changes: once per frame of the experience count (strategic.md "The experience count").

**Assignment screen press sounds** (routine 0x5C8A0, "Button sounds"): the duty menu plays the press sound when it
opens (strategic.md "The duty menu", Open step 2) and at each left click that ends the duty menu or the subject menu.
The exit button and the panel buttons 10..14 are buttons of "Button sounds" (press and release). The processing
plants page (strategic.md "The processing plants") ends with the release sound (routine 0x5C900: effect 1 (OUT),
rate 11025, volume 20, once, with the stop of the last button sound as for the press sound) when the left button
is up. The assignment screen's own op 60 requests (the reload sounds, main-bank effects 0x19 / 0x1A) play from the
main bank.

**The fire** (combat.md "Drawing the effect records"): when an entry starts the fire, op 60 sfx (0x1D, 0x2B11,
0x32, 5) plays, also when no tile animation is free.

Every call of the original's sound routines (PlaySfxSlot 0x5CB34 and the three routines 0x5C880, 0x5C8A0, 0x5C900;
105 calls in 53 functions) was checked on 2026-10-28: the library's routines send them as op 60 / 65 requests (and
op 227), the screens' calls are the ones above and in the spec pages they name; the experience count of the
morning plays the click sound per frame (strategic.md "The experience count", when the fast flag is 0). All
other ops make no sound. The sector battle sounds (op 332) are in strategic.md "The battle show".
Ops 59 walk_sound, 126 move_sound and 139 step_sound_stop are not sounds despite their names: they redraw parts
of the merc panel (the original's routines draw a slot frame, a slot, a portrait). Op 139 also clears the cursor
tile 0x229D2C when the stopping soldier is the selected one (RedrawMercPortrait? 0x336B4, port_api.md); the
walk marker 0x229CAC keeps its life. Footsteps are op 60 sfx with effects 26 / 27 from the animation frames
(soldier_update.md).

**Sound option**: when the game's sound option is off, no request starts a sound (the original answers −1 to op
60 then). This front-end has sound always on.

## Merc quotes

When a speech balloon shows (panel.md "Speech balloon", the merc with profile p says quote q of the DAY
category), the speech channel starts the quote's voice: archive `DAY\<p>` on the CD, member `<p>DAY<q>.8`, or
when that is missing the two halves `<p>DAY<q>A.8` then `<p>DAY<q>B.8` one after the other (speech.md "Quote
categories"); 22050 samples per second, volume 88, once. The same volume holds for every quote voice that a
speech balloon or a merc-face box waits on (the guide, Brenda, Santino, A.I.M., the dismissed merc): the wait
starts the voice at volume 0x58 (evidence: SpeechWait? 0x28034, its three SpeechStream 0x5CC6C calls 0x2810D,
0x28292, 0x28331 with EDX = 0x58, EBX = 1; the 8-bit-file branch 0x280F7 PlayVocSample also 0x58). Before
2026-10-27 this section said 127 (#14 D: "the voice volume is very loud"). The category the original uses here is a game byte
that holds 1 (DAY) during play (seen in every tactical save state).

## Output

The front-end mixes the channels at 22050 samples per second, mono, 16 bits: the sum over the playing channels of
(b − 128) × v / 127 × 256 for each channel's current sample byte b (stepped at rate / 22050 bytes per output
sample, nearest byte), limited to −32768 .. 32767. Each tick of the play loop (world_load.md section 8) is 10 ms of
sound; ticks are counted while the tactical view plays (the cue times of op 210 count in the same ticks). While the
end-of-day screen is open, each of its frames (op 207, 70 per second) and each of its waits (ops 37 and 57, the
given milliseconds) adds its time to the mix in the same 10 ms ticks (the remainder carries over).
The window sends each new part of the mix to the sound device at once; when more than 150 ms of older sound still
waits there (the window was held up), the older sound is thrown away first, so a click's sound never comes late
(front-end rule, not the original's; #20 H). Proof: smoke window_audio (SDL's dummy audio driver, the window
stopped 2 s; with the dummy driver the music gets no device of its own).

Script line (world_load.md section 8 "Play script"): `sound` prints `sound N1 N2 ...`, the names of the samples
started since the last `sound` line (effects as their member name without `.8`, `voc:<NAME>` for the cry channel,
`quote:<p>DAY<q>` for a merc quote), or `sound -` when none. `JA1_WAV=FILE` writes the whole mix of a script run
to FILE (WAV, 22050 Hz, mono, 16 bits).

Evidence (2026-10-25): InitAil 0x5C1DC (up to 8 effect handles into 0x32EEB8, count 0x33775C; cry handle
0x337760, speech handle 0x33776C), PlaySfxSlot 0x5CB34 (first handle with status 2 = done; sample type 0 = 8-bit
mono; rate, volume, loop count; −1 when the sound flag 0x96399 or the option byte is 0), LoadSfxSlot 0x5CA98
(buffers packed in effect order from 0x2972B8; pointer and size table 0x28F010), LoadOverheadSounds 0x31B44
(names 0x94D28, 4 bytes each; count 0x2D also loads 0x31AF8), 0x31AF8 / 0x31A70 (merc sounds, PlaySndsVoc
0x24E44 with DX ≥ 0 → `.8` into effect DX; DX < 0 → `.VOC` via LoadVocSample 0x5C960), LoadEnemySounds 0x31A10,
LoadDoorSounds (names 0x94C97), LoadMainSound 0x31B8C (FormatFxPath "mainsnd", LoadFxSound 0x24CB4 = "%s.8" of the
names 0x94CB7, 4 bytes each, into effects 0..26; callers main_, EndOfDayScreen 0x29F55, SunriseScreen? 0x2A2A4,
InitOfficeScreen? 0x4ED3F, PlayLoop 0x4FCE5, GameMain 0x502FE, DayEndScreen? 0x65C1B), EndOfDayScreen 0x29F44
(PlaySfxSlot(0x12, 0x2B11, 0x32, 0) after the picture, handle kept; after 119 frames 0x5CE60(handle) =
AIL_end_sample when the handle is >= 0; then 0x5CE08 + LoadOverheadSounds), PlayDoorFx? 0x3B03C (stop 0x5CE08, door bank, cue file FX\%s.BIN via 0x5EDE0 into
0xA5448, film 0x5BFF8 whose cue loop 0x5BE6E calls PlaySfxSlot (s, rate word 0x96370 = 0x2B11, 0x32, 1) for
s < 99, then stop 0x5CE08 and LoadOverheadSounds (0x11)), PlayVocSample 0x5C9C0 (cry handle; data from VOC + 0x21,
length size − 0x22), PlayQuote 0x24F08 (category byte 0x92F8B = 1 in build/mem_tac60.bin and mem_fight59.bin),
SpeechStream 0x5CC6C (rate 0x5622). Stop-all 0x5CE08 ends the speech, the 8 effect and the cry handles.
