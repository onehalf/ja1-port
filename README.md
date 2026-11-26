# ja1-port — a Rust port of Jagged Alliance (1994)

A reimplementation of Sir-tech / Madlab's **Jagged Alliance** (DOS, 1994) in Rust, built by reverse engineering
the original `JA.EXE`. The target is the behaviour of version 1.13b (the GOG release plus the official 1.13b patch),
the original's bugs included. It runs on Linux and Windows with SDL2.

On top of the faithful port, three things are optional and off by default:

- **A setup screen** for the play settings: mouse, window size, fullscreen, scanlines, sound, subtitles, films.
- **Fixes**: the 70 bug fixes of the unofficial 1.15m fan patch, rewritten as port code, plus 23 fixes of our own.
- **The A.I.M. grid**, the port's first mod: all mercs on one page, with a sort and a filter.

**You need your own copy of the game.** This repository holds no game files. The port reads the data from your
GOG install (the CD image `JA1.gog` and `JA.EXE`).

**Status: finished as far as this project goes; no longer in active development.** Issues and pull requests may
get no answer. Fork it and take it further: that is the point of publishing it.

## How complete is it

- The whole game runs: main menu, office, A.I.M., the island map, sector fights, the day loop, saves, the endings.
  A scripted campaign bot (from an early-game save, with cheats) played to the win ending.
- 941 of the 1,030 game functions in `JA.EXE` (91.4 %; 96.9 % of the game code bytes) are diff-tested: the original
  function runs in an emulator (Unicorn) and its results are compared with the port's on generated inputs.
- No person played a full campaign on the port from start to end. Expect bugs, and compare with the original in
  DOSBox when something looks wrong.
- Bugs of the original game that the port keeps on purpose: [docs/original_bugs.md](docs/original_bugs.md).

## Play

1. Download the archive for your system from the Releases page (Windows `.zip`, Linux `.tar.gz`) and unpack it.
2. Give it your GOG copy of Jagged Alliance:
   - **Windows, game installed by GOG in the default place** (`C:\GOG Games\Jagged Alliance` or GOG Galaxy):
     nothing to do. The port finds it.
   - **Otherwise**: copy your GOG "Jagged Alliance" folder (the one with `JA1.gog` and `JA.EXE`) into the `game`
     folder of the unpacked archive. A subfolder is fine: `game/Jagged Alliance/JA1.gog` works.
   - **Linux with only the GOG installer** (`setup_jagged_alliance_*.exe`): put it into `game/` and run
     `./install-from-gog.sh` (needs `innoextract`: `sudo apt install innoextract`).
3. Start `ja1-port.exe` (Windows) or `ja1-port` (Linux). The title films play, then the main menu opens.

Saves go to `saves/` beside the program, so unpack the archive into a folder you can write to (not
`Program Files`).

Where the port looks for the game: [docs/playing.md](docs/playing.md). Keys and mouse per screen:
[docs/keys.md](docs/keys.md).

## Setup screen

Start `setup.bat` (Windows) or `setup.sh` (Linux), or `ja1-port --setup`. The setup screen is a switch box in the
style of the game's office: click a switch to change it (right click = back), pull the red lever (or press Enter)
to play, Esc quits. The settings go to `ja1.cfg` beside the program ([ja1.cfg.example](ja1.cfg.example) lists
them); you can also edit that file by hand.

| Switch | What it does |
| --- | --- |
| Grab mouse, Speed | take the mouse as the original did in DOS (Ctrl+F10 frees it), and its speed; off = the desktop pointer, the map still scrolls at the window edge |
| Fullscreen, Size | full screen with black bars, or a window at 2x to 5x (640 x 400 to 1600 x 1000) |
| Soft pixels, Scanlines | smooth scaling; dark lines as on an old monitor |
| Music, Mute away | music on or off; no sound while another window has the focus |
| Subtitles | always show the text of speech |
| Title films, Intro film | Sir-tech logo, title and credits at the start; the island film at a new game |
| Fixes | Off, 1.15m or All (next section) |
| A.I.M. grid | the A.I.M. mod (below) |

## Fixes

By default the port plays 1.13b exactly, bugs included ([docs/original_bugs.md](docs/original_bugs.md)). The
**Fixes** switch (`fixes` in `ja1.cfg`) has three levels:

- **Off**: the game as Sir-tech made it.
- **1.15m**: 70 fixes (F01..F70) that copy the unofficial **Jagged Alliance 1.15m** patch by **DarzaR**, with bugs
  found by DarzaR and **MitchShudlem** ([GOG forum thread](https://www.gog.com/forum/jagged_alliance_series/ja1_unofficial_115_patch)).
  Examples: bandages help a wounded merc instead of hurting him, timed explosives work in compressed time, unseen
  enemies no longer block or show through a move order, and several lock-ups in the enemies' turn are gone.
- **All**: 1.15m plus 23 fixes of this project (P01..P23) for bugs that 1.15m leaves, found in the reverse
  engineering. Examples: a Ctrl-click in the inventory screen no longer scrambles a merc, the save name line no
  longer crashes on arrow keys, the End-turn key can no longer freeze the enemies' turn.

The port ships no file of the 1.15m patch. Each fix is our own code, written from the patch's changelog and checked
against its exe in a diff test (or by a code read). Each port fix is checked against the original with the same fix
as a small byte patch. [THIRD_PARTY.md](THIRD_PARTY.md) lists every fix in plain words, with its credit.

## A.I.M. grid (mod)

With the **A.I.M. grid** switch on, the A.I.M. page of the laptop opens on a grid of all mercs instead of one merc
at a time: 24 faces per page, a **SORT** menu (number, name, salary or any skill; pick it again to reverse) and a
**FILTER** menu (all, not away, can hire now, each with its count). Cells mark mercs on the team, on assignment,
dead, away, or asking for more money. A click on a face opens the original merc page; PREV / NEXT there follow the
grid's order, and the A.I.M. title goes back to the grid. Hiring, prices and saves stay the original's. Details:
[docs/playing.md](docs/playing.md#aim-grid-mod).

## Optional: the official 1.13b patch

The GOG release is version 1.13. The 1.13b patch (Sir-tech, September 1995)
changes `JA.EXE`, `DAT/XRA.DAT` and `DAT/QUESTS.DAT`. The port reads game-folder files before the CD image, as the
original does, so if you have those three patch files, put them into the game folder. Without them the port plays
with the 1.13 data. The patch is not included here.

## Build

You need Rust (stable, [rustup.rs](https://rustup.rs)) and the SDL2 development files.

```sh
# Debian/Ubuntu: sudo apt install libsdl2-dev     Fedora: sudo dnf install SDL2-devel
# macOS (untested): brew install sdl2
cd port/ja1front
cargo build --release
./target/release/ja1-port /path/to/JA1.gog /path/to/JA.EXE --menu
```

`cargo build --release --features sdl2/bundled,sdl2/static-link` builds SDL2 from source and links it into the
program (needs CMake and a C compiler); the release builds use it. The crate is `ja1front`, the program it builds
is `ja1-port`. `scripts/build-release.sh linux|windows` makes the release archives (Windows as a cross build). `cargo test` runs the port's tests; the tests
that read game files run only when `rom/` (below) holds them.

Command-line modes (`ja1-port` with a bad argument lists them): `--menu`, `--setup`, `--play SAVE`, and modes
without a window for tests (`--script`, `--serve`, `--world`, `--enter`, `--png`):
[port/ja1front/README.md](port/ja1front/README.md), [docs/serve.md](docs/serve.md).

## Inside

| Path | What it holds |
| --- | --- |
| [`port/ja1logic/`](port/ja1logic/README.md) | the game logic: Rust library with a C interface (`include/ja1logic.h`); `src/fanfix.rs` holds the fix switches |
| [`port/ja1front/`](port/ja1front/README.md) | the front end: window, screens, input, sound, music, file reads from the CD image; `src/setup.rs` + `src/config.rs` the setup screen and `ja1.cfg`, `src/aimgrid.rs` the A.I.M. grid |
| [`docs/spec/`](docs/spec/README.md) | how the original works, per system (combat, AI, strategic day, panels, saves ...), with `JA.EXE` addresses |
| [`REVERSE.md`](REVERSE.md) | the reverse-engineering notes: binary layout, memory map, findings per game system |
| [`labels.csv`](labels.csv), [`defs/`](defs) | names and comments for `JA.EXE` addresses; struct layouts, typed globals, function signatures |
| [`tools/`](tools), `t` | the RE tools: LE loader, disassembler, Ghidra project builder, Unicorn emulator, live bridge to DOSBox-X, and Python copies of original routines for the diff tests (not the port) |
| [`validate/`](validate/README.md) | the diff tests (original in Unicorn vs Python copy vs Rust port; `--fan` against the 1.15m exe, `--pfix` against the original with a port fix patched in) and the DOSBox-X scripts that make their memory dumps |
| [`docs/`](docs) | [original bugs](docs/original_bugs.md), [keys](docs/keys.md), [playing](docs/playing.md), [story bytes](docs/story_map.md), [fight event log](docs/events.md) |

## Continue the work

Read [CONTRIBUTING.md](CONTRIBUTING.md): how the port was written (keep the original's behaviour and bugs, cite
addresses), how to set up the RE tools, and how the diff tests work. Numbers such as `#24` or "issue #2" in comments
point to the private development tracker; the facts they carry are in the comment or in `docs/`.

## Credits

Most of the reverse engineering, code and tests were written by AI coding agents:

- **Claude** (Anthropic, via Claude Code): reverse engineering, the Rust port, the diff tests, the play bot.
- **Codex** (OpenAI): scoping, code review, bug fixes.
- **DeepSeek** (run locally): agent runs on the local loop.

The 1.15m fixes copy the work of **DarzaR** (author of the unofficial 1.15 patch) and **MitchShudlem** (bug finder
for 1.15m): see [THIRD_PARTY.md](THIRD_PARTY.md).

## Legal

Jagged Alliance is a trademark of its owner (THQ Nordic). This project is not affiliated with or endorsed by the
owner or by Sir-tech / Madlab. It ships no game files: no art, sound, music, maps or speech. The source holds
some short interface messages and small rule tables that the port needs and that it does not read from `JA.EXE`.
Everything else comes from the files of a copy you own.

The code is licensed under the [GNU General Public License v3.0](LICENSE). Third-party parts: [THIRD_PARTY.md](THIRD_PARTY.md).
