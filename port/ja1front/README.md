# ja1front

Front-end (window, screens, input) of the JA1 port. **How to port: see CONTRIBUTING.md "How the port was written" (the maintainer 2026-10-31,
issue #37).**

## Build and run

    ~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test
    ./target/release/ja1front ../../rom/JA1.gog ../../build/game113b/JA.EXE --menu [SAVE_DIR]   # the game

`--menu` opens the window at the main menu (world_load.md section 12); day saves go to SAVE_DIR (or `JA1_SAVE_DIR`).
`build/game113b/` comes from `validate/patch113b.sh` (`./t check` runs it). Other modes (`./target/release/ja1front`
with no arguments prints this list):

| Mode | What it does |
|---|---|
| `--play SAVE [SEED] [--png OUT]` | the window from a day save; no SEED = the clock, as the original |
| `--script SAVE\|menu SEED SCRIPT OUT` | runs a play script with no window (world_load.md section 8); the tests use it |
| `--serve SAVE\|menu SEED LOG OUT` | the same, script lines from stdin ([docs/serve.md](../../docs/serve.md)) |
| `--world SAVE OUT`, `--enter SAVE SEED OUT` | world image of a day save; sector entry (below) |
| `--walk`, `--save-copy`, `--new-game` | one walk, a save round trip, a new-game save (diff tests) |
| `[sector 1..60] [--png OUT]` | the first viewer: one sector's static scene |

The window needs the `gui` feature (default; SDL2 dev files). `--no-default-features` builds without SDL; then the
modes without a window still work.

Reads at run time: the CD image `rom/JA1.gog` (own ISO 9660 reader, `src/iso.rs`) and the game folder
(`build/game113b`, then the CD; `iso::game_file`). Nothing from `rom/` is copied into the repo.

## Older modes

In the first viewer (`[sector]`): arrows scroll one tile (clamped to the map), PageUp / PageDown = previous / next
sector (1..60), Escape quits.

    ./target/release/ja1front ../../rom/JA1.gog ../../rom/JA.EXE --world SAVE OUT

writes the world built from JA.EXE, JAGGED.DAT and the day save SAVE ([world_load.md](../../docs/spec/world_load.md)
sections 1-3, no sector entry) to OUT as the section 6 world image (94,042 bytes) and exits 0. Loader: `src/world.rs`.

    ./target/release/ja1front ../../rom/JA1.gog ../../rom/JA.EXE --enter SAVE SEED OUT

enters the save's sector (world_load.md section 4: map from TERRAIN.DAT, `ja1_enter_sector` of `port/ja1logic`
with the front-end as host, one draw of the view), writes the world image to OUT and prints `seed S draws D` (end
seed, count of random numbers drawn). SEED = start seed (decimal u32). Works without the `gui` feature.

    ./target/release/ja1front ../../rom/JA1.gog ../../rom/JA.EXE --play SAVE [SEED] [--png OUT]

enters the sector the same way (no SEED = the clock) and shows it with its soldiers in the window (arrows scroll,
Escape quits), or writes the 14 x 12 tile view to OUT. The first view is the world's view_x / view_y.

The host (`src/host.rs`, section 7) answers ops 86 open_patch, 87 read, 88 close, 96 seek from the CD image, 92
open_tmp = -1, 93 sector_resets = one random(3), 89 / 94 / 95 = 0; any other op (and 47 div_zero) stops with an
error naming the op. A sector with a saved sector file (sectors row byte 0xA9 != 0) is refused. `host_ops` = 0.
Random numbers: section 5 (Watcom rand).

## C interface (`libja1front.so`)

    int32_t ja1f_render_view(const char *iso_path, const char *exe_path, int32_t sector,
                             int32_t c0, int32_t r0, int32_t cols, int32_t rows, uint8_t *out);
    int32_t ja1f_palette(const char *iso_path, int32_t tileset, uint8_t *out768);  /* 8-bit RGB */
    int32_t ja1f_tileset(const char *iso_path, int32_t sector);                    /* 0..9 */
    int32_t ja1f_island_map(const char *iso, const char *exe, const uint8_t *sectors /*60x9*/,
                            const int8_t *marks /*60*/, uint8_t *screen /*320x200*/, uint8_t *palette /*768, 6-bit*/);
    int32_t ja1f_map_sector(int32_t x, int32_t y);          /* sector under the mouse, -1 when y >= 141 */
    int32_t ja1f_map_frame(const char *iso, const char *exe, int32_t sector /*0..59*/, int32_t colour,
                           uint8_t *screen /*320x200, in/out*/);   /* 0 = done, < 0 = error */

`ja1f_island_map` draws the island map picture (strategic.md "The island map picture", `src/island.rs`). In the
`--play` window, key M shows it from the world's sector table (marks all 0) with the map palette. In the map view
(`src/mapui.rs`; strategic.md "Sector click on the island map" and "Mouse over the island map"): the mouse frames the
sector under it; left or right click = `ja1_map_click` with the map mode; keys T / P / G = mode 0 / 1 / 2 (mode 0,
process warning flag 0 each time the view opens); Enter = `ja1_map_start` (0 leaves the view, else the message shows);
M or Escape leaves the view. The message text shows at the bottom (y >= 170, wrapped) until the next click. Host op
302 prints "guide says quote 47" to stderr; the click sound is not played. The world's `accounts` and `sector_order`
come from the day save (game state block at 0x50CC, 0xA8 bytes; sector order at 0x5436). With env `JA1_MAP_PNG=OUT`
the picture is written as PNG while the view shows.

`ja1f_render_view` writes 16*cols x 14*rows palette indexes (row-major) for the view with top-left tile (c0, r0).
Errors: -1 bad argument, -2 sector not 1..60, -3 view off the map or wider than the buffer (cols 1..22,
rows 1..56), -4 missing / bad file, -5 internal panic. Loaded data is cached per CD image, JA.EXE and tileset.

## Layout

- `src/iso.rs`: ISO 9660 (primary volume descriptor, directory records, ";1" suffix ignored).
- `src/sprite.rs`: compiled sprite x86 code (mov byte/word/dword [edi+k], imm; add edi, ebx; ret) to pixel lists.
- `src/data.rs`: TERRAIN.DAT, JAGGED.DAT tables, TILES*n*.DAT, TILE*n*.PAL, TILESET*n*.LST, JA.EXE tables.
- `src/render.rs`: the 384 x 854 back buffer and the four drawing steps.
- `src/world.rs`: the library's world from JA.EXE, JAGGED.DAT and a day save; the world image.
- `src/host.rs`: `Ja1World` / `Ja1Host` (repr(C), port_api.md) over a `World`, the random numbers, the host ops.
- `src/enter.rs`: sector entry (map, `ja1_enter_sector`, first draw) and the world view with soldiers.
- `src/island.rs`: the island map picture (cells and markers), sector under the mouse, sector frame.
- `src/mapui.rs`: the map view of the play window (mouse, click, keys, status line).
- `src/lib.rs`: cache and C entry points; `src/main.rs`: SDL2 viewer and PNG writer.

Links `port/ja1logic` as a path dependency (its `#[no_mangle]` C entry points).

Spec gaps are marked `// SPEC GAP:` in the code.

## History

The first steps (to 2026-10-29) were written only from `docs/spec/` (clean room): step 3a drew one sector's static
scene, later steps added the library world, sector entry and play. Git history holds the details.
