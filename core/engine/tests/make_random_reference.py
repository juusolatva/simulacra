"""Regenerate random_reference.json: CPython random.Random sequences for PyRandom.

For each seed, a fixed mix of calls (random, getrandbits, randint, randrange,
choice, uniform) and CPython's results. tests/py_random.rs replays the same
calls on simulacra_engine::PyRandom and requires identical results. The calls
are interleaved because _randbelow's rejection loop consumes a varying number
of draws, and each sequence is long enough to refill the 624-word state.

CPython's generator uses only integer arithmetic and one exact float scaling,
so its output is the same on every platform: unlike scene data, these
sequences can be committed. Run with CPython 3.12 or later:

    python3 core/engine/tests/make_random_reference.py
"""

import json
import pathlib
import random
import struct

OUT = pathlib.Path(__file__).resolve().parent / 'random_reference.json'
I64_MIN, I64_MAX = -(2 ** 63), 2 ** 63 - 1
# 0, small and negative seeds, both sides of 2**32 (one or two seed words)
# and the i64 extremes; abs(i64::MIN) needs two full words.
SEEDS = [0, 1, -1, 3, 42, 123456789, 2 ** 32 - 1, 2 ** 32, -(2 ** 32), 2 ** 32 + 1,
         2 ** 53 + 7, I64_MAX, I64_MIN, -1234567890123]
CALLS_PER_SEED = 400
BITS = [0, 1, 2, 7, 31, 32, 33, 52, 53, 63, 64, 65, 96, 127, 128]
RANGES = [(0, 0), (0, 1), (-5, 9), (1, 6), (0, 255), (-1000, 1000), (0, 2 ** 31),
          (0, 2 ** 32), (-(2 ** 40), 2 ** 40), (0, I64_MAX), (I64_MIN, I64_MAX),
          (I64_MIN, 0), (I64_MAX - 3, I64_MAX)]
STOPS = [1, 2, 3, 10, 100, 1000, 2 ** 31, 2 ** 32 + 5, 2 ** 62, I64_MAX]
UNIFORM = [(0.0, 1.0), (-1.0, 1.0), (0.5, 0.75), (-3.25, 7.5), (0, 360), (1e-3, 2e-3),
           (5.0, -5.0), (0.0, 0.0), (-1e9, 1e9)]


def _bits(value):
    return '%016x' % struct.unpack('<Q', struct.pack('<d', value))[0]


def _call(rng, kind, plan):
    """One call: its arguments as strings, then CPython's result as a string."""
    if kind == 'random':
        return ['random', _bits(rng.random())]
    if kind == 'getrandbits':
        k = plan.choice(BITS)
        return ['getrandbits', str(k), str(rng.getrandbits(k))]
    if kind == 'randint':
        a, b = plan.choice(RANGES)
        return ['randint', str(a), str(b), str(rng.randint(a, b))]
    if kind == 'randrange':
        stop = plan.choice(STOPS)
        return ['randrange', str(stop), str(rng.randrange(stop))]
    if kind == 'choice':
        n = plan.randint(1, 20)
        return ['choice', str(n), str(rng.choice(range(n)))]
    a, b = plan.choice(UNIFORM)
    return ['uniform', _bits(float(a)), _bits(float(b)), _bits(rng.uniform(a, b))]


def main():
    plan = random.Random(20261108)  # picks the calls, independent of the generators under test
    kinds = ['random', 'getrandbits', 'randint', 'randrange', 'choice', 'uniform']
    sequences = []
    for seed in SEEDS:
        rng = random.Random(seed)
        calls = [_call(rng, plan.choice(kinds), plan) for _ in range(CALLS_PER_SEED)]
        sequences.append({'seed': str(seed), 'calls': calls})
    lines = ['{', ' "sequences": [']
    for i, seq in enumerate(sequences):
        body = ',\n'.join('    ' + json.dumps(call) for call in seq['calls'])
        end = ',' if i < len(sequences) - 1 else ''
        lines.append(f'  {{"seed": "{seq["seed"]}", "calls": [\n{body}\n  ]}}{end}')
    lines += [' ]', '}']
    OUT.write_text('\n'.join(lines) + '\n')
    print(f'wrote {len(SEEDS)} sequences of {CALLS_PER_SEED} calls to {OUT.name}')


if __name__ == '__main__':
    main()
