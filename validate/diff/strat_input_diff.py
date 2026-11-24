"""Differential test: the strategic screen's input loop (StrategicScreen 0x432F4), map area and panel, original vs port.

usage: .venv/bin/python validate/diff/strat_input_diff.py [ncases=60] [DUMP] [--port] [--lib SO]
Per case: the sector table (60 x 9 bytes), the worker pool, the idle pool, the plants, the sector order, the
current sector, the team on duty, the balance, the hint flag 0x22A1F4, the native wage 0x229FF4 and the map mode
byte 0x93DA0 are fuzzed, then a tape of up to about 40 mouse polls (x, y, buttons) is walked through the loop from
0x4334D: sector clicks inside the island map (y <= 140) and left presses on the map panel's tab row (tabs 0..2,
y >= 141; tab 3 is out: the port keeps its processing page until the button comes up). The tape starts with one
of eleven kinds:
- none: no start press.
- 14: a start button press (281..306 x 176..191) with current sector 0xFF and a team on duty; the start check
  answers 14 ("Starting sector not specified!").
- 15: a start button press with a balance below 0; the start check answers 15 ("Costs are higher than you can
  afford!").
- hint: a valid sector, a team and a balance; the first start press answers 16 and shows messages 16 and 17
  ("You may want to visit the ASSIGNMENT screen..." and "To do this, click on the RAISED team cost button...")
  and sets the hint flag; the next start press points at the day start (ja1_map_start 0), whose "Get to work?"
  question (ConfirmDialog 0x275E4) this harness stubs with its NO answer, so the original's loop goes on.
- wage: a left press inside the panel's native wage box (138..172 x 162..170); the loop runs its own mouse
  polls (MousePoll 1) until a left press ends it. The wage does not change here, so no salary question opens.
- wagemax: the same, with the wage at its maximum 100: a right press answers "Natives' salary is at its
  maximum!" in a message box (StatusMessage), which the tape closes, and the left press then ends the box.
- finance: a left press on the panel's field 4 box (233..267 x 180..188) opens the finance page (PlayFlipAnim
  1..3), then a press on its back button (74..99 x 153..168) closes it (PlayFlipAnim 3..1, step -1).
- wageno: a left press in the native wage box, a right press (the wage goes up by one, WorkerPool 0x50BC4 and
  CalcLedgerAll run for real), a left press ends the box; the changed wage asks "Raise natives' salary?" in a
  ConfirmDialog box, answered NO: the wage goes back and WorkerPool and CalcLedgerAll run again. Two map moves
  follow, so the sector frames and the recomputed sector fields compare after the question.
- wageyes: the same, answered OK: the wage stays; the tape ends at the answer (the port's travel animation holds
  the map until a click, which this stub dialog does not model).
Both wage kinds use a wage below the maximum and sector tables with no tappers and no guards, so the recomputed
worker pool can not drop below the workers (no "guards quit this morning!" box).
- plants: a left press on the panel's tab 3 (190..225 x 155..164, table 0x93ED8) shows the processing plants page
  (DrawProcessingUsage 0x42148); the page holds while the button is down (its own WaitLeftUp 0x67C54) and the
  release ends it. The mouse moves during the hold, so the page must stop the map's hover frames. Two map moves
  follow; the map mode byte must not change.
- assign: a left press on the panel's field 0 box (22..56 x 180..188) opens the assignment screen. AssignScreen?
  0x4891C runs for real (its opening FUN_47AE4 and its drawing are stubbed), its Exit button (button 9) is pressed,
  and with no team on duty its question "Hmmm. No one assigned to duty?" (QuestionBox 0x278A0, which waits for its
  answer here) answers NO (the screen stays, Exit again) or yes. Leaving writes the team to the current sector's
  mercs byte; with no team the original then clears the current sector (0xFF). Opening sets the hint flag. Two map
  moves follow.
The original runs in Unicorn on tac60: the setup calls (DayEndMercUpdate 0x43F54, CollectCash 0x445DC, TimerStart
0x47020) and every drawing, sound and loader call are stubbed; MousePoll 0x678E8 answers the tape; StatusMessage?
0x27D24 logs its text and waits for the next button down, then until it is up (KeyOrClick 0x67E44);
FlashSectorFrame? 0x4147C logs (sector, colour); GuideSpeech 0x5F5AC logs (op, argument); PlayFlipAnim 0x51264
logs (first picture, count) and (step) and returns at once; MapClickToSector
0x41410, SectorConnectedToHome 0x41178, CalcLedgerAll 0x51178, WorkerPool 0x50BC4 (with its RecalcTerritory
0x50EA8), WaitLeftUp 0x67C54, WaitRightUp 0x67C68, MouseInRect 0x67C0C, ButtonAt 0x67CD8 and the sprintf_ of the
day start's sector text run for real. ConfirmDialog 0x275E4 answers NO (and pops its 3 stack args); for the wage
change's "Raise natives' salary?" question it waits for the answer like StatusMessage does, then returns the
case's answer. MercsQuitMsg 0x50D2C is stubbed (the fuzz keeps the idle pool at 0 or more). KeyPressed 0x5616C /
GetKey 0x560F4 answer "no key".
The run ends when MousePoll has no tape entry left (the harness stops the emulator).
The port (port/ja1front ja1f_map_pass) walks the same tape on a world from QICKSTRT.SAV whose sector table,
plants, sector order, current sector, team, balance and hint flag are the same fuzzed values, through MapUi's
track / click / release (the map panel click of the strategic screen) and start_pressed (its start button; a 0
keeps the loop, as the stub's NO answer does).
Compared: the sector frames, the message texts, the guide quotes and the finance page's flips (first picture,
count, step) in poll order, the sector table, the mouse sector number 0x22A010, the current sector, the map mode
byte (a tab press changes it), the hint flag, the native wage, the worker pool and the idle pool (from 0x229FDC
and 0x229FEC) and the polls used. The accounts' ledger fields are not
compared: CalcLedgerAll reads the money and soldier records of each side's own world, and map_diff already
compares that call on one state. Without --port only the original runs (a smoke test of the harness). Exits 1 on
mismatch. STRAT_DEBUG=1 prints the count of the day questions.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from unicorn.x86_const import UC_X86_REG_ESP
from emu import Emu, EmuStop        # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 60
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_map_pass.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_void_p] * 4 + [ctypes.c_int32] * 4 + \
        [ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p,
         ctypes.c_int32]
    PORT.ja1f_map_pass.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
    SAVE = os.path.join(ROOT, "rom", "__support", "save", "QICKSTRT.SAV").encode()

if not os.path.exists(dump):
    print(f"strat_input_diff: {dump} missing")
    sys.exit(1)

SECTORS, CUR, TEAM, MODE, MX, MY, BUTTONS, POOL, IDLE, PLANTS, ORDER, MOUSE_NUM = (
    0x22979C, 0x22A039, 0x229FAC, 0x93DA0, 0x229D00, 0x229D04, 0x229CF4, 0x229FDC, 0x229FEC, 0x22A12A, 0x92DF0,
    0x22A010)
BALANCE, HINTED, WAGE = 0x229FC8, 0x22A1F4, 0x229FF4
ENTRY = 0x432F4
MSG = {0x97B57: "This sector is in enemy hands!", 0x97ADF: "This sector is not linked!",
       0x99DAB: "No one on duty!", 0x99D91: "No Mercenaries there!",
       0x97DAF: "Starting sector already specified!", 0x99DC5: "No Tappers there!",
       0x97E27: "Not enough workers available!", 0x97F8F: "There are no trees there!",
       0x97F17: "Tappers are too scared to work there!", 0x97E9F: "No more tappers will work there!",
       0x99E13: "No Guards there!", 0x99DDF: "No workers available!", 0x99DF9: "Guard capacity reached!",
       0x97CBF: "Starting sector not specified!", 0x97D37: "Costs are higher than you can afford!",
       0x990E7: "You may want to visit the ASSIGNMENT screen where you can outfit your mercs.",
       0x9915F: "To do this, click on the RAISED team cost button in the lower left area of the screen.",
       0x97BCF: "Natives' salary is at its maximum!"}

# The map panel's tab rects (x0, y0, x1, y1): table 0x93ED8, the original's 0x433C2 panel branch. Tab 3 (the
# processing page) is left out: the port keeps that page until the button comes up, the tape would move the map.
QBUTTONS = {0: (142, 92), 1: (208, 92)}   # the question box's NO and OK (x, y; 38 x 16)
TABRECT = [(22, 155, 57, 164), (76, 168, 111, 177), (76, 155, 111, 164)]

if not os.path.exists(dump):
    print(f"strat_input_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
tape, pos, cur_poll = [], [0], [0]
orig = []
msg_by_addr = {e.rt(a): s for a, s in MSG.items()}


class TapeEnd(Exception):
    pass


def feed(ent):
    x, y, btn = ent
    e.w32(e.rt(MX), x)
    e.w32(e.rt(MY), y)
    e.w32(e.rt(BUTTONS), btn)


def poll(em, *_):
    if pos[0] >= len(tape):
        # the loop would wait for the player: the tape ends here (emu_stop inside the hook, no raise)
        em.stop_reason = "tape end"
        em.uc.emu_stop()
        return 0
    cur_poll[0] = pos[0]
    ent = tape[pos[0]]
    pos[0] += 1
    feed(ent)
    return 0


def frame(em, eax, edx, *_):
    orig.append(("f", eax & 0xFF, edx & 0xFF, cur_poll[0]))
    return 0


def message(em, eax, *_):
    txt = msg_by_addr.get(eax & 0xFFFFFFFF)
    if txt is None:
        txt = em.cstr(eax & 0xFFFFFFFF).decode("latin-1")
    orig.append(("m", txt, cur_poll[0]))
    # the box waits for a new button down, then until it is up (KeyOrClick 0x67E44)
    while pos[0] < len(tape):
        ent = tape[pos[0]]
        pos[0] += 1
        feed(ent)
        if ent[2] & 3:
            while pos[0] < len(tape) and tape[pos[0]][2] & 3:
                feed(tape[pos[0]])
                pos[0] += 1
            break
    return 0


def guide(em, eax, edx, *_):
    orig.append(("g", eax & 0xFFFFFFFF, edx & 0xFFFFFFFF, cur_poll[0]))
    return 0


def question(em, eax, *_):
    # QuestionBox: the assignment screen's question waits for a press on its button, then until the button is up
    # (the release entry included). Other questions answer no at once (no tape entry), as before.
    if eax & 0xFFFFFFFF != e.rt(NO_DUTY_Q):
        return 0
    orig.append(("q", 0, 0, 0))
    pressed = False
    while pos[0] < len(tape):
        ent = tape[pos[0]]
        cur_poll[0] = pos[0]       # the loop tail's sector frame belongs to the release entry
        pos[0] += 1
        feed(ent)
        if ent[2] & 3:
            pressed = True
        elif pressed:
            break
    return QANSWERS.pop(0)


def panel_button(em, eax, *_):
    # PressPanelButton 0x35D44: the real one waits until the left button is up. The start button's tapes let the
    # loop poll that release (no stub wait); the assignment screen's Exit (button 9) must wait, because the
    # strategic loop's start check runs right after AssignScreen returns and the two buttons overlap.
    if eax & 0xFF == 9:
        while pos[0] < len(tape):
            ent = tape[pos[0]]
            pos[0] += 1
            feed(ent)
            if ent[2] & 1 == 0:
                break
    return 0


def plants(em, *_):
    # DrawProcessingUsage 0x42148: the page stays while the left button is down (its WaitLeftUp 0x67C54 at
    # 0x42323 polls itself), then the release sound and the screen restore follow.
    orig.append(("p", 0, 0, cur_poll[0]))
    while pos[0] < len(tape):
        ent = tape[pos[0]]
        pos[0] += 1
        feed(ent)
        if ent[2] & 1 == 0:
            break
    return 0


def flip(em, eax, edx, ebx, ecx, *_):
    # PlayFlipAnim restarts the water timer 0x22A3FC. The real button routine waits for the release before the flip,
    # as the port does; the stub above does not, so the time comes from the tape's release.
    j = pos[0]
    while 0 < pos[0] and tape[pos[0] - 1][2] & 1 and j < len(tape) and tape[j][2] & 1:
        j += 1
    timer[0] = ((j + 1) if pos[0] and tape[pos[0] - 1][2] & 1 else pos[0]) * POLL_MS
    # PlayFlipAnim (name, first, count, step). The original opens the finance page at the field 4 press and closes
    # it at the back press; the port does both at the matching release, so the poll index is not compared (0).
    FLIPS[0] += 1
    orig.append(("a", edx & 0xFFFFFFFF, ebx & 0xFFFFFFFF, 0))
    orig.append(("a", ecx & 0xFFFFFFFF, 0, 0))
    return 0


POLL_MS = 100                  # ja1f_map_pass: MAP_POLL_MS
timer = [0]                    # the time of the last TimerStart on 0x22A3FC
ASKED = [0]                    # the day-start question's stub calls (STRAT_DEBUG shows the count)
FLIPS = [0]                    # the PlayFlipAnim calls (STRAT_DEBUG shows the count)
ANSWER = [0]                   # the salary question's answer for the case (0 = NO, 1 = OK)
QANSWERS = []                  # the assignment screen's question: the answers of the case, in order
NO_DUTY_Q = 0x9834F            # "Hmmm. No one assigned to duty?" (the QuestionBox text of the assignment screen)
SALARY_Q = 0x97C47             # "Raise natives' salary?" (the ConfirmDialog text of a wage change)


def confirm(em, eax, *_):
    ASKED[0] += 1
    if eax & 0xFFFFFFFF == e.rt(SALARY_Q):
        # The salary question waits for the player's answer, as the port's question box does: a press on NO or OK,
        # then until the button is up (the release entry included, the dialog's WaitLeftUp).
        while pos[0] < len(tape):
            ent = tape[pos[0]]
            pos[0] += 1
            feed(ent)
            if ent[2] & 3:
                while pos[0] < len(tape):
                    e2 = tape[pos[0]]
                    pos[0] += 1
                    feed(e2)
                    if e2[2] & 3 == 0:
                        break
                break
        timer[0] = pos[0] * POLL_MS
        return ANSWER[0]
    return 0


for a in (0x43F54, 0x445DC, 0x47020, 0x67BF0, 0x673AC, 0x674B0, 0x35F64, 0x36010, 0x35E84, 0x46920,
          0x46B1C, 0x46C1C, 0x468B0, 0x41640, 0x419BC, 0x5C8A0, 0x5C880, 0x528AC, 0x52870, 0x5C82C,
          0x5C858, 0x5E8E0, 0x5C0EC, 0x532BC, 0x53300, 0x52820, 0x51264, 0x42020, 0x41FA0,
          0x47AE4, 0x41994, 0x67478, 0x28698, 0x67E18, 0x67E30,   # the assignment screen's opening and draws
          0x35ED4,  # PressedFieldBox: ButtonAt(4,0)'s press draw + DOS mouse-queue release wait (0x6782C)
          0x50D2C,  # MercsQuitMsg: the fuzz keeps the idle pool at 0 or more, so no native quits
          0x50618, 0x50750, 0x50AC0, 0x62A00, 0x38190, 0x53538, 0x35D44, 0x5C900, 0x5CB34,
          0x42090):
    e.stub(e.rt(a), lambda em, *_: 0)
limits = []


def set_limit(em, eax, edx, ebx, ecx, *_):
    limits.append((eax & 0xFFFF, edx & 0xFFFF, ebx & 0xFFFF, ecx & 0xFFFF))
    return 0


e.stub(e.rt(0x682AC), set_limit)                # SetMouseLimit?: the mouse limits compare as their own list
e.stub(e.rt(0x275E4), confirm, 12)              # ConfirmDialog?: the day question answers NO; the salary question waits for its answer
# The day start path of the start button: the switch's sprintf_ writes the sector text (Watcom clib, cdecl).
e.stub_sprintf()
e.stub(e.rt(0x36010), lambda em, *_: 0, 4)       # DrawPanelPccFields: one stack arg (push 0xe)
e.stub(e.rt(0x52908), lambda em, *_: 0, 16)      # FadePalette: 4 stack args
e.stub(e.rt(0x52AE0), lambda em, *_: 0, 8)       # BlitCopy: 2 stack args (the finance page's two blits)
# The water rotation 0x43E07: every tape entry takes POLL_MS on the clock. The other timer waits (the finance page's
# two flip waits of 60 ms) end after one call.
def timer_start(em, *_):
    timer[0] = pos[0] * POLL_MS
    return 0


def timer_elapsed(em, *_):
    back = e.r32(em.uc.reg_read(UC_X86_REG_ESP))
    return pos[0] * POLL_MS - timer[0] if back == e.rt(0x43E11) else 0x3c


def rotate(em, eax, edx, *_):
    if edx & 0xFF == 0xD7:         # the first of the two PaletteRotate? calls
        orig.append(("r", 0, 0, pos[0]))
    return 0


e.stub(e.rt(0x47020), timer_start)
e.stub(e.rt(0x47040), timer_elapsed)
e.stub(e.rt(0x528AC), rotate)
e.stub(e.rt(0x5616C), lambda em, *_: 0)          # KeyPressed: no key
e.stub(e.rt(0x560F4), lambda em, *_: 0)          # GetKey
e.stub(e.rt(0x35D44), panel_button)
e.stub(e.rt(0x278A0), question, 32)              # QuestionBox: 8 stack args
e.stub(e.rt(0x678E8), poll)
e.stub(e.rt(0x4147C), frame)
e.stub(e.rt(0x27D24), message)
e.stub(e.rt(0x5F5AC), guide)
e.stub(e.rt(0x51264), flip)
e.stub(e.rt(0x42148), plants)

r = random.Random(3)
edge = random.Random(4)                             # own stream: the edge presses do not move the case stream
bad = 0


def point(s):
    return ((s % 10) * 31 + 3 + r.randrange(28), (s // 10) * 23 + 3 + r.randrange(20))


def a_tape(kind, team=1):
    """poll groups: moves, a click on a sector, a tab press, a start button press, the wage box and the box polls"""
    ps = []
    if kind == "assign":
        # The field 0 box: the press opens the screen, the release is the screen's first poll. Then Exit
        # (269..294 x 166..181) with its release, and with no team on duty the question's answers.
        x, y = 22 + r.randrange(35), 180 + r.randrange(9)
        ps += [(x, y, 1), (x, y, 0)]
        answers = []
        if team == 0:
            answers = r.choice([[1], [0, 1], [0, 0, 1]])
        for a in answers or [None]:
            ex, ey = 269 + r.randrange(26), 166 + r.randrange(16)
            ps += [(ex, ey, 1), (ex, ey, 0)]
            if a is not None:
                bx, by = QBUTTONS[a]
                qx, qy = bx + r.randrange(38), by + r.randrange(16)
                ps += [(qx, qy, 1), (qx, qy, 0)]
        QANSWERS[:] = answers
        for _ in range(2):
            p = point(r.randrange(60))
            ps.append((p[0], p[1], 0))
        return ps
    if kind in ("wage", "wagemax"):
        # A left press inside the native wage box (0x8a,0xa2..0xac,0xaa). The original's inner loop polls the
        # mouse itself (MousePoll 1) until a left press ends it: the entry press runs WaitLeftUp, the exit press
        # is the left-down that ends the do-while, and its WaitLeftUp eats the release.
        x, y = 138 + r.randrange(35), 162 + r.randrange(9)
        ps.append((x, y, 1))
        ps.append((x, y, 0))
        if kind == "wagemax":
            # A right press at the maximum wages: the "Natives' salary is at its maximum!" message box. The box
            # eats a new press and then until it is up; WaitRightUp polls once more (its release entry).
            ps.append((x, y, 2))
            ps.append((x, y, 0))
            ps.append((x, y, 1))
            ps.append((x, y, 0))
        ps.append((x, y, 1))
        ps.append((x, y, 0))
    elif kind == "wagechange":
        # A left press in the native wage box opens its edit loop (WaitLeftUp), a right press raises the wage by
        # one (WorkerPool and CalcLedgerAll run), a left press ends the loop (WaitLeftUp). The changed wage asks
        # "Raise natives' salary?"; the answer is a press on that box's NO (left) or OK (right) button, then its
        # release. On NO the map tracks again after the box; on OK the tape ends at the answer (the port's travel
        # animation holds the map until a click, which the stub dialog does not model).
        x, y = 138 + r.randrange(35), 162 + r.randrange(9)
        ps += [(x, y, 1), (x, y, 0), (x, y, 2), (x, y, 0), (x, y, 1), (x, y, 0)]
        bx, by = (160, 99) if ANSWER[0] == 0 else (226, 99)
        ps += [(bx, by, 1), (bx, by, 0)]
        if ANSWER[0] == 0:
            for _ in range(2):
                p = point(r.randrange(60))
                ps.append((p[0], p[1], 0))
        return ps
    elif kind == "plants":
        # A left press on tab 3 (190..225 x 155..164) shows the processing page, which waits for the left button
        # to come up; moves during the hold must not frame a sector. The release ends the page; the map tracks
        # again after it.
        x, y = 190 + r.randrange(36), 155 + r.randrange(10)
        ps.append((x, y, 1))
        for _ in range(r.randrange(1, 4)):
            if r.random() < .5:
                p = point(r.randrange(60))
            else:
                p = (x + r.randrange(-8, 9), y + r.randrange(-6, 7))
            ps.append((p[0], p[1], 1))
        ps.append((x, y, 0))
        for _ in range(2):
            p = point(r.randrange(60))
            ps.append((p[0], p[1], 0))
        return ps
    elif kind == "finance":
        # A left press on the number field 4 box (233..267 x 180..188) opens the finance page. The original runs
        # its back-button wait in a loop of its own: the release of the field 4 press is its first poll, then a
        # press on the back button (74..99 x 153..168) ends it and the page flips back. The port acts on the two
        # releases, so the same four polls run.
        x, y = 233 + r.randrange(35), 180 + r.randrange(9)
        ps.append((x, y, 1))
        ps.append((x, y, 0))
        bx, by = 74 + r.randrange(26), 153 + r.randrange(16)
        ps.append((bx, by, 1))
        ps.append((bx, by, 0))
    elif kind != "none":
        # First, a left press on the start button (281..306 x 176..191) with the left button up again: message
        # 14 (cur_sector 0xFF and a team on duty), message 15 (the balance below 0) or, with a valid sector and
        # a balance, the hint (messages 16+17). A box closes on a new press, then runs until the button is up.
        x, y = 281 + r.randrange(26), 176 + r.randrange(16)
        if edge.random() < 0.5:                     # own stream: the corners of the button, and the rows and
            x, y = edge.choice([281, 306]), edge.choice([176, 191])
        if edge.random() < 0.6:                     # columns just outside it, where no box may open
            for ox, oy in ((edge.choice([280, 307]), y), (x, edge.choice([175, 192]))):
                ps.append((ox, oy, 1))
                ps.append((ox, oy, 0))
        ps.append((x, y, 1))
        ps.append((x, y, 0))
        ps.append((x, y, 1))
        ps.append((x, y, 0))
        if kind == "hint":
            # The two hint boxes close on the third and fourth start-area presses. The fifth start press then
            # takes the day start path (hinted is set): its Get to work? question is the NO stub answer, so the
            # loop goes on.
            ps.append((x, y, 1))
            ps.append((x, y, 0))
            ps.append((x, y, 1))
            ps.append((x, y, 0))

    def move():
        s = r.randrange(60)
        ps.append((point(s)[0], point(s)[1], 0))

    for _ in range(r.randrange(1, 5)):
        for _ in range(r.randrange(0, 3)):
            move()
        p = point(r.randrange(60))
        ps.append((p[0], p[1], 0))
        btn = r.choice([1, 1, 2, 3])
        ps.append((p[0], p[1], btn))
        ps.append((p[0], p[1], 0))               # the release: WaitLeftUp polls it
        if r.random() < .6:                      # a box, if one opened, closes on this press
            q = point(r.randrange(60))
            ps.append((q[0], q[1], 1))
            ps.append((q[0], q[1], 0))
        move()
        move()
        if r.random() < .5:
            x0, y0, x1, y1 = TABRECT[r.randrange(len(TABRECT))]
            x, y = r.randrange(x0, x1 + 1), r.randrange(y0, y1 + 1)
            ps.append((x, y, 1))
            ps.append((x, y, 0))
    move()
    return ps


def fuzz(r, kind):
    tbl = bytearray()
    owned_p = r.choice([.5, .8, 1.0])
    owned = []
    for s in range(60):
        f = 1 if r.random() < owned_p else 0
        if f:
            owned.append(s)
        rec = [f, r.randrange(9), r.choice([0, 0, 1, 4, 254, 255, r.randrange(256)]),
               r.choice([0, 1, 2, 3, 6, 9, 12, 252, 253, 254, 255, r.randrange(256)]),
               r.choice([0, 1, 7, 8, 9, 255, r.randrange(256)]), r.choice([0, 0, r.randrange(256)]),
               r.choice([0, 0, 1, 3, 5, 8, 10, r.randrange(256)]), 0, 0]
        rec[7] = r.randrange(rec[6] + 1) if r.random() < .7 else r.randrange(256)
        rec[8] = r.randrange(rec[6] + 1)
        tbl += bytes(rec)
    tbl[59 * 9] |= 1
    if kind == "wagechange":
        # No tappers and no guards: the recomputed worker pool can not fall below the workers, so WorkerPool
        # calls no MercsQuitMsg box.
        for s in range(60):
            tbl[9 * s + 3] = 0
            tbl[9 * s + 4] = 0
        ANSWER[0] = r.choice([0, 0, 1])
    e.uc.mem_write(e.rt(SECTORS), bytes(tbl))
    cur, team = r.choice([0xFF, 0xFF, r.randrange(60)]), r.choice([0, 1, 1, 2, 3, 8])
    if kind == "assign":
        # Both sides of the reset: a valid sector or none, with a team or none.
        cur, team = r.choice([0xFF, r.randrange(60), r.randrange(60)]), r.choice([0, 0, 1, 2, 8])
    balance = r.choice([0, 1, 3000, r.randrange(1 << 31)])
    if kind == "14":
        # the first start check must answer 14 (starting sector not specified)
        cur, team = 0xFF, r.choice([1, 1, 2, 3, 8])
    elif kind == "15":
        # a valid sector and a team, so the balance answers 15 (costs are higher than you can afford)
        cur, team = r.randrange(60), r.choice([1, 1, 2, 3, 8])
        balance = -r.randrange(1, 100000)
    elif kind == "hint":
        # a valid sector, a team and a balance: the first start press shows messages 16 and 17, the next starts
        # the day question (hinted). The team is never 0: its path is the QuestionBox, not ConfirmDialog.
        cur, team = r.randrange(60), r.choice([1, 1, 2, 3, 8])
        balance = r.choice([0, 1, 3000, r.randrange(1 << 31)])
    e.w8(e.rt(CUR), cur)
    e.w32(e.rt(TEAM), team)
    e.w8(e.rt(HINTED), 0)
    e.w32(e.rt(BALANCE), balance & 0xFFFFFFFF)
    # The native wage: the max path of the wage box needs 100; a change needs a lower wage; the plain path takes
    # any wage.
    if kind == "wagemax":
        wage = 100
    elif kind == "wagechange":
        wage = r.randrange(100)
    else:
        wage = r.randrange(101)
    e.w32(e.rt(WAGE), wage)
    e.w32(e.rt(POOL), r.choice([15, 30, 100]))
    e.w32(e.rt(IDLE), r.choice([0, 1, 2, 3, 4, 10, 30]))
    e.uc.mem_write(e.rt(PLANTS), bytes(48))
    if r.random() < .5:
        for k in range(4):
            p = e.rt(PLANTS) + 12 * k
            e.w32(p, r.choice([0, 1, 3, 10, 100]))
            e.w32(p + 4, r.randrange(60))
            e.w8(p + 8, r.choice([2, 2, 0, 1]))
    e.uc.mem_write(e.rt(ORDER), bytes(range(60)) + bytes(60))
    mode = r.choice([0, 0, 1, 1, 2, 2, r.randrange(256)])
    e.w8(e.rt(MODE), mode)
    return mode


def original(ps):
    global tape, orig
    tape = ps
    orig = []
    limits.clear()
    pos[0] = cur_poll[0] = 0
    e.w32(e.rt(MX), 0xFFFFFFFF)
    e.w32(e.rt(MY), 0xFFFFFFFF)
    e.w32(e.rt(BUTTONS), 0)
    try:
        e.call(e.rt(ENTRY), 0, max_steps=3_000_000)
    except EmuStop as stop:
        if "tape end" not in str(stop):
            raise
    return list(orig), pos[0]


for i in range(ncases):
    kind = r.choice(["none", "none", "none", "none", "none", "none", "14", "14", "15", "hint", "hint", "hint",
                     "wage", "wage", "wagemax", "wagemax", "finance", "finance", "wagechange", "wagechange",
                     "plants", "plants", "assign", "assign", "assign"])
    mode = fuzz(r, kind)
    ps = a_tape(kind, e.r32(e.rt(TEAM)))
    if edge.random() < 0.25 and kind == "none":     # a press on the last row of the map and the first row of the panel,
        ax = point(edge.randrange(60))              # with a sector framed: the panel row must not click the sector
        ps = [(ax[0], ax[1], 0), (ax[0], edge.choice([140, 141]), 1), (ax[0], 141, 0), (ax[0], ax[1], 0)] + ps
    if edge.random() < 0.5 and kind == "none":      # a held press on the map, dragged: the loop stands still in WaitLeftUp
        ax, ay = point(edge.randrange(60))          # (no frames, no second click, no water); the other button may join
        btn = edge.choice([1, 1, 2])
        drag = [(ax, ay, 0), (ax, ay, btn)]
        for k in range(edge.randrange(4, 10)):
            qx, qy = point(edge.randrange(60))
            if k == 2 and edge.random() < 0.4:
                btn |= 3 - btn
            drag.append((qx, qy, btn))
        drag.append((qx, qy, 0))
        ps = drag + [(qx, qy, 0), (qx, qy, 1) , (qx, qy, 0)] + ps
    if len(ps) % 2:     # the row of the mouse limit change: 140 is over the map, 141 below it (no sector point reaches 140)
        ps = [(100, 140, 0), (100, 141, 0)] + ps
    sec0 = bytes(e.uc.mem_read(e.rt(SECTORS), 540))
    acc0 = bytes(e.uc.mem_read(e.rt(0x229FA0), 0xA8))
    pl0 = bytes(e.uc.mem_read(e.rt(PLANTS), 48))
    or0 = bytes(e.uc.mem_read(e.rt(ORDER), 61))
    cur0, team0 = e.r8(e.rt(CUR)), e.r32(e.rt(TEAM))
    try:
        lo, used = original(ps)
    except EmuStop as stop:
        # An unexpected stop (INT 3, INT 0x31 from unstubbed library code) leaves the machine broken: later
        # cases would all fail on the same stop, so end the run here.
        print(f"MISMATCH case {i} (kind {kind}): the original stopped at {stop}; the run ends here")
        bad += 1
        break
    sec_o = bytes(e.uc.mem_read(e.rt(SECTORS), 540))
    mouse_o = bytes(e.uc.mem_read(e.rt(MOUSE_NUM), 4))
    cur_o, mode_o, hinted_o = e.r8(e.rt(CUR)), e.r8(e.rt(MODE)), e.r8(e.rt(HINTED))
    wage_o = bytes(e.uc.mem_read(e.rt(WAGE), 4))
    pool_o = bytes(e.uc.mem_read(e.rt(POOL), 4))
    idle_o = bytes(e.uc.mem_read(e.rt(IDLE), 4))
    if not PORT:
        continue
    t = (ctypes.c_int32 * (3 * len(ps)))(*[v for q in ps for v in q])
    sec = (ctypes.c_uint8 * 720).from_buffer_copy(sec0 + bytes(720 - 540))
    pl = (ctypes.c_uint8 * 48).from_buffer_copy(pl0)
    order = (ctypes.c_uint8 * 61).from_buffer_copy(or0)
    acc = (ctypes.c_uint8 * 0xA8).from_buffer_copy(acc0)
    acc[0x0C] = team0 & 0xFF                       # team_on_duty, also a world scalar
    acc[0x99] = cur0
    cap = 8 * len(ps) + 8
    ev = (ctypes.c_int32 * (4 * cap))()
    ct = (ctypes.c_int32 * 6)()
    tx = (ctypes.c_uint8 * 2048)()
    rc = PORT.ja1f_map_pass(ISO, EXE, SAVE, sec, acc, pl, order, cur0, team0, mode, 0, t, len(ps), ev, cap, ct,
                            tx, 2048)
    got = []
    port_limits = []
    texts = bytes(tx)
    for k in range(ct[0]):
        typ, a, b, idx = ev[4 * k:4 * k + 4]
        if typ == 0:
            got.append(("f", a & 0xFF, b & 0xFF, idx))
        elif typ == 1:
            got.append(("m", texts[a:a + b].decode("latin-1"), idx))
        elif typ == 2:
            got.append(("g", a & 0xFFFFFFFF, b & 0xFFFFFFFF, idx))
        elif typ == 5:
            got.append(("p", 0, 0, idx))
        elif typ == 6:
            got.append(("q", 0, 0, idx))
        elif typ == 8:
            got.append(("r", 0, 0, idx))
        elif typ == 7:
            port_limits.append((a & 0xFFFF, (a >> 16) & 0xFFFF, b & 0xFFFF, (b >> 16) & 0xFFFF))
        else:
            got.append(("a", a & 0xFFFFFFFF, b & 0xFFFFFFFF, idx))
    # A stub that runs out of tape returns, so the original runs the rest of its round where the real one still waits:
    # a rotation at the very end of the tape does not compare.
    lo = [v for v in lo if v[:1] != ("r",) or v[3] < len(ps)]
    got = [v for v in got if v[:1] != ("r",) or v[3] < len(ps)]
    why = None
    if rc != 0:
        why = f"rc {rc}"
    elif port_limits != limits:
        why = f"mouse limits: original {limits}, port {port_limits}"
    elif got != lo:
        for k in range(max(len(got), len(lo))):
            g, o = (got[k] if k < len(got) else None), (lo[k] if k < len(lo) else None)
            if g != o:
                why = f"event {k}: original {o}, port {g}"
                break
    elif bytes(sec[:540]) != sec_o:
        j = next(j for j in range(540) if sec[j] != sec_o[j])
        why = f"sector {j // 9} byte {j % 9}: original {sec_o[j]}, port {sec[j]}"
    elif bytes(acc[0x70:0x74]) != mouse_o:
        why = f"mouse sector: original {int.from_bytes(mouse_o, 'little')}, port {int.from_bytes(bytes(acc[0x70:0x74]), 'little')}"
    elif ct[2] & 0xFF != cur_o:
        why = f"cur_sector: original {cur_o}, port {ct[2]}"
    elif ct[4] & 0xFF != mode_o:
        why = f"map mode: original {mode_o}, port {ct[4]}"
    elif ct[5] & 0xFF != hinted_o:
        why = f"hint flag: original {hinted_o}, port {ct[5]}"
    elif bytes(acc[0x54:0x58]) != wage_o:
        why = f"native wage: original {int.from_bytes(wage_o, 'little')}, port {int.from_bytes(bytes(acc[0x54:0x58]), 'little')}"
    elif bytes(acc[0x3C:0x40]) != pool_o:
        why = f"worker pool: original {int.from_bytes(pool_o, 'little')}, port {int.from_bytes(bytes(acc[0x3C:0x40]), 'little')}"
    elif bytes(acc[0x4C:0x50]) != idle_o:
        why = f"idle pool: original {int.from_bytes(idle_o, 'little')}, port {int.from_bytes(bytes(acc[0x4C:0x50]), 'little')}"
    elif ct[3] != used:
        why = f"polls used: original {used}, port {ct[3]}"
    if why:
        print(f"MISMATCH case {i} (mode {mode}): {why}; tape {ps}")
        bad += 1
        if bad > 10:
            break
print(f"strat_input_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
if os.environ.get("STRAT_DEBUG"):
    print(f"day questions asked (ConfirmDialog stubs): {ASKED[0]}")
    print(f"flip events seen (all cases): {FLIPS[0]}")
sys.exit(1 if bad else 0)
