#!/usr/bin/env python3

import inspect
import io
import math
import os
import pathlib
import re
import stat
import struct
import tempfile
import unittest
import zlib
from unittest import mock

import abstract_art as art

# Styles whose native output may differ from Python when the core is built with
# the portable math feature (see "Measured portable drift" in PLAN.md).
PORTABLE_DRIFT_STYLES = frozenset({'attractor', 'cymatics'})


def _may_drift(style):
    """True if the native core uses portable math and ``style`` can differ from Python."""
    return getattr(art._core, 'PORTABLE', False) and style in PORTABLE_DRIFT_STYLES


def chunks(path):
    data = pathlib.Path(path).read_bytes()
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    offset = 8
    result = []
    while offset < len(data):
        size = struct.unpack('>I', data[offset:offset + 4])[0]
        tag = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + size]
        crc = struct.unpack('>I', data[offset + 8 + size:offset + 12 + size])[0]
        assert crc == zlib.crc32(tag + payload) & 0xFFFFFFFF
        result.append((tag, payload))
        offset += size + 12
    return result


def lzw_decode(data):
    """Reference GIF LZW decoder (minimum code size 8) for checking the writer."""
    clear, end = 256, 257
    table = {i: bytes([i]) for i in range(clear)}
    out = bytearray()
    width, bits, buffer, position, previous, next_code = 9, 0, 0, 0, None, 258
    while True:
        while bits < width:
            buffer |= data[position] << bits
            position += 1
            bits += 8
        code = buffer & ((1 << width) - 1)
        buffer >>= width
        bits -= width
        if code == clear:
            table = {i: bytes([i]) for i in range(clear)}
            width, previous, next_code = 9, None, 258
            continue
        if code == end:
            return bytes(out)
        entry = table[code] if code in table else previous + previous[:1]
        if previous is not None:
            table[next_code] = previous + entry[:1]
            next_code += 1
            if next_code == 1 << width and width < 12:
                width += 1
        out += entry
        previous = entry


def read_gif(path):
    """Parse a GIF written by write_gif: (width, height, table, frames, delays, loops)."""
    data = pathlib.Path(path).read_bytes()
    assert data[:6] == b'GIF89a'
    assert data[-1:] == b'\x3b'
    width, height = struct.unpack('<HH', data[6:10])
    table = [tuple(data[13 + 3 * i:16 + 3 * i]) for i in range(256)]
    position, frames, delays, loops = 13 + 768, [], [], False
    while data[position] != 0x3B:
        introducer = data[position]
        if introducer == 0x21:
            label = data[position + 1]
            if label == 0xF9:
                delays.append(struct.unpack('<H', data[position + 4:position + 6])[0])
            loops = loops or label == 0xFF
            position += 2
            while data[position]:
                position += 1 + data[position]
            position += 1
            continue
        assert introducer == 0x2C
        assert data[position + 10] == 8
        position += 11
        packed = b''
        while data[position]:
            packed += data[position + 1:position + 1 + data[position]]
            position += 1 + data[position]
        position += 1
        frames.append(lzw_decode(packed))
    return width, height, table, frames, delays, loops


class RenderingTests(unittest.TestCase):
    def test_failed_animation_preserves_existing_output(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'existing.png'
            for frames in ([[]], [[[(0, 0, 0)]], []]):
                path.write_bytes(b'previous output')
                with self.assertRaises(ValueError):
                    art.write_animated_png(path, frames, 1, 1)
                self.assertEqual(path.read_bytes(), b'previous output')
                self.assertEqual(list(path.parent.iterdir()), [path])

    def test_animation_replace_failure_cleans_up_temporary_file(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'existing.png'
            path.write_bytes(b'previous output')
            with mock.patch.object(art.os, 'replace', side_effect=OSError('disk error')):
                with self.assertRaises(OSError):
                    art.write_animated_png(path, [[[(0, 0, 0)]]], 1, 1)
            self.assertEqual(path.read_bytes(), b'previous output')
            self.assertEqual(list(path.parent.iterdir()), [path])
            art.write_animated_png(path, [[[(0, 0, 0)]]], 1, 1)
            self.assertEqual(chunks(path)[-1], (b'IEND', b''))
            self.assertEqual(list(path.parent.iterdir()), [path])

    def test_large_flowfield_trails_are_connected(self):
        pixels = art.render(512, 512, 1, 'flowfield', 'deep')
        occupied = {(x, y) for y, row in enumerate(pixels)
                    for x, c in enumerate(row) if c != art.PALETTES['deep'][0]}
        isolated = sum(not any((x + dx, y + dy) in occupied
                              for dx in (-1, 0, 1) for dy in (-1, 0, 1) if dx or dy)
                       for x, y in occupied)
        self.assertGreater(len(occupied), 1000)
        self.assertLess(isolated / len(occupied), 0.01)

    def test_wide_comets_have_long_connected_trails(self):
        width, height = 1024, 16
        pixels = art.render(width, height, 1, 'comet', 'deep')
        remaining = {(x, y) for y, row in enumerate(pixels)
                     for x, c in enumerate(row) if c != art.PALETTES['deep'][0]}
        largest = 0
        while remaining:
            pending = [remaining.pop()]
            count = 0
            while pending:
                x, y = pending.pop()
                count += 1
                for dx in (-1, 0, 1):
                    for dy in (-1, 0, 1):
                        neighbor = ((x + dx) % width, (y + dy) % height)
                        if neighbor in remaining:
                            remaining.remove(neighbor)
                            pending.append(neighbor)
            largest = max(largest, count)
        self.assertGreater(largest, 100)

    @mock.patch.object(art, '_core', None)  # patches a Python helper the Rust path doesn't use
    def test_quasar_jets_change_visible_pixels(self):
        pixels = art.render(64, 64, 1, 'quasar', 'deep')
        with mock.patch.object(art, '_quasar_jet_glow', return_value=0.0):
            without_jets = art.render(64, 64, 1, 'quasar', 'deep')
        changed = sum(a != b for row, other in zip(pixels, without_jets)
                      for a, b in zip(row, other))
        self.assertGreater(changed, 64)

    def test_all_styles_have_valid_dimensions_and_channels(self):
        self.assertEqual(len(art.STYLES), 48)
        for style in art.STYLES:
            with self.subTest(style=style):
                pixels = art.render(40, 28, 1, style, 'deep')
                self.assertEqual(len(pixels), 28)
                self.assertTrue(all(len(row) == 40 for row in pixels))
                self.assertTrue(all(len(c) == 3 and all(isinstance(v, int) and 0 <= v <= 255
                                                      for v in c)
                                    for row in pixels for c in row))

    def test_previously_flat_styles_have_spatial_detail(self):
        for style in ('mandelbrot', 'petals', 'starburst', 'kaleidoscope', 'horizon', 'ink'):
            with self.subTest(style=style):
                pixels = art.render(64, 48, 1, style, 'deep')
                self.assertGreater(len({c for row in pixels for c in row}), 16)
                # A silhouette scene can legitimately have only two colors in
                # a horizontal row; it must still have mountain boundaries.
                minimum = 1 if style == 'horizon' else 4
                self.assertTrue(any(len(set(row)) > minimum for row in pixels))

    def test_mandelbrot_known_interior_and_exterior(self):
        pixels = art.render(65, 65, 1, 'mandelbrot', 'deep')
        self.assertEqual(pixels[32][32], art.PALETTES['deep'][-1])
        self.assertNotEqual(pixels[0][0], pixels[32][32])

    def test_repeatable_without_global_rng_side_effects(self):
        import random
        state = random.getstate()
        for style in ('ink', 'flowfield', 'petals', 'woodblock'):
            self.assertEqual(art.render(32, 24, 7, style, 'sunset'),
                             art.render(32, 24, 7, style, 'sunset'))
        self.assertEqual(random.getstate(), state)

    def test_lissajous_spans_canvas(self):
        pixels = art.render(96, 64, 1, 'lissajous', 'deep')
        occupied = [(x, y) for y, row in enumerate(pixels)
                    for x, color in enumerate(row) if color != art.PALETTES['deep'][0]]
        self.assertGreater(max(x for x, _ in occupied) - min(x for x, _ in occupied), 30)
        self.assertGreater(max(y for _, y in occupied) - min(y for _, y in occupied), 20)

    def test_invalid_inputs(self):
        for width, height in ((0, 8), (8, -1)):
            with self.assertRaises(ValueError):
                art.render(width, height, 1, 'waves', 'deep')
        with self.assertRaises(ValueError):
            art.render(8, 8, 1, 'waves', 'missing')
        with self.assertRaises(ValueError):
            art.render(8, 8, 1, 'missing', 'deep')
        with tempfile.TemporaryDirectory() as directory:
            for fps in (0, -1, 65536, 12.5):
                with self.assertRaises(ValueError):
                    art.write_animated_png(pathlib.Path(directory) / 'bad.png',
                                           [[[(0, 0, 0)]]], 1, 1, fps=fps)

    def test_galaxy_single_pixel(self):
        pixels = art.render(1, 1, 17, 'galaxy', 'deep')
        self.assertEqual(len(pixels), 1)
        self.assertEqual(len(pixels[0]), 1)

    def test_animation_layout_timing_and_pixel_data(self):
        frames = [[[(0, 64, 255)] * 3 for _ in range(2)],
                  [[(255, 128, 0)] * 3 for _ in range(2)]]
        with tempfile.TemporaryDirectory() as directory:
            animated = pathlib.Path(directory) / 'animated.png'
            still = pathlib.Path(directory) / 'still.png'
            art.write_animated_png(animated, frames, 3, 2, fps=12)
            art.write_png(still, frames[0], 3, 2)
            parts = chunks(animated)
            self.assertEqual([tag for tag, _ in parts],
                             [b'IHDR', b'acTL', b'fcTL', b'IDAT', b'fcTL', b'fdAT', b'IEND'])
            self.assertEqual(struct.unpack('>II', parts[1][1]), (2, 0))
            for sequence, index in ((0, 2), (1, 4)):
                control = struct.unpack('>IIIIIHHBB', parts[index][1])
                self.assertEqual(control, (sequence, 3, 2, 0, 0, 1, 12, 0, 0))
            self.assertEqual(struct.unpack('>I', parts[5][1][:4])[0], 2)
            first_raw = zlib.decompress(parts[3][1])
            self.assertEqual(first_raw, zlib.decompress(dict(chunks(still))[b'IDAT']))
            self.assertEqual(len(first_raw), 2 * (1 + 3 * 3))
            self.assertEqual(len(zlib.decompress(parts[5][1][4:])), len(first_raw))

    def test_writers_reject_wrong_shape(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'bad.png'
            with self.assertRaises(ValueError):
                art.write_png(path, [[(0, 0, 0)]], 2, 1)
            with self.assertRaises(ValueError):
                art.write_animated_png(path, [], 2, 1)


    def test_every_style_uses_a_wide_palette_range(self):
        stops = art.PALETTES['deep']
        lookup = [(art.palette_color(stops, i / 100), i / 100) for i in range(101)]

        def position(color):
            return min(lookup, key=lambda item: sum((a - b) ** 2
                                                     for a, b in zip(item[0], color)))[1]

        for style in art.STYLES:
            with self.subTest(style=style):
                pixels = art.render(64, 64, 3, style, 'deep')
                positions = {position(c) for row in pixels[::3] for c in row[::3]}
                self.assertGreater(max(positions) - min(positions), 0.5)

    def test_fbm_is_unit_range(self):
        values = [art.fbm(x * 1.7, y * 2.3, 5) for x in range(-40, 40) for y in range(-40, 40)]
        self.assertGreaterEqual(min(values), 0.0)
        self.assertLessEqual(max(values), 1.0)
        self.assertAlmostEqual(sum(values) / len(values), 0.5, delta=0.1)

    def test_polar_noise_is_continuous_across_the_angle_seam(self):
        for r in (0.0, 3.0, 40.0):
            self.assertAlmostEqual(art.polar_noise(r, math.pi - 1e-9, 2, 3.0),
                                   art.polar_noise(r, -math.pi + 1e-9, 2, 3.0), places=6)

    def test_wave_styles_are_not_aliased(self):
        # Wave frequencies are relative to the canvas, so neighbouring pixels
        # become more similar as resolution grows instead of staying noisy.
        def roughness(pixels):
            return (sum(abs(a - b) for row in pixels for left, right in zip(row, row[1:])
                        for a, b in zip(left, right))
                    / (len(pixels) * (len(pixels[0]) - 1)))

        for style in ('plasma', 'spiral', 'interference', 'cymatics', 'optics', 'opart'):
            with self.subTest(style=style):
                coarse = roughness(art.render(32, 32, 1, style, 'deep'))
                fine = roughness(art.render(128, 128, 1, style, 'deep'))
                self.assertLess(fine, coarse / 2)

    def test_formerly_fixed_size_styles_scale_with_canvas(self):
        background = art.PALETTES['deep'][0]

        def coverage(style, size):
            pixels = art.render(size, size, 4, style, 'deep')
            return sum(c != background for row in pixels for c in row) / (size * size)

        for style in ('rings', 'gems'):
            with self.subTest(style=style):
                small, large = coverage(style, 48), coverage(style, 144)
                self.assertGreater(small, 0.05)
                self.assertLess(abs(small - large), 0.5 * max(small, large))

    def test_writers_store_palette_colors_unchanged(self):
        color = art.PALETTES['deep'][0]
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'still.png'
            art.write_png(path, [[color]], 1, 1)
            raw = zlib.decompress(dict(chunks(path))[b'IDAT'])
        self.assertEqual(tuple(raw[1:4]), color)

    def test_writers_use_default_file_permissions(self):
        umask = os.umask(0o022)
        try:
            with tempfile.TemporaryDirectory() as directory:
                still = pathlib.Path(directory) / 'still.png'
                animated = pathlib.Path(directory) / 'animated.png'
                art.write_png(still, [[(0, 0, 0)]], 1, 1)
                art.write_animated_png(animated, [[[(0, 0, 0)]]] * 2, 1, 1)
                for path in (still, animated):
                    self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o644)
        finally:
            os.umask(umask)

    def test_writers_never_change_the_process_umask(self):
        # os.umask() is process-wide; changing it, even briefly, would affect
        # files created concurrently by other threads.
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(art.os, 'umask', side_effect=AssertionError('umask changed')):
            art.write_png(pathlib.Path(directory) / 'still.png', [[(0, 0, 0)]], 1, 1)
            art.write_animated_png(pathlib.Path(directory) / 'animated.png',
                                   [[[(0, 0, 0)]]] * 2, 1, 1)

    def test_pruned_nearest_segment_matches_brute_force(self):
        import random
        rng = random.Random(3)
        for _ in range(200):
            segments = []
            for _ in range(rng.randint(1, 8)):
                p1 = (rng.uniform(-50, 50), rng.uniform(-50, 50))
                p2 = (p1[0], rng.uniform(-50, 50)) if rng.random() < 0.5 else (rng.uniform(-50, 50), p1[1])
                segments.append((p1, p2, min(p1[0], p2[0]), max(p1[0], p2[0]),
                                 min(p1[1], p2[1]), max(p1[1], p2[1])))
            x, y = rng.randint(-60, 60), rng.randint(-60, 60)
            self.assertEqual(art._nearest_segment(x, y, segments),
                             min(art._seg_hit(x, y, s[0], s[1]) for s in segments))

    def test_render_rgb_packs_render_output(self):
        for style in ('ink', 'drift', 'orbits'):
            with self.subTest(style=style):
                rows = art.render(23, 17, 4, style, 'sunset')
                packed = art.render_rgb(23, 17, 4, style, 'sunset')
                self.assertEqual(packed, bytes(v for row in rows for c in row for v in c))

    def test_writers_accept_packed_rgb(self):
        rows = [[(0, 64, 255), (255, 128, 0)], [(1, 2, 3), (4, 5, 6)]]
        packed = bytes(v for row in rows for c in row for v in c)
        with tempfile.TemporaryDirectory() as directory:
            from_rows = pathlib.Path(directory) / 'rows.png'
            from_bytes = pathlib.Path(directory) / 'bytes.png'
            art.write_png(from_rows, rows, 2, 2)
            art.write_png(from_bytes, packed, 2, 2)
            self.assertEqual(from_rows.read_bytes(), from_bytes.read_bytes())
            with self.assertRaises(ValueError):
                art.write_png(from_bytes, packed[:-1], 2, 2)

    def _assert_native_matches_python(self, cases, phases):
        for style in art.NATIVE_STYLES:
            if _may_drift(style):
                continue
            for width, height, seed, palette in cases:
                for phase in phases:
                    with self.subTest(style=style, size=(width, height), seed=seed, phase=phase):
                        native = art.render_rgb(width, height, seed, style, palette, phase)
                        with mock.patch.object(art, '_core', None):
                            python = art.render_rgb(width, height, seed, style, palette, phase)
                        self.assertEqual(native, python)

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    def test_native_styles_match_python_exactly(self):
        cases = ((37, 23, 1, 'deep'), (64, 48, -7, 'neon'), (1, 1, 2 ** 40 + 3, 'gold'),
                 (2, 5, 9, 'ocean'), (29, 61, 12345, 'sunset'))
        self._assert_native_matches_python(cases, (0.0, 0.37, -1.81))

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    @unittest.skipUnless(os.environ.get('ABSTRACT_ART_SLOW'), 'set ABSTRACT_ART_SLOW=1 to run')
    def test_native_styles_match_python_exactly_at_larger_sizes(self):
        cases = ((200, 150, 3, 'lagoon'), (150, 200, 2 ** 33 + 1, 'thermal'))
        self._assert_native_matches_python(cases, (0.37, 0.81))

    def test_woodblock_seeds_vary_paper_space_and_landforms(self):
        paper = art.PALETTES['watercolor'][-1]
        open_space, cropped, forms = False, False, set()
        for seed in range(12):
            pixels = art.render_woodblock(80, 60, seed, 'watercolor')
            open_space |= any(color == paper for color in pixels[0])
            cropped |= all(color != paper for row in pixels for color in row)
            scene = art._woodblock_scene(80, 60, seed)
            forms.add(tuple(layer['wave'] for layer in scene['layers']))
        self.assertTrue(open_space)
        self.assertTrue(cropped)
        self.assertGreater(len(forms), 3)

    def test_woodblock_scales_and_reshapes_without_changing_its_inks(self):
        still = art._woodblock_scene(80, 60, 5)
        doubled = art._woodblock_scene(160, 120, 5)
        moved = art._woodblock_scene(80, 60, 5, 0.25)
        for layer, larger, animated in zip(still['layers'], doubled['layers'], moved['layers']):
            self.assertEqual([2 * y for y in layer['surface']], larger['surface'][::2])
            self.assertEqual(2 * layer['spacing'], larger['spacing'])
            self.assertNotEqual(layer['surface'], animated['surface'])
            self.assertEqual(layer['color'], animated['color'])
        # Integer turns are exact identities through the public render API.
        for seed in (0, 1, 5, 7):
            first = art.render_rgb(80, 60, seed, 'woodblock', 'watercolor')
            self.assertEqual(first, art.render_rgb(80, 60, seed, 'woodblock', 'watercolor', 1))
            near = art.render_rgb(80, 60, seed, 'woodblock', 'watercolor', 1 - 1e-7)
            self.assertLess(sum(a != b for a, b in zip(first, near)), len(first) * 0.01)

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    def test_woodblock_native_matches_landforms_and_palettes(self):
        for seed in range(8):
            for palette in ('watercolor', 'deep', 'rainbow'):
                for phase in (0.0, 0.27):
                    with self.subTest(seed=seed, palette=palette, phase=phase):
                        native = art.render_rgb(123, 87, seed, 'woodblock', palette, phase)
                        with mock.patch.object(art, '_core', None):
                            python = art.render_rgb(123, 87, seed, 'woodblock', palette, phase)
                        self.assertEqual(native, python)

    def test_mosaic_tiles_are_flat_and_set_in_grout(self):
        size = 320  # 5-pixel tiles: the first row and column of each tile are grout
        scene = art._mosaic_scene(size, size, 4)
        self.assertEqual((scene['cols'], scene['rows'], scene['tile']), (64, 64, 5.0))
        pixels = art.render(size, size, 4, 'mosaic', 'rainbow')
        grout = art.PALETTES['rainbow'][0]
        for j in range(scene['rows']):
            for i in range(scene['cols']):
                y0, x0 = j * 5, i * 5
                self.assertEqual(pixels[y0][x0 + 2], grout)
                self.assertEqual(pixels[y0 + 2][x0], grout)
                inside = {pixels[y][x] for y in range(y0 + 1, y0 + 5) for x in range(x0 + 1, x0 + 5)}
                self.assertEqual(len(inside), 1, (i, j))
        with mock.patch.object(art, '_core', None):  # grout matches in both renderers
            self.assertEqual(art.render(size, size, 4, 'mosaic', 'rainbow'), pixels)

    def test_mosaic_grout_stays_even_with_fractional_tiles(self):
        width, height = 333, 517  # 5.203... pixels per tile
        scene = art._mosaic_scene(width, height, 7)
        self.assertNotEqual(scene['tile'], int(scene['tile']))
        for length, count in ((width, scene['cols']), (height, scene['rows'])):
            cells = art._mosaic_axis(length, scene['tile'], count, scene['grout'])
            # every tile appears (a last sliver thinner than the grout may not),
            # and each one starts after a grout pixel
            shown = sorted(set(cells) - {-1})
            self.assertLessEqual(set(range(count)) - set(shown), {count - 1})
            self.assertEqual(shown, list(range(len(shown))))
            for k in shown:
                first = cells.index(k)
                self.assertEqual(cells[first - 1], -1, (length, k))
        native = art.render_rgb(width, height, 7, 'mosaic', 'jewel', 0.37)
        with mock.patch.object(art, '_core', None):
            self.assertEqual(art.render_rgb(width, height, 7, 'mosaic', 'jewel', 0.37), native)

    def test_cymatics_covers_circular_and_square_modes(self):
        geometries = set()
        for seed in range(12):
            scene = art._cymatics_scene(64, 48, seed)
            geometries.add(scene['circular'])
            if not scene['circular']:
                for n, m, _, _, _ in scene['modes']:
                    self.assertNotEqual(n, m)
        self.assertEqual(geometries, {False, True})

    def test_optics_combines_caustics_film_and_diffraction(self):
        blades = set()
        for seed in range(12):
            scene = art._optics_scene(64, 48, seed)
            self.assertEqual(len(scene['waves']), 4)
            self.assertEqual(len(scene['film']), 4)
            self.assertEqual(len(scene['flares']), 3)
            blades.add(scene['blades'])
        self.assertTrue(blades.issubset({4.0, 6.0, 8.0}))
        self.assertGreaterEqual(len(blades), 2)
        full = art.render_optics(48, 36, 1, 'deep')
        with mock.patch.object(art, '_optics_caustic', return_value=0.0):
            self.assertNotEqual(full, art.render_optics(48, 36, 1, 'deep'))
        with mock.patch.object(art, '_optics_diffraction', return_value=0.0):
            self.assertNotEqual(full, art.render_optics(48, 36, 1, 'deep'))
        flat_film = dict(art._optics_scene(48, 36, 1), film=(0.0, 0.0, 0.0, math.pi / 2))
        with mock.patch.object(art, '_optics_scene', return_value=flat_film):
            self.assertNotEqual(full, art.render_optics(48, 36, 1, 'deep'))

    def test_contour_index_lines_are_wider_than_minor_lines(self):
        # A terrain that rises evenly with x: level 7 at x = 0, one level per 23.8 px.
        def ramp(x, y, seed, octaves=4, persistence=0.5, lacunarity=2.0, phase=0.0):
            return 0.5 + x * 0.0125

        def line_pixels(level):
            centre = int((level - 7) / 0.042)
            count = 0
            for x in range(centre - 6, centre + 7):
                base = math.floor(7 + 0.042 * x) / art._CONTOUR_LEVELS
                count += abs(art._contour_value(x, 0, 200, 40, 1, 0.0) - base) > 1e-9
            return count

        with mock.patch.object(art, 'fbm', ramp):
            major = line_pixels(10)
            self.assertGreater(major, line_pixels(9))
            self.assertGreater(major, line_pixels(11))

    def test_opart_covers_every_pattern_and_keeps_lenses_inside(self):
        patterns = set()
        for seed in range(16):
            scene = art._opart_scene(64, 48, seed)
            patterns.add(scene['pattern'])
            self.assertGreaterEqual(len(scene['bulges']), 2)
            for _, _, radius, strength in scene['bulges']:
                self.assertGreater(radius, 0.0)
                self.assertLess(strength, 1.0)
        self.assertEqual(patterns, {0, 1, 2})

    def test_marbling_scene_nests_drops_between_two_combs(self):
        scene = art._marbling_scene(64, 48, 4)
        kinds = [op[0] for op in scene['ops']]
        self.assertEqual(kinds.count(0.0), 10)
        self.assertEqual(kinds.count(1.0), 2)
        still = art.render_rgb(48, 36, 4, 'marbling', 'sepia')
        self.assertNotEqual(still, art.render_rgb(48, 36, 4, 'marbling', 'sepia', 0.3))

    def test_attractor_scene_is_lively_and_frames_the_orbit(self):
        for seed in (1, 2, 3):
            for step in range(24):
                params = art._attractor_scene(80, 60, seed, step / 24)['params']
                with self.subTest(seed=seed, step=step):
                    self.assertTrue(art._clifford_is_lively(params))
        for seed in range(8):
            scene = art._attractor_scene(80, 60, seed)
            self.assertEqual(len(scene['params']), 4)
            self.assertEqual(scene['points'], 6 * 80 * 60)
        pixels = art.render(64, 64, 2, 'attractor', 'thermal')
        dark = sum(color == art.PALETTES['thermal'][0] for row in pixels for color in row)
        self.assertGreater(dark, 64 * 64 // 4)
        self.assertLess(dark, 64 * 64)

    def test_lunar_craters_are_bowls_with_raised_rims_and_nothing_far_away(self):
        crater = [(0.0, 0.0, 0.2, 0.2, 1.0)]
        self.assertEqual(art._lunar_height(2.0, 2.0, crater), 0.0)
        self.assertLess(art._lunar_height(0.1, 0.0, crater), 0.0)
        self.assertGreater(art._lunar_height(0.2, 0.0, crater), 0.0)
        flat = [(0.0, 0.0, 0.2, 0.2, 0.0)]
        self.assertLess(art._lunar_height(0.0, 0.0, flat), 0.0)  # no peak: the floor stays low
        self.assertGreater(art._lunar_height(0.0, 0.0, crater), art._lunar_height(0.0, 0.0, flat))

    def test_lunar_shadows_are_long_and_follow_the_sun(self):
        wall = [(0.0, 0.0, 0.2, 0.3, 0.0)]
        toward_sun = (1.0, 0.0, 0.3, 0.0, 0.0, 1.0)
        # A pixel on the far side of the rim from the sun sits in its shadow...
        behind = art._lunar_shadow(-0.26, 0.0, art._lunar_height(-0.26, 0.0, wall), toward_sun, wall)
        # ...while the same offset on the sunward side of the crater, and open ground, are lit.
        open_ground = art._lunar_shadow(0.9, 0.0, 0.0, toward_sun, wall)
        self.assertLess(behind, 0.5)
        self.assertEqual(open_ground, 1.0)
        light = [art._lunar_scene(64, 48, 2, phase)['light'][:2] for phase in (0.0, 0.25)]
        self.assertNotEqual(light[0], light[1])
        pixels = art.render(64, 48, 2, 'lunar', 'sepia')
        shadow = sum(art.palette_color(art.PALETTES['sepia'], 0.1))
        self.assertTrue(any(sum(color) <= shadow for row in pixels for color in row))

    def test_asteroids_are_sorted_near_first_and_drift_a_whole_loop(self):
        scene = art._asteroids_scene(80, 60, 3)
        fades = [rock[10] for rock in scene['rocks']]
        self.assertEqual(len(fades), 80)
        self.assertEqual(fades, sorted(fades, reverse=True))
        self.assertEqual(scene, art._asteroids_scene(80, 60, 3, 1.0))
        moved = art._asteroids_scene(80, 60, 3, 0.25)
        self.assertNotEqual([rock[:2] for rock in scene['rocks']],
                            [rock[:2] for rock in moved['rocks']])
        for rock in scene['rocks']:  # the outline never outgrows the cull box (1.5 radii)
            self.assertLess(1.0 + rock[4] + rock[6] + rock[8], 1.5)

    def test_blackhole_horizon_is_black_and_the_photon_ring_glows(self):
        size = 128
        scene = art._blackhole_scene(size, size, 4)
        scale = size / 2.0
        (ox, oy), radius = scene['center'], scene['radius']
        cos_r, sin_r = scene['roll']
        dark = art.PALETTES['thermal'][0]
        pixels = art.render(size, size, 4, 'blackhole', 'thermal')

        def pixel(a, b):  # a, b in horizon radii, in the disk's rolled frame
            u = ox + (a * cos_r - b * sin_r) * radius
            v = oy + (b * cos_r + a * sin_r) * radius
            return pixels[round(v * scale + (size - 1) / 2.0)][round(u * scale + (size - 1) / 2.0)]

        self.assertEqual(pixel(0.0, 0.0), dark)
        ring = pixel(0.0, -1.55)  # the lensed far side of the disk arches over the top
        self.assertGreater(sum(ring), sum(art.palette_color(art.PALETTES['thermal'], 0.4)))

    def test_supernova_core_outshines_the_shell_and_the_shell_the_void(self):
        size = 96
        scene = art._supernova_scene(size, size, 5)
        scale = size / 2.0
        (ox, oy), radius = scene['center'], scene['radius']
        stops = art.PALETTES['thermal']
        pixels = art.render(size, size, 5, 'supernova', 'thermal')

        def luminance(du, dv):
            x = round((ox + du) * scale + (size - 1) / 2.0)
            y = round((oy + dv) * scale + (size - 1) / 2.0)
            return sum(pixels[y][x])

        self.assertGreater(luminance(0.0, 0.0), sum(art.palette_color(stops, 0.8)))
        shell = max(luminance(radius * math.cos(a / 8.0), radius * math.sin(a / 8.0))
                    for a in range(51))
        self.assertGreater(shell, luminance(0.0, 0.5 * radius))
        self.assertGreater(luminance(0.0, 0.5 * radius), sum(stops[0]) + 20)

    def test_every_palette_is_valid_and_starts_dark(self):
        for name, stops in art.PALETTES.items():
            with self.subTest(palette=name):
                self.assertGreaterEqual(len(stops), 5)
                for color in stops:
                    self.assertEqual(len(color), 3)
                    self.assertTrue(all(isinstance(c, int) and 0 <= c <= 255 for c in color))
                # Many styles take their background from the first stop.
                self.assertLess(max(stops[0]), 64)

    def test_every_style_renders_with_every_palette(self):
        for palette in art.PALETTES:
            for style in art.STYLES:
                with self.subTest(style=style, palette=palette):
                    self.assertEqual(len(art.render_rgb(9, 7, 3, style, palette)), 9 * 7 * 3)

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    def test_native_matches_python_for_every_palette(self):
        for palette in art.PALETTES:
            for style in ('plasma', 'nebula', 'rings', 'mandelbrot', 'orbits'):
                with self.subTest(style=style, palette=palette):
                    native = art.render_rgb(31, 23, 6, style, palette)
                    with mock.patch.object(art, '_core', None):
                        python = art.render_rgb(31, 23, 6, style, palette)
                    self.assertEqual(native, python)

    def test_looping_noise_matches_still_noise_at_phase_zero_and_loops(self):
        for x, y, seed, octaves in ((-3.7, 12.25, 77, 4), (1e3, -2e3, 0, 3), (0.5, 0.5, -9, 5)):
            still = art.fbm(x, y, seed, octaves)
            for whole in (0.0, 1.0, -2.0, 1e12):
                self.assertEqual(art.fbm(x, y, seed, octaves, phase=whole), still)
            moving = [art.fbm(x, y, seed, octaves, phase=i / 17) for i in range(1, 17)]
            self.assertGreater(len(set(moving)), 12)
            self.assertTrue(all(0.0 <= value <= 1.0 for value in moving))

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    def test_native_looping_noise_matches_python(self):
        for x, y, seed, octaves in ((-3.7, 12.25, 77, 4), (1e3, -2e3, 0, 3), (0.5, 0.5, 2 ** 32 - 1, 6)):
            for phase in (0.0, 0.1, 0.25, 0.5, 0.99, 1.7, -0.3):
                with self.subTest(seed=seed, phase=phase):
                    self.assertEqual(art._core.fbm(x, y, seed, octaves, phase),
                                     art.fbm(x, y, seed, octaves, phase=phase))

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    def test_native_style_lists_agree(self):
        self.assertEqual(sorted(art.NATIVE_STYLES), sorted(art._core.STYLES))
        self.assertLessEqual(set(art.NATIVE_STYLES), set(art.STYLES))

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    def test_native_noise_and_palette_match_python(self):
        for x, y, seed, octaves in ((-3.7, 12.25, 77, 4), (1e6, -1e6, 0, 3), (0.5, 0.5, 2 ** 32 - 1, 1)):
            self.assertEqual(art._core.fbm(x, y, seed, octaves), art.fbm(x, y, seed, octaves))
        for t in (-1.0, 0.0, 0.3, 0.999, 1.0, 2.0):
            self.assertEqual(art._core.palette_color(art.PALETTES['ember'], t),
                             art.palette_color(art.PALETTES['ember'], t))

    def test_open_curves_are_sampled_to_their_end(self):
        points = art._sample_by_length(lambda u: (u * 100.0, 0.0), 10.0, closed=False)
        self.assertEqual(points[0], (0.0, 0.0))
        self.assertEqual(points[-1], (100.0, 0.0))
        closed = art._sample_by_length(lambda u: (math.cos(math.tau * u), math.sin(math.tau * u)),
                                       0.5, closed=True)
        self.assertEqual(closed[-1], closed[0])

    def test_packed_rgb_clamps_out_of_range_channels(self):
        rows = [[(300, -5, 12.7), (0, 128, 255)]]
        self.assertEqual(art._packed_rgb(rows, 2, 1), bytes([255, 0, 12, 0, 128, 255]))

    def test_failed_still_write_preserves_existing_output(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'existing.png'
            path.write_bytes(b'previous output')
            with mock.patch.object(art.os, 'replace', side_effect=OSError('disk error')):
                with self.assertRaises(OSError):
                    art.write_png(path, [[(0, 0, 0)]], 1, 1)
            self.assertEqual(path.read_bytes(), b'previous output')
            self.assertEqual(list(path.parent.iterdir()), [path])

    def test_cli_accepts_every_registered_style(self):
        with tempfile.TemporaryDirectory() as directory:
            out = pathlib.Path(directory) / 'cli.png'
            argv = ['abstract_art.py', '--style', 'ink', '--width', '6', '--height', '5',
                    '--out', str(out)]
            with mock.patch('sys.argv', argv), \
                    mock.patch('builtins.print'):
                art.main()
            self.assertEqual(struct.unpack('>II', chunks(out)[0][1][:8]), (6, 5))
        with mock.patch('sys.argv', ['abstract_art.py', '--style', 'nope']), \
                mock.patch('sys.stderr'), self.assertRaises(SystemExit):
            art.main()

    def run_cli(self, *args):
        """Run the CLI with ``args``; return (exit code or None, stdout, stderr)."""
        out, err = io.StringIO(), io.StringIO()
        code = None
        with mock.patch('sys.argv', ['abstract_art.py', *args]), \
                mock.patch('sys.stdout', out), mock.patch('sys.stderr', err):
            try:
                art.main()
            except SystemExit as exit_:
                code = exit_.code
        return code, out.getvalue(), err.getvalue()

    def test_cli_lists_styles_and_palettes(self):
        code, out, _ = self.run_cli('--list-styles')
        self.assertIsNone(code)
        self.assertEqual(out.split(), list(art.STYLES))
        _, out, _ = self.run_cli('--list-palettes')
        self.assertEqual(out.split(), sorted(art.PALETTES))

    def test_cli_rejects_oversized_renders_unless_limits_are_off(self):
        code, _, err = self.run_cli('--width', '20000', '--height', '20000')
        self.assertEqual(code, 2)
        self.assertIn('--no-limits', err)
        code, _, err = self.run_cli('--width', '5000', '--height', '5000', '--frames', '20')
        self.assertEqual(code, 2)
        self.assertIn('frames', err)
        with mock.patch.object(art, 'MAX_FRAME_PIXELS', 4):
            with tempfile.TemporaryDirectory() as directory:
                out = str(pathlib.Path(directory) / 'big.png')
                code, _, _ = self.run_cli('--width', '3', '--height', '3', '--out', out)
                self.assertEqual(code, 2)
                code, _, _ = self.run_cli('--width', '3', '--height', '3', '--out', out,
                                          '--no-limits')
                self.assertIsNone(code)
                self.assertTrue(pathlib.Path(out).exists())

    def test_gif_lzw_round_trips_across_table_resets(self):
        import random
        rng = random.Random(3)
        for size, spread in ((1, 256), (2, 3), (100, 256), (5000, 256), (70000, 5), (70000, 256)):
            data = bytes(rng.randrange(spread) for _ in range(size))
            self.assertEqual(lzw_decode(art._gif_lzw(data)), data)
        flat = bytes(70000)
        self.assertEqual(lzw_decode(art._gif_lzw(flat)), flat)

    def test_gif_frames_use_one_table_and_the_palette_colors(self):
        stops = art.PALETTES['sunset']
        frames = [art.render_rgb(48, 32, 4, 'plasma', 'sunset', i / 3) for i in range(3)]
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'a.gif'
            art.write_gif(path, frames, 48, 32, fps=10, stops=stops)
            width, height, table, decoded, delays, loops = read_gif(path)
        self.assertEqual((width, height, len(decoded), delays, loops), (48, 32, 3, [10] * 3, True))
        for frame, indices in zip(frames, decoded):
            worst = max(
                max(abs(a - b) for a, b in zip(frame[3 * i:3 * i + 3], table[index]))
                for i, index in enumerate(indices))
            self.assertLessEqual(worst, 24)

    def test_gif_single_frame_has_no_loop_block_and_validates_input(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'one.gif'
            art.write_gif(path, [[[(255, 0, 0)]]], 1, 1)
            self.assertFalse(read_gif(path)[5])
            for args in (([], 1, 1), ([[[(0, 0, 0)]]], 1, 1, 0), ([[[(0, 0, 0)]]], 0, 1)):
                with self.assertRaises(ValueError):
                    art.write_gif(pathlib.Path(directory) / 'bad.gif', *args)
            self.assertFalse((pathlib.Path(directory) / 'bad.gif').exists())
        self.assertEqual([art._gif_delay(f) for f in (1, 12, 25, 50, 100)], [100, 8, 4, 2, 2])

    def test_cli_writes_gif_when_out_ends_in_gif(self):
        with tempfile.TemporaryDirectory() as directory:
            for frames, expected in (('1', 1), ('3', 3)):
                out = pathlib.Path(directory) / ('x%s.GIF' % frames)
                code, _, _ = self.run_cli('--width', '16', '--height', '12', '--frames', frames,
                                          '--out', str(out))
                self.assertIsNone(code)
                width, height, _, decoded, _, _ = read_gif(out)
                self.assertEqual((width, height, len(decoded)), (16, 12, expected))

    def test_gif_dimension_limit_is_checked_before_rendering(self):
        for extra in ((), ('--no-limits',)):
            with mock.patch.object(art, 'render_rgb') as render:
                code, _, _ = self.run_cli('--width', '65536', '--height', '1',
                                          '--out', 'wide.GIF', *extra)
            self.assertEqual(code, 2)
            render.assert_not_called()

    def test_gif_color_cache_stays_bounded_and_correct(self):
        colors = art._GifColors(art._gif_table(art.PALETTES['sunset']))
        colors.EXACT_LIMIT = 50
        colors.CHUNK = 100
        data = bytes((i * 7) % 256 for i in range(3 * 1000))
        small = art._GifColors(art._gif_table(art.PALETTES['sunset'])).indices(data)
        self.assertEqual(colors.indices(data), small)
        self.assertLessEqual(len(colors.exact), 50 + 100 + 256)

    def test_cli_fps_sets_the_animation_delay(self):
        with tempfile.TemporaryDirectory() as directory:
            out = pathlib.Path(directory) / 'anim.png'
            code, _, _ = self.run_cli('--width', '4', '--height', '4', '--frames', '2',
                                      '--fps', '5', '--out', str(out))
            self.assertIsNone(code)
            control = next(data for tag, data in chunks(out) if tag == b'fcTL')
            self.assertEqual(struct.unpack('>HH', control[20:24]), (1, 5))
        for bad in ('0', '65536', 'x'):
            code, _, _ = self.run_cli('--fps', bad)
            self.assertEqual(code, 2)

    def test_cli_random_seed_is_reported_and_reproducible(self):
        with tempfile.TemporaryDirectory() as directory:
            first = str(pathlib.Path(directory) / 'a.png')
            second = str(pathlib.Path(directory) / 'b.png')
            code, out, _ = self.run_cli('--random-seed', '--style', 'plasma', '--width', '8',
                                        '--height', '8', '--out', first)
            self.assertIsNone(code)
            seed = re.search(r'seed=(\d+)', out).group(1)
            self.run_cli('--seed', seed, '--style', 'plasma', '--width', '8', '--height', '8',
                         '--out', second)
            self.assertEqual(pathlib.Path(first).read_bytes(), pathlib.Path(second).read_bytes())
        code, _, _ = self.run_cli('--random-seed', '--seed', '3')
        self.assertEqual(code, 2)

    def test_parse_hex_palette(self):
        self.assertEqual(art.parse_hex_palette('#0c061c,5814a8,f80'),
                         [(12, 6, 28), (88, 20, 168), (255, 136, 0)])
        self.assertEqual(art.parse_hex_palette(' #FFFFFF , #000 '), [(255, 255, 255), (0, 0, 0)])
        for bad in ('', 'fff', '#fff,', 'fff,ggg', 'ffff,000', '0x1,fff', '1_0,fff', 'fff 000'):
            with self.subTest(text=bad), self.assertRaises(ValueError):
                art.parse_hex_palette(bad)

    def test_cli_palette_stops_match_a_registered_palette(self):
        stops = art.parse_hex_palette('0c061c,5814a8,ff8800')
        with tempfile.TemporaryDirectory() as directory:
            custom = pathlib.Path(directory) / 'custom.png'
            code, out, _ = self.run_cli('--style', 'plasma', '--width', '12', '--height', '9',
                                        '--palette-stops', '#0c061c,5814a8,ff8800',
                                        '--out', str(custom))
            self.assertIsNone(code)
            self.assertIn('palette=custom', out)
            self.assertNotIn('custom', art.PALETTES)
            with mock.patch.dict(art.PALETTES, {'mine': stops}):
                expected = art.render_rgb(12, 9, 1, 'plasma', 'mine')
            written = pathlib.Path(directory) / 'expected.png'
            art.write_png(written, expected, 12, 9)
            self.assertEqual(custom.read_bytes(), written.read_bytes())

    def test_cli_leaves_a_callers_custom_palette_alone(self):
        mine = [(0, 0, 0), (9, 9, 9)]
        with tempfile.TemporaryDirectory() as directory:
            out = str(pathlib.Path(directory) / 'a.png')
            with mock.patch.dict(art.PALETTES, {'custom': mine}):
                self.run_cli('--width', '4', '--height', '4', '--out', out)
                self.assertEqual(art.PALETTES['custom'], mine)
                self.run_cli('--width', '4', '--height', '4', '--out', out,
                             '--palette-stops', 'fff,000,f00')
                self.assertEqual(art.PALETTES['custom'], mine)

    def test_cli_rejects_bad_or_conflicting_palette_stops(self):
        for args in (('--palette-stops', 'fff'), ('--palette-stops', 'xyz,fff'),
                     ('--palette-stops', 'fff,000', '--palette', 'ocean')):
            with self.subTest(args=args):
                code, _, err = self.run_cli(*args)
                self.assertEqual(code, 2)
                self.assertIn('palette', err)

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    def test_native_orbits_matches_python_for_any_stop_count(self):
        # orbits draws randrange(len(stops)), which consumes a stop-count-dependent
        # number of random bits, so its scene depends on the palette size.
        for count in (1, 2, 3, 4, 5, 7, 8, 9, 16, 17):
            stops = [(i * 10 % 256, i * 40 % 256, i * 70 % 256) for i in range(count)]
            with mock.patch.dict(art.PALETTES, {'mine': stops}):
                for seed in (1, 2, 3, 99):
                    with self.subTest(stops=count, seed=seed):
                        native = art.render_rgb(37, 29, seed, 'orbits', 'mine')
                        with mock.patch.object(art, '_core', None):
                            python = art.render_rgb(37, 29, seed, 'orbits', 'mine')
                        self.assertEqual(native, python)

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    def test_native_matches_python_for_custom_stops(self):
        with mock.patch.dict(art.PALETTES, {'mine': art.parse_hex_palette('000,0f0,f0f,fff')}):
            for style in ('plasma', 'rings', 'torus'):
                with self.subTest(style=style):
                    native = art.render_rgb(31, 23, 4, style, 'mine')
                    with mock.patch.object(art, '_core', None):
                        python = art.render_rgb(31, 23, 4, style, 'mine')
                    self.assertEqual(native, python)

    def test_animated_styles_are_registered_and_accept_a_phase(self):
        self.assertTrue(art.ANIMATED_STYLES)
        self.assertLessEqual(art.ANIMATED_STYLES, set(art.STYLES))
        for style in art.ANIMATED_STYLES:
            self.assertIn('phase', inspect.signature(art.STYLES[style]).parameters)

    def test_animated_styles_move_and_loop(self):
        for style in sorted(art.ANIMATED_STYLES):
            with self.subTest(style=style):
                start = art.render_rgb(40, 30, 5, style, 'rainbow')
                self.assertEqual(start, art.render_rgb(40, 30, 5, style, 'rainbow', 0.0))
                self.assertNotEqual(start, art.render_rgb(40, 30, 5, style, 'rainbow', 0.25))
                # A whole loop returns to the start, up to rounding in the trigonometry.
                for whole in (1.0, -1.0, 3.0):
                    looped = art.render_rgb(40, 30, 5, style, 'rainbow', whole)
                    self.assertLessEqual(max(abs(a - b) for a, b in zip(start, looped)), 3)

    def test_constellation_lines_are_measured_between_the_moved_stars(self):
        for phase in (0.0, 0.3, 0.7):
            scene = art._constellation_scene(120, 90, 4, phase)
            stars = scene['stars']
            for i, j, dist in scene['pairs']:
                self.assertEqual(dist, math.hypot(stars[j][0] - stars[i][0],
                                                  stars[j][1] - stars[i][1]))
        still = art._constellation_scene(120, 90, 4)
        moved = art._constellation_scene(120, 90, 4, 0.3)
        self.assertEqual([p[:2] for p in still['pairs']], [p[:2] for p in moved['pairs']])

    def test_cosmos_bands_move_outward_as_the_phase_grows(self):
        # A ring's brightness is sin(2 * ring + phase), and ring index grows with distance.
        # A peak stays where 2 * ring + phase is constant, so the phase must fall for
        # the bands to move outward.
        still = art._cosmos_scene(120, 90, 4)['centers']
        moved = art._cosmos_scene(120, 90, 4, 0.1)['centers']
        for before, after in zip(still, moved):
            self.assertAlmostEqual(before[4] - after[4], art._loop(0.1, 1), places=12)

    def test_styles_without_motion_ignore_the_phase(self):
        for style in sorted(set(art.STYLES) - art.ANIMATED_STYLES):
            with self.subTest(style=style):
                self.assertEqual(art.render_rgb(24, 18, 3, style, 'deep', 0.37),
                                 art.render_rgb(24, 18, 3, style, 'deep'))

    def test_phase_must_be_a_finite_number(self):
        for bad in (float('nan'), float('inf'), '0.5', None, True):
            with self.subTest(phase=bad), self.assertRaises(ValueError):
                art.render_rgb(8, 8, 1, 'waves', 'deep', bad)
        self.assertEqual(len(art.render(8, 8, 1, 'waves', 'deep', 2)), 8)

    def test_huge_phases_are_reduced_to_a_loop_position(self):
        for style in sorted(art.ANIMATED_STYLES):
            with self.subTest(style=style):
                self.assertEqual(art.render_rgb(20, 14, 3, style, 'neon', 1e308),
                                 art.render_rgb(20, 14, 3, style, 'neon', 0.0))
                # Whole turns, however large, leave the image where it was.
                self.assertEqual(art.render_rgb(20, 14, 3, style, 'neon', 2.0 ** 40 + 0.25),
                                 art.render_rgb(20, 14, 3, style, 'neon', 0.25))
        with mock.patch.object(art, '_core', None):
            self.assertEqual(art.render_rgb(12, 9, 3, 'waves', 'neon', 1e308),
                             art.render_rgb(12, 9, 3, 'waves', 'neon', 0.0))

    def test_cli_frames_keep_their_steps_at_a_huge_phase(self):
        parsed = art._build_parser().parse_args(['--style', 'rings', '--frames', '4',
                                                 '--phase', '1e16'])
        self.assertEqual([phase for _, phase in art._frame_plan(parsed, 1)],
                         [0.0, 0.25, 0.5, 0.75])

    def test_cli_rejects_non_finite_phases_with_a_usage_error(self):
        for bad in ('nan', 'inf', '-inf', 'x'):
            with self.subTest(phase=bad):
                code, _, err = self.run_cli('--phase', bad)
                self.assertEqual(code, 2)
                self.assertIn('--phase', err)
                self.assertNotIn('Traceback', err)

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    def test_native_matches_python_at_any_phase(self):
        for style in sorted(s for s in art.ANIMATED_STYLES if not _may_drift(s)):
            for phase in (0.13, 0.5, 2.25, -0.4):
                with self.subTest(style=style, phase=phase):
                    native = art.render_rgb(37, 29, 6, style, 'neon', phase)
                    with mock.patch.object(art, '_core', None):
                        python = art.render_rgb(37, 29, 6, style, 'neon', phase)
                    self.assertEqual(native, python)

    def test_cli_frames_move_one_scene_for_animated_styles(self):
        def plan(*args):
            parsed = art._build_parser().parse_args(args)
            with mock.patch('sys.stderr', io.StringIO()) as err:
                return art._frame_plan(parsed, parsed.seed), err.getvalue()

        frames, note = plan('--style', 'rings', '--seed', '7', '--frames', '4')
        self.assertEqual(frames, [(7, 0.0), (7, 0.25), (7, 0.5), (7, 0.75)])
        self.assertEqual(note, '')
        frames, _ = plan('--style', 'rings', '--seed', '7', '--frames', '2', '--phase', '0.5')
        self.assertEqual(frames, [(7, 0.5), (7, 1.0)])
        # Every built-in style moves now, so pretend one does not.
        with mock.patch.object(art, 'ANIMATED_STYLES', frozenset()):
            frames, note = plan('--style', 'prism', '--seed', '7', '--frames', '3')
        self.assertEqual(frames, [(7, 0.0), (8, 0.0), (9, 0.0)])
        self.assertIn('slideshow', note)
        frames, note = plan('--style', 'rings', '--seed', '7', '--frames', '2', '--slideshow')
        self.assertEqual(frames, [(7, 0.0), (8, 0.0)])
        self.assertEqual(note, '')

    def test_cli_phase_renders_that_position(self):
        with tempfile.TemporaryDirectory() as directory:
            out = pathlib.Path(directory) / 'phase.png'
            code, _, _ = self.run_cli('--style', 'rings', '--width', '10', '--height', '8',
                                      '--phase', '0.3', '--out', str(out))
            self.assertIsNone(code)
            expected = pathlib.Path(directory) / 'expected.png'
            art.write_png(expected, art.render_rgb(10, 8, 1, 'rings', 'deep', 0.3), 10, 8)
            self.assertEqual(out.read_bytes(), expected.read_bytes())


if __name__ == '__main__':
    unittest.main()
