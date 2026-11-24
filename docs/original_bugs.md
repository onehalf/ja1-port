# Bugs of the original game

The port keeps the behaviour of the original JA.EXE 1.13b, its bugs too. While porting, these bugs and quirks of the
original came to light. Each row says in plain words what goes wrong, where it is in JA.EXE, and how far the bug was
proved in play.

Proof:

- **live**: seen in the running original (DOSBox-X), with normal play.
- **forced**: seen in the original, with a state edited to reach it.
- **unreachable**: a data scan or code read shows the game data never reaches it.
- **port**: a port-only matter (the port differs on purpose, for example to avoid a crash or a memory write).
- **later**: not proved either way.

Counts: live 16, forced 20, unreachable 10, port 1, later 17 (64 rows).

A fix pass was planned after parity, behind an explicit switch. It did not happen; a fork is welcome to do it.

| # | In plain words | Where (JA.EXE) | Proof |
|---|---|---|---|
| 1 | A crowbar on a door with a very strong lock can give a wrong result: a number gets too big for its storage. | UseCrowbar 0x3B474 | unreachable |
| 2 | A crowbar with no strength against a certain kind of lock crashes the game. | UseCrowbar 0x3B474 | unreachable |
| 3 | Disarming a pile of more than 4 explosives on the ground can damage the next item on the ground. | DisarmExplosivePrompt 0x3B9D8 | later |
| 4 | When an event in the home sector kills the last enemies, it skips one enemy, who stays alive. | KillStragglers 0x74C44 | unreachable |
| 5 | Burying a mine can read past the end of a table, so the result depends on unrelated data. | BuryMine | later |
| 6 | An enemy with exactly 255 action points freezes the game when it thinks about a throw. | ThrowOption | unreachable |
| 7 | The door-lock lookup reads two entries too many; they belong to a different list. | SectorLockLookup | unreachable |
| 8 | When a merc looks along the top edge of the map, the game can overwrite part of the record of the last enemy in the sector. | MercView 0x1491C (roof opening) | unreachable |
| 9 | When a merc opens a door with a key from a key ring kept in a pocket (not the first key on the ring), the item in the merc's hand loses the condition of its fourth unit. | use key 0x3AE78 (called by PlayDoorFx? 0x3B03C, host op 70) | forced |
| 10 | In the inventory screen, a Ctrl-click on the small attachment box (or the ground box) can scramble a lot of that merc's data, such as where the merc stands and how it moves. | inventory loop 0x53A9C, item click with ctrl -> unstack 0x53580 (UseThrow? 0x535E8) | live |
| 11 | If your squad ends the day in a sector you own that is cut off from your home land, each merc has about a 1 in 100 chance to crash the game. | retreat MsgRetreatCasualty? 0x30B04 (from EndOfDayScreen 0x29F44) | forced |
| 12 | At the end of the day the game means to reset a status value for all eight mercs, but it only resets the first merc's. | EndOfDayScreen 0x29F44 tail 0x2A23B | later |
| 13 | When natives quit in the morning because there are not enough workers, the message about quitting tappers shows the wrong number (the number of guards who quit, or 0). | MercsQuitMsg? 0x50D2C (called by the morning worker-pool update 0x50BC4) | forced |
| 14 | When a merc trains in explosives, the game counts the practice by the merc's experience level instead of the explosives skill, so a skilled expert still trains as fast as a beginner. | morning duties 0x4B48C (training kind +0x199 = 2, branch 0x4B60B) | forced |
| 15 | A merc who starts the morning with less than 15 health gets free maximum health (up to 14 points), and a wounded merc can grow above 100 maximum health, because the morning stat update checks the current health instead of the maximum health. | morning stat changes ExperienceChangeMsg? 0x45BBC (stat 1 branch) | forced |
| 16 | On the island map, a click on the far right edge of a sector in the east column picks the first sector of the next row instead. | sector under the mouse 0x41410 (StrategicScreen 0x432F4) | live |
| 17 | When the game checks for the blight cure item in the item store without taking it, it does not see it if it is the last entry in the store's list (not yet checked in the running game). | FindSquadItem? 0x61528 probe → 0x51A24 (store search) | later |
| 18 | When two or more hired guards reach a sector at the same moment, the message names a wrong sector number. | DayClock 0x1F724 (labels: GuardsArriveMsg) at 0x1F9B9..0x1F9DD | unreachable |
| 19 | On the assignment screen, clicking the first item-store slot while the store list shows no items can put a junk item on the mouse or crash the game. | InventoryScreen? 0x4891C at 0x49E45..0x49E91 (store take, view 0) and 0x51A24 | live |
| 20 | On the assignment screen, the E key (put all items in the store) destroys an item when the store already holds 99 or more of that kind: the item leaves the merc but never reaches the store. | InventoryScreen? 0x4891C at 0x48A74 → 0x52150 (key E), AddStashItem? 0x51D50 | forced |
| 21 | On the assignment screen, a stat that went up this morning is shown in white, but once the mouse passes over its row the number turns back to its normal colour, so the player loses the mark. | stat row light 0x473BC (called from the hover part of InventoryScreen? 0x4891C, 0x4A0F0..0x4A2C3); the picture draws it in FUN_00047504 | forced |
| 22 | If you send guards to a sector while other guards are still walking there from an earlier send the same day, the earlier guards never arrive and the sector stays full of guards who do not exist until the day ends. | Guard screen close, GuardAssignScreen? 0x427B4 at 0x42FBC..0x430A8 | forced |
| 23 | When the enemy moves troops out of a sector during the day, it usually sends only one group, even when it has many soldiers to spare: it sends a second group only by chance, when two neighbouring sectors looked better one after the other. | enemy_move 0x3EDE8, loop test at 0x3F383 (store at 0x3F1BE..0x3F1CB) | later |
| 24 | After an explosion, some of the scorch and rubble marks the game meant to leave on walls and objects never appear. | ExplodeMarks? 0x33820 (called by LoadExplodeDat 0x33BD4, host op 336) | later |
| 25 | When an interrupt ends and no soldier is left waiting in the interrupt line, the game marks a spot on the map instead of a soldier, and the ground of one map square (row 18, column 46) can change. | ShowTurnBanner? 0x1054C at 0x10582 (after the interrupt queue 0x22A412 returns 0x40) | later |
| 26 | When you type a name for a saved game, the arrow keys, Home, End and Delete do not move the cursor: they put the signs < > [ ] or a strange sign into the name. | SaveSlotNameInput? 0x4DEF4 + SaveSlotKeyHandler? 0x4DC1C | live |
| 27 | When you type a name for a saved game, the Delete key can crash the game: the strange sign it puts into the name cannot be drawn. | SaveSlotNameInput? 0x4DEF4 (redraw of the name) -> DrawText 0x358D0, font 1 | live |
| 28 | At A.I.M., the warning that an interview leaves you too little money for the helicopter never shows. | AimScreen 0x204E0 (string 0x9771F) | later |
| 29 | A merc who bled during the day and then gets a night's healing can wake up unable to act in combat at all: every turn it has less than no action points. | OvernightHeal 0x4B3F0, MercMaxAP 0x56210, StartTurn? 0x11FB4 | later |
| 30 | When the squad crosses from sector 39 northward into sector 29, it lands in the sea at almost every place of the edge and starts to drown; only a crossing near the west corner reaches the shore. | Traverse 0x315DC, TraverseCheck 0x31344, merc entry in SetupSectorFromXra (port: ja1_traverse_check, ja1_merc_entry) | later |
| 31 | In the replay of the day on the island map, once a sector with mercs and enemies shows its enemy flag, every later sector with mercs also shows an enemy flag, even with no enemies there. | replay redraw 0x41B24 (DayReplay's replay_draw (1, 1); strategic.md "The replay pictures") | unreachable |
| 32 | In the replay of the day, a battle in the top left sector of the island map (sector 1) shows a second, shifted copy of that sector's markers (guards, trees, mercs) while the battle flashes, and the copy stays on the replay map. | ReplayBattle 0x6494C (replay_anim 2; strategic.md "The replay pictures") | forced |
| 33 | When you page through the four map views (normal, wide, full screen, overview) and come back to the normal view, the map has moved three tiles left and one row up. | SetViewMode 0x248E4 (panel.md "View modes") | live |
| 34 | In the item store, after you take out the only item the store holds, taking out another item later can make the items listed after it vanish from the store. | stash list (strategic.md "The item store": stash_put 0x51CB0, removal 0x519C8, LoadItemStash 0x51508) | live |
| 35 | When your first repaired processing plant starts to work, the guide's speech about the new plant capacity is lost: the game asks for a quote that does not exist, so the guide's face (or an empty caption box) appears with no voice and no words, and stays until you press a key or click. | NativeGuidePrompt? 0x5F94C step 2 (0x5FAC4..0x5FB34), CalcProcessCap 0x41574 | forced |
| 36 | Three of the demolition charges in the plant sectors 23 and 26 are broken: when the mission charges go off, each of these makes only a small puff of smoke, does no damage and leaves no crater. | Explode 0x3B79C step 1 (jump table 0x3B77C, kinds 2..9 only), Ground blast 0x355D0 (kind = s8 E[1] + 1) | forced |
| 37 | When a certain A.I.M. merc leaves the team with a strobe or a plastic bag in one particular pocket, later a merc who uses an item on a spot with nothing on the ground can call the enemies to that spot (strobe) or open and close doors (plastic bag), as if that item lay there. | UseOnTile? 0x3CFD0 at 0x3D3E1 (no g < 0xFF test before the type read 0x3D3F0) | forced |
| 38 | On the assignment screen, a click on an empty item store (in the list view) freezes the game or puts a broken item in your hand. | InventoryScreen? 0x4891C store place press (view 0x93EA4 = 0) + take 0x51A24 | live |
| 39 | On the assignment screen, an item can vanish from the item store after the store was empty and new items were put in, when you take out an item from the middle of the list. | take TakeStashItem? 0x51A24 (unlink at 0x51B37), insert MsgNoRoomForItems? 0x51CB0 / InsertStashRecord? 0x51C44, tail 0x229C60, head 0x229C78 | live |
| 40 | In a fight, the first press of the louder-music key or the louder-sound key makes it quieter: the game starts at full volume, but its own volume number starts lower (music 100 of 127, sounds 70 of 127). | TacticalKeyHandler 0x13490 cases + 0x13D0A / . 0x13D2E; InitAil 0x5C25E / 0x5C351 | live |
| 41 | Quick Restore brings back the saved music and sound volume numbers, but the game keeps playing at the old loudness until you press a volume key. | QuickRestore 0x2D688 / 0x2D6BC | live |
| 42 | After the last enemy falls, the port gives no victory quote to mark the sector as clear. | FUN_0003A0B0 (sector victory) -> ShowMercMessage? 0x299B8 | port |
| 43 | The thin frame around a merc's health bars can stay on screen after the mouse moves quickly from the bars onto the map. | TacticalKeyHandler 0x14225 | later |
| 44 | With key L (say it again), a quote where a merc's name comes late in the text loses letters around the name or shows old leftover text. | StrReplace 0x252B0 (all callers; seen through SpecialNpcSpeech? 0x29D54) | later |
| 45 | With key L, a quote that names a merc when no merc is on record shows leftover text from an earlier message in place of the name. | SpecialNpcSpeech? 0x29D54 at 0x29E5x | unreachable |
| 46 | On the A.I.M. screen, a thin line above the merc's name shows a few wrong pixels, taken from the bottom of the screen. | ShowAimScreen 0x1FEB0 + AimDrawTextPanel 0x215BC | live |
| 47 | If you open a merc's inventory, switch to another merc there and change that merc's hand item, the second merc keeps its old reserved action points until something else recalculates them. | InvScreenRun? 0x54D50 | live |
| 48 | If the game has no merc on duty when it sets up a sector, it uses a wrong starting spot for mercs that are placed there. | PlaceMercsAtEntry 0x2B470 at 0x2B4C8 | later |
| 49 | If your last merc on duty dies at the same moment as the last enemy, the game looks for a speaker that does not exist and reads map data as a merc. | SectorVictory? 0x3A0B0 at 0x3A111 | later |
| 50 | When a merc on A.I.M. speaks the name of a merc it holds a grudge against, some long name samples are too big for their memory space and damage the list of sound effect channels, and the game then freezes when the quote box closes; Biff and Moses have such samples. | LoadVocSample 0x5C960, PlayMercVoice 0x24D14 | forced |
| 51 | If the merc with profile 29 (Fox) holds a grudge against merc 40 (Lance), its speech says the name of Needle instead, because Lance's name sample is missing from the disc. | PlayMercVoice 0x24D14, ArcSeek 0x534A4 | forced |
| 52 | Holding an item with the mouse at the very top of the screen makes the game write a few item pixels into computer memory outside the screen; nothing visible goes wrong in DOSBox. | DrawMousePointer? 0x67A28 | live |
| 53 | A line that has no recording makes the game play a piece of another recording, and then the last merc name it loaded, where you would expect silence. | PlayQuote 0x24F08, SpeechWait? 0x28268 | forced |
| 54 | After you send guards from the guard screen, the island map shows extra guard marks on the target sector until you next open the guard screen. | DrawSectorMarkers? 0x419BC (read at 0x41AE2), GuardAssignScreen? 0x4283B | unreachable |
| 55 | With Subtitles on, a long remark from the inventory ("No Room", "Empty") stops in the middle while its text box shows, and the end of the word plays only after the box goes. | PlayInvExtra? 0x25AD8, SpeechStream 0x5CC6C, ShowMessage? 0x27B98 | live |
| 56 | A battle sound on the map box stops after about 3 seconds, even when the recording is longer. | ResolveSectorBattle 0x3E508, SpeechRefill? 0x5CCF8 | forced |
| 57 | When a merc breaks in on the enemies' turn or a new fight turn starts, the mouse pointer jumps to wherever it was when the last box or view slide opened, even to the top-left corner right after the game starts. | NewTurn? 0x126E8, InterruptTurn? 0x13150, MouseRestorePos2? 0x67E30 | forced |
| 58 | Closing a picture item's box can in rare cases throw the mouse pointer to an old place. | ItemInfoBox 0x26BB0 | later |
| 59 | If a guide, Brenda or Santino quote plays while you hold an item, the pointer turns into the plain arrow; you still hold the item, and the picture comes back only when you move an item. | SantinoSpeech? 0x5F519, GuideSpeech? 0x5F611, BrendaSpeech? 0x5F7C7 | forced |
| 60 | If a merc is thrown back by a blast against a wall and you press Escape while he picks a way to fall, the next blast that throws him back can crash the game. | AnimFrame 0x18498 words 0x1E5/0x1E6, AnimNext 0x19470, SetAnim 0x17F90 | later |
| 61 | With voices off, the second half of a two-part guide or Brenda message stays on screen as long as the first half needs, not as long as its own text needs. | SpeechBalloon? 0x29544 | later |
| 62 | On a day when your plant capacity goes up while your home plant does not work yet (for example you take a plant sector), the guide's end-of-day remark about the new capacity is lost: his face appears for less than a second with a short piece of another voice line and no words, then he goes on by himself. | DayEndVerdict 0x60680 step 13 (strategic.md "The day's verdict" 13) | forced |
| 63 | If you end your turn in the short moment after an enemy wins an interrupt but before it acts, that enemy takes its shot and then the enemies' turn freezes: nobody moves and your turn never comes back. | EndTurn? 0x121D8 step 9 + FireNow 0x57B34 + NextTurn? 0x12D58 | forced |
| 64 | If a day ends while you hold an item with the mouse, the item lands in the selected merc's main hand and destroys what was there. | CursorItemBack? 0x53660 from TacticalLoop 0x11370 | live |
