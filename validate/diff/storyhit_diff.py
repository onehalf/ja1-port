"""Differential test: the port's story-object hit (ja1_story_hit, library op 177, port/ja1logic/src/storyfind.rs)
vs the original StoryHit? 0x61D28 in Unicorn (ja1-re#61: until 2026-11-08 no front-end handled op 177, so an order
enemy shot Brenda's tile turn after turn and she lived; the original kills her on the first hit).

usage: .venv/bin/python validate/diff/storyhit_diff.py [N=1500] [DUMP ...] [--seed S] [-v] --port
Each case fuzzes the current sector, story byte 0x15, the sector's 5 story slots (+0x9F.., 0xAC..0xAF and others)
and their tiles (+0x82.., words; on the map or 0xC41), and the tile bytes 1 of those tiles. RadioResult? 0x63254 is
a stub that logs its call and returns a fuzzed step; the port runs with every op on the host. Compared: the call
list and the whole data object. Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
from emu import Emu                  # noqa: E402
import port_lib                      # noqa: E402

DUMPS = ["tac60"]
DATA_LO, DATA_HI = 0x90000, 0x33A730
STORY = 0x22A040
TILES, SECTORS, CUR = ai.G_TILEMAP, ai.G_SECTORS, ai.G_CUR_SECTOR
STORY_HIT, RADIO_RESULT = 0x61D28, 0x63254

args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args and args[0].isdigit() else 1500
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
if "--port" not in sys.argv:
    sys.exit("storyhit_diff: only --port (the Python mirror is tools/ai.py story_hit)")


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    rt = e.rt
    log, case = [], {}

    def radio(em, *_):
        log.append(("radio_result", []))
        return case["ret"]
    e.stub(rt(RADIO_RESULT), radio, pop=0)
    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    fails = done = 0
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        fuzz(e, rt, rng)
        case["ret"] = rng.choice([0, 1, 3, 4, 0xFF, rng.randrange(256)])
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        log.clear()
        e.call(rt(STORY_HIT), max_steps=1_000_000)
        log_o, post_o = list(log), bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        e.uc.mem_write(rt(DATA_LO), pre)
        log_p = port_run(e, case)
        post_p = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        done += 1
        if log_o != log_p or post_o != post_p:
            fails += 1
            if fails <= 5:
                print(f"MISMATCH case {i}: calls orig {log_o} port {log_p}")
                diffs = [j for j in range(len(post_o)) if post_o[j] != post_p[j]]
                if diffs:
                    print("  data " + ", ".join(f"0x{DATA_LO + j:X} {post_o[j]:02X}/{post_p[j]:02X}" for j in diffs[:8]))
        elif VERBOSE:
            print("ok", i, post_o[STORY + 0x15 - DATA_LO])
    return fails, done


def port_run(e, case):
    bufs, scalars = port_lib.world_load(e, ai)
    log = []

    def call(name, a):
        log.append((name, list(a)))
        return case["ret"], None
    port_lib.sector_run("story_hit", bufs, scalars, [], lambda n_: 0, call)
    port_lib.world_store(e, ai, bufs, scalars)
    return log


def fuzz(e, rt, rng):
    w8 = lambda a, v: e.w8(rt(a), v)            # noqa: E731
    w16 = lambda a, v: e.w16(rt(a), v)          # noqa: E731
    cur = rng.choice([rng.randrange(60), e.r8(rt(CUR)), 23])
    w8(CUR, cur)
    w8(STORY + 0x15, rng.choice([0, 0, 1, rng.randrange(256)]))
    r = SECTORS + 0xB1 * cur
    for k in range(5):
        v = rng.choice([0xAC, 0xAD, 0xAE, 0xAF, 0xAE, 0xAF, 0, 0xAB, 0xB0, 0x5C])
        # a story slot holds a tile on the map (the original writes past the map for 0xC41: not a case)
        tile = rng.randrange(3136) if v in (0xAC, 0xAD, 0xAE, 0xAF) else rng.choice([0xC41, rng.randrange(3136)])
        w8(r + 0x9F + k, v)
        w16(r + 0x82 + 2 * k, tile)
        if tile < 3136:
            w8(TILES + 8 * tile + 1, rng.choice([v, 0, rng.randrange(256)]))


def main():
    rng = random.Random(SEED)
    fails = done = 0
    per = max(N // len(dumps), 1)
    for d in dumps:
        if not os.path.exists(d):
            print("missing", d)
            sys.exit(2)
        f, c = run_dump(d, rng, per)
        fails += f
        done += c
    print(f"storyhit_diff --port: {done} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails or done < N // 2 else 0)


if __name__ == "__main__":
    main()
