"""Start the original in DOSBox-X and drive it to tactical in home sector 60 from the QICKSTRT day-0 save (slot 0).
Every step waits on game state through tools/jactl.py: the current screen (Game.screen(), read from the live call
stack: screens are nested function loops) or a known variable; no fixed delays. Ends with the save state "tac60"
(./t jactl load tac60 to come back here).

usage: .venv/bin/python validate/to_tactical.py [--no-start]     (--no-start: DOSBox-X already running)
"""
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tools"))
from jactl import Game, Live  # noqa: E402


def wait_loaded(timeout=60):
    """Until DOSBox-X runs and JA.EXE's objects are in guest RAM."""
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        try:
            if Live().locate():
                return
        except RuntimeError:
            pass
        time.sleep(0.5)
    raise TimeoutError("JA.EXE did not load")


def until(g, act, done, timeout=60, retry=1.5):
    """Repeat act() whenever the screen has not reacted within `retry` s, until done() holds. Input is sometimes
    lost (the first keys after start; a click just as the main menu comes up), and repeating it is harmless as
    long as nothing changed."""
    end = time.monotonic() + timeout
    while not done():
        if time.monotonic() > end:
            raise TimeoutError(f"screen stuck at {g.screen()}")
        before = g.screen()
        act()
        try:
            g.wait(lambda: done() or g.screen() != before, retry)
        except TimeoutError:
            pass


def screen_is(g, name):
    return lambda: g.screen() == name


def dismiss(g, kinds, done, timeout=60):
    """Answer each modal box of `kinds` (speech, message) with Return until done(). Boxes can follow each other
    directly (same screen name), so a box counts as gone when the call path to it changes."""
    def where():                    # frames down to the box's caller (its return address = the call site)
        out = []
        for _, r, f, _ in g.stack():
            if f in g.SCREENS and g.SCREENS[f][0] in kinds:
                break
            out.append((f, r))
        return out
    end = time.monotonic() + timeout
    while not done():
        if time.monotonic() > end:
            raise TimeoutError(f"stuck at {g.screen()}")
        if g.screen() in kinds:
            box = where()
            until(g, lambda: g.key("Return"), lambda: done() or g.screen() not in kinds or where() != box, 20)
        time.sleep(0.1)


def main():
    if "--no-start" not in sys.argv:
        subprocess.run([os.path.join(ROOT, "validate", "run_x.sh")], check=True)
    wait_loaded()
    g = Game()
    g.wait(lambda: g.at("GetKey"), 30)                        # "Press any key"
    until(g, lambda: g.key("Escape"), screen_is(g, "menu"), 90)  # -> title/intro parts -> main menu
    until(g, lambda: g.click(140, 74), screen_is(g, "office"))  # Continue Game
    until(g, lambda: g.click(197, 72), screen_is(g, "slots"))   # calendar bottom: Restore Game
    until(g, lambda: g.click(160, 43), lambda: g.screen() != "slots")  # slot 0 "Day 0 Quick Start"
    g.wait("g_day == 0 and g_soldiers[0].nickname == 'Grunty' and screen() == 'office'", 20)
    until(g, lambda: g.click(50, 142), lambda: g.v.g_day == 1)  # bed: Sleep -> sunrise -> strategic
    dismiss(g, ("speech",), lambda: g.screen() == "strategic")  # guide speech while the map sets up
    g.click(39, 168)                                           # TEAM
    g.wait("screen() == 'strategic'", 5)
    g.click(300, 128)                                          # home sector 60
    g.wait("g_curSector == 59", 10)
    until(g, lambda: g.click(290, 191), screen_is(g, "message"))  # OK -> assignment hints
    dismiss(g, ("message",), lambda: g.screen() == "strategic")
    until(g, lambda: g.click(290, 191), screen_is(g, "confirm"))  # OK -> "Get to work?"
    until(g, lambda: g.click(226, 107), # OK -> travel animation (ConfirmDialog? calling from 0x2580D) or already tactical
          lambda: g.at("ConfirmDialog?", 0x2580D) or g.screen().startswith("tactical"))
    g.wait("screen() == 'tactical'", 60)                       # TacticalLoop past sector load + fade
    g.wait("g_soldiers[0].tile < 0xC40 and g_tileMap[g_soldiers[0].tile].occupant == 0", 5)
    print(g.state())
    g.shot("tac_start")
    g.save_state("tac60", "to_tactical.py: day 1, tactical home sector 60, QICKSTRT team, turn not started")


if __name__ == "__main__":
    main()
