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

The job runs from the repository root; the Cargo workspace is `core/`. Every
Cargo and `cargo llvm-cov` command passes `--manifest-path core/Cargo.toml`
(maturin takes `-m core/py/Cargo.toml`, in the same workspace), so all builds
share `core/target`, while reports (`coverage.xml`, `lcov.info`) are written
relative to the repository root.

1. Install the Rust component and `cargo-llvm-cov` (its own step).
2. Run the Python suite without the core under `coverage` (unchanged, its own
   step).
3. **Collect Rust coverage in one step**, a single bash script that first
   runs `source <(cargo llvm-cov show-env --sh --manifest-path
   core/Cargo.toml)`, so every later command in the script inherits the
   instrumentation variables as the shell decoded them. Nothing is written
   to `$GITHUB_ENV`: `show-env` prints shell-quoted values (redirecting them
   into `$GITHUB_ENV` would keep literal quotes in `RUSTFLAGS`), and
   `$GITHUB_ENV` only reaches later steps anyway. In order:
   1. `cargo llvm-cov clean --workspace --manifest-path core/Cargo.toml`;
   2. `cargo test --manifest-path core/Cargo.toml` and
      `cargo test --manifest-path core/Cargo.toml -p simulacra_engine
      --features json,portable`;
   3. `maturin develop -m core/py/Cargo.toml` (an instrumented core), then
      the Python suite again with `coverage run --append`, then `coverage
      xml`, then an intermediate report (`core/target/lcov-python.info`)
      and the `python-lcov` check (below);
   4. `cargo build --manifest-path core/Cargo.toml -p simulacra_cli` (an
      instrumented binary), then the standalone CLI tests with
      `SIMULACRA_CLI=core/target/debug/simulacra`, run through the check
      script (below) so that zero tests run or any skip fails the step;
   5. `cargo llvm-cov report --manifest-path core/Cargo.toml --lcov
      --output-path lcov.info`, and a per-file summary to the job log;
   6. the coverage check (below) on `lcov.info`.
4. The scanner reads `coverage.xml` and `lcov.info`.

**One profile.** `cargo llvm-cov report` reads the objects of one profile
directory, so every instrumented build and test above uses the same profile:
the dev profile (`cargo test`, `cargo build` and `maturin develop` without
`--release`, and `report` without `--release`). If the CLI tests are too slow
on a debug binary, the whole collection moves to the release profile
together (`cargo test --release`, `maturin develop --release`, `cargo build
--release`, `SIMULACRA_CLI=core/target/release/simulacra`, `report
--release`), never one build alone.

`sonar-project.properties` gains `sonar.rust.lcov.reportPaths=lcov.info`, and
its comment saying Rust coverage is not collected is replaced.

## Coverage check (`core/cli/tests/check_coverage.py`)

A standard-library script, run in CI, so that a collection failure cannot
silently lower coverage later. It has two commands:

- `run-cli-tests`: loads `StandaloneCliTests` from `test_simulacra.py` through
  the `unittest` API, runs them, and fails if any test fails, if none ran or
  if any was skipped (for example because `SIMULACRA_CLI` was not set).
- `python-lcov FILE`: run on a report written right after the Python suite
  and before the binary's tests (which also render through the extension, so
  the final report cannot show whether the suite reached it); fails unless
  it shows hits in `core/py/src`, which `cargo test` never loads.
- `lcov FILE`: fails unless the final report shows:
  - a nonzero hit count on the first statement of `fn main()` in
    `core/cli/src/main.rs` (`let args = Args::parse();`), which only the
    binary executes (the CLI's unit tests call helpers, never `main`); the
    script finds the line by its text, not a fixed number;
  - nonzero hits in `core/engine/src/styles` (the shaders).

## Points to verify during implementation

Each is checked locally before CI, by running the collection script end to
end:

- `show-env`, `clean` and `report` accept `--manifest-path core/Cargo.toml`
  and agree on `core/target`.
- maturin builds the extension with the instrumentation `RUSTFLAGS` inherited
  from the sourced environment (otherwise `core/py/src` has no hits and the
  check fails).
- `cargo llvm-cov report` merges the default and `portable` builds of the
  engine into one figure per source file.
- `StandaloneCliTests` with a debug binary finishes in reasonable time (a few
  minutes); otherwise switch the whole collection to release as above.
- The coverage check passes, and fails when the CLI tests are skipped or the
  extension is built without instrumentation (tried once by hand).
- The pull request's SonarQube analysis shows Rust coverage.

## Consequences

- The overall coverage figure drops from Python's 99.1% to the combined
  figure. Record the measured Rust figure in PLAN.md.
- New Rust code on pull requests becomes subject to the quality gate's
  new-code coverage condition (80%). The gate itself is not changed.
- The SonarQube job takes longer (instrumented builds and the extra `cargo
  test` and CLI runs); record the time.

## Documentation

- `AGENTS.md`: the CI section (SonarQube collects Rust coverage too, how, and
  the coverage check) and a local command for producing `lcov.info`.
- `README.md`: the testing paragraph.
- `PLAN.md`: tick the Phase 6 coverage item with the measured figure and the
  job time.

## Out of scope

- Importing Clippy output into SonarQube as external issues.
- Coverage for pull requests from forks (the scan already skips them, since
  they cannot read `SONAR_TOKEN`).
- Changing the quality gate or excluding Rust files from coverage.
