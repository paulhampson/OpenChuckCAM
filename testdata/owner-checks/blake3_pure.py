# SPDX-License-Identifier: AGPL-3.0-or-later
"""BLAKE3 (hash mode, 32-byte output) in plain Python, for the owner-check generators that write a
model too large to store: the project records each model's BLAKE3 hash, and Python's standard
library has none. It follows the BLAKE3 specification's reference implementation; it is slow
(about 1 MB a second) and used only when the `blake3` package is not installed. Written for this
repository (lane OWNERCHECKS); AGPL-3.0-or-later, as the rest of the repository.
"""
import struct

IV = (0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB,
      0x5BE0CD19)
PERM = (2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8)
CHUNK_START, CHUNK_END, PARENT, ROOT = 1, 2, 4, 8
M32 = 0xFFFFFFFF


def _compress(cv, block, counter, length, flags):
    m = list(struct.unpack("<16I", block))
    v = list(cv) + list(IV[:4]) + [counter & M32, (counter >> 32) & M32, length, flags]

    def g(a, b, c, d, x, y):
        v[a] = (v[a] + v[b] + x) & M32
        t = v[d] ^ v[a]
        v[d] = ((t >> 16) | (t << 16)) & M32
        v[c] = (v[c] + v[d]) & M32
        t = v[b] ^ v[c]
        v[b] = ((t >> 12) | (t << 20)) & M32
        v[a] = (v[a] + v[b] + y) & M32
        t = v[d] ^ v[a]
        v[d] = ((t >> 8) | (t << 24)) & M32
        v[c] = (v[c] + v[d]) & M32
        t = v[b] ^ v[c]
        v[b] = ((t >> 7) | (t << 25)) & M32

    for r in range(7):
        g(0, 4, 8, 12, m[0], m[1])
        g(1, 5, 9, 13, m[2], m[3])
        g(2, 6, 10, 14, m[4], m[5])
        g(3, 7, 11, 15, m[6], m[7])
        g(0, 5, 10, 15, m[8], m[9])
        g(1, 6, 11, 12, m[10], m[11])
        g(2, 7, 8, 13, m[12], m[13])
        g(3, 4, 9, 14, m[14], m[15])
        if r < 6:
            m = [m[i] for i in PERM]
    return [v[i] ^ v[i + 8] for i in range(8)]


def _chunk_output(data, counter):
    """The last block's inputs of one chunk (up to 1024 bytes): (cv, block, len, flags)."""
    cv = IV
    blocks = [data[i:i + 64] for i in range(0, len(data), 64)] or [b""]
    for i, block in enumerate(blocks):
        flags = (CHUNK_START if i == 0 else 0) | (CHUNK_END if i == len(blocks) - 1 else 0)
        padded = block.ljust(64, b"\0")
        if i == len(blocks) - 1:
            return cv, padded, counter, len(block), flags
        cv = _compress(cv, padded, counter, 64, flags)
    raise AssertionError


def _cv(output):
    cv, block, counter, length, flags = output
    return _compress(cv, block, counter, length, flags)


def hexdigest(data):
    """The BLAKE3 hash of `data` as 64 hexadecimal digits."""
    chunks = [data[i:i + 1024] for i in range(0, len(data), 1024)] or [b""]
    stack = []  # chaining values of complete subtrees, merged as in the reference
    for index, chunk in enumerate(chunks):
        output = _chunk_output(chunk, index)
        if index == len(chunks) - 1:
            break
        cv = _cv(output)
        total = index + 1
        while total & 1 == 0:
            cv = _compress(IV, struct.pack("<8I", *stack.pop()) + struct.pack("<8I", *cv), 0,
                           64, PARENT)
            total >>= 1
        stack.append(cv)
    while stack:
        right = _cv(output)
        output = (IV, struct.pack("<8I", *stack.pop()) + struct.pack("<8I", *right), 0, 64,
                  PARENT)
    cv, block, counter, length, flags = output
    return struct.pack("<8I", *_compress(cv, block, counter, length, flags | ROOT)).hex()
