# Validation

| Files | What they do |
| --- | --- |
| `diff/*_diff.py` | diff tests: original functions in Unicorn against the Python copies in `tools/` and, with `--port`, the Rust port (`diff/port_lib.py` loads the port libraries built in `port/*/target/release`) |
| `diff/livecast.py` | a no-op progress hook (the tests report progress through it) |
| `gate_diff_tested*.py` | coverage gates: 1 in 4 / 1 in 3 / 1 in 2 of the known game functions are diff-tested (they read `build/emu_cov/`, written by `./t check`) |
| `emu_smoke.py` | checks the Unicorn harness |
| `patch113b.sh` | builds the port's game folder `build/game113b` from `rom/`, with the optional 1.13b patch files of `patch113b/` over it |
| `patch115m.sh` | builds `build/game115m` (and `run115/C` for DOSBox-X) with the unofficial fan patch 1.15m over 1.13b, for the fan-fix diff tests; supply the patch files yourself in `third_party/ja115m/patch/JA115/` |
| `run.sh`, `dosbox.conf` | the original in DOSBox, from a scratch copy in `run/C` (`--fresh` makes it) |
| `run_x.sh`, `dosbox-x.conf`, `x*.sh`, `bridge/` | the original in DOSBox-X with its debugger in tmux, and the live memory bridge for `./t jactl` |
| `to_tactical.py`, `to_tactical.sh` | drive the original from the start to the tactical screen and save the state `tac60` |
| `states.tsv` | the save states the original project used (name, date, game state, JA.EXE hash) |

Most diff tests need a memory dump of the running original: `./t jactl dump NAME` writes `build/mem_NAME.bin`.
`tools/check.py` (the `NEEDS` table) lists which dump each test needs. Run everything with `./t check --allow-skip`.
