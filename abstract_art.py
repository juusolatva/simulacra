#!/usr/bin/env python3

"""Procedurally generated abstract art.

Pure Python (stdlib only) generative art engine.

Styles:
    waves    - layered sine waves over a palette gradient
    blobs    - fractal (fBm) noise fields mapped through palettes
    orbits   - starfield with noise-driven orbital rings
    mandelbrot - classic Mandelbrot set framed by a palette gradient
    gems     - scattered round gems with banded radial shading
    rings    - concentric rings shaded by a smooth angular sweep
    aurora   - flowing wavy ribbons with soft gaussian falloff
    plasma   - sine plane waves from several directions warped by noise
    spiral   - polar spiral arms modulated by smooth winding phases
    bloom    - radial glowing cloud blooms
    flowfield - noise-driven particle flow trails
    torus    - glowing parametric torus knot curves
    constellations - starfield with connected constellation lines
    cosmos - concentric ripples from luminous centers
    nebula   - layered radial glow clouds with noise-driven starlight
    kaleidoscope - radial symmetry patterns via folded angular sectors
    petals   - radial petal flower arrangements modulated by noise
    horizon   - layered mountain silhouettes over a gradient sky
    starburst - radiating starburst rays from a central point
    interference - layered sine waves creating interference patterns
    galaxy      - spiral galaxy disk with central bulge and curved arms
    voronoi    - glowing cell clouds from randomly placed seed sites
    pinwheel   - swirling radial pinwheel arms modulated by noise
    lissajous  - glowing parametric harmonic curves (Lissajous figures)
    geo        - layered geometric rosettes with glow halos
    prism      - stacked faceted gem facets with ridge shading and glow
    shard       - fractured irregular gem shards with glow halos and cracks
    circuit    - glowing printed-circuit traces with node pads
    cosmic     - starfield with nebula glow and curved spiral arm
    quasar     - glowing relativistic accretion disk with Doppler-beamed jets
    mandala    - concentric noise-wobbled rings of glowing glyphs with spokes
    drift      - noise fields sampled through a current-warped coordinate grid
    crystal    - noise-grown terrain quantized into faceted gem crystal faces
    ember      - rising glowing embers with flicker and soft falloff
    comet      - fast glowing streaks with fading motion trails
    ink        - flowing liquid ink strokes with tapered tails and soft edges

Usage:
    python abstract_art.py --style waves --seed 7 --palette sunset --out art.png

Palette stops are sRGB colors and are written to PNG unchanged.
"""

import argparse
import contextlib
import itertools
import math
import os
import random
import re
import secrets
import struct
import sys
import zlib

try:
    # Optional Rust core (see core/); every style also has a pure-Python renderer.
    import abstract_art_core as _core
except ImportError:
    _core = None

# ---------------------------------------------------------------- palettes

PALETTES = {
    "deep": [
        (13, 19, 49),
        (27, 42, 94),
        (61, 90, 153),
        (111, 163, 206),
        (168, 208, 230),
    ],
    "sunset": [
        (26, 5, 51),
        (93, 31, 63),
        (168, 50, 74),
        (232, 99, 92),
        (249, 168, 39),
    ],
    "forest": [
        (11, 31, 18),
        (20, 61, 36),
        (47, 107, 60),
        (85, 160, 106),
        (142, 201, 160),
    ],
    "ember": [
        (26, 12, 8),
        (64, 28, 18),
        (130, 56, 30),
        (216, 102, 48),
        (255, 178, 102),
    ],
    "ocean": [
        (8, 16, 38),
        (16, 42, 72),
        (32, 72, 106),
        (56, 108, 140),
        (96, 148, 178),
    ],
    "neon": [
        (10, 0, 20),
        (40, 0, 60),
        (90, 10, 90),
        (180, 20, 160),
        (255, 110, 220),
    ],
    "gold": [
        (18, 12, 2),
        (72, 48, 8),
        (152, 102, 26),
        (226, 158, 52),
        (255, 222, 140),
    ],
    "ultraviolet": [
        (8, 2, 18),
        (28, 8, 48),
        (64, 18, 92),
        (120, 36, 150),
        (200, 140, 255),
    ],
    "lagoon": [
        (2, 16, 20),
        (8, 48, 56),
        (20, 96, 102),
        (56, 160, 156),
        (176, 230, 220),
    ],
    # Multi-hue palettes. They still start dark, because many styles draw their
    # background from the first stop, and then sweep through several hues.
    "rainbow": [
        (12, 6, 28),
        (88, 20, 168),
        (30, 96, 230),
        (24, 190, 96),
        (250, 224, 40),
        (250, 120, 24),
        (236, 40, 56),
    ],
    "candy": [
        (28, 10, 44),
        (150, 40, 150),
        (240, 90, 160),
        (120, 220, 220),
        (255, 240, 150),
    ],
    "tropical": [
        (4, 24, 34),
        (0, 110, 120),
        (30, 190, 150),
        (190, 230, 70),
        (255, 90, 130),
    ],
    "jewel": [
        (10, 6, 24),
        (20, 70, 160),
        (14, 140, 90),
        (170, 40, 140),
        (250, 190, 50),
    ],
    "vaporwave": [
        (20, 8, 48),
        (100, 40, 180),
        (240, 70, 190),
        (80, 220, 240),
        (255, 230, 245),
    ],
    "sherbet": [
        (36, 8, 52),
        (160, 30, 120),
        (250, 90, 80),
        (255, 170, 60),
        (255, 245, 190),
    ],
}


_HEX_COLOR = re.compile(r"[0-9a-fA-F]{6}|[0-9a-fA-F]{3}")


def parse_hex_palette(text):
    """Parse comma-separated hex colors such as ``"#0c061c,5814a8,f80"`` into RGB stops.

    Each color is ``rrggbb`` or the shorthand ``rgb`` (each digit doubled), with
    an optional leading ``#``. At least two colors are required. The stops
    run from the first (the background in many styles) to the last.
    """
    stops = []
    for item in text.split(","):
        digits = item.strip().removeprefix("#")
        if not _HEX_COLOR.fullmatch(digits):
            raise ValueError("invalid hex color %r (use rrggbb or rgb, e.g. #ff8800)" % item.strip())
        if len(digits) == 3:
            digits = "".join(c * 2 for c in digits)
        stops.append((int(digits[0:2], 16), int(digits[2:4], 16), int(digits[4:6], 16)))
    if len(stops) < 2:
        raise ValueError("a palette needs at least two colors")
    return stops


# ---------------------------------------------------------------- noise

def _hash(x, y, seed):
    n = (x * 3747612 + y * 6682691 + seed * 974631) & 0xFFFFFFFF
    n = (n ^ (n >> 13)) * 0x5BD1E995 & 0xFFFFFFFF
    n = (n ^ (n >> 16)) & 0xFFFFFFFF
    return n / 4294967295.0


def value_noise(x, y, seed, cell=8.0):
    gx = math.floor(x / cell)
    gy = math.floor(y / cell)
    fx = x / cell - gx
    fy = y / cell - gy
    fx = fx * fx * (3 - 2 * fx)
    fy = fy * fy * (3 - 2 * fy)
    n00 = _hash(gx, gy, seed)
    n10 = _hash(gx + 1, gy, seed)
    n01 = _hash(gx, gy + 1, seed)
    n11 = _hash(gx + 1, gy + 1, seed)
    return (n00 * (1 - fx) * (1 - fy) + n10 * fx * (1 - fy)
            + n01 * (1 - fx) * fy + n11 * fx * fy)


def fbm(x, y, seed, octaves=4, persistence=0.5, lacunarity=2.0):
    total = 0.0
    norm = 0.0
    freq = 1.0
    amp = 1.0
    for _ in range(octaves):
        total += value_noise(x * freq, y * freq, seed) * amp
        norm += amp
        amp *= persistence
        freq *= lacunarity
    return total / norm


def polar_noise(r_coord, theta, seed, around, octaves=2):
    """fBm sampled on a circle in noise space so it is continuous across theta = +/-pi.

    ``r_coord`` shifts the circle per radius; ``around`` sets its noise-space
    radius, i.e. how much the noise varies over one revolution.
    """
    return fbm(r_coord + around * math.cos(theta), around * math.sin(theta),
               seed, octaves=octaves)


def _contrast(n, gain):
    """Stretch fbm output (centered near 0.5, rarely reaching 0 or 1) around 0.5."""
    return 0.5 + (n - 0.5) * gain


# ---------------------------------------------------------------- palettes

def palette_color(stops, t):
    """Map t in [0,1] to an RGB tuple using linear interpolation."""
    t = max(0.0, min(1.0, t))
    if t <= 0.0:
        return stops[0]
    if t >= 1.0:
        return stops[-1]
    steps = len(stops) - 1
    if steps < 1:
        return stops[0]
    for i in range(steps):
        span = (i + 1) / steps
        if t <= span:
            local = (t - i / steps) * steps
            r = stops[i][0] + (stops[i + 1][0] - stops[i][0]) * local
            g = stops[i][1] + (stops[i + 1][1] - stops[i][1]) * local
            b = stops[i][2] + (stops[i + 1][2] - stops[i][2]) * local
            return (int(r), int(g), int(b))
    return stops[-1]


def _point_in_poly(x, y, poly):
    """Ray casting point-in-polygon test."""
    inside = False
    n = len(poly)
    j = n - 1
    for i in range(n):
        xi, yi = poly[i]
        xj, yj = poly[j]
        if ((yi > y) != (yj > y)) and (x < (xj - xi) * (y - yi) / (yj - yi) + xi):
            inside = not inside
        j = i
    return inside


def _poly_min_dist(x, y, poly):
    """Minimum distance from point to polygon ring segments."""
    n = len(poly)
    best = 1e9
    for i in range(n):
        x1, y1 = poly[i]
        x2, y2 = poly[(i + 1) % n]
        dx, dy = x2 - x1, y2 - y1
        seg_len2 = dx * dx + dy * dy
        if seg_len2 < 1e-12:
            d = math.hypot(x - x1, y - y1)
        else:
            t = ((x - x1) * dx + (y - y1) * dy) / seg_len2
            t = max(0.0, min(1.0, t))
            px, py = x1 + t * dx, y1 + t * dy
            d = math.hypot(x - px, y - py)
        if d < best:
            best = d
    return best


# ---------------------------------------------------------------- styles

def _loop(phase, cycles):
    """Angle that completes ``cycles`` whole turns as ``phase`` goes 0 to 1.

    Animated scenes add this to a phase or angle, so phase 1.0 repeats phase 0
    and an animation over phases ``i / frames`` loops seamlessly. ``phase`` 0
    adds exactly 0.0, which leaves a style's still image unchanged.
    """
    return math.tau * cycles * phase


def _waves_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    return {"layers": [
        (30.0, 2.0, rng.uniform(0, 6.28) + _loop(phase, 1)),
        (18.0, 3.5, rng.uniform(0, 6.28) + _loop(phase, -1)),
        (12.0, 5.0, rng.uniform(0, 6.28) + _loop(phase, 2)),
        (7.0, 7.0, rng.uniform(0, 6.28) + _loop(phase, -2)),
    ]}


def render_waves(width, height, seed, palette_name, phase=0.0):
    stops = PALETTES[palette_name]
    layers = _waves_scene(width, height, seed, phase)["layers"]
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            n = fbm(x * 0.015, y * 0.015, seed, octaves=3)
            v = 0.1 + n * 0.5
            for (amp, freq, phase) in layers:
                angle = (x + y * 0.9) * freq * math.tau / max(width, height) + phase
                v += amp / 67.0 * math.sin(angle) * (0.3 + n)
            t = min(1.0, max(0.0, v * 1.4))
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


def render_blobs(width, height, seed, palette_name):
    stops = PALETTES[palette_name]
    # Noise coordinates scale with the canvas: about 5 base cells across.
    k = 40.0 / max(width, height)
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            n = fbm(x * k, y * k, seed, octaves=5)
            row.append(palette_color(stops, _contrast(n, 2.2)))
        pixels.append(row)
    return pixels


# exp(-q) is exactly 0.0 in double precision once q exceeds ~745.13, so a
# Gaussian exp(-(d / sigma) ** 2 / 2) contributes exactly nothing beyond
# d = sigma * sqrt(2 * 746). Culling past that distance keeps output identical.
GAUSSIAN_ZERO = math.sqrt(2.0 * 746.0)


class _RowFilter:
    """Items that can affect a canvas row, kept in their original order.

    An item whose vertical center is more than its reach away from a row
    cannot affect any pixel in it (a point-to-pixel distance is never less
    than the vertical offset). The last row's list is cached, so per-pixel
    callers that scan rows in order filter once per row.
    """

    def __init__(self, items, center_y, reach):
        self._entries = [(center_y(item), reach(item), item) for item in items]
        self._row = None
        self._items = []

    def __call__(self, y):
        if y != self._row:
            self._row = y
            self._items = [item for (cy, r, item) in self._entries if abs(y - cy) <= r]
        return self._items


def _shade(width, height, stops, value):
    """Build ``height`` rows of palette colors from ``value(x, y)`` in [0, 1]."""
    return [[palette_color(stops, value(x, y)) for x in range(width)]
            for y in range(height)]


def _orbits_color(x, y, seed, stops, center, rings, stars):
    """Noise background, overridden by any ring band and then by any star."""
    n = fbm(x * 0.008, y * 0.008, seed, octaves=3)
    c = palette_color(stops, _contrast(n, 1.6))
    r = math.sqrt((x - center[0]) ** 2 + (y - center[1]) ** 2)
    for (radius, ring_color) in rings:
        if radius * 0.9 < r < radius * 1.1:
            c = ring_color
    for (sx, sy, sr) in stars(y):
        if math.hypot(x - sx, y - sy) < sr * 1.2:
            c = (255, 255, 255)
    return c


def _orbits_scene(width, height, seed, n_stops):
    """Stars and ring bands; rings store palette stop indices.

    The ring draw depends on the palette size (randrange(n_stops)), so the
    scene needs it to reproduce the same random sequence.
    """
    rng = random.Random(seed)
    stars = [
        (rng.uniform(0, width - 1), rng.uniform(0, height - 1), rng.uniform(0.5, 3.0))
        for _ in range(60)
    ]
    max_r = math.sqrt((width / 2.0) ** 2 + (height / 2.0) ** 2)
    rings = [(max_r * rng.uniform(0.15, 0.9), rng.randrange(n_stops)) for _ in range(6)]
    return {"stars": stars, "rings": rings}


def render_orbits(width, height, seed, palette_name):
    stops = PALETTES[palette_name]
    scene = _orbits_scene(width, height, seed, len(stops))
    cx = width / 2.0
    cy = height / 2.0
    rings = [(radius, stops[index]) for radius, index in scene["rings"]]
    star_rows = _RowFilter(scene["stars"], lambda s: s[1], lambda s: s[2] * 1.2 + 1.0)
    return [[_orbits_color(x, y, seed, stops, (cx, cy), rings, star_rows) for x in range(width)]
            for y in range(height)]


def render_mandelbrot(width, height, seed, palette_name):
    """Mandelbrot escape times with an aspect-correct complex-plane viewport."""
    stops = PALETTES[palette_name]
    scale = 3.2 / min(width, height)
    max_iter = 100

    def value(x, y):
        cy = ((height - 1) / 2.0 - y) * scale
        cx = (x - (width - 1) / 2.0) * scale - 0.6
        zx = zy = 0.0
        n = 0
        while zx * zx + zy * zy <= 4.0 and n < max_iter:
            zx, zy = zx * zx - zy * zy + cx, 2.0 * zx * zy + cy
            n += 1
        return 1.0 if n == max_iter else math.sqrt(n / max_iter)

    return _shade(width, height, stops, value)

def _spiral_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    arms = []
    for _ in range(rng.randint(2, 5)):
        arms.append((rng.uniform(1.0, 4.0), rng.uniform(0.0, 6.28) + _loop(phase, 1),
                     rng.uniform(0.4, 1.0), rng.uniform(0.08, 0.35)))
    return {"arms": arms}


def render_spiral(width, height, seed, palette_name, phase=0.0):
    """Polar spiral arms: smooth Gaussians placed where winding phases align."""
    stops = PALETTES[palette_name]
    arms = _spiral_scene(width, height, seed, phase)["arms"]
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    max_r = max(1.0, math.hypot(cx, cy))

    def value(x, y):
        dx = x - cx
        dy = y - cy
        r = math.sqrt(dx * dx + dy * dy) / max_r
        theta = math.atan2(dy, dx)
        v = 0.0
        for turns, phase, amp, arm_width in arms:
            # "turns" is the number of windings between the center and corners.
            d = (r * turns * math.tau - theta - phase) % math.tau
            dist = abs(d - math.pi)
            v += amp * math.exp(-((dist / arm_width) ** 2) / 2.0)
        v *= 1.0 - 0.5 * r
        return 1.0 - math.exp(-1.6 * v)

    return _shade(width, height, stops, value)



def _nebula_scene(width, height, seed):
    rng = random.Random(seed)
    diag = math.sqrt(width * width + height * height)
    clouds = []
    for _ in range(rng.randint(3, 6)):
        ccx = rng.uniform(width * 0.1, width * 0.9)
        ccy = rng.uniform(height * 0.1, height * 0.9)
        cr = rng.uniform(0.08, 0.25) * diag
        clouds.append((ccx, ccy, cr, rng.uniform(0.5, 1.5)))
    stars = [
        (rng.uniform(0, width - 1), rng.uniform(0, height - 1), rng.uniform(0.5, 2.0))
        for _ in range(40)
    ]
    return {"clouds": clouds, "stars": stars}


def render_nebula(width, height, seed, palette_name):
    """Nebula: stacked radial glow clouds over a noise starfield."""
    stops = PALETTES[palette_name]
    scene = _nebula_scene(width, height, seed)
    clouds = scene["clouds"]
    noise_k = 24.0 / max(width, height)
    star_rows = _RowFilter(scene["stars"], lambda s: s[1], lambda s: s[2] + 1.0)
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            for (ccx, ccy, cr, amp) in clouds:
                d2 = (x - ccx) ** 2 + (y - ccy) ** 2
                v += math.exp(-d2 / (2.0 * cr ** 2)) * amp
            n = fbm(x * noise_k, y * noise_k, seed, octaves=4)
            v *= max(0.0, 0.2 + 1.6 * (_contrast(n, 2.0) - 0.2))
            for (sx, sy, sr) in star_rows(y):
                if math.hypot(x - sx, y - sy) < sr:
                    v += 1.2
                    break
            row.append(palette_color(stops, 1.0 - math.exp(-1.3 * v)))
        pixels.append(row)
    return pixels


def _kaleidoscope_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    return {"folds": rng.randint(4, 8), "phase": rng.uniform(0.0, math.tau) + _loop(phase, 1)}


def render_kaleidoscope(width, height, seed, palette_name, phase=0.0):
    """Mirror a noise-modulated radial pattern into equal angular sectors."""
    stops = PALETTES[palette_name]
    scene = _kaleidoscope_scene(width, height, seed, phase)
    folds, phase = scene["folds"], scene["phase"]
    cx, cy = (width - 1) / 2.0, (height - 1) / 2.0
    scale = max(1.0, min(width, height) / 2.0)
    sector = math.tau / folds
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            dx, dy = (x - cx) / scale, (y - cy) / scale
            r = math.hypot(dx, dy)
            angle = abs((math.atan2(dy, dx) + sector / 2) % sector - sector / 2)
            u, v = r * math.cos(angle), r * math.sin(angle)
            n = fbm(u * 32, v * 32, seed, octaves=3)
            t = 0.5 + 0.5 * math.sin(r * 18 + angle * folds * 2 + n * 4 + phase)
            row.append(palette_color(stops, t * math.exp(-0.35 * r * r)))
        pixels.append(row)
    return pixels


def _petals_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    return {"count": rng.randint(5, 8), "length": rng.uniform(0.65, 0.95),
            "phase": rng.uniform(0.0, math.tau) + _loop(phase, 1)}


def render_petals(width, height, seed, palette_name, phase=0.0):
    """Radial flower with tapered petals and a luminous center."""
    stops = PALETTES[palette_name]
    scene = _petals_scene(width, height, seed, phase)
    count, length, phase = scene["count"], scene["length"], scene["phase"]
    cx, cy = (width - 1) / 2.0, (height - 1) / 2.0
    scale = max(1.0, min(width, height) / 2.0)

    def value(x, y):
        dx, dy = (x - cx) / scale, (y - cy) / scale
        r = math.hypot(dx, dy)
        theta = math.atan2(dy, dx)
        angular = (0.5 + 0.5 * math.cos(count * theta + phase)) ** 3
        radial = math.exp(-((r - length * 0.52) / (length * 0.3)) ** 2)
        center = math.exp(-(r / 0.16) ** 2)
        return angular * radial + center

    return _shade(width, height, stops, value)



def _horizon_scene(width, height, seed):
    rng = random.Random(seed)
    # (ridge height fraction, noise offset, palette position) per layer, far to near
    return {"layers": [(fraction, rng.uniform(0, 100), 0.55 - i * 0.13)
                       for i, fraction in enumerate((0.35, 0.5, 0.65, 0.8))]}


def render_horizon(width, height, seed, palette_name):
    """Layered mountain silhouettes over a vertical gradient sky."""
    stops = PALETTES[palette_name]
    ridges = []
    for fraction, offset, color in _horizon_scene(width, height, seed)["layers"]:
        ridge = [height * (fraction + 0.18 * (fbm(
            x / max(1, width - 1) * 40, offset, seed, octaves=3) - 0.5))
                 for x in range(width)]
        ridges.append((ridge, palette_color(stops, color)))
    pixels = []
    for y in range(height):
        row = []
        sky = palette_color(stops, 0.85 - 0.35 * y / max(1, height - 1))
        for x in range(width):
            color = sky
            for ridge, mountain_color in ridges:
                if y >= ridge[x]:
                    color = mountain_color
            row.append(color)
        pixels.append(row)
    return pixels

def _starburst_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    return {"rays": rng.randint(8, 14), "phase": rng.uniform(0.0, math.tau) + _loop(phase, 1)}


def render_starburst(width, height, seed, palette_name, phase=0.0):
    """Glowing rays radiating from a central focal point."""
    stops = PALETTES[palette_name]
    scene = _starburst_scene(width, height, seed, phase)
    rays, phase = scene["rays"], scene["phase"]
    cx, cy = (width - 1) / 2.0, (height - 1) / 2.0
    scale = max(1.0, min(width, height) / 2.0)

    def value(x, y):
        dx, dy = (x - cx) / scale, (y - cy) / scale
        r = math.hypot(dx, dy)
        theta = math.atan2(dy, dx)
        ray = (0.5 + 0.5 * math.cos(theta * rays + phase)) ** 12
        glow = math.exp(-(r / 0.18) ** 2)
        return glow + ray * math.exp(-r * 1.3) * (1 - math.exp(-r * 12))

    return _shade(width, height, stops, value)



def _interference_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    size = max(width, height)
    layers = []
    for turns in (1, -1, 2):
        direction = rng.uniform(0.0, math.pi)
        # Spatial frequency in cycles across the canvas, far below Nyquist.
        k = rng.uniform(4.0, 18.0) * math.tau / size
        layers.append((rng.uniform(0, 6.28) + _loop(phase, turns), k * math.cos(direction),
                       k * math.sin(direction), rng.uniform(1.0, 4.0)))
    return {"layers": layers}


def render_interference(width, height, seed, palette_name, phase=0.0):
    """Interference: plane waves from several directions summed into moire bands."""
    stops = PALETTES[palette_name]
    layers = _interference_scene(width, height, seed, phase)["layers"]
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            for (phase, kx, ky, amp) in layers:
                v += math.sin(x * kx + y * ky + phase) * amp
            n = fbm(x * 0.006, y * 0.006, seed, octaves=2)
            v += n * 2.0
            t = math.sin(v * 0.35) * 0.5 + 0.5 + (n - 0.5) * 0.3
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


def _constellation_scene(width, height, seed):
    rng = random.Random(seed)
    stars = [(rng.uniform(0, width - 1), rng.uniform(0, height - 1),
              rng.uniform(0.7, 2.2), rng.uniform(0.45, 1.0))      # x, y, radius, brightness
             for _ in range(rng.randint(28, 48))]
    max_dist = math.sqrt(width * width + height * height) * 0.32
    pairs = []
    for i, (xi, yi, _, _) in enumerate(stars):
        for j in range(i + 1, len(stars)):
            dist = math.hypot(stars[j][0] - xi, stars[j][1] - yi)
            if dist <= max_dist:
                pairs.append((i, j, dist))
    return {"stars": stars, "pairs": pairs}


def _constellation_lines(stars, pairs, width, height):
    """Rasterize lines between nearby stars into an additive glow grid."""
    line_grid = [[0.0] * width for _ in range(height)]
    for (i, j, dist) in pairs:
        _draw_constellation_line(line_grid, stars[i], stars[j], dist, width, height)
    return line_grid


def _draw_constellation_line(line_grid, start, end, dist, width, height):
    """Accumulate one line, brightest at ``start`` and fading toward ``end``."""
    sx, sy = start[0], start[1]
    ex, ey = end[0], end[1]
    steps = max(1, int(dist / 1.2))
    for k in range(steps + 1):
        t = k / steps
        ix = int(sx + (ex - sx) * t)
        iy = int(sy + (ey - sy) * t)
        if 0 <= ix < width and 0 <= iy < height:
            falloff = math.exp(-((k / steps) * (k / steps)) / 2.0)
            line_grid[iy][ix] += falloff * 0.9


def _constellation_value(x, y, stars, line_grid):
    v = 0.0
    for sx, sy, radius, brightness in stars(y):
        d = math.hypot(x - sx, y - sy)
        v += math.exp(-((d / radius) ** 2) / 2.0) * brightness
    v += line_grid[y][x]
    return min(1.0, max(0.0, 1.0 - math.exp(-v)))


def render_constellations(width, height, seed, palette_name):
    """Constellations: starfield whose stars are loosely connected by lines."""
    stops = PALETTES[palette_name]
    scene = _constellation_scene(width, height, seed)
    stars = scene["stars"]
    line_grid = _constellation_lines(stars, scene["pairs"], width, height)
    star_rows = _RowFilter(stars, lambda s: s[1], lambda s: s[2] * GAUSSIAN_ZERO + 1.0)
    return _shade(width, height, stops,
                  lambda x, y: _constellation_value(x, y, star_rows, line_grid))


def _cosmos_scene(width, height, seed):
    rng = random.Random(seed)
    centers = []
    for _ in range(rng.randint(2, 5)):
        centers.append((
            rng.uniform(width * 0.2, width * 0.8),                                  # x
            rng.uniform(height * 0.2, height * 0.8),                                # y
            rng.uniform(0.25, 0.6) * math.sqrt(width * width + height * height),    # max_r
            rng.randint(3, 8),                                                      # rings
            rng.uniform(0.0, 6.28),                                                 # phase
            rng.uniform(0.6, 1.4),                                                  # amp
        ))
    return {"centers": centers}


def render_cosmos(width, height, seed, palette_name):
    """Cosmos: concentric ripples emanating from random luminous centers."""
    stops = PALETTES[palette_name]
    centers = _cosmos_scene(width, height, seed)["centers"]

    def value(x, y):
        v = 0.0
        weight = 0.0
        for cx, cy, max_r, rings, phase, amp in centers:
            d = math.sqrt((x - cx) ** 2 + (y - cy) ** 2)
            if d < max_r:
                k = int((d / max_r) * (rings - 1) + 0.5)
                w = math.exp(-((d / (max_r / max(rings - 1, 1))) ** 2) / 2.0)
                s = math.sin(k * 2.0 + phase)
                v += w * amp * (0.5 + 0.5 * s)
                weight += w
        return min(1.0, max(0.0, v / (weight + 0.6))) * 0.95

    return _shade(width, height, stops, value)

def _galaxy_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    max_r = max(1.0, math.hypot((width - 1) / 2.0, (height - 1) / 2.0))
    bulge_r = max_r * rng.uniform(0.10, 0.26)
    bulge_amp = rng.uniform(0.9, 1.5)
    arms = []
    for _ in range(rng.randint(2, 3)):
        r0 = max_r * rng.uniform(0.015, 0.05)
        arm_width = rng.uniform(0.08, 0.18)
        arm_phase = rng.uniform(0.0, 6.28) + _loop(phase, 1)
        pitch = rng.uniform(1.5, 3.5)
        arms.append((r0, arm_width, arm_phase, pitch, rng.uniform(0.5, 1.0)))
    return {"bulge_r": bulge_r, "bulge_amp": bulge_amp, "arms": arms}


def render_galaxy(width, height, seed, palette_name, phase=0.0):
    """Galaxy: spiral disk of stars with a bright central bulge and curved arms."""
    stops = PALETTES[palette_name]
    scene = _galaxy_scene(width, height, seed, phase)
    bulge_r, bulge_amp, arms = scene["bulge_r"], scene["bulge_amp"], scene["arms"]
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    max_r = max(1.0, math.hypot(cx, cy))
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            dx = x - cx
            dy = y - cy
            r = math.sqrt(dx * dx + dy * dy)
            theta = math.atan2(dy, dx)
            n = fbm(x * 0.0035, y * 0.0035, seed, octaves=2)
            v = math.exp(-0.5 * (r / bulge_r) ** 2) * bulge_amp * (0.8 + 0.4 * n)
            for (r0, arm_width, phase, pitch, amp) in arms:
                t = math.log1p(r / r0)
                arm_theta = phase + t * pitch
                d = abs((arm_theta - theta + math.pi) % math.tau - math.pi)
                w = math.exp(-0.5 * (d / arm_width) ** 2 - r / max_r)
                v += w * amp
            t_val = 1.0 - math.exp(-v)
            row.append(palette_color(stops, t_val * 0.95))
        pixels.append(row)
    return pixels


def _bloom_scene(width, height, seed):
    rng = random.Random(seed)
    scale = max(1.0, min(width, height))
    blooms = []
    for _ in range(rng.randint(4, 9)):
        bx = rng.uniform(0, width - 1)
        by = rng.uniform(0, height - 1)
        br = scale * rng.uniform(0.08, 0.27)
        blooms.append((bx, by, br, rng.uniform(0.4, 1.0)))
    return {"blooms": blooms}


def render_bloom(width, height, seed, palette_name):
    """Radial blooms: soft glowing clouds modulated by noise."""
    stops = PALETTES[palette_name]
    blooms = _bloom_scene(width, height, seed)["blooms"]
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            n = fbm(x * 0.006, y * 0.006, seed, octaves=3)
            for (bx, by, br, amp) in blooms:
                d = math.hypot(x - bx, y - by)
                v += math.exp(-((d / br) ** 2) / 2.0) * amp
            v *= 0.4 + 1.2 * n
            row.append(palette_color(stops, 1.0 - math.exp(-1.4 * v)))
        pixels.append(row)
    return pixels


def _flowfield_scene(width, height, seed):
    rng = random.Random(seed)
    return {"starts": [(rng.uniform(0, width - 1), rng.uniform(0, height - 1))
                       for _ in range(rng.randint(40, 80))]}


def render_flowfield(width, height, seed, palette_name):
    """Particle trails following a smooth, deterministic noise vector field."""
    stops = PALETTES[palette_name]
    glow = [0.0] * (width * height)
    scale = max(1.0, min(width, height))
    step = max(0.35, scale / 200.0)
    for x, y in _flowfield_scene(width, height, seed)["starts"]:
        for _ in range(120):
            angle = fbm(x / scale * 40, y / scale * 40, seed, octaves=3) * math.tau * 2
            ox, oy = x, y
            x += math.cos(angle) * step
            y += math.sin(angle) * step
            # Subpixel segment samples keep trails connected at any resolution.
            samples = max(1, math.ceil(step * 2))
            previous = None
            for i in range(samples + 1):
                fraction = i / samples
                px = math.floor(ox + (x - ox) * fraction)
                py = math.floor(oy + (y - oy) * fraction)
                if 0 <= px < width and 0 <= py < height and (px, py) != previous:
                    glow[py * width + px] += 0.15
                    previous = (px, py)
            if not (0 <= x < width and 0 <= y < height):
                break
    return _glow_pixels(glow, width, height, stops)


def _sample_by_length(point_at, spacing, closed):
    """Sample ``point_at(u)`` for u in [0, 1] roughly ``spacing`` pixels apart.

    ``point_at`` returns a tuple whose first two items are screen x and y.
    Closed curves repeat their first point at the end; open curves end at
    ``point_at(1.0)``.
    """
    coarse = [point_at(i / 512) for i in range(513)]
    length = sum(math.hypot(b[0] - a[0], b[1] - a[1]) for a, b in zip(coarse, coarse[1:]))
    count = max(64, math.ceil(length / spacing))
    points = [point_at(i / count) for i in range(count)]
    points.append(points[0] if closed else point_at(1.0))
    return points


def _segment_box(p0, p1, reach, width, height):
    """Yield ``(row, col, distance, t)`` for pixels in the segment's box grown by ``reach``.

    ``distance`` is to the closest point on the segment, which lies at
    fraction ``t`` (0 at ``p0``, 1 at ``p1``) along it.
    """
    ax, ay = p0[0], p0[1]
    dx, dy = p1[0] - ax, p1[1] - ay
    len2 = dx * dx + dy * dy
    x0 = max(0, int(min(ax, p1[0]) - reach))
    x1 = min(width - 1, int(max(ax, p1[0]) + reach) + 1)
    y0 = max(0, int(min(ay, p1[1]) - reach))
    y1 = min(height - 1, int(max(ay, p1[1]) + reach) + 1)
    for yy in range(y0, y1 + 1):
        for xx in range(x0, x1 + 1):
            t = 0.0 if len2 < 1e-12 else min(1.0, max(0.0, ((xx - ax) * dx + (yy - ay) * dy) / len2))
            yield yy, xx, math.hypot(xx - (ax + t * dx), yy - (ay + t * dy)), t


def _torus_knot_point(tube, a, b, u):
    """Model-space (x, y, z) of the (a, b) torus knot at u in [0, 1)."""
    t = math.tau * u
    ct = math.cos(b * t)
    return ((2.0 + tube * ct) * math.cos(a * t), (2.0 + tube * ct) * math.sin(a * t),
            tube * math.sin(b * t))


def _torus_scene(width, height, seed):
    """The knot's screen-space polyline as ``(x, y, depth)`` points."""
    rng = random.Random(seed)
    tube = rng.uniform(0.6, 1.2)
    a, b = rng.choice(((2, 3), (3, 2), (3, 5), (5, 2)))
    model = [_torus_knot_point(tube, a, b, i / 2200) for i in range(2200)]
    xs, ys, zs = zip(*model)
    cx = (min(xs) + max(xs)) / 2.0
    cy = (min(ys) + max(ys)) / 2.0
    z_lo, z_hi = min(zs), max(zs)
    span = max(max(xs) - min(xs), max(ys) - min(ys), 1e-9)
    s = min(width, height) * 0.85 / span
    r_glow = max(2.0, min(width, height) * 0.016)

    def screen(u):
        x, y, z = _torus_knot_point(tube, a, b, u)
        return ((x - cx) * s + (width - 1) / 2.0, (cy - y) * s + (height - 1) / 2.0,
                0.45 + 0.55 * (z - z_lo) / max(1e-9, z_hi - z_lo))

    points = _sample_by_length(screen, max(1.0, r_glow * 0.5), closed=True)
    return {"points": points, "r_glow": r_glow}


def render_torus(width, height, seed, palette_name):
    """Torus knot: glowing parametric knot curve with depth shading."""
    stops = PALETTES[palette_name]
    scene = _torus_scene(width, height, seed)
    points, r_glow = scene["points"], scene["r_glow"]

    # Glow falls off with the distance to the curve, scaled by the depth at the
    # closest point (larger z = nearer). Keeping the maximum lets the front
    # strand win where the knot crosses itself.
    glow = [[0.0] * width for _ in range(height)]
    for p0, p1 in zip(points, points[1:]):
        for yy, xx, d, t in _segment_box(p0, p1, r_glow, width, height):
            if d < r_glow:
                depth = p0[2] + (p1[2] - p0[2]) * t
                glow[yy][xx] = max(glow[yy][xx], (1.0 - d / r_glow) * depth)

    def value(x, y):
        background = _contrast(fbm(x * 0.012, y * 0.012, seed, octaves=3), 1.6) * 0.4
        return max(background, glow[y][x])

    return _shade(width, height, stops, value)


def _voronoi_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    sites = [(rng.uniform(0, width - 1), rng.uniform(0, height - 1), rng.uniform(0.1, 0.7))
             for _ in range(rng.randint(10, 16))]
    # Each site circles its home position. The offset is measured from where the
    # circle starts, so phase 0 leaves the sites exactly where they were drawn.
    reach = max(1.0, min(width, height)) * 0.06
    drifting = []
    for i, (x, y, color) in enumerate(sites):
        start = i * 2.399963229728653  # golden angle spreads the starting points
        angle = start + _loop(phase, 1)
        drifting.append((x + reach * (math.cos(angle) - math.cos(start)),
                         y + reach * (math.sin(angle) - math.sin(start)), color))
    return {"sites": drifting}


def render_voronoi(width, height, seed, palette_name, phase=0.0):
    """Voronoi cells shaded by nearest-site distance with glowing boundaries."""
    stops = PALETTES[palette_name]
    sites = _voronoi_scene(width, height, seed, phase)["sites"]
    scale = max(1.0, min(width, height))

    def value(x, y):
        distances = sorted((math.hypot(x - sx, y - sy), color)
                           for sx, sy, color in sites)
        nearest, color = distances[0]
        gap = distances[1][0] - nearest
        edge = math.exp(-gap / max(0.5, scale * 0.012))
        return color * math.exp(-nearest / (scale * 0.3)) + edge * 0.5

    return _shade(width, height, stops, value)


def _pinwheel_scene(width, height, seed):
    rng = random.Random(seed)
    return {"n_arms": rng.randint(3, 5), "twist": rng.uniform(2.0, 7.0)}


def render_pinwheel(width, height, seed, palette_name):
    """Pinwheel: swirling radial arms with noise-modulated twist."""
    stops = PALETTES[palette_name]
    scene = _pinwheel_scene(width, height, seed)
    n_arms, twist = scene["n_arms"], scene["twist"]
    # 1.05 keeps t < 1 at the corners, so the arm width below never reaches zero.
    max_r = math.sqrt((width / 2.0) ** 2 + (height / 2.0) ** 2) * 1.05
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            dx = x - width / 2.0
            dy = y - height / 2.0
            r = math.sqrt(dx * dx + dy * dy)
            theta = math.atan2(dy, dx) if (dx or dy) else 0.0
            t = min(1.0, max(0.0, r / max_r))
            arm_phase = theta * n_arms + t * twist
            d = abs(((arm_phase + math.pi) % (2 * math.pi)) - math.pi)
            w = math.exp(-((d / (0.35 * (1.0 - t))) ** 2) / 2.0)
            n = fbm(x * 0.006, y * 0.006, seed, octaves=2)
            v = w * (1.0 - 0.35 * t) + n * 0.15
            t_val = min(1.0, max(0.0, v / 1.2))
            row.append(palette_color(stops, t_val))
        pixels.append(row)
    return pixels


def _curve_distance_grid(points, reach, width, height):
    """Distance from each pixel near the polyline to it (1e9 elsewhere)."""
    min_d = [[1e9] * width for _ in range(height)]
    for p0, p1 in zip(points, points[1:]):
        for yy, xx, d, _ in _segment_box(p0, p1, reach, width, height):
            if d < min_d[yy][xx]:
                min_d[yy][xx] = d
    return min_d


def _lissajous_value(x, y, glow_values, n_curves):
    v = 0.0
    for (min_d, sigma, amp) in glow_values:
        d = min_d[y][x]
        if d < 1e8:
            v += math.exp(-((d / sigma) ** 2) / 2.0) * amp
    return min(1.0, max(0.0, v / (n_curves * 0.9)))


def _lissajous_scene(width, height, seed):
    """Each curve as ``(points, reach, sigma, amp)``, plus the curve count."""
    rng = random.Random(seed)
    curves = []
    n_curves = rng.randint(2, 3)
    for _ in range(n_curves):
        curves.append({
            "ax": rng.uniform(0.25, 0.8),
            "ay": rng.uniform(0.25, 0.8),
            "fx": rng.uniform(1.0, 2.5),
            "fy": rng.uniform(2.0, 4.5),
            "px": rng.uniform(0.0, 6.28),
            "py": rng.uniform(0.0, 6.28),
            "amp": rng.uniform(0.5, 1.0),
            "sigma": max(0.8, min(width, height) * rng.uniform(0.008, 0.02)),
        })

    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    result = []
    for c in curves:
        def point_at(u, c=c):
            t = math.tau * u
            return (cx + cx * c["ax"] * math.sin(c["fx"] * t + c["px"]),
                    cy - cy * c["ay"] * math.sin(c["fy"] * t + c["py"]))

        reach = max(2, math.ceil(c["sigma"] * 3))
        # Non-integer frequencies leave the curve open over one period.
        points = _sample_by_length(point_at, max(1.0, reach / 2), closed=False)
        result.append((points, reach, c["sigma"], c["amp"]))
    return {"curves": result, "n_curves": n_curves}


def render_lissajous(width, height, seed, palette_name):
    """Lissajous: glowing parametric harmonic curves traced through the canvas."""
    stops = PALETTES[palette_name]
    scene = _lissajous_scene(width, height, seed)
    glow_values = [(_curve_distance_grid(points, reach, width, height), sigma, amp)
                   for points, reach, sigma, amp in scene["curves"]]
    return _shade(width, height, stops,
                  lambda x, y: _lissajous_value(x, y, glow_values, scene["n_curves"]))


def _geo_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    cx, cy = (width - 1) / 2.0, (height - 1) / 2.0
    scale = max(1.0, min(width, height) / 2.0)
    polygons = []
    for n in range(rng.randint(4, 6)):
        sides = rng.randint(3, 8)
        radius = scale * rng.uniform(0.45, 0.95)
        angle = rng.uniform(0.0, math.tau) + _loop(phase, 1 if n % 2 == 0 else -1)
        polygons.append([(cx + radius * math.cos(i * math.tau / sides + angle),
                          cy + radius * math.sin(i * math.tau / sides + angle))
                         for i in range(sides)])
    return {"polygons": polygons}


def render_geo(width, height, seed, palette_name, phase=0.0):
    stops = PALETTES[palette_name]
    polygons = _geo_scene(width, height, seed, phase)["polygons"]
    sigma = max(0.6, max(1.0, min(width, height) / 2.0) * 0.025)
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            # sum() of floats uses compensated summation (Python 3.12+); the
            # Rust core replicates it, so keep sum() here.
            v = sum(math.exp(-0.5 * (_poly_min_dist(x, y, poly) / sigma) ** 2)
                    for poly in polygons)
            row.append(palette_color(stops, 1 - math.exp(-v)))
        pixels.append(row)
    return pixels


def _prism_layers(rng, g, small):
    """Nested polygons for one gem; layer 0 spans the full gem radius."""
    layers = []
    n_layers = rng.randint(2, 4)
    shrink = rng.uniform(0.55, 0.8)
    for k in range(n_layers):
        # The innermost layer keeps (1 - shrink) of the gem radius.
        radius = g["r"] * (1.0 - shrink * k / max(1, n_layers - 1))
        sides = rng.randint(3, 6)
        phase = rng.uniform(0.0, 6.28)
        gain = rng.uniform(0.35, 0.9)
        if radius < 1.0:
            continue
        poly = [(g["cx"] + radius * math.cos(math.tau * i / sides + phase),
                 g["cy"] - radius * math.sin(math.tau * i / sides + phase))
                for i in range(sides)]
        layers.append((poly, max(2.0, gain * small * 0.025), gain))
    return layers


def _prism_value(x, y, scenes):
    v = 0.0
    for (g, layers) in scenes:
        if abs(x - g["cx"]) > g["r"] or abs(y - g["cy"]) > g["r"]:
            continue
        for (poly, ridge, gain) in layers:
            if _point_in_poly(x, y, poly):
                d = _poly_min_dist(x, y, poly)
                v += g["glow"] * (0.25 * gain + math.exp(-((d / ridge) ** 2) / 2.0))
    return 1.0 - math.exp(-1.1 * v)


def _prism_scene(width, height, seed):
    rng = random.Random(seed)
    n_gems = rng.randint(2, 5)
    small = min(width, height)
    gems = []
    for _ in range(n_gems):
        gems.append({
            "cx": rng.uniform(width * 0.15, width * 0.85),
            "cy": rng.uniform(height * 0.15, height * 0.85),
            "r": rng.uniform(small * 0.08, small * 0.28),
            "glow": rng.uniform(0.5, 1.0),
        })
    return {"gems": [(g["cx"], g["cy"], g["r"], g["glow"], _prism_layers(rng, g, small))
                     for g in gems]}


def render_prism(width, height, seed, palette_name):
    """Prism: stacked faceted gem facets with ridge shading and soft glow."""
    stops = PALETTES[palette_name]
    scenes = [({"cx": cx, "cy": cy, "r": r, "glow": glow}, layers)
              for cx, cy, r, glow, layers in _prism_scene(width, height, seed)["gems"]]
    return _shade(width, height, stops, lambda x, y: _prism_value(x, y, scenes))


def _seg_hit(x, y, p1, p2):
    """Distance from point to segment (clamped), used for crack lines."""
    x1, y1 = p1
    x2, y2 = p2
    dx, dy = x2 - x1, y2 - y1
    seg_len2 = dx * dx + dy * dy
    if seg_len2 < 1e-12:
        return math.hypot(x - x1, y - y1)
    t = ((x - x1) * dx + (y - y1) * dy) / seg_len2
    t = max(0.0, min(1.0, t))
    px, py = x1 + t * dx, y1 + t * dy
    return math.hypot(x - px, y - py)


def _make_shard(rng, width, height):
    """One irregular polygon with 1-3 cracks between non-adjacent vertices."""
    cx = rng.uniform(width * 0.2, width * 0.8)
    cy = rng.uniform(height * 0.2, height * 0.8)
    r = rng.uniform(min(width, height) * 0.15, min(width, height) * 0.42)
    m = rng.randint(5, 8)
    poly = []
    for i in range(m):
        ang = 2 * math.pi * i / m + rng.uniform(-0.12, 0.12)
        rad = r * (0.55 + rng.uniform(0.45, 1.05))
        poly.append((cx + rad * math.cos(ang), cy - rad * math.sin(ang)))
    cracks = []
    for _ in range(rng.randint(1, 3)):
        i = rng.randrange(m)
        # A non-adjacent vertex, so every crack actually crosses the shard.
        j = (i + rng.randint(2, m - 2)) % m
        cracks.append((poly[i], poly[j]))
    return {"r": r, "poly": poly, "cracks": cracks}


def _shard_value(x, y, shards):
    v = 0.0
    for s in shards:
        poly = s["poly"]
        sigma = max(1.0, s["r"] * 0.9)
        halo = math.exp(-((_poly_min_dist(x, y, poly) / sigma) ** 2) / 2.0)
        v += halo * (1.95 if _point_in_poly(x, y, poly) else 1.4)
        crack_reach = max(s["r"] * 0.12, 2.0)
        for (p1, p2) in s["cracks"]:
            d = _seg_hit(x, y, p1, p2)
            if d < crack_reach:
                v += 0.9 * math.exp(-(d / sigma) ** 2 / 2.0)
    return min(1.0, max(0.0, v / (len(shards) * 2.2)))


def _shard_scene(width, height, seed):
    rng = random.Random(seed)
    shards = [_make_shard(rng, width, height) for _ in range(rng.randint(2, 4))]
    return {"shards": [(s["r"], s["poly"], s["cracks"]) for s in shards]}


def render_shard(width, height, seed, palette_name):
    """Shard: fractured irregular gem shards with glow halos and crack lines."""
    stops = PALETTES[palette_name]
    shards = [{"r": r, "poly": poly, "cracks": cracks}
              for r, poly, cracks in _shard_scene(width, height, seed)["shards"]]
    return _shade(width, height, stops, lambda x, y: _shard_value(x, y, shards))


def _make_trace(rng, width, height):
    """Random node pads joined by PCB-style right-angled legs."""
    nodes = [(rng.uniform(width * 0.08, width * 0.92),
              rng.uniform(height * 0.08, height * 0.92))]
    for _ in range(rng.randint(2, 5)):
        nodes.append((rng.uniform(width * 0.04, width * 0.96),
                      rng.uniform(height * 0.04, height * 0.96)))
    segments = []
    for (x0, y0), (x1, y1) in zip(nodes, nodes[1:]):
        # One horizontal and one vertical leg per hop.
        corner = (x1, y0) if rng.random() < 0.5 else (x0, y1)
        segments.append(((x0, y0), corner))
        segments.append((corner, (x1, y1)))
    return nodes, segments


def _nearest_segment(x, y, segments):
    """Same value as ``min(_seg_hit(x, y, p1, p2) for ...)``, with pruning.

    A segment is skipped only when the distance to its bounding box (a lower
    bound on the distance to the segment) clearly exceeds the best distance so
    far, so the minimum, and therefore the output, is unchanged.
    """
    best = math.inf
    for (p1, p2, x0, x1, y0, y1) in segments:
        gx = max(x0 - x, 0.0, x - x1)
        gy = max(y0 - y, 0.0, y - y1)
        if gx * gx + gy * gy > (best + 1e-6) ** 2:
            continue
        d = _seg_hit(x, y, p1, p2)
        if d < best:
            best = d
    return best


def _circuit_value(x, y, traces, core, halo, pad):
    v = 0.0
    for (pad_rows, segments) in traces:
        d = _nearest_segment(x, y, segments)
        v += math.exp(-((d / core) ** 2) / 2.0) + 0.3 * math.exp(-((d / halo) ** 2) / 2.0)
        for (px, py) in pad_rows(y):
            dn = math.hypot(x - px, y - py)
            if dn < pad * 2.5:
                v += 1.5 * math.exp(-((dn / pad) ** 2) / 2.0)
    return 1.0 - math.exp(-1.2 * v)


def _circuit_scene(width, height, seed):
    rng = random.Random(seed)
    return {"traces": [_make_trace(rng, width, height) for _ in range(rng.randint(3, 6))]}


def render_circuit(width, height, seed, palette_name):
    """Circuit: glowing right-angled traces routed between node pads."""
    stops = PALETTES[palette_name]
    small = max(1.0, min(width, height))
    core = max(0.7, small * 0.008)
    halo = max(2.0, small * 0.04)
    pad = max(1.5, small * 0.018)
    traces = []
    for nodes, segments in _circuit_scene(width, height, seed)["traces"]:
        boxes = [(p1, p2, min(p1[0], p2[0]), max(p1[0], p2[0]),
                  min(p1[1], p2[1]), max(p1[1], p2[1])) for p1, p2 in segments]
        traces.append((_RowFilter(nodes, lambda n: n[1], lambda n: pad * 2.5 + 1.0), boxes))
    return _shade(width, height, stops,
                  lambda x, y: _circuit_value(x, y, traces, core, halo, pad))


def _cosmic_scene(width, height, seed):
    rng = random.Random(seed)
    stars = []
    for _ in range(rng.randint(40, 90)):
        stars.append((rng.uniform(0, width), rng.uniform(0, height),
                      rng.uniform(0.2, 1.2), rng.uniform(0.4, 1.0)))
    small = max(1.0, min(width, height))
    arm_r = rng.uniform(small * 0.5, small * 0.8)
    arm_ang = rng.uniform(0, 2 * math.pi)
    return {"stars": stars, "arm_r": arm_r, "arm_ang": arm_ang, "arm_w": rng.uniform(0.4, 1.0)}


def render_cosmic(width, height, seed, palette_name):
    """Cosmic: layered starfield with nebula glow and a bright arm."""
    stops = PALETTES[palette_name]
    scene = _cosmic_scene(width, height, seed)
    arm_r, arm_ang, arm_w = scene["arm_r"], scene["arm_ang"], scene["arm_w"]
    small = max(1.0, min(width, height))
    ox, oy = width / 2, height / 2
    glow_sigma = small * 0.55
    arm_sigma = max(1.0, small * 0.08)
    star_sigma = max(1.0, small * 0.02)
    star_rows = _RowFilter(scene["stars"], lambda s: s[1], lambda s: star_sigma * 3.0 + 1.0)

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            rad = math.hypot(x - ox, y - oy)
            v = math.exp(-((rad / glow_sigma) ** 2) / 2.0) * 0.6
            ang = math.atan2(y - oy, x - ox)
            ang_diff = (ang - arm_ang + math.pi) % (2 * math.pi) - math.pi
            dist_along = rad * math.tan(arm_w) + arm_r * math.cos(ang_diff / arm_w)
            d2 = math.hypot(x - (ox + dist_along * math.cos(arm_ang)),
                            y - (oy + dist_along * math.sin(arm_ang)))
            v += math.exp(-((d2 / arm_sigma) ** 2) / 2.0) * 1.2
            for (sx, sy, size, brightness) in star_rows(y):
                ds = math.hypot(x - sx, y - sy)
                if ds < star_sigma * 3.0:
                    v += size * brightness * math.exp(-((ds / star_sigma) ** 2) / 2.0)
            row.append(palette_color(stops, 1.0 - math.exp(-1.2 * v)))
        pixels.append(row)
    return pixels


def _quasar_jet_glow(along, perpendicular, length, opening, shadow, beta):
    """Bipolar cone emission along the visible axis, fading toward the tips."""
    distance = abs(along)
    if distance <= shadow or distance >= length:
        return 0.0
    cone_width = max(0.75, opening * (distance - shadow))
    cross_section = math.exp(-0.5 * (perpendicular / cone_width) ** 2)
    launch = min(1.0, (distance - shadow) / max(1.0, shadow * 0.5))
    beaming = 0.35 + 0.65 * (beta if along > 0 else 1.0 - beta)
    return 3.0 * cross_section * launch * math.exp(-2 * distance / length) * beaming


def _quasar_scene(width, height, seed):
    """Random layout for a quasar; shared by the Python and Rust renderers."""
    rng = random.Random(seed)
    beta = rng.uniform(0.55, 0.9)
    spin = rng.uniform(0.0, 2 * math.pi)
    r_shadow = max(1.5, min(width, height) * rng.uniform(0.05, 0.11))
    r_outer = min(width, height) * rng.uniform(1.4, 3.2)
    jet_half_angle = rng.uniform(0.15, 0.3)
    jet_len = rng.uniform(min(width, height) * 1.0, min(width, height) * 2.2)
    orbits = sorted(rng.uniform(r_shadow * 1.25, r_outer) for _ in range(rng.randint(5, 8)))
    return {
        "center": ((width - 1) / 2.0, (height - 1) / 2.0),
        "beta": beta,
        "r_shadow": r_shadow,
        "r_outer": r_outer,
        "jet_axis": (math.cos(spin), math.sin(spin)),
        "jet_half_angle": jet_half_angle,
        "jet_len": jet_len,
        # Each orbit ring has its own turbulence seed.
        "orbits": [(ro, seed + int(ro * 10.0)) for ro in orbits],
    }


def _quasar_value(x, y, seed, q):
    dx = x - q["center"][0]
    dy = y - q["center"][1]
    r_shadow, r_outer = q["r_shadow"], q["r_outer"]
    r = math.hypot(dx, dy)
    theta = math.atan2(dy, dx)
    v = 0.0
    # gravitational lensing glow ring around the shadow
    lens_r = r_shadow * 2.4
    lens_w = max(1.0, r_shadow * 0.9)
    lens_wob = 0.7 + 0.6 * polar_noise(r * 0.35, theta, seed, 2.0)
    lens_w = max(0.4, lens_w * lens_wob)
    v += math.exp(-((r - lens_r) ** 2) / (2.0 * lens_w * lens_w)) * 2.2
    # black hole shadow with a hot inner edge
    if r < r_shadow:
        hot = math.exp(-((r - r_shadow) / max(1.0, r_shadow * 0.15)) ** 2 / 2.0)
        v += hot * 2.5
    # relativistic accretion disk: glowing orbit rings, hotter toward the hole
    doppler = 0.35 + 0.65 * (0.5 - 0.5 * math.cos(theta))
    disk_scale = max(1.0, r_outer - r_shadow)
    disk_t = (r - r_shadow) / disk_scale
    disk_level = 1.0 - 0.45 * (r / max(r_outer, 1.0))
    for ro, orbit_seed in q["orbits"]:
        w = max(0.4, (ro - r_shadow * 1.2) * 0.06)
        # turbulence makes the glow rings irregular and organic
        wob = 0.35 + 0.65 * polar_noise(r * 0.5, theta, orbit_seed, 3.0, octaves=3)
        ro_t = ro + w * (wob - 0.5)
        g = math.exp(-((r - ro_t) ** 2) / (2.0 * w * w)) * doppler * disk_level
        v += g * (1.5 - 0.6 * disk_t)
    # relativistic jets along the spin axis, with cone and Doppler beaming
    ux, uy = q["jet_axis"]
    t = dx * ux + dy * uy
    perp = -dx * uy + dy * ux
    v += _quasar_jet_glow(t, perp, q["jet_len"], q["jet_half_angle"], r_shadow, q["beta"])
    # faint starfield backdrop
    n = fbm(x * 0.004, y * 0.004, seed, octaves=2)
    v += max(0.0, n - 0.5) * 0.6
    return 1.0 - math.exp(-0.75 * v)


def render_quasar(width, height, seed, palette_name):
    """Quasar: glowing relativistic accretion disk with Doppler-beamed jets and lensing glow."""
    stops = PALETTES[palette_name]
    q = _quasar_scene(width, height, seed)
    return _shade(width, height, stops, lambda x, y: _quasar_value(x, y, seed, q))


def _mandala_value(x, y, scene):
    cx, cy = scene["center"]
    dx = x - cx
    dy = y - cy
    r = math.hypot(dx, dy)
    theta = math.atan2(dy, dx)
    medallion = scene["medallion"]
    v = math.exp(-(r * r) / (2.0 * medallion * medallion)) * 3.5
    # ring glows; "wob" is the wobble amplitude as a fraction of the radius
    noise = polar_noise(r * 0.05, theta, scene["seed"], 3.0)
    for rib in scene["rings"]:
        if abs(r - rib["radius"]) > rib["reach"]:
            continue
        wobbled = rib["radius"] * (1.0 + rib["wob"] * (noise - 0.5))
        v += math.exp(-((r - wobbled) ** 2) / (2.0 * rib["size"] ** 2)) * 1.6
    for (r0, r1, phase) in scene["spokes"]:
        dang = math.atan2(math.sin(theta - phase), math.cos(theta - phase))
        if r0 <= r <= r1 and abs(dang) < 0.06:
            v += 0.9
    for (gx, gy, size) in scene["glyphs"](y):
        d = math.hypot(x - gx, y - gy)
        if d < size * 0.9:
            v += math.exp(-(d * d) / (2.0 * size * size)) * 2.2
    return 1.0 - math.exp(-0.7 * v)


def _mandala_scene(width, height, seed):
    rng = random.Random(seed)
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    max_r = max(1.0, math.sqrt(cx * cx + cy * cy))
    rings = []
    for _ in range(rng.randint(6, 12)):
        rings.append({
            "radius": max_r * rng.uniform(0.2, 0.9),
            "glyphs": rng.randint(2, 10),
            "size": max_r * rng.uniform(0.014, 0.044),
            "phase": rng.uniform(0.0, 6.28),
            "wob": rng.uniform(0.05, 0.18),
        })
    # Spokes join neighbouring rings, which requires radial order.
    rings.sort(key=lambda rib: rib["radius"])
    spokes = [(inner["radius"], outer["radius"], rng.uniform(0.0, 6.28))
              for inner, outer in zip(rings, rings[1:])]
    glyphs = [(cx + math.cos(ga) * rib["radius"], cy + math.sin(ga) * rib["radius"], rib["size"])
              for rib in rings
              for ga in (rib["phase"] + (gi / rib["glyphs"]) * 2.0 * math.pi
                         for gi in range(rib["glyphs"]))]
    return {"center": (cx, cy), "medallion": max(3.0, max_r * 0.06),
            "rings": [(rib["radius"], rib["size"], rib["wob"]) for rib in rings],
            "spokes": spokes, "glyphs": glyphs}


def render_mandala(width, height, seed, palette_name):
    """Concentric mandala: noise-wobbled rings of glowing glyphs with radial spokes and a center medallion."""
    stops = PALETTES[palette_name]
    scene = _mandala_scene(width, height, seed)
    rings = []
    for radius, size, wob in scene["rings"]:
        # Wobble moves a ring by at most radius * wob / 2; beyond that plus the
        # Gaussian's zero distance the ring contributes exactly 0.0.
        rings.append({"radius": radius, "size": size, "wob": wob,
                      "reach": radius * wob * 0.5 + size * GAUSSIAN_ZERO + 1.0})
    glyph_rows = _RowFilter(scene["glyphs"], lambda g: g[1], lambda g: g[2] * 0.9 + 1.0)
    value_scene = {"center": scene["center"], "medallion": scene["medallion"], "seed": seed,
                   "rings": rings, "spokes": scene["spokes"], "glyphs": glyph_rows}
    return _shade(width, height, stops, lambda x, y: _mandala_value(x, y, value_scene))


def render_drift(width, height, seed, palette_name):
    """Drift: liquid flow-fields where nested noise currents warp the sampling grid,
    producing smooth organic contours that feel like slow-moving currents."""
    stops = PALETTES[palette_name]
    # Noise coordinates scale with the canvas so the composition is size-independent.
    k = 32.0 / max(width, height)
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            u, w = x * k, y * k
            # current offsets from low-frequency turbulence
            ox = (fbm(u * 0.55, w * 0.55, seed, octaves=3) - 0.5) * 24.0
            oy = (fbm(u * 0.5 + 2.1, w * 0.5 + 5.7, seed, octaves=2) - 0.5) * 18.0
            # sample the value field displaced along the current
            v = fbm(u + ox, w + oy, seed, octaves=4)
            row.append(palette_color(stops, _contrast(v, 2.6)))
        pixels.append(row)
    return pixels


def _crystal_scene(width, height, seed):
    """The fixed light and its Blinn-Phong half vector (no random draws)."""
    light = (0.45, -0.55, 0.70)
    ln = math.sqrt(sum(c * c for c in light))
    light = (light[0] / ln, light[1] / ln, light[2] / ln)
    hx, hy, hz = light[0], light[1], light[2] + 1.0  # half vector, viewer at +z
    hn = math.sqrt(hx * hx + hy * hy + hz * hz)
    return {"light": light, "half": (hx / hn, hy / hn, hz / hn)}


def render_crystal(width, height, seed, palette_name):
    """Crystal: noise-grown terrain quantized into low-poly faceted gem faces.

    A height field is grown from fBm noise and its gradient is measured with
    finite differences. Each surface normal is snapped to one of eight
    compass directions and a few discrete tilts, so neighbouring pixels share
    flat facets that are lit with diffuse and Blinn-Phong specular terms.
    """
    stops = PALETTES[palette_name]
    scene = _crystal_scene(width, height, seed)
    light, half = scene["light"], scene["half"]
    k = 28.0 / max(width, height)  # about 3.5 base noise cells across the canvas
    # Per-pixel slopes scale with k (median ~0.064 per noise unit); this maps the
    # typical slope to a ~45 degree facet tilt at any canvas size.
    relief = 14.0 / k
    tilt_step = math.pi / 10
    az_step = math.pi / 4

    field = [[fbm(x * k, y * k, seed, octaves=3) for x in range(width)]
             for y in range(height)]

    pixels = []
    for y in range(height):
        row = []
        ym, yp = max(0, y - 1), min(height - 1, y + 1)
        for x in range(width):
            xm, xp = max(0, x - 1), min(width - 1, x + 1)
            gx = (field[y][xp] - field[y][xm]) / (xp - xm) if xp != xm else 0.0
            gy = (field[yp][x] - field[ym][x]) / (yp - ym) if yp != ym else 0.0
            tilt = round(math.atan(math.hypot(gx, gy) * relief) / tilt_step) * tilt_step
            azimuth = round(math.atan2(gy, gx) / az_step) * az_step
            n = (-math.sin(tilt) * math.cos(azimuth),
                 -math.sin(tilt) * math.sin(azimuth),
                 math.cos(tilt))
            diffuse = max(0.0, n[0] * light[0] + n[1] * light[1] + n[2] * light[2])
            spec = max(0.0, n[0] * half[0] + n[1] * half[1] + n[2] * half[2]) ** 24
            t = 0.75 * diffuse * (0.5 + field[y][x]) + 0.5 * spec - 0.2
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


def _glow_pixels(values, width, height, stops):
    """Map a row-major intensity buffer to a rectangular RGB image."""
    return [[palette_color(stops, 1.0 - math.exp(-max(0.0, value)))
             for value in values[y * width:(y + 1) * width]]
            for y in range(height)]


def _ember_scene(width, height, seed):
    rng = random.Random(seed)
    return {"embers": [(rng.uniform(0.0, width - 1.0),          # x
                        rng.uniform(0.0, height - 1.0),         # y
                        rng.uniform(0.15, 0.55) * height,       # rise
                        rng.uniform(0.6, 2.4),                  # size
                        rng.uniform(0.0, 6.28),                 # phase
                        rng.uniform(0.5, 1.0))                  # flicker
                       for _ in range(rng.randint(60, 110))]}


def render_ember(width, height, seed, palette_name):
    """Ember: rising glowing embers with flicker and soft radial falloff."""
    stops = PALETTES[palette_name]
    glow_sum = [0.0] * (width * height)
    for cx, y0, rise, size, phase, base_flicker in _ember_scene(width, height, seed)["embers"]:
        flicker = base_flicker * (0.55 + 0.45 * math.sin(phase + 2.0 * math.sin(phase)))
        cy_top = y0 - rise
        # The horizontal falloff is exactly 0.0 once |dx| >= size * 2 + 0.01.
        reach = size * 2.0 + 0.01 + 1.0
        x_lo = max(0, math.floor(cx - reach))
        x_hi = min(width, math.ceil(cx + reach) + 1)
        for yy in range(height):
            dy = yy - cy_top
            if abs(dy) > rise * 3.0:
                continue
            fall = max(0.0, 1.0 - abs(dy) / (rise * 2.5 + 0.01))
            if fall < 0.02:
                continue
            glow = fall ** 2 * (0.35 + 0.65 * flicker)
            for xx in range(x_lo, x_hi):
                dx = xx - cx
                g = glow * max(0.0, 1.0 - (dx / ((size * 2.0) + 0.01)) ** 2) ** 2
                glow_sum[yy * width + xx] += g
    return _glow_pixels(glow_sum, width, height, stops)


def _stamp_comet(glow_sum, comet, width, height):
    """Paint one comet's fading streak; the canvas wraps at its edges."""
    x0, y0, angle, speed, radius = comet
    steps = max(1, math.ceil(speed * 2))
    reach = math.ceil(radius * 2)
    for s in range(steps):
        t = (s + 0.5) / steps
        px = x0 + math.cos(angle) * speed * t
        py = y0 + math.sin(angle) * speed * t
        ix, iy = math.floor(px), math.floor(py)
        fall = max(0.0, 1.0 - t * 1.6)
        g = fall ** 3 * (0.4 + 0.6 * (1.0 - t))
        for ddx in range(-reach, reach + 1):
            for ddy in range(-reach, reach + 1):
                dx, dy = ix + ddx - px, iy + ddy - py
                d2 = dx * dx + dy * dy
                if d2 <= (radius * 2) ** 2:
                    xx, yy = (ix + ddx) % width, (iy + ddy) % height
                    glow_sum[yy * width + xx] += g * math.exp(-2 * d2 / radius ** 2)


def _comet_scene(width, height, seed):
    rng = random.Random(seed)
    # (x, y, angle, speed, brush radius in pixels)
    return {"comets": [(rng.uniform(0.0, width - 1.0), rng.uniform(0.0, height - 1.0),
                        rng.uniform(0.0, 2.0 * math.pi), rng.uniform(0.2, 0.6) * width,
                        rng.uniform(0.8, 3.0))
                       for _ in range(rng.randint(3, 6))]}


def render_comet(width, height, seed, palette_name):
    """Comet: fast glowing streaks with fading motion trails."""
    stops = PALETTES[palette_name]
    glow_sum = [0.0] * (width * height)
    for comet in _comet_scene(width, height, seed)["comets"]:
        _stamp_comet(glow_sum, comet, width, height)
    return _glow_pixels(glow_sum, width, height, stops)


def _ink_scene(width, height, seed):
    """Strokes as ``(x0, y0, x1, y1, cx, cy, brush, alpha, steps)``."""
    rng = random.Random(seed)
    strokes = []
    for _ in range(rng.randint(5, 9)):
        x0, y0 = rng.uniform(0, width - 1), rng.uniform(0, height - 1)
        x1, y1 = rng.uniform(0, width - 1), rng.uniform(0, height - 1)
        cx, cy = rng.uniform(0, width - 1), rng.uniform(0, height - 1)
        brush = max(0.8, min(width, height) * rng.uniform(0.008, 0.025))
        alpha = rng.uniform(0.35, 0.9)
        length = math.hypot(cx - x0, cy - y0) + math.hypot(x1 - cx, y1 - cy)
        strokes.append((x0, y0, x1, y1, cx, cy, brush, alpha, max(8, math.ceil(length * 2))))
    return {"strokes": strokes}


def render_ink(width, height, seed, palette_name):
    """Quadratic Bezier ink strokes rasterized with a tapered soft brush."""
    stops = PALETTES[palette_name]
    glow = [0.0] * (width * height)
    for x0, y0, x1, y1, cx, cy, brush, alpha, steps in _ink_scene(width, height, seed)["strokes"]:
        for i in range(steps + 1):
            t = i / steps
            u = 1 - t
            px = u * u * x0 + 2 * u * t * cx + t * t * x1
            py = u * u * y0 + 2 * u * t * cy + t * t * y1
            radius = max(0.5, brush * (1 - t) ** 0.7)
            reach = math.ceil(radius * 2)
            for yy in range(max(0, int(py) - reach), min(height, int(py) + reach + 1)):
                for xx in range(max(0, int(px) - reach), min(width, int(px) + reach + 1)):
                    d2 = (xx - px) ** 2 + (yy - py) ** 2
                    if d2 <= (radius * 2) ** 2:
                        glow[yy * width + xx] += alpha * math.exp(-2 * d2 / radius ** 2) * 0.3
    return _glow_pixels(glow, width, height, stops)


def _gems_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    small = max(1.0, min(width, height))
    gems = []
    for _ in range(3):
        cx = rng.uniform(0, width - 1)
        cy = rng.uniform(0, height - 1)
        cr = small * rng.uniform(0.1, 0.25)
        gems.append((cx, cy, cr, rng.uniform(0, 6.28) + _loop(phase, 1)))
    return {"gems": gems}


def render_gemstones(width, height, seed, palette_name, phase=0.0):
    stops = PALETTES[palette_name]
    gems = _gems_scene(width, height, seed, phase)["gems"]

    def pixel(x, y):
        c = stops[0]
        for (cx, cy, cr, rot) in gems:
            dx = x - cx
            dy = y - cy
            d2 = dx * dx + dy * dy
            if d2 < cr * cr:
                t = math.sqrt(d2) / cr
                v = 1.0 - t
                local = (math.cos(rot + t * 3.0) + 1.0) / 2.0
                c = palette_color(stops, min(1.0, max(0.0, v * local)))
        return c

    return [[pixel(x, y) for x in range(width)] for y in range(height)]

def _rings_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    scale = max(1.0, min(width, height) / 2.0)
    # Neighbouring rings turn in opposite directions.
    return {"rings": [(scale * rng.uniform(0.15, 0.95),
                       rng.uniform(0.0, math.tau) + _loop(phase, 1 if i % 2 == 0 else -1))
                      for i in range(rng.randint(4, 9))]}


def render_rings(width, height, seed, palette_name, phase=0.0):
    """Concentric rings, each shaded by a smooth sweep of hue around its circle."""
    stops = PALETTES[palette_name]
    rings_cfg = _rings_scene(width, height, seed, phase)["rings"]
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0

    def pixel(x, y):
        c = stops[0]
        r = math.sqrt((x - cx) ** 2 + (y - cy) ** 2)
        for (radius, phase) in rings_cfg:
            if radius * 0.9 < r < radius * 1.1:
                angle = math.atan2(y - cy, x - cx)
                c = palette_color(stops, 0.3 + 0.35 * (1.0 + math.cos(angle + phase)))
        return c

    return [[pixel(x, y) for x in range(width)] for y in range(height)]


def _aurora_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    ribbons = []
    for i in range(rng.randint(3, 7)):
        ribbons.append((
            rng.uniform(0, height - 1),                       # y0
            max(1.0, height * rng.uniform(0.06, 0.27)),       # amp
            rng.uniform(1.0, 5.0) / max(1, width),            # freq
            rng.uniform(0.0, 6.28) + _loop(phase, 1 if i % 2 == 0 else -1),  # phase
            rng.uniform(0.6, 1.4),                            # bw
        ))
    return {"ribbons": ribbons}


def render_aurora(width, height, seed, palette_name, phase=0.0):
    """Flowing ribbons: wavy curves sampled across the canvas, blurred by gaussian weighting."""
    stops = PALETTES[palette_name]
    ribbons = _aurora_scene(width, height, seed, phase)["ribbons"]

    def value(x, y):
        v = 0.0
        for y0, amp, freq, phase, bw in ribbons:
            yy = y0 + amp * math.sin(x * freq + phase)
            d = (y - yy) / amp
            w = math.exp(-d * d)
            s = math.sin(x * freq * 2.0 + phase)
            v += w * (0.5 + 0.5 * s) * bw
        return 0.05 + 0.95 * (1.0 - math.exp(-1.5 * v))

    return _shade(width, height, stops, value)

def _plasma_scene(width, height, seed, phase=0.0):
    rng = random.Random(seed)
    size = max(width, height)
    layers = []
    # (amplitude, cycles across the canvas) - kept well below the Nyquist limit
    for amp, cycles, turns in ((1.0, 1.5, 1), (0.8, 2.5, -1), (0.6, 4.0, 2), (0.45, 6.5, -1)):
        direction = rng.uniform(0.0, math.tau)
        k = cycles * math.tau / size
        layers.append((amp, k * math.cos(direction), k * math.sin(direction),
                       rng.uniform(0.0, 6.28) + _loop(phase, turns)))
    return {"layers": layers}


def render_plasma(width, height, seed, palette_name, phase=0.0):
    """Plasma effect: sine plane waves from several directions warped by noise."""
    stops = PALETTES[palette_name]
    layers = _plasma_scene(width, height, seed, phase)["layers"]
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            n = fbm(x * 0.01, y * 0.01, seed, octaves=4)
            for (amp, kx, ky, phase) in layers:
                v += amp * math.sin(x * kx + y * ky + phase)
            row.append(palette_color(stops, 0.5 + 0.5 * math.sin(v * 1.5 + n * 3.0)))
        pixels.append(row)
    return pixels


# ---------------------------------------------------------------- render dispatch

# Single source of truth for style names: used by render(), the CLI and tests.
STYLES = {
    "waves": render_waves,
    "blobs": render_blobs,
    "orbits": render_orbits,
    "mandelbrot": render_mandelbrot,
    "gems": render_gemstones,
    "rings": render_rings,
    "aurora": render_aurora,
    "plasma": render_plasma,
    "spiral": render_spiral,
    "nebula": render_nebula,
    "kaleidoscope": render_kaleidoscope,
    "petals": render_petals,
    "horizon": render_horizon,
    "starburst": render_starburst,
    "interference": render_interference,
    "constellations": render_constellations,
    "cosmos": render_cosmos,
    "galaxy": render_galaxy,
    "bloom": render_bloom,
    "flowfield": render_flowfield,
    "torus": render_torus,
    "voronoi": render_voronoi,
    "pinwheel": render_pinwheel,
    "lissajous": render_lissajous,
    "geo": render_geo,
    "prism": render_prism,
    "shard": render_shard,
    "circuit": render_circuit,
    "cosmic": render_cosmic,
    "quasar": render_quasar,
    "mandala": render_mandala,
    "drift": render_drift,
    "crystal": render_crystal,
    "ember": render_ember,
    "comet": render_comet,
    "ink": render_ink,
}


def _seed32(seed):
    """The noise hash only uses a seed modulo 2**32; the Rust core takes it as u32."""
    return seed & 0xFFFFFFFF


def _quasar_native_scene(width, height, seed):
    q = _quasar_scene(width, height, seed)
    return dict(q, orbits=[(ro, _seed32(orbit_seed)) for ro, orbit_seed in q["orbits"]])


def _scene_without_stops(builder):
    return lambda width, height, seed, stops, phase=0.0: builder(width, height, seed)


def _animated_scene(builder):
    """Like ``_scene_without_stops`` for styles whose scene moves with ``phase``."""
    return lambda width, height, seed, stops, phase=0.0: builder(width, height, seed, phase)


def _no_scene(width, height, seed, stops, phase=0.0):
    return {}


# Styles with a Rust implementation, mapped to the scene builder whose plain
# data (numbers, tuples, lists) the core receives. Python still makes every
# random draw, and native output is tested to be byte-identical to Python.
NATIVE_STYLES = {
    "aurora": _animated_scene(_aurora_scene),
    "blobs": _no_scene,
    "bloom": _scene_without_stops(_bloom_scene),
    "circuit": _scene_without_stops(_circuit_scene),
    "cosmic": _scene_without_stops(_cosmic_scene),
    "cosmos": _scene_without_stops(_cosmos_scene),
    "crystal": _scene_without_stops(_crystal_scene),
    "drift": _no_scene,
    "galaxy": _animated_scene(_galaxy_scene),
    "gems": _animated_scene(_gems_scene),
    "geo": _animated_scene(_geo_scene),
    "horizon": _scene_without_stops(_horizon_scene),
    "interference": _animated_scene(_interference_scene),
    "kaleidoscope": _animated_scene(_kaleidoscope_scene),
    "mandala": _scene_without_stops(_mandala_scene),
    "mandelbrot": _no_scene,
    "nebula": _scene_without_stops(_nebula_scene),
    "orbits": lambda width, height, seed, stops, phase=0.0: _orbits_scene(
        width, height, seed, len(stops)),
    "petals": _animated_scene(_petals_scene),
    "pinwheel": _scene_without_stops(_pinwheel_scene),
    "plasma": _animated_scene(_plasma_scene),
    "prism": _scene_without_stops(_prism_scene),
    "quasar": _scene_without_stops(_quasar_native_scene),
    "rings": _animated_scene(_rings_scene),
    "shard": _scene_without_stops(_shard_scene),
    "spiral": _animated_scene(_spiral_scene),
    "starburst": _animated_scene(_starburst_scene),
    "voronoi": _animated_scene(_voronoi_scene),
    "waves": _animated_scene(_waves_scene),
    "torus": _scene_without_stops(_torus_scene),
    "lissajous": _scene_without_stops(_lissajous_scene),
    "constellations": _scene_without_stops(_constellation_scene),
    "ink": _scene_without_stops(_ink_scene),
    "flowfield": _scene_without_stops(_flowfield_scene),
    "comet": _scene_without_stops(_comet_scene),
    "ember": _scene_without_stops(_ember_scene),
}


# Styles that move with ``phase``, a loop position where 1.0 repeats 0.0. Other
# styles ignore it, so their frames only differ by seed. Each is tested to match
# its still image at phase 0 and to loop.
ANIMATED_STYLES = frozenset({
    "aurora", "galaxy", "gems", "geo", "interference", "kaleidoscope", "petals",
    "plasma", "rings", "spiral", "starburst", "voronoi", "waves",
})


def _render_native(style, width, height, seed, stops, phase=0.0):
    scene = NATIVE_STYLES[style](width, height, seed, stops, phase)
    return _core.render(style, width, height, _seed32(seed), stops, scene)


def _render_python(style, width, height, seed, palette_name, phase):
    if style in ANIMATED_STYLES:
        return STYLES[style](width, height, seed, palette_name, phase)
    return STYLES[style](width, height, seed, palette_name)


def _check_render_args(width, height, style, palette_name, phase=0.0):
    _validate_dimensions(width, height)
    if isinstance(phase, bool) or not isinstance(phase, (int, float)) or not math.isfinite(phase):
        raise ValueError("phase must be a finite number")
    if palette_name not in PALETTES:
        raise ValueError("unknown palette: %s" % palette_name)
    if style not in STYLES:
        raise ValueError("unknown style: %s" % style)


def _use_native(style):
    return _core is not None and style in NATIVE_STYLES


def render(width, height, seed, style, palette_name, phase=0.0):
    """Render ``style`` and return ``height`` rows of ``width`` RGB tuples.

    ``phase`` is the position in a loop (1.0 repeats 0.0) for the styles in
    ``ANIMATED_STYLES``; other styles ignore it.
    """
    _check_render_args(width, height, style, palette_name, phase)
    if _use_native(style):
        rgb = _render_native(style, width, height, seed, PALETTES[palette_name], phase)
        stride = width * 3
        return [list(zip(rgb[o:o + stride:3], rgb[o + 1:o + stride:3], rgb[o + 2:o + stride:3]))
                for o in range(0, stride * height, stride)]
    return _render_python(style, width, height, seed, palette_name, phase)


def render_rgb(width, height, seed, style, palette_name, phase=0.0):
    """Render ``style`` as packed RGB bytes: row-major, 3 bytes per pixel.

    Much smaller and faster than ``render()`` for large images, since no
    per-pixel tuples are created when the Rust core renders the style.
    ``phase`` is as for ``render()``.
    """
    _check_render_args(width, height, style, palette_name, phase)
    if _use_native(style):
        return _render_native(style, width, height, seed, PALETTES[palette_name], phase)
    return _packed_rgb(_render_python(style, width, height, seed, palette_name, phase),
                       width, height)


# ---------------------------------------------------------------- png writer

def _validate_dimensions(width, height):
    if (not isinstance(width, int) or not isinstance(height, int)
            or width <= 0 or height <= 0):
        raise ValueError("width and height must be positive integers")


def _png_chunk(tag, data):
    return (struct.pack(">I", len(data)) + tag + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))


def _packed_rgb(pixels, width, height):
    """Validate a frame and return it as packed RGB bytes.

    ``pixels`` is either ``height`` rows of ``width`` RGB tuples (channels are
    clamped to 0..255) or a bytes-like object of ``width * height * 3`` bytes.
    """
    _validate_dimensions(width, height)
    if isinstance(pixels, (bytes, bytearray, memoryview)):
        data = bytes(pixels)
        if len(data) != width * height * 3:
            raise ValueError("pixel buffer must contain width * height * 3 bytes")
        return data
    if len(pixels) != height or any(len(row) != width for row in pixels):
        raise ValueError("pixel array must match width and height")
    try:
        # Fast path for the usual case: integer channels already in 0..255.
        data = b"".join(bytes(itertools.chain.from_iterable(row)) for row in pixels)
        if len(data) == width * height * 3:
            return data
    except (TypeError, ValueError):
        pass
    data = bytearray()
    for row in pixels:
        for r, g, b in row:
            data.extend((min(255, max(0, int(r))),
                         min(255, max(0, int(g))),
                         min(255, max(0, int(b)))))
    return bytes(data)


def _png_pixels(pixels, width, height):
    """Encode 8-bit sRGB samples as compressed PNG scanlines.

    Palette stops are authored as sRGB colors, so samples are written as-is.
    """
    data = _packed_rgb(pixels, width, height)
    stride = width * 3
    # Each scanline starts with filter type 0 (None).
    raw = b"".join(b"\x00" + data[o:o + stride] for o in range(0, len(data), stride))
    return zlib.compress(raw, 9)


def _png_header(width, height):
    _validate_dimensions(width, height)
    return (b"\x89PNG\r\n\x1a\n" + _png_chunk(
        b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)))


def _create_temporary(directory):
    """Create a new, exclusive temporary file next to the destination.

    The file is opened with mode 0o666 so the kernel applies the process umask
    exactly as for a normal open(). Reading the umask with os.umask() would
    briefly change it for every thread in the process.
    """
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_BINARY", 0)
    for _ in range(100):
        name = os.path.join(directory, ".abstract-art-%s.tmp" % os.urandom(8).hex())
        try:
            return name, os.open(name, flags, 0o666)
        except FileExistsError:
            continue
    raise FileExistsError("could not create a unique temporary file in %s" % directory)


def _atomic_write(path, chunks):
    """Write byte chunks to a temporary file, then replace ``path``.

    ``chunks`` may be lazy; any exception while producing or writing them
    leaves an existing destination untouched and removes the temporary file.
    """
    temporary = None
    try:
        temporary, descriptor = _create_temporary(os.path.dirname(os.path.abspath(path)))
        with os.fdopen(descriptor, "wb") as output:
            for chunk in chunks:
                output.write(chunk)
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            try:
                os.unlink(temporary)
            except FileNotFoundError:
                pass


def write_animated_png(path, frames, width, height, fps=12):
    """Write a looping APNG; each frame replaces the complete canvas."""
    if not frames:
        raise ValueError("no frames to write")
    if not isinstance(fps, int) or not 1 <= fps <= 65535:
        raise ValueError("fps must be an integer between 1 and 65535")
    header = _png_header(width, height)

    def chunks():
        yield header
        yield _png_chunk(b"acTL", struct.pack(">II", len(frames), 0))
        sequence = 0
        for index, pixels in enumerate(frames):
            data = _png_pixels(pixels, width, height)
            control = struct.pack(">IIIIIHHBB", sequence, width, height, 0, 0, 1, fps, 0, 0)
            yield _png_chunk(b"fcTL", control)
            sequence += 1
            if index == 0:
                yield _png_chunk(b"IDAT", data)
            else:
                yield _png_chunk(b"fdAT", struct.pack(">I", sequence) + data)
                sequence += 1
        yield _png_chunk(b"IEND", b"")

    _atomic_write(path, chunks())


def write_png(path, pixels, width, height):
    """Write a static PNG, replacing ``path`` only after a successful write."""
    data = _png_pixels(pixels, width, height)
    _atomic_write(path, (_png_header(width, height),
                         _png_chunk(b"IDAT", data),
                         _png_chunk(b"IEND", b"")))


# ---------------------------------------------------------------- cli

# Limits the CLI enforces unless --no-limits is given. A frame is held as packed
# RGB (3 bytes per pixel), and an animation keeps every frame in memory.
MAX_FRAME_PIXELS = 100_000_000
MAX_ANIMATION_BYTES = 1 << 30


def check_limits(width, height, frames):
    """Raise ValueError if the render would be unreasonably large."""
    if width * height > MAX_FRAME_PIXELS:
        raise ValueError("%d x %d is %d pixels; the limit is %d (use --no-limits to override)"
                         % (width, height, width * height, MAX_FRAME_PIXELS))
    total = width * height * 3 * frames
    if frames > 1 and total > MAX_ANIMATION_BYTES:
        raise ValueError("%d frames of %d x %d need %d bytes in memory; the limit is %d "
                         "(use --no-limits to override)"
                         % (frames, width, height, total, MAX_ANIMATION_BYTES))


def _fps(text):
    value = int(text)
    if not 1 <= value <= 65535:
        raise argparse.ArgumentTypeError("fps must be between 1 and 65535")
    return value


def _hex_stops(text):
    try:
        return parse_hex_palette(text)
    except ValueError as error:
        raise argparse.ArgumentTypeError(str(error)) from None


def _build_parser():
    parser = argparse.ArgumentParser(description="Procedurally generated abstract art")
    parser.add_argument("--style", default="waves", choices=list(STYLES))
    seeds = parser.add_mutually_exclusive_group()
    seeds.add_argument("--seed", type=int, default=1)
    seeds.add_argument("--random-seed", action="store_true",
                       help="pick a random seed and print it so the image can be reproduced")
    palettes = parser.add_mutually_exclusive_group()
    palettes.add_argument("--palette", default="deep", choices=sorted(PALETTES))
    palettes.add_argument("--palette-stops", type=_hex_stops, metavar="HEX,HEX,...",
                          help="custom palette: two or more comma-separated hex colors from "
                               "dark to light, e.g. 0c061c,5814a8,ff8800 (quote values "
                               "starting with #)")
    parser.add_argument("--width", type=int, default=256)
    parser.add_argument("--height", type=int, default=256)
    parser.add_argument("--out", default="art.png")
    parser.add_argument("--frames", type=int, default=1,
                        help="number of frames (default 1). Styles with motion animate one "
                             "scene in a seamless loop; others use consecutive seeds")
    parser.add_argument("--phase", type=float, default=0.0,
                        help="loop position for styles with motion; 1.0 repeats 0.0 "
                             "(default 0). With --frames, the first frame's position")
    parser.add_argument("--slideshow", action="store_true",
                        help="make every frame an unrelated scene with consecutive seeds, "
                             "even for styles with motion")
    parser.add_argument("--fps", type=_fps, default=12,
                        help="animation frame rate with --frames > 1 (default 12)")
    parser.add_argument("--no-limits", action="store_true",
                        help="skip the size limits on pixels and animation memory")
    parser.add_argument("--list-styles", action="store_true",
                        help="print the style names, one per line, and exit")
    parser.add_argument("--list-palettes", action="store_true",
                        help="print the palette names, one per line, and exit")
    return parser


def _list_names(args):
    """Return the names requested by --list-styles / --list-palettes, or None."""
    if not (args.list_styles or args.list_palettes):
        return None
    return (list(STYLES) if args.list_styles else []) + (
        sorted(PALETTES) if args.list_palettes else [])


def _check_args(parser, args):
    if args.width <= 0 or args.height <= 0:
        parser.error("width and height must be positive")
    if args.frames <= 0:
        parser.error("frames must be positive")
    if not args.no_limits:
        try:
            check_limits(args.width, args.height, args.frames)
        except ValueError as error:
            parser.error(str(error))


@contextlib.contextmanager
def _temporary_palette(name, stops):
    """Register ``stops`` as ``PALETTES[name]``, then restore what was there before."""
    missing = object()
    previous = PALETTES.get(name, missing)
    PALETTES[name] = stops
    try:
        yield name
    finally:
        if previous is missing:
            del PALETTES[name]
        else:
            PALETTES[name] = previous


def main():
    parser = _build_parser()
    args = parser.parse_args()

    names = _list_names(args)
    if names is not None:
        print("\n".join(names))
        return
    _check_args(parser, args)
    seed = secrets.randbelow(2 ** 31) if args.random_seed else args.seed
    if not args.palette_stops:
        _render_and_write(args, seed, args.palette)
        return
    # render_rgb() takes palette names, so register the stops for this render only.
    with _temporary_palette("custom", args.palette_stops) as palette:
        _render_and_write(args, seed, palette)


def _frame_plan(args, seed):
    """``(seed, phase)`` for each frame: one moving scene, or a slideshow of seeds."""
    if args.style in ANIMATED_STYLES and not args.slideshow:
        return [(seed, args.phase + i / args.frames) for i in range(args.frames)]
    if args.frames > 1 and not args.slideshow:
        print("note: %s has no motion yet, so each frame uses the next seed "
              "(a slideshow)" % args.style, file=sys.stderr)
    return [(seed + i, args.phase) for i in range(args.frames)]


def _render_and_write(args, seed, palette):
    if args.frames > 1:
        frames = [render_rgb(args.width, args.height, frame_seed, args.style, palette, phase)
                  for frame_seed, phase in _frame_plan(args, seed)]
        write_animated_png(args.out, frames, args.width, args.height, fps=args.fps)
        print("wrote %d animation frames to %s (%s, seed=%d, palette=%s, %d fps)"
              % (args.frames, args.out, args.style, seed, palette, args.fps))
    else:
        pixels = render_rgb(args.width, args.height, seed, args.style, palette, args.phase)
        write_png(args.out, pixels, args.width, args.height)
        print("wrote %s (%s, seed=%d, palette=%s)" % (args.out, args.style, seed, palette))


if __name__ == "__main__":
    main()
