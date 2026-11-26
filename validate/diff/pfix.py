"""#71 port fixes (P ids): the same fix as a small patch of the ORIGINAL in Unicorn, so a diff test can compare the
port at a P id with the fixed original (`--pfix IDS` of ai_diff.py / inv_diff.py). Each patch is the smallest byte
change that fixes the #2 bug in 1.13b. Patches that need room jump to a cave on an own page (CAVE)."""
import struct

CAVE = 0x0C800000          # an own page, outside the image and the emulator's other maps (HEAP 0x0D000000, RET 0x0E000000)


def _cave_unstack(base_reg_add):
    """UnstackItem 0x53580 call site `imul eax, eax, 0xC; add eax, REG; call 0x53580` (10 bytes) -> `call cave` +
    5 nops. Cave: rect index eax >= 12 (attachment / ground: no item entry) returns 0, else the old three steps."""
    return (b"\x83\xF8\x0C"          # cmp eax, 12
            b"\x73\x0A"              # jae zero
            b"\x6B\xC0\x0C" + base_reg_add +   # imul eax, eax, 12 ; add eax, REG
            b"\xE9REL0"              # jmp UnstackItem (it returns to the call site)
            b"\x31\xC0\xC3")         # zero: xor eax, eax ; ret


def _cave_map_column():
    """MapClickToSector? 0x41426 `sub ebx, 3; mov esi, 0x17` (8 bytes) -> `call cave` + 3 nops. Cave: column eax
    (x - 3) / 31 above 9 becomes 9, then the old two steps."""
    return (b"\x83\xF8\x09"          # cmp eax, 9
            b"\x7E\x05"              # jle +5
            b"\xB8\x09\x00\x00\x00"  # mov eax, 9
            b"\x83\xEB\x03"          # sub ebx, 3
            b"\xBE\x17\x00\x00\x00"  # mov esi, 0x17
            b"\xC3")                 # ret


def _cave_no_record_left(e):
    """AssignScreen? store take, view 0, 0x49E51 `mov eax, [cur]; xor edx, edx` (7 bytes) -> `call cave` + 2 nops.
    Cave: no current record (0x229C7C = 0) leaves the call and goes on at 0x49E96 (cursor picture of the empty
    cursor, wait for the button up), else the old two steps."""
    return (b"\xA1" + struct.pack("<I", e.rt(0x229C7C)) +   # mov eax, [cur]
            b"\x85\xC0"              # test eax, eax
            b"\x75\x08"              # jnz ok
            b"\x83\xC4\x04"          # add esp, 4 (the call's return address)
            b"\xE9REL0"              # jmp 0x49E96
            b"\x31\xD2\xC3")         # ok: xor edx, edx ; ret


def _cave_no_record_right(e):
    """AssignScreen? right press on a store place, view 0, 0x4A083 `mov eax, [cur]; call 0x52108` (10 bytes) ->
    `call cave` + 5 nops. Cave: no current record answers 0 (no info box), else PlayStashRecordInfo? 0x52108 (it
    returns to the call site)."""
    return (b"\xA1" + struct.pack("<I", e.rt(0x229C7C)) +   # mov eax, [cur]
            b"\x85\xC0"              # test eax, eax
            b"\x74\x05"              # jz zero
            b"\xE9REL0"              # jmp 0x52108
            b"\x31\xC0\xC3")         # zero: xor eax, eax ; ret


def _cave_name_keys(e):
    """SaveSlotNameInput? 0x4E075: `mov dl, al; test al, al; jne 0x4E125; call GetKey` (15 bytes) -> `call cave`,
    `jne 0x4E125`, 4 nops. Cave: the key reader's signs ] [ < > 0x7F become the scan codes O G K M S for the cursor
    switch (ZF = 1, no second key read); another non-zero key goes on as a letter (ZF = 0); 0 reads the second key as
    before."""
    pairs = [(0x5D, 0x4F), (0x5B, 0x47), (0x3C, 0x4B), (0x3E, 0x4D), (0x7F, 0x53)]
    code = b"\x88\xC2"                                      # mov dl, al
    for i, (c, k) in enumerate(pairs):                      # cmp al, c ; jne next ; mov al, k ; jmp mapped (51)
        code += bytes([0x3C, c, 0x75, 0x04, 0xB0, k, 0xEB, 41 - 8 * i])
    code += b"\x84\xC0\x75\x08"                              # test al, al ; jnz r (54)
    code += b"\xE8REL0"                                      # call GetKey
    code += b"\x38\xC0\xC3"                                  # mapped: cmp al, al (ZF = 1) ; ret
    code += b"\xC3"                                          # r: ret (ZF = 0)
    assert len(code) == 55
    return code


def _cave_use_tile_none():
    """UseOnTile? 0x3D3E1 `xor edx, edx; mov dl, [esp+0x1C]` (6 bytes) -> `call cave` + 1 nop. Cave: g = 0xFF (no
    ground item) goes on at 0x3D40E (not a no-pickup type), else the old two steps (the stack is 4 deeper here)."""
    return (b"\x31\xD2"              # xor edx, edx
            b"\x8A\x54\x24\x20"      # mov dl, [esp+0x20]
            b"\x80\xFA\xFF"          # cmp dl, 0xFF
            b"\x75\x08"              # jne ok
            b"\x83\xC4\x04"          # add esp, 4
            b"\xE9REL0"              # jmp 0x3D40E
            b"\xC3")                 # ok: ret


def _cave_reserve_shown(e):
    """InvScreenRun? 0x54E7B `call ReserveAp?` (5 bytes, EAX = ESI = the opener, EDX = 0) -> `call cave`. Cave: the
    old call, then ReserveAp?((&0x100958)[selected 0x284F11], 0) when the selected slot is < 8 and its soldier is not
    the opener. ECX is free here (the function pops it), EDX stays 0 for the loop after."""
    return (b"\xE8REL0"                                            # call ReserveAp? (the old call)
            b"\x0F\xB6\x05" + struct.pack("<I", e.rt(0x284F11)) +   # movzx eax, byte [selected]
            b"\x83\xF8\x08"                                        # cmp eax, 8
            b"\x73\x14"                                            # jae done
            b"\x8B\x04\x85" + struct.pack("<I", e.rt(0x100958)) +   # mov eax, [eax*4 + slot table]
            b"\x39\xF0"                                            # cmp eax, esi
            b"\x74\x09"                                            # je done
            b"\xB9" + struct.pack("<I", e.rt(0x17C88)) +             # mov ecx, ReserveAp?
            b"\x31\xD2"                                            # xor edx, edx
            b"\xFF\xD1"                                            # call ecx
            b"\xC3")                                               # done: ret


def _cave_no_end_turn(e):
    """TacticalKeyHandler 0x13490, key D: 0x13DE8 `call EndTurn? 0x121D8` (5 bytes) -> `call cave`. Cave: an enemy
    interrupt waits (turn_moves 0x92DD0 != 0) returns at once, else EndTurn (it returns to the call site)."""
    return (b"\x80\x3D" + struct.pack("<I", e.rt(0x92DD0)) + b"\x00"   # cmp byte [turn_moves], 0
            b"\x75\x05"              # jne done
            b"\xE9REL0"              # jmp EndTurn
            b"\xC3")                 # done: ret


def _cave_no_name(e):
    """LoadVocSample 0x5C960: 0x5C98B `mov ebx, 1` (5 bytes, right after ArcSeek) -> `call cave`. Cave: a missed member
    (ArcSeek -2, no seek) sets the size 0x337940 to 0x22, the VOC header alone: the sample plays 0 bytes (silence)
    instead of the old size from the place behind the directory; then the old step."""
    return (b"\x83\xF8\xFE"                                       # cmp eax, -2
            b"\x75\x0A"                                           # jne ok
            b"\xC7\x05" + struct.pack("<I", e.rt(0x337940)) + b"\x22\x00\x00\x00"   # mov dword [size], 0x22
            + b"\xBB\x01\x00\x00\x00\xC3")                       # ok: mov ebx, 1 ; ret


def _cave_cursor_back(e):
    """CursorItemBack? 0x53660: 0x536BD `call CopyBytes12? 0x47090` (5 bytes) -> `call cave`. Cave: the shown strip
    entry (eax) empty -> the old copy. Else the first empty entry in the strip order 0, 7, 8, 9, 10, 11, 1 (a vest pocket
    7..11 only when the merc has it: +0x17A >= 8 or the body armour +0x48 >= 0x32..0x35) gets the copy; with none, PutItem
    0x3FC78 puts the item on the merc's tile (+0x114, b6 2, as the inventory screen's put-down). The caller then clears
    the cursor item."""
    soldiers, sel, held = e.rt(0xFA958), e.rt(0x284F11), e.rt(0x222B50)
    out = bytearray(b"\x80\x38\x00"            # cmp byte [eax], 0
                    b"\x75\x05"                # jne search
                    b"\xE9REL0")                # copy: jmp CopyBytes12 (it returns to the call site)
    out += b"\x56\x57"                          # push esi ; push edi
    out += b"\x0F\xB6\x35" + struct.pack("<I", sel)        # movzx esi, byte [selected]
    out += b"\xC1\xE6\x09"                                 # shl esi, 9
    out += b"\x81\xC6" + struct.pack("<I", soldiers)        # add esi, soldiers
    need = {7: 0x32, 8: 0x32, 9: 0x33, 10: 0x34, 11: 0x35}
    for k in (0, 7, 8, 9, 10, 11, 1):
        ok = b"\x89\xF8\x5F\x5E\xE9"           # ok: mov eax, edi ; pop edi ; pop esi ; jmp copy
        test = b""
        if k in need:
            test = (b"\x80\xBE\x7A\x01\x00\x00\x08"   # cmp byte [esi+0x17A], 8
                    b"\x73\x06"                      # jae ok
                    b"\x80\x7E\x48" + bytes([need[k]]) +  # cmp byte [esi+0x48], need
                    b"\x72" + bytes([len(ok) + 4]))   # jb next
        out += b"\x8D\xBE" + struct.pack("<I", 12 * k)     # lea edi, [esi+12k]
        out += b"\x80\x3F\x00\x75" + bytes([len(test) + len(ok) + 4])   # cmp byte [edi], 0 ; jne next
        out += test + ok
        out += struct.pack("<i", 5 - (len(out) + 4))       # rel32 to copy
    out += b"\x6A\x00\x6A\x00\x6A\x00\x6A\x02"            # push 0, 0, 0, 2
    for d in (8, 4, 0):
        out += b"\xFF\x35" + struct.pack("<I", held + d)    # push dword [held+d]
    out += b"\x0F\xBF\x86\x14\x01\x00\x00"                # movsx eax, word [esi+0x114]
    out += b"\xBF" + struct.pack("<I", e.rt(0x3FC78)) + b"\xFF\xD7"   # mov edi, PutItem ; call edi
    out += b"\x5F\x5E\xC3"                                  # pop edi ; pop esi ; ret
    return bytes(out)


def _cave_charge_kind(e):
    """SetupSectorFromXra 0x2E918 after the 50 XRA ground items: 0x2EF26 `mov esi, 0x32` (5 bytes) -> `call cave`. Cave:
    the first charge (type 0x71) whose E[1] is not 0x63 gives its E[1] to every charge with E[1] = 0x63 (kind 100 makes
    no blast); then the old step."""
    gi = e.rt(0xEB160)
    return (b"\x60"                                         # pushad
            b"\xB8" + struct.pack("<I", gi) +              # mov eax, items
            b"\xB9\x32\x00\x00\x00"                        # mov ecx, 50
            b"\x80\x38\x71\x75\x06"                        # find: cmp byte [eax], 0x71 ; jne nf
            b"\x80\x78\x01\x63\x75\x07"                    #       cmp byte [eax+1], 0x63 ; jne got
            b"\x83\xC0\x0C\xE2\xF0"                        # nf:   add eax, 12 ; loop find
            b"\xEB\x20"                                     #       jmp done
            b"\x8A\x50\x01"                                 # got:  mov dl, [eax+1]
            b"\xB8" + struct.pack("<I", gi) +              #       mov eax, items
            b"\xB9\x32\x00\x00\x00"                        #       mov ecx, 50
            b"\x80\x38\x71\x75\x09"                        # set:  cmp byte [eax], 0x71 ; jne ns
            b"\x80\x78\x01\x63\x75\x03"                    #       cmp byte [eax+1], 0x63 ; jne ns
            b"\x88\x50\x01"                                 #       mov [eax+1], dl
            b"\x83\xC0\x0C\xE2\xED"                        # ns:   add eax, 12 ; loop set
            b"\x61"                                         # done: popad
            b"\xBE\x32\x00\x00\x00\xC3")                    #       mov esi, 0x32 ; ret


def _cave_overview_back(e):
    """SetViewMode 0x248E4, mode 0 after the mouse point: 0x24B64 `mov ebx, 0xC` (5 bytes) -> `call cave`. Cave: old mode
    0x92DB9 (signed) >= 2 adds 48 to view x, old mode 3 also 14 to view y (the wide-view shifts taken back), then the old
    step."""
    return (b"\x0F\xBE\x05" + struct.pack("<I", e.rt(0x92DB9)) +    # movsx eax, byte [old mode]
            b"\x83\xF8\x02\x7C\x13"                                 # cmp eax, 2 ; jl done
            b"\x83\x05" + struct.pack("<I", e.rt(0x229CF8)) + b"\x30"  # add dword [view x], 48
            b"\x83\xF8\x03\x75\x07"                                 # cmp eax, 3 ; jne done
            b"\x83\x05" + struct.pack("<I", e.rt(0x229CFC)) + b"\x0E"  # add dword [view y], 14
            b"\xBB\x0C\x00\x00\x00\xC3")                             # done: mov ebx, 0xC ; ret


# id -> list of (address, bytes) or (address, "cave", cave code (or a function of the emulator that returns it), site length, jump target of b"REL0" or None)
PATCH = {
    # P01 (#2 row 9, ConsumeKey? 0x3AE78): `mov [edi+0xB], 0` cleared the hand entry's 4th key id; after the move-down
    # loop eax = ring entry + 3, so `mov [eax+8], 0` clears the ring's own.
    1: [(0x3AF66, b"\xC6\x40\x08\x00")],
    # P02 (#2 row 10): ctrl + left click on item rect 12 (attachment) or 13 (ground) unstacked soldier +0x90 / +0x9C.
    # UseInventoryItem? 0x54179 (base esi) and AssignScreen? 0x49619 (base ebp): skip the unstack for rect >= 12.
    # P03 (#2 row 13, MercsQuitMsg? 0x50D2C): the tapper message took CL (guards quit) for BH (tappers quit).
    3: [(0x50E64, b"\x88\xF8")],
    2: [(0x54179, "cave", _cave_unstack(b"\x01\xF0"), 10, 0x53580),
        (0x49619, "cave", _cave_unstack(b"\x01\xE8"), 10, 0x53580)],
    # P04 (#2 row 16, MapClickToSector? 0x41410): x = 313..319 gave column 10 (the next row's first sector); the
    # column is capped at 9, as the left margin x = 0..2 gives column 0.
    4: [(0x41426, "cave", _cave_map_column(), 8, None)],
    # P05 (#2 row 19, AssignScreen? 0x4891C): a left or right press on a store place in view 0 with no current record
    # (an empty store) read the "record" at memory address 0; with the fix it takes nothing and shows no info.
    5: [(0x49E51, "cave", _cave_no_record_left, 7, 0x49E96),
        (0x4A083, "cave", _cave_no_record_right, 10, 0x52108)],
    # P06 (#2 row 20, StoreAllItems 0x52150, key E and FIRE): bytes 0..5 of the entry were cleared after
    # AddStashItem?(copy, 0) even when it stored nothing (99+ of the type). AddStashItem? clears its copy when it
    # stores, so the entry is cleared only when the copy's type byte [esp] is 0 (the same 24 bytes, eax is free here).
    6: [(0x5217C, b"\x80\x3C\x24\x00"      # cmp byte [esp], 0
                  b"\x75\x11"              # jne 0x52193 (not stored: keep the entry)
                  b"\x31\xC0"              # xor eax, eax
                  b"\x89\x03"              # mov [ebx], eax (bytes 0..3)
                  b"\x66\x89\x43\x04"      # mov [ebx+4], ax (bytes 4, 5)
                  + b"\x90" * 9)],
    # P07 (#2 rows 26 + 27, SaveSlotNameInput? 0x4DEF4): Home/Left/Right/End/Delete reach the name input's cursor
    # switch; before, their signs went into the name (and a drawn 0x7F crashed).
    7: [(0x4E075, "cave", _cave_name_keys, 5, 0x560F4),
        (0x4E07A, b"\x0F\x85" + struct.pack("<i", 0x4E125 - 0x4E080) + b"\x90" * 4)],
    # P08 (#2 row 37, UseOnTile? 0x3CFD0): the no-pickup type test (0x44 / 0x52 / 0x70) read ground entry 0xFF.
    8: [(0x3D3E1, "cave", _cave_use_tile_none(), 6, 0x3D40E)],
    # P11 (#2 row 47, InvScreenRun? 0x54D50): only the opener's points were reserved after the screen; a merc switched
    # to inside it (the selected slot at the end) kept its old reserve. The cave reserves for that merc too.
    11: [(0x54E7B, "cave", _cave_reserve_shown, 5, 0x17C88)],
    # P10 (#2 row 46, ShowAimScreen 0x1FEB0): save area 4 (never restored) shared back buffer row 66 with area 5 and
    # was saved after it; the two saves change places (0x1FF2D..0x1FF54, the calls re-aimed).
    10: [(0x1FF2D, b"\xBB\xB9\x00\x00\x00\xBA\x18\x01\x00\x00\xB8\x04\x00\x00\x00\xE8"
         + struct.pack("<i", 0x467E4 - 0x1FF41)
         + b"\xBB\x4E\x00\x00\x00\xBA\x10\x00\x00\x00\xB8\x05\x00\x00\x00\xE8"
         + struct.pack("<i", 0x467E4 - 0x1FF55))],
    # P18 (#2 row 51, LoadVocSample 0x5C960): a missing name member (speaker 29 has no 40.VOC) played the old size of
    # bytes from behind the directory (another merc's name); with the fix it plays nothing.
    18: [(0x5C98B, "cave", _cave_no_name, 5, None)],
    # P20 (#2 row 62, DayEndVerdict 0x60680 step 13): a capacity above the start with no quote in the table said
    # q + signum(score) (JAC 998..1000, no record). The four default jumps to 0x60ECC go to 0x60EEE instead (its test
    # start_cap >= cap fails here, so on to 0x60F33: no remark, `said` unchanged).
    20: [(0x60E7F, b"\xEB\x6D"), (0x60E86, b"\xEB\x66"), (0x60E94, b"\xEB\x58"), (0x60E9B, b"\xEB\x51")],
    # P21 (#2 row 63, TacticalKeyHandler 0x13490 key D): EndTurn popped an enemy that won an interrupt while the
    # merc's action was busy; turn_moves stayed 1 and the enemies' turn stopped after its shot. No EndTurn while
    # turn_moves != 0.
    21: [(0x13DE8, "cave", _cave_no_end_turn, 5, 0x121D8)],
    # P22 (#2 row 64, CursorItemBack? 0x53660 at the end of a day): the held item was copied over the shown strip
    # entry (the gun was lost). It goes to the first empty entry, else to the ground under the merc.
    # P16 (#2 row 36, SetupSectorFromXra 0x2E918): a charge with E[1] = 0x63 got blast kind 100 (no blast, no damage, no
    # crater). It takes the kind of the sector's first other charge.
    # P15 (#2 row 33, SetViewMode 0x248E4): the overview kept the wide-view coordinates, so 1 -> 2 -> 3 -> 0 -> 1 moved
    # the view (-48, -14). 2 / 3 -> 0 takes the shifts back (cave), 0 -> 2 / 3 shifts as from 1 (the `je` of the old
    # mode 0 test at 0x249E4 gone).
    # P14 (#2 row 32, ReplayBattle 0x6494C): each flash frame called DrawMarkersTo 0x41788 with the sector's map
    # coordinates into the 32 x 24 scratch box (sectors 0..2: a shifted copy of the markers). The call is gone.
    14: [(0x64B5E, b"\x90" * 5)],
    15: [(0x249E4, b"\x90\x90"), (0x24B64, "cave", _cave_overview_back, 5, None)],
    16: [(0x2EF26, "cave", _cave_charge_kind, 5, None)],
    22: [(0x536BD, "cave", _cave_cursor_back, 5, 0x47090)],
}


def parse(argv):
    """Takes `--pfix IDS` (or `all`) out of argv; returns the id list or None."""
    if "--pfix" not in argv:
        return None
    i = argv.index("--pfix")
    v = argv[i + 1]
    del argv[i:i + 2]
    return sorted(PATCH) if v == "all" else [int(x.strip().lstrip("Pp")) for x in v.split(",")]


def apply(e, ids, port_lib):
    """Original side: write each id's patch into emulator `e`; port side: the P mask bits (`port_lib` = any object
    with a port_fixes(mask) method)."""
    cave = None
    off = 0
    for p in ids:
        for a, *rest in PATCH[p]:
            if rest[0] == "cave":
                if cave is None:
                    cave = CAVE
                    e.uc.mem_map(cave, 0x1000)
                code, n, target = rest[1:]
                if callable(code):       # a cave with image addresses in it
                    code = code(e)
                at = cave + off
                if target is not None:
                    j = code.index(b"REL0")
                    code = code[:j] + struct.pack("<i", e.rt(target) - (at + j + 4)) + code[j + 4:]
                e.uc.mem_write(at, code)
                site = e.rt(a)
                e.uc.mem_write(site, b"\xE8" + struct.pack("<i", at - (site + 5)) + b"\x90" * (n - 5))
                off += (len(code) + 15) & ~15
            else:
                e.uc.mem_write(e.rt(a), rest[0])
    e.uc.ctl_flush_tb()
    port_lib.port_fixes(sum(1 << (p - 1) for p in ids))
