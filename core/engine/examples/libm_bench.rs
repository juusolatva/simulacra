//! Times each function behind the `Libm` trait (src/math.rs) on the platform
//! libm and on the pure-Rust `libm` crate, in nanoseconds per call (best of
//! five passes over the same inputs), with the ratio of the two. Inputs span
//! ranges the shaders use: `exp` of negative arguments (Gaussians), `pow` of
//! bases in [0, 1], angles for the trigonometric functions. Used by
//! `core/bench_backends.sh` to explain style-level differences.
//!
//! ```text
//! cargo run --release --manifest-path core/Cargo.toml -p simulacra_engine --features portable \
//!     --example libm_bench [-- --calls N]
//! ```

// Calling both backends directly is the point here.
#![allow(clippy::disallowed_methods)]

use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

type Unary = fn(f64) -> f64;

/// Name, input range, platform call, `libm` crate call.
const FUNCTIONS: [(&str, f64, f64, Unary, Unary); 9] = [
    ("exp", -20.0, 0.0, f64::exp, libm::exp),
    ("sin", -10.0, 10.0, f64::sin, libm::sin),
    ("cos", -10.0, 10.0, f64::cos, libm::cos),
    ("tan", -1.5, 1.5, f64::tan, libm::tan),
    ("tanh", -3.0, 3.0, f64::tanh, libm::tanh),
    ("atan", -10.0, 10.0, f64::atan, libm::atan),
    ("atan2", -1.0, 1.0, |y| y.atan2(0.7), |y| libm::atan2(y, 0.7)),
    ("pow", 0.0, 1.0, |x| x.powf(1.7), |x| libm::pow(x, 1.7)),
    ("ln_1p", 0.0, 10.0, f64::ln_1p, libm::log1p),
];

fn nanoseconds_per_call(f: Unary, low: f64, high: f64, calls: usize) -> f64 {
    let step = (high - low) / calls as f64;
    let mut best = f64::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        let mut sum = 0.0;
        for i in 0..calls {
            sum += f(black_box(low + i as f64 * step));
        }
        black_box(sum);
        best = best.min(start.elapsed().as_secs_f64() * 1e9 / calls as f64);
    }
    best
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let calls = match args.as_slice() {
        [] => 20_000_000,
        [flag, n] if flag == "--calls" => match n.parse::<usize>() {
            Ok(n) if n > 0 => n,
            _ => {
                eprintln!("libm_bench: --calls: not a positive whole number: {n}");
                return ExitCode::from(2);
            }
        },
        _ => {
            eprintln!("usage: libm_bench [--calls N]");
            return ExitCode::from(2);
        }
    };
    println!("function\tplatform_ns\tlibm_crate_ns\tratio");
    for (name, low, high, platform, crate_fn) in FUNCTIONS {
        let a = nanoseconds_per_call(platform, low, high, calls);
        let b = nanoseconds_per_call(crate_fn, low, high, calls);
        println!("{name}\t{a:.2}\t{b:.2}\t{:.2}", b / a);
    }
    ExitCode::SUCCESS
}
