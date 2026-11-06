//! `PyRandom` against CPython: replays the call sequences in
//! `random_reference.json` (written by `make_random_reference.py` from
//! CPython's `random.Random`) and requires identical results.

use std::path::Path;

use serde::Deserialize;
use simulacra_engine::PyRandom;

#[derive(Deserialize)]
struct Reference {
    sequences: Vec<Sequence>,
}

#[derive(Deserialize)]
struct Sequence {
    seed: String,
    /// `[name, arguments..., result]`, all as strings: integers in decimal,
    /// floats as the hex of their bits.
    calls: Vec<Vec<String>>,
}

fn float(hex: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(hex, 16).expect("float bits"))
}

fn int<T: std::str::FromStr>(text: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    text.parse().expect("integer")
}

/// Replays one call and returns `(result, expected)` as comparable strings.
fn replay(rng: &mut PyRandom, call: &[String]) -> (String, String) {
    let expected = call.last().expect("result").clone();
    let got = match (call[0].as_str(), &call[1..call.len() - 1]) {
        ("random", []) => format!("{:016x}", rng.random().to_bits()),
        ("getrandbits", [k]) => rng.getrandbits(int(k)).to_string(),
        ("randint", [a, b]) => rng.randint(int(a), int(b)).to_string(),
        ("randrange", [stop]) => rng.randrange(int(stop)).to_string(),
        ("choice", [n]) => {
            let items: Vec<usize> = (0..int(n)).collect();
            rng.choice(&items).to_string()
        }
        ("uniform", [a, b]) => format!("{:016x}", rng.uniform(float(a), float(b)).to_bits()),
        _ => panic!("unknown call {call:?}"),
    };
    (got, expected)
}

#[test]
fn replays_cpython_sequences() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/random_reference.json");
    let text = std::fs::read_to_string(&path).expect("random_reference.json");
    let reference: Reference = serde_json::from_str(&text).expect("valid reference");
    assert!(reference.sequences.len() >= 10);
    let mut kinds = std::collections::BTreeSet::new();
    for sequence in &reference.sequences {
        let mut rng = PyRandom::new(int(&sequence.seed));
        for (i, call) in sequence.calls.iter().enumerate() {
            kinds.insert(call[0].clone());
            let (got, expected) = replay(&mut rng, call);
            assert_eq!(got, expected, "seed {}, call {i}: {call:?}", sequence.seed);
        }
    }
    assert_eq!(kinds.len(), 6, "every helper is covered: {kinds:?}");
}
