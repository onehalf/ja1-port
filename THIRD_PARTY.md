# Third-party software

The release builds link these libraries into `ja1front`. All have licences that allow use in a GPL-3.0 program.

| Part | Licence | Source |
| --- | --- | --- |
| SDL 2 (linked statically in the release builds) | zlib | https://www.libsdl.org |
| sdl2, sdl2-sys (Rust bindings) | MIT | https://github.com/Rust-SDL2/rust-sdl2 |
| png | MIT or Apache-2.0 | https://github.com/image-rs/image-png |
| flate2, miniz_oxide, fdeflate, crc32fast, adler2, simd-adler32, zlib-rs | MIT / Apache-2.0 / Zlib (each crate states its own) | https://crates.io |
| bitflags, cfg-if, lazy_static, libc, version-compare | MIT or Apache-2.0 | https://crates.io |

`port/ja1front/Cargo.lock` holds the exact versions.

## Not included

- The game Jagged Alliance (Sir-tech / Madlab, 1994): you need your own copy (GOG).
- The official 1.13b patch files: optional, supply your own (README "Play").
- For the RE tools: Capstone, Unicorn, Ghidra, DOSBox-X are installed separately; the Watcom libraries that
  `./t libid` compares against are not included.
