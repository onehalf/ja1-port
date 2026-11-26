# Live script mode: `ja1-port --serve` (#30)

`ja1-port JA1.gog JA.EXE --serve SAVE|menu SEED LOG OUT` runs the same play loop as `--script SAVE|menu SEED SCRIPT
OUT` (port/ja1front/src/main.rs `fn script`), but the script lines come from stdin in chunks. The test player
(validate/gate_campaign.py `Run` with `LIVE`, `WN_LIVE=1` in validate/week_normal.py) keeps one process per start
save and sends only the lines that are new since its last run, so a run no longer replays the day's lines again.

## Input

- Stdin carries script lines (the `--script` syntax, section 8 "Play script" of docs/spec). A line `sync` ends a
  chunk. The end of stdin ends the program (exit 0, no output).
- Each chunk is appended to LOG (the file is new at the start), so LOG always holds every line that ran.
- After each chunk the program parses LOG's whole text again. The commands that already ran must come out the same:
  a `key I` line takes the `inv` lines after it, so a `sync` between them is refused (see Errors).
- Every command of `--script` is accepted. A chunk may be empty (only `sync`): the output repeats.

## Where a sync is legal

A sync is legal where `--script` could end with these lines and give the same state as a longer script at that
point: after a command finished, outside a menu stage. Two places are refused:

- **Inside a menu stage** (the main menu at the start with `menu`, or the menu after Leave): the chunk ends before
  the menu gives an outcome (New Game, Continue). `--script` would end there; a longer script goes on in the menu.
- **After a `key I` line** whose `inv` lines come in the next chunk (the parse of the earlier commands changes).

Not a sync point but an end: when the play loop stops for good (Exit to DOS, Leave without a menu, a game end that
leaves the loop), the output ends with `\x01exit` and the program ends; later chunks are not read.

## Output at a sync

At each sync the program prints what `--script` prints at its end for all lines so far, in parts with marks
(each mark is a line of its own; `\x01` = byte 1):

1. stdout: the lines the run printed while it ran (the same as with `--script`, e.g. the office texts).
2. stdout `\x01day`, then the day lines (map, start, day morning, msgbox ...) that are new
   since the last sync. Each day line comes out once; the reader keeps the lines of the earlier syncs.
3. OUT gets the world image (as `--script` writes it at its end).
4. stderr `\x01sync`, then the lines of the open shots and walks closed as at a script end (docs/events.md), then
   `\x01end`. These close on a copy of the event log: the run goes on with them open, as a longer script does. So
   the lines between these two marks count for this sync only.
5. stdout `\x01report`, then the script report (`slot` lines ...), then `\x01end` (or `\x01exit`, see above).

The `--script` output for the same lines = all lines of part 1 so far + all day lines so far + the report. The
`--script` stderr = all stderr lines outside the sync marks so far + the lines inside the newest marks.

No pictures: the `JA1_*_PNG` outputs of `--script` are not written. The test player replays with `--script` when
a run asks for pictures. The save directory (`JA1_SAVE_DIR`) belongs to the process: the test player gives each
process its own directory and copies it to the run's directory after each sync.

## Errors

A refused sync or a failed command prints the error on stderr (`ja1-port: serve: ...`) and exits with code 1,
without the sync output. The test player then replays the run with `--script` (which shows the real error, if any)
and starts a new process for the next run.

## Proof

validate/serve_diff.py (in `./t check`): week_normal.py seeds with `WN_LIVE=1`; at every sync it replays the
same lines with `--script` from the same start save and compares stdout, stderr, the world image and every file
of the save directory byte for byte. The day saves are compared at each sync, and each next day starts from the
live day save, so a state that differs only later still shows at a later sync.
