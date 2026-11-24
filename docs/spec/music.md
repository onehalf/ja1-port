# Music (front-end)

The game plays XMIDI music through the Miles AIL 3 MIDI driver that SOUND.CFG names. The GOG install names
`sbpro1.mdi` (Sound Blaster Pro, FM synthesis, timbre bank `MADLAB.AD`). This page says which music file and which
sequence ("theme") each screen plays, when a theme changes, and how the files become sound. Sound effects are on
sound.md.

## Files

Three music files, in the game's hard-disk folder `MISC\` beside JA.EXE: `INTRO`, `MENUS`, `DAY`. Each has two
forms: `<name>.XMI` for FM drivers and `<name>GM.XMI` for General MIDI drivers. The game takes the GM form when the
installed MIDI driver is a General MIDI one; it falls back to the CD path when the hard-disk file is missing. The
FM form is the one that the GOG install plays; the front-end uses it.

An XMI file is an IFF file: `FORM XDIR` (an `INFO` chunk: u16 LE sequence count), then `CAT XMID` with one
`FORM XMID` per sequence. Each sequence holds a `TIMB` chunk (u16 LE count, then (patch, bank) byte pairs) and an
`EVNT` chunk (the events). Chunk lengths are u32 big-endian; a chunk of odd length has one pad byte.

| File | Seq. | Length | Loops | Played for |
|---|---|---|---|---|
| all | 0 | 0.1 s | no | driver set-up only (controllers, no notes) |
| INTRO | 1 | 87 s | no | the title part (credits scroll); the main menu is silent |
| INTRO | 2..4 | | | intro and end films (film scripts; not used by the front-end) |
| MENUS | 1 | 72 s | yes | the office |
| MENUS | 2 | 69 s | yes | the island map before the day, and the day replay prompt after the day |
| MENUS | 3 | 64 s | yes | A.I.M. |
| DAY | 1 | 128 s | yes | a sector with no enemy, or no enemy seen yet after a sting |
| DAY | 2 | 57 s | yes | enemies in the sector, none in sight |
| DAY | 3 | 76 s | yes | enemies in sight (combat) |
| DAY | 4 | 10 s | no | sting: the enemies went out of sight, or the sector is clear after theme 2 |
| DAY | 5 | 20 s | no | sting: the last enemy of the sector is gone after a fight |
| DAY | 6 | 12 s | no | sting: the squad is lost |

## Events (EVNT)

Time is counted in ticks of 1/120 s. A byte below 0x80 is a delay: add it to the time (several delay bytes in a
row add up). Other events are MIDI events with a status byte (no running status):

- 0x9n note on: key, velocity, then a variable-length duration in ticks. The note ends by itself after its
  duration; XMIDI has no note-off events. The sequencer sends the note-on and puts (channel, key, duration) in
  the first free entry of its note queue (32 entries). Each tick first counts every queued note down by one
  (queue order); a note whose count drops below 1 gets a note-off (0x8n key 0) and leaves the queue. Then the
  events that are due run. So a note with duration d that starts in tick T ends in tick T + d (d = 0: T + 1).
  A note-on with all 32 entries in use stops the sequence ("Internal note queue overflow").
- 0x8n, 0xAn, 0xBn, 0xEn: two data bytes. 0xCn, 0xDn: one data byte.
- 0xFF meta: type byte, variable-length size, data. 0x2F ends the sequence. 0x51 tempo (u24 BE microseconds per
  quarter note) and 0x58 time signature (numerator, denominator as a power of 2, ...) do not change the tick rate;
  they only give beats and bars (see "Bars").
- 0xF0 / 0xF7 system exclusive: variable-length size, data (ignored).

Controllers with a special meaning (0xBn c v):

- 114 patch bank select: the channel's next program changes take their timbre from bank v.
- 116 loop start: remembers this place in the stream and the count v (0 = for ever). Loops nest (4 levels).
- 117 loop end: with v >= 64 the stream goes back to the place after the innermost loop start (the count goes
  down by one first; at 0 the loop is left). All loop themes of the game loop for ever (116 with v = 0).
- 7 volume, 11 expression, 10 pan, 0x40 sustain as in MIDI. 0xEn pitch bend: +-2 semitones.
- Other controllers (32..36, 58, 61..63 in sequence 0; 1; 112) go to the driver ("The FM driver" below: only 1 and
  112 act there).

**A sequence stops** (a theme change, "Stop the music", the end event 0xFF 0x2F) only when it is playing: a
note-off (0x8n key 0) for each queued note, in queue order, then for each channel 0..15: cc64 0 when the
sequence's last cc64 on that channel was >= 64, then cc112 0 when its last cc112 was >= 64 (both "none" at the
sequence start). The driver's channel state (program, volume, pan ...) stays as it was for the next sequence.
Evidence: AIL_API_stop_sequence 0x842F0 (status 4 only; queue flush 0x82770: entries +0x558 channel, +0x5D8
key, sent through the event routine 0x822E0 as 0x80|ch, key, 0; channel values +0x454 cc64, +0x254 cc112),
AIL_API_start_sequence 0x842B0 (stops first, then 0x82B30 sets the queue and the channel values to -1), the
XMIDI service 0x82C50 (queue count-down +0x658 before the events; queue search and overflow).

## Bars

The game means to change themes at a bar line, but the check never waits. The bar check compares the driver's
bar number with a stored one; the stored value is set to -1 at each theme start and changes only when it equals the
bar number or is 0. So after the first theme start the check always says "new bar", and every change below
happens at once. (build/mem_tac60.bin, mem_fight59.bin, mem_aim_newgame.bin all hold -1 there.) The front-end
changes themes at once.

## Theme state

Three values:

- the loaded music file (the last file that a screen loaded), its sequences in memory;
- the playing theme, -1 = none (saved with a quick save and taken back by Restore);
- the wanted theme (kept while music is off).

Music is on when a MIDI driver is installed and the Control Panel switch "Music" is on.

**Load a file** (screen entry): with music on, read the whole file into memory. The playing theme goes on until a
theme change; the next theme comes from the new file.

**Play theme t** (with a "stop first" flag):
1. Music off: if t > 0, the wanted theme = t. Nothing plays.
2. t equals the playing theme: nothing changes.
3. t = 0: play sequence 0 to its end (the game waits), then the playing theme = -1.
4. Else, if a theme plays or the flag is set: stop it (after the bar check of "Bars", which does not wait).
5. t = -1: nothing more (music stops).
6. Else start sequence t of the loaded file at volume 127; playing = wanted = t.

The Control Panel's Music switch, when turned on, plays the playing theme again from its start.

## Who plays what

**Stop the music** (0x5C82C): end the playing sequence; playing theme = -1 (the wanted theme stays). Screens
stop the music when they end: START NEW GAME in the main menu (GameMain; Continue stops nothing), A.I.M. after its fade-out, the
sunrise screen, the island map when the day starts (also wanted = -1), the tactical loop when the sector is left
(host op 216 music_stop), the day's end paths of PlayLoop, Restore of a quick save. A stop matters: "Play
theme t" does nothing when t equals the playing theme, even when a new file was loaded.

- Program start: load INTRO; a GM driver plays sequence 0 first. The title pictures end with "play theme 1" (flag
  set) when no theme plays (the credits scroll plays it). The title part always ends with "stop the music"
  (also after Escape), so the main menu is silent: seen live, playing theme -1 in the menu after the title. The
  cue table of title1.flc starts theme 1 before the credits (ending.md "The title part (program start)").
- Office entry (also after A.I.M., Load and New game): load MENUS, play theme 1.
- A.I.M.: play theme 3 (MENUS is loaded).
- Island map at the start of the day: play theme 2.
- Leaving the sector (the day ends): music_stop (op 216): no music during the sunset.
- Day replay prompt after the day (PlayLoop): load MENUS, play theme 2.
- Main menu again after Leave: no change. The main menu screen itself starts and stops no music, so the office's
  theme (MENUS theme 1) goes on. START NEW GAME stops it; Continue does not, and the office's "play theme 1"
  then changes nothing (the theme goes on without a restart). Seen live: playing theme 1 in the menu after Leave
  and again in the office after Continue.
- Sector entry: load DAY, play theme 1.
- Squad lost: play theme 6.
- Win sequence: load INTRO (its film script picks the themes).
- Restore of a quick save: play the saved theme when it is > 0.

### The tactical theme rule

Each pass of the tactical loop (after keys) looks at the playing theme. "Seen" is the count of soldiers in slots
0x28..0x2F (sector enemies) that are in the sector, in play, not dead (+0x188 = 0) and seen (+0x9C > 0); the loop
counts it later in the same pass (so the rule reads the count of the pass before). "Enemies" = the sector
table's enemies-alive byte of the current sector. "At a bar" below is the bar check of "Bars": it is always true.

- Theme 1 or 2: enemies = 0: if theme 2, at a bar play theme 4. Enemies > 0 and seen = 0: at a bar play theme 2.
  Enemies > 0 and seen > 0: at a bar play theme 3.
- Theme 3: only when seen = 0 and Santino has not spoken: enemies > 0: at a bar play 4; enemies = 0: at a bar
  play 5.
- Theme 4, 5 or 6: when it no longer plays (its end came), play theme 1.

When the "enemies nearby" message shows at a sector entry, the game plays theme 2 at once. At the end of each
turn the same choice as above runs.

## The FM driver (SBPRO1.MDI)

The music card's driver turns the sequencer's MIDI messages into register writes of a Yamaha OPL2 FM chip. The
driver keeps no clock of its own: every write happens inside a message (it asks for no timer service; no volume
ramps, no software vibrato). Its I/O base comes from SOUND.CFG (0x220).

**The two chips.** The driver writes each register to two OPL2 chips: the left pair (base+0 index, base+1 data)
and the right pair (base+2, base+3), in the order left index, right index, left data, right data. Both chips get
the same value, except the level registers 0x40.. (pan, below). What the player hears: the GOG install emulates a
Sound Blaster 16 with an OPL3 whose OPL3 mode stays off (the driver never writes 0x105). There an index written to
base+2 selects the same register set as base+0, so each register ends with the RIGHT value (written last) and
the output is one mono OPL2 with 9 voices. The front-end therefore keeps one register set, applies every write in
the order above, and plays it mono. How the chip makes sound from its registers is the chip's own behaviour
(YM3812), not the game's; the front-end imitates it (envelopes from the attack, decay, sustain and release
registers, waveforms, feedback, KSL, tremolo/vibrato depth from 0xBD).

**Start.** For r = 0x01..0xF5 in order the driver writes byte r of its init table: 0x01 = 0x20 (waveform
select on), 0x04 = 0x60, 0x20..0x35 = 0x01, 0x40..0x55 = 0x3F (silent), 0x60..0x75 = 0xFF, 0x80..0x95 = 0x0F,
0xBD = 0xC0 (AM depth 4.8 dB, vibrato depth 14 cent, rhythm mode off); every other register 0. Shutdown writes the
same table. The driver's channel state starts at zero (volume 0, expression 0, pan 0, bend range 0, no timbre,
patch 0xFF, bank 0). Then the sequencer's install sends, for each channel 0..15 in turn: cc114 0; program 0;
pitch bend 0x2000; cc112 0; cc1 0; cc7 127; cc10 64; cc11 127; cc64 0; cc91 40; cc93 0; cc100 0; cc101 0; cc38 0;
cc6 2. After this every channel has volume 127, expression 127, pan 64, bend range 2.

**Timbres.** A timbre is the 14-byte entry of `MADLAB.AD` (beside JA.EXE): entries of 6 bytes (patch, bank, u32 LE
offset) until patch = bank = 0xFF; at the offset: u16 size (14), i8 transpose t[2], then t[3..13] = modulator
20/40/60/80 bytes, the C0 byte (feedback/connection), carrier 20/40/60/80 bytes, carrier E0 byte; the modulator
E0 byte is t[7] and t[8] is the C0 byte (t[3] mod 20, t[4] mod 40, t[5] mod 60, t[6] mod 80, t[7] mod E0, t[8] C0,
t[9] car 20, t[10] car 40, t[11] car 60, t[12] car 80, t[13] car E0). A theme start installs the timbres that the
sequence's TIMB chunk lists, in order; when one is not in the bank the install stops there and fails (only
sequence 0 of each file asks for two missing timbres, bank 1 patches 126 and 124). Program change: the channel's
timbre = the installed timbre (channel bank, patch), else none. A program change that comes before its timbre's
install takes effect at the install. There is no fall-back bank: a note on a channel with no timbre is dropped. A sounding note keeps the timbre
of its note start: later channel messages (volume, pan, cc1, bend, cc121) and a voice stolen for a waiting note
and a waiting note that gets a voice later use the timbre of that note's start, also after a program change.
Cc114 sets the channel bank for the next program changes.

**Notes and voices.** 16 note slots, 9 voices. Note-on (velocity 0 = note-off): on channel 10 (index 9) the
timbre is (bank 127, patch = key) and the sounding key is that timbre's transpose byte with transpose 0; on other
channels the channel's timbre, the MIDI key, and transpose t[2]. No timbre: dropped. The note takes the lowest
free slot (all 16 used: dropped). Velocity gain = VEL[velocity >> 3], VEL = 52 55 58 5B 5E 61 64 67 6A 6D 70 73 76
79 7C 7F (hex). Voice: round robin, from the voice after the one last taken (the first note takes voice 0), the
first free voice of 0..8 (wrapping); the pointer moves to each voice tested, so after a failed search it is back
where it started. Then the voice gets all its registers (below).

No free voice: priority of each active slot p = (0xFFFF when its channel has cc112 >= 64, else 0x7FFF) minus the
number of voices that its channel holds (at least 0), computed once. Repeat at most (active slots) times: U = the
waiting slot (no voice) with the highest p (ties: the higher slot), V = the voiced slot with the lowest p (ties:
the higher slot); if p(U) < p(V) or p(U) = 0, stop; else V's note ends (key-off written, slot freed) and U takes
V's voice and all its registers. A note that gets no voice stays silent in its slot until its note-off, and can
get a voice in a later search.

Note-off (0x8n, or 0x9n with velocity 0): every active slot with this channel and MIDI key: when the channel's
sustain (cc64) is >= 64 the slot is only marked sustained; else B0 = the voice's last B0 without the key-on bit
(A0 is not written again), the voice and the slot are freed. The release is the timbre's own release rate.

**Pitch** (A0/B0 of a voice; integer arithmetic, sar = arithmetic shift right):
```
bend = sar(((msb << 7) | lsb) - 0x2000, 5)            ; -256..255
bend = low16(bend * bend_range)                         ; bend_range = the channel's last cc6 value
n    = key + transpose ; k = n - 12, then +12 while k < 0, then -12 while k > 95
a    = sar(low16(bend + (k << 8)) + 8, 4)               ; 1/16 semitone
a    folded into 0..0x5FF by adding or taking 0xC0
s    = a >> 4 ; f = a & 15
fnum = FNUM[(s mod 12) * 16 + f] ; block = s div 12 - 1
if fnum & 0x8000: block = block + 1     ; (fnum's low 10 bits are used)
if block < 0: block = 0, fnum = sar(fnum, 1)
A0 = fnum & 0xFF ; B0 = ((fnum >> 8) & 3) | block << 2 | 0x20
```
FNUM (16 values per halftone, hex): see the table below. Key 60 with transpose 0 sounds as C in block 3.

**Registers of a voice** (mod = the voice's modulator operator, car = its carrier; operator offsets of voices
0..8: 00 01 02 08 09 0A 10 11 12, carrier = +3). At a note start all of them, in this order:
- 20+mod = t[3] | V, 20+car = t[9] | V, V = 0x40 when the channel's cc1 >= 64.
- 40+mod, 40+car: the levels (below).
- 60+mod = t[5], 60+car = t[11], 80+mod = t[6], 80+car = t[12].
- E0+car = t[13], then E0+mod = t[7].
- C0 = t[8] & 0x0F.
- A0, B0 (pitch).

**Levels.** gain(a, b) = ((a × b × 2) >> 8), plus 1 when that is not 0. g = gain(gain(cc7, cc11), velocity gain).
Left g_L = gain(PAN[pan], g), right g_R = gain(PAN[127 − pan], g), PAN[i] = 2i for i < 63, else 127. For an
operator byte b (t[4] or t[10]): L = 63 − (b & 0x3F); level = (b & 0xC0) | (63 − floor(L × g_x / 127)). The
carrier always gets this; the modulator only when t[8] bit 0 = 1 (additive), else its byte b unchanged. The left
chip gets the g_L values and the right chip the g_R values.

**Channel messages on sounding notes.** Cc7, cc10, cc11 write the levels of every voice of the channel; cc1
writes the 20 registers; a pitch bend (0xEn: d1 = low 7 bits, d2 = high 7 bits) writes A0/B0; cc121 writes the
20 registers, the levels and A0/B0.

**Controllers in the driver:** 1 vibrato bit (>= 64); 6 bend range (any cc6); 7 volume; 10 pan; 11 expression;
64 sustain (a value < 64 sends a note-off for each sustained slot of the channel, with the slot's SOUNDING key);
112 voice protection (above); 113 timbre protection (cache only: no effect for JA); 114 bank; 121 reset (sustain 0
with its note-offs, cc1 0, expression 127, bend centre; volume, pan, range and 112 kept); 123 all notes off (a
note-off for each active slot of the channel, with the slot's sounding key; sustain still applies). The sounding
key on channel 10 is the timbre's transpose byte, so sustained or all-notes-off drums usually stay on. All other
controllers (0, 32..36, 38, 58, 61..63, 91, 93, 100, 101, ...), aftertouch (0xAn) and channel pressure (0xDn) do
nothing.

**What the sequencer sends.** The AIL sequencer in JA.EXE keeps 108..111, 115..119 for itself (loops: "Events"
above), sends cc6 after cc100 0, cc101 0, cc38 0, scales cc7 by sequence volume × master volume / 16129 (127 each, until a fade: see musicfade_diff and
`Music::set_volume`; the fade sends the channel volumes only each 8th service tick, so its last value may not go out), and sends every other channel message to the driver. The game's music files use no pitch
bend, cc6, cc11 or cc64; cc1 only with value 0; cc112 three times.

FNUM table (row = halftone C..B of s mod 12, 16 fractions each):
```
C  02B2 02B4 02B7 02B9 02BC 02BE 02C1 02C3 02C6 02C9 02CB 02CE 02D0 02D3 02D6 02D8
C# 02DB 02DD 02E0 02E3 02E5 02E8 02EB 02ED 02F0 02F3 02F6 02F8 02FB 02FE 0301 0303
D  0306 0309 030C 030F 0311 0314 0317 031A 031D 0320 0323 0326 0329 032B 032E 0331
D# 0334 0337 033A 033D 0340 0343 0346 0349 034C 034F 0352 0356 0359 035C 035F 0362
E  0365 0368 036B 036F 0372 0375 0378 037B 037F 0382 0385 0388 038C 038F 0392 0395
F  0399 039C 039F 03A3 03A6 03A9 03AD 03B0 03B4 03B7 03BB 03BE 03C1 03C5 03C8 03CC
F# 03CF 03D3 03D7 03DA 03DE 03E1 03E5 03E8 03EC 03F0 03F3 03F7 03FB 03FE FE01 FE03
G  FE05 FE07 FE08 FE0A FE0C FE0E FE10 FE12 FE14 FE16 FE18 FE1A FE1C FE1E FE20 FE21
G# FE23 FE25 FE27 FE29 FE2B FE2D FE2F FE31 FE34 FE36 FE38 FE3A FE3C FE3E FE40 FE42
A  FE44 FE46 FE48 FE4A FE4C FE4F FE51 FE53 FE55 FE57 FE59 FE5C FE5E FE60 FE62 FE64
A# FE67 FE69 FE6B FE6D FE6F FE72 FE74 FE76 FE79 FE7B FE7D FE7F FE82 FE84 FE86 FE89
B  FE8B FE8D FE90 FE92 FE95 FE97 FE99 FE9C FE9E FEA1 FEA3 FEA5 FEA8 FEAA FEAD FEAF
```

Evidence (2026-10-28): SBPRO1.MDI (driver file offsets): entry 0x132 (AX function, BX driver), DRV_INIT 0x3901
(service_rate -1), INIT_DEV 0x3A39 -> 0x2AB4 (table 0xA33), register write 0x28E3 (ports L idx, R idx, L data, R
data) and 0x2912 (levels), INSTALL_T_SET 0x3B26 / 0x2BCA / 0x2D54, MIDI_XMIT 0x3AAA, note-on 0x360A (VEL 0xC1E),
allocation 0x2EC4 (pointer 0x1443), stealing 0x33DD, pitch 0x32E1 (FNUM 0x7F4, BLOCK 0x974, HALF 0x9D4), voice
registers 0x2F73 (PAN 0xC2E), controllers 0x3728 / 0x3794, note-off 0x35A7, sustain release 0x36F2. JA.EXE: AIL
install reset loop 0x83A60 (prefs 0x33939C = 127, 0x3393A8 = 2), sequencer event routine 0x822E0. Run in Unicorn
(build/probe/mdi/run.py) against a re-statement of these rules: 500+ random streams of 400 messages, 0
differences. Test: validate/diff/mdi_diff.py --port (the original driver in Unicorn, validate/diff/mdi_run.py, vs
the front-end's driver; every register write of the start, the install reset, a timbre install and 300 random
messages). DOSBox (GOG dosboxja.conf: sbtype sb16, oplmode auto, oplemu default = DBOPL): adlib.cpp
Module::PortWrite keeps one index latch in OPL3 mode, dbopl.cpp Chip::WriteAddr maps base+2 to bank 0 unless
0x105 bit 0 is set, Handler::Generate plays GenerateBlock2 (mono) then.

## The chip imitation against DBOPL

JA.EXE holds no chip code. The chip that the original plays on here is DOSBox's DBOPL, so `validate/diff/oplchip_diff.py` compares
fm.rs `Opl` with DBOPL register script by register script (level per 10 ms, harmonics, tremolo depth). The curves are in
`validate/diff/oplchip_ref.json`. Rules taken from DBOPL on 2026-11-09: the key scale level = (block x 8 - KslCreateTable[fnum >> 6]) /
8 octaves, not the pitch in octaves above note 48 (this was up to 8 dB too low at high notes); the attack falls as (att + 0.1875 dB)
with 4.8 time constants per table time (measured). Known difference: the port mixes at 22050 Hz and DBOPL at 49716 Hz, so strong
feedback (6, 7) gives other upper harmonics, up to 8 dB. The Level (VOICE_SCALE 4900) again matches the DOSBox-X capture.

## Output and script line

The music advances with the sound mix: each tick of the play loop is 10 ms of music too (sound.md "Output"); the
window's screens outside play (main menu, office, A.I.M.) advance it by their own clock (an audio callback).
Script line `music` prints `music <FILE> <theme> notes <n> changes <list>`: the loaded file (INTRO, MENUS, DAY),
the playing theme (-1 = none), the notes started and the theme changes since the last `music` line (`FILE:t` for
a start, `stop` for a stop, `-` for none), e.g. `music DAY 3 notes 41 changes DAY:2 DAY:3`. `JA1_WAV` holds the music in the mix. `JA1_MUSIC=0` turns the
music off (the Control Panel switch).

**Level.** The chip imitation's output scale (music.rs `VOICE_SCALE` 4200) comes from a DOSBox-X capture of the
original (2026-11-05, #31 evidence): DAY theme 1 in sector 59 of DAY_day3, 21 s matched to the port's song time
23.96..44.96 s by the band spectrum. The battle samples (ATTACK, DIE0) have the same gain in both mixes (172..178 per
sample unit), so the digital path is the common reference. The mix of the original has rms 1455; the effects
alone (port, Music off) have rms 663, so the music part is rms 1296. The old scale 8000 gave rms 2524 (+5.8 dB, the
music too loud beside the effects). gate_music step 7 checks the level within 1 dB. The same capture also compared
the timbre: the log band spectrum per 23 ms frame has r = 0.874 with the original (0.868 with the wrong sign of the
modulation, so the sign is not audible; it follows DBOPL `Operator::GetSample`).

Evidence (2026-10-25): PlayXmiMusic? 0x5C4C8 (flag 0x9639A = MIDI driver installed, 0x9639B = GM driver: suffix
"gm.xmi" 0x910A4 else ".xmi" 0x910AC; FileExists, else CD prefix 0x92F8C + ":\" + name; whole file into
0x32F1D8), SetMusic? 0x5C5EC (EAX theme, EDX stop-first; 0x1163B9 = option Music; playing theme = high word of
0x9635C, wanted 0x96360; AIL handle 0x337774; AIL_init_sequence(handle, file, t), start, volume 127), bar wait
0x5C78C / bar check 0x5C724 (AIL_sequence_position 0x7AE9F, last bar 0x92E54), still playing 0x5C768 (status 4),
InitAil 0x5C1DC (GTL prefix "MADLAB", flags 0x96398), callers: InitMainDatMouse 0x36FD1/0x36FF4, PlayTitleIntro?
0x6672B, InitOfficeScreen? 0x4ED3A/0x4EDD0, ShowAimScreen 0x20034, StrategicGuide? 0x4248B, DayEndScreen? (end of
day, PlayLoop 0x4FC50) 0x65BF6/0x65C07, LoadTacticalSector? 0x2B1F1/0x2B234, TacticalLoop 0x10B0x..0x10BB1 and
0x1100F (enemies nearby), turn end 0x315DC (0x317A0), squad lost 0x3A35A (theme 6), QuickRestore 0x2D872,
Control Panel 0x4BF8C / 0x4D075, film script code 9998 0x5BFE3 (theme table 0xA5448). Seen count 0x22A478 (word;
TacticalLoop and NewTurn 0x31550). Live dumps: build/mem_tac60.bin playing theme 1, mem_fight59.bin theme 3,
mem_aim_newgame.bin theme 3; all with 0x96398 = 00 01 01 00 (FM driver, music on). The seen count agrees with the rule: mem_fight59.bin count 1 (enemy 42 seen) with theme 3, mem_tac60.bin count 0 with theme 1.
