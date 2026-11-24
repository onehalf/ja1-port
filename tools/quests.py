"""Quest status from a save: the 43 story bytes as quest chains (docs/story_map.md).

usage: ./t quests SAVE [--bytes] [--json]
       ./t quests --campaign DIR [--log FILE] [--json]
       ./t quests --check
  SAVE = any day, office or quick save of the port or the original (layout from savefile.py).
  default: one line per chain A..L: not started / open / done / failed, plus its counters and the quest items
    the squad holds.
  --bytes: also every story byte that is not 0, with its meaning from docs/story_map.md.
  --campaign DIR: logs each new state of the saves the game writes in DIR (GAME0-9, DAY, OFFICE, QUICK; not the
    DAY_*.SAV fixtures) to FILE (default build/quest_log.jsonl), because the game writes over DAY.SAV every day;
    then prints the first day each chain reached each status over the whole log (coverage by real play).
  --json: the same as JSON.
  --check: every chain and byte of docs/story_map.md has a rule here and every rule byte has a map row; a built-in
    sample of story blocks gives the expected statuses (exit 1 on a fail).

Names and byte meanings come from docs/story_map.md only (the chain table and the byte table). This file holds the
status rules: which bytes give the stage of a chain, and which bytes count as fail or cost.
Byte 0x19 is merc_tire (story_map.md "Aliases"); a save writes the field there, so the save view is right.
"""
import json
import os
import re
import sys
from common import ROOT

STORY_ADDR, STORY_LEN = 0x22A040, 43
MAP = os.path.join(ROOT, "docs", "story_map.md")

# Item numbers per chain (story_map.md "Items" column): the squad holds them -> shown with the status.
ITEMS = {"A": (0x79,), "E": (0x77,), "G": (0x86, 0x7A, 0x5C), "H": (0x51, 0x5E), "I": (0x6E, 0x7C), "J": (0x5F, 0x7B)}


def stage(s, k):
    return {0: "not started", 1: "open", 2: "done"}.get(s[k], f"stage {s[k]}?")


def rule_a(s):
    return stage(s, 5), {"pickup quote said": s[0x1A]}


def rule_b(s):
    return stage(s, 6), {"countdown S[2]": s[2], "Brenda water message": s[0x25], "merc_tire": s[0x19]}


def rule_c(s):
    return stage(s, 7), {"countdown S[2]": s[2], "merc_tire": s[0x19]}


def rule_d(s):
    return stage(s, 8), {"days cut off": s[0x24], "reminder due": s[0x12], "secured once": s[0x13]}


def rule_e(s):
    return stage(s, 0x0C), {"days missing": s[0x18]}


def rule_f(s):
    return stage(s, 9), {"bonus days left": s[0x0F], "reminders": s[0x26]}


def rule_g(s):
    st = "failed (Brenda dead)" if s[0x15] else stage(s, 0x0B)
    sec = "none" if s[0x17] in (0, 0xFF) else f"P {s[0x17]}"
    return st, {"days missing": s[0x14], "found": s[0x16], "object sector": sec, "sign quote said": s[0x1B]}


def rule_h(s):
    return stage(s, 0x0A), {"antidote used": s[0x20], "Brenda makes it": s[0x21], "days to antidote": s[0x22],
                            "spread warning": s[0x23]}


def rule_i(s):
    st = stage(s, 0x0E)
    if s[0x1E]:
        st = "failed (journal burned)" if st == "done" else st + ", journal burned"
    return st, {"picked up": s[0x1C]}


def rule_j(s):
    st = "open" if s[0x1F] else "not started"
    if s[0x1D]:
        st = "sapling found"
    return st, {"hint said": s[0x1F], "pickup quote said": s[0x1D]}


def rule_k(s):
    st = {0: "not started", 4: "done"}.get(s[0x27], f"open (stage {s[0x27]})")
    day = s[0x28] - 256 if s[0x28] > 127 else s[0x28]
    return st, {"day counter": day, "drop said": s[0x29], "raise said": s[0x2A]}


def rule_l(s):
    return {0: "not started", 1: "open (warned)", 2: "done"}.get(s[0x0D], f"stage {s[0x0D]}?"), {}


RULES = {"A": rule_a, "B": rule_b, "C": rule_c, "D": rule_d, "E": rule_e, "F": rule_f, "G": rule_g, "H": rule_h,
         "I": rule_i, "J": rule_j, "K": rule_k, "L": rule_l}
# Bytes each rule reads (for --check against the map's Bytes column and byte table).
RULE_BYTES = {"A": {5, 0x1A}, "B": {6, 2, 0x25, 0x19}, "C": {7, 2, 0x19}, "D": {8, 0x24, 0x12, 0x13}, "E": {0x0C, 0x18},
              "F": {9, 0x0F, 0x26}, "G": {0x0B, 0x14, 0x15, 0x16, 0x17, 0x1B}, "H": {0x0A, 0x20, 0x21, 0x22, 0x23},
              "I": {0x0E, 0x1C, 0x1E}, "J": {0x1D, 0x1F}, "K": {0x27, 0x28, 0x29, 0x2A}, "L": {0x0D}}
NOT_CHAINS = {"M"}          # Santino and the win: no story byte (story_map.md)


def read_map():
    """(chains {letter: (name, bytes set)}, bytes {index: (chain column, meaning)}) from docs/story_map.md."""
    chains, rows = {}, {}
    for line in open(MAP, encoding="utf-8"):
        m = re.match(r"\| ([A-M])\. (.+?) \| ([^|]*) \|", line)
        if m:
            bs = {int(x, 16) if x.startswith("0x") else int(x) for x in re.findall(r"0x[0-9A-Fa-f]+|\b\d+\b", m.group(3))
                  if not m.group(3).startswith("none")}
            chains[m.group(1)] = (m.group(2), bs)
        m = re.match(r"\| 0x([0-9A-F]{2}) \| ([^|]+) \| ([^|]+) \|", line)
        if m:
            rows[int(m.group(1), 16)] = (m.group(2).strip(), m.group(3).strip())
    return chains, rows


def held_items(save):
    """Item types in the hands and pockets of player mercs 0..7."""
    from jatypes import Types
    T, r = Types(), save.reader()
    sol = T.view(r, "g_soldiers")
    out = set()
    for i in range(8):
        if sol[i].hpMax:
            out.update(sol[i].items[j].type for j in range(12))
    return out


def load(path):
    """(story bytes, day, difficulty, held item types) of a save."""
    return load_bytes(open(path, "rb").read())


def load_bytes(data):
    """load() of save bytes."""
    from savefile import Save
    from jatypes import Types
    s = Save(data)
    r, T = s.reader(), Types()
    return bytes(r.read(STORY_ADDR, STORY_LEN)), T.view(r, "g_day"), T.view(r, "g_difficulty"), held_items(s)


def status(story, held=frozenset()):
    """{letter: {name, status, counters, items}} for chains A..L."""
    chains, _ = read_map()
    out = {}
    for k, rule in RULES.items():
        st, counters = rule(story)
        out[k] = {"name": chains[k][0], "status": st, "counters": counters,
                  "items": [f"0x{i:02X}" for i in ITEMS.get(k, ()) if i in held]}
    return out


def kind(st):
    """The status word the coverage counts: not started / open / done / failed (sapling found counts as open)."""
    for w in ("failed", "done", "not started"):
        if st.startswith(w):
            return w
    return "open"


def show(path, want_bytes, as_json):
    story, day, diff, held = load(path)
    res = status(story, held)
    if as_json:
        print(json.dumps({"save": path, "day": day, "difficulty": diff, "story": list(story), "chains": res}))
        return
    print(f"{path}: day {day}, difficulty {diff}")
    for k, c in res.items():
        cnt = ", ".join(f"{n} {v}" for n, v in c["counters"].items() if v not in (0, "none"))
        it = f"  holds {' '.join(c['items'])}" if c["items"] else ""
        print(f"  {k}. {c['name'][:44]:44s} {c['status']:24s} {cnt}{it}")
    if want_bytes:
        _, rows = read_map()
        print("  story bytes not 0:")
        for i, v in enumerate(story):
            if v:
                ch, mean = rows.get(i, ("?", "no map row"))
                print(f"    0x{i:02X} = {v:3d}  {ch:6s} {mean}")


GAME_SAVE = re.compile(r"^(GAME\d|DAY|OFFICE|QUICK)\.SAV$", re.I)    # the files the game writes (not DAY_*.SAV fixtures)
LOG = os.path.join(ROOT, "build", "quest_log.jsonl")


def log_saves(d, log=LOG):
    """Append one log row per game-written save in DIR whose story, day or held items are new; the rows in the log."""
    rows = []
    if os.path.exists(log):
        rows = [json.loads(x) for x in open(log, encoding="utf-8") if x.strip()]
    known = {(r["file"], r["mtime"]) for r in rows}
    states = {(r["day"], tuple(r["story"]), tuple(r["held"])) for r in rows}
    new = []
    for f in sorted(os.listdir(d)):
        p = os.path.join(d, f)
        if not GAME_SAVE.match(f) or not os.path.isfile(p) or (f, os.path.getmtime(p)) in known:
            continue
        try:
            story, day, diff, held = load(p)
        except (SystemExit, KeyError, ValueError):
            continue                      # not a day or quick save, or a file the game still writes
        held = sorted(i for k in ITEMS.values() for i in k if i in held)
        st = (day, tuple(story), tuple(held))
        if st in states:
            continue
        states.add(st)
        new.append({"file": f, "mtime": os.path.getmtime(p), "day": day, "difficulty": diff, "story": list(story),
                    "held": held})
    if new:
        os.makedirs(os.path.dirname(log), exist_ok=True)
        with open(log, "a", encoding="utf-8") as fh:
            for r in new:
                fh.write(json.dumps(r) + "\n")
    return rows + new


def coverage(rows):
    """{(chain, status kind): (day, file)}: the first day each chain reached each status, plus the newest row."""
    first = {}
    for r in sorted(rows, key=lambda r: (r["day"], r["mtime"])):
        for k, c in status(bytes(r["story"]), set(r["held"])).items():
            first.setdefault((k, kind(c["status"])), (r["day"], r["file"]))
    return first


def campaign(d, as_json, log=LOG):
    """First day per (chain, status kind) over every logged save of DIR (the log keeps days the game overwrote)."""
    rows = log_saves(d, log)
    first = coverage(rows)
    newest = max(rows, key=lambda r: r["mtime"]) if rows else None
    if as_json:
        print(json.dumps({"dir": d, "log": log, "rows": len(rows),
                          "newest": newest and {**newest, "chains": status(bytes(newest["story"]), set(newest["held"]))},
                          "first": {f"{k} {w}": v for (k, w), v in first.items()}}))
        return
    days = sorted(r["day"] for r in rows)
    print(f"{d}: {len(rows)} logged save states ({log}), days {days[0] if days else '-'}..{days[-1] if days else '-'}")
    chains, _ = read_map()
    for k in RULES:
        cells = []
        for w in ("open", "done", "failed"):
            v = first.get((k, w))
            cells.append(f"{w} day {v[0]} ({v[1]})" if v else f"{w} -")
        print(f"  {k}. {chains[k][0][:40]:40s} " + "  ".join(cells))


# --check sample: (edits {byte: value}, {chain: expected status})
SAMPLE = [({}, {k: "not started" for k in RULES}),
          ({5: 1, 6: 2, 7: 2, 2: 1}, {"A": "open", "B": "done", "C": "done"}),
          ({0x0B: 1, 0x15: 1, 0x14: 8}, {"G": "failed (Brenda dead)"}),
          ({0x0B: 2, 0x16: 1}, {"G": "done"}),
          ({0x0E: 2, 0x1E: 1}, {"I": "failed (journal burned)"}),
          ({0x0E: 1, 0x1E: 1}, {"I": "open, journal burned"}),
          ({6: 1}, {"B": "open", "C": "not started"}), ({7: 2, 8: 1}, {"B": "not started", "C": "done", "D": "open"}),
          ({0x1F: 1}, {"J": "open"}), ({0x1F: 1, 0x1D: 1}, {"J": "sapling found"}),
          ({0x27: 2}, {"K": "open (stage 2)"}), ({0x27: 4}, {"K": "done"}),
          ({0x0D: 1}, {"L": "open (warned)"}), ({0x0A: 1, 0x0C: 1, 8: 2, 9: 1}, {"H": "open", "E": "open", "D": "done",
                                                                                  "F": "open"})]


def check():
    chains, rows = read_map()
    bad = []
    for k, (name, bs) in chains.items():
        if k in NOT_CHAINS:
            continue
        if k not in RULES:
            bad.append(f"chain {k} ({name}) has no rule")
        elif bs - RULE_BYTES[k]:
            bad.append(f"chain {k}: map bytes {sorted(bs - RULE_BYTES[k])} not read by its rule")
    for k, bs in RULE_BYTES.items():
        for b in bs:
            if b not in rows:
                bad.append(f"chain {k}: rule byte 0x{b:02X} has no map row")
            elif k not in rows[b][0] and rows[b][0] != "alias":
                bad.append(f"chain {k}: byte 0x{b:02X} is chain {rows[b][0]} in the map")
    for b, (ch, _) in rows.items():
        for k in re.findall(r"\b[A-L]\b", ch):
            if b not in RULE_BYTES.get(k, ()):
                bad.append(f"map byte 0x{b:02X} (chain {k}) not read by rule {k}")
    for edits, want in SAMPLE:
        s = bytearray(STORY_LEN)
        for b, v in edits.items():
            s[b] = v
        got = status(bytes(s))
        for k, w in want.items():
            if got[k]["status"] != w:
                bad.append(f"sample {edits}: chain {k} = {got[k]['status']!r}, want {w!r}")
    z = [0] * STORY_LEN
    lrows = [{"file": "DAY.SAV", "mtime": 3, "day": 2, "story": z[:5] + [2] + z[6:], "held": []},
            {"file": "GAME1.SAV", "mtime": 1, "day": 1, "story": z[:5] + [1] + z[6:], "held": []},
            {"file": "OFFICE.SAV", "mtime": 2, "day": 1, "story": z[:5] + [1] + z[6:], "held": []}]
    cov = coverage(lrows)
    for key, want in ((("A", "open"), (1, "GAME1.SAV")), (("A", "done"), (2, "DAY.SAV")), (("B", "not started"), (1, "GAME1.SAV"))):
        if cov.get(key) != want:
            bad.append(f"coverage {key} = {cov.get(key)}, want {want}")
    if ("A", "failed") in cov or ("B", "open") in cov:
        bad.append(f"coverage has a status no row reached: {sorted(cov)}")
    for b in bad:
        print("FAIL", b)
    print(f"quests --check: {len(RULES)} chain rules, {len(rows)} map bytes, {len(SAMPLE)} samples, {len(bad)} fails")
    return not bad


def main():
    a = sys.argv[1:]
    as_json = "--json" in a
    if "--check" in a:
        sys.exit(0 if check() else 1)
    if "--campaign" in a:
        campaign(a[a.index("--campaign") + 1], as_json, a[a.index("--log") + 1] if "--log" in a else LOG)
        return
    files = [x for x in a if not x.startswith("--")]
    if not files:
        sys.exit(__doc__)
    for f in files:
        show(f, "--bytes" in a, as_json)


if __name__ == "__main__":
    main()
