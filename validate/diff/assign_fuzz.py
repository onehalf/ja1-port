"""Fuzzed team slots for the assignment screen tests (assign_diff, duty_diff)."""
import struct

NICK_CHARS = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!?.-' "


def fuzz_soldier(r, s, k, n):
    s[0x182] = 1 if k < n else (r.choice([0, 0, 1]) if k == 8 else 0)
    s[0x17E] = r.choice([0, 0, 1, 2, 3, 3, 4, 5, 5, 6, 7])
    s[0x199] = r.randrange(5)
    hm = r.randint(1, 100)
    hp = r.randint(0, hm)
    s[0x184], s[0x183], s[0x185] = hm, hp, r.randint(0, hm - hp)
    if r.random() < 0.1:
        s[0x183], s[0x184], s[0x185] = (r.randrange(128) for _ in range(3))
    for o in range(0x18B, 0x192):
        s[o] = r.randint(0, 100)
    s[0x192] = r.randint(1, 10)
    if r.random() < 0.3:           # small doctor caps and repair values (the digit count of c)
        s[0x18E], s[0x190] = r.randint(1, 25), r.randint(1, 25)
    if r.random() < 0.15:          # cap 10 and repair value 10 exactly (d = 1 / 2 border)
        s[0x18C], s[0x192], s[0x18E], s[0x190] = 70, 1, 10, 10
        s[0x18D] = 50 + (100 - 70) * 20 // 100 + r.choice([0, 0, 5, -5])
    if r.random() < 0.3:
        s[0x18E] = 0
    if r.random() < 0.3:
        s[0x190] = 0
    s[0x112:0x114] = struct.pack("<H", r.choice([r.randrange(65536), r.randrange(2000), r.randrange(100)]))
    for i in range(12):
        if r.random() < (0.9 if i == 0 else 0.3):
            s[i * 12:i * 12 + 12] = bytes(r.randrange(256) for _ in range(12))
            s[i * 12] = r.choice([0x49, 0x4A, 0x4B, 0x49, 0x4A, 0x4B, r.randrange(256)])
            s[i * 12 + 1] = r.randint(-2, 4) & 0xFF
            s[i * 12 + 2] = r.randint(0, 100)
            for j in range(8, 12):
                s[i * 12 + j] = r.randint(0, 100)
    s[0x17A] = k if r.random() < 0.8 else r.randrange(8)
    s[0x179] = r.randrange(64)
    nick = bytes(r.choice(NICK_CHARS) for _ in range(r.randint(1, 8)))
    s[0x16E:0x177] = (nick + b"\0" * 9)[:9]
