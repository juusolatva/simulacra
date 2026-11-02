"""Procedurally generated abstract art.

Pure Python (stdlib only) generative art engine.

Styles:
    waves    - layered sine waves over a palette gradient
    blobs    - fractal (fBm) noise fields mapped through palettes
    orbits   - starfield with noise-driven orbital rings
    mandelbrot - classic Mandelbrot set framed by a palette gradient
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
    crystal    - noise-grown terrain quantized into faceted gem crystal faces
    ember      - rising glowing embers with flicker and soft falloff
    comet      - fast glowing streaks with fading motion trails
    ink        - flowing liquid ink strokes with tapered tails and soft edges

Usage:
    python abstract_art.py --style waves --seed 7 --palette sunset --out art.png
"""

import argparse
import math
import os
import random
import struct
import tempfile
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
            base = n * 0.5 + 0.5
            v = base * 0.5
            for (amp, freq, phase) in layers:
                angle = (x + y * 0.9) * freq * math.tau / max(width, height) + phase
                v += amp / 67.0 * math.sin(angle) * (0.5 + 0.5 * n)
            t = min(1.0, max(0.0, v * 1.4))
            c = palette_color(stops, t)
            row.append(c)
        pixels.append(row)
    return pixels


def render_blobs(width, height, seed, palette_name):
    stops = PALETTES[palette_name]
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            n = fbm(x * 0.02, y * 0.02, seed, octaves=5)
            t = n * 0.5 + 0.5
            c = palette_color(stops, t)
            row.append(c)
        pixels.append(row)
    return pixels


def render_orbits(width, height, seed, palette_name):
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    stars = [
        (rng.uniform(0, width - 1), rng.uniform(0, height - 1), rng.uniform(0.5, 3.0))
        for _ in range(60)
    ]
    pixels = []
    cx = width / 2.0
    cy = height / 2.0
    max_r = math.sqrt(cx * cx + cy * cy)
    rings = [
        (max_r * rng.uniform(0.15, 0.9), rng.uniform(0, len(stops) - 1), rng.uniform(0, 6.28))
        for _ in range(6)
    ]
    for y in range(height):
        row = []
        for x in range(width):
            n = fbm(x * 0.008, y * 0.008, seed, octaves=3)
            t = n * 0.5 + 0.5
            c = palette_color(stops, t)
            r = math.sqrt((x - cx) ** 2 + (y - cy) ** 2)
            for (radius, idx, phase) in rings:
                if r > radius * 0.9 and r < radius * 1.1 and int(idx) >= 0 and int(idx) < len(stops):
                    c = stops[int(idx)]
            for (sx, sy, sr) in stars:
                d = math.hypot(x - sx, y - sy)
                if d < sr * 1.2:
                    c = (255, 255, 255)
            row.append(c)
        pixels.append(row)
    return pixels


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
            "radius": rng.uniform(0.5, 3.0),
            "phase": rng.uniform(0.0, 6.28),
            "amp": rng.uniform(0.4, 1.0),
            "width": rng.uniform(0.08, 0.35),
        })
    pixels = []
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    for y in range(height):
        row = []
        for x in range(width):
            dx = x - cx
            dy = y - cy
            r = math.sqrt(dx * dx + dy * dy)
            theta = math.atan2(dy, dx)
            v = 0.0
            for arm in arms:
                d = (r * arm["turns"] - theta - arm["phase"]) % (2.0 * math.pi)
                dist = abs(d - math.pi)
                if dist > math.pi:
                    dist = 2.0 * math.pi - dist
                v += arm["amp"] * math.exp(-((dist / arm["width"]) ** 2) / 2.0) / (arm["width"] * math.sqrt(2.0 * math.pi))
            t = min(1.0, max(0.0, v * 0.9 / (n_arms * 0.8)))
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


def render_nebula(width, height, seed, palette_name):
    """Nebula: stacked radial glow clouds over a noise starfield."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    clouds = []
    n_clouds = rng.randint(3, 6)
    for _ in range(n_clouds):
        clouds.append({
            "cx": rng.uniform(width * 0.1, width * 0.9),
            "cy": rng.uniform(height * 0.1, height * 0.9),
            "r": rng.uniform(0.15, 0.45) * math.sqrt(width * width + height * height),
            "amp": rng.uniform(0.5, 1.5),
        })
    pixels = []
    stars = [
        (rng.uniform(0, width - 1), rng.uniform(0, height - 1), rng.uniform(0.5, 2.0))
        for _ in range(40)
    ]
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            weight_total = 0.0
            for cloud in clouds:
                d = math.sqrt((x - cloud["cx"]) ** 2 + (y - cloud["cy"]) ** 2)
                w = math.exp(-((d / cloud["r"]) ** 2) / 2.0)
                v += w * cloud["amp"]
                weight_total += w
            n = fbm(x * 0.004, y * 0.004, seed, octaves=3)
            star = 0.0
            for (sx, sy, sr) in stars:
                if math.hypot(x - sx, y - sy) < sr:
                    star = 1.0
            v += (n * 0.5 + 0.5) * 0.35 + star * 0.4
            t = min(1.0, max(0.0, v / (weight_total + 1.2)))
            row.append(palette_color(stops, t))
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
    """Interference: layered sine waves creating interference patterns."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    layers = [
        (rng.uniform(0, 6.28), rng.uniform(0.5, 2.5), rng.uniform(0, 4.0), rng.uniform(0, 20.0)),
        (rng.uniform(0, 6.28), rng.uniform(0.5, 2.5), rng.uniform(0, 4.0), rng.uniform(0, 14.0)),
        (rng.uniform(0, 6.28), rng.uniform(0.5, 2.5), rng.uniform(0, 4.0), rng.uniform(0, 9.0)),
    ]
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            for (phase, freq, amp, amp2) in layers:
                v += math.sin(x * freq + y * freq * 0.8 + phase) * amp
            n = fbm(x * 0.006, y * 0.006, seed, octaves=2)
            v += (n * 0.5 + 0.5) * 2.0
            t = min(1.0, max(0.0, math.sin(v * 0.35) * 0.5 + 0.5 + (n * 0.5 + 0.5 - 0.5) * 0.3))
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


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

    max_dist = math.sqrt(width * width + height * height) * 0.32
    line_grid = [[0.0] * width for _ in range(height)]

    pairs = []
    for i in range(len(stars)):
        for j in range(i + 1, len(stars)):
            sx, sy = stars[i]["x"], stars[i]["y"]
            ex, ey = stars[j]["x"], stars[j]["y"]
            dist = math.hypot(ex - sx, ey - sy)
            if dist <= max_dist:
                pairs.append((i, j, dist))

    for (i, j, dist) in pairs:
        sx, sy = stars[i]["x"], stars[i]["y"]
        ex, ey = stars[j]["x"], stars[j]["y"]
        steps = max(1, int(dist / 1.2))
        for k in range(steps + 1):
            t = k / steps
            lx = sx + (ex - sx) * t
            ly = sy + (ey - sy) * t
            ix = int(lx)
            iy = int(ly)
            if 0 <= ix < width and 0 <= iy < height:
                falloff = math.exp(-((k / steps) * (k / steps)) / 2.0)
                line_grid[iy][ix] += falloff * 0.9

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            for star in stars:
                d = math.hypot(x - star["x"], y - star["y"])
                v += math.exp(-((d / star["r"]) ** 2) / 2.0) * star["b"]
            v += line_grid[y][x]
            t = min(1.0, max(0.0, 1.0 - math.exp(-v)))
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


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
    """Radial blooms: soft glowing clouds spawned at noise-driven points."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    blooms = []
    for _ in range(rng.randint(4, 9)):
        blooms.append({
            "x": rng.uniform(0, width - 1),
            "y": rng.uniform(0, height - 1),
            "r": rng.uniform(20.0, 70.0),
            "amp": rng.uniform(0.4, 1.0),
        })
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            n = fbm(x * 0.006, y * 0.006, seed, octaves=3)
            base = 0.5 + 0.5 * n
            for b in blooms:
                d = math.hypot(x - b["x"], y - b["y"])
                w = math.exp(-((d / b["r"]) ** 2) / 2.0) * b["amp"]
                w *= (0.5 + 0.5 * n) * (0.6 + 0.4 * base)
                v += w
            t = min(1.0, max(0.0, v / (len(blooms) * 1.4)))
            row.append(palette_color(stops, t))
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


def render_torus(width, height, seed, palette_name):
    """Torus knot: glowing parametric knot curve sampled through a palette gradient."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    R = rng.uniform(0.6, 1.2)
    a, b = rng.choice(((2, 3), (3, 2), (3, 5), (5, 2)))
    samples = 2200
    pts = []
    for i in range(samples):
        t = 2.0 * math.pi * i / samples
        ct = math.cos(b * t)
        pts.append(((2.0 + R * ct) * math.cos(a * t),
                    (2.0 + R * ct) * math.sin(a * t),
                    R * math.sin(b * t)))
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    zs = [p[2] for p in pts]
    cx = (min(xs) + max(xs)) / 2.0
    cy = (min(ys) + max(ys)) / 2.0
    cz = (min(zs) + max(zs)) / 2.0
    span = max(max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    s = min(width, height) / (2.0 * span + 1.0) * 0.85
    screen = []
    for (x, y, z) in pts:
        px = (x - cx) * s + width / 2.0
        py = (cy - y) * s + height / 2.0
        pz = (z - cz) * s
        screen.append((px, py, pz))

    # rasterize: for each knot sample, flood its neighborhood and keep the
    # minimum distance from every pixel to the curve
    min_d = [[1e9] * width for _ in range(height)]
    R_GLOW = 4.0
    for (px, py, pz) in screen:
        x0 = int(max(0.0, px - R_GLOW))
        x1 = int(min(width - 1, px + R_GLOW))
        y0 = int(max(0.0, py - R_GLOW))
        y1 = int(min(height - 1, py + R_GLOW))
        for yy in range(y0, y1 + 1):
            drow = min_d[yy]
            dy = yy - py
            dy2 = dy * dy
            for xx in range(x0, x1 + 1):
                d = math.sqrt((xx - px) * (xx - px) + dy2)
                if d < drow[xx]:
                    drow[xx] = d

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            d = min_d[y][x]
            if d <= R_GLOW:
                tfrac = min(1.0, max(0.0, (R_GLOW - d) / R_GLOW))
                c = palette_color(stops, tfrac * 0.85 + 0.05)
            else:
                c = palette_color(stops, fbm(x * 0.012, y * 0.012, seed, octaves=3) * 0.5 + 0.5)
            row.append(c)
        pixels.append(row)
    return pixels


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
            v = w * (1.0 - 0.35 * t) + (n * 0.5 + 0.5) * 0.15
            t_val = min(1.0, max(0.0, v / 1.2))
            row.append(palette_color(stops, t_val))
        pixels.append(row)
    return pixels


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
    traced = []
    for c in curves:
        pts = []
        for i in range(samples):
            t = 2.0 * math.pi * i / samples
            pts.append((cx + cx * c["ax"] * math.sin(c["fx"] * t + c["px"]),
                        cy - cy * c["ay"] * math.sin(c["fy"] * t + c["py"])))
        traced.append((pts, c))

    glow_values = []
    for (pts, c) in traced:
        min_d = [[1e9] * width for _ in range(height)]
        R_GLOW = max(2, math.ceil(c["sigma"] * 3))
        for (px, py) in pts:
            x0 = max(0, int(px - R_GLOW))
            x1 = min(width - 1, int(px + R_GLOW))
            y0 = max(0, int(py - R_GLOW))
            y1 = min(height - 1, int(py + R_GLOW))
            for yy in range(y0, y1 + 1):
                drow = min_d[yy]
                dy = yy - py
                dy2 = dy * dy
                for xx in range(x0, x1 + 1):
                    d = math.sqrt((xx - px) * (xx - px) + dy2)
                    if d < drow[xx]:
                        drow[xx] = d
        glow_values.append((min_d, c))

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            for (min_d, c) in glow_values:
                d = min_d[y][x]
                if d < 1e8:
                    v += math.exp(-((d / c["sigma"]) ** 2) / 2.0) * c["amp"]
            t = min(1.0, max(0.0, v / (n_curves * 0.9)))
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


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


def render_prism(width, height, seed, palette_name):
    """Prism: stacked faceted gem facets with ridge shading and soft glow."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    n_gems = rng.randint(2, 5)
    gems = []
    for _ in range(n_gems):
        gems.append({
            "cx": rng.uniform(width * 0.15, width * 0.85),
            "cy": rng.uniform(height * 0.15, height * 0.85),
            "r": rng.uniform(20.0, min(width, height) * 0.28),
            "glow": rng.uniform(0.5, 1.0),
        })

    scenes = []
    for g in gems:
        layers = []
        base_r = g["r"]
        n_layers = rng.randint(2, 4)
        shrink = rng.uniform(0.55, 0.8)
        for k in range(n_layers):
            layers.append({
                "sides": rng.randint(3, 6),
                "r": base_r * (1.0 - shrink * (k + 1) / max(1, n_layers - 1)),
                "phase": rng.uniform(0.0, 6.28),
                "gain": rng.uniform(0.35, 0.9),
            })
        scenes.append((g, layers))

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            for (g, layers) in scenes:
                cx, cy, r = g["cx"], g["cy"], g["r"]
                if abs(x - cx) > r or abs(y - cy) > r:
                    continue
                for L in layers:
                    rL = L["r"]
                    if rL < 1.0:
                        continue
                    poly = [
                        (cx + rL * math.cos(2 * math.pi * i / L["sides"] + L["phase"]),
                         cy - rL * math.sin(2 * math.pi * i / L["sides"] + L["phase"]))
                        for i in range(L["sides"])
                    ]
                    if not _point_in_poly(x, y, poly):
                        continue
                    d = _poly_min_dist(x, y, poly)
                    w = max(2.0, L["gain"] * 6.0)
                    v += math.exp(-((d / w) ** 2) / 2.0)
            t = min(1.0, max(0.0, v / (len(scenes) * 1.4)))
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels



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



def render_shard(width, height, seed, palette_name):
    """Shard: fractured irregular gem shards with glow halos and crack lines."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    n_shards = rng.randint(2, 4)
    shards = []
    for _ in range(n_shards):
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
            j = rng.randrange(m)
            cracks.append((poly[i], poly[j]))
        shards.append({"cx": cx, "cy": cy, "r": r, "poly": poly, "cracks": cracks})

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            for s in shards:
                poly = s["poly"]
                if _point_in_poly(x, y, poly):
                    d = _poly_min_dist(x, y, poly)
                    v += 0.55 * math.exp(-((d / max(1.0, s["r"] * 0.9)) ** 2) / 2.0)
                d = _poly_min_dist(x, y, poly)
                v += math.exp(-((d / max(1.0, s["r"] * 0.9)) ** 2) / 2.0) * 1.4
                for (p1, p2) in s["cracks"]:
                    if _seg_hit(x, y, p1, p2) < max(s["r"] * 0.12, 2.0):
                        v += 0.9 * math.exp(-(_seg_hit(x, y, p1, p2) / max(1.0, s["r"] * 0.9)) ** 2 / 2.0)
            t = min(1.0, max(0.0, v / (len(shards) * 2.2)))
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels



def render_circuit(width, height, seed, palette_name):
    """Circuit: glowing printed-circuit traces routed between node pads."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    n_traces = rng.randint(2, 4)
    traces = []
    for _ in range(n_traces):
        path = [(rng.uniform(width * 0.08, width * 0.92), rng.uniform(height * 0.08, height * 0.92))]
        for _ in range(rng.randint(2, 5)):
            path.append((rng.uniform(width * 0.04, width * 0.96), rng.uniform(height * 0.04, height * 0.96)))
        traces.append({"path": path})

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            for tr in traces:
                path = tr["path"]
                glow = 0.0
                for i in range(len(path) - 1):
                    d = _seg_hit(x, y, path[i], path[i + 1])
                    glow = max(glow, math.exp(-((d / max(1.0, min(width, height) * 0.12)) ** 2) / 2.0))
                v += glow * 1.6
                for (px, py) in path:
                    d = math.hypot(x - px, y - py)
                    if d < 3.0:
                        v += math.exp(-((d / 3.0) ** 2) / 2.0)
            t = min(1.0, max(0.0, v / (len(traces) * 2.0)))
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels



def render_cosmic(width, height, seed, palette_name):
    """Cosmic: layered starfield with nebula glow and a curved spiral arm."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    stars = []
    for _ in range(rng.randint(40, 90)):
        stars.append((rng.uniform(0, width), rng.uniform(0, height),
                      rng.uniform(0.2, 1.2), rng.uniform(0, 2 * math.pi)))
    arm_r = rng.uniform(min(width, height) * 0.5, min(width, height) * 0.8)
    arm_ang = rng.uniform(0, 2 * math.pi)
    arm_w = rng.uniform(0.4, 1.0)

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            r_c, g_c, b_c = palette_color(stops, 0.5)
            nr, ng, nb = palette_color(stops, 0.85)
            # nebula glow
            d = math.hypot(x - width / 2, y - height / 2)
            glow = math.exp(-((d / max(1.0, min(width, height) * 0.55)) ** 2) / 2.0)
            v += glow * 1.5
            # spiral arm
            ox, oy = width / 2, height / 2
            ang = math.atan2(y - oy, x - ox)
            rad = math.hypot(x - ox, y - oy)
            ang_diff = (ang - arm_ang + math.pi) % (2 * math.pi) - math.pi
            dist_along = rad * math.tan(arm_w) + arm_r * math.cos(ang_diff / arm_w)
            d2 = math.hypot(x - (ox + dist_along * math.cos(arm_ang)),
                            y - (oy + dist_along * math.sin(arm_ang)))
            v += math.exp(-((d2 / max(1.0, min(width, height) * 0.08)) ** 2) / 2.0) * 2.0
            for (sx, sy, sr, sa) in stars:
                d2 = math.hypot(x - sx, y - sy)
                v += sr * math.exp(-((d2 / max(1.0, min(width, height) * 0.02)) ** 2) / 2.0) * sa
            t = min(1.0, max(0.0, v / (len(stars) * 2.5 + 3.0)))
            row.append(palette_color(stops, t))
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
            lens_wob = 0.7 + 0.6 * fbm(r * 0.35, theta * 0.9, seed, octaves=2)
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
                wob = 0.35 + 0.65 * fbm(r * 0.5, theta * 1.5, seed + int(ro * 10.0), octaves=3)
                ro_t = ro + w * (wob - 0.5)
                g = math.exp(-((r - ro_t) ** 2) / (2.0 * w * w)) * doppler * disk_level
                v += g * (1.5 - 0.6 * disk_t)
            # relativistic jets along the spin axis, with cone and Doppler beaming
            ux, uy = jet_axis
            px_ = -uy
            py_ = ux
            t = dx * ux + dy * uy
            perp = dx * px_ + dy * py_
            v += _quasar_jet_glow(t, perp, jet_len, jet_half_angle, r_shadow, beta)
            # faint starfield backdrop
            n = fbm(x * 0.004, y * 0.004, seed, octaves=2)
            v += max(0.0, n - 0.5) * 0.6
            tval = min(1.0, max(0.0, v / (len(orbits) * 2.2 + 4.0)))
            row.append(palette_color(stops, tval))
        pixels.append(row)
    return pixels


def render_mandala(width, height, seed, palette_name):
    """Concentric mandala: noise-wobbled rings of glowing glyphs with radial spokes and a center medallion."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    max_r = math.sqrt(cx * cx + cy * cy)

    rings = []
    n_rings = rng.randint(6, 12)
    for _ in range(n_rings):
        rings.append({
            "radius": max_r * rng.uniform(0.2, 0.9),
            "glyphs": rng.randint(2, 10),
            "size": rng.uniform(2.5, 8.0),
            "phase": rng.uniform(0.0, 6.28),
            "wob": rng.uniform(0.05, 0.18),
        })
    spokes = []
    for i in range(len(rings) - 1):
        spokes.append((rings[i]["radius"], rings[i + 1]["radius"], rng.uniform(0.0, 6.28)))

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            dx = x - cx
            dy = y - cy
            r = math.hypot(dx, dy)
            theta = math.atan2(dy, dx)
            v = 0.0
            # center medallion glow
            v += math.exp(-((r - 0.0) ** 2) / (2.0 * max(3.0, max_r * 0.06) ** 2)) * 3.5
            # ring glows with noise-wobbled radii
            for rib in rings:
                wob = rib["radius"] + rib["wob"] * fbm(r * 0.9, theta * 1.6, seed, octaves=2)
                v += math.exp(-((r - wob) ** 2) / (2.0 * rib["size"] ** 2)) * 1.6
            # radial spokes connecting consecutive rings
            for (r0, r1, phase) in spokes:
                span = max(0.05, r1 - r0)
                dang = math.atan2(math.sin(theta - phase), math.cos(theta - phase))
                if abs(dang) < 0.06:
                    tt = min(1.0, max(0.0, (r - r0) / span))
                    v += math.exp(-((r - (r0 + tt * span)) ** 2) / (2.0 * 2.0 * 2.0)) * 0.9
            # glyph hits on rings
            for rib in rings:
                for gi in range(rib["glyphs"]):
                    ga = rib["phase"] + (gi / rib["glyphs"]) * 2.0 * math.pi
                    gx = cx + math.cos(ga) * rib["radius"]
                    gy = cy + math.sin(ga) * rib["radius"]
                    d = math.hypot(x - gx, y - gy)
                    if d < rib["size"] * 0.9:
                        v += math.exp(-((d ** 2) / (2.0 * rib["size"] * rib["size"]))) * 2.2
            t = min(1.0, max(0.0, v / (len(rings) * 2.2 + 5.0)))
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


def render_drift(width, height, seed, palette_name):
    """Drift: liquid flow-fields where nested noise currents warp the sampling grid,
    producing smooth organic contours that feel like slow-moving currents."""
    stops = PALETTES[palette_name]
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            # current offsets from low-frequency turbulence
            ox = (fbm(x * 0.008, y * 0.008, seed, octaves=3) - 0.5) * 12.0
            oy = (fbm(x * 0.007 + 2.1, y * 0.007 + 5.7, seed, octaves=2) - 0.5) * 9.0
            # sample the value field displaced along the current
            v = fbm(x * 0.015 + ox, y * 0.015 + oy, seed, octaves=4)
            t = min(1.0, max(0.0, v * 1.35 - 0.32)) * 1.08
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


# ---------------------------------------------------------------- render dispatch

def render_crystal(width, height, seed, palette_name):
    """Crystal: noise-grown terrain quantized into low-poly faceted gem faces.

    A value field is grown from fBm noise, its gradient is measured with
    finite differences, and the tilt direction is quantized into discrete
    facet orientations. The result is faceted crystal terrain with crisp
    highlights and dark crevices.
    """
    stops = PALETTES[palette_name]
    light = (0.45, -0.55, 0.70)
    ln = math.sqrt(sum(c * c for c in light)) or 1.0
    light = (light[0] / ln, light[1] / ln, light[2] / ln)

    def height_at(x, y):
        return fbm(x * 0.011, y * 0.011, seed, octaves=5) * 0.5 + 0.5

    def gradient(x, y):
        xm = max(0, x - 1)
        xp = min(width - 1, x + 1)
        ym = max(0, y - 1)
        yp = min(height - 1, y + 1)
        dx = (height_at(xp, y) - height_at(xm, y)) / (xp - xm) if xp != xm else 0.0
        dy = (height_at(x, yp) - height_at(x, ym)) / (yp - ym) if yp != ym else 0.0
        return dx, dy

    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            dx, dy = gradient(x, y)
            h = math.sqrt(dx * dx + dy * dy) or 0.0001
            tx, ty = -dy / h, dx / h
            k = int(round(math.atan2(ty, tx) / (math.pi / 4))) % 8
            qt = (k - int(k / 8)) * math.pi / 4
            n = (math.cos(qt) * tx, math.cos(qt) * ty, math.sin(qt))
            diffuse = max(0.0, n[0] * light[0] + n[1] * light[1] + n[2] * light[2])
            spec = max(0.0, 2.0 * diffuse - (n[0] * light[0] + n[1] * light[1] + n[2] * light[2])) * max(0.0, 1.0 - diffuse)
            t = min(1.0, max(0.0, diffuse * 0.9 + spec * 1.6 - 0.05))
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
            "trail": rng.uniform(0.5, 1.2) * height,
            "width_px": rng.uniform(0.8, 3.0),
        })
    glow_sum = [0.0] * (width * height)
    for c in comets:
        steps = max(1, math.ceil(c["speed"] * 2))
        for s in range(steps):
            t = (s + 0.5) / steps
            px = c["x"] + math.cos(c["angle"]) * c["speed"] * t
            py = c["y"] + math.sin(c["angle"]) * c["speed"] * t
            ix, iy = math.floor(px), math.floor(py)
            fall = max(0.0, 1.0 - t * 1.6)
            g = fall ** 3 * (0.4 + 0.6 * (1.0 - t))
            radius = c["width_px"]
            reach = math.ceil(radius * 2)
            for ddx in range(-reach, reach + 1):
                for ddy in range(-reach, reach + 1):
                    dx, dy = ix + ddx - px, iy + ddy - py
                    d2 = dx * dx + dy * dy
                    if d2 <= (radius * 2) ** 2:
                        xx, yy = (ix + ddx) % width, (iy + ddy) % height
                        glow_sum[yy * width + xx] += g * math.exp(-2 * d2 / radius ** 2)
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


def render(width, height, seed, style, palette_name):
    _validate_dimensions(width, height)
    if palette_name not in PALETTES:
        raise ValueError("unknown palette: %s" % palette_name)
    if style == "waves":
        return render_waves(width, height, seed, palette_name)
    if style == "blobs":
        return render_blobs(width, height, seed, palette_name)
    if style == "orbits":
        return render_orbits(width, height, seed, palette_name)
    if style == "mandelbrot":
        return render_mandelbrot(width, height, seed, palette_name)
    if style == "gems":
        return render_gemstones(width, height, seed, palette_name)
    if style == "rings":
        return render_rings(width, height, seed, palette_name)
    if style == "aurora":
        return render_aurora(width, height, seed, palette_name)
    if style == "plasma":
        return render_plasma(width, height, seed, palette_name)
    if style == "spiral":
        return render_spiral(width, height, seed, palette_name)
    if style == "nebula":
        return render_nebula(width, height, seed, palette_name)
    if style == "kaleidoscope":
        return render_kaleidoscope(width, height, seed, palette_name)
    if style == "petals":
        return render_petals(width, height, seed, palette_name)
    if style == "horizon":
        return render_horizon(width, height, seed, palette_name)
    if style == "starburst":
        return render_starburst(width, height, seed, palette_name)
    if style == "interference":
        return render_interference(width, height, seed, palette_name)
    if style == "constellations":
        return render_constellations(width, height, seed, palette_name)
    if style == "cosmos":
        return render_cosmos(width, height, seed, palette_name)
    if style == "galaxy":
        return render_galaxy(width, height, seed, palette_name)
    if style == "bloom":
        return render_bloom(width, height, seed, palette_name)
    if style == "flowfield":
        return render_flowfield(width, height, seed, palette_name)
    if style == "torus":
        return render_torus(width, height, seed, palette_name)
    if style == "voronoi":
        return render_voronoi(width, height, seed, palette_name)
    if style == "pinwheel":
        return render_pinwheel(width, height, seed, palette_name)
    if style == "lissajous":
        return render_lissajous(width, height, seed, palette_name)
    if style == "geo":
        return render_geo(width, height, seed, palette_name)
    if style == "prism":
        return render_prism(width, height, seed, palette_name)
    if style == "shard":
        return render_shard(width, height, seed, palette_name)
    if style == "circuit":
        return render_circuit(width, height, seed, palette_name)
    if style == "cosmic":
        return render_cosmic(width, height, seed, palette_name)
    if style == "quasar":
        return render_quasar(width, height, seed, palette_name)
    if style == "mandala":
        return render_mandala(width, height, seed, palette_name)
    if style == "drift":
        return render_drift(width, height, seed, palette_name)
    if style == "crystal":
        return render_crystal(width, height, seed, palette_name)
    if style == "ember":
        return render_ember(width, height, seed, palette_name)
    if style == "comet":
        return render_comet(width, height, seed, palette_name)
    if style == "ink":
        return render_ink(width, height, seed, palette_name)

    raise ValueError("unknown style: %s" % style)


def render_gemstones(width, height, seed, palette_name):
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    gems = []
    for i in range(3):
        cx = rng.uniform(0, width - 1)
        cy = rng.uniform(0, height - 1)
        cr = rng.uniform(6, 12)
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
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    rings_cfg = []
    n_rings = rng.randint(4, 9)
    for i in range(n_rings):
        rings_cfg.append((rng.uniform(2.0, 8.0), rng.uniform(0, 1.0), rng.uniform(0, 6.28)))
    pixels = []
    cx = (width - 1) / 2.0
    cy = (height - 1) / 2.0
    for y in range(height):
        row = []
        for x in range(width):
            c = stops[0]
            for (radius, t0, phase) in rings_cfg:
                r = math.sqrt((x - cx) ** 2 + (y - cy) ** 2)
                if r > radius * 0.9 and r < radius * 1.1:
                    t = (math.atan2(y - cy, x - cx) + math.pi) / (2 * math.pi) + phase
                    c = palette_color(stops, min(1.0, max(0.0, t)))
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
            "amp": rng.uniform(15.0, 70.0),
            "freq": rng.uniform(0.004, 0.02),
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
            t = min(1.0, max(0.0, v / (len(ribbons) * 1.6)))
            row.append(palette_color(stops, t * 0.9 + 0.05))
        pixels.append(row)
    return pixels



def render_plasma(width, height, seed, palette_name):
    """Plasma effect: layered sine waves summed with noise, mapped through palette."""
    stops = PALETTES[palette_name]
    rng = random.Random(seed)
    layers = [
        (1.0, 3.0, rng.uniform(0.0, 6.28)),
        (0.8, 5.0, rng.uniform(0.0, 6.28)),
        (0.6, 8.0, rng.uniform(0.0, 6.28)),
        (0.45, 13.0, rng.uniform(0.0, 6.28)),
    ]
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            v = 0.0
            n = fbm(x * 0.01, y * 0.01, seed, octaves=4) * 0.5 + 0.5
            for (amp, freq, phase) in layers:
                v += amp * math.sin(x * freq + y * freq * 1.7 + phase) * (0.5 + 0.5 * n)
            t = min(1.0, max(0.0, v * 1.15 - 0.1))
            row.append(palette_color(stops, t))
        pixels.append(row)
    return pixels


# ---------------------------------------------------------------- png writer

def _validate_dimensions(width, height):
    if (not isinstance(width, int) or not isinstance(height, int)
            or width <= 0 or height <= 0):
        raise ValueError("width and height must be positive integers")


def _png_chunk(tag, data):
    return (struct.pack(">I", len(data)) + tag + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))


def _png_pixels(pixels, width, height):
    """Encode linear RGB samples consistently for still and animated PNGs."""
    _validate_dimensions(width, height)
    if len(pixels) != height or any(len(row) != width for row in pixels):
        raise ValueError("pixel array must match width and height")

    def to_srgb(c):
        c = max(0.0, min(1.0, c / 255.0))
        if c > 0.0031308:
            c = 1.055 * c ** (1.0 / 2.4) - 0.055
        else:
            c *= 12.92
        return int(c * 255 + 0.5)

    raw = bytearray()
    for row in pixels:
        raw.append(0)  # PNG filter: None
        for r, g, b in row:
            raw.extend((to_srgb(r), to_srgb(g), to_srgb(b)))
    return zlib.compress(raw, 9)


def _png_header(width, height):
    _validate_dimensions(width, height)
    return (b"\x89PNG\r\n\x1a\n" + _png_chunk(
        b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)))


def write_animated_png(path, frames, width, height, fps=12):
    """Write a looping APNG; each frame replaces the complete canvas."""
    if not frames:
        raise ValueError("no frames to write")
    if not isinstance(fps, int) or not 1 <= fps <= 65535:
        raise ValueError("fps must be an integer between 1 and 65535")
    header = _png_header(width, height)
    sequence = 0
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(
                dir=os.path.dirname(os.path.abspath(path)), prefix=".abstract-art-",
                delete=False) as output:
            temporary = output.name
            output.write(header)
            output.write(_png_chunk(b"acTL", struct.pack(">II", len(frames), 0)))
            for index, pixels in enumerate(frames):
                data = _png_pixels(pixels, width, height)
                control = struct.pack(">IIIIIHHBB", sequence, width, height, 0, 0, 1, fps, 0, 0)
                output.write(_png_chunk(b"fcTL", control))
                sequence += 1
                if index == 0:
                    output.write(_png_chunk(b"IDAT", data))
                else:
                    output.write(_png_chunk(b"fdAT", struct.pack(">I", sequence) + data))
                    sequence += 1
            output.write(_png_chunk(b"IEND", b""))
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            try:
                os.unlink(temporary)
            except FileNotFoundError:
                pass



def write_png(path, pixels, width, height):
    data = _png_pixels(pixels, width, height)
    with open(path, "wb") as output:
        output.write(_png_header(width, height))
        output.write(_png_chunk(b"IDAT", data))
        output.write(_png_chunk(b"IEND", b""))


# ---------------------------------------------------------------- cli

def main():
    parser = argparse.ArgumentParser(description="Procedurally generated abstract art")
    parser.add_argument("--style", default="waves", choices=["waves", "blobs", "orbits", "mandelbrot", "gems", "rings", "aurora", "plasma", "spiral", "nebula", "kaleidoscope", "petals", "horizon", "starburst", "interference", "constellations", "cosmos", "galaxy", "bloom", "flowfield", "torus", "voronoi", "pinwheel", "lissajous", "geo", "prism", "shard", "circuit", "cosmic", "quasar", "mandala", "drift", "crystal", "ember", "comet", "ink"])
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
