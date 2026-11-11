#!/usr/bin/env bash
# Collects Rust coverage from every test set that exercises Rust and writes
# lcov.info at the repository root (the SonarQube job runs this; see AGENTS.md).
#
# Run from anywhere, with a virtual environment that has coverage and maturin
# (PYTHON, default .venv/bin/python) and cargo-llvm-cov with the
# llvm-tools-preview component. The Python suite's second run (with the core)
# is appended to .coverage, so run the core-less suite first if you want the
# combined Python report too.
#
# Everything runs in this one shell: show-env prints shell-quoted variables
# (RUSTC_WRAPPER and friends) that must be sourced, not copied into
# $GITHUB_ENV, and every build uses the dev profile, since `report` reads the
# objects of one profile directory.
set -euo pipefail

cd "$(dirname "$0")/.."
python=${PYTHON:-.venv/bin/python}
manifest=core/Cargo.toml

source <(cargo llvm-cov show-env --sh --manifest-path "$manifest")
cargo llvm-cov clean --workspace --manifest-path "$manifest"

# Rust tests: the workspace on the default backend, and the engine's portable
# fixtures (the libm-crate path in math.rs).
cargo test --locked --manifest-path "$manifest"
cargo test --locked --manifest-path "$manifest" -p simulacra_engine --features json,portable

# The Python suite against an instrumented core: the PyO3 adapter, the shaders
# through native == Python, the Rust scene builders.
"$python" -m maturin develop -m core/py/Cargo.toml
"$python" -m coverage run --append -m unittest -q
"$python" -m coverage xml
# The binary's tests below render through the extension too, so check now that
# the Python suite itself reached the adapter.
cargo llvm-cov report --manifest-path "$manifest" --lcov --output-path core/target/lcov-python.info
"$python" core/cli/tests/check_coverage.py python-lcov core/target/lcov-python.info

# The standalone binary, instrumented. check_coverage.py fails if no test ran
# or any was skipped.
cargo build --locked --manifest-path "$manifest" -p simulacra_cli
SIMULACRA_CLI=core/target/debug/simulacra "$python" core/cli/tests/check_coverage.py run-cli-tests

cargo llvm-cov report --manifest-path "$manifest" --lcov --output-path lcov.info
cargo llvm-cov report --manifest-path "$manifest" --summary-only
"$python" core/cli/tests/check_coverage.py lcov lcov.info
