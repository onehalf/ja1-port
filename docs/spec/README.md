# Game system notes

These pages are notes per game system: what the game does, in plain language, with formulas, tables and file
layouts, and how each point was checked (diff test, live session, save comparison). Port code may be written
straight from the asm and the decomp (the maintainer ruling 2026-10-29 19:53; CONTRIBUTING.md "How the port was written", issue #37); a port
change needs no page change first, but a page that is wrong gets fixed. Nothing from `rom/` goes into git.

Pages: [port_api.md](port_api.md) (the library's C interface), [inventory.md](inventory.md), [ai.md](ai.md), [combat.md](combat.md), [savegame.md](savegame.md), [speech.md](speech.md), [strategic.md](strategic.md), [tactical_map.md](tactical_map.md), [soldier_update.md](soldier_update.md), [shooting.md](shooting.md), [click.md](click.md), [select.md](select.md), [panel.md](panel.md), [cursor.md](cursor.md), [sound.md](sound.md)

## Accepted differences

the maintainer ruled that the port may differ from the original at these points. Each bug stays listed in issue #2.

- **Item store list** (the maintainer 2026-10-29, issue #19): the port keeps a plain list and never loses a store item. See
  [strategic.md](strategic.md), the "Accepted difference" note just above "Key E: all items into the store".
- **Key repeat rate** (the maintainer 2026-11-07, issue #56, ruling comment 12095): the port uses the player's system key
  repeat rate (SDL key events with the repeat flag). It does not copy the key timer of the DOS original. At the
  same rate, a held Space gives the same merc changes in the same order at the same speed (evidence comment 11941,
  validate/live_held_space.py: 78 changes in 3.03 s, original and port).

## History (to 2026-10-29)

Until the ruling the project kept a clean-room split: the analysis side (REVERSE.md, labels, tools/) read the
original, these pages held only verified behaviour, and the port was written from these pages alone.

Port workflow: the port side reads only these pages; a test that fails against the original means the page is
incomplete or wrong. The analysis side fixes the page (not the port code), then the port is rewritten from the page.
The pilot (movement) found 9 such gaps in "Finding the path" and 0 in the move-code and AP rules.
Slice 2a (the AI decision layer, [port_api.md](port_api.md)) failed on all 9 entry points at first: the old
ai.md text described the decisions but not the order of calls, the widths of values and the record bytes written.
The sections One tick .. Engaged and "Starting the chosen action" were rewritten as exact procedures; after that
only 2 gaps remained (pixel arguments of `direction`, an unsigned AP byte in Engaged).
