# Jagged Alliance (DOS, 1994 — GOG v1.13) — Reverse Engineering Notes

## Binary Identification

| Field | Value |
|-------|-------|
| File | `rom/JA.EXE` (932,827 bytes, sha256 93c51ff785d228c31837cd8048319a9a7e01e13e7cb06af62c0be61476559fe6) |
| Format | MZ stub (DOS/4G 1.97-era, 'BW' DOS/16M collection) → inner MZ @ F:0x352A4 → LE @ F:0x37D4C |
| Platform | MS-DOS, DOS/4GW 32-bit flat protected mode |
| Compiler | Watcom C/C++32 (runtime strings © 1988-1994), version string "Release v1.13 Sep 05-95" |
| CPU | i386 (LE cpu=2) |
| Objects | obj1 code base 0x10000 vsize 0x7AC7D (r-x); obj2 data base 0x90000 vsize 0x2AA730 (rw-, auto DS) |
| Pages | 136 × 4 KiB, last page 0xB37; data pages @ F:0x5C0A4 (LE field + inner MZ base) |
| Fixups | 17,816 internal (17,815 off32, 1 sel16); no imports |
| Entry point | obj1+0x56F6C = 0x66F6C (`_cstart_`), stack obj2+0x2AA730 |
| Data files | 53 MB in rom/ (sound drivers, FX/, INTRO/, DAT/, INV/, SNDS/, EDT/), 390 MB CD image (AIM/, NAMES/, DAY/, PST/, MISC/, DAT/ …) — see analysis/inventory.txt |
| Libraries | Watcom clib; Miles AIL v3 (AIL_* debug-trace strings present: they name the API functions) |

---

## Memory Map

| Address Range | Purpose |
|---------------|---------|
| 0x00010000–0x0008AC7D | obj1: code (game + Watcom clib + AIL) |
| 0x00090000–0x0009CB37 | obj2: initialized data (strings, tables) |
| 0x0009CB37–0x0033A730 | obj2: bss (≈2.6 MB: game state, buffers, stack at top) |
| 0x000A0000 (real-mode) | VGA framebuffer, reached through DOS/4GW's 1:1 low-memory mapping |

---

## Data-Range Map

| Start | End | Size | Classification | Notes |
|-------|-----|------|----------------|-------|
| 0x10000 | 0x8AC7D | 0x7AC7D | code | 1375 procedures found by Ghidra |
| 0x66F6C | ~0x66FE4 | | code+string | `_cstart_` jmp over WATCOM banner |
| 0x90000 | 0x9CB37 | 0xCB37 | data | 1145 strings (analysis/strings.txt) |
| 0x9CB37 | 0x33A730 | | bss | unclassified |

---

## Key Findings

### Architecture

*(call graph, module boundaries, entry points, execution flow)*

- Startup: `_cstart_` (0x66F6C) → `__InitRtns` (0x7CA48) → `__CMain` (0x7C9F9) → `main_` (0x10150) → `exit_` (0x755B4).
- `main_`: per-argv `ParseCmdArg` (0x1008C: NOCRITICAL, RESET, ALLOWSAVE) → printf banner "Loading Jagged Alliance %s" +
  v1.14 quick-save notice → "Press any key" → init chain 0x374B0, 0x68784, 0x373F0, 0x5C0B8, 0x66EF8, 0x56188, then
  0x560F4, 0x53564, 0x52C10, 0x50440, 0x38038, 0x377B0, 0x37A0C, 0x67294, 0x5CED4, 0x36F90, 0x31B8C, 0x4FFEC ; labeled by string refs:
  `InitCDROM` 0x373F0, `CheckDiskSpace` 0x50440, `InitTempFiles` 0x38038, `InitSound` 0x5CED4 (sound.cfg),
  `InitMainDatMouse` 0x36F90 (dat\main.dat + mouse), `LoadMainSound` 0x31B8C, **`GameMain` 0x4FFEC** (intro, quick.sav,
  "Thanks for playing": top-level loop). Unnamed (no strings): 0x374B0 (49 calls), 0x68784, 0x5C0B8, 0x66EF8, 0x56188,
  0x53564, 0x52C10, 0x377B0, 0x37A0C, 0x67294 (0x560F4 = GetKey; screens: see State Machine).
- Asset paths found in strings: `dat\JAGGED.DAT`, `DAT\TERRAIN.DAT`, `DAT\POPUPS.DAT`, `dat\sanim.dat`,
  `%s%s%02d.pcc`, many `.pcx` screens (aimscrn, statbgnd, sunset, helin6, faces …), `<@FX\%s`.

### Data Structures

- **DAT\JAGGED.DAT** (CD, 22,696 B), read whole by `OpenJaggedDat2` 0x371BC (`FormatCDPath(name, ofs)` opens, `fseek_?` to
  ofs, keeps EDX). Layout, verified by size sum = 22,696 exactly:
  | Offset | Size | Contents |
  |---|---|---|
  | 0x0000 | 0x40 | header (skipped) |
  | 0x0040 | 60×(1+1) | per sector i: 1 byte → T[i]+6, 1 byte → T[i]+0, where T = 0x22979C, 60 × 9-byte table |
  | 0x00B8 | 60×0x30 | → `g_sectors[i]`+0x00 |
  | 0x0BF8 | 60×0x28 | → +0x30 |
  | 0x1558 | 60×0x28 | → +0x58 |
  | 0x1EB8 | 60×(1+1) | → +0x80, +0x81 (sizes inferred: no EDX set) |
  | 0x1F30 | 0x1540 | global block A |
  | 0x3470 | 0x2200 | global block B |
  | 0x5670 | 0x9D | block C |
  | 0x570D | 0x100 | block D (then call 0x756A0) |
  | 0x580D | 0x9B | block E (ends 0x58A8 = 22,696) |
  `OpenJaggedDat` 0x3712C re-reads only the 0x30/0x28 blocks (seek 0xB8): likely a reset of per-sector state.
  **Contents (from the CD file):** the 0x30/0x28 blocks are arrays of `(u16 tile, u16 value)` (12 resp. 10 entries).
  `tile` = index into the 56×56 sector map (0..3135); **0x0C40 = 3136 = 56·56 marks an empty slot**, with value 0xC7
  (A, C blocks) or 0x63 (B block), likely the field's max. Sector 0 A starts (0x1C8,0x62) (0x3F9,0x5C)… Header bytes
  0x00–0x3F are all 7..11 (60 per-sector values + 4?). 0x1EB8 block: sector 0 = (0,0), 1 = (98,255), then 255s.
  **Block C (+0x58) confirmed use:** `CheckSectorTileTriggers` 0x19FA0 (diff-tested): merc on the tile and exp level
  (merc+0x192) ≥ value → trigger (0x19F30) + 0x62A68(merc,4,10); **negative value = inverted gate**: fires if level ≤
  −value, reward 5. So C = level-gated tile triggers (hidden item/trap?). Tile → screen: x = (t%56)·16+5,
  y = (t/56)·14+5 (TileToX/Y).
  **Lock-lookup quirk:** `SectorLockLookup?` scans 12 pairs from +0x30 but B holds 10, so it also matches C's first two
  triggers (real data: sector 1 tile 1526 → 6). A port must keep this.
  Fired triggers are consumed: `ConsumeSectorTrigger` 0x19E20 sets tile = **0x0C41** (used) vs 0x0C40 (empty slot).
  `FireSectorTrigger` 0x19F30 shows a message (data near BAD/GOOD/EMPTY/NOTHING/HMM) and recomputes merc x/y;
  0x62A68 checks merc+0x17A (≤7) and +0x19B (status?) before acting: likely an experience/stat-gain call (unverified).
  Merc struct so far: +0x114 u16 tile index, +0x126/+0x128 x/y, +0x192 i8 stat, +0x17A, +0x19B. g_sectors is saved by QuickSave 0x2D880 / QuickRestore 0x2C898.
- **DAT\XRA.DAT** (703,752 B) = **59 records × 11,928 B (0x2E98)**, record = sector index: `SetupSectorFromXra` 0x2E918
  seeks `g_curSector * 11928` (computed by shift/add chain) and reads in 0x200 chunks into buffer [0x93E64] plus small
  reads (0xC, 0x28). Only taken when `g_sectorTbl9[cur].b0 & 1` == 0; otherwise another path, probably the visited-sector
  temp file sec%02d.tmp. (`XRA\%2d.xra` is sprintf'd first: dev-time loose-file name, not on the CD.) 59 vs 60
  sectors: which one lacks a record is TBD.
  Record layout (partial): 2 passes × 8 soldier slots (k = 0x28..0x2F) × **0x200 B soldier struct**: pass 1 keeps
  the read into the soldier array [0x1009F8] if `g_sectors[cur]+0x6E+k` (rec+0x96..+0x9D) == 2, pass 2 if == 1;
  otherwise reads into scratch [0x93E64] and drops it. After each read, slot+0x17A = k. 0x2000 of 0x2E98 B explained;
  remaining 0xE98 B (small reads 0xC/0x28 etc.) TBD.
- **Soldier struct = 0x200 B** (array ptr at [0x1009F8]; same struct as the "merc" fields above: +0x114 tile,
  +0x126/+0x128 x/y, +0x17A slot index (enemy slots 0x28..0x2F; 0x62A68 requires ≤7 → own mercs 0..7?), +0x192 stat,
  +0x19B status). Slots 40–47 = sector enemies/NPCs (hypothesis).
  From XRA.DAT sample slots (sectors 1–2): records hold 16 similar enemy templates; first 0x24 B = 3 × 12-byte
  item entries? (`0e 14 64 00 63 64 …`, 0x64 = 100 condition?; InitXraEnemy fixes up 3 × 12-B entries at +0x54..+0x77);
  **+0x114/+0x116/+0x118 = 3 candidate start tiles** (corrected by the Ph6 live dump: InitXraEnemy picks one with
  Random(3); in memory +0x116/+0x118 are then x/y); +0x17A slot within group (0..7 in file); **stat block** (UI label table at 0x91180 is in this order; +0x18D = wisdom **confirmed** in code: 0x38D80
  copies "wisdom" into a message and `sub [soldier+0x18D], al` (stat loss, next to "head" → head wound)):
  +0x183 Health (Max) | +0x18B Agility | +0x18C Dexterity | +0x18D Wisdom | +0x18E Medical | +0x18F Explosives |
  +0x190 Mechanical | +0x191 Marksmanship | +0x192 Experience level (sample: 77|76 74 78 41 70 32 84|7).
  `CheckSectorTileTriggers` therefore gates on experience level. Remaining fields: confirm via DOSBox memory dump.
  Each record's tail from 0x2000 starts with 0xFF fill.
  **A/B by usage (hypothesis):** A (+0x00) = doors, B (+0x30) = locks (strength ≤99): `SectorDoorLookup?` 0x3AFE0 /
  `SectorLockLookup?` 0x3B430 return val for a tile; both called by crowbar 0x3B474.
  **Record field users** (imul 0xB1 scan): +0xAF mines (BuryMine 0x3CB24 + 3 more), +0xAA ownership? (0x3E508 "ENEMIES GAIN
  SECTOR"), +0x9F home sector (0x60680), +0x82..+0xB0 heavy use in SetupSectorFromXra 0x2E918 (21 sites; bounds OK: 0x2E918–0x2F7A1, xref junk = linear-sweep through inline data)
  and 0x5F94C/0x61FF4. Old note: placement lists (enemies / items / trees-mines?) per sector: check in the tactical loader that
  consumes `g_sectors`+0x00/+0x30/+0x58 (`./t xref 0x222B74`).
  `g_sectors` 0x222B74 = 60 × 0xB1-byte records; fields still unnamed (next: dump the blocks per sector, compare with
  the strategic map / manual's sector info, then confirm in a DOSBox memory dump).


- **Day save `game%d.sav` (QICKSTRT.SAV, 21,866 B)**, from `./t ioseq LoadGame` (0x4E964) + sample file:
  | Offset | Size | Contents |
  |---|---|---|
  | 0x0000 | 0x1C | 2 bytes from 0x229FA4 + 0x1A save name from 0x28EDFE (SaveGame order; slot list 0x4DB34 reads it) |
  | 0x001C | 8 × 0x200 | `g_soldiers` slots 0–7 (QICKSTRT: all 8 = Grunty Hector Tex It Vinny Hurl Beth Larry) |
  | 0x101C | 60 × 0x5A | `g_profiles` (60 AIM merc records, layout below) |
  | 0x2534 | 60 × 9 | `g_sectorTbl9` |
  | 0x2750 | 60 × 0xB1 | `g_sectors` |
  | 0x50CC | 0x356 | 0x229FA0 block |
  | 0x5422 | 0x14 | 0x1163B8 |
  | 0x5436 | 0x3D | 0x92DF0 |
  | 0x5473 | 0x1E | 0x92ED2 |
  | 0x5491 | 18,080 × n | **one sec%02d.tmp copy per sector with g_sectors+0xA9 set** (SaveSectorTmps 0x4E740, sector order; n = 0 in QICKSTRT). VERIFIED: day-3 DAY.SAV / OFFICE.SAV 57,993 B = 0x5491 + 2 × 18,080 (SEC58, SEC59) + 184 |
  | after | 0x88 | 136 per-item-type counts (0x93DD4), read by 0x51508 (offset 0x5491 when n = 0) |
  | after | Σcounts × 3 | records (type, q, cond) grouped by type; Σ = 27 in QICKSTRT ✓ (e.g. 20 × type 0x36 `36 01 64`). Item stockpile? each added via 0x51D50 |
  **QuickSave 0x2D880 write order** (fwrite_ 0x75FE3, EAX=buf EDX=size EBX=n): 0x229FA4 (0x1C hdr) → soldiers
  0xFA958 0x200×**0x30** → profiles 0xEBB40 0x5A×0x3C → g_sectorTbl9 0x21C → g_sectors 0xB1×0x3C → 0x229FA0 0x356 →
  0x1163B8 0x14 → 0x92DF0 0x3D → 0x92ED2 0x1E → 0x10E380 0x20 → small globals (0x22A486, 0x22A484, 0x92DCD, 0x284F28,
  0x229E04, 0x92DC4, 0x92E3C, 0x22A488 0x41, 0x22A44E 0x1E, …). **Two formats, confirmed in code:** day saves `game%d.sav` (`SaveGame` 0x4EBC8 / `LoadGame` 0x4E964) store
  **8** soldiers (player mercs) + profiles + …; mid-day `quick.sav` (QuickSave/QuickRestore) stores all **48** (incl.
  sector enemies). QICKSTRT.SAV is a day save: `NewGameSetup` 0x4F548 does FileExists?(qickstrt.sav) →
  rename_(qickstrt.sav, game0.sav) (**confirmed in DOSBox**: GAME0.SAV byte-identical, original removed; the scratch copy needs QICKSTRT.SAV re-copied per run). Its layout matches (0x1C+8·0x200+0x1518+0x21C = 0x2750 ✓). Soldier array 0xFA958 = 48 slots (0..7 mercs, 0x28..0x2F sector enemies).
  +0x192 = exp level: all 8 QICKSTRT mercs have 1 and so do their profiles (+0x44), so it's real.
- **Merc profile (`g_profiles`, 60 × 0x5A, id = index)**, decoded from QICKSTRT.SAV and **confirmed on the AIM screen**
  (Needle, id 0: salary 1900, HLT 92, AGI 80, DEX 81, WIS 88, MED 98, EXP 6, MECH 34, MRK 62, class 2; shots/39):
  | Off | Field | | Off | Field |
  |---|---|---|---|---|
  | 0x00 | nickname, 10 B | | 0x2D | marksmanship |
  | 0x0A | full name, 0x1E B | | 0x2E | wisdom |
  | 0x28 | u16 salary (per day) | | 0x40 | medical |
  | 0x2A | health | | 0x41 | mechanical |
  | 0x2B | 100 in all records (energy/morale?) | | 0x42 | dexterity |
  | 0x2C | agility | | 0x43 | explosives |
  |  |  | | 0x44 | experience level |
  0x2F–0x3F and 0x45–0x59 are still unknown (a few bytes at 0x33 = 0x33/0x63, 0x3A–0x3C = 0x63 look like caps).
  Records 50–59 have leftover text after the NULs (uninitialized buffer when the data was authored).
- **Soldier ← profile (cross-check, all 8 QICKSTRT mercs, 0 mismatches):** +0x179 = profile id, +0x16E nickname,
  +0x110 = +0x112 = salary, +0x183/+0x184 = health (current/max? equal here), +0x18A ← prof+0x2B, then
  +0x18B agi, +0x18C dex, +0x18D wis, +0x18E med, +0x18F expl, +0x190 mech, +0x191 mrk, +0x192 level, in AIM-screen order.
  **Live dump (DOSBox-X) confirms** the 8 structs in memory byte-for-byte. LoadGame's post-load patches seen in
  memory exactly as coded at 0x4EB34..: profile 24 (Fidel) +0x39 2 → 0x65, profile 26 (Biff) +0x39 0x65 → 2 (and
  profile 41 +0x3A 0x63 → 0x19, +0x3C → 0x63, not triggered here).
- **Ph6 live session (VERIFIED, DOSBox-X dumps build/mem_{walk1,sector59,combat1}.bin, `./t soldiers DUMP`):**
  - **Health: +0x183 = current, +0x184 = max, +0x185 = wound total (max − current)**: after an enemy turn in sector 59
    Hector 31/79 (+0x185 = 48), Hurl 40/100 (+0x185 = 60). Hector also lost wisdom +0x18D 72 → 66 and +0x18A 100 → 96:
    the head-wound stat loss traced at 0x38D80, seen live.
  - **Slots:** 0–7 mercs, **8–15 guards** ("GUARD n", profile id 64; re-rolled by `InitGuard?` 0x2B7DC on sector load:
    level = Random(day+5)/10+1, day = [0x229FA4]; off-map tile 0x0C41 when not in the current sector), 0x28–0x2F enemies
    ("ENEMY n", profile 64). +0x182 = 1 for soldiers in the current sector.
  - **XRA → enemy slots confirmed:** sector index 58 (UI "59"): g_sectors+0x96..+0x9D = 1,1,1,1,FF,FF,FF,FF, so pass 2
    keeps templates 8–11 of XRA record 58 into slots 0x28–0x2B, the rest stay empty, exactly as traced. Each template is
    then finished by **`InitXraEnemy` 0x2B6D0** (called at 0x2EFE2), which explains every byte that differs from the file:
    **file +0x114/+0x116/+0x118 = 3 candidate start tiles**, one chosen by Random(3) → +0x114 (+0x124, +0x12E);
    afterwards +0x116/+0x118 hold x/y like +0x126/+0x128; +0x184 = +0x183 (file has 0); +0x185 = 0; +0x18A = 100;
    +0x17B += 4; +0x17C = Random(8)+2; +0x1E0 = Random(4); +0x1BD = 0x6D; +0x182 = +0x138 = 1. Stats +0x18B..+0x192 unchanged.
    `Random(n)` 0x53538 = rand_()·n/0x7FFF % n (Watcom `rand_` 0x76929, LCG 0x41C64E6D/0x3039).
  - **Tactical tile map `g_tileMap` 0x100A18 = 56×56 × 8 B:** byte 5 = slot of the soldier standing there (64 = none;
    16/16 soldiers match in walk1, incl. enemies 40–43 in combat1), byte 6 = index into the ground-item list (255 = none;
    matches all used entries in both dumps), byte 7 = 0, byte 3 changes as mercs move (visibility/seen?), bytes 0–4 terrain.
  - **Ground items `g_groundItems` 0xEB160 = 200 × 12 B** (b0 type, b1 count/qty, b2 condition, b6 = FF in combat, b8.. per-unit
    conditions?): read with 0xC-size freads in SetupSectorFromXra.
  - **`sec%02d.tmp` (visited sector state) layout VERIFIED:** `SaveSectorTmp` 0x2F800 writes, per tile (3136), bytes
    1,2,3,4,6 of g_tileMap (5 B), then 0x960 B of g_groundItems: 15,680 + 2,400 = **18,080 B = SEC59.TMP** (written on
    leaving sector index 59). Item part byte-identical to memory; tile part differs only in byte 3 of 113 tiles walked
    after the dump. Bytes 0, 5 (occupant), 7 are not saved.
  - **DAY.SAV** (21,866 B, written at day start, same layout as game%d.sav) vs live memory: day, name, profiles,
    g_sectorTbl9, g_sectors, 0x1163B8, 0x92DF0, 0x92ED2, item counts all identical except the fields the session changed:
    soldier position/runtime fields, **g_sectorTbl9[i]+2 = number of our mercs in sector i** (8 moved 59 → 58; strategic
    code adds team size [0x229FAC] there, 0x43A1E), g_sectors+0xA9 0 → 1 for both sectors (entered/combat?), +0x9E of 58
    0 → 121, and the 0x229FA0 block (g_curSector 0x22A039 59 → 58 + 23 bytes of tactical state). +0 bit 0 of g_sectorTbl9 =
    owned (60: 0x0D, 59: 0x18).
  - **Traverse rules (from code, observed in game):** `TraversePrompt` 0x1CBA8 (T key or TRAV button; N/E/S/W popup, keys
    N E S W; merc hp > 15) → `CanTraverseEdge` 0x30FA0 (tile within 5 of that edge, neighbour exists in the 10-wide sector
    grid) → `TraverseCheck` 0x31344 → `TeamNearby?` 0x311D8: every other squad merc in the sector must be within 7, else
    "Can't traverse here!". The squad arrives on the opposite edge (columns 54–55 after going west) in turn-based combat.
  - **Soldier inventory (VERIFIED on the INV screen):** 12 × 12-B item entries at +0x00 (b0 type, b1 qty/ammo, b2
    condition; same 12-B record as XRA templates and g_groundItems). Item names = 15-B records at **0x96AEF + 15·type**
    (0 "Nothing", 1 ".38 Revolver", 0x23 Helmet, 0x2A Radio, 0x36 ".38 Ammo(6)", 0x1C Grenade, 0x57 Key ...). Screen slot
    map: entry 0 = left hand box (revolver "06"), 1 = right hand box, 2 = head (helmet; body % = armour: Grunty 5% with
    helmet, Hurl 0%), 4 = ear (radio), 6 = vest. QICKSTRT team: all 8 carry a .38 (6 rounds) + radio; medics carry Medical
    Kit / First Aid Kit, Vinny a Tool Kit. INV stats/salary (+0x110 u16 = 490 Grunty, 400 Hurl) match the struct.
  - **Death (VERIFIED, mem_fight8 → mem_death1, Hurl killed by enemy 41):** item entries cleared, tile → 0x0C41 (and
    +0x124/+0x120), +0x183/+0x184 → 0, +0x18A → 0, +0x138 → 0 (in play), +0x17D → 3, +0x16D → 1, +0x186 → 0xFF,
    +0x188 → 1, **+0x1DA = slot of the killer (41)**; portrait becomes a skull.
  - **Day restart (VERIFIED, mem_restart1):** ABORT → "RESTART this day?" **OK = reload DAY.SAV and redeploy straight into
    tactical** (profiles, tables, stash identical to DAY.SAV; soldiers differ only in the placement fields of a fresh entry).
    NO = restore OFFICE.SAV, back to the office (see above).
  - **Outfit screen (morning strategic → raised team-cost box (39,184) → team/salary screen → portrait):** pool (stash) on
    the left, items at screen x ≈ 28–72 (rows y ≈ 73/90/108/125), ↑/↓/%-# buttons at x ≈ 89; merc slots: main hand
    (133,89), attachment (133,109), secondary hand (228,106), vest pockets (279,74+19·i), ←/→ merc (17,32)/(78,32), portrait
    (47,40) = exit. Left click picks one item, left click places it. **Memory matches the screen exactly:** entry 1 =
    secondary hand, entries 7.. = vest pockets (It: rock in 7, ammo in 8); g_stashCounts updates as items move. The day
    save written after outfitting (DAY.SAV 21,833 B = 0x5519 + 3·16) shrinks with the stash: layout confirmed again.
    The morning "visit the ASSIGNMENT screen" hints only appear when the screen was skipped.
  - **QUICK.SAV (103,702 B, OPT → Quick Save → "Save day in progress?") VERIFIED block by block:** `QuickSave` 0x2D880
    first calls SaveSectorTmp (refreshes sec%02d.tmp), turns each soldier's +0x90 pointer into an offset (−0x106C18) at
    +0x98, then writes 143 blocks directly (85,402 B: g_day, all **48** soldiers, profiles, tables, ~130 globals,
    g_groundItems, g_tileMap 0x6200) followed by 0x4E740 = copy of each sec%02d.tmp (here SEC59.TMP, 18,080 B,
    byte-identical), 0x51590 = item stash (0x88 counts + 3·Σ, identical to the day-save stash), 0x2C834 = REPLAY.REC
    (3 B·[0x22A46C], identical). 85,402 + 18,080 + 217 + 3 = 103,702 ✓. Versus the pre-save dump only +0x98 (the
    serialised pointer) and +0xA8/+0xB0/+0xCC of the 16 soldiers in the sector, plus 5 small counters, differ.
  - **Save files in one day loop (all 21,866 B, day-save layout):** OFFICE.SAV = autosave on entering the office (before
    Sleep); DAY.SAV = written at day start after Sleep (day 1, team assigned, g_curSector set). Tactical ABORT → "ABORT this
    day?" OK → "RESTART this day?" **NO = restore OFFICE.SAV and return to the office** (g_day back to 0; memory then equals
    OFFICE.SAV), OK = restart the day (presumably DAY.SAV, not tested). ~~sec%02d.tmp and REPLAY.REC are deleted at day
    end~~ **corrected (day-2 end, see Ph6 day loop below):** the sector files of visited sectors persist (and go into every
    later day save); REPLAY.REC is reset to the 3-B "X" record at the next day start and deleted on the ABORT → office path.
    **SaveGame verified end to end:** calendar Save → slot 1 "PH6 TEST" → GAME1.SAV equals the live dump on every block
    (soldiers, profiles, tables, globals, 27 stash records) and differs from OFFICE.SAV only in the 8 name bytes.

- **Combat rules (traced, ported in tools/combat.py, diff-tested 0 mismatches vs the original in Unicorn:
  validate/diff/combat_diff.py, 80 dumped soldiers + 3,000 fuzzed AP cases + 6,020 hit-chance cases):**
  - **`MercMaxAP` 0x56210** (AP per turn): (level·10 + agi·2 + maxHP + dex + 10) / 20 (max 25, the manual's cap);
    wounded: −AP·2·lost/(3·maxHP), lost = (max−cur−wound)/2 + (max−cur); energy +0x18A < 100: −(100−e)·AP/200;
    minimum 8. Energy is what dropped our mercs from ~21 to 14–17 AP during the fight.
  - **`ShotApCost` 0x564F8** (EAX = soldier = main-hand item entry, EDX = target tile): (new target tile ≠ +0xD0
    ? 1 : 0) + (AP·200 / ((mrk/2 + 50)·speed/2) + 1) / 2, clamped 1..AP; class-4 weapons (knives) use avg(agi, dex)
    instead of mrk. So a shot costs a fixed *fraction of the turn* (manual's "fast guy pays more" explanation).
    `ActionApCost` 0x56310 dispatches on item kind (item table +2: 0x92 gun → ShotApCost + wpn[2]).
  - **`AimLoop` 0x1D744** (right-click while targeting): +1 AP and +1 aim click each, cap = min(base+4, AP left),
    **one click past the cap wraps back to the base cost (aim 0)**. Aim stored in +0x1FD. Real-time shots get aim 4.
  - **`ChanceToHit` 0x56630** (EAX shooter, EDX target tile, BL aim) → 1..99 %, rolled in `FireGun` 0x5959C as
    Random(100)+1 ≤ chance (+0x1BC = hit flag, +0x1FC = margin; on a miss the margin goes to 0x598A8 = stray-bullet
    path, the friendly-fire source). Terms, in order:
    mrk (averaged with gun condition when that is lower) · +10 same target tile as last shot (+0xD0) · **+10 per aim
    click** · enemy shooter: difficulty table 0x95C4E (s16: −10/0/+10) · shooter +0x1B9 == 4: −20 ·
    **range: (75 − d)/3** with d = `Distance` 0x1FD28 in units of 15 per tile (0 at 5 tiles, ±5 % per tile) · Sun
    Goggles (0x27) in face slot (+0x24): d −10 % · Sniper Scope (0x42) on main hand (+3) and aim > 0: d shortened
    beyond 10 tiles · target animation (+0x1E8 + 2·+0x1F3): ≤3 or ≥0x39 → −(d−20)/3 more, 5–7 or 14–16 → −20
    (crouch/prone? live: 0x04 standing, 0x1B gun raised, 0x15/0x0A moving) · d > 5 tiles and target +0x1B1 set with
    +0x1B3 ∈ {1,2,4,5,6,7}: −5 · conscious target (hp ≥ 15): tiles moved m = `tilesMoved` +0x1F2 (scaled by AP spent this turn), +agi/20
    + level/2 when it sees the shooter (+0x13A/+0x112 + slot == 1); m above the gun's tracking byte wpn[4] costs
    5 % per point · shooter wounds: −(c·2·lost/(3·maxHP))·(100 − (level−1)·10)/100 · energy < 100: −((100−e)·c/200)·
    (110 − dex)/100 · **beyond the gun's range (item table +0xB tiles × 15): halved**.
  - Weapon records 0x94C31 (6 B / type 1..16): class (1 pistol, 2 shotgun, 3 rifle, 4 knife), ammo item, extra AP,
    speed, tracking, b5 (damage?). Item records 0x94DF8 (12 B): +2 kind, +0xB range in tiles: .38 = 10, .45 11, 9mm
    12, .357 13, shotgun 16, M14 20, M16 25, Mod. M16 30.
  - Practical: a mrk-60 merc with a .38 at 13 tiles has ~15–35 %; at 7 tiles 50–90 % (aim 0–4). Fight inside 5–7
    tiles, keep the same target (+10, and no +1 AP), spend leftover AP on aim, crouch (−20 against standing shooters? —
    posture codes still to confirm), and stay out of the line between a shooter and the target (misses travel on).
  - **A successful roll is not a hit (VERIFIED live, Ph6 sector-59 fight):** `FireGun` only fixes the bullet's end point
    (+0xDE/+0xE0 = target tile px) and height +0xE2 (soldier target: Random band ≈ 2500 / 5000 / low, ÷6 when the target
    animation is ≤3 or ≥0x39 = prone; otherwise by terrain class 0x239CC: <6 → 0, 9 → 2000, else 5000). The projectile then
    flies and can be stopped by trees and bushes: three shots with +0x1BC = 1 (margin up to 56) did no damage when the
    line crossed vegetation, while shots over open grass hit. `tools/combat.py` hit % is the roll only; the flight /
    obstacle code is still to trace (from 0x598A8 and the projectile update).
  - **FireGun 0x5959C and AttackGo? 0x5AD50 mirrored (2026-10-22; ai_diff `fire_gun` / `attack_go`, 3000 cases each,
    0 mismatches, 9 hand mutants caught):** AnimFrame op 0x1EE runs StartAttack then AttackGo (stage 2, aiming animation
    0x1B / 0x1F / 0x23): FireGun (combat: focus on a seen enemy, effect tile 0x92E4C; SpendAP(ShotApCost + aim, attack
    sound 0x92E66[weapon class]); end point / height; ChanceToHit roll; a miss runs ThrowScatter with the margin and
    the Distance, then height ± height × margin / 100 − distance), then the skip distance +0x1FF (ChebPx to a seen
    target, or to the end point when SightLine answers), then the fire animation (0x2D point blank, 0x1C pistols, 0x20
    rifles, 0x24 knives). SightLine's "old flag" argument is AttackGo's caller's BL: the soldier record pointer's low
    byte, 0x58 in the game (hooked in walk_diff --shoot). Spec: shooting.md batch 5.

- **Damage, armour, movement (2026-10-18; VERIFIED by diff tests: `validate/diff/damage_diff.py` 4,000 cases with
  scripted Random, `move_diff.py` all 60 sectors' tiles + fuzz, `flight_diff.py`; ported in tools/combat.py; spec combat.md):**
  `BulletDamage` 0x39FD8, `HitLocationDamage` 0x38D80 (was ApplyStatLoss?: head/torso/legs by projectile height,
  helmet/body-armour absorb + wear tables 0x92F14/0x92F1E/0x92F20/0x92F3C; item 0x31 = Shielded Vest in the
  vest-pocket entry 6, body armour entry 5), `MoveCode` 0x2BE54 → `g_moveCode` 0x108258, `StepApCost` 0x39B30,
  `PathApCost` 0x1D32C (+ getting-up cost). Flags: `g_shiftHeld` 0x284F21 (sneak, base 2), soldier +0x177 (base 3,
  unconfirmed: crouch?), `g_altHeld` 0x284F20 (+1); `g_kbdFlags` 0x22A46C = BIOS shift word.
  Traced, NOT yet verified (keep out of the spec until checked):
  - `TakeDamage` 0x392FC (EAX soldier, EDX height, EBX damage, ECX kind: 1 bullet, 4 = also HitLocationDamage,
    2/3/6 others): hp −= min(dmg, hp); enemies (slot ≥ 0x28) under 15 hp → 0; StatPractice(level) for mercs;
    wounds = hpMax − hp − previous loss; hp < 15: collapse (+0x1AC = 1 under 10), hit/fall animations by posture
    (0x92E72 knockdown → anim 0x28/0x2B), AP loss ≈ damage/4 (+?) capped by AP left (0x329BC), +0x1DA killer slot,
    0x74304 notifies the attacker. 0x3928C = torso hit without a projectile (HitLocationDamage(s, 2500, n)).
  - Projectile: `LaunchProjectile` 0x58DE4 (start height +0xF2 = 4500 + Random(1001) for bullets, Bresenham setup,
    gun wear: condition −1 with chance 1/(2·item +3), ammo −1; targets closer than 21 px are hit at once),
    `ProjectileStep` 0x57BB8 (15 px per frame, +0xC8 flown += 1 per px), `ProjectileImpact` 0x5713C →
    TakeDamage(BulletDamage, kind 1) + StatPractice(marksmanship, 1). **`ProjectileHitTest` 0x577B4 is VERIFIED**
    (ported, `validate/diff/flight_diff.py` 20,000 cases): trees stop with chance 1/max(1, 25 − flown/15) — this is
    the live "hit roll succeeded but the tree took it" observation; rules in docs/spec/combat.md.
  - `ActionApCost` 0x56310: gun = ShotApCost + weapon extra AP unless the gun is already raised (animations 0x1B,
    0x1F, 0x23; then +1 if the facing has to change); knife = ShotApCost + PathApCost to an adjacent tile (99 when
    unreachable); thrown items 0x58B14.
  - `StatPractice` 0x62A68 + `g_statProgress` (see labels.csv): experience = per-stat practice counters; gain chance
    100 − (stat + progress/100), scaled by wisdom.
- **Ph6 sector capture + day loop (VERIFIED live, dumps build/mem_{s59start,kill41,kill43,kill40,won1,eod1,eod2,office2,
  day3,office2b}.bin; `./t sectors DUMP --check`, `./t savecmp SAVE DUMP`):**
  - **Fight won** in sector index 58 (UI 59), 8 mercs vs 4 enemies, ~12 turns, no merc lost (Hector −10 HP). Each kill:
    the death fields seen for Hurl (+0x183/+0x184/+0x18A/+0x138 → 0, +0x186 = FF, +0x188 = 1) and **+0x1DA = killer slot**
    (41 ← 5 Hurl, 43 ← 4 Vinny, 40 ← 7 Larry, 42 ← 0 Grunty); `g_sectors[58]+0x96+k` 1 → 0xFE for each dead enemy
    0x28+k; `g_sectorTbl9[58].b1` (enemies alive) 4 → 0. After the last kill the game leaves turn-based mode (DONE button
    gone) and `ResolveSectorBattle` 0x3E508 runs: no enemies + not owned → "GUARDS REGAIN SECTOR" path:
    `SetSectorOwned` 0x3DDE4 (tbl9[58].b0 0x18 → 0x19), `g_lastSectorGained` [0x22A020] = 58, `g_sectorsGained`
    [0x22A018] 0 → 1, RecordReplayEvent 'O', `RecalcTerritory` 0x50EA8.
  - **g_sectorTbl9 decoded** (all bytes now): b0 bit0 owned, bits 1–4 = per-side barrier 2 N / 4 E / 8 S / 0x10 W
    (`SetSectorEdge` 0x3DE20 sets the mirror bit in the neighbour; home 59: E+S, 58: S+W = coast/river sides),
    b1 enemies (NewGameInit 0x37A0C: 4/6/8 by difficulty, −2 for indices 48/49/58, +2 for 28/37 in one game mode, ≤ 8;
    home 0 enemies + b4 = 8 guards), b2 our mercs, b3 tappers, b4 guards, b6 Fallow trees, b7 workable ("green") trees,
    b8 unhealthy ("light brown") trees. **b7 = `SectorWorkable` 0x41E1C:** owned and `SectorConnectedToHome` 0x41178
    (path of owned sectors to 0x3B) → security = 25 % per side that is map edge / owned neighbour / barrier bit,
    b7 = min(b6 − b8, b6 · security / 100). Live: home 59 2 → 3 when 58 was taken (50 → 75 %), 58: 1 (75 % of 2).
    Matches the manual ("the safer a sector is ... adjoining sectors, the more the natives will do") and the End of Day
    screen (Trees owned = Σ b6 = 6, Harvestable = Σ b7 = 4, Sectors 2).
  - **Economy (RecalcTerritory 0x50EA8):** [0x22A014] owned count, [0x229FE0] Σ tappers, [0x229FE4] Σ guards,
    [0x229FB8] Σ trees, [0x229FB4] Σ b7, [0x229FBC] trees tapped = Σ min(b6 − b8, tappers/3) capped by
    `g_processCap` [0x22A024], [0x229FE8] workers, [0x229FEC] = `g_workerPool` [0x229FDC] − workers. Strategic screen
    live: TAPPERS on sector 60 adds 3 per click up to 3 × b7 ("No more tappers will work there!"), cost $35 per native.
    `g_processCap` = 0 so far → revenue $0 even with 9 tappers.
  - **Micro-Purifier:** item 0x79. `g_purifierPending` [0x22A0CB] → SetupSectorFromXra gives it to an enemy of the next
    non-home sector (entry 9, +0x6C) and stores 0x79 in `g_sectors+0x9E`; the death handler (0x3ABDA) clears +0x9E when
    the carrier dies. Live: ENEMY 40 carried it; it now lies in sector 58 (g_groundItems #8 = `79 63 64`). Probably the
    key to `g_processCap` (to trace).
  - **End of day (real time):** the day clock does not advance while idle; **C toggles compressed time**
    (`TacticalKeyHandler` 0x13490: toupper(ch) − 1 indexes jump table 0x13328; refused while enemies are near).
    End of Day screen (map + Merc/Native/Enemy deaths, Sectors/Trees owned/Harvestable + change, Rating 6000), then the
    team/finance screen: Harvested 0 × $500, natives $280 (8 × 35), team $2,585 (Σ salaries), balance 19,200 → 16,335 =
    `g_balance` [0x229FC4] (prev [0x229AC0], [0x229FD4] salaries, [0x229FFC] natives, [0x22A004] total). Overnight:
    Hurl marksmanship 60 → 61; wounds persist (Hector 69/79). g_day stays 2 in the office; the bed → "Day 3".
  - **Saves across the day boundary:** OFFICE.SAV is written when you Sleep (bed), DAY.SAV deleted then rewritten at the
    day start; both are now 57,993 B (sector files appended, layout above). `./t savecmp`: OFFICE.SAV = memory of the
    office on **every** block, SEC58/SEC59 copies byte-identical to the .TMP files; DAY.SAV (day 3) = memory except the
    soldier placement fields and 12 B of tactical state written after it. ABORT → NO restores OFFICE.SAV: memory equals
    the file again (0 diffs) and the loader re-extracts SEC58/59.TMP from it.
  - **REPLAY.REC** = 3-B records (from, to, code) written by `RecordReplayEvent` 0x3FA04. Day 2:
    `;;X ;:J ::M ::T ::T ::T ::T ::O ::Y` = entered 59, squad 59 → 58, combat, 4 enemy kills, sector gained, day end.
    `DayReplay` 0x652B8 re-applies codes J..] to a copy of the table (0x2254F0) for the strategic replay.
  - **AIM hire/fire (VERIFIED):** full team → "Your team is complete..." (`TeamSize` 0x513B8 > 7). Fire Larry → only
    profile 12 +0x3D 0 → 1 (soldier slot 7 untouched); hire Ears (profile 13, $375) → slot 7 rebuilt from the profile
    (nickname, +0x179 = 13, salary +0x110/+0x112 = 375, HP 72/72, his gear in entries 6–8), profile 13 +0x4B 0 → 50,
    profile 12 +0x4B 100 → 1. `MercRefusalReason` 0x219CC explains +0x3D: a merc you fired refuses to sign again
    (reason 9); other refusal fields: +0x35 availability day, +0x36/+0x37/+0x39 score thresholds, +0x38, +0x3B/+0x4D merc
    ids it reacts to, +0x3E, +0x4C buddy (see labels.csv).
- **.DAT archives** (MAIN, OFFICE, TITLE, ISLAND, SANIM*, STAB, MF1/2, DROWN, ENDGRAPH ... and CD DAT/0..9.DAT?):
  0x00 name[16] (own name), 0x10 u32 cap, 0x14 u32 count; entries at 0x20 + 0x20·i = name[16], u32 abs offset,
  u32 size, 8 zero; data from (cap+1)·0x20. Last member ends at EOF in every archive (`./t datarc`). From the files:
  the code-side reader (which opens dat\main.dat etc.) is not traced yet. Members: PCX/PCC (both plain 8-bit RLE PCX,
  `./t pcx`), FLC, SAB, sounds. MAIN.DAT: members `0`..`66` = per-merc face sheets (148x69: face + eye/mouth frames; index = profile id, 0 =
  Needle), FACES.PCX = small-portrait atlas, AIMSCRN.PCX (AIM screen background, 83% of shot 39 = all but text and
  portrait), plus map/panel/body art.
- **CD speech + quote text (2026-10-18; text VERIFIED: `validate/diff/edt_diff.py`, all 4787 records of the 11
  *QUOTE.EDT files split into words identically to the original `WrapNextWord` 0x36184 in Unicorn).** Tool: `./t speech`.
  - Every CD directory except DAT/ and MISC/ holds Sir-tech archives (same container as the .DAT files, `./t datarc`)
    named by speaker profile id: AIM/0..59, DAY/0..63, PST/0..59, NAMES/0..63, JAC/64 + 64B (quotes >= 80), QST/64,
    BRE/65, SAN/66, NAT/NAT, WPRO/WPRO1-2. Hard disk: SNDS/0..63 (combat grunts HIT/DIE/ATT .8), INV/1_54, 55_81,
    82_135, EXTRA, MONEY (item and money narration; MONEY mixes .VOC words "AND", "DOLLARS" with .8 numbers).
  - `PlayQuote` 0x24F08 (was PlayAimQuote?): category index → EDT\<CAT>QUOTE.EDT (table 0x24BA0: AIM DAY PST JAC INV
    SIG QST BRE NAT SAN INT) and archive dir (0x24C2F). Text = 200-byte record number row + quote, row = speaker x
    stride (call sites: AIM `imul 0xE`, DAY `shl 5 + id` = 33, PST `imul 0xF`; others row 0). Voice (play flag) =
    member "<id><CAT><quote>.8" in archive <CAT>\<id>, or the halves "...A.8" + "...B.8"; JAC quote >= 80 → 64B; NAT
    has no id prefix (NAT\NAT, "NAT30.8"). `ArcSeek` 0x534A4 = member lookup + fseek.
  - **.8 = raw unsigned 8-bit mono, 22050 Hz**: `SpeechStream` 0x5CC6C sets sample type 0 (8-bit mono) and every
    one of its 19 callers passes EAX = 0x5622. Effects (`PlaySfxSlot` 0x5CB34) mostly 0x2B11 = 11025 Hz. .VOC
    members (NAMES/, MONEY) are Creative Voice files with their own rate (`PlayMercVoice` 0x24D14: NAMES\<id>
    member "<nn>.VOC").
  - **EDT text encoding**: each byte except the space separator is stored +1, NUL-terminated, rest of the record is
    random filler (do not decode past the NUL). Decoding happens while word-wrapping (`WrapNextWord` subtracts 1 when
    its encrypted flag is set). Decoded 0xB1 = special glyph past the font (width 1; used like a dash between words, rendering not checked);
    0x7E..0x8B = Cyrillic letters of the game font (Ivan's DAY lines 594..626 are Russian); unused slots read
    "Member N Bug Q". Placeholders such as MEMBER are filled by `StrReplace` 0x252B0. INV = item descriptions
    (continued over several records), SIG = map sign texts, INT = intro lines, BIOQUOTE = AIM bios (several records
    per merc), WALQUOTE = radio/sound captions for WPRO.
  - **NEWPROF.EDT** = the initial `g_profiles` (64 x 0x5A, unencoded; `LoadNewProfEdt` 0x379B4). Ids 60-63 are the
    natives Elio, Hamous, Juan, Wahan ("Native # n"); saves store only the first 60. g_profiles is now profile[64].
  - **MISC patches** (0-based sector index in the names): `ApplyTileObjPatch` 0x317EC reads u16 tile + u16 object
    (t1) records (CRATE, LIVEPL, DEADPL); `ApplyItemPatch` 0x318C4 reads u16 tile + 12-byte item records (ITEM).
    Supply crates (`g_crateDrops` 0x95CC0: index 35 day 9, 17 day 17, 42 day 26, 3 day 31) and the **processing
    plants** (`g_plants` 0x22A12A, PL = plant building: LIVEPL intact, DEADPL destroyed) use them.
    MISC\ITWORKS.8 not referenced by name (unused?).
- **Screens are 320x200** (menus, office, options), PCX palettes 8-bit (→ DAC v>>2). PCC sprites carry their own
  palette, but the game draws them with the current screen palette (CASE1 at (152,91) only matches that way).
- **Tactical map graphics (2026-10-18, VERIFIED: `validate/diff/tiles_diff.py`, 600/600 random views of all 60
  sectors / 10 tilesets byte-identical to the original passes replayed in Unicorn).** Tools: `./t csprite`,
  `./t sectormap SECTOR|live` (docstrings hold the full rules).
  - **TILES%d.DAT = compiled sprites**: u32 N (1327), N u32 offsets relative to the table end, then x86 code.
    `LoadOffsetBank` 0x52E00 reads the file into `g_tileSprites` 0xA8C48, drops the count, offsets → pointers;
    `DrawCompiledSprite` 0x52DC8 sets EDI = dest + y·pitch + x, EBX = pitch and *calls* the sprite: `mov byte/word/dword
    [edi+k], imm` per opaque run, `add edi, ebx` per row, `ret`, often a `nop` + u16 w + u16 h trailer. Entries
    0..156 ground (t0), 157.. objects (157+t1), 413.. overlays/roofs (413+t2, 413+t3), ~450 4x3 mini tiles (overview
    map?), UI icons, and 1293..1326 = **raw** 16x14 shadow bitmaps (not code). SINV.DAT is the same format (118);
    SH.DAT / INV.DAT are not (todo). Renderer executes each sprite in Unicorn and records its writes
    (gotcha: `ctl_remove_cache` after rewriting code at the same address, Unicorn otherwise reruns stale blocks).
  - **TERRAIN.DAT** = 60 × 0x3D41: 3136 × 5 B (t0..t4) stored last tile first, then the tileset byte (`g_tileset`
    0x22A038 → TILES%d.DAT, MISC\TILE%d.PAL 6-bit VGA, MISC\TILESET%d.LST = remap table 28 with fixed overrides).
  - **JAGGED.DAT tail** (LoadJaggedDat; section sizes add up to the 22696-byte file): 0x1540 B at 0x106C18, 34 × 256
    remap tables (`g_remapTables` 0xE8C48 → 0xE8D00..), 157 B `g_t0Class`, 256 B `g_t1Class`, 155 B `g_t2Class`.
  - **Redraw** = `TacticalRedraw` 0x176C8 (was "DrawTacticalModeBanner?"), back buffer `g_backBuffer` 0x22A4EF pitch
    0x180, window (14 × 12 tiles, `g_viewCols/Rows`) at (32,28): ground pass 0x169E8 → shadows 0x16C48 (RemapBlit
    0x52425: key 0 skip, key 0x17 darken dst via table 28, else table 0) → 0x16DD0 → objects + soldiers 0x17028/0x16E3C
    (window + 1 tile border; 2x2 composites 0x164BC) → overlays + roofs 0x17098 → projectiles 0x17548 → cursor 0x171E8
    → banners → copy to VGA. **t3 = roof/interior**: t3 > 0x50 draws the roof sprite and `g_t3GroundVisible[t3]` hides
    ground/objects under it (that is why t3 "changes as mercs move": entering a building opens it). Quirks kept:
    shadow table read as dword>>24 (real base 0x95E64, not 0x95E61); a shadow at y<0 skips rows but still blits 14.
  - The live back buffer is useless as a reference: the game redraws almost continuously, so a frozen read lands
    mid-pass (16 distinct images in 20 reads); the play area never exists in the 320x200 buffer *0x93E60 (straight to
    VGA). Hence the Unicorn replay: `Emu(dump=...)` maps the 16 MB snapshot flat and calls `e.rt(image_addr)`.

### Pathfinder and enemy AI (2026-10-18; VERIFIED by diff tests, spec combat.md "Finding the path" and ai.md)

- `FindPath` 0x40810 (A*, ported tools/combat.py `find_path`, validate/diff/path_diff.py: real map + fuzzed move
  codes/occupants + open field / maze, 0 mismatches). Node pool g_pathNodes 0x288F50 (512 x 16 B: next, prev, tile,
  g, h, trail; node 0 = open-list head sorted by g+h, ties by Manhattan distance to the goal; node 0x1FF = closed
  head, recycled oldest-first once nodes 2..0x1FE are used), trails g_pathTrails 0x284F50 (4096 x parent+dir; out =
  no path), best cost g_pathBest 0x28AF50. Result: g_paths 0x109BD8 + slot*0x50 (directions 0 N .. 7 NW clockwise,
  max 80), soldier +0xD4 count, +0x1AF = 0. Edge costs = move codes with the special cases in the spec.
  Callers besides PathApCost: 0x19818, 0x19D74, 0x1EF34, 0x3DC4C (ReachTile), 0x72578 (CanStandAt?).
- AI decision layer ported to tools/ai.py, diff-tested by validate/diff/ai_diff.py (each function run in Unicorn
  with Random and the not-yet-ported searches stubbed by the same script on both sides; soldier record, globals and
  the ordered call list compared; a one-constant mutation is caught): `AiThink` 0x6D1A8 → `UpdateAlertState`
  0x6D574, `ChooseAiAction` 0x6D7C0 → `CalmAction` 0x6DB10 / `AlertAction` 0x6DEE8 / `CombatAction` 0x6E3F4 /
  `EngagedAction` 0x6EB54, `AiActionAffordable` 0x6D864, `AiDoAction` 0x6D904, `InWater` 0x23A34. Soldier fields:
  +0xBA aiType, +0xBB alert, +0x131 bypassToGreen, +0x132 aiTarget, +0x1D1 aiAction, +0x1D3 attitude / +0x1D4 orders, +0x1D6,
  +0xF0, +0x1C1 rethink, +0x1B0 turnDone ... (types.toml). Quirks kept by the port: ChooseAiAction returns the
  caller's EDX for alert > 3; EngagedAction's hit-chance threshold is uninitialised for situation scores > 4.
  Scripted mission globals: g_missionOrder 0x92DA8 (slot + tile), g_missionStep 0x22A0D0, spot 0x92E50, radio item
  0x40, done flag 0x22A1FB per sector; g_alarm 0x22A0CE, g_alarmChance 0x95C5A (50/80/95 by difficulty).
- Situation score ported + diff-tested (ai_diff fuzzes all 24 soldier slots and the knowledge tables): `OppStrength`
  0x6F314 → `SoldierPower` 0x6F8DC → `ArmorRating` 0x55500 (+ MercMaxAP). Knowledge: g_known 0x10D860 (i8 per slot,
  what the other side knows: 1 seen now, 2..4 older, < 0 heard; written by the perception code 0x72F50..0x746D4),
  soldier +0x13A sight flags (same levels), g_rememberE 0x10D890 / g_rememberM 0x10D9D0 (per-pair flags),
  g_fresher 0x95DE7 ([old*8+new] > 0: new fresher; 1 > 2 > −1 > 3 > −2 > 4 > −3 > 0), weights 0x6D1A0 in the code segment
  (60 70 80 50 100 90 80 70 % for levels −3..4). g_santinoSpoke 0x22A0C2: enemies −2 once he has spoken.
- `TargetTile` 0x70114 (nearest known opponent: own last-seen tile g_lastSeenE 0x10DA90 / g_lastSeenM 0x10E190 when
  the sight flag is fresher, else the opponent's +0x108 knownTile) and `UpdateSightings` 0x740FC (SightCheck?
  0x72F50 per flagged opponent, then optional aging; AiThink passes 0, 0x11FB4 passes 1) ported + diff-tested.
  Enemies scan opponent slots 0..0x27, so soldier +0x13A sees is 40 bytes (types.toml).
- `MoveApCost` 0x1B118 (cheapest neighbour step, 99 none) with `OffMapStep` 0x46F70 / `StepTile` 0x46FF4 /
  `DirStep` 0x47010 (table 0x92FBA) ported + diff-tested (ai_diff with edge tiles and random move codes).
- Calls / noise ported + diff-tested: `NoiseTile` 0x7038C (writes its score through EDX; "+0x120" in the old notes
  is really the word at +0x122 = noiseGridno), `AllyCallTile` 0x702A8 (was LastSeenTile?: nearest ally in combat or
  g_lastSpotter 0x22A108, by PathApCost), `NewsValue?` 0x705F4 (was AlliesNear?: call level above the side's +
  freshness gained), `TileDistance` 0x15A24 (Chebyshev). Side calls g_callM 0x92DBA / g_callE 0x92DBE (tile, level)
  set by ShareSightings from soldier +0x122 noiseGridno / +0x1C0 noiseVolume.
- Positions ported + diff-tested (CanStandAt?, PathApCost, TileDistance, Random stubbed): `PatrolTile` 0x6FAEC,
  `BuddyTile` 0x6FC90 (was SpotTile?: next to a random living comrade), `ApproachTile` 0x7068C, `AdvanceTile`
  0x6FE7C. Leash by orders +0x1D4 (1/5/15/99) around the post +0x12E. Quirks: Approach/Buddy clear a
  tried-directions array but never fill it; BuddyTile's slot range is uninitialised for AI types > 4.
  `CanStandAt` 0x72578 (FindPath stubbed), `TileStandable` 0x23538 (direct test), `FindDryTile` 0x713D0 too.
  `RetreatTile` 0x70FD0 (was ObjectiveTile?: box 3/5/7 by difficulty, table 0x95C60, maximise the nearest known
  opponent's distance); "objective" in older notes = this retreat. `SearchTile` 0x70850 + `PositionScore` 0x6F750
  ((300 - dist) x power x line-of-fire difference, stack args power/dist, ret 8) ported + diff-tested.
  `GunOption` 0x71590 + `ExpectedDamage` 0x7208C ported + diff-tested (ChanceToHit / ShotApCost stubbed: both are
  diff-tested on their own). Gotcha: dword [s+0xB7] >> 24 is +0xBA (AI type), [s+0xB8] >> 24 is +0xBB (alert).
  `KnifeOption` 0x71DD4, `MeleeChance` 0x56A78, `KnifeDamage` 0x723D0 ported + diff-tested; ai_diff now counts a
  divide-by-zero in the original (faulting div/idiv) as a match when the port raises ZeroDivisionError too.
  `ThrowOption` 0x717F8, `ThrowDamage` 0x72250, `ThrowApCost` 0x58B14, `ThrowChance` 0x58BB0 ported + diff-tested.
  `LineOfFire` 0x580CC (Bresenham + height model; ProjectileHitTest 0x577B4 and SightLine 0x58760 stubbed with
  their pointer outputs) and `ChebPx` 0x1FD8C ported + diff-tested.
- `SightCheck` 0x72F50 + `MarkSeen` 0x732FC + `NoAllySees` 0x73268 ported + diff-tested (SightLine 0x58760,
  DirectionTo, TileDistance, ClearRoomFlags? 0x61744 stubbed; ai_diff now also compares g_known, the remember and
  last-seen tables, and the opponent's record). Vision cone g_sightRange 0x92FC3 [facing*8 + dir]: 14/12/8/3/0
  tiles at 0/45/90/135/180 degrees; 14 regardless while a flag is set. `BumpOrders` 0x6F29C = UpdateAlertState's
  tail (also MarkSeen, MsgHearsNoise?). Santino (type 4) sees whoever has flag 1 on slot 0x28.
- `SectorValue` 0x3E03C (was SectorStrength?: enemy interest in a sector, 20..100; also used by the strategic
  FUN_3ede8) ported + diff-tested (direct call over fuzzed sector table / story bytes / plants / day;
  SectorConnectedToHome and QuestCheck? 0x633D8 stubbed). Story bytes 0x22A046/49/4B/4D/55/56/57 still unnamed.
- `ShareSightings` 0x736BC (was GuardSeesEnemy?: pushes a soldier's sight flags into g_known + knownTile, counts
  new sightings / noises for mercs and guards, merc reaction 0x56D28, guard message, side call) ported +
  diff-tested (ai_diff compares all 48 soldier records for it).
- `AiIdle` 0x5AFA4 + `ClearTarget` 0x56ED0 ported + diff-tested (AnimStep? 0x1F2C4 stubbed).
- `ReachTile` 0x3DC4C (+ `UseSideOk` 0x3DC04; writes g_missionSpot 0x92E50) ported + diff-tested; ai_diff's
  find_path stub takes (found, pathLen) tuples to model FindPath setting +0xD4.
- `AttackReady` 0x584A0 (was AimCheck?) + `HandGunState` 0x56F20 ported + diff-tested.
- `SightLine` 0x58760 + `TileOpacity` 0x23818 ported + diff-tested (direct call over a fuzzed map, window table and
  g_sightBlock 0x9567C; 4 surviving mutations are equivalent: diagonal major-axis choice, limit clamp below 0,
  brush re-query on entry, steps/15/2 vs steps/30).
- AI effect routines: `TurnTo` 0x1AA18 (+ `TurnStep` 0x19540, `ApShort` 0x1A9A8) and `WalkTo` 0x195A0 ported +
  diff-tested; WalkTo is only a wrapper: the walking is AnimStep? 0x1F2C4 (the per-tick animation routine).
  compose_diff now runs them as ports (SetAnim 0x17F90, sound stay leaves).
- `AnimStep` 0x1F2C4 (+ AtWaypoint 0x1EE84, AnimCanWalk 0x1EEEC, StartLeg 0x1EF34, DirectionPx 0x15A90, AxisRatio
  0x383E0, ClearTask 0x3DDB8) ported + diff-tested and wired into compose_diff (0 mismatches) and live_ai (25/25).
  Leaves left: EndTask? 0x3D98C, ReplaceSoldier? 0x2FB98, TacticalClick 0x59C14, sound.
- `SetAnim` 0x17F90 (+ AnimDelay 0x15BB8, MsgNotEnoughAP 0x15960) ported + diff-tested; in compose_diff and live_ai.
  The frame-table pointer +0x90 holds a runtime address (World.ptr / raw16).
- `AnimFrame` 0x18498 (the animation script interpreter) + `AnimNext` 0x19470, AnimTurn 0x724E0, TurnOne 0x14F7C,
  RotateDir 0x14FEC, WrapDir 0x1FD10, OnScreen 0x1FC34, OccupyTile 0x23974, VacateTile 0x2395C, NextPathDir 0x195F8
  ported + diff-tested (ai_diff with fuzzed scripts on the scratch heap; tile map compared); in compose_diff and
  live_ai. The whole AI → walk → animation chain now runs as port; leaves are effects (sounds, doors, mines,
  projectile, death, speech, UI).
- `EndTask` 0x3D98C (flag = whole EDX) and `ReplaceSoldier` 0x2FB98 ported + diff-tested; ai_diff MEMRUN mode runs a
  port directly on emulator memory when it writes records that may be the soldier's own.
- `StopAnim` 0x196F8, `SnapToTile` 0x197D8, `StopWalk` 0x104C8 ported + diff-tested; in compose_diff.
- `TacticalClick` 0x59C14 (the use/attack dispatcher of AI and player) + JamCheck 0x59A54, AimPosture 0x5AA08,
  UseCost 0x59AE8, TileItemIndex 0x23768 ported + diff-tested; in compose_diff. Ghidra's pseudocode for
  TacticalClick is unusable (stack confusion): ported from the asm. DirectionAtan? 0x5260C (x87 atan2) stays a
  leaf (ai_diff scripts it; compose_diff runs the original). Unsampled: exact reach-distance boundaries.
- `FireNow` 0x57B34 and `LaunchProjectile` 0x58DE4 ported + diff-tested; in compose_diff. Finding: enemies
  (slot >= 8) firing guns spend no ammo and cause no wear (only mercs and throwers do).
- `ShotEnd` 0x56DD4 and `ProjectileStep` 0x57BB8 ported + diff-tested (ProjectileHitTest stubbed; diff-tested
  itself in flight_diff). A quirk kept: the no-shadow test compares the x word (+0xE4) with 0xC41.
- Window walls (objects 0x7F..0x82, only sector 60 has them) in an enemy line of fire: no real dump has an enemy
  line across one, so `compose_diff --windows` plants them between the AI soldier and another soldier. The window
  rule (0x57AB7) scales LineOfFire's chance per pixel; lines that reach the target end at 5..16 %. A port that
  skips the scaling changes 7 of 300 enemy decisions; a small error in the scale changes none (2026-11-06).
- `ProjectileImpact` 0x5713C ported + diff-tested; in compose_diff (BulletDamage / SectorDoorLookup as oracles).
  Leaves: explosion 0x3B79C, landing item 0x56FE4, effects 0x157BC / 0x156B8, 0x3ACB8.
- `TakeDamage` 0x392FC ported + diff-tested (34 mutants: 28 caught, 1 bad anchor, 5 equivalent with the real data: the AP table has no 99 for queued aims, 0x90914 = 0, two `<`/`<=` ties); in compose_diff (HitLocationDamage as oracle). An attacker that is the soldier itself reads its own record, already changed by the hit (aliasing; the port must use the record, not a copy).
- `HitReaction?` 0x74304 + `HitAlert?` 0x743E8 ported + diff-tested (mutants: 8/9 and 18/19 caught; the escapes are a log mask and writing 0xFF over 0xFF). compose_diff `--flight` (2026-10-19) composes launch → flight → impact → BulletDamage / HitLocationDamage (tools/combat.py ports, no longer oracles) → TakeDamage → reaction per whole shot.
- Interrupts: `InterruptCheck?` 0x73AB4, `MercInterrupt?` 0x73D28, `EnemyInterrupt?` 0x73F3C ported + diff-tested (mutants 22/22, 30/31 caught; the escape subtracts 0). In compose --flight. Found while composing: HitLocationDamage also writes the stat-loss message (0x22A1F1 slot, 0x92DE8 = 500, 0x22A02F amount, texts at 0xFA908 / 0xFA918; head hit without a loss: any soldier, amount 0xFF). tools/combat.py lacked it and damage_diff did not compare those bytes; both fixed.
- `MakeNoise` 0x727D0 ported + diff-tested (direct call, 20/20 mutants caught); in compose. Found by compose --flight (mem_fight7, adjacent shooter and target): LaunchProjectile's short-shot path (end within 20 px) reads the attack tile +0xDC before ShotEnd clears it; the port read it after. ai_diff stubs ShotEnd, so only the composed test could see it. compose --flight now seeds each dump on its own name (a failing shot reproduces on its dump alone).
- `MsgHearsNoise?` 0x72A60 ported + diff-tested (25/27 mutants; 2 equivalent); in compose. Its sprintf_ (Watcom clib) faults in Unicorn on a segment `pop es`: Emu.stub_sprintf formats %s/%d/%u/%c in Python (ai_diff, compose_diff, live_ai).
- `Rest?` 0x32754 (fatigue +0x10C / energy +0x18A; was guessed "morale") and `ClearPath` 0x1A460 (+0xD4 = 0; was guessed "knocked out") ported + diff-tested; 7/11 mutants caught, the 4 escapes are dead code behind the 0..10000 clamp. In compose.
- `HidePathMarks?` 0x1D8B8 (the leaf known as "ui_refresh") ported + diff-tested (direct call, 6/6 mutants); `0x1D8A4` is a wrapper that falls into it with force = preview flag set (its 0x1D946 tail: xor eax / jmp). In compose.
- `Explode` 0x3B79C ported + diff-tested (direct call, 14/14 mutants after widening the fuzz); in compose --flight, which now throws explosives (+0x1C9 fuzzed, 30 % of shots). Puff calls now pass and compare their effect-list pointer (World.ptr) instead of skipping it.
- `LandItem?` 0x56FE4 ported + diff-tested (8/10 mutants; 2 equivalent); in compose --flight.
- Turn start: `StartTurn?` 0x11FB4, `Bleed?` 0x15034, `Drown?` 0x3843C ported + diff-tested (37 mutants: 32 caught at first, 2 more after fuzzing +0x1E3 / +0x16D; 4 equivalent or unobservable). New compose_diff `--turn` mode (StartTurn on every dump, TakeDamage composed); in ./t check.
- `ReviveRoll?` 0x401B4 (AnimFrame op 0x1EA) ported + diff-tested (16 mutants: 13 caught; 3 equivalent: the 95 cap and a negative chance absorb them); in compose (AiThink path).
- `MeleeHit?` 0x3521C (AnimFrame op 0x1EB) ported + diff-tested (25 mutants: 24 caught after fuzz fixes, 1 equivalent: condition 11..19 never wears). New compose_diff `--melee` mode (MeleeChance as an oracle with two soldier pointers); in ./t check.
- `SoldierDies?` 0x3A8C0 ported + diff-tested (34 mutants: 33 caught after fuzz fixes, 1 masked by the byte log). In compose (reached 40x in 240 --turn starts: drowning and bleeding deaths).
- Blasts: `Blast?` 0x38B60, `BlastTile?` 0x38638, `BlastWear?` 0x389BC (+0x38AC8), `ObjBlock?` 0x23780 ported + diff-tested (direct calls; mutants: 33 + 24, all non-equivalent ones caught after fuzzing items and blaster slots). New compose_diff `--blast` mode (in ./t check). `GasTick?` 0x38520 (per-turn gas clouds) ported, composed in --blast (3/3 mutants caught there). Still a leaf: the effect step that calls Blast (0x173B4).
- `NewTurn?` 0x126C8 (the side's turn change) ported; composed in the new compose_diff `--newturn` mode (guards' AiThink, GasTick, StartTurn for all; UI calls are leaves with scripted returns via LEAF_RET, the same list on both sides). Mutants via compose: 10/12 caught; open: the r = 8 boundary of the "shown merc down" path (rarely sampled). In ./t check.
- `ItqPop?` 0x12C14, `NextTurn?` 0x12D58, `InterruptTurn?` 0x1304C ported + diff-tested (direct calls, 16/16 mutants caught); composed in all compose modes (they were leaves).
- `ShowTurnBanner?` 0x1054C is the turn START of the selected soldier, not only UI (2026-10-22): not in play / hp < 15
  -> +0x1B0 = 1; else (turn not done, AI type != 2) banner text 0x10E380 + colour 0x92DDE (5 kinds), +0x197 = 0,
  0x73578 / 0x7351C + ShareSightings, UpdateAlertState, alert 2/3 -> +0x1C1 = 1 else `AllyAlert?` 0x74A60, then
  0x92DB0 = 15000 and turn-based 0x92DAC = 0 (15 s of real time; the 10 ms timer at 0x5F03A counts 0x92DB0 down by
  10 and sets 0x92DAC = 1; TacticalLoop 0x10AB7 restarts it when selected >= 8, 0x92DAC = 0 and 0x92DB0 <= 0),
  MercView think 1. Mirrored: ai.turn_start, ai.ally_alert, soldier_ticks fast timer (ai_diff turn_start: 11
  hand mutants caught). Spec: turns.md Turn start (library op 123, host op 194 banner).
- The mercs get their turn back through 0x1148C (op 120 low_ap_stop, "select the soldier"): New turn step 14e calls
  it; stubbed, selected stays 0x2F and the next frame starts the enemies' turn again (walk_diff --fight runs it real).
  Mirrored as ai.select_merc (0x1148C) + ai.select_soldier (0x17D80) (ai_diff select_merc, 10 of 11 hand mutants
  caught, the survivor is equivalent). Open for the spec: with +0x16D the path preview calls PathApCost with BL = CL =
  1 (marks in tile byte 7 + list 0x10DF90 / count 0x229E08, and one step more).
- TacticalLoop pass start 0x10800..0x108D8 (read 2026-10-22, UI only): g_lossTimer 0x92DE8 = 0 and g_lossSlot 0x22A1F1 <
  0x40 -> lossTimer = -1; s8 amount 0x22A02F >= 0: sprintf stat-loss text (name +0x16E, part 0xFA908, stat 0xFA918,
  0x9AA43 when amount 1 else 0x9AA5D) + ShowMessage?(0xEBAC0, 0, 0, 1, 1, 0, 2000, 1); amount -1: MsgMercText(name
  table 0x100958[slot], 0x98F07, 800); below -1: TeamQuote?(0x1C, 0xE). The front-end shows this (host op 183
  loss_msg sets the values). Every pass ends with the leave tests (0x1130D) and the leave counter [ebp-4] is
  tested only there: Update end runs also after a One frame result != 0. LoadTacticalSector? sets the turn step
  timer 0x229D24 = 10 (0x2AE2F).
- TacticalLoop head 0x108D8..0x10AC1 (2026-10-22 late, MIRRORED: ai.loop_head + ai.combat_end 0x106F4; ai_diff
  loop_head 500 cases 0, 21 hand mutants; not spec'd / ported yet). Turn timers every 100 ms (0x229D24, 10 ms timer
  -10 after the gate): screen not drawn -> s16 0x22A484 + 1 up to 20; drawn -> 0x22A484 - 1, at 0 the turn timer
  0x22A486 - 1 when new_round = 1. 0x229E00 < 0x30 -> 0x56D3C weapon status text (UI). COMBAT START: sector byte 1
  (enemies) != 0, combat 0x92DCB = 0 and sector byte 2 (mercs here) != 0 -> combat = 1, 0x67E18, merc_to_show >= 8 ->
  FirstMerc, NewTurn(0), LoadStripPccs(0x92DDF < 3, 1), turn_shown = 1 (0x11D40(0) is a no-op); turn clock = -1.
  COMBAT END (no enemies, combat on): combat = 0, clock 20000, turn_shown 0, 0x106F4 (soldiers 0..0x27 in play with
  hp: AP 1, +0x1B0/+0x194/+0x17E/+0x197 = 0, AiIdle; screen_drawn = new_round = 1, selected >= 8 -> FirstMerc, select
  light 0x46A4C(sel, 0xC), 0x92DDC = 0x92DCE = 0, LoadStripPccs(1, 1)). Not in combat: clock 0 and busy < 1 ->
  busy = 0, NewTurn. redraw_pending 0x92DA4 -> NewTurn. Then busy timer 5000, fast timer 15000 starts. After the key
  handler (UI): music choice 0x9635E (UI), fuse beep 0x284F29 -> PlaySfxSlot(0x2C, 0x2B11, 0x68, 1) and clear, story
  finder 0x92DE3 hp 15+ -> 0x6179C, Santino 0x22A0BF = 1 -> 0x22A0C2 + 1, FocusOn(0x28, 1, 0), TacticalClick(soldier
  0, 0x123) (not read further).
- TacticalLoop after the soldier loop (2026-10-22; MIRRORED 2026-10-22 evening: ai.update_rest 0x10F4E..0x111F5,
  ai.fx_fuses 0x11228..0x1130D, ai.start_tile_anim 0x15720, soldier_ticks + ambient / blink / ripple timers and the
  fuse countdown 0x5F214; ai_diff loop_rest / fuses / ticks; spec soldier_update.md Update end, Fuses). Corrections
  to the notes below: the blink counter is byte 0x284F04 (s8 > 2 -> 0; the code reads dword 0x284F01 >> 24), the
  splash byte is 0x92E76 + sector, Random(20) and the sound happen only under BOTH ambient timers, the fuse
  pass's kind 0xA goes off on any map tile (item or not), the fuse countdown's +0xC beep part runs only while +0 > 0.
  Leave tests (0x1130D, MIRRORED 2026-10-22: ai.leave_tests, ai_diff leave_tests 0 mismatches, 4/4 hand mutants): 0x2299AF (row 59 = home sector, byte 0) bit 0 clear -> "Home sector LOST!" (0x9AC65, DH 4),
  0x93449 = 4, counter + 1; 0x22A05C != 0 and s8 0x22A04E < 2 -> counter + 1. Counter 0 -> next pass (0x10800).
  Leaving: 0x52908(0xEDA08, 0, 0x38, 0x100, 0) (palette fade), 0x53660, 0x5C82C, 0x284F13 = 0, 0x96360 = -1; when
  0x92E74 = 0 and 0x93449 = 0: EndOfDayScreen(.., 0), sector byte 2 > 0 -> RecordReplayEvent(sector, 0x59) + byte 2
  = 0, 0x30D6C(1), Santino spoke and 0x93449 = 0 -> 0x93449 = 1; then 0x5F298 (timer off), sector = 0xFF.
  (MIRRORED 2026-10-23: ai.leave_path 0x11358..0x1140F, ai_diff leave_path 0 mismatches, 62/64 mutants (2 equivalent:
  the fade table address is an "x" argument, never compared; the 0x93449+1 survivor is killed since the fuzz sets 0x9344A); 0x5C82C = music stop (AIL_end_sequence, 0x9635E = -1), 0x5F298 =
  AIL_release_timer_handle. ai.leave_sector = LeaveSectorCleanup? 0x30D6C(AL flag): mercs 0..7 flag -> +0x17E 8 -> 0,
  hp 0 in sector/in play/+0x188 0 -> MercDies(1); enemy_up (0x28..0x2F) / guard_up (8..0x27, mercs NOT counted) = in
  sector with s8 hp >= 15; dying (0 < hp < 15) enemies killed when !enemy_up or guard_up, dying 8..0x27 when !guard_up
  or enemy_up (SoldierDies 0), then hp 0 -> MercDies(0); sector enemies != 0: guards/tappers or owned ->
  ResolveSectorBattle(cur, 1); none and not owned -> SectorGained(cur); SaveSectorTmp. ai_diff leave_sector 0
  mismatches, 111/113 mutants after the 2026-10-23 fuzz fix (weak sides + one lone soldier at the range edges);
  the 2 survivors read record 0x30, past the table. Spec: soldier_update.md "Leaving the sector", ops 214..220.)
  Original notes: Update pass (flag 0x284F14 = 0), in order: (1) 0x10F4E: sector table 0x22979C (9 B per sector) byte 0 bit 0
  clear and byte 1 = 0: 0x3FA54(s8 0x22A039), StatPractice(merc, 9, 10, 0) for mercs 0..7 in play with hp 15+;
  sector 0: 0x61688(0x1D, 0x63, 1). Bit 0 set and byte 1 > 0 and 0x22A0CF = 0: MsgEnemiesNearby, 0x5C5EC(2, 0).
  (Conditions to confirm: a one-time arrival event?) (2) 0x11014: blink timer 0x229D48 <= 0: 0x33768(0), 0x284F06
  ^= 1, timer 800 / 400, 0x284F04 cycles (reset when s8 0x284F01 > 2), 0x93AAA = 190 + 2 x it (UI). (3) 0x11078
  WATER RIPPLES: timer 0x92EF8 <= 0 (the 10 ms timer takes 10 off, same gates as front_ticks; sector start = 250):
  t = Random(3136) (0x229AFC); TileGround(t) >= 0x97, TileObject (byte 1) = 0, TileOccupant >= 0x40, Random(s8
  splash[sector] = 0x92E76 + sector) = 0: StartTileAnim 0x15720(t, frames 0x95659, count 0x12); then timer = s8
  splash[sector] x 12. These Random draws are in the game's one random sequence: a front-end must make them.
  0x15720 (EAX tile, EDX frame table, BL count): first record k = 1..9 of 0x229E24 (11 B: s8 count, i32 timer +1,
  s16 tile +5, frame table +7; record 0 is never used) with count 0: count = BL - 1, +7 = EDX, +5 = tile (timer
  not set); count != 0: tile byte 3 = k. No free record: nothing. Other callers: DrawProjectilePass? 0x1769B and
  OverviewRedraw (was ShowTacticalModeBanner?) 0x24068 (effect kind 5 on a tile with byte 3 = 0 and byte 0 < 0x6C: frames 0x9562E, count
  0x2B, sfx 0x1D). (4) 0x1110C AMBIENT SOUND (option 0x1163C0, timer 0x229D30 <= 0): 0x528AC(0xEDA08, 0xAC, 0xB2),
  timer 200; second timer 0x229D34 <= 0: 0x528AC(.., 0xA5, 0xAB), 300, 0x284F19 counter (+1 up to 0xFE, else
  0xF8); Random(20) = 0 and time compression 0x9300C = 0: PlaySfxSlot(Random(6) + 0x24, 0x2B11, 0x23, 1); then
  0x52870(0xEDA08, 0xA5, 0xB2). (5) 0x284F14 = 1. Draw pass (flag 1): time compression 0 -> 0x1CB78(0x92DD4) draw,
  frame count; else flag = 0. Then EVERY pass (0x11228): the 5 timed effects 0x22A488: i32 +0 = 0 (exactly) and s8
  +9 <= 0: +0 = -1; tile s16 +6 < 0xC41: (TileItemIndex < 0xFF and ExplosiveKind(ground type) = +8) or +8 = 0xA:
  Explode(+0xB, tile, +8, 1); tile >= 0xC41: 0x355D0(+8, s8 +0xA (push), s16 +4, +0xB, s8 +0xC) (as turns.md End
  turn step 1, but +0 = 0 and the kind-0xA case; the fuse +0 counts down in the 10 ms timer 0x5F214: +0 > 0 and
  s8 +9 <= 0: +0 -= 10, floor 0; +0xC != 0 with 0x229E0C > 0: -10, at <= 0 0x284F29 = 1 and 0x229E0C = 700; not
  in ai.soldier_ticks yet). Then the day-end tests (0x2299AF bit 0, 0x22A05C / 0x22A04B,
  0x93449) and the leave path (EndOfDayScreen, RecordReplayEvent 0x59, 0x30D6C(1), 0x5F298, sector = 0xFF).
- **Doors (2026-10-22, diff-tested: ai_diff door_fx 4000 cases 0 mismatches, 163/167 mutants):** a closed door (objects 0x3D/0x3E/0x43/0x45/0x5C..., move code 1500,
  tile_standable < 0) is not a walk target: MoveMercTo refuses it, FindPath only enters 1500 as the destination. The map
  click UseItemAt? 0x1D9B8 keeps a cursor mode 0x284F1B (1 walk, 2 item, 3 hand; right click steps it 1->2->3->1) and a
  cursor 0x284F1D; hovering a door in walk mode gives cursor 0xA4 (seen door) and a left click then runs
  TacticalClick(s, tile, 0) (the hand): walk next to it, Use (3 AP, anim 0x26, task 0xFF); the animation's op 0x1C2 calls
  PlayDoorFx? 0x3B03C (host op 70 door_fx). PlayDoorFx: door value v (SectorDoorLookup?, > 100 = big, v - 100); v 0 ->
  open (r 11); lock pick 0x4C: skill = (wisdom +0x18D x mechanical +0x190 / 100) x dexterity +0x18C / 100 + level
  +0x192 x 5 (the asm reads them as dwords at +0x18A / +0x18D / +0x189 / +0x18F shifted right 24); condition x skill / 100 >= v -> 10 (picked, 3 StatPractice) or 7 (Random(100) < v wears it); keys
  0x3AE78 (0x57, 0x58 big; ids in bytes 2, 8..11; uses the id up) -> 9, else 6 (key in hand) / 8. UI: FX\I_DR|I_PD +
  KEY|PCK|HAN .BIN sound + .FLC (tileset digit), then the outcome with I -> F (r < 9) / S. r >= 9: BreakObject(tile, obj,
  1), MakeNoise(slot, tile, +0x1B3, Random(3), kind 2), +0x9D = 1. Mirrored as ai.door_fx / ai.use_key. Bug in
  0x3AE78 (confirmed by the diff test, issue #2 row 9): a key id 1..3 from a pocket key ring clears byte 0x0B of the
  HAND item instead of the ring's. Test names: 0x5BFF8 gets "I_PDKEY4.FLC" (byte 0 I / F / S = phase, I_PD big,
  KEY / PCK / HAN, the digit = byte 0x22A035 or 5), 0x5EDE0 the "FX\\...BIN" sound. The port front-end stubs host ops 70 door_fx, 107 break_object and 127 make_noise (no doors
  open, nobody hears noises).
- PathApCost's marking part (BL = 1) mirrored (2026-10-22, ai.path_ap_cost; ai_diff path_ap fuzzes BL, CL, option
  0x1163BF, 0x229DF4 / 0x229DD8 / 0x229E08 and tile bytes 0 / 1 / 7 near the walker, compares the flags, count, list
  and every tile's byte 7: 12000 cases 0 mismatches, 8/8 hand mutants caught). Marks only with option 0x1163BF on.
  Tile byte 7: high nibble = the step's direction + 1 (in), low nibble = the next step's direction + 1 (out). The
  start tile gets only the low nibble, and only when the FIRST STEP's tile has byte 0 < 0x9A; then the list restarts
  at [start] with count 1 (else the old count stays and the next tiles append to it). Steps 0 .. n - 2 append their
  tile; a tile with byte 0 >= 0x9A and byte 1 = 0 gets high nibble 0xA instead (low kept). So without CL the
  destination tile has no mark; with CL (n = steps + 1) the destination's out = its own in, and the extra step past
  the count is priced but not marked. At the end, also with no path: 0x229DF4 = 1; with CL too: 0x229DD8 = slot + 1.
- `SpendAP?` 0x329BC (+ `FatigueScale?` 0x32AD8) ported + diff-tested (17/18 mutants; 1 equivalent). It was labelled SoldierSound?: it spends AP (reserve first) and fatigue; the ext key "sound" is now "spend_ap". Composed in every compose mode, so AP spending is now port code there too.
- `StatPractice` 0x62A68 ported + diff-tested (15/15 mutants); composed in every compose mode (it was a leaf called from nearly everything).
- Small leaves ported and composed: `TurnRest?` 0x32D04 (diff-tested, 7/7 mutants; its 64-entry animation switch is data TURN_REST), `UseThrow?` 0x535E8 (direct test), `DropItem` 0x470B0, `MercSees?` 0x56D28, `MercsAlert?` 0x3A2D4. compose World.img converts item-entry arguments (slot / record / runtime pointer) to image addresses.
- First aid ported (2026-10-19): `FirstAidTick?` 0x15260, `ApplyFirstAid?` 0x39CA4, `ItemCharges?` 0x151BC, `UseCharges?` 0x151FC. ai_diff `aid_tick` (original runs 0x39CA4 for real) and `apply_aid` (direct, fuzzed EBX/ECX: the original wraps at 32 bits, so the port wraps too); 25/28 mutants caught, the 3 escapes are equivalent boundary changes. compose --turn now sets up a medic bandaging a live patient (40% of turns), so turn starts run real first aid on both sides. New leaf kind `m` = message text (the port passes the string, the original's pointer is read): merc_text is now `smi` (MsgMercText ignores ECX).
- `FirstAidStart?` 0x3D750 ported (+ `SetTileMarker?` 0x1F634): ai_diff `first_aid` 0 mismatches, 20/21 mutants (1 equivalent: occupant >= 0x30 is already 0x40). Composed in compose_diff (TacticalClick's first-aid branch).
- `MercDies?` 0x3A4A4 ported (2026-10-19): the UI half (death portrait load + 10 blits, sounds, voice, mouse, squad-lost screen 0x3A32C, sector won 0x3A0B0, kill quote 0x1B2A4) stays leaves; ai_diff `merc_dies` 0 mismatches, 26/26 mutants (ai_diff now also compares G_SELECTED and 0x229D2C is fuzzed: the dump already held 0xC41). New compose mode `--dies` (original MercDies unstubbed on real dumps, mercs + enemies, quiet + loud; in ./t check): 0 mismatches. The death animation never reaches its end in the other compose modes, so MercDies needed its own mode.
- `NoiseInterrupt?` 0x73C00 ported (kind-8 noise hands the turn over): ai_diff `noise_int` 0 mismatches, 12/12 mutants; ai_diff now also compares 0x92DD2. Composed (MsgHearsNoise).
- `ShotAlert?` 0x746D4 ported (target reaction after a shot): ai_diff `shot_alert` 0 mismatches, 22/24 mutants (escapes: attack slots 0x30..0x3F are unreachable from ShotEnd and the pointer table holds garbage there; `occ` is only computed without the flag). ai_diff compares G_LAST_SEEN_M over 0x280 bytes now (merc rows reach it from ally slots). Composed under ShotEnd in compose --flight.
- Clean-room port (2026-10-25): Explode 0x3B79C = library op 90 (`ja1_explode`), EffectHit + GasStart = `ja1_effect_hit` (port blast.rs; combat.md Explosion, exact steps); ja1front keeps the effect records (op 178 puff, looks 2/3) and calls `ja1_effect_hit` at count <= 0x13 as DrawProjectilePass 0x17548 does. ai_diff --port explode / effect_hit 3000 cases each, 0 mismatches. Op 160 = 0x1583C (random water-tile splash animation 0x3D, 1000 / splash[sector] Random(0xC40) draws per turn).
- `EffectHit?` 0x173B4 + `GasStart?` 0x38594 ported: the explosion effect record (DrawProjectilePass) hits its tile and starts the Blast. ai_diff `effect_hit` (direct, Blast real on both sides) 0 mismatches, 20/22 mutants (2 equivalent: the cloud index is only compared with -1; sectors are below 0x80). compose --blast now runs the effect step every fourth case. Replay events take the sector sign-extended (sar of 0x22A036), as the original passes it.
- `PutItem` 0x3FC78 ported with its helpers (0x2390C, FindItemTile 0x3FB6C, ItemTileOk 0x3FAF0, ObjectSurface 0x23610, NoteAlreadyPlaced 0x3FC34): ai_diff `put_item` (direct, 7 stack dwords) 0 mismatches, 25/25 mutants after adding a flooded-square scenario (ring 11 / no tile) and a nearly full ground table; ObjectSurface tested over all 256 objects. A crash in the harness made every mutant look caught once: check the run prints its result line before trusting a mutation score. Composed (LandItem, SoldierDies).
- `TriggerItem?` 0x1AC68 (+ `ExplosiveKind?` 0x5AB78) ported: ai_diff `trigger` 0 mismatches, 25/25 mutants (the mutation script now asserts a result line, so a harness crash cannot pass as caught). The pseudocode had the flare branch inverted (count > 0 runs 0x157BC); read from the asm. Composed (LandItem).
- BlastTile port fix: its 0x17B70 call passes the record's slot byte (+0x17A), not the tile occupant index. Found when the new deciders shifted ai_diff's random sequence and a fuzzed +0x17A byte reached the blast test; both are equal in the real game.
- `SectorWorkable` 0x41E1C ported to the world interface (ai.sector_workable, writes b7): ai_diff `workable` (direct) 0 mismatches, 10/11 mutants (1 equivalent). SectorConnectedToHome 0x41178 was then still an oracle / scripted.
- `SectorConnectedToHome` 0x41178 ported exactly (ai.connected_home): a wall-follow walk, not a reachability search. Copy of every b0 gets side marks 4/8/0x10/2 for E/S/W/N; a sector whose copy is >= 0x1E is flagged done on the first visit and pops the junction stack on later visits (the pop returns to the entry just removed); a move lands even on a done sector; a new sector gets the entry-side mark and is pushed when > 2 of its 4 neighbours are owned in the REAL table, read without edge/range checks (bytes before/after g_sectorTbl9). ai_diff `connected` (direct, table and 90 B on each side fuzzed) 0 mismatches; mutation results in the commit. Sector >= 60 reads past the frame (callers never pass it). compose_diff now runs the port instead of the oracle. Spec: docs/spec/strategic.md (new page).
- Ground item pickup cluster ported (2026-10-19): CallEnemies? 0x72568, IsListedItem? 0x3FC04, TakeGroundItem? 0x3C05C (+ 0x19D50, BlastListRemove? 0x3B718 now shared with Explode), TakeGroundPatch? 0x19EB0 (flood over same-type neighbours, unchecked 16-bit tiles; on a listed type it recurses until the stack runs out), FindSquadItem? 0x61528 (the 0x100958 pointer table always points at g_soldiers, so the port indexes g_soldiers), PickUpItem? 0x3C16C (story-item quotes). ai_diff `pickup` (all six called directly, the ported callees run for real on both sides, 0x54EC8 stub empties the cursor in half the cases so the squad-search branch is reached) 0 mismatches; 37/37 mutants. compose_diff runs the ports in place of the leaves.
- Door cluster ported (2026-10-19): SectorDoorLookup? 0x3AFE0 (was a compose oracle), BreakObject? 0x3ACB8 (open / break: object numbers drop, a west or north neighbour changes, move code 0x6338), CloseDoor? 0x3ADB4 (not while occupied, move code 0x5DC), ToggleDoorsById? 0x35774 (all 0xC40 map tiles). ai_diff `doors` (direct, the four run for real with real TileStandable) 0 mismatches; 32/32 mutants after adding occupant 0x2F and a door on tile 0xC40 to the fuzz.
- `ReserveAp?` 0x17C88 ported (ai.reserve_ap; the harness leaf was misnamed "posture", now "reserve_ap"): +0xEF = AP held back for the item in hand. ai_diff `reserve_ap` (decider, focused setup of type / +0x1A5 / +0x16A, scripted ActionApCost now also returns 0x7D) 0 mismatches; 12/13 mutants (1 equivalent: `>=` when apLeft equals the cost is undone by the cap). Types: soldier +0xEF reservedAp, +0x16A b16A.
- `GunJam?` 0x58574 ported (ai.gun_jam; animation op 0x1C1): jam roll by condition, unjam roll by mechanical (+0x190). ai_diff `gun_jam` (decider, focused setup incl. ClearTarget fields and the current animation) 0 mismatches; 16/18 mutants (1 equivalent; 1 not observable: the voice leaf "cry" does not compare EBX, the sample-name pointer 0x91084 jam / 0x91088 cleared).
- `StageEffects?` 0x635DC ported (ai.stage_effects): table 0x95E0E filtered by stage 0x22A1F2 and difficulty, effects via the 0x157BC leaf; stage 0 also arms the stat-loss message timer (0x1D4C). ai_diff `stage` (direct) 0 mismatches, 12/12 mutants.
- `AgeKnowledge?` 0x741F8 ported (ai.age_knowledge; NewTurn calls it): the call level byte (+2 of g_callM / g_callE) is a countdown that clears the call tile; knowledge levels move away from 0 and reset past +4 / -3 (byte wrap at 0x7F / 0x80 kept). ai_diff `age` (direct) 0 mismatches, 14/14 mutants.
- `StoryHit?` 0x61D28 (ai.story_hit) and `StartAttack?` 0x5ABE0 (ai.start_attack) ported. ai_diff `story` (direct) 0 mismatches, 8/9 mutants (1 equivalent); `start_attack` (decider, MEMRUN + OTHERS + MAPCMP, focused setup) 0 mismatches, 19/20 (1 equivalent). Setup trap: the pair flag +0x162 + slot of slot 24 is +0x17A (the slot byte) - pair flags exist for mercs only, so the setup writes them only for slots 0..7.
- `BuryMine` 0x3CB24 ported (ai.bury_mine; animation op 0x1E9): object table 0x95694 (9 signed bytes per object from 0x37, value 0xB) pushes the target one more step; mine of condition >= 10 armed + sector mine count +0xAF; other items set off in place via 0x157BC. ai_diff `bury_mine` (decider, focused setup) 0 mismatches, 21/21 mutants. The put_item leaf no longer compares EDX/EBX/ECX (PutItem reads EAX + stack only; BuryMine leaves garbage there).
- `PickUpHere?` 0x3CDD4 ported (ai.pickup_here; animation op 0x1E8) with its helpers MineSpotted? 0x3C31C, FxHere? 0x3CCEC, BlastListAdd? 0x3B744, FxCancel? 0x15760. 0x22A488 = the 5 x 13 B timed-effect table of 0x157BC (+9 = g_storyTimers). DisarmExplosivePrompt 0x3B9D8 is a leaf ("disarm_prompt", ret 4). ai_diff `pickup_here` (decider, focused setup: ground item, effect table, blast list) 0 mismatches, 31/32 mutants (1 unobservable: the "no item any more" branch needs the prompt to change the tile, which the stub never does). The compared globals now include the effect table, the blast list, 0x284F29 and ground entries up to 0xFF (lookups with 0xFF read past the 200 entries).
- `UseOnTile?` 0x3CFD0 ported (ai.use_on_tile; animation op 0x1E9 when the current animation is 0xC): containers (0x8E; sector 0xB needs item 0x87 once), locked objects per tileset (0x95681 / 0x9568B), break-open with flag 0x76, mines and the disarm prompt. UseCrowbar? 0x3B474 is a leaf ("crowbar"). ai_diff `use_on_tile` (decider, MAPCMP, focused setup) 0 mismatches, 45/45 mutants (2 equivalent mutants dropped: the occupant is never 0x30..0x3F; practice is off at both early stops).
- `ForgetSoldier?` 0x3A148, `ForgetTiles?` 0x74BD4 and `KillStragglers?` 0x74C44 ported (ai.forget_soldier, ai.forget_tiles, ai.kill_stragglers): SoldierDies calls 0x3A148 (target fields 0x40, the other side's sight flags / last-seen tiles / memory, own last-known row via 0x74BD4, last spotter, knowledge 0); animation op 0x1F1 calls 0x74BD4; stage 5 of the home-sector event calls 0x74C44, which starts at slot 0x29, not 0x28. ai_diff `forget` and `stragglers` (direct; MercDies stays a stub there) 0 mismatches, 40/40 mutants; `kill` now runs in MEMRUN (0x3A148 writes the soldier's own record too). The original reads the slot byte signed and indexes the pointer table 0x100958 with it; slots are always 0..0x2F.
- `UseCrowbar?` 0x3B474 ported (ai.use_crowbar, with ai.lock_lookup for SectorLockLookup? 0x3B430): lock = door value x 2 on an unstandable tile, else the lock value; force from strength and hp; the crowbar (item 0x76, condition 10+) adds half and may break ("Crowbar breaks!"); fatigue from the ratio; strength practice on success. ai_diff `crowbar` (direct, door lookup unstubbed, sector record fuzzed) 0 mismatches, 41/41 mutants; a negative lock with zero force divides by zero (kept). use_on_tile now runs the real crowbar in the original. Harness: the voice leaf "cry" now compares its EBX sample name ("BAD" / "GOOD", kind `m`), which closes the GunJam gap.
- `DisarmExplosivePrompt` 0x3B9D8 ported (ai.disarm_explosive; called by UseOnTile, PickUpHere and the 0x3CDD4 mine check): armed test, known (blast list) or spotted (explosives / 30 + level vs strength, blue flag 0x86), the yes/no prompt 0x28C84 (now a leaf "yes_no"), the disarm roll and the per-type result, per-unit conditions of stacks. ClearTileMarker86? 0x19D50 ported too (ai.clear_marker86, 5/5 mutants via `disarm`, which now compares the tile map). New leaves: 0x53708 (ground item pointer), 0x1CB78 (menu refresh, 0x92DD4). ai_diff `disarm` (direct; Random(100) fuzzed at the chance boundary) 0 mismatches, 57/57 mutants (2 equivalent dropped: `armed` is moot below -100; the 100 clamp at exactly 100). The stack loop writes +8.. for count - 1 units, past the 12-byte entry for counts above 4 (kept).
- Movement leaves (2026-10-19): `RebuildMoveCodes` 0x2E730 (ai.rebuild_move_codes, with MarkDoorSide 0x2BFEC; replaces the w_2e730 stub in EffectHit, ClearTileMarker86, TriggerItem) — ai_diff `rebuild` (fuzzed tile map, whole table compared) 0 mismatches, 12/12 mutants. `MercView` 0x1491C (ai.reveal_view, with ViewDirs 0x147E4; replaces the `faced` stub in AnimTurn, TurnOne, StartTurn) — ai_diff `view` (direct; map, ground items, mines ahead, story globals fuzzed) 0 mismatches, 49/51 mutants (2 equivalent for real data: budget 99 vs 98 needs a 98-step ray; side = tile for table value 5 differs only on tiles 1/3/5/7). `MoveMercTo` 0x1C608 (ai.move_merc; caller UseOnTile with the mine detector) — ai_diff `move` (direct, MEMRUN, squad recursion real on both sides) 0 mismatches, 36/38 mutants (equivalent: MsgNotEnoughAP BL 1 vs 0; the path restore after a cancelled confirm, because the stubbed PathApCost never changes the path). New leaves: mine_flag 0x19D74, confirm_click 0x1C514 (UI wait loop), w_1c464 (cursor state). Found: the ray tables hold index 5, one past the 5 directions, which reads the zero step (ends the ray) or, as the side, the ray's current tile number; MoveMercTo's "same pixel point at distance 1" test for items 0xF/0x10 can never hold; the confirm click result is tested on the whole EAX.
- `validate/diff/compose_all.sh` (2026-10-19): every compose mode over every dump in parallel, 11.5 min. It found two crashes the quick gate cannot see: g_curSector 0xFF in office dumps (connected_home now answers 0 off the map) and an endless animation in --turn on 7 dumps (both sides fail; counted apart).
- Harness lessons (2026-10-19): a callee key added twice to a dict literal silently replaced the older stub ("first_aid" 0x3D750 by 0x15260; the turn-start one is now "aid_tick"), so `./t check` now fails on duplicate dict keys in validate/diff/*.py and tools/ai.py. VOID tests no longer turn the original's divide-by-zero into 0. MeleeHit? uses the existing melee_chance port (a global MeleeChance stub had cut knife_option short).
- **Composed test** validate/diff/compose_diff.py (2026-10-19): the original AiThink runs unstubbed in Unicorn on the
  real dumps (real Random = Watcom rand_ seed 0x9BBBC, real pathfinder) against ai.think with every port wired in;
  only effect leaves (walk, turn, click, sound, UI, speech) are stubbed on both sides, callees without a port in
  ai.py run as original code from the port side ("oracle"), soldier records are emulator memory (MemRec), and the
  whole data object must match after each tick. Full run: 11,800 ticks over 128 dumps, 0 mismatches (actions 0, 1,
  2, 3, 5, 8, 11 shoot, 14 stab). It found one port bug the unit test could not: PositionScore moves both soldiers
  onto the scored tiles for its LineOfFire calls. ./t check runs 7 dumps x 25 ticks.
- Not yet traced (stubbed in ai_diff, tentative names in labels.csv): the side effects: AnimStep? 0x1F2C4, MercSees? 0x56D28, FocusOn? 0x3F724, QuestCheck?
  0x633D8, SectorConnectedToHome 0x41178, ChanceToHit / ShotApCost / FindPath (diff-tested elsewhere).

### State Machine

Screens are nested function loops; no global names the current one (snapshot diffs found no candidate byte, and
the loops share static tables, e.g. g_officeHotspots, not a "current" pointer). Observed live on the game stack
(`./t jactl stack`, chain-validated return addresses below the LE stack top 0x33A730), verified on 3 runs of
validate/to_tactical.py:

```
main_ ─ GetKey 0x560F4 ("Press any key")      ─ InitMainDatMouse 0x36F90 ─ PlayTitleIntro? 0x66604 (Escape skips parts)
      └ GameMain 0x4FFEC: main-menu loop (from 0x50057, after MainMenu draw; Continue/New/Exit)
          └ PlayLoop 0x4FB94 (was "EndGameSequence")
              +0x31 OfficeScreen 0x4EE64 (loop from 0x4EE9F) ─ RestoreGameDialog? 0x4E700 ─ SelectSaveSlot? ─ SlotListLoop? 0x4DCCC
              +0x6F SunriseScreen? 0x2A268 (after save OFFICE.SAV, g_day++)
              +0x7E StrategicScreen 0x432F4 (1,1): setup 0x4247C = map + guide SpeechBalloon?/SpeechWait? 0x28034,
                    loop from 0x4334D; ShowMessage? 0x27B98 hints; ConfirmDialog? 0x275E4 "Get to work?" (input
                    at ret 0x2574B, then travel animation at ret 0x2580D)
              +0x8C TacticalLoop 0x107BC: LoadTacticalSector? + fade, loop from 0x107FA (TacticalKeyHandler, draw 0x1CB78)
```
`Game.screen()` reports the innermost of these (boot intro menu office restore slots sunrise strategic tactical
speech message confirm), with "/setup" while a screen is before its loop head. Input is sometimes dropped (the first
keys after start; a click right as the menu appears): scripts repeat an action until the screen reacts
(to_tactical.py `until`). Timer helpers: TimerStart 0x47020 / TimerElapsedMs 0x47040 (ftime_ = DOS time of day: it moves in BIOS ticks of ~55 ms, so a wait of 1-2 ms lasts ~55 ms; the sunset runs at 54.7 ms per frame live, world_load.md "Game speed").

---

### Soldier update per frame (IN PROGRESS 2026-10-21; for the playable sector)

TacticalLoop 0x107BC runs, per frame (flag 0x284F14 set by the timer), for every soldier in the sector and in play
(+0x188 = 0): with timer +0xB0 <= 0, 0x1B9D4 for the soldier whose turn it is (+0x1B0 = 0 and slot = 0x284F11) or
any of slots 0..15 out of combat, else 0x1B694. Then the +0xC4 / +0xBC / +0xA8 countdowns (portrait panel, idle
remarks), ProjectileStep for +0x1BF = 3, and the turn hand-over (0x12C14 ItqPop, 0x12DD4, 0x12404 / NewTurn).
- 0x1B694 (other soldiers): not walking yet (+0x1B8 = 0, hp 15+): at the step point -> StopAnim + +0x1B8 = 1 +
  AnimStep, else WalkStep 0x1A108; +0x9D -> MercView; animations with ANIM_STILL or +0xBA = 2: AnimNext, +0xB0 =
  s16 +0xD2, 0x1B504, 0x1B188 (or tile +0x120 = none when hp < 15), patrol re-walk, OccupyTile.
- VERIFIED leaves (ai_diff walk_step / walk_blocked, mutants 324/332 and 92/95, survivors equivalent): WalkMove
  0x1A048, WalkStep 0x1A108, WalkBlocked 0x19818. Trap: the decompiler shows `dword [+0x122] >> 16` as +0x122; it
  is the word at +0x124 (step target). Also VERIFIED: StandUp 0x1B188 (87/87; the decompiler shows its +0xD0 / +0x126
  / +0x128 writes as other fields), WalkEnd 0x1B8AC (115/117), FrameChecks 0x1B504 (69/70; MercQuoteEvent?
  0x1B324 stubbed in its cases), EnemiesSee 0x7365C (24/25), MercQuoteEvent? 0x1B324 (97/101, survivors equivalent:
  the uncompared "" text pointer, ClearTarget flag 2, a 5-byte timer read). MercQuoteEvent? plays the one queued merc
  quote (slot 0x92DEC, event 0x284F27, tile 0x92DEE, delay 0x284F26, focus flag 0x284F25) when the soldier is that
  slot and a merc: a delay above 0 counts down first; event < 0x21 = text (ShowMercMessage?, event 5 also forgets the
  attack); 0xF9..0xFD = cries BAD / GOOD / EMPTY / NOTHING / HMM, plus sample 0x2B11 and timer +0xC0 = 400 when that
  timer is out and +0x16C = 0. Every played event clears the queue.
- VERIFIED 2026-10-21: SoldierFrame 0x1B694 (ai.soldier_frame; ai_diff soldier_frame with WalkStep, MercView,
  FrameChecks, MercQuoteEvent?, StandUp, MoveMercTo, AtWaypoint, OccupyTile real; mutants 145/145). The animation
  code is the LOW BYTE of the word, read once at the start. Patrol re-walk: +0xBA = 2, code 4+ and 0x93166 set; a
  soldier off its patrol tile +0x12E (which is not the step target +0x124) gets +0x1B8 = 0 and MoveMercTo(+0x12E, 0,
  0; 0, 0, 0, 0); at the waypoint an animation other than 0x11 starts 0x11 and +0x1A2 = +0x1A4.
- VERIFIED 2026-10-21: ActiveFrame 0x1B9D4 (ai.active_frame; ai_diff active_frame; mutants 662/685, survivors
  equivalent or behind the stub 0x1A46C). Order of the walking part (+0x1B8 = 0, hp 15+), at the step point only
  (else WalkStep and on to the view part):
  1. A door to close (+0x106 not none and not the own tile): CloseDoor?(tile, tile byte 1, seen) and +0x106 = none.
  2. Sneak practice: a merc with +0x1A8 and +0x1D5 > 0 practises stat 2 (StatPractice(2, 1, 0)) once per enemy
     0x28+j it sees (+0x13A + j = 1) that does not see it (enemy +0x13A + slot != 1).
  3. A step taken (+0x193, cleared): on a new tile (+0xF4 != tile) SpendAP?(StepApCost(tile, +0x1A8, +0x196,
     +0x177), 0x39C10(tile)); +0xF4 = tile, +0x17D = 0, CheckSectorTileTriggers (it keeps EDX, so the "throw" flag is
     always 1 after a step); the ground item on the tile (byte 6 < 0xFF): mercs and guards always TriggerItem?(tile,
     index, 0), enemies only for type 0x70; an enemy on type 0x11 with byte +2 >= 10 explodes it (Explode(slot, tile,
     2, 1), TakeGroundItem?). Out of energy in water with hp > 0: Drown?, +0x124 = tile, WalkEnd.
  4. Enemies (0x28+): 0x73578, 0x735BC, seen ones focus (and redraw with 0x92DDF); 0x92DCF -> InterruptTurn?,
     else 0x92DD0 -> NextTurn?(0); either ends the walk ("ended"). Others: slots < 0x10 0x7351C, EnemiesSee,
     ShareSightings(0), MercView; then with 0x92DD0 the walk ends (0x284F0F = turn slot when a merc, StopAnim off the
     target, +0x197 = 1, NextTurn?(0)).
  5. After a step: ThrowItem? 0x19A6C. Real-time combat (0x92DCB) and not the turn slot: StopAnim off the target.
     Mine flag +0xEC: MineFlagReroute?, +0xEC = none. The turn slot runs AiThink. A merc in deep water (+0x1B2 >=
     0x9A) runs 0x407B8.
  6. At the step target: WalkEnd(code at the start). The queued action +0x1A6 (not with +0x1B0, and in real-time
     only for the turn slot): 0xFE / 0xFF -> 0, same as item 0 type -> that type, else none; none or water ->
     ClearTask(1). A merc in real-time whose UseCost (target +0x134) is above its AP (+0x1A5, minus the reserve +0xEF
     when +0x16B and not yet warned) and +0x16B: warned (0x22A0EC), 0x95DC8 = 2000, step point = own tile, StopAnim,
     ClearTask(0), +0x16D = 1, 0x1148C(slot, 1, 0). Else TacticalClick(target, action).
  7. Not at the target, energy left or in water, walk not ended: +0x1A2 = NextPathDir (+4, wrapped, when +0x196);
     0x1A46C true -> +0xD4 - 1, +0x1AF + 1, WalkStep. Out of energy (not water) in animation 4+: step point = own
     tile, StopAnim(1), path marks off, SetAnim 0x2B. Then 0x1B5E4.
  Still part (+0x1B8 or hp < 15): AnimNext for ANIM_STILL, AiThink for the turn slot; hp 15+ or water: a soldier
  with item 0 type > 0x10, out of water and +0x1BE = 1 starts the attack on +0xDC (0x5ABE0, 0x5AFBC), AnimStep
  when +0x12A is not none; else 0x1B5E4. Then for every case: a merc with +0x9D: 0x7351C, EnemiesSee,
  ShareSightings, with 0x92DD0 the walk ends (StopAnim always), MercView. FrameChecks; hp 15+: StandUp and, for
  posture 4 with 0x1AAF0 > 5, message 4 once (0x22A0FF); hp < 15: +0x120 = none. Timer +0xB0: 0 for an enemy with
  +0x9C = -1; else s16 +0xD2, halved (toward 0) for +0xBA = 1 in real time, or with option 0x1163C1 in a walking
  animation. OccupyTile. A pending turn slot 0x284F10 becomes 0x284F11 with the turn banner (0x1054C).
  Leaves mirrored with it (real on both sides): 0x39C10 = the step's energy cost by move code (ai.STEP_ENERGY:
  1000 -> 5, 1200 -> 10, 1500 -> 5, 1600 -> 20, 2000 -> 35, 3000 -> 50, 3600 -> 75, 5000 -> 100, 25300 / 25400 -> 5,
  else 0), StepApCost (combat.step_ap_cost), ExhaustedFall 0x1B5E4 (hp < 15 in animation 4..0x27: stop on the own
  tile, SetAnim 0x40 in water else 0x2B), SoakItems 0x407B8 (deep water: each item with item table +8 loses
  Random(100) condition when that roll is below 10; below 0 -> Random(8) + 2); the water fall is unreachable in
  ActiveFrame (hp < 15 never reaches it in water). Also mirrored: CheckSectorTileTriggers 0x19FA0 (ai.tile_triggers),
  FireSectorTrigger 0x19F30 (ai.fire_trigger; ConsumeSectorTrigger 0x19E20 stays a stub) and StepStart 0x1A46C
  (ai.step_start, mutants 375/394, survivors equivalent: the direction table 0x92FB8 repeats N at index 0 and NW
  at 9, unused trigger index with keep, uncompared ECX of BreakObject).
  StepStart details: the direction is +0x1A2 plus 4 (wrapped) while backing up (+0x196) - ActiveFrame already
  turned +0x1A2 the same way, so a backing soldier steps in the turn-target direction rotated twice (= +0x1A2 as
  NextPathDir gave it). Backing up ends on a water tile. The AP check here takes the reserve only for +0x16B = 1
  (ActiveFrame's action check takes any non-zero +0x16B). A detected mine (ground item 0x13 / 0x2C, s8 +0x192 >=
  |byte +2|) stops the step: message 4 and stop via FireSectorTrigger(0xFF, keep), then MineFlagReroute?.
  Step animations: on land (code < 0x32, not in water) 0x32 only for ground class >= 8 and object 0 or >= 0x21; in
  water: class < 8 or object 1..0x20 -> 0x31 (wade), else by ground t0: >= 0x9A -> 0x3A unless code >= 0x39,
  0x97..0x99 -> 0x37 unless code 0x36..0x38, else 0x34 for code > 0x34, 0x32 for code < 0x32.
  Leaves mirrored 2026-10-21 (late; run for real in active_frame, stubbed only with --port, CASE_STUBS):
  EnemyStepSeen 0x735BC (ai.mercs_see; mutants 49/53, survivors equivalent): the enemy drops out of view, every
  living merc / guard 0..15 that sees it shares sightings and asks MercInterrupt?; the spotted enemy 0x229C94 is
  cleared when still unseen. StepNoise 0x19A6C (was "ThrowItem?"; ai.step_noise, 230/236, survivors equivalent or a
  1% edge): the loudness of a step (skills, health, energy, water, posture; sneaking rolls Random(100) against it;
  mercs hear "splash" / "clunk" / "snap"), then MakeNoise. ConsumeSectorTrigger 0x19E20 (ai.consume_trigger): the
  trigger tile becomes 0xC41 and triggers on the 4 neighbour tiles are used up recursively (a neighbour sum of
  0xC41 recurses forever in the original: old tile 0xC09 / 0xC40 / 0xC42 / 0xC79). Harness: active_frame now runs
  the mirror on emulator memory (MEMRUN), because tile_standable reads the soldier's own +0x9C there.
  NearestMerc 0x1AAF0 (ai.nearest_merc, direct test with EDX input, 28/29): mirrored, but active_frame keeps it a
  stub: its entry EDX for slot 0 comes from TacticalLoop's previous loop pass (0x10D1F..), not worth modelling.
  ThrowFollow 0x5AFBC (ai.throw_follow; mutants with throw_aim 198/208), ThrowAim 0x59364 (ai.throw_aim; 165/170)
  and ThrowScatter 0x598A8 (ai.throw_scatter; 198/208) mirrored 2026-10-21 (survivors equivalent or edge values
  that the fuzz rarely hits: odd margins at multiples of 15, reach exactly -1 / odd = distance). active_frame runs
  ThrowFollow for real now. Traps: ThrowAim indexes the kind table with the SIGNED type byte but the range table
  with the unsigned one; Ghidra shows the Distance / ViewTile arguments as other fields (dword >> 16 = the NEXT
  word: read the asm). In ThrowScatter `mov dl, [esp+4]` clobbers DL before `xor dh, dh`, so a facing without
  the x axis moves x by 0 (as Ghidra says). The 5 % of fuzz cases that hang divide by zero in ThrowChance (max
  health 0 from the fuzz, not reachable in the game).
  Spec + clean-room port of EnemyStepSeen, StepNoise, ConsumeSectorTrigger (library ops 111, 117, 106) and the
  throw follow-up (op 122): soldier_update.md; ai_diff --port 0 mismatches. No game data sector has a trigger on
  tile 0xC09 (3081), so the endless ConsumeSectorTrigger chain never happens in the game.
  For click-to-walk (2026-10-21): FindPath 0x40810 as the op the game uses (ai.find_path_op: store flag BX =
  low 16 bits; found + store -> g_paths[slot] first 80, +0xD4 count, +0x1AF = 0; else nothing written) and
  PathApCost 0x1D32C without the path marks (ai.path_ap_cost: FindPath(tile, store 1) ALWAYS, so pricing a walk
  also stores its path; G_WALK_AP 0x229DF0 = the first step's AP + getting up) mirrored: ai_diff find_path 250 and
  path_ap 250, 0 mismatches (CASE_REAL in ai_diff runs FindPath for real in the path_ap cases). Next for the port:
  spec ops 40 / 41 (and 29 walk_to, 1 idle = "Forget the attack (1)" when +0x1FE) as library ops, then the move
  order (MoveMercTo 0x1C608, mirror ai.move_merc) as an entry point, then the frame loop in ja1front --play.
  Spec + clean-room port DONE 2026-10-21: docs/spec/soldier_update.md, port/ja1logic src/update.rs; ai_diff --port
  walk_step / walk_blocked / walk_end / stand_up / frame_checks / merc_quote / enemies_see / soldier_frame /
  active_frame 0 mismatches (port-mode fuzz keeps animation words < 0x100, item types < 136, the queued target on
  the map: the original reads past those tables).
  The frame (2026-10-21): TacticalLoop alternates one update pass (flag 0x284F14 = 0: the soldier loop
  0x10C5B..0x10F4E and the rest, then the flag = 1) and one draw (0x1CB78 -> TacticalRedraw 0x176C8 clears the
  flag). Soldiers are paced by their own timers, not by the frame rate: the AIL timer callback 0x5EE98 (registered
  by 0x5F2B0 with period 10000 us = every 10 ms) takes the tick step 0x92F60 (10; 1000 in a fast mode set by
  0x11D40) off +0xB0 / +0xCC, 10 off +0xA8 (hp 15+ and screen drawn), +0xC0, +0xC4 (floor -1), +0xB4 (floor 0),
  mercs' +0xBC (while +0x189 > 0) and +0x1DC, plus the stop timer 0x95DC8 and many global UI timers (0x92DB4 clears
  the busy count 0x92E38 after 5 s; 0xEAFF0 16 x 0x12 effect animations, 140 ms per count; 0x229E24 10 x 11 B
  tile effects that clear tile byte 3; mirrored 2026-10-22 as ai.front_ticks, ai_diff ticks, 4 hand mutants caught: tile
  animation k 0..9 = s8 count, i32 timer, s16 tile +5, created by 0x15720 from TacticalLoop / draw passes). Mirrored: ai.soldier_loop (ai_diff loop: the original runs as a code range
  through a trampoline that sets EBP; mutants 281/288) and ai.soldier_ticks (ai_diff ticks; 157/171), survivors
  equivalent. DirectionTo? 0x52560 = x87 atan2(1.2 dy, dx) (dx 0 -> 2^-8) + pi/8, x 4/pi, truncated, table 0x96268;
  ai.direction_to matches in double precision (validate/diff/pure_diff.py, 20,000 cases).
  validate/diff/walk_diff.py (in ./t check; 0 mismatches 2026-10-21) runs whole walks in the original (MoveMercTo + ticks + soldier loops, every library
  op real, every host op a stub) and in the port, comparing the world after every frame. Op 62 reveal_view (a
  host op in the port) is answered there by the original MercView 0x1491C on memory.
- VERIFIED 2026-10-21: AnimEnd 0x1F0F0 (script word 0x1F3, the last frame word of EVERY animation script, always
  followed by 0x3E7): pops the animation queue (+0x1E8 words, index +0x1F3) back to the queued animation, or runs
  AnimStep for an empty queue (mirror ai.anim_end, ai_diff anim_end, mutants 111/113, survivors equivalent). The
  "animation 499" seen in walk_test before was a harness artefact: with AnimEnd stubbed, the script ran on into the
  0x3E7 terminator (word - 0x1F4 = 499) and then read bss at 0x110808 (0xBABA fill) as a script. AnimEnd has a dead
  second exit (ReplaceSoldier, tile point, StopAnim, StopWalk) behind a stack flag that is never set.
  0x1AAF0 (nearest merc distance, for message 4) has a register quirk: it tests the CALLER's hp / sector / in-play
  bytes (not the other merc's), and for its own slot (and when those tests fail) it compares the stale EDX again;
  for slot 0 that is the caller's EDX (MercView's range +0x19E when the view part ran). Mirror it with EDX as input.
  Harness note: ai_diff MAPCMP deciders now compare the pointer table 0x100958 and 64 tiles past the map too; for
  active_frame / soldier_frame the 0x200 bytes before the map are restored as well, because MercView for a merc on
  map row 0 writes the north roof (tile - 56, byte 3) before the map, into soldier 0x2F's record (+0x12B..). The
  harness now reads every result region before it restores any (it lost the original's writes there before).

### Sector start: SetupSectorFromXra 0x2E918 (VERIFIED 2026-10-20: mirror tools/ai.py setup_sector, ai_diff sector_start 0 mismatches)

Verified by a direct diff test of the whole routine (XRA.DAT served from the CD, leaves stubbed except SlotReset,
InitXraEnemy, MercEntry, StepTile, DirStep). Corrections to the draft below:
- The ground item block is **50 x 12 B** (600 B), not 200. The routine therefore reads the WHOLE record:
  0x2000 (16 templates) + 3136 (ground item byte per tile) + 600 = 0x2E98. Question 10 is void.
- **InitGuard 0x2B7DC has a 5th (stack) argument**, popped with `ret 4`: the tree tile for guards near a tree,
  0xC41 for the leftover guards round the map centre.
- Story items: PutItem(tile; the 12 B item at 0x222B68 as three dwords, then -1 if the type is 0x5E else 0, 1, 1, 0).
  Type 0x86 becomes bytes +3 = 0x69, +4 = 99, +5 = 100 before the call. FUN_53708(0x222B68, 1, 2) runs first.
- Leftover guards read the direction table by index m; m >= 8 reads past the 8-byte local (stack garbage). The
  mirror raises LookupError there; the test skips such cases.
- 0x22A478 is a word.
- Leaves mirrored and diff-tested 2026-10-20 (ai_diff guard_leaves): InitTapper? 0x2BB90 (ai.init_tapper), InitGuard?
  0x2B7DC (ai.init_guard: level = Random(day + 5) / 10 + 1 capped 9, stats + (level - 1) x 5 capped 100, c =
  [0x22A030] + (level - 1) x 5 drives +0x1B1 and items 0/1/2/5), GuardTile? 0x2C70C (ai.guard_tile). GuardTile with a
  direction 0 or > 8 never returns to its start direction: the original loops forever (callers pass 1..8 only).
  Mirror mutants (validate/mut_par.py): init_tapper 135/135; init_guard survivors are equivalent cap compares at the
  cap value; guard_tile 58/64 (left: the exact edge of the 150-miss limit, class 7 counted as a miss only when that
  limit decides, d wrap to 0, occupant 0x3F which cannot occur).
- Equipment roll 0x2FCE4 mirrored and diff-tested 2026-10-20 (ai.equip; ai_diff equip, direct, plus inside
  sector_start with the "equip" stub lifted): flag = DL byte (0x100 -> 0). c = signed byte [0x95C48 + 2 x difficulty]
  + (day < 5 signed ? day x 5 - 25 : 0) + (level - 4) x 5; A/B/C = u8 0x22A0BE/BF/C1. Order of rolls: +0x1B1; gun
  (u 7..1, table 0x29F26 = 1 2 3 4 8 9 10 11, u 0 = gun 1 when no roll stops); item 1 (thrown table 0x29F2E by
  Random(7), count table 0x29F20 by Random(6); else knife 0xF / 0x10 with +25 for attitude 5); armour 0x26..0x23 and
  helmet 0x30..0x2D (a step rolls only while step - base < +0x1B4); item 6; items 4 and 3; attachment ('B' = 0x42 at
  c - 5 + A, else '[' = 0x5B at c - 10 + A, from the 82 x 4 B table 0x94AAA); then flag 0: clip counts for items
  7..9 of type 0x36..0x3E and item 1 type 0x1C condition 100 -> 50 + Random(50); flag: copies of items 0..5 into 7..9
  (bytes 0..2 and 8..11 only, Random(100) < 8 each, at most 3); last: ammo into item 11 at 45 / 25.
- Item reset FUN_53708 (EAX item, DL condition mode, BL count mode) mirrored as ai.item_init and diff-tested with
  every DL / BL value (equip section, direct cases; the item table +5 / +6 fuzzed). Clears attachment bytes 3..5
  ALWAYS: so in 0x2FCE4 flag 0 the attachment test on item 0 always sees 0. Random(max - 1) uses the signed byte.
  Stack count 0x30A88 (ai.clip_count): class 5 only, Random(100) < 1 / 3 / 10 / 25 -> 5 / 4 / 3 / 2, else 1.
  sector_start now runs FUN_53708 for real on both sides. Mutants (validate/mut_par.py, 12000): 691/710; the 19
  survivors are equivalent (bytes the reset or the memset already fixed, counts the reset caps, edges the real
  tables never reach).
- InitSoldierNames 0x2C034 mirrored (ai.init_soldier) with ArmorBonus? 0x5383C (ai.armor_bonus) and WakesAlert?
  0x2C6A0 (ai.wakes_alert); ai_diff init_soldier runs ShotEnd, ClearTarget, MercMaxAP, AnimDelay, ForgetTiles and the
  tile readers for real. 0x2C6A0 takes DX from its caller for attitude > 4: InitSoldierNames clears EDX at 0x2C264 and
  every callee keeps it, so the base is 0. MercMaxAP divides by max hp x 3: max hp 0 below hp crashes the original
  (the test skips those cases). Mutants 693/708 (survivors: fields ShotEnd / ClearTarget write again, slot 0x10 is
  always a tapper, one rare roll edge). sector_start now runs tapper, guard tile, guard init and InitSoldierNames for
  real. validate/mut_par.py now refuses to run when the unmutated base run is not clean (a crashing base counted every
  mutant as killed: 708/708 once).
- sector_start runs MoveCode, PutItem and NoteAlreadyPlaced for real too (note flags + types fuzzed).
- Supply drops mirrored 2026-10-20: LoadCrateDrops 0x62360 (ai.crate_drops), CrateLost? 0x62780 (ai.crate_lost),
  FixedItems? 0x2ACAC (ai.fixed_items); ai_diff 'supplies' calls all three directly (drop table, states, day, owner
  flag fuzzed), sector_start runs them for real. Drop table 0x95CC0 = 4 x {i8 sector, i8 day, i8 record count, u8
  enemy keeps it}: sectors 0x23 / 0x11 / 0x2A / 3 on days 9 / 17 / 26 / 31. State 0x22A06B + k, shown 0x22A06F + k.
  A crate the enemy takes (0x62780) raises the enemy supply bonuses 0x22A0BE..C1, the same bytes the equipment roll
  reads. The patch routines ApplyTileObjPatch 0x317EC / ApplyItemPatch 0x318C4 stay logged stubs (EAX path, EDX
  count, BL, CL). Mutants 210/223, all 13 survivors equivalent. Harness fix: emu.stub_sprintf now honours %02d (it
  printed "CRATE 3"; the game prints "CRATE03"). Harness trap fixed: Emu.stub in fast mode added a second code hook
  when an address was stubbed again after a `del e.stubs[...]`, so the stub ran twice and returned to a bad address
  (EIP 1, step limit) - only in the full ai_diff run, never with --only. Emu now keeps one hook per address.
- LoadPlantSector 0x62848 mirrored 2026-10-20 (ai.plant_sector; ai_diff 'supplies' calls it directly, sector_start
  runs it for real): records k = 1..3 at g_plants 0x22A12A + 12 k (record 0 is never read). Match = whole dword
  +4 == u8 current sector; the FIRST match decides: state = signed byte +8 (the draft note said +5: wrong, the asm
  reads dword +5 >> 24); only state 2 acts. [0x22A045] (signed) < 2 -> MISC\DEADPL%02d.BIN tiles (EDX = signed +9,
  BL 0, CL 0) + MISC\ITEM%02d.BIN items (EDX = signed +10, BL 1, CL 0), state 0, RecordReplayEvent(signed sector, 0,
  'Q' 0x51); else LIVEPL tiles + ITEM items (owned sector: BL 1; else BL 0 and state 3). %02d = the dword +4.
  0x629AC (next routine) = the same search, state 3 -> 0 + replay 'Q'. Mutants 80/80 (the fuzz writes record 0 and
  the 12 bytes after record 3 too, or a 4-record loop survives).
- SectorTmpFile 0x2BCF0 mirrored 2026-10-20 (ai.tmp_file, sector_start runs it for real; the harness serves a
  sec%02d.tmp built from the dump's map with 1 % of the bytes changed): fopen "sec%02d.tmp" "rb" (u8 sector) into
  g_datFile 0x92F54; per tile 0..3135 one fread each of t1 t2 t3 t4 groundItem (+1 +2 +3 +4 +6); then 0x960 bytes of
  ground items; fclose; returns 1, or 0 without the file. Mutants 32/34 (2 equivalent). Harness fix: the sector_start
  and merc_entry tests skipped EVERY LookupError, so an IndexError in a mutant counted as a skip; the two real
  undefined cases now raise ai.Undefined and only that is skipped.
- GroundBlast? 0x355D0 mirrored 2026-10-20 (ai.ground_blast + 0x629AC ai.plant_done; ai_diff 'ground_blast' direct,
  sector_start runs it for real). Args AL item type, DL flag, EBX power, CL thrower, stack byte (ret 4; the routine
  saves only ESI EDI EBP). Type 0x11 with power < 100 rolls 500 + Random(1000) (a dead value: the 0x11 branch never
  uses it). Tiles 0..3135, ground item byte < 0xFF with the type: 0x11 -> TakeGroundItem(tile, 0), sector +0xB0 - 1,
  go on while it stays nonzero (sector start: up to +0xB0 such items leave the map); other types -> Explode(thrower,
  tile, signed item byte 1 + 1, 1) for the FIRST such tile only (0x6E: byte 1 = 1 first, 0x6E / 0x71: 0x629AC); 0x3F
  counts in i32 0x92E3C; 0x29 / 0x3F / 0x71 then effect 0x157BC(0xC41, type, flag, power, thrower, signed stack
  byte) with flag 1 once 3+ 0x3F items went off (flag < 1), flag 0 for 0x29. Mutants 157/161 (4 equivalent).
- ApplyTileObjPatch 0x317EC / ApplyItemPatch 0x318C4 mirrored 2026-10-20 (ai.tile_patch / ai.item_patch; ai_diff
  'patches' calls both on fuzzed MISC files, 'supplies' and sector_start run them for real on the CD's files through
  the shared CdFiles server). OpenCDFile -1 -> the routine returns -1 (the error jump skips `mov eax, edi`), else the
  number of records applied. Records: u16 tile + u16 object (stack words) / u16 tile (stack word) + 12-byte item into
  0x222B68; 16-bit counter against signed EDX; tile != 0 and signed < 0xC40 (tile -1 writes just before the map).
  Tile patch: CL -> object 0x9E..0xAA (signed word) + 1, t1 = low byte, MoveCode BEFORE BL's t3 = 0x73. Item patch:
  type byte != 0; BL -> TakeGroundItem(tile, 0); else CL and a ground item on the tile -> PutItem(that entry, signed
  +6, 0, 0, 0), then the type of the tile's entry (read again) = 0, then PutItem(0x222B68.., signed +6, 1, 1, 0). The
  second PutItem usually takes the slot the first one used: the copy only stays when the second one does nothing (a
  placed note). A file shorter than one record makes the original read stack garbage (not tested). Mutants: tile
  patch 10 survivors, item patch 94/102, all survivors equivalent (slice growth, flag 2 = flag 1, fclose argument).
  MISC files on the CD: CRATE03/17/35/42 (10 records), DEADPL / LIVEPL 26 30 53, ITEM03/17/26/30/35/42/53.
- Sector start stubs left: rebuild 0x2E730, drop_item, replace, reserve_ap, radio_result, TakeGroundItem 0x3C05C (all
  mirrored elsewhere) and the file calls (OpenCDFile, fopen, fseek, fread, fclose = host I/O).
- Sector start port: batch 1 (slot reset, item reset, equipment roll) in docs/spec/sector_start.md +
  port/ja1logic/src/sector.rs, ai_diff --port slot_reset / equip 0 mismatches on the first run (port-mode fuzz keeps
  the game's ranges: 136 item types, classes 1..5, difficulty 0..2). Batch 2 (placing soldiers) is specified, not
  ported yet (todos in sticky #1).

Written by an analysis agent from analysis/decomp + `./t disas`; next step = a mirror in tools/ and a diff test
(Random counts inside 0x2FCE4 / 0x53708 are data-dependent: trace them in Unicorn, do not trust this list).

#### Sector start: SetupSectorFromXra (0x2E918) - analysis notes

Sources: analysis/decomp/02E918_SetupSectorFromXra.c, 02B6D0, 02B470, 02B380, 02BB90, 02B7DC, 02C70C, 02B250, 023974, 02FB98, 02FCE4, 053708, 02C034, REVERSE.md (XRA section, Ph6), labels.csv. Disasm checked for InitXraEnemy and the callers.
Soldier offsets are struct offsets (struct = 0x200 B). Soldier ptr table: DAT_00100958[slot] (slot 0..0x2F); g_soldiersPtr 0x1009F8 = entry for slot 0x28 (= 0x100958 + 0xA0), NOT slot 0.
Random(n) 0x53538 = (rand()*n/0x7FFF) % n.

##### 0. Signature and callers
SetupSectorFromXra(EAX = entry tile, DL = entry facing).
- 0x2B122 (LoadTacticalSector? 0x2AD20): EAX = dword [0x95CD0 + 4*g_curSector], DL = 1 (facing N), ECX=5 (unused).
- 0x3A3A0 (FUN_0003a32c, sector 0x3B): same table, DL = 1.
- 0x3167B (FUN_000315dc, leave-sector/move): EAX = [0x22A474] (tile), DL = byte [0x22A47A] (facing).
Entry-tile table: 0x95CD0 = i32[60], one entry tile per sector (e.g. sector 0 = 0x5FC, 1 = 0x974). Facing 1..8 = N,NE,E,SE,S,SW,W,NW.
Tile index = y*56 + x (3136 tiles, 0xC40; 0xC41 = "no tile"). g_dirStep: 1..8 = -56 -55 +1 +57 +56 +55 -1 -57.

##### 1. Ordered steps
###### (a) Global resets (0x2E918..)
One line per group. Addresses are DAT_ names as in the decomp (obj2 linear).
1. Local: local_68/64 = 0x08040602,0x07030501 = guard direction table bytes {2,6,4,8,1,5,3,7}; local_18 = 0 ("purifier given" flag); local_54 = EAX (entry tile), uStack_1f hi = DL (facing).
2. Zero: 92DAC, 92DB0, 92DCE, 229DD4, 229DDC, 229DC8(dword), 284F24 (3 bytes), 93D54, 92DBC, 92E3C, 92DC0, 92DCD, 92DC4, 22A482+2(w), 22A486, 92DDC, 92DDD, 92DD0(+0,+1), 92DCF, 22A0D1, 92E38 (at end).
3. 0x40 ("none" slot): 284F10, 92DC2, 95E0C, 92E52, 229C94, 92DEC, 229CB4, 22A1F1, g_missionOrder.b0, 284F28, 92DD0+2, g_lastSpotter, memset(22A412, 0x40, 0x32).
4. 0xC41 ("no tile"): 229C8C, g_callM, g_callE, 22A44C+2 (5 x 6-byte stride, word), 229D50 (STOSB fill 5 dwords 0x0C410C41).
5. Other consts: 229D1C = 20000; 284F19 = 0xF8; 92DE8 = 229E04 = -1; 22A0CD = 1; 10E380 = 22A444.b0 = [90684]; 22A488 + 0xD*k (5 dwords) = -1.
6. memset 0: g_rememberE 0x140, g_rememberM 0x80, g_known 0x30, 22A0EC 8, FA928 0x30, 10E290 0xF0.
7. g_alarm = g_sectorTbl9[cur].flags & 1 (sector owned).
8. Random(3) #1: 284F01+1 = Random(3) + 1 (value 1..3, global, meaning unknown).
9. End of function re-sets (0x2F5xx): 92DC7 = [284F11]; 22A478 = 0; 92EF8 = 0xFA; 229E00..03 = 0x40,0x7F,0x40,0xFF; item/aux table clears (see (e)).

###### (a2) Soldier slot init (0x2EA..): all 48 slots
For k = 0..0x2F: SlotReset 0x2B250(DAT_00100958[k], EDX = k) (mirror ai.slot_reset, diff-tested, 122/122 mutants). Clears per-soldier state; destination = tile = 0xC41 (off map); slot (+0x17A) = k; +0x180 = (k > 3); +0x1BD = 0; +0x1C8 = 0x78; +0x188 = 1; anim[0] = 4; for k >= 8: +0x182 = 0 (not in sector), +0x17E = 1, +0x1B1 = 0. No Random. Not placement-specific, but it sets every soldier off-map first.
Then: SectorTmpFile 0x2BCF0 -> local_20 = 1 if sec%02d.tmp was opened and read (tile bytes t1,t2,t3,t4,groundItem x 3136 + g_groundItems 0x960 B); else 0. Then RebuildMoveCodes 0x2E730 (no args; the passed EAX/0 are ignored).

###### (b) Merc placement (only if g_teamOnDuty != 0, near 0x2F3xx, after XRA load and map-object pass)
Call: FUN_0002b470(EAX = entry tile (local_54), EDX = facing). Not conditional on the tmp file. g_teamOnDuty is read at 0x229FAC; the lead merc index is byte [0x284F11] (selected merc).
Algorithm of 0x2B470 (all inputs are arguments plus soldier state):
1. placed[8] = 0; ring2 = 0; anchor = entry tile; last = -1.
2. If TileStandable(entry) > 0: place lead merc ([284F11]) on the entry tile via 0x2B380; placed[lead] = 1; leadTile = entry tile. If not standable: lead is NOT placed yet and leadTile stays unset (local_1c is uninitialised-ish, see questions).
3. ReserveAp 0x17C88(lead) (sets +0xEF; not needed).
4. For soldier s = slot 0..7 in order. Skip (and set inPlay=0) if !inCurSector, !inPlay or hp == 0. Otherwise, while !placed[s.slot]:
   a. For dir d = 1..8 (fixed order N,NE,E,SE,S,SW,W,NW): t = StepTile(anchor, DirStep(d)); if ring2: t = StepTile(t, DirStep(d)) again (distance 2). If TileStandable(t) > 0 and TileOccupant(t) >= 0x40 (free): 0x2B380(s, t, facing); placed[s]=1; if s is the lead: leadTile = t; stop.
   b. If all 8 dirs fail: anchor = tile of the next already-placed merc after index `last` (scan last+1..7; takes the first placed one; last = that index). If the scan runs off the end (>7): if ring2 already set -> give up on this merc; else ring2 = 1 and retry (anchor/last unchanged).
   So the mercs fill a spiral/BFS around the entry tile: ring 1 around anchor, then around each placed merc in index order, then distance-2 positions.
5. Final swap: find highest slot u in 7..0 that is placed and != lead. Put lead on u's tile and u on leadTile (two 0x2B380 calls: 0x2B380(lead, tile_u), 0x2B380(u, leadTile)). If no such merc, return.
   Net: lead ends on the tile of the highest-index placed merc; that merc takes the entry tile.
0x2B380(s, tile, facing): destination = tile = tile; +0x1E0 = slot; g_tileMap[tile].occupant = slot; tileMap[tile].t4 |= 0x80; postFacing = 0; +0x9C = 1; desdir = facing = facing; ReplaceSoldier 0x2FB98(s); +0x1BD = 0x6A; inPlay = 1; orders/attitude: profile > 0x3B -> orders = attitude = 2; else if medical <= marksmanship: orders = 4 and attitude by +0x19C: {2,6}: Random(5); 3: 5; {4,8}: 0; else Random(6); else orders = 3, attitude = 0. (Random in 0x2B380 only in the "orders = 4" branch.)
Merc placement is deterministic except these Random(5)/Random(6) calls. Mercs 0..7 are never read from the XRA file.
ReplaceSoldier 0x2FB98: +0xF4 = tile; groundUnder/objectUnder from tile; anim = 4 (or 0x33/0x36/0x39 on water); ptr90 = anim table; AnimDelay 0x15BB8; inPlay = 1; +0x1B3/+0x1B5/+0x1B6/+0x1B7 = ground/object classes. No Random.

###### (c) XRA load (only if (g_sectorTbl9[cur].flags & 1) == 0, i.e. sector NOT owned)
Order: sprintf "XRA\%2d.xra" (unused name), OpenCDFile "DAT\XRA.DAT", fseek(cur * 0x2E98, SEEK_SET). Record = 0x2E98 = 11928 B, 59 records.
Per-slot flag f(k) = g_sectors[cur] + 0x96 + (k - 0x28), k = 0x28..0x2F (record byte; 1 present, 2 = elite/pass-1 type, 0xFF none, 0xFE killed; >0 as signed byte means present).
- Pass 1: for k = 0x28..0x2F: fread 0x200 into slot k if f(k) == 2, else into scratch (PTR 0x93E64, discarded). Then DropItem 0x470B0 (memset 12 B) on items[7] (+0x54) and items[8] (+0x60) of slot k (always, even when the read went to scratch); inCurSector = 0; slot (+0x17A) = k.
- Pass 2: for k = 0x28..0x2F: fread 0x200 into slot k if f(k) == 1, else scratch; inCurSector = 0; slot = k. (items[7..8] are NOT cleared in pass 2.)
- So the file holds templates 0..7 (pass 1, "type 2") then 8..15 (pass 2, "type 1"); the slot k takes template (k-0x28) or (k-0x28+8) by its flag.
- Only if no sec tmp (local_20 == 0): 
  - 3136 single-byte freads to g_tileMap[t].groundItem (+6), tiles in DESCENDING order 0xC3F..0 (stride -8).
  - 50 x fread 12 B into g_groundItems[i] (0xEB160) (draft said 200: wrong). Per entry: if byte[7]==0 (unitCond[-5]... i.e. +7): bytes +9,+10 := 0 (the decomp indexes: unitCond[i-3], unitCond[i-4]; see questions). For multi-unit entries (type != 0, item table field_0x6 > 1, count byte (+0x?) > 1): for each of count-1 extra units a condition byte is derived with Random(10) (cond = base - 5 + Random(10), clamp 255/100 or copy base; two branches by sign of base). The number of Random(10) calls is data dependent.
  - Then for each tile 0..0xC3F with a ground item: if NoteAlreadyPlaced(item.type) == 1 (note already found): TakeGroundItem 0x3C05C(tile, 0) removes it.
- fclose.
- Total bytes read: 0x2000 + 3136 + 600 = 0x2E98 = the whole record.

###### (d) Per-enemy init, sector purifier, equipment (after XRA load, still inside the same `if`)
1. Purifier: if g_purifierPending (22A0CB) != 0 and cur != 0x3B: g_sectors[cur].specialItem (+0x9E) = 0x79; pending = 0.
2. For i = 0..7 (soldier slot 0x28+i, ptr starts at g_soldiersPtr): if (signed) g_sectors[cur] +0x96+i > 0:
   - InitXraEnemy(s) (below).
   - FUN_0002fce4(s, EDX = (flag == 2)) = equipment roll (see section 2 for Random).
   - If specialItem (+0x9E) != 0 and not yet given (local_18 == 0): items[9] = {type = specialItem, qty = item table field (kind>>0x18 < 99 ? field_0x5 : 99), condition = 100, +3..+6 = 0}; local_18 = 1 (only the first present enemy gets it).
   Enemies with flag <= 0 are never touched: they keep tile 0xC41, inCurSector = 0 (not in sector).

####### InitXraEnemy (0x2B6D0), complete, in order
1. Random(4) #A -> +0x1E0.
2. +0x182 (inCurSector? "present") = 1 (decomp: s->inCurSector = 1) and +0x138 = 1 (inPlay = 1).
3. Random(3) #B = r: pick the 32-bit read `dword [s+0x112+2*r]` then sar 16 = word at +0x114 (r=0), +0x116 (r=1), +0x118 (r=2). That is the chosen start tile T (word, sign-extended to int).
4. If byte +0x3C (items[5].type) == 0x31 then = 0x30.
5. For items[7], [8], [9] (+0x54, +0x60, +0x6C): if type == 0: +2 (condition) = 0, +1 (qty) = 0; else if byte +3 == 0: +5 = 0, +4 = 0.
6. word +0x124 = word +0x114 = word +0x12E = T (destination, tile, whereIWas).
7. +0x185 (wounds) = 0; +0x1BD = 0x6D; word +0x10C = 0; +0x18A (energy) = 100; +0x184 (hpMax) = +0x183 (hp); +0x17B += 4 (byte add).
8. Random(8) #C: +0x17C = Random(8) + 2.
9. OccupyTile 0x23974(T, slot = +0x17A): if T > 0xC40 returns 0; if occupant is a different soldier with slot < 0x30 returns 0; else sets occupant slot. RETURN VALUE IGNORED: the enemy is still placed even if it lost the tile.
10. ReplaceSoldier 0x2FB98(s).
Notes: the file template supplies everything else (stats +0x18B..+0x192, items 0..11, facing, etc.). The chosen tile is one of 3 per template; no collision resolve, no map-walkable check, no facing set here (facing comes from the file).

###### (d2) Tappers, guards, names (after merc placement)
Runs even when the sector is owned (outside the XRA `if`).
Let tp = g_sectorTbl9[cur].tappers, gd = .guards; G = (tp + 2) / 3 (number of tree groups).
Tree tile test FUN_236d0(t) = (g_tileMap[t].t1 == 0x36) (after the map-object pass; 0x36 is presumably a tappable tree object; see questions).
1. Tapper groups, slots start at 0x10: scan tile t = 0xC3F down to 0. For each tree tile while groupsDone < G:
   - Random(4) = r.
   - n = min(tp - 3*groupsDone, 3). dirI = 1.
   - For j = 0..n-1: if j == r: dirI += 2. facing = (dirI + 3) % 8 + 1; tile = StepTile(t, DirStep(dirI)); FUN_0002bb90(soldier slot (0x10 + placedCount), tile, facing); dirI += 2.
   - So tappers stand on the cardinal neighbours (dirs 1,3,5,7 = N,E,S,W) of the tree, one cardinal skipped depending on r when n==3 and r<3; facing = toward the tree.
   - groupsDone++; (placed count advances by n).
   FUN_0002bb90 (tapper init, no standable check): inCurSector = inPlay = 1; +0x9C = 1; +0x17E = 0; +0x1BD = 0x6C; +0x17B = 0x19; tile = destination = whereIWas = tile; facing = desdir = +0x1A4 = facing; Random(8)+2 -> +0x17C; Random(4) -> +0x1E0; orders = attitude = 0; +0x19C = 7; level 1; agility = 30 + R(26) + R(26); dexterity = 40 + R(26) + R(26); hp = 40 + R(21) + R(21); wisdom = 20 + R(26) + R(26); wounds 0; energy 100; anim = 0x11; hpMax = hp; OccupyTile(tile, slot).
2. Guards (slots 8..15): per = 0 if gd == 0 or G == 0; 1 if gd < G; else gd / G; per = min(per, 3). If per != 0: scan t = 0xC3F down to 0; for each tree tile (any tree, no cap per tree except total): for m = 0..per-1: R1 = Random(3); tile = 0x2C70C(t, dir = {2,6,4,8,1,5,3,7}[m], dist = R1 + 2); R2 = Random(8); InitGuard_(slot, tile, facing = R2 + 1, orders = 1); placed++. Stop scan after placed >= G*per (the check is after each tree).
   0x2C70C(tile, dir, dist): walk `dist` steps in `dir`; accept if TileStandable > 0, free (occupant >= 0x40), t0 class != 3 and < 8; else rotate dir (1..8 wrap), increase dist after a full turn; gives up (returns the start tile) when dist > 0x1B or after 0x96 water tries.
3. Leftover guards: left = gd - placed. For m = 0..left-1 (direction table index restarts at 0; slot continues): R1 = Random(11); tile = 0x2C70C(0x63C (map centre tile 1596), dir = table[m], dist = R1 + 5); R2 = Random(8); InitGuard_(slot, tile, R2 + 1, orders = 3).
   InitGuard_ (0x2B7DC) is NOT needed for the XRA/merc path; but it has many Random calls (see section 2).
4. InitSoldierNames 0x2C034 for all 48 slots (loop to 0xC0 bytes): sets aiType (0 merc, 1 guard "GUARD n", 2 tapper "TAPPER n", 4 "SANTINO" for sector 0 slot 0x28, 3 "ENEMY n"), state resets; sets +0xA8 = Random(300) for EVERY slot; for slot >= 0x28: +0xB9 via FUN_2C6A0 (returns 1 if cur == 0, else 1 if Random(100) < table[+0x1D4]{0:5,1:10,2:50,3,4:25} + (level-1)*5) and DAT_10D650[slot] = (orders in {3,4}).
5. After names: 22A478 = 0; if g_teamOnDuty: soldier[ [284F11] ].+0x1BD = 0x6E.

###### (e) Everything else (one line each)
- 0x2EA8x: SectorTmpFile + RebuildMoveCodes happen BEFORE the XRA block (see a2).
- LoadCrateDrops 0x62360: applies MISC\CRATE%02d.BIN + ITEM%02d.BIN if a crate drops here today (no Random).
- LoadPlantSector 0x62848: g_plants processing plant files (DEADPL/LIVEPL) (no Random).
- g_missionStep = RadioResult_() 0x63254.
- Story objects loop (5 slots, g_sectors +0x82+2i tile, +0x8F type, +0x9F, +0xAC/AD...): for i = 0..4, if tile word +0x? != 0xC41 and obj byte != 0: place object into tileMap.t1 (0xAC/0xAD removed if story flag 22A048.b3 == 2) + MoveCode(tile); second table: tile (word +0x?+0xA) != 0xC41: build an item {obj byte, ... ; 0x86 -> type 0x69 qty 0x64, 99} and PutItem 0x3FC78, then consume the slot. 
- Sector aux loop: while field_0xAE != 0 && field_0xAD != 0: both decremented (clamps).
- If field_0xB0 (mines) != 0: FUN_000355d0(0x11,0,0,0x40,0) -> removes 0x11 items from the tile map ground (Random(1000) +500 when ebx<100: ebx = 0 so Random(1000) is called); decrements field_0xB0.
- Tile object pass over all 3136 tiles: t1 0x35/0x36 objects converted via sector counters +0xAC/AD/AE (0x34/0x35/0x36 states; likely tree states); t4 of tile+0x38 &= 0xB7 when turning a tile into 0x34.
- Clears of 0xEB002 + 0x12*k (k=1..15) byte and 0x229E2F + 0xB*k (k=1..9) byte (per-slot/per-item flags, unknown).
- If cur == 0x3B and g_sectors[0x3B].hasTmp == 0: FUN_0002acac (sector-59 special setup; contents not read).
- If g_sectorTbl9[cur].mercs != 0: g_sectors[cur].hasTmp (+0xA9) = 1.

##### 2. Random calls, in procedure order
Numbering is by call sequence in a normal first-visit run of an unowned sector. "Cond" = conditional.
R1. Random(3) in main body (start): 284F01 = r+1.
[If owned-check passes (unowned) and no tmp file:]
R2. Random(10) per extra unit of multi-unit ground items (while reading the 200 g_groundItems; 0..many; data dependent).
[Enemy loop, for each slot with flag > 0, in slot order 0x28..0x2F; per enemy:]
R3. Random(4) (InitXraEnemy: +0x1E0).
R4. Random(3) (InitXraEnemy: which of 3 tiles).
R5. Random(8) (InitXraEnemy: +0x17C = r + 2).
R6.. FUN_0002fce4 equipment (arg2 = flag == 2). In order:
   - Early out with no Random if byte +0xBA == 4 (items[2], items[5] cleared).
   - arg2 == 0: for items[0..11] in order: 0x53708(item, DL = cond==0, BL = cond==0). Per non-empty item with cond==0: Random(0x1f) (condition = 0x46 + r); if item class (field_0x6)==1 and field_0x5 != 99: Random(field_0x5 - 1) (qty = r+1); for class > 1 with qty > 1: Random(0x1f) per extra unit (qty-1 times).
   - arg2 != 0: memset(s, 0, 0x90) (clears items 0..11, no Random).
   - Random(100): +0x1B1 = r < a0be + cVar2 + 15 (cVar2 from day and level; a0be, a0bf, a0c1 are difficulty globals).
   - If arg2 != 0 or [22A0BE] != 0: weapon roll: Random(100) repeated up to 7 times (u = 7..1, break when r < (7 - u)*5 + cVar2 + [22A0BF]); if weapon upgrades: 0x53708(s, DL = 1) -> Random(0x1f) (+ BL unspecified).
   - If items[1] empty: Random(100) (< a0c1 + cVar2 + 15?): yes -> Random(7), Random(6), then 0x53708(items+1,1,0) -> Random(0x1f). No -> Random(100) (< a0be + 25*(attitude==5) + 25?) ; if yes: Random(100) (type 0x10 vs 0x0F), then 0x53708 -> Random(0x1f).
   - Armor (0x26 -> 0x23): per step Random(100) only if objectUnder(+0x1B?) > b-0x23 (short circuit); then 0x53708(items+2,1,0) -> Random(0x1f).
   - Helmet (0x30 -> 0x2D): same pattern; then 0x53708(items+5,1) -> Random(0x1f)?
   - items[6]: if objectUnder == 4: Random(100) (< a0be + cVar2 -> 0x31) else/otherwise Random(4) (type = 0x32 + r); 0x53708(items+6).
   - items[4] empty: Random(100); if fails: Random(100) (second tier). Same for items[3] (Random(100), Random(100)). Each success calls 0x53708 -> Random(0x1f) (condition) when DL=1.
   - If items[0].+3 == 0: for weapon with table kind 'B' or '[': Random(100) < threshold -> Random(0x1f) (sets +5 = 0x46 + r).
   - arg2 == 0: for items[7..9] with type 0x36..0x3E: 0x30A88 (Random(100) if item class 5) + 0x53708; then if items[1].type == 0x1C and condition == 100: Random(0x32).
   - arg2 != 0: for items[0..5] non-empty: Random(100) < 8 -> copy to items[7..9] (max 3) + 0x53708(DL=0).
   - Final: if items[0] is a gun (1..14): Random(100) < (arg2 ? 0x2D : 0x19) -> items[11] = ammo; 0x30A88 may Random(100); 0x53708.
[After the enemy loop:]
R7. Merc placement Random(5)/Random(6) inside 0x2B380 (once per 0x2B380 call that reaches the attitude switch: also the two swap calls).
R8. Tapper loop: per tree used: Random(4); then per tapper (0x2BB90) in order: Random(8), Random(4), then pairs Random(26) x2 (agility), Random(26) x2 (dexterity), Random(21) x2 (hp), Random(26) x2 (wisdom).
R9. Guard loop (trees): per guard: Random(3), Random(8), then InitGuard_ Randoms: Random(8), Random(4), Random(6), Random(day+5), Random(26)x2 (agility), x2 (dexterity), x2 (hp), x2 (wisdom), x2 (marksmanship), Random(16)x2 (medical), Random(100) (+0x1B1), Random(2) (weapon), 0x53708(s,1,2) (DL=1,BL=2: Random(0x1f) cond, qty fixed), Random(100) [then Random(100) + 0x53708 if the first passes], then if bVar7 != 0: Random(bVar7) (armor) and Random(bVar7) (helmet) - each followed by 0x53708 only if r/10 != 0.
R10. Leftover guards: per guard Random(11), Random(8), then InitGuard_ Randoms as R9.
R11. InitSoldierNames: for each of the 48 slots in order 0..47: Random(300); and for slot >= 0x28 (not sector 0): Random(100) (via 0x2C6A0).
R12. FUN_000355d0 when mines field_0xB0 != 0: Random(1000).
R13. FUN_0002acac (sector 0x3B first visit only): not read.
Note: R7..R12 order inside the function follows source order shown in section 1: merc placement (R7) comes BEFORE tapper placement (R8), BEFORE guards (R9, R10), BEFORE names (R11). R12 (mines) and the story-object/PutItem pass (PutItem 0x3FC78: no Random found) come BEFORE merc placement in code order (see ordering question).
Source order of the whole function: R1, [tmp load], [XRA read: R2], [enemy loop R3..R6], crate/plant, story objects, R12, map-object pass, merc placement (R7), tappers (R8), guards (R9, R10), names (R11).

##### 3. Callees
| Addr | Label | Job | Needed for placement |
|---|---|---|---|
| 0x2B250 | (none) FUN_0002b250 | per-soldier reset to off-map (0xC41), slot = k | yes (resets all 48 slots) |
| 0x2BCF0 | SectorTmpFile | read sec%02d.tmp (tile bytes + ground items) | no (state load) but chooses the XRA tile/item path |
| 0x2E730 | RebuildMoveCodes | rebuild g_moveCode | no (needed for pathing/TileStandable class) |
| 0x470B0 | DropItem | memset 12 B item | yes (pass 1 item wipe) |
| 0x23974 | OccupyTile | claim g_tileMap occupant | yes |
| 0x2FB98 | ReplaceSoldier | refresh ground/object/anim classes at +0xF4 tile | yes |
| 0x2B6D0 | InitXraEnemy | per-enemy init | yes |
| 0x2FCE4 | (none) | equipment roll for XRA enemies | no |
| 0x53708 | (none) | item condition/qty fill | no |
| 0x30A88 | (none) | ammo qty roll | no |
| 0x3FC34 | NoteAlreadyPlaced | note once-only flag | no |
| 0x3C05C | TakeGroundItem? | remove ground item | no |
| 0x62360 | LoadCrateDrops | crate file load | no |
| 0x62848 | LoadPlantSector | plant file load | no |
| 0x63254 | RadioResult? | mission step | no |
| 0x2BE54 | MoveCode | rebuild one tile's move code | no (needed after tile object edits) |
| 0x3FC78 | PutItem | place an item on the ground | no |
| 0x355D0 | (none) | remove 0x11 items (mines) | no |
| 0x2B470 | (none) | MERC PLACEMENT around entry tile | yes |
| 0x2B380 | (none) | place one merc on a tile, orders/attitude | yes |
| 0x17C88 | ReserveAp? | +0xEF | no |
| 0x23538 | TileStandable | tile standable | yes |
| 0x23944 | TileOccupant | occupant slot of a tile | yes |
| 0x47010 | DirStep | direction -> tile delta | yes |
| 0x46FF4 | StepTile | tile + step (off-map safe) | yes |
| 0x236D0 | (none) | t1 == 0x36 (tree) test | yes for tappers/guards |
| 0x2BB90 | (none) | tapper init | yes (for tappers) |
| 0x2C70C | (none) | search tile near a tile in a dir | yes (for guards) |
| 0x2B7DC | InitGuard? | guard init | yes (for guards) |
| 0x2C034 | InitSoldierNames | names/aiType/state | no (aiType needed by AI) |
| 0x2C6A0 | (none) | +0xB9 flag roll | no |
| 0x2ACAC | (none) | sector 0x3B first-visit extra setup | not read |
| 0x23530 / 0x23760 | TileGround / TileObject | t0 / t1 byte | yes (via ReplaceSoldier) |
| 0x15BB8 | AnimDelay | anim ticks | no |
| 0x53538 | Random | RNG | yes |

##### 4. Open questions
1. Lead merc when the entry tile is not standable: in 0x2B470 `local_1c` (lead tile) is only set if the entry tile is standable or the lead is placed in the loop; the final swap uses it unconditionally. Uninitialised-looking; check disassembly of 0x2B470 before relying on it.
2. 0x2B470 ring2 retry: when the anchor scan finds no placed merc beyond `last`, the decomp leaves `last` and the anchor unchanged. Whether the second pass restarts the scan at last+1 or at 0 should be confirmed with `./t disas 0x2B470`.
3. Lead merc index [0x284F11]: only seen as "selected merc"; no label. In 0x315DC it is conditionally used (< 8).
4. Offsets of the ground item reading: the decomp indexes `unitCond[i-8]` etc. as 12-byte entry fields; the exact meaning of the "+7 == 0 clears +9/+10" and the count byte (>>0x18 of `0xEB15E+i`) is ambiguous; confirm with `./t ioseq 0x2E918` and disas around 0x2EAxx.
5. FUN_0002fce4: the DL/BL inputs to 0x53708 for some calls (e.g. `FUN_00053708(s, 1)` with BL not set) are left over registers in the pseudocode; the number of Random(0x1f) / Random(qty-1) calls there is not derived. If bit-exact RNG sync is needed, trace in Unicorn (`./t emu`) instead.
6. `s->objectUnder` in 0x2FCE4 and 0x1BD values: the decomp names the field by the first user; in 0x2FCE4 it is compared like a level/rank (probably +0x192 level, misnamed). Treat as soldier "level" until confirmed.
7. What object t1 == 0x36 is: used as a tree for tappers/guards; the map-object pass rewrites 0x34/0x35/0x36 via sector counters +0xAC/AD/AE (fallow/workable/unhealthy trees). Confirm from tile art if the port generates the sector.
8. Order of the equipment Random calls versus the sector-flag loop: the story-object / PutItem pass has no Random (verified for 0x3FC78 decomp), but PutItem calls 0x3FC04, FUN_00019d50 etc. not read here.
9. Tile occupancy vs `t4 |= 0x80` in 0x2B380: sets bit 0x80 on t4 (not occupant); meaning unknown.
10. (void: the function reads the whole record; the draft counted 200 ground items instead of 50.)
11. g_sectors +0x96..+0x9D semantics: signed byte > 0 = present, template type chosen by value 1/2 (REVERSE says 0xFE = killed, 0xFF none).
12. Facing argument of 0x315DC ([0x22A47A]) is a byte whose range was not checked.

## Intermediate Output Files

| File | Contents |
|------|----------|
| build/JA.elf | LE→ELF conversion for Ghidra/objdump (`./t le2elf`, gitignored) |
| build/ghidra/JA.gpr | headless Ghidra project with __watcall, structs, typed globals, labels (`./t ghidra`) |
| analysis/decomp/*.c | Ghidra pseudocode per procedure (`./t ghidra`; generated, gitignored like all of analysis/) |
| analysis/decomp/procedures.json | Ghidra's procedure list with sizes (xref attribution, `./t status`) |
| analysis/strrefs.txt | per-procedure strings via LE fixups (`./t strrefs [addr…] [--write]`): first stop for naming |
| analysis/strings.txt | strings + absolute ref counts (`./t strings_dump --write`) |
| analysis/inventory.txt | every data file (rom/ + CD) with size + magic (`./t inventory`) |
| defs/types.toml | struct layouts + typed globals with evidence levels (single source of truth; `./t jatypes`) |
| defs/cdecl.txt | stack-argument functions (AIL, clib varargs); all others are __watcall |
| docs/spec/*.md | verified behaviour in plain language: what the port is written from (clean-room split) |
| validate/shots/ | reference screenshots of the original (gitignored: game art) |
| build/mem_*.bin | DOSBox-X 16 MB guest memory dumps (`./t jactl dump NAME`, formerly `validate/xdump.sh`; gitignored) |
| build/states/*.sav, validate/states.tsv | DOSBox-X save states + their catalogue (`./t jactl save/load/states`) |
| gfx/ | decoded images (`./t pcx`, `./t csprite` sheets, `./t sectormap` maps; gitignored: copyrighted art) |
| validate/diff/spec.py | analysis-side reference reimplementations (original integer semantics + quirks) used by the diff tests |
| validate/diff/*_diff.py | differential tests: spec vs original in Unicorn (`tools/emu.py`, `Emu.stub` replaces callees) |

---

## Verification Checklist

- [x] Harness: LE loader verified (entry disassembles to Watcom `jmp` + banner; ELF loads in Ghidra, 1375 procs)
- [x] Harness: original boots in DOSBox to intro + main menu, input via xdotool works (validate/shots/01–04)
- [x] Harness: Unicorn runs an original function (validate/emu_smoke.py)
- [x] Ph3: 5 functions traced and diff-tested vs the original in Unicorn (validate/diff/sector_diff.py: TileToX/Y, door/lock lookups, CheckSectorTileTriggers)
- [x] Ph4: 5 images decoded and pixel-compared with DOSBox shots (validate/gfx_compare.py): ROOM.PCX 99.0% (rest =
      overlays), CASE2.PCC 100% at (127,137), CASE1.PCC 98.0% at (152,91) (rest = cursor), MAINMENU.PCX (all but the
      text area), SWICHBOX.PCC 89.5% (switches drawn on top). Tactical map (TILES?.DAT compiled sprites, TERRAIN.DAT,
      shadows, roofs): byte-identical to the original's static draw passes (tiles_diff, all 60 sectors).
- [x] Ph5: soldier struct confirmed in a live DOSBox-X memory dump (validate/xdump.sh): after Restore Game of
      QICKSTRT, g_soldiers slots 0-7 are byte-identical to the save (all 0x200 B x 8) and the View Team screen shows
      the same SAL/HEA/LVL; g_profiles identical except the 2 bytes LoadGame patches on purpose (see below)
- [x] Ph6: full day loops played on the original and dumped at each stage (DOSBox-X): office → Restore → Sleep →
      strategic assignment → tactical → traverse into enemy sector 59 → turn-based fight **won** (4 kills, sector
      captured: owned bit, enemy count, per-enemy flags, territory/tree recalculation) → natural **end of day** (compressed
      time, End of Day + finance screens: salaries/natives/balance match memory) → office (day 2) → Sleep → day 3
      (tappers hired, DAY.SAV) → ABORT → office (OFFICE.SAV restore) → **AIM fire + hire**. Every save written on the way
      (QUICK.SAV in combat, OFFICE.SAV, DAY.SAV, GAME1.SAV) equals memory block by block (`./t savecmp`); one layout
      correction (day saves also carry the sector files). Not exercised: revenue > 0 (needs `g_processCap`), a merc
      death in a won fight, the RESTART-day OK path (seen in Ph5 notes)
- [ ] Ph7: port pixel-compared against emulator screenshots

---

## Reference Resources

- manual.pdf / quickrefcard.pdf in rom/ (game rules, merc stats, weapons).
- JA2 source (Strategy First 2004 release, e.g. github.com/dariusk/ja2): different engine generation, but same
  studio/design lineage — useful for naming and guessing intent (stats, AIM, morale), never as ground truth.
- Watcom calling conventions: register (EAX, EDX, EBX, ECX), callee cleans; `__cdecl` for AIL.
- AIL 3 API reference (Miles Sound System 3 docs) for the sound layer.
- ModDB JA1 page: sector map (60 sectors × 56×56 tiles), scanned strategy guide.

---

## Next Tasks

### Agreed work queue (2026-10-18, user approved; do in this order, commit after each step)

1. [x] **Bridge fixes** (tools/jactl.py, validate/run_x.sh) — done 2026-10-18: `Live.frozen()`/`Game.frozen()`
   (re-entrant, waits for state T; state ≈2 ms, 16 MB dump ≈8 ms frozen, 0 bytes change inside). Gotcha found: tmux
   SIGCONTs a stopped pane process, so run_x.sh keeps a shell between tmux and dosbox-x. build/dosbox-x pinned.
   - Atomic reads: freeze DOSBox-X with SIGSTOP, read, SIGCONT (≈10 ms, invisible). Use it in `dump` and `state`;
     add a `Game.frozen()` context manager for scripts reading several fields that must be consistent.
   - Pin `build/dosbox-x`: run_x.sh copies /usr/bin/dosbox-x only if the copy is missing (today it re-copies when the
     system binary is newer, and a DOSBox-X update silently invalidates every save state in build/states/). Document:
     after a deliberate refresh, re-record states with validate/to_tactical.py.
   - Leave as is: screens are not readable from RAM (tactical play area is not in a memory buffer; X screenshots
     stay for visual checks); the small input delays in jactl.Input (clicks drop without them).
2. [x] **Screen variable** — done 2026-10-18, differently than planned: there is no screen global; the screen *is*
   the live call stack (nested loops), read by `Game.stack()` / `Game.screen()` (see State Machine). No types.toml
   entry (nothing to type). to_tactical.py has no fixed sleeps left (~42 s, 3 clean runs). Original plan:
   (option 1 of the discussion): no global "current screen" byte exists (snapshot diff of menu /
   office / strategic / tactical found none); screens are probably nested function loops (MainMenu 0x4FDF4,
   office/AIM/strategic/tactical loops under GameMain 0x4FFEC). Trace those loops' input/redraw path for a shared
   pointer that identifies the screen (current hotspot table such as g_officeHotspots 0x960D2, current background
   PCX, current palette, or a dispatcher index). Name it, add it to defs/types.toml, add `Game.screen()` and replace
   the fixed sleeps in validate/to_tactical.py (incl. the 10 s "tactical drawn" wait) with waits on it.
   Fallback if it stalls: match small reference crops of screenshots (office wallpaper, strategic frame, tactical
   panel). Known non-answers: 0x92DC7 (64 outside tactical, 0 in it; written by SetupSectorFromXra; not the selected
   merc), 0x9B69C (Watcom keyboard-buffer counter in clib), tile-map occupant (already set during "Destination").
   Note: in tactical, Space did not change the selected merc; clicking a portrait does.
3. [x] **Parameter types in Ghidra** — done 2026-10-18: defs/functions.toml (175 signatures: 31 hand-written from
   label comments, 8 V diff-tested; 144 from `./t paramscan --toml`, T when ≥3 soldier offsets dereferenced, else G),
   `T*`/`?` types in jatypes + ApplyJA (updateFunction, dynamic storage from the convention), g_soldiersPtr =
   soldier*. CheckSectorTileTriggers now reads `s->tile`, `s->level`; decompile failures 3 -> 2. Plan was: new defs/functions.toml (`name = "ret (type name, ...)"` + evidence V/T/G), applied
   by tools/ghidra/ApplyJA.java (setSignature with __watcall, keep stack args for cdecl list). Seed it from
   (a) labels.csv comments that state arguments ("EAX=merc"...), (b) call sites passing &g_soldiers[k] /
   g_soldiersPtr-derived pointers (scan with capstone: lea/mov of 0xFA958+k*0x200 into EAX before call),
   (c) functions dereferencing a parameter at soldier offsets (+0x114, +0x183, +0x192 ...). Type g_soldiersPtr as
   soldier*. Check with ./t ghidra that e.g. CheckSectorTileTriggers reads `s->tile`, `s->level`.
4. After each step: update REVERSE.md / validate/README.md where tools changed, `./t check`, commit
   (no need to ask: see memory). Then report to the user.

Decided, no action: git history keeps the old screenshots (user doesn't care); machine changes stay
(build/dosbox-x capability-free copy, build/ptrace_any.so preload); library work is done (Watcom 10.0a signatures,
146 unnamed functions left in the library region, mostly AIL internals: only name them when game code needs them).

### RE Investigation

- [x] **Harness: DOSBox mouse fixed** with `autolock=false` in [sdl]: `shot.sh --click X Y` now works with 640x400
      window coords (= 2x the game's 320x200 screen). Verified: main-menu hover/click, difficulty and quick-save choices
      by mouse, office cursor visible, clicking the desk device (355,237) opens the AIM merc screen (shots/36–39).
      shot.sh finds the window by DOSBox pid. AIM screen stat order: Health Agility Dexterity Wisdom | Medical
      Explosives Mechanical Marksmanship Exp. Class (matches the label table at 0x91180).
- New-game flow observed: main menu → intro (Metavira Island) → Set Difficulty → Set Quick-Save Mode → room/office
  screen (validate/shots/19–35). QICKSTRT.SAV must be copied into validate/run/C before each run (the game renames it).

- [x] Harness: Watcom register calling in Ghidra: `__watcall` spec extension applied by `./t ghidra`
      (tools/ghidra/ApplyJA.java), cdecl exceptions in defs/cdecl.txt; pseudocode shows real arguments now
- [x] Ghidra: parameter/return *types* per function: defs/functions.toml applied by ApplyJA (CheckSectorTileTriggers
      shows `soldier *s`, `s->tile`). 2026-10-18: the 35 G soldier* entries reviewed in the pseudocode (all confirmed,
      several second args typed: tile, dir, i32* out); tile/profile/sector index + sectorState*/profile* parameters from
      `./t paramscan --index` (CFG dataflow, linear index coefficient must equal the struct size), 58 new signatures,
      233 total, G 0. Gotchas: &g_profiles[k] is often passed as the nickname string (char*, not profile*); tbl+sector*9
      functions need the sector added by hand (base is a parameter, not the global). tile.t1 0x86 = fallow tree
- [x] Harness (2026-10-18): live bridge `./t jactl` (guest RAM read via /proc/PID/mem, typed by defs/types.toml),
      DOSBox-X save states, validate/to_tactical.py with state waits; `./t check`, `./t status`
- [x] Library identification (`./t libid`): AIL 3 complete by debug strings (109 wrappers + 110 API functions +
      AIL_startup); Watcom clib/math by OMF signatures, best match = **Watcom 10.0a** (Sep 1994 libs; 106 whole
      modules, all 64 overlaps with earlier hand labels agree), gaps from 11.0c / 10.6 / Open Watcom 2.0 (label comments
      name the version). Libraries in third_party/watcom*/libs (only the 386 clib/math/emu .LIBs kept).
      Layout: game C objects 0x10000.., game assembly modules 0x67C38–0x754D7 (no strings, custom register use),
      **all library code from 0x754F0** (strcmp_) to the end: `./t status` counts unnamed functions there as
      "library?" (146 left, mostly AIL internals: XMIDI/sample code ~0x80000–0x83500).
- [x] Find `main`: `main_` = 0x10150 (via `__CMain` 0x7C9F9); startup chain labeled
- [x] Walk main_'s init chain: 7 of 18 named by string refs (see Architecture)
- [x] GameMain callees named by strrefs: MainMenu 0x4FDF4, NewGameSetup 0x4F548, SetQuickSaveMode 0x4F898,
      ContinueDayPrompt 0x4FF00, PlayLoop 0x4FB94 (was EndGameSequence; it is the office/day loop, verified on the live stack)
- [x] Sweep analysis/strrefs.txt and bulk-label obvious ones (2026-10-18): 131 of the 137 unnamed string-referencing
      procs labeled after a pseudocode skim (names without `?` = loader/message behaviour read in the pseudocode, `?` =
      string-based guess; AIL internals 0x76ABD–0x83530 as ail_*?). Left unnamed (strings not telling): 0x25CA4,
      0x44C10, 0x44D28, 0x521B4, 0x6A92A ("Quick Save" ref looks like data), 0x81A40
- [ ] Trace `GameMain` 0x4FFEC (53 calls): screen/state dispatch (intro → main menu → AIM/laptop → strategic map → tactical)
- [ ] Name the stringless inits (0x374B0 etc.) via their callees once clib/AIL are labeled
- [ ] Label Watcom clib: identify printf/sprintf/fopen/fread/malloc/strcpy etc. via their string refs & shapes
- [x] Label AIL API functions from their AIL_* debug-trace format strings (each API logs its own name): `./t libid ail`
- [ ] Locate the file I/O layer: who opens `dat\JAGGED.DAT` / `DAT\TERRAIN.DAT`; document DAT container format
- [x] Decode graphics, part 1: DAT archive container + PCX/PCC → gfx/ (`./t datarc`, `./t pcx`)
- [x] Decode graphics, part 2a (2026-10-18): TILES?.DAT compiled sprites, TERRAIN.DAT, JAGGED.DAT draw tables, the
      static tactical draw passes (`./t sectormap`, diff-tested 600/600). See "Tactical map graphics" above.
- [ ] Graphics part 2b: soldiers (DrawSoldier? 0x165E8: which bank, frames per direction/animation), ground items
      (0xA6C48 bank, `itemType` +4 = sprite), t7 marks, t3 1..9 animations (0x229E24 records), cursor pass; then
      sectormap live can reproduce a whole frame. Also SH.DAT / INV.DAT (not compiled sprites), SANIM*.DAT, FLC.
- [ ] t4 bit 7 and the meaning of g_t1Class values 0..5 / g_t2Class (walkability? LOS height?): check the movement
      and line-of-sight code that reads them (0x23538, 0x239CC, 0x239D8, 0x2FB98, TacticalClick?).
- [x] Harness: live memory dumps with DOSBox-X (AUR dosbox-x-sdl2, debugger built in): `validate/run_x.sh` runs it
      in tmux session "jax", `validate/xdump.sh NAME` → build/mem_NAME.bin (Alt+Pause, MEMDUMPBIN, F5),
      `./t memdump FILE [addr|label [len]]` reads it by image address. DOS/4GW relocates the objects: obj1 → 0x206000
      (+0x1F6000), obj2 → 0x281000 (+0x1F1000); paging off, selectors 0x180/0x188 flat. Debugger console commands
      can be sent with `tmux send-keys -t jax` (BP, BPM, D, SR ... : `HELP`); add the delta to image addresses.
- [x] Office navigation: hotspots read from the binary (g_officeHotspots 0x960D2, labels.csv). "Continue Game" does
      NOT load game0.sav (empty team, difficulty EASY): use the calendar's lower half (Restore Game) → slot list.
      The earlier "hung" options panel was the Control Panel; the door is Leave (back to main menu).
- [ ] **Next session (start here):** `./t jactl load tac60` (day 1, tactical sector 60) or a fresh
      `.venv/bin/python validate/to_tactical.py`. The Ph6 in-memory state (office day 2, Larry fired / Ears hired) was
      lost when DOSBox-X was restarted for the bridge on 2026-10-18 and run/C/OFFICE.SAV + DAY.SAV were overwritten.
      To resume Ph6 (sectors 59+60 owned, Micro-Purifier on the ground in index 58): copy build/OFFICE_day3.SAV to
      validate/run/C/GAME2.SAV, calendar Restore → slot 2, then `./t jactl save ph6_office`. Mid-fight saves:
      build/QUICK_*.SAV (copy to run/C/QUICK.SAV).
- [x] Trace the bullet flight / obstacle test: done (ProjectileHitTest, flight_diff; spec combat.md Bullet flight).
- [~] **Enemy AI (2026-10-18):** pathfinder and the whole decision layer ported + diff-tested (see Key Findings
      "Pathfinder and enemy AI", spec ai.md). Next: port the searches it calls, one by one with ai_diff-style tests,
      done (all diff-tested in ai_diff, spec ai.md): OppStrength, SoldierPower, ArmorRating, TargetTile,
      UpdateSightings, MoveApCost, NoiseTile, AllyCallTile, NewsValue?, TileDistance, Patrol/Buddy/Approach/Advance/
      Retreat/Search/DryTile, CanStandAt, TileStandable, PositionScore, Gun/Knife/ThrowOption + ExpectedDamage,
      MeleeChance, KnifeDamage, ThrowDamage, ThrowApCost, ThrowChance (2026-10-18),
      SightCheck + MarkSeen + NoAllySees, SectorValue, ShareSightings, AiIdle + ClearTarget, ReachTile, AttackReady,
      SightLine + TileOpacity (2026-10-19). Every AI search is now ported.
      Composed AiThink test clean on all dumps (compose_diff.py).
      **Live check done (2026-10-19, validate/live_ai.py):** state fight59 (day 2, sector 59 combat, enemy 42), DOSBox-X
      debugger breakpoints on AiThink entry + caller returns, 25 real ticks of enemy 42 (it chose to shoot, then the
      shot ran): composed port = original on all 25 (whole data object, leaf calls); real game = Unicorn except the
      caller's stack bytes and, in long ticks, bytes that change outside AiThink (soldier +0xB0/+0xCC of all 48, view
      and buffer areas). Gotchas: AiThink also runs every frame for the selected merc (returns at once), so the
      breakpoints slow the game to ~2 stops per frame; a plain click is lost while stopped — press DONE as a held
      click; a DONE click made while the debugger held the game went into the all-move handler instead.
      ThrowChance has its own direct test now (the +10 at the last target was never decisive inside ThrowOption).
      Original quirks seen by ai_diff (cases skipped, counted): ThrowOption's aim loop counts a byte with `jbe` up to
      its bound, so a bound of 0xFF never ends (fuzzed AP values, ~2 %); SearchTile also never returned on 2 of 3000
      fuzzed cases (not traced).
- [ ] `g_processCap` [0x22A024]: who raises it (0x432F4, NativeGuidePrompt?, HomeSectorLost?, 0x62EB4); try bringing the
      Micro-Purifier home and check revenue at the next day end.
- [ ] Name the remaining tile bytes 0–4 / byte 3 of g_tileMap from code (SaveSectorTmp keeps 1,2,3,4,6).
- [ ] Profile bytes still unnamed: +0x2F, +0x33, +0x3E, +0x4B (status: 100 on team, 50 just hired, 1 after firing),
      +0x4C (set 100 by the salary negotiation), +0x50.. ; score function 0x1FDEC (reputation?).
- [x] Ph5 follow-up: +0x183 = current, +0x184 = max health (wounded mercs in mem_combat1, see Ph6 live session).
- [x] **Tactical driving notes (from the Ph6 attempts, read before playing again):**
      - View origin = [0x229CF8]/[0x229CFC] world px; one arrow key = one tile (16 / 14 px); the 320x200 screen shows
        world (x, y) at (x − vx + 48, y − vy + 12) = **the tile itself: click exactly there** (aiming 4–5 px higher hits
        the tile north of the target, which wasted a whole volley). `./t tacview DUMP [--center WX WY]` prints these.
      - Select a merc by clicking its figure (no recentre) or Space / double-click portrait (recentres). Click twice to
        walk; "CONT" on a portrait / blue frame = interrupted move, re-order it. In real time only: hold left on the
        destination + click right = **whole squad gathers** (used to reach the sector edge in one go).
      - Fire: right-click in the play window (target cursor, shows AP cost: .38 = 8 AP), then click the target twice
        (`validate/fire.sh X Y`). The main hand must hold the gun: right-click the secondary-hand slot (292,195) swaps.
      - Interrupts: the view centres on the enemy that triggered it; **scrolling works during an interrupt**. Handle each
        interrupt by hand (shoot or DONE at (67,7)); a scripted loop that ignores them gets stuck.
      - **Cursor shapes: square + number = move (AP cost), circle + number = target.** Right-click toggles between them;
        a click with the circle cursor on a merc fires at that merc (Tex shot at Grunty's tile once this way, missed).
      - Turn OFF "Follow move" (OPT) before scripting moves, otherwise the view follows each walker. The view still
        recentres on any enemy a merc spots. Clicks sent while a merc is still walking (trees are slow) are dropped;
        wait ~12 s. A merc whose tile is at y ≥ ~183 cannot be clicked: scroll first.
      - Scripts: `validate/center.sh COL ROW`, `validate/move.sh SX SY N DX DY`, `validate/shoot2.sh MX MY TX TY [SLOT]`,
        `validate/qsave.sh`, `.venv/bin/python validate/advance.py DX DY SLOT…` (centre, select, walk by a tile offset;
        a target tile that is occupied or blocked makes the merc stay put). Mercs reload automatically (manual).
      - Walking costs ~2 AP per open tile, ~4–5 in brush/trees; enemies hide in trees, so get a clear line, not just range.
      - QICKSTRT mercs carry only the 6 rounds in their .38 and medics/Vinny hold a kit in the main hand: outfit first
        (above). Enemies in sector index 58 carry .38s too and can kill a 100-HP merc in a few turns.
- [x] Harness: `validate/to_tactical.sh` (route below, scripted), `validate/xin.sh` (DOSBox-X input in game coords),
      `./t soldiers DUMP` (slot table). `xdump.sh` now re-breaks until CS = 0180: a break inside a real-mode interrupt
      makes `188:0` mean segment 0x188 and the dump is garbage (happened once). Debugger input is cleared with Backspace,
      not Escape (curses merges Escape + next key into Meta-key). In tactical: Space = next merc, click twice = walk,
      arrow keys scroll, T = traverse, DONE at (67,7), view = 1:1 world px.
- [x] **Harness: private X display** (2026-10-18): `validate/xdisplay.sh` starts Xvfb (headless) or, with `--watch` /
      `XD_MODE=--watch ./run_x.sh`, a Xephyr window on :99 (`JA_DISPLAY`); run_x/xin/xdump/to_tactical source xenv.sh
      and use `windowfocus` (no WM there), so driving the game never touches the user's mouse. dosbox-x.conf runs
      `JA.EXE ALLOWSAVE`: restoring the pre-1.14 QICKSTRT-based saves patches g_quickSaveInCombat 0x22A1F9 = 1
      (verified in mem_sec60b), so OPT → Quick Save works in combat. GAME1.SAV slot now = copy of the outfitted
      DAY.SAV (old one in build/GAME1.SAV.bak): calendar Restore → slot 1 → bed → TEAM → 60 → OK → OK → tactical.
- [x] **Route to tactical (DOSBox-X, game coords):** Escape ×4 → Continue Game (140,74) → calendar Restore (197,72)
      → slot "Day 0 Quick Start" (160,43) → bed (50,142) = Sleep → "Day 1" → strategic map: Return to dismiss
      dialogs (a click on the map clears the selection), **TEAM button (39,168), then an owned sector** (home = 60 at
      (300,128); `g_sectorTbl9[i].b0 & 1` = owned, else "enemy hands", see 0x439A2; g_curSector is only set in TEAM
      mode, 0x43A28) → OK (290,191) → Return ×2 (hints) → OK → "Get to work?" OK (226,107) → tactical sector 60.
      Dump build/mem_tactical60.bin (shot validate/shots/t24.png): g_curSector = 0x3B (0-based, UI shows 60);
      slots 0–7 = the 8 mercs, tile/x/y consistent with TileToX/Y (974 → 357,243); **slots 8–15 = the 8 hired guards**
      ("GUARD n", profile id 64, own HP); slots 0x28–0x2F "ENEMY 40".. empty (tile **0x0C41** = off map, HP 0:
      home sector has no enemies). Next: enter an enemy sector for the 0x28..0x2F slots and a wounded merc.
- [x] Decode CD data (2026-10-18): speech archives, EDT quote text, NEWPROF.EDT, MISC patches — see Data Structures.
      Open: credits.bin, FX\\*.BIN, SH.DAT / INV.DAT sprites, rom/INV narration indexing.
- [x] Save format (2026-10-18): `./t savefile` parses both formats from the original's own fwrite_ sequence
      (SaveGame / QuickSave via ioseq.calls) + tail; 19/19 local saves round-trip (`validate/diff/save_diff.py`).
      Spec: docs/spec/savegame.md. Named in the 0x229FA0 block (`./t varmap 0x229FA0 0x356` lists the rest):
      ledger (CalcTeamPay 0x51068, CalcNativesCost 0x5102C, CalcLedger 0x5113C, CalcLedgerAll 0x51178,
      ApplyDayLedger 0x51210; old g_nativesCost was the guards' wages only), game totals (merc/native/enemy deaths,
      sectors owned/lost), AIM statistics (AimRate 0x1FDEC; profile +0x36..+0x39 = refusal thresholds), DayScore
      0x61264 (-3..3, rating words TERRIBLE..OUTSTANDING; g_badDayStreak > 4 ends the game in DayEndVerdict 0x60680),
      g_plants (processing plants, capacity + state; g_processCap), g_difficulty 0x22A0CC, g_statProgress +
      StatPractice 0x62A68 (stat gain rolls), g_options 0x1163B8 (16 Control Panel switches), g_sectorOrder 0x92DF0,
      g_notesFound 0x92ED2.
      Day's verdict (asm read 2026-10-24, not yet spec/port; S[i] = byte 0x22A040+i = world `story`, A+o = `accounts`):
      DayEndVerdict 0x60680 (caller DayEndScreen? 0x65C39 after DayReplay + b5 clear; result -> 0x93449, != 0 = game
      over): crate check 0x62698 (crate_state 0x22A06B+k vs g_crateDrops day/sector/+3 flag, CrateLost 0x62780,
      A+0x90 += 10 per state 2 with flag 0), DayScore 0x61264 (all 16-bit: snapshots 0x229AC4 trees owned, 0x229ACC
      workable, 0x229AD4 tapped, 0x229AD8 sectors, 0x229AF4 processCap, 0x229AF8 plants are written with the death
      counter resets in LoadTacticalSector? 0x2AFA3..0x2B016; the port's sector entry does not write them yet).
      Order: home 0x3B not owned -> StatusMessage "Home sector LOST!", 4; S[0xA]=1 and no owned sector with trees
      -> say 0xB2, 3; streak > 4 -> say 0x95, 2; DrawText rating word 0x965D6+12(score+3); S[0xB]=1 block (S[0x16],
      S[0x15], S[0x14] counter, flags 0x22A1F6..F8 for items 0x86/0x7A/0x5C, i16 tables 0x95CA8/0x95C7E/0x95C9C/
      0x95CA2/0x95C84 by difficulty, 0x92DD6 = 1 + u16 0x92DD8 += 15000 / 1000, read by 0x615DC); streak 4 -> 0x94;
      deaths ladder (quote = score + 0x2D/0x3B/0x26/0x34/0x42; ESI keeps it and the later process-cap quote starts
      from ESI); S[5] (item 0x79), S[0xA] blight (0x5E, 0x51, Brenda 0x10/0xE/0xF/0xD -> S[0x22]), S[0xE] (0x6E,
      0x7C), plants 1..3 state 3, process-cap quotes, Random(5)*7+score+3 when nothing said, 0x61488(n) = all 5
      sectors of list 0x95BD8+16n owned (n = 1 and 2), connected(5) / S[8] / S[0x13], S[0xC] (0x77, A+0x94 += 150),
      plants state 0 -> 1 with countdown Random(3)+2, S[9] + connected(0x1B) -> Brenda 0xB. Say = 0x5F5AC(q, 3)
      = host op 302 guide_say; FindSquadItem EAX type, DL drop, BL 0x40 (all), CL probe. Open: ~130 unnamed bytes of the block (story flags 0x22A045..0x22A067, Walter
      0x22A0AE..0x22A0CA, tactical 0x22A0CD..0x22A10D, guard screen 0x22A10D..0x22A129) and 126 of the 144 quick-save
      blocks.
- [ ] Video/input layer: VGA mode set (640x480 SVGA via VESA? intro is 640x400), mouse INT 33h, keyboard
- [ ] Game state: merc struct (stats from manual: health, agility, dexterity, wisdom, marksmanship, medical,
      explosives, mechanical, experience), sector/map struct, inventory items
- [~] Combat rules (2026-10-18): to-hit, shot/move AP, bullet damage, hit location + armour diff-tested (spec
      combat.md). Open: verify TakeDamage, StatPractice; melee, thrown items and explosions (0x3843C, 0x38638,
      0x401B4); morale/interrupts. Pathfinder and AI decisions: done (item above).
- [ ] Decide port target (C/SDL2 source port vs web) once core formats are known

### Port

### Documentation

- [ ] docs/architecture_exe.md (module map, call graph, per-turn flow)

## Merc panel (tactical view, 2026-10-23)

The tactical screen keeps a 320 x 200 panel buffer ([0x93E60], pitch 320): 8 merc slots of 48 x 42 (slot origins
g_slotPos 0x93686: x 0 / 272, y 14 / 56 / 98 / 140), the top bar and the bottom bar. 0x33204 copies one slot to the
screen. DrawMercSlot? 0x32EB8 (AL slot, DL copy) draws a whole slot, in this order:
1. element 0 (7 when the slot is the selected merc 0x284F11) of the panel sheet [0x93454] at the slot origin, then
   the bar frame element 4 (11) at the health-bar offset. Elements: table 0x93732, 8 bytes (i16 x, y, w-1, h-1),
   drawn with key 0 (0x469F0; 0x46990 with a given height).
2. the name (0x32DB8): nickname + 0x20 on its first byte (into g_textBuf 0xEBAC0), font 1 at (3, 0x20); colours
   0x16/0x15 selected, else 0x0E/0xFE.
3. in the sector (+0x182): health bar 0x323D4, energy bar 0x32610, AP bar 0x328A4. Each is 25 rows; per-side offsets
   g_slotOffsets 0x936A6 (+0x180 = 0 left column, 1 right). Health: gap (100 - hpMax) / 4, wound rows
   (wounds - 1) / 4 + 1, empty rows, hp rows (hp - 1) / 4 + 1 (one less when the remainders say so). Energy: +0x18A / 4
   (at least 1 when > 0). AP: apLeft, at most 25.
4. the portrait 0x31C98 (unless +0x194 and slot == 0x284F0E): face 31 x 28 from the face buffer [0x93428 + 4 slot]
   (32 x 32: rows 28..31 = two 2-row eye strips), +0x1AC eye state (set to 0 when +0x189, 3 when AP 0 and hp 15+),
   eye strip at row +0x1AA when +0x1AC < 2; tint element 17 (remap [0x1B] = colour: dying table 0x94BF8[+0x189] or 2
   when in the sector but out of play); icon +0x181 = tile sprite 413 + icon (0xA92BC = g_tileSprites + 4 x 413, the overlay sprites; 0xB7..0xB9 = first aid);
   the AP number (combat 0x92DCB, hp 15+, in play: red 0x72 below reservedAp +0xEF, 0x16 when g_apWarned, black box
   behind it), darkened face (table [0xE8CBC]) when +0x17E or AP 0, ON DUTY text (0x9662A + 8 x +0x17E, 1..7),
   frames 0x333B8 (face frame 0xFF for slot 0x284F0E, else 0xA2 / 0x16 by +0x1A8, face or whole slot by +0x17F), and
   a 0x27 frame round the bars for slot 0x93448.
5. +0x194 or +0x1BB: element 104 four times (remap [0x1B] = 0x70), then the portrait again for slot 0x284F0E.
Mirror: tools/panel.py (`./t panel DUMP OUT.png`). validate/diff/panel_diff.py (in ./t check): 0x32EB8 in Unicorn vs
the mirror on 5 dumps, slot area filled with noise, soldier fields and panel globals fuzzed: 600 cases 0 mismatches;
2 hand mutants caught. Unicorn trap: 0x32DB8 copies the name with an inline strcpy that pushes ds/es (faults in
Unicorn); panel_diff hooks 0x32DCE and goes on at 0x32E07.
Screen layout (2026-10-23): the panel buffer [0x93E60] is the whole tactical screen (320 x 200); TacticalRedraw copies
the map window from g_backBuffer (32, 28) to (x [0x229CB8], y [0x229CC4]) = (48, 14) in all tactical dumps (14 x 12
tiles = 224 x 168, so the map fills x 48..271, y 14..181 between the two slot columns). At sector start
LoadStripPccs? (0x2FB83) draws all 8 slots through 0x331A4 (slot from record +0x17A): +0x17F = 0 -> DrawMercSlot?,
+0x17F = 1 -> 0x330C0 (the slot's item view: three items via DrawMaxAimLabel_, the name, the frames; not mirrored yet).
Redraw check (validate/screen_redraw.py, 2026-10-23): the mirror redraws the 8 slots of each dump's panel buffer;
100 of 131 dumps with a merc in the sector come out the same. Mismatches: other screens using the buffer (office,
AIM, outfit, eod), and 2..9 pixels in single slots (e.g. a30 slot 0, w30 slot 5: probably a change the original had not redrawn
yet; not traced). The port (port/ja1front src/screen.rs) redraws all 8 slots every frame.
Panel data (loaders, 2026-10-23; mirror tools/panel_data.py, test validate/diff/panel_data_diff.py in ./t check:
14 dumps 375 checks 0 mismatches, `--all` = 132 dumps 3671 checks 0 mismatches, 4 hand mutants caught):
- panel sheet [0x93454] = MAIN.DAT PANELS.PCC (320 x 160, pitch 320; LoadPanels 0x46EC4). Other screens (sunrise
  HELIN6.PCX, money, yes/no dialog, speech balloons) use the buffer as scratch and call LoadPanels again after.
- face buffer [0x93428 + 4 slot] (32 x 32), LoadMercFace 0x286C4 (AL slot, DL = +0x179 profileId = face number):
  MAIN.DAT FACES.PCX (loaded once into the panel buffer, flag 0x9344B), cell 28 x 29 at (28 (n % 11), 29 (n / 11)):
  rows 0..24 to buffer (2, 2), rows 25..28 (eye strips) to buffer rows 28..31 at x 0; +0x1AA = JAGGED.DAT byte [n].
  hp <= 0 at load: DEADFACE.PCC to (2, 2). Death in the sector (MercDies? 0x3A4A4): MORF<n>.PCC (CD DAT/MF1.DAT for
  n < 30, else MF2.DAT), 10 frames of 28 x 25 side by side, drawn in turn to (2, 2) (the buffer keeps the last one).
- fonts: rom JA_SMALL.FNT, JA_M_ENG.FNT, JA_BIG.FNT read whole one after another at 0x22570C (sizes 0xC16, 0xF72,
  0x24FC are built into LoadSmallFont 0x381F8), pointers 0x229790. Char table 0x9453A (3 x 131 x (u16 offset,
  u8 width)): widths from the exe, offsets = running sum of the widths, filled at load.
- remap tables: 34 x 256 bytes at 0xE8D00 (pointers 0xE8C48 + 4 k) = JAGGED.DAT 0x3470..0x5670 (LoadJaggedDat
  0x371BC), then darken table 29 [0xE8CBC] byte 0x1B = 0xC6. Each sector load (LoadTerrainTileset 0x2A9FC) rebuilds
  table 28 from the tileset and sets darken bytes 0x27 = 0x2B, 0xF1 = 0xF2. Table 0 byte 0x1B = the tint colour,
  set by the slot drawing before each tinted draw.
- exe tables that never change at run time (all 132 dumps = JA.EXE): 0x93686 slot origins, 0x936A6 side offsets,
  0x93732 sheet elements (0..104), 0x949D6 font sizes, 0x94BF8 dying tints, 0x9662A ON DUTY texts. Still from
  the game files for the port: the icon = tile sprite 413 + (+0x181) of the sector tileset (compiled sprite).
## Tactical inventory (first map, 2026-10-23)

How the player reaches items in the tactical view (FUN_00011714 = the per-slot mouse handler of the panel, TacticalKeyHandler):
- Mouse over the bar area of a slot (11 x 27 at the health-bar offset): 0x93448 = slot, a 0x27 frame round the bars.
  Left button held there: DrawPopupInventory? 0x3FF3C shows RPOPINV.PCC (100 x 42) with the 10 item entries
  (0x3FE50) next to the slot (x + 48 left column, x - 100 right column) until the button is released (display only).
- Left click elsewhere on the slot: select merc (0x1148C) or, with the selected dead-flag merc, the pending action.
- Right click on the portrait (+0x17E = 0, hp != 0): +0x17F += 1 (flip animation 0x332E4) = the slot's switch view
  0x330C0: three switches at (X + 0x23, Y + 5 + 9 i) toggle +0x16A, +0x16B, +0x16C (sprites 'F'/'G', 'J'/'K' when
  selected); after +0x16A / +0x16B ReserveAp 0x17C88 runs again. Right click again flips back.
- Key 'I' (0x49, TacticalKeyHandler, hp 15+): 0x54D50 = the inventory screen: DrawMercBody? 0x555DC (body strips) +
  UseInventoryItem? 0x53A9C (580 lines, the UI loop with the cursor item 0x222B50): item moves cost AP through
  MsgNotEnoughAP 0x15960; 'Better put that down first!' when an item is on the cursor. After it: in all 8 records
  +0x1BB and +0x130 = 0, LoadStripPccs?, ReserveAp, and per merc in the sector 0x1F2C4.
AP in 0x53A9C (asm, MsgNotEnoughAP DL = AP, BL 99): 5 at 0x53C63 (rect (0x6C,0x3E)-(0x9C,0x56), spent 0x53CE8
SpendAP? 5) and the rect (0x70,0x52)-(0x96,0x76); 10 at 0x53DDB (ground pick-up, item 0x13 / 0xFF checks, then
0x28EF30[slot] = 1 = "already picked here"; spent at 0x53E2E); 6 at 0x53FAE / 0x53FEB (cursor item to another merc's
slot: both mercs need 6 when hp 15+); 4 at 0x549BA (PutItem 0x3FC78 of the cursor item, spent 0x549DF SpendAP? 4).
Other callees: MergeItemsPrompt 0x4A838, UseThrow? 0x535E8, DropItem, PutItem, 0x53580 (ctrl-click on an entry).
Next: mirror these moves as state functions (item entries, cursor item 0x222B50, AP) and diff-test them.
Harness plan (2026-10-23, asm read 0x53A9C..0x54AF9; probe works): run 0x53A9C in Unicorn on tac60 with the mouse
poll 0x678E8 stubbed (it copies 0x229D00/0x229D04 to 0x229CF0/0x229CEC, then reads x, y, buttons [0x229CF4] via
0x68470(EAX &x, EDX &y, EBX &buttons)): the stub feeds scripted (x, y, buttons) events and stops the run when they run
out; KeyPressed 0x5616C = 0; UI stubs: 0x673AC, 0x67BF0, 0x674B0, 0x67478, 0x5C880, StatusMessage 0x27D24,
PlayInvExtra 0x25AD8, ItemInfoBox 0x25D20 (once named PlayMoneyAmount), DrawMercBody 0x555DC (returns the armour value the loop compares),
DrawItemSlot 0x4AAD4, item info 0x25B84, tooltips 0x23420 / 0x23264, 0x4B730 / 0x4B764, DrawVestMap 0x48000,
MsgMercText 0x15910, PlaySfxSlot 0x5CB34, slot redraw 0x331A4, yes/no 0x278A0 (scripted answer), 0x682AC, 0x3F994.
Left-click order per poll: rect (0x6C,0x3E)-(0x9C,0x56) with AP 5 (canteen 0x41 / 0x4F: drink, SpendAP 5, Rest
(100 - energy) x n), rect (0x70,0x52)-(0x96,0x76) with AP 10 (camo kit 0x47: +0x1B1 += 3, SpendAP 10, cursor
cond -20), rect (0x46,0x1C)-(0x66,0x38) (face: leave when the cursor is empty, [0x93EA8] = 0, [0x284F13] = 1),
then x outside 0x30..0x10F = the 8 panel slots (g_soldiers[k] direct; AP 6 for both mercs when the cursor holds an
item for another merc; SelectSoldier 0x17D80; +0x1D0 = level > 3, +5 - (100 - item4 cond) / 20 with item 4 type
0x2B; 0x2B6A0), else the 14 item rects (table 0x949F2: i16 x, y per entry, rect x-4..x+0x2D, y-4..y+0x13; ctrl =
0x53580): 0..11 pockets (reload: empty gun < 0xF + cursor = its ammo 0x94C32 + 6 type, count 1), 12 = item 0's
attachment bytes +3..+5, 13 = the ground (cursor empty: pick up with AP 4 once per merc [0x28EF30 + slot],
MineSpotted, DisarmExplosivePrompt, PickUpItem; cursor full: PutItem on the merc's tile, vest 0x32..0x35 moves
the whole vest of merc [0x92E40]). MergeItemsPrompt 0x4A838 gates every pocket move (1 = go on, 0 = "huh",
-1 = cancelled; merge table 0x94AAA: 82 x (a, b, result, prompt)); swap = copy cursor to 0x222B68, cursor <- entry,
entry <- 0x222B68 (0x47090 = copy 12 bytes, EAX dst, EDX src); shift = take the whole stack, else one unit (0x535E8).
0x53958(slot) = the "gave an item" pair [0x28EF38] / [0x28EF39]: both mercs SpendAP 6 (fatigue 0x14) when hp 15+.
Mirrored 2026-10-23 (late): tools/invscreen.py = the loop 0x53A9C with the same poll structure + its item helpers
(0x53958 gave-item pair, 0x53580 ctrl unstack, 0x538A0 stack merge, 0x538FC drop lost pockets, 0x470E8 pocket swap,
0x4A464 vest pockets 7..11 by vest type 0x32..0x35). validate/diff/inv_diff.py (in ./t check, 2000 cases on 5 dumps,
~25 s): original in Unicorn with scripted mouse / key events vs the mirror, UI + prompts + Spend AP / Rest / PutItem /
PickUpItem / SelectSoldier / MergeItemsPrompt stubbed on both sides; whole data object + call list compared: 0
mismatches, 10 hand mutants caught. Details read from the asm: +0x1D0 = (level > 3) + (item 4 = 0x2B: 5 - (100 -
cond) / 20); reload = gun 1..14 with 0 rounds + cursor = its ammo, count 1 (+clip, StatPractice 3 when +0x1D5 > 0);
right click under the panel (y + [0x229C48] >= 0xB6) leaves; a ground pick-up whose item is gone afterwards = a mine
went off: sector mines - 1, FocusOn, leave.
MergeItemsPrompt 0x4A838 mirrored (2026-10-23, invscreen merge_items / merge_apply 0x4A4DC / vest_not_empty
0x4A7BC; inv_diff stubs the question 0x278A0, Random, PlayQuote, YesNoPrompt? instead; 2000 cases 0 mismatches, 12
hand mutants: 11 caught, 1 equivalent = s[+4] after a kind-4 overflow, the caller clears +3..+5 for result 4).
Rules: a vest on the cursor (0x31..0x35): entry 6 yes, entries 0/1 only when the vest owner's pockets 7..11 are
empty, else no. Entry 2 helmets 0x23..0x26, 3 = 0x27..0x28 (sun goggles only one), 4 = 0x2A..0x2B, 5 = 0x2D..0x30,
6 = vests, 7..11 = no vests, 0/1 and others = anything. Entry 12 (on the hand item): no when the cursor item has an
attachment (+3) or either item is a stack of 2+; table hit with result 0 = attach (silencer, scope, note on lab
coat); else the question, then by kind: 0 = paper -> taped paper / combination (StatPractice 4, 2), 1 = chunk of
steel + gun -> modified gun, roll Random(100) < mechanical - (100 - dexterity) / 2 else damage (Random(3): both
half / hand / cursor item lose roll - skill condition; 0 condition = item gone), quote 6 + YesNoPrompt? text, 2 =
compound 17 + armour -> treated (always), 3 = explosives + detonator, molotov parts (new condition from both
conditions, explosives - Random(100), 10..100), 4 = kit + same kit (conditions added up to 100, the rest stays).
Result type takes the table's result; a result type with clip +5 = 99 also sets the count to 99.

## Game cursor (read 2026-10-24; drawing in docs/spec/cursor.md + ported, hover notes there)
- Mouse pointer (drawn by the game into VGA): SetMouseShape? 0x674B0 (shape 0 = panel element 78 arrow, 200 = element
  62 (button held), 201 = 63, 202 = 97, 203 = 98; any other shape < 0xC9 = the item picture of that item type, 42 x 16);
  hotspot offsets 0x229C70 (x, default -3) / 0x229C48 (y). Poll 0x678E8 swaps 0 <-> 200 / 0xCA <-> 0xCB while a button is
  down; HideMouse? 0x67368 / 0x67330, ShowMouse? 0x673AC, restore 0x67848, draw 0x67A28 (saves the background); hidden flag
  0x284F0B; above y 16 (top bar) the pointer is not drawn (0x92E70). Over the tactical map the pointer is HIDDEN: UseItemAt
  sets 0x284F13 = 1 and the tile cursor is the only cursor.
- Tile cursor: value 0x284F1D, mode 0x284F1B (0 none, 1 walk, 2 weapon, 3 hand), mouse tile 0x229D2C. Picture =
  TILES%d.DAT sprite 413 + table 0x9545A[value * 2 + 0x955C8] (0x955C8 = out of range, column 1 = dim variants
  165..180). Values: 0x8E walk square, 0xB5 X (can't go), 0x91 yellow brackets (own merc), 0xA1 "?", 0xA4 DOOR,
  0x94 hand, 0x77 red hand (ground item), 0x8C grave, 0x8F red square (confirm walk), 0x92/0x95/0x96/0x97/0x98 weapon
  kinds (circle, brackets, knife, syringe, key; = item kind via 0x1C39C), 0x74/0x75/0x76 = red versions on a soldier,
  0x93 first aid, 0x8B/0xA2 red syringe / key, 0x9C fire confirm, 0x90 "ALL".
- Drawing: TacticalRedraw clears 0x284F03 and name x 0x22A480 = -1; DrawTargetMark? 0x16DD0 (after shadows): value
  0x8E..0x91 or 0xA4 and a soldier on the tile (occupant < 0x30) -> sprite now (under the soldier), else 0x284F03 = 1;
  DrawCursorPass? 0x171E8: 0x284F03 = 0 and value outside 0x8E..0x91 -> sprite at (-2, -2); 0x284F03 = 1 -> at (0, 0)
  (0xA4: at (0x22A47E, 0x22A47C)); name text 0x22A444 at (0x22A480, 0x22A482) (DrawSoldier 0x16921 sets it for the
  hovered soldier); then marks 0x229CB4 / 0x229C94 / 0x229C8C / 0x229CAC and the clock. Sprite routine 0x15E8C flag 0:
  tile visible in the window, saves 16 x 14 under it, blink timer 0x229D4C (200, toggles 0x95F5C), AP number 0x229DE0
  (itoa of 0x229DEC, shown in combat 0x92DCB when != 0) by DrawText font 0 centred at (x + 9 - mode, y + 4), colours
  0x92E5C / 0x92E60 (1 / 0 = not enough AP, else 0xFF / 0xFE; 0xFF / 0xFE while blinking unless 0x95E0D); door wobble
  0x229D3C > 0 -> sprite 0xA4 offset -4 in x (0x284F05 = -2) or y.
- Hover (UseItemAt? 0x1D9B8, no button, 0x1E436..0x1ECFD): new tile -> timers 0x229DD4 = 800 (AP number delay, -10 per
  call) and 0x229DDC = 2000 (item hand delay), mode 1 -> value 0x8E, HidePathMarks(0). Soldier tile (occupant < 0x40,
  +0x9C > 0): merc -> portrait light 0x3349C / name, own selected merc -> 0xA1 (alive) / 0x8E, other merc -> 0x91;
  enemy -> SetSpeakerName 0x336E8 (health word DYING.. by hp / 15), weapon values -> red 0x74..0x76. Empty tile:
  mode 1 + 0x8E / 0xA4 + timer done -> PathApCost(s, tile, mark = (0x229DD8 = 0), 0, shift 0x284F21, +0x177, 0x284F20)
  = AP number + path marks; TileStandable < 1 -> 0xB5 (0) or 0xA4 / 0xB5 (door, by 0x229D3C); item tile after the
  2000 delay -> 0x94; tail 0x1ECFD: mode >= 2 -> 0x955C8 = distance > range (mode 3: 15; throw 0x58AB8; else range x 15).
- Original pictures without the X window: /tmp-style harness = Emu(dump) on build/mem_fight59.bin (new dump, combat,
  Hector selected): call 0x1D9B8(tile) ~100 times with UI stubs 0x67BF0 0x67C54 0x67C68 0x331A4 0x3349C 0x33570 0x336E8,
  then 0x46F3C, 0x169E8, 0x16C48, 0x16DD0, 0x17028, 0x17098, 0x17548, 0x171E8 on a zeroed back buffer (window at 32, 28).
  Shows the walk square + AP "18" + the dotted path marks. (The DOSBox-X X-window grab under Xvfb stayed stale.)
- Ground pass 0x169E8 also draws ground items (DAT/SINV.DAT bank at 0xA6C48, sprite = item table +4, tile byte 6 < 200,
  item b6 > 0, t1 < 0x75), t7 marks (high nibble: sprite 644 + v for v < 10 else 595; low nibble: 652 + v) and t3 1..9
  animations: the front-end draws none of these yet.

## Game end and SAB scripts (read 2026-10-25; spec: docs/spec/ending.md, which corrects the notes below)
- **Game end code** = byte 0x93449 (world `leave_reason`). PlayLoop 0x4FB94 after TacticalLoop: 0 -> DayEndScreen?
  (day verdict writes 2/3/4 here), then a jump table 0x4FB84 on values 1..4:
  1 (win: Santino spoke in sector 0 and the squad left, leave path step 2d) -> delay 2000 (0x67250), WinGameSequence?
  0x5F2F8, PerformanceRating? 0x62EB4, 0x382AC(g_textBuf) = copy as exit message + exit_(0) (the program ends; the case-2 code after it is never reached after a win);
  2 (five bad days) -> SAB DAT\ENDSAB.DAT LOS1GAME.SAB (0x5D0C8), 0x5CE08 (all sound stops), StopMusic;
  3 (blight) -> LOS2GAME.SAB, same; 4 (home lost) -> nothing. All: GameOverScreen 0x504B4, NewGameInit(difficulty
  0x22A0C9), 0x22A043 = 0, LoadMainSound, 0x93449 = 0, back to the main menu.
- WinGameSequence 0x5F2F8: sets SAB vars 0..3 (0x337798..): v0 = (0x22A048 = 1 or 0x22A055 != 0), v1 = (s8 0x22A04B >
  0), v2 = (0x22A04D = 2 and 0x22A05E = 0), v3 = (FindSquadItem 0x5F found and 0x7B not found); music MISC\intro;
  SAB ENDSAB WINGAME.SAB (0x5D0EC + 0x5D2D4); when it ran to the end: 0x93D58 = 1, 0x5EBE8(0x4F, 0x50, 7), 0x5EDE0(0,
  0, ""), FLC INTRO05.FLC (0x5BFF8(name, 0, 0xF, 1, 0)); then PlayTitleIntro(0), music fade.
- **SAB = a script of 44-byte records** (record size dword 0x96388 = 0x2C) read one by one from an archive member
  (0x5D0EC opens archive EAX at handle 0x92F50, dir 0x10AB58, ArcSeek member EDX; vars 4..25 = 0). Record (copy at
  0x337914): +0 op, +2 a0, +4 name (18 bytes), +0x16.. +0x2A = w0..w10 (s16). Vars = s16 at 0x337798 + 2 i; an
  operand >= 10000 means var (operand - 10000): 0x5CF74 (ops < 17 only: a0 of ops 1..7 = var index, else value; w1..
  w10 resolved). Loop 0x5D2D4 (top 0x5D2FF), jump table 0x5D1B8 (op 1 = entry 0; 14 = no-op; others/99 = nothing).
  After each op: key poll (Escape -> 0x95DC4 = 1 -> stop all, return 0); cond 0x96354 = 0 -> skip records to op 19 or
  43; read next record; op 99 -> re-seek the member start, read, and return 1 unless op 20 ran (then loop again).
- Ops (seen in the 3 end scripts): 1..7 var = / += / -= / *= / /= / %= / Random; 8 PaletteSet(slot a0, w1, w2);
  9 sprite[a0].+0xA = buf1 + w2*W1 + w1; 10 FLC(name, a0, w1, 1, w2); 12/13 sprite blits; 15/16 sprite draw
  (0x5E018 copy / 0x5E178 key; w1 = 9999 -> its saved place; centre flag 0x96374 subtracts half size); 17 0x5E2EC
  timed rect copy buf1 -> screen (w1..w4, 9999 = last rect); 18 if (0x5E514: two compares, op codes 1..7 / 1..6, w3 =
  0 one compare, 1 = and, else or); 19 endif; 20 loop flag; 21/22 fgetpos loop; 23 0x5E960 wait a0 ms then buf1 ->
  screen (+ buf0 -> buf1 when 0x96380); 24 / 25 fade slot a0 in / out over w1 steps (0x52908); 26 0x5EA78 sprite
  define (a0 = sprite, src buffer w5: x w1, y w2, w w3, h w4, save place w6/w7 or -1); 27 centre flag; 28 clear the
  text rows (FillRect screen+0xBDFC 320 x 35); 29 0x5EBE8 quote a0 text at (w1, w2) (PlayQuote cat 0x40 + 0x3625C,
  only with subtitles 0x1163BD or a0 > 0x4C); 35 draw target buf1 (a0 = 0) / screen; 36 0x5E748 wait a0 ms;
  37 ArcSeek; 38 sub start (skip to 39), 39 return, 40 call record at byte a0 (one level: 0x337778); 41 PCX palette
  -> slot a0 (RGBX 0x400 each at 0x336758); 42 PCX -> buf0 (+copy to buf1 when 0x96380); 43 else; 44 / 53 palette
  rotate / 0x528DC + PaletteSet(0xEDA08, 0xA5, 0xB2); 46 memset buf0 + buf1 = a0; 47 clear screen; 48 0x96380 = a0;
  49 buf1 -> buf0; 50 PCX name (TEMPSTR = g_textBuf) -> buffer a0 at (w1, w2); 51 key colour 0x9637E; 52 buffer a0
  size (w1, w2); 54 / 55 open / close graphics archive (0x92F48); 56 / 57 sound archive (0x92F4C); 58 seek sound
  member; 59 0x5EDE0 FLC cue table (0xA5448: 50 x (9999, 0)); 60 load music; 61 SetMusic; 62 music volume;
  63 music stop; 64/65 voc; 66 sample rate; 67 LoadSfxSlot(name, a0); 68 PlaySfxSlot(a0, w1, w2, w3); 69 sample
  volume; 70 SpeechStream(rate a0, volume w1 when w1 != 0); 71 all sound stops.
- Buffers 0x93E80: 4 x (ptr, u16 w, u16 h); tac60: 0 = 320x200, 1 = 320x303, 2 = 320x160, 3 = 640x150. Sprites 0xED300,
  0x12 bytes: +0 src ptr, +4 w, +6 h, +8 src pitch, +0xA save ptr, +0xE / +0x10 save x, y.
- validate/diff/sab_diff.py: the original player in Unicorn on tac60 (stubs: files, timer, keys, palette (DAC model),
  sound, FLC); it plays LOS1 (2367 steps), LOS2 (1652) and WINGAME (5451) to the end; `--png DIR` = pictures (Jack at
  the helicopter, Brenda + Jack, HELILIFT / HELILEAV films stubbed). `--port` waits for `ja1front --sab`.

## Control Panel (scout 2026-10-26, asm read by a helper agent; '?' = guess)

Tactical part (sections 1-4) VERIFIED 2026-10-26 18:50: asm of 0x4B788 / 0x4BAA4 re-read (place loop, S/R rule,
search + Sound FX / Subtitles refusal, button loop runs after the switch search in every case, hover colours,
toggle, Music: SetMusic(dword 0x9635E sar 16 = the wanted theme)); live in DOSBox-X tac60: F10 -> ControlPanel on
the stack, two Music clicks: playing -1 then 1. Spec: panel.md "The Control Panel (key F10, `opt` button)".
0x92DC3 = a hidden save-anywhere byte (xor-toggled by a TacticalKeyHandler key at 0x139B3; QuickSave key at 0x138C6
checks it too). The office switch box (section 5) is verified (2026-10-26).

There are TWO Control Panel screens. They share the 16 switch bytes and the name table, but the code, the
pictures and the rules differ:
- **Tactical panel** (F10 or the `opt` button): text only, drawn straight on the screen over the tactical map.
  ControlPanel? 0x4B788 + ControlPanelInput? 0x4BAA4.
- **Office switch box** (office hot spot 6): a picture (switchbox.pcc) with lever switches.
  SwitchBoxScreen? 0x4D310 + input loop 0x4D114 + ToggleSwitch? 0x4CF10.
music.md "0x4BF8C / 0x4D075" = the two SetMusic calls (0x4BF8C in the tactical input, 0x4D075 in ToggleSwitch?).

## The 16 bytes
g_options 0x1163B8..0x1163C7 (u8 0/1), names at 0x9675F (12 B each): 0 Sound FX 0x1163B8, 1 Music 0x1163B9,
2 Fast Scroll 0x1163BA, 3 Alt Scroll 0x1163BB, 4 Shadows 0x1163BC (g_shadowsOn), 5 Subtitles 0x1163BD,
6 Item Info 0x1163BE, 7 Show Paths 0x1163BF, 8 Pal Cycling 0x1163C0, 9 Fast Anims 0x1163C1, 10 Follow move
0x1163C2, 11 Step Sounds 0x1163C3, 12 Day Replays 0x1163C4, 13 Fast Firing 0x1163C5, 14 Safety Move 0x1163C6
(g_confirmMove?), 15 Merc OKs 0x1163C7. Extra bytes 0x1163C8..0x1163CB (savegame "four further bytes").
Port world image: the block starts at offset 0x5422; world_load.md already maps opt_step_text 0x5427 (5),
opt_path_marks 0x5429 (7), opt_ambient 0x542A (8), opt_half_delay 0x542B (9), opt_follow 0x542C (10), opt_sound
0x542D (11), opt_confirm_move 0x5430 (14), opt_move_fx 0x5431 (15). The offsets agree with the order above.
Evidence: labels.csv 0x1163B8; peek 0x9675F; docs/spec/world_load.md lines 164-171.

## 1. Entry points and callers
1. ControlPanel? 0x4B788 (no args). Two callers, both in TacticalKeyHandler 0x13490:
   - key F10: extended key (GetKey 0 then scan 0x44) at 0x13563/0x13570 -> 0x137B9 CursorItemBack? 0x53660 (the
     cursor item goes back first), then call 0x137BE.
   - `opt` top-strip button: 0x67CD8(0x17, 1) (button table 0x9345A entry 0x17) true -> call 0x145E6.
2. SwitchBoxScreen? 0x4D310: one caller, OfficeScreen 0x4F43F (hot spot 6 "Control Panel", rect 0,21,24,70;
   0x468B0(0xD, ...) runs just before and after it ? = label/redraw of the hot spot).
3. ControlPanel? does nothing unless 0x92DCC ≠ 0 (? = player's turn / input allowed; NextTurn? clears it,
   InterruptTurn? sets it). Evidence: 0x4B791.

## 2. Tactical panel: drawing (0x4B788)
1. 0x92E4F = 1 while open (?). Quick-save allowed A = (0x92DCB = 0 (? enemies in sector)) or 0x92DC3 ≠ 0 or
   g_quickSaveInCombat 0x22A1F9 ≠ 0. Restore allowed R = FileExists?("quick.sav" 0x90C40).
2. 0x93DBE = 1; tactical redraw 0x1CB78(byte 0x92DD4). If 0x92DE0 = 0: 0x92DE0 = 1, 0x92DE2 = 1, 0x92DE5 = 0
   (? pause flag, put back on exit). 0x284F06 = 1, 0x229D48 = 800 (? both unknown). Rows N = 8 (0x28EDFC).
3. Hide mouse 0x67BF0; mouse limits 0x682AC(0x229CB8, 0x229CC4, 0x229CD8, 0x229CDC) (? = the map window rect).
4. Switch i = 0..15: x = 55, y = 25 + 13 i for i < 8; x = 170, y = 25 + 13 (i - 8) for i ≥ 8. Stored in the
   rect table 0x28E1A8 (x word) / 0x28E1AA (y word), 4 B per entry.
   Name: DrawText(name i, screen 0x95678, pitch 320, x, y, centre 0, fg 0xFF, shadow 8, font 1, mode 300).
   Value: text "OFF" (0x999CF + 0x48*26 = 0x9A11F) or "ON" (0x9A139) at (x + 75, y), fg 0x6F when off,
   0xFF when on, shadow = the value byte itself (0/1), font 1, mode 300.
5. Entry 16 = "Continue Game" (0x9A153) at (120, (N+1)*13 + 25 = 142), fg 0xFF shadow 8 (rect table 0x28E1E8).
6. Buttons: "Quick Save" (0x9641C) at (55, 165) only if A; "Quick Restore" (0x9643C) at (170, 165) only if R.
   Positions table 0x9624A/0x9624C. Same DrawText style (fg 0xFF, shadow 8).
7. Show mouse, then ControlPanelInput?(EAX = A, EDX = R).
Evidence: 0x4B788..0x4BA20, strrefs, peek 0x96248 / 0x9A11F.

## 3. Tactical panel: input loop (0x4BAA4), returns the chosen index
1. Each pass: MousePoll?(1). Hover h = first entry k = 0..16 with the mouse in (x, y-4, x+95, y+9). It is a
   valid target only when:
   - k = 1 (Music): a MIDI driver is installed (0x9639A ≠ 0);
   - k = 0 (Sound FX): a digital driver is installed (0x96399 ≠ 0 ?), and not (Sound FX on and Subtitles off);
   - k = 5 (Subtitles): not (Subtitles on and Sound FX off).
   (So at least one of Sound FX and Subtitles always stays on.) First match ends the search even if invalid.
2. If no switch matched: buttons j = 0, 1 with the mouse in (x, y-4, x+80, y+5); Save needs A, Restore needs R;
   hit -> h = 100 + j. No valid hit -> h = -1.
3. Hover change: the old item is drawn again in its normal colours, the new one with fg 0x16 (name shadow 8;
   value shadow = value byte; Continue Game shadow = byte 0x1163C8 (quirk ?); buttons shadow 8).
4. Left button (g_mouseButtons? 0x229CF4 bit 0) on h ≠ -1: 0x67C54 (? wait for release), PlaySfxSlot(0, 0x2B11,
   0x14, 1) click. h ≥ 16 (Continue / Quick Save / Quick Restore): done, return h. h < 16: toggle, see 4.
5. Escape (EscPressed? 0x67300): done, return -1. No other keys.
6. After return (in 0x4B788): if 0x92DE2 then 0x92DE0 = 0; 0x92DE5 = 0x92DE2 = 0; hide mouse; mouse limits
   0..319 x 0..199; 0x92E4F = 0. Result ≥ 100: 0x93DBE = 1, redraw 0x1CB78(0x92DD4); 100 -> QuickSave, 101 ->
   QuickRestore(1). 16 / -1: nothing else (the tactical loop redraws ?).
Evidence: 0x4BAC4..0x4BFE9 (asm /tmp/cpin.asm), 0x4BA25..0x4BA99.

## 4. What a toggle changes at once (both panels)
1. Show Paths (7): if it was on before the click, PathMarksOff? 0x1D8A4 (tactical only, 0x4BE93).
2. The byte flips (xor 1).
3. Tactical: the value area (x+75, y, 20 x 13) is put back from g_backBuffer 0x22A4EF (pitch 0x180, col x -
   [0x229CB8] + 0x6B, row y - [0x229CC4] + 0x1C), then "ON"/"OFF" in fg 0x16 (shadow = new value).
4. Music (1) turned on and wanted theme (word 0x96360) > 0: SetMusic(wanted, 0) — plays it again from the start.
   Music turned off and playing theme (word 0x9635E) > 0: StopMusic 0x5C82C.
5. Sound FX (0) turned on: the click sound plays again (now audible).
6. All other switches: nothing at once; the game reads the byte where it is used (e.g. Shadows -> DrawShadowPass,
   Subtitles -> Santino/guide balloons 0x5F4C4/0x5F5AC, Item Info -> panel.md, Safety Move -> MoveMercTo).
Evidence: 0x4BE93..0x4BFC0; ToggleSwitch? 0x4CF10..0x4D10F.

## 5. Office switch box (0x4D310)
VERIFIED 2026-10-26 (Unicorn: validate/diff/switchbox_diff.py runs 0x4D310 with 0x4D114 / 0x4CF10 / OfficeHighlight
(8, ..) / ScaledBlit / dissolve 0x52C98 for real; spec strategic.md "The switch box (Control Panel)"). Corrections to
the draft: OfficeHighlight(8, ..) = the box door, 50 x 74 at (0, 9), frames PANBRUSH (50 j, 0), off sound = main bank
effect 0; the room save 0x22A521 = g_backBuffer + 50 (the door's own save sits at + 0).
1. Pictures: panbrush.pcx (0x90C80) into the sheet 0x93E5C (pitch 640); switchbox.pcc (0x90C90) into
   g_panelBuf (pitch 320); room.pcx (0x90C74) for the palette (0xFA608 -> 0xEDA08 via 0x52820); at the end
   offbrush.pcx (0x90CA0) is loaded back into the sheet. OfficeHighlight(8, 1) before, (8, 0) after (? k = 8).
2. The room screen is saved (318 x 200 into 0x22A521, pitch 0x17C).
3. 20 switches i = 0..19 (also the 4 extra bytes; any value other than 0/1 is set to 0 first):
   position x = word 0x96142 + 4i, y = word 0x96144 + 4i: i = 0..9 at (28, 26 + 15 i), i = 10..19 at
   (137, 26 + 15 (i - 10)). Lever 26 x 7 from sheet (26, 74) when off, (0, 74) when on (tables 0x9611A,
   0x9612E/0x96130). For i < 16 also a lamp 6 x 4 at (x+10, y+7) from (52, 74) off / (58, 74) on (0x96122,
   0x96136/0x96138), and the name at (x+30, y+2), fg 0x44 off / 0x28 on, shadow 3, font 1, mode 300.
   Switches 16..19 have no lamp, no name and no click.
4. Open animation: the panel (282 x 200) zooms in 16 steps (ScaledBlit? 0x52B7C, w 21..276, h 31..196, top
   y 24 down by 4 to 0), then a full copy; palette set 0xEDA08; show 0x46BF4.
5. Input 0x4D114 (only on left button): switch k = 0..15 hit in (x, y, x+26, y+7):
   - k = 1 needs 0x9639A, k = 0 needs 0x96399 (as tactical).
   - k = 0 while Sound FX on and Subtitles off: ToggleSwitch(5) first (Subtitles go on), then Sound FX off.
   - k = 5 while Sound FX off: ignored. Else ToggleSwitch(k).
   (Different from the tactical panel, which refuses the Sound FX click.)
6. ToggleSwitch?(k) 0x4CF10: PlaySfxSlot(2, ...); flip; redraw lever + lamp + name (colours above); Music on/off
   and Sound FX on as in section 4 (sound slot 2 here); panel to screen 0x46BF4.
7. Close: one exit only, the lever at (244, 31) (0x96192/0x96194), hit rect 22 x 65: hide mouse, PlaySfxSlot(3),
   9 frames 22 x 130 from sheet (200 + 22 f, 0), ≥ 90 ms each (timer 0x22A3FC); then 0x67F90, DrawRoomScreen(0).
   No keys (no Escape) in the office box ?.
8. Back: room restored (0x52C98), OfficeHighlight(8, 0), DrawRoomScreen(0), highlight area restore
   (0x961AE, 0x960C6/0x960C8).
Evidence: decomp 04D310/04D114/04CF10, peek 0x96110..0x9619F, strrefs 0x4D310.

## 6. Front-end port today
- No Control Panel screen. port/ja1front/src/iteminfo.rs: constants OPT_INFO = true, OPT_SUB = false (env
  JA1_SUBTITLES=1); "the front-end has no Control Panel yet".
- World image already carries 8 of the 16 bytes as named opt_* fields (world.rs ~145-230, world_load.md
  section table); Sound FX, Music, Fast/Alt Scroll, Shadows, Item Info, Day Replays, Fast Firing are not in it.
- panel.md line 768: `opt` button = SPEC GAP (pressed face only); strategic.md line 708: office hot spot 6 =
  SPEC GAP; F10 not mentioned in the specs.

## Open points (?)
- 0x92DCC, 0x92DCB, 0x92DC3, 0x92DE0/2/5, 0x92E4F, 0x284F06, 0x229D48, 0x93DBE, 0x67C54, 0x682AC: names guessed.
- Whether 0x96399 = digital driver installed (only pattern: next to 0x9639A MIDI).
- Continue Game hover shadow from 0x1163C8 looks like an index-16 side effect.
- Which game code reads Fast Scroll, Alt Scroll, Day Replays, Fast Firing (not traced here).
