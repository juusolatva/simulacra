//! CPython's `random.Random`, ported draw for draw so a seed gives the same
//! numbers in Rust as in Python.
//!
//! The generator is MT19937 as in `Modules/_randommodule.c` (`init_genrand`,
//! `init_by_array`, `genrand_uint32`, `random_seed`, `random()` and
//! `getrandbits()`), and the helpers follow `Lib/random.py` (`_randbelow`,
//! `randrange`, `randint`, `choice` and `uniform`). Both files make the same
//! draws in CPython 3.12 and 3.14. Only integer arithmetic and one exact float
//! scaling are involved, so the sequences are the same on every platform and
//! under either math backend.

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

/// A `random.Random(seed)` with the methods Simulacra's scene builders use.
#[derive(Clone)]
pub struct PyRandom {
    state: [u32; N],
    index: usize,
}

impl PyRandom {
    /// `random.Random(seed)`. CPython seeds with `abs(seed)` split into 32-bit
    /// words from the least significant end (one word for 0), so `seed` and
    /// `-seed` give the same sequence. `i64::MIN` is handled through
    /// `unsigned_abs`, whose magnitude does not fit in an `i64`.
    #[must_use]
    pub fn new(seed: i64) -> Self {
        let magnitude = seed.unsigned_abs();
        let low = (magnitude & 0xffff_ffff) as u32;
        let high = (magnitude >> 32) as u32;
        if high == 0 {
            Self::from_key(&[low])
        } else {
            Self::from_key(&[low, high])
        }
    }

    /// `init_by_array` over `key`, the seed's 32-bit words, least significant first.
    fn from_key(key: &[u32]) -> Self {
        let mut rng = Self::from_u32(19_650_218);
        let mt = &mut rng.state;
        let (mut i, mut j) = (1_usize, 0_usize);
        for _ in 0..N.max(key.len()) {
            let prev = mt[i - 1] ^ (mt[i - 1] >> 30);
            // `j` is below `key.len()`, so the u32 cast matches CPython's `(uint32_t)j`.
            mt[i] = (mt[i] ^ prev.wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                mt[0] = mt[N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..N - 1 {
            let prev = mt[i - 1] ^ (mt[i - 1] >> 30);
            mt[i] = (mt[i] ^ prev.wrapping_mul(1_566_083_941)).wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                mt[0] = mt[N - 1];
                i = 1;
            }
        }
        mt[0] = 0x8000_0000; // MSB is 1, so the initial array is not all zero.
        rng
    }

    /// `init_genrand(s)`.
    fn from_u32(seed: u32) -> Self {
        let mut state = [0_u32; N];
        state[0] = seed;
        for i in 1..N {
            let prev = state[i - 1] ^ (state[i - 1] >> 30);
            state[i] = 1_812_433_253_u32.wrapping_mul(prev).wrapping_add(i as u32);
        }
        Self { state, index: N }
    }

    /// `genrand_uint32`: the next 32-bit output.
    pub fn next_u32(&mut self) -> u32 {
        if self.index >= N {
            self.twist();
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// Generates the next `N` words at once, as `genrand_uint32` does.
    fn twist(&mut self) {
        let mt = &mut self.state;
        let mag = |y: u32| if y & 1 == 0 { 0 } else { MATRIX_A };
        for kk in 0..N - M {
            let y = (mt[kk] & UPPER_MASK) | (mt[kk + 1] & LOWER_MASK);
            mt[kk] = mt[kk + M] ^ (y >> 1) ^ mag(y);
        }
        for kk in N - M..N - 1 {
            let y = (mt[kk] & UPPER_MASK) | (mt[kk + 1] & LOWER_MASK);
            mt[kk] = mt[kk + M - N] ^ (y >> 1) ^ mag(y);
        }
        let y = (mt[N - 1] & UPPER_MASK) | (mt[0] & LOWER_MASK);
        mt[N - 1] = mt[M - 1] ^ (y >> 1) ^ mag(y);
        self.index = 0;
    }

    /// `random()`: a float in [0, 1) from 53 random bits (two draws).
    pub fn random(&mut self) -> f64 {
        let a = self.next_u32() >> 5;
        let b = self.next_u32() >> 6;
        (f64::from(a) * 67_108_864.0 + f64::from(b)) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// `getrandbits(k)` for `k <= 128`: words are filled from the least
    /// significant end, and the last one keeps its most significant bits.
    ///
    /// # Panics
    ///
    /// If `k > 128`; Python's unbounded result does not fit in a `u128`.
    pub fn getrandbits(&mut self, k: u32) -> u128 {
        assert!(k <= 128, "getrandbits({k}) does not fit in a u128");
        if k == 0 {
            return 0;
        }
        if k <= 32 {
            return u128::from(self.next_u32() >> (32 - k));
        }
        let (mut result, mut left, mut shift) = (0_u128, k, 0_u32);
        while left > 0 {
            let mut word = self.next_u32();
            if left < 32 {
                word >>= 32 - left;
            }
            result |= u128::from(word) << shift;
            shift += 32;
            left = left.saturating_sub(32);
        }
        result
    }

    /// `_randbelow(n)` (`_randbelow_with_getrandbits`): uniform in `0..n`,
    /// drawing `n.bit_length()` bits until the value is below `n`.
    fn randbelow(&mut self, n: u128) -> u128 {
        debug_assert!(n > 0, "callers reject empty ranges");
        let k = u128::BITS - n.leading_zeros();
        loop {
            let r = self.getrandbits(k);
            if r < n {
                return r;
            }
        }
    }

    /// `randrange(stop)`: uniform in `0..stop`.
    ///
    /// # Panics
    ///
    /// If `stop <= 0` (Python raises `ValueError`).
    pub fn randrange(&mut self, stop: i64) -> i64 {
        assert!(stop > 0, "empty range for randrange({stop})");
        // Below `stop`, so it fits back into an i64.
        self.randbelow(stop as u128) as i64
    }

    /// `randint(a, b)`: uniform in `a..=b`, as `a + _randbelow(b - a + 1)`.
    ///
    /// # Panics
    ///
    /// If `b < a` (Python raises `ValueError`).
    pub fn randint(&mut self, a: i64, b: i64) -> i64 {
        assert!(b >= a, "empty range in randint({a}, {b})");
        // The width is at most 2**64, which fits in a u128 but not a u64.
        let width = (i128::from(b) - i128::from(a) + 1) as u128;
        // a + r <= b, so the sum fits back into an i64.
        (i128::from(a) + self.randbelow(width) as i128) as i64
    }

    /// `choice(seq)`: `seq[_randbelow(len(seq))]`.
    ///
    /// # Panics
    ///
    /// If `seq` is empty (Python raises `IndexError`).
    pub fn choice<'a, T>(&mut self, seq: &'a [T]) -> &'a T {
        assert!(!seq.is_empty(), "cannot choose from an empty sequence");
        // Below seq.len(), so it fits back into a usize.
        &seq[self.randbelow(seq.len() as u128) as usize]
    }

    /// `uniform(a, b)`: `a + (b - a) * random()`, in Python's operation order.
    /// Integer arguments in Python are converted exactly as long as they fit
    /// in 53 bits, which every builder's bounds do.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }
}

#[cfg(test)]
mod tests {
    use super::PyRandom;

    #[test]
    fn matches_cpython_for_seed_zero() {
        // python3 -c "import random; r = random.Random(0); print([r.getrandbits(32) for _ in range(3)], r.random())"
        let mut rng = PyRandom::new(0);
        let words: Vec<u32> = (0..3).map(|_| rng.next_u32()).collect();
        assert_eq!(words, [3_626_764_237, 1_654_615_998, 3_255_389_356]);
        assert_eq!(rng.random(), 0.890243920837131);
    }

    #[test]
    fn negative_seeds_match_their_magnitude() {
        for seed in [1_i64, 42, 1 << 32, i64::MAX] {
            let (mut pos, mut neg) = (PyRandom::new(seed), PyRandom::new(-seed));
            for _ in 0..5 {
                assert_eq!(pos.next_u32(), neg.next_u32(), "seed {seed}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "empty range")]
    fn randint_rejects_an_empty_range() {
        let _ = PyRandom::new(1).randint(3, 2);
    }

    #[test]
    #[should_panic(expected = "empty sequence")]
    fn choice_rejects_an_empty_slice() {
        let empty: [u8; 0] = [];
        let _ = PyRandom::new(1).choice(&empty);
    }
}
