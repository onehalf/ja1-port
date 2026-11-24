# Continue the work

This project is no longer in active development. These notes say how it was built, so a fork can go on in the
same way.

## How the port was written

The target is the 1.13b game, "faithful-ish": the original's behaviour exactly, the internal structure free.

- Understand an original function before you port it: its callers, the state it reads and writes, its side
  effects. The evidence is the disassembly and the Ghidra decompiler output; check the decompiler against the asm,
  because it is often wrong (argument lists, junk CONCATs).
- Keep the original's game behaviour, its bugs too. Do not fix a bug silently and do not add new differences. A
  bug goes into [docs/original_bugs.md](docs/original_bugs.md). A fix belongs behind an explicit switch.
- DOS, mouse-driver and other platform code may be replaced or left out, after a check that it has no purpose a
  player sees.
- Port code stays platform agnostic: no `/tmp`, `/proc` or `/dev` paths, no `std::os::unix`, no `libc`, no
  shell-outs. Build paths with `Path::join`; temp files go under `std::env::temp_dir()`.
- A name that says what a value means in the game (a trait, a posture, a mood) needs proof: game text, the game's
  own debug page names (`docs/spec/cursor.md`), or a look at the running original. Without proof, use a neutral
  name with a `?`.
- Cite the original's addresses in comments and in `docs/spec/`: `MoveMercTo 0x1C608`. Address notation = linear
  addresses at the LE preferred bases, identical to Ghidra on `build/JA.elf`: code 0x10000..0x8AC7D, data
  0x90000..0x33A730. File offsets are written `F:0x...`.

## Setup for the RE tools

```sh
# your game files: the GOG installer unpacked into rom/ (rom/JA1.gog, rom/JA.EXE, rom/DAT/ ...)
innoextract -e setup_jagged_alliance_*.exe -d rom      # then delete rom/DOSBOX rom/__redist rom/app rom/commonappdata
# optional: the official 1.13b patch files in patch113b/ (JA.EXE, DAT/XRA.DAT, DAT/QUESTS.DAT)
python3 -m venv .venv && .venv/bin/pip install capstone unicorn   # Python 3.11+; also install bsdtar (libarchive)
validate/patch113b.sh                                   # builds the port's game folder build/game113b
./t le                                                  # LE header, objects, fixups of JA.EXE
```

`./t <name>` runs `tools/<name>.py` (in `.venv` when present). The main tools:

| Command | What it does |
| --- | --- |
| `./t le`, `./t le2elf` | read the DOS/4GW LE executable; write `build/JA.elf` for Ghidra (objects at their bases, `labels.csv` as symbols) |
| `./t ghidra` | headless Ghidra project with Watcom register calls, the types of `defs/` and the names of `labels.csv`; pseudocode into `analysis/decomp/` (needs Ghidra 12 and JDK 21; `GHIDRA_HOME`, default `/opt/ghidra`) |
| `./t disas ADDR [n]`, `./t xref ADDR`, `./t peek ADDR` | disassembly, cross references, hex dump of the loaded image |
| `./t emu ADDR [args]` | run one original function in Unicorn |
| `./t strings_dump`, `./t strrefs` | strings of the binary; the strings each function references |
| `./t speech`, `./t datarc`, `./t csprite`, `./t sectormap` | quote text and voice, Sir-tech archives, compiled sprite banks, tactical maps |
| `./t savefile SAVE`, `./t quests SAVE` | parse a day or quick save; quest status from the 43 story bytes |
| `./t jactl ...` | live bridge to the original running in DOSBox-X: read memory, save states, clicks, memory dumps |
| `./t status`, `./t sweep` | progress numbers; game functions without a diff test, sorted by risk |

`defs/types.toml` is the single source of truth for struct layouts and typed globals; `defs/functions.toml` holds
function signatures; `labels.csv` holds names (`?` at the end = tentative). Run `./t ghidra` after a change.

## Tests

- `cargo test --release` in `port/ja1logic` and `port/ja1front`: the port's own tests. The ones that read game files
  run only when `rom/` holds them.
- `validate/diff/*_diff.py`: diff tests. Each runs original functions in Unicorn and compares them with the Python
  copy in `tools/` and, with `--port`, with the Rust port (`validate/diff/port_lib.py` loads the port libraries).
- Most diff tests need memory dumps of the running original (`build/mem_tac60.bin` and others). Make them with the
  DOSBox-X kit: `validate/run.sh --fresh` (scratch copy of the game in `validate/run/C`), `validate/run_x.sh`
  (DOSBox-X with the live bridge; needs `dosbox-x`, `tmux`, `xdotool`, a C compiler), `validate/to_tactical.py`
  (drives the original to the tactical screen and saves the state), then `./t jactl dump NAME`.
  `validate/states.tsv` lists the states the original project used. Some states came from played sessions; you
  have to make your own.
- `./t check [--quick] [--allow-skip]` runs the label and type checks, both crates' builds and tests, and every diff
  test; `--allow-skip` counts a test with a missing dump as SKIP.

The development history (about 1,300 commits, the tracker and the review notes) stays private; numbers such as
`#24` in comments point to it. Comments and specs also name test suites that are not in this repository:
`smoke_play` (scripted play of the window), `gate_*` (whole-game runs against references), `quest_scenarios`, the
campaign bot and the live checks (`live_*`). They depended on the original project's machine, saves and states.
