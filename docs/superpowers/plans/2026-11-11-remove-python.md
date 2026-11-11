# Remove Python (Python cutover, PR 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The repository builds, tests, measures and releases with Cargo alone: no Python file, extension, workflow step or documented command remains.

**Architecture:** First, replace the three things that still need Python with Rust or shell:

- fixture regeneration becomes a bless mode in `fixtures.rs`;
- the per-stage benchmark becomes an engine example;
- the coverage safeguards become `core/check_lcov.sh`.

Then delete the Python reference, the PyO3 adapter and the engine's `python` feature, simplify CI, and rewrite the docs for a Rust-only repository. The golden hashes from PR 1 (#51) pin the output throughout and must not change.

**Tech Stack:** Rust (Cargo workspace in `core/`, `serde_json`, `sha2`), POSIX sh and awk, cargo-llvm-cov 0.9.1, GitHub Actions, SonarQube Cloud.

**Spec:** `docs/superpowers/specs/2026-11-11-python-cutover-design.md`, section "PR 2: Remove Python". PR 1's plan is `docs/superpowers/plans/2026-11-11-rust-tests-take-over.md`.

## Global Constraints

- `core/cli/tests/golden/SHA256SUMS`, `PIXELS` and every `core/engine/tests/fixtures/*.json` stay byte-identical. No image changes in this PR.
- Committed reference data stays: `core/engine/tests/random_reference.json` and the fixture scenes. The files that describe them must say they were generated from CPython and the Python builders before the cutover.
- No Python remains: no `*.py` file, `core/py`, PyO3 or maturin; no `python` feature or `cfg_attr(feature = "python", …)`; no `setup-python` in any workflow; no Python settings in `sonar-project.properties`.
- Clippy with `-D warnings` for every CI combination: the workspace with and without `portable`, and the engine alone with `portable`, `json` and `json,portable`.
- Scripts: POSIX sh plus coreutils and awk.
- New crate versions must be at least two weeks old. None are expected.
- Commit messages: an imperative sentence, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Branch: `phase6/remove-python`, from `origin/main` at the PR #51 merge (`518d9ac`).
- Owner action after the merge (not a file): remove `python` from CodeQL's default setup in the repository settings. Say so in the PR description.

## Review Focus

1. **Bless run with nothing changed:** `bless_fixtures` must leave every fixture byte-identical, including the 47 Python-built scenes whose floats a Rust builder reproduces only to the last bit. Test: `blessing_leaves_matching_fixtures_untouched` (Task 1).
2. **Coverage collection that silently stops contributing:** an uninstrumented binary, or the engine tests skipped, must fail `check_lcov.sh`, not just lower the figure. Test: the hand-run negative cases recorded in the PR (Task 3), as the spec requires.
3. **A Python reference left behind:** a workflow, doc or command still naming a deleted file must fail review. Test: the `git grep` sweep in Task 5, with an empty result expected outside `docs/superpowers/` and the history sections.
4. **A Rust-written fixture that loads back differently:** a float written by the bless mode must parse back to the same bits. Test: `fixture_writer_round_trips_every_number` (Task 1).
5. **Benchmark on a style the scene builders don't list:** it must name the style and exit nonzero, not panic. Test: the `--styles nosuch` verification (Task 2).

---

## File Structure

- Modify `core/engine/tests/fixtures.rs`: bless mode, a fixture JSON writer, updated module docs and messages.
- Create `core/engine/examples/bench.rs`: scene and shading times per style.
- Create `core/check_lcov.sh`. Rewrite `core/coverage.sh`.
- Modify `.github/workflows/sonarqube.yml`, `sonar-project.properties`, `.github/workflows/tests.yml` and `.github/workflows/release-binaries.yml`. Delete `.github/workflows/slow-tests.yml`.
- Delete `simulacra.py`, `test_simulacra.py`, `bench_simulacra.py`, `.coveragerc`, `core/py/`, `core/cli/tests/check_release.py`, `core/cli/tests/check_coverage.py`, `core/engine/tests/make_fixtures.py` and `core/engine/tests/make_random_reference.py`.
- Modify `core/Cargo.toml` (members, `pyo3`), `core/engine/Cargo.toml` (`python` feature), `core/Cargo.lock`, the engine sources that carry the PyO3 derive (49 lines), `core/engine/src/lib.rs` (crate docs), `core/engine/tests/py_random.rs` (docs), and `core/cli/src/main.rs` (user-facing help and module docs). `.gitignore` keeps its Python rules: working copies still hold `.venv/`, caches and coverage files from before the cutover.
- Rewrite `README.md` and `AGENTS.md`. Update `PLAN.md`. Check `GLOSSARY.md`, which has no Python references today.

**Comment policy (decided here):** about 30 source comments name the Python function a piece of code was ported from (for example, "`_mosaic_scene` in Python"). They stay. One paragraph in `core/engine/src/lib.rs`'s crate docs says that such names refer to the Python reference retired in this PR, which can be read in git history before it. Text a user sees (`--help`, README) must not mention `simulacra.py` except in history.

---

### Task 1: Fixture blessing in Rust

**Files:**

- Modify: `core/engine/tests/fixtures.rs`

**Interfaces:**

- Produces:
  - Blessing is a dedicated ignored test, `bless_fixtures`, run on its own so that no other test reads the fixtures while it writes them: `cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable --test fixtures -- --ignored --exact bless_fixtures`, followed by the normal fixture tests. No environment variable is needed. It rewrites a fixture when any of these holds:
    - the fixture is missing;
    - its file does not parse, or its stored scene does not deserialize (`scene_from_json` fails, for example after a scene field is renamed) or does not pass the checked `render`;
    - its scene built by the Rust builder renders a hash other than the recorded one. The rewritten file gets the Rust-built scene and the new hash, at the existing settings: 48 × 36, seed 3, phase 0.37, and the palette at `sorted(PALETTES)[index % 24]`, where `index` is the style's position in sorted `STYLES`. All other fixtures are left untouched. It also deletes fixture files whose style no longer exists. Blessing on a non-`portable` build panics with `blessing needs --features portable`. Each file is written to a temporary file in the same directory and then renamed over the old one.
  - `fn bless_into(dir: &Path) -> Vec<String>` returns the styles it rewrote, sorted. `bless_fixtures` calls it on `fixture_dir()`, and the tests below call it on a copy in their own directory under `CARGO_TARGET_TMPDIR`, so they never touch the real fixtures.
  - `fn write_fixture_json(value: &serde_json::Value) -> String` writes one level per line with a one-space indent per level, a list of plain scalars on one line, and keys in the order given. That is the layout of the deleted `make_fixtures.py:_dump`, with `serde_json`'s number formatting, plus a trailing newline. Rewritten files may format numbers differently from the Python-written ones; that is accepted.
  - Read the scene through `simulacra_engine::build_scene_json` and parse it into a `serde_json::Value`. Without `serde_json`'s `preserve_order` feature (not enabled; no new dependency), a `Value` map sorts its keys, so a rewritten fixture lists its fields and scene keys alphabetically. That is accepted: key order carries no meaning, and only deliberately changed fixtures are rewritten. The fields are the same as now: `style, width, height, seed, core_seed, phase, palette, stops, sha256, scene`.
  - The module docs: the committed scenes came from the Python builders (the Rust builder for `attractor`) before the cutover. Blessing is how a deliberate change updates them. The failure messages point at the `bless_fixtures` command instead of `make_fixtures.py`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn fixture_writer_round_trips_every_number() {
    // For every committed fixture: parse(write_fixture_json(parse(text))) == parse(text),
    // comparing f64 values with to_bits().
}

#[test]
fn fixture_writer_keeps_the_layout() {
    // write_fixture_json(json!({"a": [1, 2.5], "b": {"c": [[1, 2], [3, 4]]}})) ==
    // "{\n \"a\": [1, 2.5],\n \"b\": {\n  \"c\": [\n   [1, 2],\n   [3, 4]\n  ]\n }\n}\n"
}

#[cfg(feature = "portable")]
#[test]
fn blessing_leaves_matching_fixtures_untouched() {
    // Copy fixtures/ to a fresh dir under CARGO_TARGET_TMPDIR; bless_into(copy) returns [];
    // every file is byte-identical to the original.
}

#[cfg(feature = "portable")]
#[test]
fn blessing_rewrites_a_changed_or_missing_fixture() {
    // In a copy: set waves.json's "sha256" to 64 zeros and delete blobs.json.
    // bless_into(copy) == ["blobs", "waves"]; both load, render their new recorded hash,
    // and that hash equals the original files' hash. The other 46 are byte-identical.
}

#[cfg(feature = "portable")]
#[test]
fn blessing_repairs_a_scene_that_no_longer_loads() {
    // In a copy: rename one key of waves.json's "scene" object (for example the first key,
    // with "_old" appended), keeping "sha256" as it was, so the image hash alone would not
    // trigger a rewrite. bless_into(copy) == ["waves"]; the rewritten scene loads through
    // scene_from_json and renders the recorded hash. The other 47 are byte-identical.
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable --test fixtures`
Expected: a compile error, because `write_fixture_json` and `bless_into` don't exist.

- [ ] **Step 3: Implement `write_fixture_json`, `bless_into` and the ignored `bless_fixtures` test**

- [ ] **Step 4: Run the tests, then try the bless command on the real directory**

Run: `cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable --test fixtures`
Expected: all pass.

Then the bless command, three ways, with `B="cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable --test fixtures"`:

1. **Nothing changed:** `$B -- --ignored --exact bless_fixtures && $B && git status --short core/engine/tests/fixtures`. Expected: passes, and `git status` prints nothing.
2. **A missing and a changed fixture:** delete `blobs.json` and set `waves.json`'s `sha256` to 64 zeros, then run `$B -- --ignored --exact bless_fixtures && $B`. Expected: both files are rewritten and the normal tests pass (both now render their recorded hashes, equal to the hashes in `git show HEAD:core/engine/tests/fixtures/<style>.json`).
3. Then restore both with `git checkout -- core/engine/tests/fixtures`, because a rewritten file has sorted keys and is not byte-identical to the committed one.

Also confirm that the default backend still passes: `cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json --test fixtures`, and Clippy with `--features json,portable`.

- [ ] **Step 5: Commit**

```bash
git add core/engine/tests/fixtures.rs
git commit -m "Regenerate the portable fixtures from Rust with an ignored bless test"
```

---

### Task 2: Benchmark example

**Files:**

- Create: `core/engine/examples/bench.rs`

**Interfaces:**

- Produces: `cargo run --release --manifest-path core/Cargo.toml -p simulacra_engine [--features portable] --example bench -- [--width N] [--height N] [--phase X] [--seed N] [--repeat N] [--palette NAME] [--styles a,b,...]`. The defaults are 640, 480, 0.37, 1, 3, `deep` and every style in `SCENE_BUILDERS`. The output starts with a header naming the size, phase, repeats and backend (`portable` or `system`, from `simulacra_engine::PORTABLE`). Then it prints a tab-separated table, `style  scene_ms  shade_ms`, with the minimum of the repeats in milliseconds to 3 decimals, and a final `total` row. Shading is timed through the checked `render`. An unknown style or palette prints `bench: unknown style 'x'` (or `palette`) and exits with status 2. Arguments are parsed by hand (no new dependency).
- Encoding and whole runs are timed by running the binary. Task 5 documents a `hyperfine` command and a plain `time` loop.

- [ ] **Step 1: Write the example**

The `main` function prints the header, then the table. Put argument parsing in `fn parse(args: impl Iterator<Item = String>) -> Result<Options, String>`.

- [ ] **Step 2: Run it**

Run: `cargo run -q --release --manifest-path core/Cargo.toml -p simulacra_engine --example bench -- --width 64 --height 48 --repeat 1 | tail -n +2 | wc -l` and then `cargo run -q --release --manifest-path core/Cargo.toml -p simulacra_engine --example bench -- --styles nosuch; echo "exit $?"`
Expected: 50 lines (the column header, 48 styles and `total`), then `bench: unknown style 'nosuch'` and `exit 2`.

- [ ] **Step 3: Lint and commit**

Run: `cargo clippy --manifest-path core/Cargo.toml -p simulacra_engine --all-targets --features portable -- -D warnings`
Expected: no warnings.

```bash
git add core/engine/examples/bench.rs
git commit -m "Add a Rust benchmark of scene building and shading per style"
```

---

### Task 3: Coverage without Python

**Files:**

- Create: `core/check_lcov.sh` (executable)
- Modify: `core/coverage.sh`, `.github/workflows/sonarqube.yml`, `sonar-project.properties`

**Interfaces:**

- `core/check_lcov.sh LCOV_FILE` exits 0 if all three checks below pass. Otherwise it prints each failure and exits 1.
  - Every `DA:` line matches `^DA:[0-9]+,[0-9]+(,.*)?$`; otherwise it prints `malformed DA line <n>: <text>`.
  - The line of `core/cli/src/main.rs` that holds `let args = Args::parse();` (its number found with `grep -n` on that text; fail if the text is missing) has a hit in the record whose `SF:` path ends in `core/cli/src/main.rs`. Otherwise: `the binary's main() has no hits: its instrumentation was lost`.
  - Some record whose `SF:` path contains `core/engine/src/styles/` has a `DA` line with a nonzero hit count. Otherwise: `no hits in core/engine/src/styles`.
- `core/coverage.sh` (bash, `set -euo pipefail`) sources `cargo llvm-cov show-env`, runs `clean --workspace`, `cargo test --locked` (the workspace; `cli.rs` runs the instrumented binary) and `cargo test --locked -p simulacra_engine --features json,portable`, then writes `lcov.info` with `report --lcov`, prints `--summary-only` and runs `sh core/check_lcov.sh lcov.info`. No `PYTHON` and no maturin.
- `sonarqube.yml` drops `setup-python` and the Python test step, and keeps the toolchain, cargo-llvm-cov 0.9.1, `core/coverage.sh` and the scan.
- `sonar-project.properties` sets `sonar.sources=core/engine/src,core/cli/src` and `sonar.tests=core/engine/tests,core/cli/tests`. It keeps `sonar.exclusions`, `sonar.cpd.exclusions` (with its comment now saying that a unit test keeps the table sorted and valid) and `sonar.rust.lcov.reportPaths`. It drops `sonar.python.*` and `sonar.coverage.exclusions`, and updates the comments.

- [ ] **Step 1: Write `check_lcov.sh` and record its negative cases by hand**

Run each of these and keep the output for the PR description:

```bash
printf 'SF:%s\nDA:1,1\nend_of_record\n' "$PWD/core/engine/src/lib.rs" > /tmp/claude-1000/a.info   # neither hit
sh core/check_lcov.sh /tmp/claude-1000/a.info; echo "exit $?"      # expect both messages, exit 1
printf 'SF:x\nDA:oops\nend_of_record\n' > /tmp/claude-1000/b.info
sh core/check_lcov.sh /tmp/claude-1000/b.info; echo "exit $?"      # expect "malformed DA line", exit 1
```

Write the files in the session scratchpad rather than `/tmp/claude-1000` if one is listed.

- [ ] **Step 2: Rewrite `coverage.sh`, run it, and check that the real report passes**

Run: `core/coverage.sh 2>&1 | tail -5`
Expected: the summary prints, and `check_lcov.sh` exits 0 on `lcov.info`. Note the total line coverage for the PR.

Also run the negative case with real data: build without instrumentation (`cargo build -p simulacra_cli`, outside the `show-env` shell), point a test run at it, regenerate a report, and expect the `main()` message. If wiring that up takes more than a few commands, record that the synthetic case above covers it.

- [ ] **Step 3: Update the SonarQube workflow and properties, lint, and commit**

Run: `actionlint .github/workflows/sonarqube.yml`
Expected: no errors.

```bash
git add core/check_lcov.sh core/coverage.sh .github/workflows/sonarqube.yml sonar-project.properties
git commit -m "Collect Rust coverage without Python"
```

---

### Task 4: Delete Python and the PyO3 adapter

**Files:**

- Delete: `simulacra.py`, `test_simulacra.py`, `bench_simulacra.py`, `.coveragerc`, `core/py/` (all of it), `core/cli/tests/check_release.py`, `core/cli/tests/check_coverage.py`, `core/engine/tests/make_fixtures.py`, `core/engine/tests/make_random_reference.py`, `.github/workflows/slow-tests.yml`
- Modify:
  - `core/Cargo.toml`: members become `["cli", "engine"]`; drop the `pyo3` workspace dependency; update the header comment.
  - `core/engine/Cargo.toml`: drop `pyo3` and the `python` feature.
  - `core/Cargo.lock`: `cargo update --workspace` only removes entries; check that no version changes.
  - Every `#[cfg_attr(feature = "python", …)]` line in `core/engine/src` (49 of them).
  - `core/engine/src/lib.rs`: crate docs, with the comment-policy paragraph.
  - `core/engine/tests/py_random.rs` and `fixtures.rs`: docs say the reference data came from CPython and the Python builders before the cutover, and that their generators were removed.
  - `core/cli/src/main.rs`: the `--seed` help becomes `Seed; the same seed gives the same image`, and the module docs describe the options without comparing them with `simulacra.py`.
  - `.github/workflows/tests.yml`: remove the `python` job; in `rust`, remove `setup-python` and the PyO3 `cargo tree` check (keep the engine-alone test and Clippy steps); update the comments.
  - `.github/workflows/release-binaries.yml`: remove the step "The binary does not depend on PyO3".

- [ ] **Step 1: Delete the files and edit the manifests and derives**

Run: `grep -rln 'feature = "python"' core/engine/src` and remove each line it finds.

- [ ] **Step 2: Check that no Python is left in the build**

Run: `git ls-files '*.py' core/py; grep -rn 'pyo3\|PyO3\|maturin\|feature = "python"' core .github --include=*.rs --include=*.toml --include=*.yml --include=*.lock`
Expected: no output.

- [ ] **Step 3: Run every CI test command and check that the golden and fixture data are unchanged**

```bash
M="--manifest-path core/Cargo.toml"
cargo test --locked $M && cargo clippy --locked $M --all-targets -- -D warnings
cargo test --locked $M --features portable && cargo clippy --locked $M --all-targets --features portable -- -D warnings
cargo test --locked $M -p simulacra_engine && cargo clippy --locked $M -p simulacra_engine --all-targets --features portable -- -D warnings
cargo test --locked $M -p simulacra_engine --features json,portable && cargo test --locked $M -p simulacra_engine --features json
cargo clippy --locked $M -p simulacra_engine --all-targets --features json,portable -- -D warnings
sh core/cli/tests/golden_selftest.sh
cargo build --locked --release $M -p simulacra_cli --features portable
sh core/cli/tests/golden.sh check core/target/release/simulacra --keep "$KEEP"   # a fresh scratchpad dir
SIMULACRA_GOLDEN_OUT="$KEEP" cargo test --locked --release $M -p simulacra_cli --features portable --test golden_pixels -- --ignored
actionlint .github/workflows/*.yml
git status --short core/cli/tests/golden core/engine/tests/fixtures
```

Expected: everything passes, `343 workloads match`, and the last command prints nothing.

- [ ] **Step 4: Commit**

Stage explicitly, never with `git add -A` or `git add .`: working copies still hold `.venv/`, `__pycache__/` and coverage output. Delete with `git rm` (the listed files and `git rm -r core/py`), stage the edited files by path, then check `git status --short` for nothing unintended before committing.

```bash
git rm simulacra.py test_simulacra.py bench_simulacra.py .coveragerc core/cli/tests/check_release.py core/cli/tests/check_coverage.py core/engine/tests/make_fixtures.py core/engine/tests/make_random_reference.py .github/workflows/slow-tests.yml
git rm -r core/py
git add core/Cargo.toml core/Cargo.lock core/engine/Cargo.toml core/engine/src core/engine/tests/py_random.rs core/engine/tests/fixtures.rs core/cli/src/main.rs .github/workflows/tests.yml .github/workflows/release-binaries.yml
git status --short   # only the intended deletions and edits
git commit -m "Remove the Python reference, its tests and the PyO3 extension"
```

---

### Task 5: Documentation for a Rust-only repository

**Files:**

- Modify: `README.md`, `AGENTS.md`, `PLAN.md`

- [ ] **Step 1: Rewrite the README**

Lead with the binary. The sections are:

- install (the current "Installing the binary", promoted);
- quick start with `simulacra`;
- command-line options;
- examples (the current example commands rewritten for the binary, same arguments);
- styles and palettes (tables unchanged);
- animation;
- how rendering works (engine terms);
- performance and limits (the binary's limits, plus the benchmark example and `hyperfine --warmup 1 'simulacra --style ink --width 1920 --height 1080 --out /tmp/x.png'` or a `time` loop for whole runs);
- testing (`cargo test`, Clippy, the portable fixtures and bless, the golden check, the pixel test and the bless rules, `core/coverage.sh`);
- project files;
- "Adding a style" in Rust, which says that styles are not being added for now and lists the steps: scene struct and shader, `BuildScene`, `for_each_style!`, `scene_builders!`, `validate`, a fixture through `bless_fixtures`, the golden bless and the style table.

Remove "Python API", the Python quick start, "Optional Rust core", "Extending the script" and the Python suite's test list. Keep "Differences from simulacra.py" only as a short history note, or drop it.

- [ ] **Step 2: Rewrite AGENTS.md**

- **Overview:** the binary and the engine. `simulacra.py` was retired in this PR; its history is in git.
- **Progress table:** keep it, and add a row for this PR.
- **Commands:** Cargo, the scripts and the benchmark only.
- **CI:** Tests (Rust and golden jobs), SonarQube (Rust coverage, `check_lcov.sh`), Release binaries (no PyO3 step) and OSV-Scanner; slow tests are gone.
- **Architecture:** drop the Python registry, renderer contract and adapter items. Restate randomness as Rust scene builders on `PyRandom`, noise, colors, writers (`output.rs`), scene validation, fixtures with bless, golden hashes, animation and the style notes, in Rust terms.
- **Conventions:** end the freeze policy. Styles live in Rust only, and a deliberate change re-blesses the fixtures and both golden manifests in the same PR. Drop the Python-specific rules (complexity limits for Python, `snake_case`) and keep the size-independence and brightness rules.
- **SonarQube and CodeQL notes:** keep the Rust and `output.rs` items, and drop the Python-only ones.
- **Known issues:** drop the Python items. Next: PR 3 (math backends).

- [ ] **Step 3: Update PLAN.md**

Record the cutover under Phase 6. Tick "Remove Python as an application runtime dependency" and resolve "Retain the Python reference temporarily" (retired). Answer the authoring question: styles are written directly in Rust. Mark the freeze policy section as ended. Keep the Python-era baselines as history.

- [ ] **Step 4: Sweep for leftover references**

Run: `git grep -nE 'simulacra\.py|test_simulacra|bench_simulacra|make_fixtures|check_release|check_coverage|make_random_reference|maturin|unittest|\.venv|SIMULACRA_CLI|SIMULACRA_SLOW' -- ':!docs/superpowers' ':!PLAN.md'`
Expected: only lines that are explicitly history ("retired", "before the cutover") or the comment-policy porting notes in `core/engine/src`. List any other hit and fix it. In `PLAN.md`, check by reading that every such mention is past tense.

- [ ] **Step 5: Commit**

```bash
git add README.md AGENTS.md PLAN.md
git commit -m "Document Simulacra as a Rust-only project"
```

---

### Task 6: Final verification and pull request

- [ ] **Step 1: Rerun every Task 4 Step 3 command, plus `core/coverage.sh`**

Expected: everything passes, the golden and fixture data are unchanged, and `check_lcov.sh` passes.

- [ ] **Step 2: Rebase, push and open the PR**

```bash
git fetch && git rebase origin/main
git push -u origin phase6/remove-python
gh pr create --title "Remove Python: Simulacra is a Rust-only project" --body "<what was deleted and what replaced it (fixture bless, benchmark example, check_lcov.sh); golden and fixture data unchanged; coverage figure; the check_lcov.sh negative cases with output; owner action: remove python from CodeQL default setup; 🤖 Generated with [Claude Code](https://claude.com/claude-code)>"
```
