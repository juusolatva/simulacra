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
import math
import os
import random
import struct
import zlib

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
}


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

def render_waves(width, height, seed, palette_name):
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    layers = [
        (30.0, 2.0, rng.uniform(0, 6.28)),
        (18.0, 3.5, rng.uniform(0, 6.28)),
        (12.0, 5.0, rng.uniform(0, 6.28)),
        (7.0, 7.0, rng.uniform(0, 6.28)),
    ]
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


def _shade(width, height, stops, value):
    """Build ``height`` rows of palette colors from ``value(x, y)`` in [0, 1]."""
    return [[palette_color(stops, value(x, y)) for x in range(width)]
            for y in range(height)]


def _box_distances(px, py, reach, width, height, pad=0):
    """Yield ``(row, col, distance)`` for canvas cells in a box around (px, py).

    The box spans ``reach`` pixels each way, plus ``pad`` extra columns and
    rows on the far side, clipped to the canvas.
    """
    x0 = max(0, int(px - reach))
    x1 = min(width - 1, int(px + reach) + pad)
    y0 = max(0, int(py - reach))
    y1 = min(height - 1, int(py + reach) + pad)
    for yy in range(y0, y1 + 1):
        dy2 = (yy - py) * (yy - py)
        for xx in range(x0, x1 + 1):
            yield yy, xx, math.sqrt((xx - px) * (xx - px) + dy2)


def _orbits_color(x, y, seed, stops, center, rings, stars):
    """Noise background, overridden by any ring band and then by any star."""
    n = fbm(x * 0.008, y * 0.008, seed, octaves=3)
    c = palette_color(stops, _contrast(n, 1.6))
    r = math.sqrt((x - center[0]) ** 2 + (y - center[1]) ** 2)
    for (radius, ring_color) in rings:
        if radius * 0.9 < r < radius * 1.1:
            c = ring_color
    for (sx, sy, sr) in stars:
        if math.hypot(x - sx, y - sy) < sr * 1.2:
            c = (255, 255, 255)
    return c


def render_orbits(width, height, seed, palette_name):
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    stars = [
        (rng.uniform(0, width - 1), rng.uniform(0, height - 1), rng.uniform(0.5, 3.0))
        for _ in range(60)
    ]
    cx = width / 2.0
    cy = height / 2.0
    max_r = math.sqrt(cx * cx + cy * cy)
    rings = [(max_r * rng.uniform(0.15, 0.9), stops[rng.randrange(len(stops))])
             for _ in range(6)]
    return [[_orbits_color(x, y, seed, stops, (cx, cy), rings, stars) for x in range(width)]
            for y in range(height)]


def render_mandelbrot(width, height, seed, palette_name):
    """Mandelbrot escape times with an aspect-correct complex-plane viewport."""
    stops = PALETTES[palette_name]
    scale = 3.2 / min(width, height)
    max_iter = 100
    pixels = []
    for y in range(height):
        row = []
        cy = ((height - 1) / 2.0 - y) * scale
        for x in range(width):
            cx = (x - (width - 1) / 2.0) * scale - 0.6
            zx = zy = 0.0
            n = 0
            while zx * zx + zy * zy <= 4.0 and n < max_iter:
                zx, zy = zx * zx - zy * zy + cx, 2.0 * zx * zy + cy
                n += 1
            t = 1.0 if n == max_iter else math.sqrt(n / max_iter)
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


def render_spiral(width, height, seed, palette_name):
    """Polar spiral arms: smooth Gaussians placed where winding phases align."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    arms = []
    n_arms = rng.randint(2, 5)
    for _ in range(n_arms):
        arms.append({
            "turns": rng.uniform(1.0, 4.0),
            "phase": rng.uniform(0.0, 6.28),
            "amp": rng.uniform(0.4, 1.0),
            "width": rng.uniform(0.08, 0.35),
        })
    pixels = []
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    max_r = max(1.0, math.hypot(cx, cy))
    for y in range(height):
        row = []
        for x in range(width):
            dx = x - cx
            dy = y - cy
            r = math.sqrt(dx * dx + dy * dy) / max_r
            theta = math.atan2(dy, dx)
            v = 0.0
            for arm in arms:
                # "turns" is the number of windings between the center and corners.
                d = (r * arm["turns"] * math.tau - theta - arm["phase"]) % math.tau
                dist = abs(d - math.pi)
                v += arm["amp"] * math.exp(-((dist / arm["width"]) ** 2) / 2.0)
            v *= 1.0 - 0.5 * r
            row.append(palette_color(stops, 1.0 - math.exp(-1.6 * v)))
        pixels.append(row)
    return pixels


def render_nebula(width, height, seed, palette_name):
    """Nebula: stacked radial glow clouds over a noise starfield."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    diag = math.sqrt(width * width + height * height)
    clouds = []
    n_clouds = rng.randint(3, 6)
    for _ in range(n_clouds):
        clouds.append({
            "cx": rng.uniform(width * 0.1, width * 0.9),
            "cy": rng.uniform(height * 0.1, height * 0.9),
            "r": rng.uniform(0.08, 0.25) * diag,
            "amp": rng.uniform(0.5, 1.5),
        })
    noise_k = 24.0 / max(width, height)
    pixels = []
    stars = [
        (rng.uniform(0, width - 1), rng.uniform(0, height - 1), rng.uniform(0.5, 2.0))
        for _ in range(40)
    ]
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            for cloud in clouds:
                d2 = (x - cloud["cx"]) ** 2 + (y - cloud["cy"]) ** 2
                v += math.exp(-d2 / (2.0 * cloud["r"] ** 2)) * cloud["amp"]
            n = fbm(x * noise_k, y * noise_k, seed, octaves=4)
            v *= max(0.0, 0.2 + 1.6 * (_contrast(n, 2.0) - 0.2))
            for (sx, sy, sr) in stars:
                if math.hypot(x - sx, y - sy) < sr:
                    v += 1.2
                    break
            row.append(palette_color(stops, 1.0 - math.exp(-1.3 * v)))
        pixels.append(row)
    return pixels


def render_kaleidoscope(width, height, seed, palette_name):
    """Mirror a noise-modulated radial pattern into equal angular sectors."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    folds = rng.randint(4, 8)
    phase = rng.uniform(0.0, math.tau)
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


def render_petals(width, height, seed, palette_name):
    """Radial flower with tapered petals and a luminous center."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    count = rng.randint(5, 8)
    length = rng.uniform(0.65, 0.95)
    phase = rng.uniform(0.0, math.tau)
    cx, cy = (width - 1) / 2.0, (height - 1) / 2.0
    scale = max(1.0, min(width, height) / 2.0)
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            dx, dy = (x - cx) / scale, (y - cy) / scale
            r = math.hypot(dx, dy)
            theta = math.atan2(dy, dx)
            angular = (0.5 + 0.5 * math.cos(count * theta + phase)) ** 3
            radial = math.exp(-((r - length * 0.52) / (length * 0.3)) ** 2)
            center = math.exp(-(r / 0.16) ** 2)
            row.append(palette_color(stops, min(1.0, angular * radial + center)))
        pixels.append(row)
    return pixels


def render_horizon(width, height, seed, palette_name):
    """Layered mountain silhouettes over a vertical gradient sky."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    layers = [(fraction, rng.uniform(0, 100), 0.55 - i * 0.13)
              for i, fraction in enumerate((0.35, 0.5, 0.65, 0.8))]
    ridges = []
    for fraction, offset, color in layers:
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


def render_starburst(width, height, seed, palette_name):
    """Glowing rays radiating from a central focal point."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    rays = rng.randint(8, 14)
    phase = rng.uniform(0.0, math.tau)
    cx, cy = (width - 1) / 2.0, (height - 1) / 2.0
    scale = max(1.0, min(width, height) / 2.0)
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            dx, dy = (x - cx) / scale, (y - cy) / scale
            r = math.hypot(dx, dy)
            theta = math.atan2(dy, dx)
            ray = (0.5 + 0.5 * math.cos(theta * rays + phase)) ** 12
            glow = math.exp(-(r / 0.18) ** 2)
            t = glow + ray * math.exp(-r * 1.3) * (1 - math.exp(-r * 12))
            row.append(palette_color(stops, min(1.0, t)))
        pixels.append(row)
    return pixels


def render_interference(width, height, seed, palette_name):
    """Interference: plane waves from several directions summed into moire bands."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    size = max(width, height)
    layers = []
    for _ in range(3):
        direction = rng.uniform(0.0, math.pi)
        # Spatial frequency in cycles across the canvas, far below Nyquist.
        k = rng.uniform(4.0, 18.0) * math.tau / size
        layers.append((rng.uniform(0, 6.28), k * math.cos(direction),
                       k * math.sin(direction), rng.uniform(1.0, 4.0)))
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


def _constellation_lines(stars, width, height):
    """Rasterize lines between nearby stars into an additive glow grid."""
    max_dist = math.sqrt(width * width + height * height) * 0.32
    line_grid = [[0.0] * width for _ in range(height)]
    pairs = []
    for i in range(len(stars)):
        for j in range(i + 1, len(stars)):
            dist = math.hypot(stars[j]["x"] - stars[i]["x"], stars[j]["y"] - stars[i]["y"])
            if dist <= max_dist:
                pairs.append((i, j, dist))
    for (i, j, dist) in pairs:
        _draw_constellation_line(line_grid, stars[i], stars[j], dist, width, height)
    return line_grid


def _draw_constellation_line(line_grid, start, end, dist, width, height):
    """Accumulate one line, brightest at ``start`` and fading toward ``end``."""
    sx, sy = start["x"], start["y"]
    ex, ey = end["x"], end["y"]
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
    for star in stars:
        d = math.hypot(x - star["x"], y - star["y"])
        v += math.exp(-((d / star["r"]) ** 2) / 2.0) * star["b"]
    v += line_grid[y][x]
    return min(1.0, max(0.0, 1.0 - math.exp(-v)))


def render_constellations(width, height, seed, palette_name):
    """Constellations: starfield whose stars are loosely connected by lines."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    n_stars = rng.randint(28, 48)
    stars = []
    for _ in range(n_stars):
        stars.append({
            "x": rng.uniform(0, width - 1),
            "y": rng.uniform(0, height - 1),
            "r": rng.uniform(0.7, 2.2),
            "b": rng.uniform(0.45, 1.0),
        })
    line_grid = _constellation_lines(stars, width, height)
    return _shade(width, height, stops,
                  lambda x, y: _constellation_value(x, y, stars, line_grid))


def render_cosmos(width, height, seed, palette_name):
    """Cosmos: concentric ripples emanating from random luminous centers."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    n_centers = rng.randint(2, 5)
    centers = []
    for _ in range(n_centers):
        centers.append({
            "x": rng.uniform(width * 0.2, width * 0.8),
            "y": rng.uniform(height * 0.2, height * 0.8),
            "max_r": rng.uniform(0.25, 0.6) * math.sqrt(width * width + height * height),
            "rings": rng.randint(3, 8),
            "phase": rng.uniform(0.0, 6.28),
            "amp": rng.uniform(0.6, 1.4),
        })

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            weight = 0.0
            for c in centers:
                d = math.sqrt((x - c["x"]) ** 2 + (y - c["y"]) ** 2)
                if d < c["max_r"]:
                    k = int((d / c["max_r"]) * (c["rings"] - 1) + 0.5)
                    w = math.exp(-((d / (c["max_r"] / max(c["rings"] - 1, 1))) ** 2) / 2.0)
                    s = math.sin(k * 2.0 + c["phase"])
                    val = w * c["amp"] * (0.5 + 0.5 * s)
                    v += val
                    weight += w
            t = min(1.0, max(0.0, v / (weight + 0.6)))
            row.append(palette_color(stops, t * 0.95))
        pixels.append(row)
    return pixels


def render_galaxy(width, height, seed, palette_name):
    """Galaxy: spiral disk of stars with a bright central bulge and curved arms."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    max_r = max(1.0, math.hypot(cx, cy))
    bulge_r = max_r * rng.uniform(0.10, 0.26)
    bulge_amp = rng.uniform(0.9, 1.5)
    n_arms = rng.randint(2, 3)
    arms = []
    for _ in range(n_arms):
        arms.append({
            "r0": max_r * rng.uniform(0.015, 0.05),
            "width": rng.uniform(0.08, 0.18),
            "phase": rng.uniform(0.0, 6.28),
            "pitch": rng.uniform(1.5, 3.5),
            "amp": rng.uniform(0.5, 1.0),
        })
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
            for arm in arms:
                t = math.log1p(r / arm["r0"])
                arm_theta = arm["phase"] + t * arm["pitch"]
                d = abs((arm_theta - theta + math.pi) % math.tau - math.pi)
                w = math.exp(-0.5 * (d / arm["width"]) ** 2 - r / max_r)
                v += w * arm["amp"]
            t_val = 1.0 - math.exp(-v)
            row.append(palette_color(stops, t_val * 0.95))
        pixels.append(row)
    return pixels


def render_bloom(width, height, seed, palette_name):
    """Radial blooms: soft glowing clouds modulated by noise."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    scale = max(1.0, min(width, height))
    blooms = []
    for _ in range(rng.randint(4, 9)):
        blooms.append({
            "x": rng.uniform(0, width - 1),
            "y": rng.uniform(0, height - 1),
            "r": scale * rng.uniform(0.08, 0.27),
            "amp": rng.uniform(0.4, 1.0),
        })
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            n = fbm(x * 0.006, y * 0.006, seed, octaves=3)
            for b in blooms:
                d = math.hypot(x - b["x"], y - b["y"])
                v += math.exp(-((d / b["r"]) ** 2) / 2.0) * b["amp"]
            v *= 0.4 + 1.2 * n
            row.append(palette_color(stops, 1.0 - math.exp(-1.4 * v)))
        pixels.append(row)
    return pixels


def render_flowfield(width, height, seed, palette_name):
    """Particle trails following a smooth, deterministic noise vector field."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    glow = [0.0] * (width * height)
    scale = max(1.0, min(width, height))
    step = max(0.35, scale / 200.0)
    for _ in range(rng.randint(40, 80)):
        x, y = rng.uniform(0, width - 1), rng.uniform(0, height - 1)
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


def _torus_knot_points(tube, a, b, samples):
    pts = []
    for i in range(samples):
        t = 2.0 * math.pi * i / samples
        ct = math.cos(b * t)
        pts.append(((2.0 + tube * ct) * math.cos(a * t),
                    (2.0 + tube * ct) * math.sin(a * t),
                    tube * math.sin(b * t)))
    return pts


def render_torus(width, height, seed, palette_name):
    """Torus knot: glowing parametric knot curve with depth shading."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    tube = rng.uniform(0.6, 1.2)
    a, b = rng.choice(((2, 3), (3, 2), (3, 5), (5, 2)))
    pts = _torus_knot_points(tube, a, b, max(2200, 4 * (width + height)))
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    zs = [p[2] for p in pts]
    cx = (min(xs) + max(xs)) / 2.0
    cy = (min(ys) + max(ys)) / 2.0
    z_lo, z_hi = min(zs), max(zs)
    span = max(max(xs) - min(xs), max(ys) - min(ys), 1e-9)
    s = min(width, height) * 0.85 / span

    # Each sample stamps a glow scaled by its depth (larger z = nearer); keeping
    # the maximum lets the front strand win where the knot crosses itself.
    glow = [[0.0] * width for _ in range(height)]
    r_glow = max(2.0, min(width, height) * 0.016)
    for (x, y, z) in pts:
        px = (x - cx) * s + (width - 1) / 2.0
        py = (cy - y) * s + (height - 1) / 2.0
        depth = 0.45 + 0.55 * (z - z_lo) / max(1e-9, z_hi - z_lo)
        for yy, xx, d in _box_distances(px, py, r_glow, width, height, pad=1):
            if d < r_glow:
                glow[yy][xx] = max(glow[yy][xx], (1.0 - d / r_glow) * depth)

    def value(x, y):
        background = _contrast(fbm(x * 0.012, y * 0.012, seed, octaves=3), 1.6) * 0.4
        return max(background, glow[y][x])

    return _shade(width, height, stops, value)


def render_voronoi(width, height, seed, palette_name):
    """Voronoi cells shaded by nearest-site distance with glowing boundaries."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    sites = [(rng.uniform(0, width - 1), rng.uniform(0, height - 1), rng.uniform(0.1, 0.7))
             for _ in range(rng.randint(10, 16))]
    scale = max(1.0, min(width, height))
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            distances = sorted((math.hypot(x - sx, y - sy), color)
                               for sx, sy, color in sites)
            nearest, color = distances[0]
            gap = distances[1][0] - nearest
            edge = math.exp(-gap / max(0.5, scale * 0.012))
            t = color * math.exp(-nearest / (scale * 0.3)) + edge * 0.5
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


def render_pinwheel(width, height, seed, palette_name):
    """Pinwheel: swirling radial arms with noise-modulated twist."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    n_arms = rng.randint(3, 5)
    twist = rng.uniform(2.0, 7.0)
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


def _curve_distance_grid(pts, reach, width, height):
    """Distance from each pixel near the curve to its closest sample (1e9 elsewhere)."""
    min_d = [[1e9] * width for _ in range(height)]
    for (px, py) in pts:
        for yy, xx, d in _box_distances(px, py, reach, width, height):
            if d < min_d[yy][xx]:
                min_d[yy][xx] = d
    return min_d


def _lissajous_value(x, y, glow_values, n_curves):
    v = 0.0
    for (min_d, c) in glow_values:
        d = min_d[y][x]
        if d < 1e8:
            v += math.exp(-((d / c["sigma"]) ** 2) / 2.0) * c["amp"]
    return min(1.0, max(0.0, v / (n_curves * 0.9)))


def render_lissajous(width, height, seed, palette_name):
    """Lissajous: glowing parametric harmonic curves traced through the canvas."""
    stops = PALETTES[palette_name]
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
    samples = 1400
    glow_values = []
    for c in curves:
        pts = [(cx + cx * c["ax"] * math.sin(c["fx"] * t + c["px"]),
                cy - cy * c["ay"] * math.sin(c["fy"] * t + c["py"]))
               for t in (2.0 * math.pi * i / samples for i in range(samples))]
        reach = max(2, math.ceil(c["sigma"] * 3))
        glow_values.append((_curve_distance_grid(pts, reach, width, height), c))
    return _shade(width, height, stops,
                  lambda x, y: _lissajous_value(x, y, glow_values, n_curves))


def render_geo(width, height, seed, palette_name):
    """Layered rotated regular polygons with glowing edges."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    cx, cy = (width - 1) / 2.0, (height - 1) / 2.0
    scale = max(1.0, min(width, height) / 2.0)
    polygons = []
    for _ in range(rng.randint(4, 6)):
        sides = rng.randint(3, 8)
        radius = scale * rng.uniform(0.45, 0.95)
        phase = rng.uniform(0.0, math.tau)
        polygons.append([(cx + radius * math.cos(i * math.tau / sides + phase),
                          cy + radius * math.sin(i * math.tau / sides + phase))
                         for i in range(sides)])
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = sum(math.exp(-0.5 * (_poly_min_dist(x, y, poly) /
                                    max(0.6, scale * 0.025)) ** 2)
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


def render_prism(width, height, seed, palette_name):
    """Prism: stacked faceted gem facets with ridge shading and soft glow."""
    stops = PALETTES[palette_name]
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
    scenes = [(g, _prism_layers(rng, g, small)) for g in gems]
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


def render_shard(width, height, seed, palette_name):
    """Shard: fractured irregular gem shards with glow halos and crack lines."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    shards = [_make_shard(rng, width, height) for _ in range(rng.randint(2, 4))]
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


def _circuit_value(x, y, traces, core, halo, pad):
    v = 0.0
    for (nodes, segments) in traces:
        d = min(_seg_hit(x, y, p1, p2) for p1, p2 in segments)
        v += math.exp(-((d / core) ** 2) / 2.0) + 0.3 * math.exp(-((d / halo) ** 2) / 2.0)
        for (px, py) in nodes:
            dn = math.hypot(x - px, y - py)
            if dn < pad * 2.5:
                v += 1.5 * math.exp(-((dn / pad) ** 2) / 2.0)
    return 1.0 - math.exp(-1.2 * v)


def render_circuit(width, height, seed, palette_name):
    """Circuit: glowing right-angled traces routed between node pads."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    small = max(1.0, min(width, height))
    core = max(0.7, small * 0.008)
    halo = max(2.0, small * 0.04)
    pad = max(1.5, small * 0.018)
    traces = [_make_trace(rng, width, height) for _ in range(rng.randint(3, 6))]
    return _shade(width, height, stops,
                  lambda x, y: _circuit_value(x, y, traces, core, halo, pad))


def render_cosmic(width, height, seed, palette_name):
    """Cosmic: layered starfield with nebula glow and a bright arm."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    stars = []
    for _ in range(rng.randint(40, 90)):
        stars.append((rng.uniform(0, width), rng.uniform(0, height),
                      rng.uniform(0.2, 1.2), rng.uniform(0.4, 1.0)))
    small = max(1.0, min(width, height))
    arm_r = rng.uniform(small * 0.5, small * 0.8)
    arm_ang = rng.uniform(0, 2 * math.pi)
    arm_w = rng.uniform(0.4, 1.0)
    ox, oy = width / 2, height / 2
    glow_sigma = small * 0.55
    arm_sigma = max(1.0, small * 0.08)
    star_sigma = max(1.0, small * 0.02)

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
            for (sx, sy, size, brightness) in stars:
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


def render_quasar(width, height, seed, palette_name):
    """Quasar: glowing relativistic accretion disk with Doppler-beamed jets and lensing glow."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    beta = rng.uniform(0.55, 0.9)
    spin = rng.uniform(0.0, 2 * math.pi)
    r_shadow = max(1.5, min(width, height) * rng.uniform(0.05, 0.11))
    r_outer = min(width, height) * rng.uniform(1.4, 3.2)
    jet_axis = (math.cos(spin), math.sin(spin))
    jet_half_angle = rng.uniform(0.15, 0.3)
    jet_len = rng.uniform(min(width, height) * 1.0, min(width, height) * 2.2)
    orbits = sorted(rng.uniform(r_shadow * 1.25, r_outer) for _ in range(rng.randint(5, 8)))

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            dx = x - cx
            dy = y - cy
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
            for ro in orbits:
                w = max(0.4, (ro - r_shadow * 1.2) * 0.06)
                # turbulence makes the glow rings irregular and organic
                wob = 0.35 + 0.65 * polar_noise(r * 0.5, theta, seed + int(ro * 10.0),
                                                3.0, octaves=3)
                ro_t = ro + w * (wob - 0.5)
                g = math.exp(-((r - ro_t) ** 2) / (2.0 * w * w)) * doppler * disk_level
                v += g * (1.5 - 0.6 * disk_t)
            # relativistic jets along the spin axis, with cone and Doppler beaming
            ux, uy = jet_axis
            t = dx * ux + dy * uy
            perp = -dx * uy + dy * ux
            v += _quasar_jet_glow(t, perp, jet_len, jet_half_angle, r_shadow, beta)
            # faint starfield backdrop
            n = fbm(x * 0.004, y * 0.004, seed, octaves=2)
            v += max(0.0, n - 0.5) * 0.6
            row.append(palette_color(stops, 1.0 - math.exp(-0.75 * v)))
        pixels.append(row)
    return pixels


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
        wobbled = rib["radius"] * (1.0 + rib["wob"] * (noise - 0.5))
        v += math.exp(-((r - wobbled) ** 2) / (2.0 * rib["size"] ** 2)) * 1.6
    for (r0, r1, phase) in scene["spokes"]:
        dang = math.atan2(math.sin(theta - phase), math.cos(theta - phase))
        if r0 <= r <= r1 and abs(dang) < 0.06:
            v += 0.9
    for (gx, gy, size) in scene["glyphs"]:
        d = math.hypot(x - gx, y - gy)
        if d < size * 0.9:
            v += math.exp(-(d * d) / (2.0 * size * size)) * 2.2
    return 1.0 - math.exp(-0.7 * v)


def render_mandala(width, height, seed, palette_name):
    """Concentric mandala: noise-wobbled rings of glowing glyphs with radial spokes and a center medallion."""
    stops = PALETTES[palette_name]
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
    scene = {"center": (cx, cy), "medallion": max(3.0, max_r * 0.06), "seed": seed,
             "rings": rings, "spokes": spokes, "glyphs": glyphs}
    return _shade(width, height, stops, lambda x, y: _mandala_value(x, y, scene))


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


def render_crystal(width, height, seed, palette_name):
    """Crystal: noise-grown terrain quantized into low-poly faceted gem faces.

    A height field is grown from fBm noise and its gradient is measured with
    finite differences. Each surface normal is snapped to one of eight
    compass directions and a few discrete tilts, so neighbouring pixels share
    flat facets that are lit with diffuse and Blinn-Phong specular terms.
    """
    stops = PALETTES[palette_name]
    light = (0.45, -0.55, 0.70)
    ln = math.sqrt(sum(c * c for c in light))
    light = (light[0] / ln, light[1] / ln, light[2] / ln)
    hx, hy, hz = light[0], light[1], light[2] + 1.0  # half vector, viewer at +z
    hn = math.sqrt(hx * hx + hy * hy + hz * hz)
    half = (hx / hn, hy / hn, hz / hn)
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


def render_ember(width, height, seed, palette_name):
    """Ember: rising glowing embers with flicker and soft radial falloff."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    n_embers = rng.randint(60, 110)
    embers = []
    for _ in range(n_embers):
        embers.append({
            "x": rng.uniform(0.0, width - 1.0),
            "y": rng.uniform(0.0, height - 1.0),
            "rise": rng.uniform(0.15, 0.55) * height,
            "size": rng.uniform(0.6, 2.4),
            "phase": rng.uniform(0.0, 6.28),
            "flicker": rng.uniform(0.5, 1.0),
        })
    glow_sum = [0.0] * (width * height)
    for e in embers:
        flicker = e["flicker"] * (0.55 + 0.45 * math.sin(e["phase"] + 2.0 * math.sin(e["phase"])))
        cx = e["x"]
        cy_top = e["y"] - e["rise"]
        for yy in range(height):
            dy = yy - cy_top
            if abs(dy) > e["rise"] * 3.0:
                continue
            fall = max(0.0, 1.0 - abs(dy) / (e["rise"] * 2.5 + 0.01))
            if fall < 0.02:
                continue
            glow = fall ** 2 * (0.35 + 0.65 * flicker)
            for xx in range(width):
                dx = xx - cx
                g = glow * max(0.0, 1.0 - (dx / ((e["size"] * 2.0) + 0.01)) ** 2) ** 2
                glow_sum[yy * width + xx] += g
    return _glow_pixels(glow_sum, width, height, stops)


def _stamp_comet(glow_sum, c, width, height):
    """Paint one comet's fading streak; the canvas wraps at its edges."""
    steps = max(1, math.ceil(c["speed"] * 2))
    radius = c["width_px"]
    reach = math.ceil(radius * 2)
    for s in range(steps):
        t = (s + 0.5) / steps
        px = c["x"] + math.cos(c["angle"]) * c["speed"] * t
        py = c["y"] + math.sin(c["angle"]) * c["speed"] * t
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


def render_comet(width, height, seed, palette_name):
    """Comet: fast glowing streaks with fading motion trails."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    comets = []
    for _ in range(rng.randint(3, 6)):
        comets.append({
            "x": rng.uniform(0.0, width - 1.0),
            "y": rng.uniform(0.0, height - 1.0),
            "angle": rng.uniform(0.0, 2.0 * math.pi),
            "speed": rng.uniform(0.2, 0.6) * width,
            "width_px": rng.uniform(0.8, 3.0),
        })
    glow_sum = [0.0] * (width * height)
    for c in comets:
        _stamp_comet(glow_sum, c, width, height)
    return _glow_pixels(glow_sum, width, height, stops)


def render_ink(width, height, seed, palette_name):
    """Quadratic Bezier ink strokes rasterized with a tapered soft brush."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    glow = [0.0] * (width * height)
    for _ in range(rng.randint(5, 9)):
        x0, y0 = rng.uniform(0, width - 1), rng.uniform(0, height - 1)
        x1, y1 = rng.uniform(0, width - 1), rng.uniform(0, height - 1)
        cx, cy = rng.uniform(0, width - 1), rng.uniform(0, height - 1)
        brush = max(0.8, min(width, height) * rng.uniform(0.008, 0.025))
        alpha = rng.uniform(0.35, 0.9)
        length = math.hypot(cx - x0, cy - y0) + math.hypot(x1 - cx, y1 - cy)
        steps = max(8, math.ceil(length * 2))
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


def render_gemstones(width, height, seed, palette_name):
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    small = max(1.0, min(width, height))
    gems = []
    for _ in range(3):
        cx = rng.uniform(0, width - 1)
        cy = rng.uniform(0, height - 1)
        cr = small * rng.uniform(0.1, 0.25)
        gems.append((cx, cy, cr, rng.uniform(0, 6.28)))
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
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
            row.append(c)
        pixels.append(row)
    return pixels


def render_rings(width, height, seed, palette_name):
    """Concentric rings, each shaded by a smooth sweep of hue around its circle."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    scale = max(1.0, min(width, height) / 2.0)
    rings_cfg = []
    for _ in range(rng.randint(4, 9)):
        rings_cfg.append((scale * rng.uniform(0.15, 0.95), rng.uniform(0.0, math.tau)))
    pixels = []
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    for y in range(height):
        row = []
        for x in range(width):
            c = stops[0]
            r = math.sqrt((x - cx) ** 2 + (y - cy) ** 2)
            for (radius, phase) in rings_cfg:
                if radius * 0.9 < r < radius * 1.1:
                    angle = math.atan2(y - cy, x - cx)
                    c = palette_color(stops, 0.3 + 0.35 * (1.0 + math.cos(angle + phase)))
            row.append(c)
        pixels.append(row)
    return pixels


def render_aurora(width, height, seed, palette_name):
    """Flowing ribbons: wavy curves sampled across the canvas, blurred by gaussian weighting."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    ribbons = []
    for _ in range(rng.randint(3, 7)):
        ribbons.append({
            "y0": rng.uniform(0, height - 1),
            "amp": max(1.0, height * rng.uniform(0.06, 0.27)),
            "freq": rng.uniform(1.0, 5.0) / max(1, width),
            "phase": rng.uniform(0.0, 6.28),
            "bw": rng.uniform(0.6, 1.4),
        })
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            for rib in ribbons:
                yy = rib["y0"] + rib["amp"] * math.sin(x * rib["freq"] + rib["phase"])
                d = (y - yy) / rib["amp"]
                w = math.exp(-d * d)
                s = math.sin(x * rib["freq"] * 2.0 + rib["phase"])
                v += w * (0.5 + 0.5 * s) * rib["bw"]
            row.append(palette_color(stops, 0.05 + 0.95 * (1.0 - math.exp(-1.5 * v))))
        pixels.append(row)
    return pixels


def render_plasma(width, height, seed, palette_name):
    """Plasma effect: sine plane waves from several directions warped by noise."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    size = max(width, height)
    layers = []
    # (amplitude, cycles across the canvas) - kept well below the Nyquist limit
    for amp, cycles in ((1.0, 1.5), (0.8, 2.5), (0.6, 4.0), (0.45, 6.5)):
        direction = rng.uniform(0.0, math.tau)
        k = cycles * math.tau / size
        layers.append((amp, k * math.cos(direction), k * math.sin(direction),
                       rng.uniform(0.0, 6.28)))
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


def render(width, height, seed, style, palette_name):
    """Render ``style`` and return ``height`` rows of ``width`` RGB tuples."""
    _validate_dimensions(width, height)
    if palette_name not in PALETTES:
        raise ValueError("unknown palette: %s" % palette_name)
    if style not in STYLES:
        raise ValueError("unknown style: %s" % style)
    return STYLES[style](width, height, seed, palette_name)


# ---------------------------------------------------------------- png writer

def _validate_dimensions(width, height):
    if (not isinstance(width, int) or not isinstance(height, int)
            or width <= 0 or height <= 0):
        raise ValueError("width and height must be positive integers")


def _png_chunk(tag, data):
    return (struct.pack(">I", len(data)) + tag + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))


def _png_pixels(pixels, width, height):
    """Encode 8-bit sRGB samples as compressed PNG scanlines.

    Palette stops are authored as sRGB colors, so samples are written as-is.
    """
    _validate_dimensions(width, height)
    if len(pixels) != height or any(len(row) != width for row in pixels):
        raise ValueError("pixel array must match width and height")

    raw = bytearray()
    for row in pixels:
        raw.append(0)  # PNG filter: None
        for r, g, b in row:
            raw.extend((min(255, max(0, int(r))),
                        min(255, max(0, int(g))),
                        min(255, max(0, int(b)))))
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

def main():
    parser = argparse.ArgumentParser(description="Procedurally generated abstract art")
    parser.add_argument("--style", default="waves", choices=list(STYLES))
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--palette", default="deep", choices=sorted(PALETTES))
    parser.add_argument("--width", type=int, default=256)
    parser.add_argument("--height", type=int, default=256)
    parser.add_argument("--out", default="art.png")
    parser.add_argument("--frames", type=int, default=1,
                        help="number of frames using consecutive seeds (default 1)")
    args = parser.parse_args()

    if args.width <= 0 or args.height <= 0:
        parser.error("width and height must be positive")
    if args.frames <= 0:
        parser.error("frames must be positive")
    if args.frames > 1:
        frames = [
            render(args.width, args.height, args.seed + i, args.style, args.palette)
            for i in range(args.frames)
        ]
        write_animated_png(args.out, frames, args.width, args.height, fps=12)
        print("wrote %d animation frames to %s (%s, seed=%d, palette=%s)" % (args.frames, args.out, args.style, args.seed, args.palette))
    else:
        pixels = render(args.width, args.height, args.seed, args.style, args.palette)
        write_png(args.out, pixels, args.width, args.height)
        print("wrote %s (%s, seed=%d, palette=%s)" % (args.out, args.style, args.seed, args.palette))


if __name__ == "__main__":
    main()
