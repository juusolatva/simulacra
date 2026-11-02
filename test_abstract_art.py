#!/usr/bin/env python3

import ast
import pathlib
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

    def test_quasar_jets_change_visible_pixels(self):
        pixels = art.render(64, 64, 1, 'quasar', 'deep')
        with mock.patch.object(art, '_quasar_jet_glow', return_value=0.0):
            without_jets = art.render(64, 64, 1, 'quasar', 'deep')
        changed = sum(a != b for row, other in zip(pixels, without_jets)
                      for a, b in zip(row, other))
        self.assertGreater(changed, 64)

    def test_all_styles_have_valid_dimensions_and_channels(self):
        tree = ast.parse(pathlib.Path(art.__file__).read_text())
        styles = [node.comparators[0].value for node in ast.walk(tree)
                  if isinstance(node, ast.Compare)
                  and isinstance(node.left, ast.Name) and node.left.id == 'style']
        self.assertTrue(styles)
        for style in styles:
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


if __name__ == '__main__':
    unittest.main()
