#!/usr/bin/env bash
# Collects Rust coverage from the workspace tests and writes lcov.info at the
# repository root (the SonarQube job runs this; see AGENTS.md). Needs
# cargo-llvm-cov and the llvm-tools-preview component.
#
# Everything runs in this one shell: show-env prints shell-quoted variables
# (RUSTC_WRAPPER and friends) that must be sourced, not copied into
# $GITHUB_ENV, and every build uses the dev profile, since `report` reads the
# objects of one profile directory. The CLI integration tests run the binary
# Cargo builds for them, which is instrumented too; check_lcov.sh fails if
# its hits, or the shaders', are missing from the report.
set -euo pipefail

cd "$(dirname "$0")/.."
manifest=core/Cargo.toml

source <(cargo llvm-cov show-env --sh --manifest-path "$manifest")
cargo llvm-cov clean --workspace --manifest-path "$manifest"

# The workspace on the default backend, and the engine's portable fixtures
# (the libm-crate path in math.rs).
cargo test --locked --manifest-path "$manifest"
cargo test --locked --manifest-path "$manifest" -p simulacra_engine --features json,portable

cargo llvm-cov report --manifest-path "$manifest" --lcov --output-path lcov.info
cargo llvm-cov report --manifest-path "$manifest" --summary-only
sh core/check_lcov.sh lcov.info
