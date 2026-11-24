r"""Speech: quote text (EDT/*QUOTE.EDT) + voice samples (CD archives AIM/ DAY/ PST/ JAC/ QST/ BRE/ NAT/ SAN/, NAMES/).

usage: ./t speech cats                          categories, record counts, archive dirs
       ./t speech text CAT [SPEAKER]           decoded quote text (CAT = AIM DAY PST ... ; SPEAKER = profile id)
       ./t speech wav CAT SPEAKER QUOTE [OUT]    voice sample -> WAV (default gfx/speech/CAT_SPEAKER_QUOTE.wav)
       ./t speech names SPEAKER                  members of the NAMES/<id> .VOC archive (short merc remarks)

Traced in JA.EXE (PlayQuote 0x24F08, signature in defs/functions.toml):
  text = record (row + quote) of 200 bytes in EDT\<CAT>QUOTE.EDT, row = speaker * stride (AIM 14, DAY 33, PST 15,
  else 0). Encoding: every byte except the space separator is stored +1 (WrapNextWord 0x36184 subtracts 1 per
  byte while splitting words); NUL-terminated, rest of the record is random filler. Decoded 0xB1 is a special
  glyph outside the font (width 1, shown here as '--'); 0x7E..0x8B are the game font's Cyrillic glyphs (Ivan's
  Russian lines, DAY 594..626). 'MEMBER' etc. are placeholders replaced at run time (StrReplace 0x252B0).
  voice = archive file <CAT>\<speaker> (JAC with quote >= 80: JAC\64B; NAT: NAT\NAT, no speaker prefix), member
  "<speaker><CAT><quote>.8"; if absent, the quote is split into "...A.8" + "...B.8" played in sequence.
  .8 = raw unsigned 8-bit mono PCM, 22050 Hz (every speech stream call passes 0x5622 to SpeechStream 0x5CC6C).
  INV/SIG/INT are text only (no CD archive; INV voice lives on the hard disk in INV/, see REVERSE.md).
"""
import os, struct, sys
from common import ROOT, cd_read
from datarc import members

RATE = 22050
REC = 200
# index = PlayQuote category argument (table at 0x24BA0 / 0x24C2F); stride = row multiplier at the call sites
CATS = ["AIM", "DAY", "PST", "JAC", "INV", "SIG", "QST", "BRE", "NAT", "SAN", "INT"]
STRIDE = {"AIM": 14, "DAY": 33, "PST": 15}
ARCHIVE = {"AIM", "DAY", "PST", "JAC", "QST", "BRE", "NAT", "SAN"}


def edt(cat):
    return open(os.path.join(ROOT, "rom", "EDT", f"{cat}QUOTE.EDT"), "rb").read()


def decode(rec):
    raw = rec.split(b"\0")[0]
    return bytes(c if c == 0x20 else (c - 1) & 0xFF for c in raw)


def show(b):
    out = []
    for c in b:
        out.append("--" if c == 0xB1 else chr(c) if 0x20 <= c < 0x7E else f"\\x{c:02x}")
    return "".join(out)


def nicknames():
    """profile id -> nickname, from the initial profile table EDT\\NEWPROF.EDT (64 x struct profile)."""
    from jatypes import Types, Type, Bytes, View
    T = Types()
    d = open(os.path.join(ROOT, "rom", "EDT", "NEWPROF.EDT"), "rb").read()
    return {i: p.nickname for i, p in enumerate(View(Bytes(d), 0, Type("profile[64]", T)))}


def records(cat):
    d = edt(cat)
    return [decode(d[i:i + REC]) for i in range(0, len(d) - REC + 1, REC)]


def voice(cat, speaker, quote):
    """[pcm bytes] (one or two parts)"""
    if cat not in ARCHIVE:
        raise SystemExit(f"{cat}: text only, no voice archive")
    if cat == "NAT":
        path, prefix = "NAT/NAT", ""
    else:
        path, prefix = f"{cat}/{speaker}" + ("B" if cat == "JAC" and quote >= 80 else ""), str(speaker)
    data = cd_read(path)
    m = {n.upper(): (o, s) for n, o, s in members(data)}
    base = f"{prefix}{cat}{quote}"
    names = [base + ".8"] if base + ".8" in m else [base + "A.8", base + "B.8"]
    if not all(n in m for n in names):
        raise SystemExit(f"{path}: no member {base}.8 / {base}A.8+B.8")
    return [data[m[n][0]:m[n][0] + m[n][1]] for n in names]


def write_wav(path, pcm, rate=RATE):
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "wb") as f:
        f.write(b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVEfmt " +
                struct.pack("<IHHIIHH", 16, 1, 1, rate, rate, 1, 8) + b"data" + struct.pack("<I", len(pcm)) + pcm)


def main():
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    if not a:
        sys.exit(__doc__)
    cmd = a[0]
    if cmd == "cats":
        for i, c in enumerate(CATS):
            n = len(edt(c)) // REC
            st = STRIDE.get(c)
            print(f"{i:2d} {c}  {n:5d} records" + (f" = {n // st} speakers x {st}" if st else "") +
                  ("  voice: CD " + ("NAT/NAT" if c == "NAT" else f"{c}/<id>") if c in ARCHIVE else "  text only"))
    elif cmd == "text":
        cat = a[1].upper()
        recs = records(cat)
        st = STRIDE.get(cat)
        names = nicknames()
        rng = range(len(recs))
        if len(a) > 2:
            sp = int(a[2])
            rng = range(sp * st, (sp + 1) * st) if st else rng
        for i in rng:
            t = recs[i]
            who = f"{i // st:2d} {names.get(i // st, ''):8s} {i % st:2d}" if st else f"{i:3d}"
            print(f"{who}  {show(t)}")
    elif cmd == "wav":
        cat, sp, q = a[1].upper(), int(a[2]), int(a[3])
        pcm = b"".join(voice(cat, sp, q))
        out = a[4] if len(a) > 4 else os.path.join(ROOT, "gfx", "speech", f"{cat}_{sp}_{q}.wav")
        write_wav(out, pcm)
        print(f"{out}: {len(pcm)} bytes, {len(pcm) / RATE:.2f} s")
        st = STRIDE.get(cat)
        if os.path.exists(os.path.join(ROOT, "rom", "EDT", f"{cat}QUOTE.EDT")):
            row = sp * st if st else 0
            print(show(records(cat)[row + q]))
    elif cmd == "names":
        data = cd_read(f"NAMES/{int(a[1])}")
        for n, o, s in members(data):
            print(f"  {n:14s} {s:7d}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
