# Rust coverage in SonarQube (PLAN.md Phase 6)

## Goal

Report Rust line coverage alongside Python coverage in SonarQube Cloud.
Today SonarQube counts 7,574 lines of Rust but no Rust lines to cover: its
99.1% coverage figure is Python alone (2,232 lines to cover), and the share of
logic in Rust keeps growing.

Success: the SonarQube analysis of main and of pull requests shows coverage
for the files under `core/engine/src`, `core/py/src` and `core/cli/src`,
measured from every test set that exercises them.

## Decisions

- **Which tests count:** all three test sets that exercise Rust:
  1. `cargo test` for the workspace (default backend), and
     `cargo test -p simulacra_engine --features json,portable` (the portable
     `libm` path and the fixtures);
  2. the Python suite against an instrumented `simulacra_core` (the PyO3
     adapter, the shaders through `test_native_styles_match_python_exactly`,
     the scene builders through `RustSceneBuilderTests`);
  3. `StandaloneCliTests` against an instrumented `simulacra` binary (option
     handling, messages, limits and writers in `core/cli/src`).
- **Where it runs:** in the existing job in `.github/workflows/sonarqube.yml`,
  the only consumer of the report. Rejected: a separate workflow handing over
  an artifact (plumbing without benefit) and an outside coverage service (a
  third-party token, and the figure would not sit next to Python's).
- **Tool:** `cargo-llvm-cov` (LLVM source-based coverage), installed with
  `taiki-e/install-action` pinned by commit SHA, with an exact tool version at
  least two weeks old, plus the `llvm-tools-preview` component.
- **Format:** LCOV, read by SonarQube through `sonar.rust.lcov.reportPaths`.

## CI steps (`sonarqube.yml`)

1. Install the Rust component and `cargo-llvm-cov`.
2. Run the Python suite without the core under `coverage` (unchanged).
3. Enable instrumentation: write the variables from `cargo llvm-cov show-env`
   into `$GITHUB_ENV`, then run `cargo llvm-cov clean --workspace`.
4. Run the two `cargo test` commands above.
5. Build an instrumented core with `maturin develop -m core/py/Cargo.toml` and
   run the Python suite again with `coverage run --append`, then write
   `coverage.xml` (unchanged).
6. Build an instrumented binary with `cargo build -p simulacra_cli` and run
   `StandaloneCliTests` with `SIMULACRA_CLI=core/target/debug/simulacra`.
7. Write `lcov.info` with `cargo llvm-cov report --lcov --output-path
   lcov.info`, and print a per-file summary to the job log.
8. The scanner reads `coverage.xml` and `lcov.info`.

`sonar-project.properties` gains `sonar.rust.lcov.reportPaths=lcov.info`, and
its comment saying Rust coverage is not collected is replaced.

## Points to verify during implementation

Each is checked locally before CI, by running the steps above end to end:

- maturin builds the extension with the instrumentation `RUSTFLAGS` from
  `show-env` (otherwise the adapter shows no coverage).
- `cargo llvm-cov report` merges the default and `portable` builds of the
  engine into one figure per source file.
- `StandaloneCliTests` with a debug binary finishes in reasonable time (a few
  minutes). If not, build the binary with `--release` and pass `--release` to
  `report` as well, or build everything in one profile.
- `lcov.info` holds covered lines from all three sources: nonzero hits in
  `core/py/src`, in `core/cli/src/main.rs` and in the shaders under
  `core/engine/src/styles`.
- The pull request's SonarQube analysis shows Rust coverage.

## Consequences

- The overall coverage figure drops from Python's 99.1% to the combined
  figure. Record the measured Rust figure in PLAN.md.
- New Rust code on pull requests becomes subject to the quality gate's
  new-code coverage condition (80%). The gate itself is not changed.
- The SonarQube job takes longer (instrumented builds and the extra `cargo
  test` and CLI runs); record the time.

## Documentation

- `AGENTS.md`: the CI section (SonarQube collects Rust coverage too, and how)
  and a local command for producing `lcov.info`.
- `README.md`: the testing paragraph.
- `PLAN.md`: tick the Phase 6 coverage item with the measured figure and the
  job time.

## Out of scope

- Importing Clippy output into SonarQube as external issues.
- Coverage for pull requests from forks (the scan already skips them, since
  they cannot read `SONAR_TOKEN`).
- Changing the quality gate or excluding Rust files from coverage.
