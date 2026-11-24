# Merc personality (soldier byte +0x19C)

Soldier byte `+0x19C` is a per-merc **personality** value. InitSoldierFromProfile? 0x21B74 copies it from profile
byte `+0x2F` (rom/EDT/NEWPROF.EDT, 90-byte records). InitGuard? 0x2B7DC and InitTapper 0x2BB90 set 7; EnemyWalkIn?
0x306BC sets 0. Every value has one or more rules in the code and proof in the game's own text.

Proof sources (read 2026-11-06):
- **Bio** = rom/EDT/BIOQUOTE.EDT, 60 records of 320 bytes, record = profile id, bytes - 1, 0-terminated (the A.I.M.
  file text). Read: `python3 -c` over `d[p*320:(p+1)*320]`, each byte - 1.
- **DAY n** = merc message n (DAYQUOTE.EDT, `./t speech text DAY | awk '$3==n'`).
- **PST n** = PSTQUOTE.EDT quote n (`./t speech text PST`).

Code users found by `objdump -d build/JA.elf | grep '0x19c\]'` (soldier records only; 0x366B9 and 0x717F8 hits are
stack slots) and `grep field_0x2f analysis/decomp/*.c` (profile byte).

| Value | Name | Mercs (profile +0x2F) | Rules in the code | Proof |
|---|---|---|---|---|
| 0 | none | the other 32 mercs; walk-in enemies (0x306BC) | none | - |
| 1 | **informer** | Gary, Beth, Bud, Ears, Hurl, Weasel, Lance, Carp | ComplainPick? 0x45010 picks only a value-1 merc to tell you the other mercs' A.I.M. rate complaints (PST 7 intro, PST 8..11 "MEMBER is thinking about leaving..."). PayMercs? 0x44740 skips values 1 and 7: they never run off with the team's cash. | Bios of Gary, Beth, Hurl and Carp say the merc passes on what the team members think. PST 7 (a confidential intro line) has text only for these mercs. |
| 2 | **forgetful** | Larry, Moses, Bernie, Pops, Kaboom | StepStart 0x1A46C (step 7): the merc can stop a walk and say DAY 13; in combat the stop costs Random(AP) + 1 AP. PlaceMercOnTile 0x2B380: AI attitude = Random(5) (like 6). | DAY 13 (the merc forgets the order or the walk target). Bios of Moses and Kaboom mention a bad memory. |
| 3 | **relentless** (strategy guide trait **OverEnth**) | Screw, Reuban, Fidel, Leech, Skitz | MoveMercTo 0x1C608 and UseItemAt 0x1D9B8: while the last target (+0xCE) is alive (and +0x1D5 > 0, loaded gun, for the move) the merc refuses the new order with DAY 8. TakeDamage 0x392FC keeps the last target (+0xCE) when hit. StartAttack? 0x5ABE0: no friendly-fire DAY 8 from it. StepNoise 0x19A6C: quietness - 10 (louder steps; enemies with attitude 5 too). PlaceMercOnTile: attitude = 5. Profile +0x2F = 3: never picked by CorpseGrudge? 0x46548 or FireGrudge 0x50B50; AimScreen 0x2011C answer 8 hires it anyway; DayEndMercUpdate? 0x43F54: the night kill of the hated merc (Hate and the night, below). | DAY 8 for these mercs = a refusal: the merc wants to finish off the current target (for other mercs DAY 8 is the friendly-fire line). Bios of Screw and Leech describe a merc who does not give up. |
| 4 | **nervous alone** | Speck | ActiveFrame 0x1B9D4: when the nearest merc is more than 5 tiles away and flag 0x22A0FF (`alone_warned`) is clear: DAY 4 once. PlaceMercOnTile: attitude = 0. | DAY 4 (Speck): he feels exposed and asks for back-up. His bio calls him nervous. |
| 5 | **cannot swim** | Hitman, Turtle, Sparky, It | Drown? 0x3843C: drowning damage x2. FatigueScale? 0x32AD8: fatigue x5 when swimming. | DAY 10 of these mercs: the merc cannot swim and goes under. Bios of Sparky, Hitman and It say so too. |
| 6 | **poor shot** | Needle, Eli, Wally | StatPractice 0x62A68: marksmanship gain chance x 1/5. PlaceMercOnTile: attitude = Random(5). | Bios of Eli, Needle and Wally say the merc's marksmanship does not improve. |
| 7 | **native** | none (code only): guards 0x2B7DC, tappers 0x2BB90 | PayMercs? skips it. FatigueScale? divides the factor by 5. | Set only for the island's natives (guards and tappers). |
| 8 | **quits after a kill** | Biff | RaiseReason 0x44634 returns 3 when the merc has a kill (+0x1D9 != 0): the morning prompt has him leave with PST 3. RaiseSalaryPrompt? 0x448A8: when he leaves with a kill, his wisdom becomes 0. PlaceMercOnTile: attitude = 0. | PST 3 (Biff): after a kill he quits A.I.M. His bio calls him reluctant and nervous. |

Notes:
- PlaceMercOnTile 0x2B380 uses the value only when medical <= marksmanship (orders = 4); the attitude rules above are for that case.
- RaiseReason reason 3 is also the bad-day reason; the quote shown is PST 3 for every merc.
- The names are player words for the proved behaviour. A new rule found later goes into the row of its value.
- `defs/types.toml` keeps the field name `b19C` (Ghidra, tools and the port use it); its comment points here.

## OverEnth = relentless (the strategy guide's name)

The 1995 strategy guide (Chart 1 legend) names value 3 **OverEnth**: in short, the merc will not move until his
target is dead, and he may threaten or try to kill a team mate at night. Chart 1 gives OverEnth to the same five
mercs as profile `+0x2F` = 3 (Fidel, Screw, Leech, Skitz, Reuban). The first sentence is the MoveMercTo / UseItemAt rule of
row 3 (DAY 8). The second sentence is the night rule below. the maintainer saw the first rule in the port with Fidel
(issue #26, comment 11028).

## Hate and the night

Profile bytes `hates` `+0x3A` and `toHate` `+0x3C` (99 = none). Initial values (rom/EDT/NEWPROF.EDT): Beth, Fox,
Wolf hate Pops; Lynx hates Smoke; Vicki hates Lance; Rusty hates Weasel; Stella hates Hurl; Wally hates Fox; Skitz
hates Sparky. Screw will hate Bob, Reuban Ivan, Fidel Hurl, Leech Skitz. LoadGame and FixNewProfiles? 0x503F8
give Stella (41) `hates` = Hurl when it is 99.

1. Night (DayEndMercUpdate? 0x43F54, strategic.md Strategic screen opening step 4b), each team merc in random
   order: when `hates` = 99 and the `toHate` merc is in the team: `hates` = `toHate`.
2. Morning (HatedCheck? 0x44C10, strategic.md A disliked merc in the team): when the `hates` merc is in the team:
   `toHate` = `hates` and the merc says PST 6, an ultimatum (the hated merc leaves by morning, or he does). The
   OverEnth mercs (Screw, Reuban, Fidel, Leech, Skitz) threaten to kill the hated merc instead.
3. Next night: when `hates` = `toHate` and that merc is still in the team: an OverEnth merc (profile `+0x2F` = 3)
   kills that merc: the hated merc leaves the team with `hpMax` 0 and counts as a merc death (accounts +0x00). Any other
   merc leaves the team instead (accounts +0xA0, resignations). In the guide's Hates column a "*" after the name marks a merc who kills. Both show the
   status line "<nickname> seems to have disappeared!" after guide speech 0x98.
4. Merc leaves 0x50618: when `toHate` ≠ 99 and `toHate` = `hates`: `hates` = 99 (the ultimatum is forgotten; on a
   new hire step 1 starts again).
5. A.I.M.: MercRefusalReason 0x219CC answers refusal 8 while the `toHate` merc is in the team (the guide's "To
   Hate" column says the same).

The guide chart's Hates / To Hate columns do not match the NEWPROF bytes for every merc (the chart puts Fidel's Hurl
under Hates and Skitz's Sparky under To Hate); the game uses the bytes. Diff test: stratopen_diff (fuzzes `+0x2F`,
`+0x3A`, `+0x3C`; mirror and --port), complain_diff (HatedCheck?). Asm check 2026-11-17: 0x4402A
`cmp byte [ebx+0x2f], 3` picks the kill branch; 0x4408B..0x44099 g_mercDeathsTotal + 1.
