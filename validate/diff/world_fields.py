"""The port's world fields by name against the original's memory (quickrestore_diff, loadgame_diff; #39).

A file round trip through the port's reader and writer cannot see a field in the wrong place: the writer undoes
what the reader did. So these tests read the fields out of the port's world (quicksave::field_table) and compare
each with the original's memory at the field's own address. The addresses come from port_lib._world_map, the map
that the AI and strategic diff tests use, never from the save layout.
"""
import struct

import ai
import port_lib

FLAGS = ("INVENTORY", "BLAST", "WALTER", "PROFILES", "LEDGER", "PAY", "STATS", "MORNING", "DAYSTART", "STORYDAY",
         "CLOCK", "GUIDE", "ENEMY", "VERDICT")
# Names that the map holds under another name, or not at all (with the test that names the address).
EXTRA = {"clock.minute": (0x22A0F5, 1), "clock.hour": (0x22A0F6, 1), "clock.hours": (0x22A0F4, 1),  # clock_diff
         "clock.minutes": (0x22A0F7, 4), "clock.bar": (0x22A0FB, 4),                                # clock_diff
         "marker_kind": (0x229CA8, 4), "marker_tile": (0x229CAC, 4),                                # cursor_diff
         "loss_slot": (0x22A1F1, 1), "loss_timer": (0x92DE8, 4), "loss_part": (0xFA908, 16),         # lossmsg_diff
         "loss_stat": (0xFA918, 16),                                                                # lossmsg_diff
         "cursor_mode": (0x284F1B, 1), "cursor_value": (0x284F1D, 1), "music_wanted": (0x96360, 2),  # quickrestore_diff
         "sight_queue": (0x229E00, 3),                                                              # MercSees? 0x56D28
         "small_panel": (0x92E30, 4),                                                               # labels.csv g_smallPanel
         "tile_anim_block": (0x229E24, 110), "sees_text": (0x10DA50, 64),
         "guide_queue": (0x22A4C9, 7),                                                              # guide_diff
         "full_shown": (0x92E34, 1), "panel_ends": (0x92E35, 1), "strip_flag": (0x92E36, 1),        # world.rs, timer_diff
         "mouse_saved_x": (0x229C40, 4), "mouse_saved_y": (0x229C44, 4),                            # MouseSavePos2? 0x67E18
         "opt_confirm_move": (0x1163C6, 1),                                                         # 0x1C7A8
         "last_quote": (0x22A1EA, 6), "quote_member": (0x22A1F0, 1),                                # complainquote_diff
         "hinted": (0x22A1F4, 1)}                                                                   # map_diff


def addresses():
    """field name -> (address, size), with every optional part of the map switched on."""
    keep = {f: getattr(port_lib, f) for f in FLAGS}
    for f in FLAGS:
        setattr(port_lib, f, True)
    try:
        scalars, bufs = port_lib._world_map(ai)
    finally:
        for f, v in keep.items():
            setattr(port_lib, f, v)
    out = {k: (v[0], v[1]) for k, v in scalars.items()}
    out.update({k: (v[0], v[1]) for k, v in bufs.items()})
    out.update(EXTRA)
    return out


def parse(raw):
    """the bytes of quicksave::field_table: name -> field bytes"""
    out, at = {}, 0
    while at < len(raw):
        n = raw[at]
        name = raw[at + 1:at + 1 + n].decode()
        at += 1 + n
        size = struct.unpack_from("<I", raw, at)[0]
        at += 4
        out[name] = raw[at:at + size]
        at += size
    return out


def short(name):
    """w.update.turn_timer -> turn_timer; w.clock.minute -> clock.minute (the clock's names are not in the map)"""
    name = name[2:] if name.startswith("w.") else name
    for part in ("update.", "inv."):
        if name.startswith(part):
            name = name[len(part):]
    return name


def compare(fields, segments, rt, skip=lambda addr, off: False):
    """fields: name -> bytes (port). segments: [(memory address, file offset, memory after the call)] (original).
    rt: image address -> memory address. A field counts only where the original's read reached its bytes; skip
    (image address, field offset) leaves out single bytes. -> (fields compared, bytes compared, [(name, offset, port
    byte, original byte)], names with no address)."""
    addr = addresses()
    mem = {}
    for dest, _, data in segments:
        for i, v in enumerate(data):
            mem[dest + i] = v
    nfields, nbytes, diffs, unmapped = 0, 0, [], []
    for full, value in fields.items():
        name = short(full)
        if name not in addr:
            name = name.split(".")[-1]
        if name not in addr:
            unmapped.append(short(full))
            continue
        base, size = addr[name]
        at = rt(base)
        hit = 0
        for i in range(min(size, len(value))):
            if at + i not in mem or skip(base, i):
                continue
            hit += 1
            if value[i] != mem[at + i]:
                diffs.append((short(full), i, value[i], mem[at + i]))
        if hit:
            nfields += 1
            nbytes += hit
    return nfields, nbytes, diffs, unmapped
