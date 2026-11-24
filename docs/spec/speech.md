# Speech, quote text and merc profiles

## Archives

Voice samples live in Sir-tech archives (same container as the `.DAT` files):

| Offset | Size | Field |
|---|---|---|
| 0x00 | 16 | archive name, NUL-padded |
| 0x10 | 4 | capacity (directory slots) |
| 0x14 | 4 | number of directory entries |
| 0x20 + 0x20·i | 32 | entry: name[16] (NUL-padded), u32 absolute offset, u32 size, 8 zero bytes |

Data starts at (capacity + 1) · 0x20. Entries with an empty name are skipped. Member names are compared
case-insensitively (the game upper-cases the name it looks for).

Archive file names (CD, relative to its root) are `<CAT>\<speaker>`, where speaker is a profile id in decimal:
AIM\0..59, DAY\0..63, PST\0..59, JAC\64, QST\64, BRE\65, SAN\66, NAT\NAT, NAMES\0..63. Jack's quotes numbered 80
and above are in JAC\64B instead of JAC\64.

## Sample formats

- `.8` members: raw unsigned 8-bit mono PCM (silence = 0x80), no header. Speech plays at **22050 Hz**.
- `.VOC` members (NAMES\, hard-disk INV\MONEY): Creative Voice files, rate taken from the file.
- Hard-disk SNDS\0..63 (per-merc combat sounds: HIT.8, DIE.8, ATT.8 ...): raw 8-bit; sound effects mostly play
  at 11025 Hz (the rate is chosen by the code that plays them, not stored in the file).

## Quote categories

A quote is identified by (category, speaker, quote number). Categories, in the game's own order:

| # | Category | Text file | Records | Row stride | Voice |
|---|---|---|---|---|---|
| 0 | AIM (AIM screen replies) | EDT\AIMQUOTE.EDT | 840 | 14 per merc (60) | AIM\\id |
| 1 | DAY (in-game remarks) | EDT\DAYQUOTE.EDT | 2112 | 33 per profile (64) | DAY\\id |
| 2 | PST (pay, contract, quitting) | EDT\PSTQUOTE.EDT | 900 | 15 per merc (60) | PST\\id |
| 3 | JAC (Jack, end of day) | EDT\JACQUOTE.EDT | 201 | — | JAC\64, 64B |
| 4 | INV (item descriptions) | EDT\INVQUOTE.EDT | 183 | — | text only |
| 5 | SIG (map signs) | EDT\SIGQUOTE.EDT | 60 | — | text only |
| 6 | QST (quest news) | EDT\QSTQUOTE.EDT | 101 | — | QST\64 |
| 7 | BRE (Brenda) | EDT\BREQUOTE.EDT | 31 | — | BRE\65 |
| 8 | NAT (natives) | EDT\NATQUOTE.EDT | 60 | — | NAT\NAT |
| 9 | SAN (Santino) | EDT\SANQUOTE.EDT | 1 | — | SAN\66 |
| 10 | INT (intro) | EDT\INTQUOTE.EDT | 80 | — | text only |

Text record number = speaker × stride + quote for AIM/DAY/PST, otherwise just the quote number.

Voice member name = `<speaker><CAT><quote>.8` (e.g. `6AIM5.8`). If that member is missing the line is recorded in
two halves `<speaker><CAT><quote>A.8` and `...B.8`, played one after the other with the VOC buffer (the last
loaded `.VOC` sample, volume 0x68) between them (only AIM quotes 5, 6, 12 and PST quotes 5, 8..11 have halves; strategic.md
"Refusal reason"). A line can have one half only (PlayQuote 0x24F08 mode byte 0x284F23, SpeechWait? 0x28260..0x2834D): A alone
= A, then the VOC buffer at 0x68; B alone = the VOC buffer at 0x58, then B. Neither: PlayQuote stays in mode 1 with no seek, so the
stream starts at the file place behind the directory with the old size (live: validate/live_nomember.py), and the port plays
nothing; every such line in the EDT files is a "Bug" placeholder text. The silence byte 0x337C9F stays 0 during the VOC buffer: the
mouth keeps moving (live: `validate/live_halves.py`, a cold start with sound on, shows the order, the volumes and the byte in all modes). NAT members carry no speaker prefix (`NAT30.8`).

Other text files: BIOQUOTE.EDT (AIM biographies; a bio continues over several records), WALQUOTE.EDT (captions
for the WPRO radio sounds `wp<n>.8`).

## Text encoding (EDT files)

- Records are 200 bytes. The text ends at the first 0 byte; the rest of the record is random filler.
- Every byte of the text except the space (0x20) is stored with 1 added. Decode: subtract 1 from each non-space
  byte. Spaces separate words and are stored as is.
- After decoding, 0xB1 is a special mark outside the font, used like a dash between words (counted as 1 pixel wide
  when wrapping). 0x7E..0x8B are Cyrillic letters in the game's font (Ivan's Russian remarks).
- Some lines contain placeholders that are replaced when shown, e.g. `MEMBER` (another merc's name).
- Unused slots contain the text `Member N Bug Q`.

## Initial merc profiles (EDT\NEWPROF.EDT)

64 profile records of 0x5A bytes (layout: `struct profile` in defs/types.toml), stored unencoded. A new game copies
the whole file into the profile table. Ids 0–59 are the AIM mercs, 60–63 the four natives (Elio, Hamous, Juan,
Wahan). Save files store only the first 60.

## Sector patch files (CD MISC\)

The two-digit number in a name is a sector index (0-based; the game shows index + 1).

- `CRATE<nn>.BIN`, `LIVEPL<nn>.BIN`, `DEADPL<nn>.BIN`: records of u16 tile + u16 object; the tile's object layer
  (t1) is set to the object. Tile 0 and tiles ≥ 0xC40 are skipped.
- `ITEM<nn>.BIN`: records of u16 tile + a 12-byte ground item; the item is placed on (or removed from) the tile.
- Supply crates arrive on fixed days: sector index 35 on day 9, 17 on day 17, 42 on day 26, 3 on day 31.
- Processing plants stand in sector indices 59 (home), 26, 53 and 30. When a plant sector is first set up, its
  tiles come from LIVEPL (intact plant) or DEADPL (destroyed plant, depending on a story flag not yet traced) plus
  its ITEM file. See savegame.md for the plant table.

## Evidence

Text decoding diff-tested against the original word splitter in Unicorn, all 4787 records of the 11 quote files
(`validate/diff/edt_diff.py`). Category table, strides, archive/member naming, sample rate and patch formats traced
in the binary (REVERSE.md, "CD speech + quote text"). Tool: `./t speech`.
