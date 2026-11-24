# Front-end requests (draft)

The library never draws, plays sound or reads input. Where the original game logic does one of these in the
middle of a rule, the library must ASK the front-end and wait for it. This list is the planned set of such
requests. It is a DRAFT (2026-10-19): it comes from the screen, sound and timing routines that the tested game
logic calls. Entries marked "unknown" are calls whose purpose is not analysed yet; each one is analysed when the
port needs it.

Interrupts and reactions happen between animation steps, so animation timing belongs to the request list too:
the library decides when a step happens, the front-end only shows it.

## Requests with a known purpose

| group | request | when the logic makes it | answer |
|---|---|---|---|
| prompt | yes / no question (text, speaking merc) | before a choice the player confirms | yes or no |
| message | status line text | sightings ("Guard sees a new enemy!"), item use | none |
| message | merc message ("Enemy spotted!") | sector triggers, sightings | none |
| message | merc text ("<name> <text>") | merc-only results (slot below 8) | none |
| message | turn banner (guards' / opponents' turn, interrupts) | a turn change | none |
| sound | effect from a slot (rate, volume, loops) | shots, hits, doors, explosions | none |
| sound | soldier cry (hit, die, attack ...) per profile | damage, death, attacks | none |
| sound | merc voice remark (profile, remark number) | sightings, orders, reactions | none |
| sound | empty-gun click | an attack with no ammunition | none |
| sound | quote of the radio contact (Walter) | story events | none |
| sound | Santino's quote | once per game | none |
| view | focus the view on a soldier (if the player can see it) | guard sightings, interrupts | none |
| view | redraw the tactical frame | before each AI action when "show AI" is on | none |
| view | hide path marks | after a walk order | none |
| view | merc portrait panel, energy bar | AP, energy or health changes | none |
| timing | wait n ms, delay n ms | radio use, AI pacing | none |
| animation | start an animation (code, keep, reset) | every action | none (the library keeps the soldier state) |
| animation | next animation frame / step | each game tick of a moving soldier | none |

## Calls not analysed yet (unknown)

Calls that the tested logic makes into screen, sound or file code, with no known purpose yet: walk sound, step
sound, mouse hide / show, effect load / play, picture load (PCC), copy to the screen, sector data file name, the
deselect call, item panel, puff and effect animations (0x157BC, 0x156B8), and the unnamed routines 0x320A0,
0x355D0, 0x17B70, 0x33204, 0x323D4, 0x32EB8, 0x32828, 0x10398, 0x1148C, 0x67E30, 0x53708, 0x1CB78, 0x1F6E4,
0x331A4, 0x328A4, 0x10310, 0x254F0.

## Open questions

- Which requests must block the library (yes / no, animation steps) and which can be queued (messages, sound)?
- Does any message or sound change game state in the original (for example a voice that also sets a flag)?
- The library's tick and the front-end's frame rate: one AI tick per frame as in the original, or a fixed rate?
