"""Regenerate the portable fixtures in fixtures/: one JSON file per native style.

Each file holds a scene built by the Python scene builder (the Rust one for
the styles in RUST_BUILT, see below), the render parameters, and the SHA-256 of the packed RGB that a core built with the
`portable` feature renders from it. `cargo test -p simulacra_engine --features
json,portable` (tests/fixtures.rs) loads the scene through serde, renders it
and compares the hash, with no Python involved. Because the hash comes from the
Python adapter (PyO3) and the test renders through serde, a match also shows
that both ways of loading a scene agree.

Scenes depend on the platform libm through the Python builders, so they are
generated once and committed, not rebuilt in CI. Rerun this deliberately when a
shader or scene changes on purpose, and review the diff:

    python -m venv /tmp/portable && /tmp/portable/bin/pip install maturin
    VIRTUAL_ENV=/tmp/portable PATH=/tmp/portable/bin:$PATH \\
        maturin develop --release -m core/py/Cargo.toml --features portable
    /tmp/portable/bin/python core/engine/tests/make_fixtures.py
"""

import hashlib
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
import simulacra as art  # noqa: E402  (needs the repository root on the path)

FIXTURES = pathlib.Path(__file__).resolve().parent / 'fixtures'
# A small canvas keeps the files reviewable (scenes such as woodblock's
# surfaces and the sampled torus curve grow with the canvas); the phase is
# nonzero so the animated scene paths are covered, and the palette cycles
# through every built-in palette across the styles.
WIDTH, HEIGHT, SEED, PHASE = 48, 36, 3, 0.37
# Styles whose fixture scene comes from the Rust builder (on this portable
# core) instead of the Python one. The attractor frames its figure by the
# bounds of a short chaotic orbit, so a scene built with glibc's sin and cos
# and one built with the libm crate's differ in `mid` and `scale` by up to a
# few percent. Release binaries build their scenes in Rust with the libm
# crate, the same on every target, so their scene is the one to record.
RUST_BUILT = frozenset({'attractor'})


def _tuples(value):
    """JSON lists as tuples, which the adapter reads into scene tuple fields."""
    if isinstance(value, list):
        return tuple(_tuples(item) for item in value)
    if isinstance(value, dict):
        return {key: _tuples(item) for key, item in value.items()}
    return value


def _scalar(value):
    return not isinstance(value, (dict, list, tuple))


def _dump(value, indent=0):
    """JSON with one level per line, but a list of plain numbers on one line."""
    pad = ' ' * indent
    if isinstance(value, dict):
        items = [f'{pad} {json.dumps(k)}: {_dump(v, indent + 1)}' for k, v in value.items()]
        return '{\n' + ',\n'.join(items) + f'\n{pad}}}'
    if isinstance(value, (list, tuple)):
        if all(map(_scalar, value)):
            return json.dumps(list(value))
        items = [f'{pad} {_dump(v, indent + 1)}' for v in value]
        return '[\n' + ',\n'.join(items) + f'\n{pad}]'
    return json.dumps(value)


def _fixture(index, style):
    palettes = sorted(art.PALETTES)
    palette = palettes[index % len(palettes)]
    stops = art.PALETTES[palette]
    if style in RUST_BUILT:
        scene = _tuples(json.loads(art._core.build_scene(style, WIDTH, HEIGHT, SEED, stops, PHASE)))
    else:
        scene = art.NATIVE_STYLES[style](WIDTH, HEIGHT, SEED, stops, PHASE)
    core_seed = art._seed32(SEED)
    rgb = art._core.render(style, WIDTH, HEIGHT, core_seed, stops, scene)
    return {
        'style': style,
        'width': WIDTH,
        'height': HEIGHT,
        'seed': SEED,
        'core_seed': core_seed,
        'phase': PHASE,
        'palette': palette,
        'stops': [list(stop) for stop in stops],
        'sha256': hashlib.sha256(rgb).hexdigest(),
        'scene': scene,
    }


def main():
    if art._core is None or not art._core.PORTABLE:
        sys.exit('make_fixtures.py needs a Rust core built with --features portable')
    FIXTURES.mkdir(exist_ok=True)
    styles = sorted(art.NATIVE_STYLES)
    for stale in set(FIXTURES.glob('*.json')) - {FIXTURES / f'{s}.json' for s in styles}:
        stale.unlink()
    for index, style in enumerate(styles):
        (FIXTURES / f'{style}.json').write_text(_dump(_fixture(index, style)) + '\n')
    print(f'wrote {len(styles)} fixtures to {FIXTURES.relative_to(ROOT)}')


if __name__ == '__main__':
    main()
