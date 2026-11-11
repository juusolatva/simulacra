"""Checks that Rust coverage collection saw every test set that exercises Rust.

The SonarQube job (``.github/workflows/sonarqube.yml``) runs ``core/coverage.sh``,
which calls this script twice:

``run-cli-tests``
    Runs ``StandaloneCliTests`` from ``test_simulacra.py`` against the binary
    named by ``SIMULACRA_CLI`` and fails if a test fails, if none ran or if any
    was skipped (the whole class skips when ``SIMULACRA_CLI`` is unset).

``lcov FILE``
    Fails unless the LCOV report shows hits where only one test set reaches:
    the first statement of ``fn main()`` in ``core/cli/src/main.rs`` (only the
    binary runs it; the CLI's unit tests call helpers), the PyO3 adapter in
    ``core/py/src`` (only the Python suite runs it) and the shaders in
    ``core/engine/src/styles``. Without this, a collection step that silently
    stopped contributing would only lower the coverage figure.

Standard library only.
"""

import argparse
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MAIN_RS = 'core/cli/src/main.rs'
MAIN_FIRST_STATEMENT = 'let args = Args::parse();'


def run_cli_tests():
    """Run StandaloneCliTests; return an error message, or None if all ran and passed."""
    sys.path.insert(0, str(ROOT))
    import test_simulacra  # noqa: E402 (needs the repository root on the path)

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(test_simulacra.StandaloneCliTests)
    result = unittest.TextTestRunner(verbosity=1).run(suite)
    if not result.wasSuccessful():
        return 'StandaloneCliTests failed'
    if result.testsRun == 0:
        return 'no StandaloneCliTests ran'
    if result.skipped:
        return f'{len(result.skipped)} StandaloneCliTests were skipped (is SIMULACRA_CLI set?)'
    return None


def read_lcov(path):
    """Map each source file in an LCOV report to {line: hits}."""
    files, current = {}, None
    for line in Path(path).read_text().splitlines():
        if line.startswith('SF:'):
            current = files.setdefault(Path(line[3:]).resolve(), {})
        elif line.startswith('DA:') and current is not None:
            number, hits = line[3:].split(',')[:2]
            current[int(number)] = current.get(int(number), 0) + int(hits)
        elif line == 'end_of_record':
            current = None
    return files


def main_first_statement_line(source):
    """The 1-based line of the first statement in ``fn main()``."""
    lines = source.splitlines()
    start = next(i for i, text in enumerate(lines) if text.startswith('fn main()'))
    for i in range(start + 1, len(lines)):
        if lines[i].strip() == MAIN_FIRST_STATEMENT:
            return i + 1
    raise ValueError(f'{MAIN_RS}: {MAIN_FIRST_STATEMENT!r} not found in fn main()')


def hits_under(files, directory):
    """Total hits over the source files under ``directory``."""
    base = (ROOT / directory).resolve()
    return sum(sum(lines.values()) for path, lines in files.items() if path.is_relative_to(base))


def check_lcov(path):
    """Return the list of problems with the LCOV report at ``path``."""
    files = read_lcov(path)
    problems = []
    main_rs = (ROOT / MAIN_RS).resolve()
    line = main_first_statement_line(main_rs.read_text())
    if files.get(main_rs, {}).get(line, 0) == 0:
        problems.append(f'{MAIN_RS}:{line} ({MAIN_FIRST_STATEMENT}) has no hits: the binary did not run instrumented')
    for directory, reached_by in (
        ('core/py/src', 'the Python suite did not run an instrumented core'),
        ('core/engine/src/styles', 'no shader ran instrumented'),
    ):
        if hits_under(files, directory) == 0:
            problems.append(f'{directory} has no hits: {reached_by}')
    return problems


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('run-cli-tests', help='run StandaloneCliTests; fail on any failure or skip')
    lcov = commands.add_parser('lcov', help='check that an LCOV report covers every test set')
    lcov.add_argument('file')
    args = parser.parse_args(argv)
    if args.command == 'run-cli-tests':
        problems = [problem] if (problem := run_cli_tests()) else []
    else:
        problems = check_lcov(args.file)
    for problem in problems:
        print(f'check_coverage: {problem}', file=sys.stderr)
    if not problems and args.command == 'lcov':
        print(f'check_coverage: {args.file} covers the binary, the Python extension and the shaders')
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
