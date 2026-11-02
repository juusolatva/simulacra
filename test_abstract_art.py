#!/usr/bin/env python3

import math
import os
import pathlib
import stat
import struct
import tempfile
import unittest
import zlib
from unittest import mock

import abstract_art as art


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
        self.assertEqual(len(art.STYLES), 36)
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
        for style in ('ink', 'flowfield', 'petals'):
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

        for style in ('plasma', 'spiral', 'interference'):
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

    @unittest.skipIf(art._core is None, 'Rust core not built (see README)')
    def test_native_styles_match_python_exactly(self):
        cases = ((37, 23, 1, 'deep'), (64, 48, -7, 'neon'), (1, 1, 2 ** 40 + 3, 'gold'),
                 (2, 5, 9, 'ocean'), (29, 61, 12345, 'sunset'))
        for style in art.NATIVE_STYLES:
            for width, height, seed, palette in cases:
                with self.subTest(style=style, size=(width, height), seed=seed):
                    native = art.render_rgb(width, height, seed, style, palette)
                    with mock.patch.object(art, '_core', None):
                        python = art.render_rgb(width, height, seed, style, palette)
                    self.assertEqual(native, python)

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


if __name__ == '__main__':
    unittest.main()
