# Playing the port

## Start

Unpack the release archive, copy your GOG "Jagged Alliance" folder into its `game` folder, and start `ja1-port`
(`ja1-port.exe`). On Windows, a GOG install in the default place needs no copy. For the settings, start `setup.sh`
(`setup.bat` on Windows) first.

With no arguments the program:

1. finds the game: a folder that holds `JA1.gog` and `JA.EXE`, searched in this order (the first four with their
   subfolders, two levels down):
   - `JA1_GAME_DIR`
   - `game/` beside the program
   - the program's folder
   - the working folder
   - the program's parent folder
   - the GOG install folders: `C:\GOG Games`, the GOG Galaxy `Games` folder, `~/GOG Games`, `~/Games/Heroic`;
2. works in the program's folder: `ja1.cfg` and `saves/` are there;
3. plays the title films (setting Title films), then opens the main menu.

With no game files it shows a message that says what to do and lists the folders it searched.

Other ways to start:

```sh
ja1-port --setup                         # the setup screen (setup.sh / setup.bat do this)
ja1-port --menu                          # straight to the main menu (game files found as above)
ja1-port JA1.gog JA.EXE --menu [SAVE_DIR] # explicit paths; saves go to SAVE_DIR (or JA1_SAVE_DIR, or .)
ja1-port JA1.gog JA.EXE --setup          # the setup screen with explicit paths
ja1-port JA1.gog JA.EXE --play SAVE      # straight into a day save
```

On Windows a console window opens beside the game window: it shows the program's log lines. Keep it open; copy
its text into a bug report.

## Setup screen

Start it with `setup.sh` / `setup.bat` (or `ja1-port --setup`). Click a switch to change it (right click = the
value before), pull the red lever (or press Enter) to play, Esc quits. The settings go to `ja1.cfg`; an environment variable of the same option wins over the file.

| Switch | What it does |
| --- | --- |
| Grab mouse | on = the window takes the mouse as the original did in DOS (Ctrl+F10 gives it back; a click takes it again). off = the normal desktop pointer; the map still scrolls when the pointer goes past the window edge |
| Speed | the pointer speed with Grab mouse on (DOS = as the original) |
| Fullscreen, Size | full screen with black bars at the sides; window size 2x to 5x |
| Soft pixels, Scanlines | smooth scaling; dark lines as on an old monitor |
| Music, Mute away | music on/off; no sound while another window has the focus |
| Subtitles | always show the text of speech |
| Title films, Intro film | Sir-tech logo, title and credits at the start; the island film at a new game |
| Fixes | Off = the game as Sir-tech made it (1.13b, its bugs too). 1.15m = the bug fixes of the unofficial 1.15m fan patch by DarzaR, written as port code. All = 1.15m + the port's own fixes. List: `THIRD_PARTY.md` |
| A.I.M. grid | a mod: A.I.M. opens on a grid of all mercs (24 per page) with a sort and a filter. Off = the original A.I.M. pages |

The lever always starts the main menu.

## Fixes

The port plays version 1.13b with its bugs by default ([original_bugs.md](original_bugs.md)). The Fixes switch
(`fixes` in `ja1.cfg`) has three levels:

- **Off**: 1.13b as Sir-tech made it.
- **1.15m**: 70 fixes (F01..F70) that copy the unofficial 1.15m patch by DarzaR (bugs found by DarzaR and
  MitchShudlem). The port ships no file of that patch: each fix is port code, checked against the patch's exe.
- **All**: 1.15m plus 23 fixes of this project (P01..P23) for original bugs that 1.15m leaves.

Each fix has a row in plain words in `THIRD_PARTY.md` (beside `README.md`).

## A.I.M. grid (mod)

With the A.I.M. grid switch on, the A.I.M. page of the laptop opens on a grid of all mercs: 6 x 4 faces per page,
PREV / NEXT through the pages.

- **SORT**: number, name, salary, or a skill (health, agility, dexterity, wisdom, medical, explosives, mechanical,
  marksmanship, experience). The same sort again reverses the order.
- **FILTER**: all mercs, not away, or only mercs you can hire now. Each item shows its count.
- Each cell shows the face, name and the sorted value; TEAM (on your team), OUT (on assignment), the dead face, a
  darkened face (away) and $ (asks for more money) mark the merc's state.
- A click on a face opens the original merc page; PREV / NEXT there step through the grid's list, and a click on
  the A.I.M. title goes back to the grid.

The grid only changes how you browse: hiring, the interview, the prices and the saves are the original's.

## Saves

- The office calendar: top half = Save Game (`saves/GAME0..9.SAV`), bottom half = Restore Game.
- At each new day the game writes `saves/OFFICE.SAV` (the evening before) and `saves/DAY.SAV` (the morning).
- Quick Save / Quick Restore: Control Panel (F10) when the quick-save mode allows it, or Alt+S / Alt+R outside a
  fight.
- The save files have the original's format: saves of the original game load in the port, and port saves load in
  the original.

## Recordings

With `JA1_RECORD_DIR=DIR` the port keeps a record of the campaign in DIR: `events.log` (fights, kills, days, sectors
won and lost), a copy of each morning's day save (`day<N>.SAV`) and `meta.txt`. Recording changes nothing in the
game, and nothing leaves your computer.

## Keys

All keys and clicks per screen: [keys.md](keys.md). Most used in a sector:

- F1..F8 or a click on a face: choose a merc. Left click: walk (Safety Move on: click the tile twice).
- Right click: weapon cursor. Left click on an enemy = aim, right click = more aim, left click = fire.
- D = end turn, I = inventory, M = guard screen, T = traverse, F10 = Control Panel, P = pause, Z = look.
- PgUp / PgDn = map views, Insert = overview, Alt+X = quit.

## Compare with the original

The port keeps the original's behaviour, its bugs too ([original_bugs.md](original_bugs.md)). When something looks
wrong, try the same thing in the original under DOSBox or DOSBox-X: the CD image `JA1.gog` mounts as drive D:.
