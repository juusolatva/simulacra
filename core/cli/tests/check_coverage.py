"""Checks that Rust coverage collection saw every test set that exercises Rust.

The SonarQube job (``.github/workflows/sonarqube.yml``) runs ``core/coverage.sh``,
which calls this script twice:

``run-cli-tests``
    Runs ``StandaloneCliTests`` from ``test_simulacra.py`` against the binary
    named by ``SIMULACRA_CLI`` and fails if a test fails, if none ran or if any
    was skipped (the whole class skips when ``SIMULACRA_CLI`` is unset).

``python-lcov FILE``
    Fails unless the report, written right after the Python suite and before
    the binary's tests (which render through the extension as well), shows
    hits in the PyO3 adapter, ``core/py/src``: cargo test never loads it, so
    the hits come from the Python suite.

``lcov FILE``
    Fails unless the final report shows hits on the first statement of
    ``fn main()`` in ``core/cli/src/main.rs`` (only the binary runs it; the
    CLI's unit tests call helpers) and in the shaders in
    ``core/engine/src/styles``.

Without these checks, a collection step that silently stopped contributing
would only lower the coverage figure.

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
    for line in Path(path).read_text(encoding='utf-8').splitlines():
        if line.startswith('SF:'):
            current = files.setdefault(Path(line[3:]).resolve(), {})
        elif line.startswith('DA:') and current is not None:
            try:
                number, hits = (int(field) for field in line[3:].split(',')[:2])
            except ValueError:
                raise ValueError(f'{path}: malformed LCOV line {line!r}') from None
            current[number] = current.get(number, 0) + hits
        elif line == 'end_of_record':
            current = None
    return files


def main_first_statement_line(source):
    """The 1-based line of the first statement in ``fn main()``."""
    lines = source.splitlines()
    start = next((i for i, text in enumerate(lines) if text.startswith('fn main()')), None)
    if start is None:
        raise ValueError(f'{MAIN_RS}: fn main() not found')
    for i in range(start + 1, len(lines)):
        if lines[i].strip().startswith(MAIN_FIRST_STATEMENT):
            return i + 1
    raise ValueError(f'{MAIN_RS}: {MAIN_FIRST_STATEMENT!r} not found in fn main()')


def hits_under(files, directory):
    """Total hits over the source files under ``directory``."""
    base = (ROOT / directory).resolve()
    return sum(sum(lines.values()) for path, lines in files.items() if path.is_relative_to(base))


def check_python_lcov(path):
    """Return the problems with the report written right after the Python suite."""
    if hits_under(read_lcov(path), 'core/py/src') == 0:
        return ['core/py/src has no hits: the Python suite did not run an instrumented core']
    return []


def check_lcov(path):
    """Return the problems with the final LCOV report at ``path``."""
    files = read_lcov(path)
    problems = []
    main_rs = (ROOT / MAIN_RS).resolve()
    line = main_first_statement_line(main_rs.read_text(encoding='utf-8'))
    if files.get(main_rs, {}).get(line, 0) == 0:
        problems.append(f'{MAIN_RS}:{line} ({MAIN_FIRST_STATEMENT}) has no hits: the binary did not run instrumented')
    if hits_under(files, 'core/engine/src/styles') == 0:
        problems.append('core/engine/src/styles has no hits: no shader ran instrumented')
    return problems


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('run-cli-tests', help='run StandaloneCliTests; fail on any failure or skip')
    checks = {
        'python-lcov': (check_python_lcov, 'check the report written after the Python suite', 'the Python extension'),
        'lcov': (check_lcov, 'check the final report', 'the binary and the shaders'),
    }
    for name, (_, description, _) in checks.items():
        commands.add_parser(name, help=description).add_argument('file')
    args = parser.parse_args(argv)
    if args.command == 'run-cli-tests':
        problems = [problem] if (problem := run_cli_tests()) else []
    else:
        check, _, covered = checks[args.command]
        problems = check(args.file)
        if not problems:
            print(f'check_coverage: {args.file} covers {covered}')
    for problem in problems:
        print(f'check_coverage: {problem}', file=sys.stderr)
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
