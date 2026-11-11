//! Properties every style keeps, checked through the engine's scene builders
//! and the checked `render` at small sizes. They hold on both math backends;
//! the exact images are pinned by the golden hashes in `core/cli/tests/golden`.

use simulacra_engine::{
    PALETTES, Palette, SCENE_BUILDERS, STYLES, build_scene, palette_stops, render, scene_work,
    seed32,
};

fn named(name: &str) -> Palette {
    Palette::new(palette_stops(name).expect(name).to_vec()).unwrap()
}

fn rgb(style: &str, w: usize, h: usize, seed: i64, palette: &Palette, phase: f64) -> Vec<u8> {
    let scene = build_scene(style, w, h, seed, palette, phase).expect(style);
    render(&*scene, w, h, seed32(seed), palette)
        .unwrap_or_else(|err| panic!("{style} at {w}x{h}: {err}"))
}

/// Every style, after checking the builder list against the registry.
fn styles() -> &'static [&'static str] {
    let mut registry = STYLES.to_vec();
    registry.sort_unstable();
    assert_eq!(SCENE_BUILDERS.to_vec(), registry);
    assert_eq!(SCENE_BUILDERS.len(), 48);
    &SCENE_BUILDERS
}

#[test]
fn every_style_renders_with_every_palette() {
    for &(name, _) in PALETTES {
        let palette = named(name);
        for style in styles() {
            assert_eq!(
                rgb(style, 9, 7, 3, &palette, 0.0).len(),
                9 * 7 * 3,
                "{style} {name}"
            );
        }
    }
}

#[test]
fn every_style_uses_a_wide_palette_range() {
    let palette = named("deep");
    let lookup: Vec<[u8; 3]> = (0..=100)
        .map(|i| palette.color(f64::from(i) / 100.0))
        .collect();
    let position = |pixel: &[u8]| {
        let distance = |c: &[u8; 3]| -> i32 {
            c.iter()
                .zip(pixel)
                .map(|(&a, &b)| (i32::from(a) - i32::from(b)).pow(2))
                .sum()
        };
        // The first sample of least distance, as Python's min().
        let mut best = 0;
        for (i, c) in lookup.iter().enumerate() {
            if distance(c) < distance(&lookup[best]) {
                best = i;
            }
        }
        best
    };
    for style in styles() {
        let image = rgb(style, 64, 64, 3, &palette, 0.0);
        let positions: Vec<usize> = (0..64)
            .step_by(3)
            .flat_map(|y| (0..64).step_by(3).map(move |x| (y * 64 + x) * 3))
            .map(|i| position(&image[i..i + 3]))
            .collect();
        let span = positions.iter().max().unwrap() - positions.iter().min().unwrap();
        assert!(span > 50, "{style} spans {span}/100 of its palette");
    }
}

#[test]
fn rendering_is_deterministic() {
    let palette = named("sunset");
    for style in styles() {
        assert_eq!(
            rgb(style, 32, 24, 7, &palette, 0.37),
            rgb(style, 32, 24, 7, &palette, 0.37),
            "{style}"
        );
    }
}

#[test]
fn every_style_moves_and_loops() {
    let palette = named("rainbow");
    for style in styles() {
        let start = rgb(style, 40, 30, 5, &palette, 0.0);
        assert_ne!(
            start,
            rgb(style, 40, 30, 5, &palette, 0.25),
            "{style} does not move"
        );
        for whole in [1.0, -1.0, 3.0] {
            let looped = rgb(style, 40, 30, 5, &palette, whole);
            let worst = start
                .iter()
                .zip(&looped)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            assert!(
                worst <= 3,
                "{style} at phase {whole} is {worst} levels from phase 0"
            );
        }
    }
}

#[test]
fn huge_phases_reduce_to_a_loop_position() {
    let palette = named("neon");
    for style in styles() {
        assert_eq!(
            rgb(style, 20, 14, 3, &palette, 1e308),
            rgb(style, 20, 14, 3, &palette, 0.0),
            "{style} at 1e308"
        );
        assert_eq!(
            rgb(style, 20, 14, 3, &palette, 2f64.powi(40) + 0.25),
            rgb(style, 20, 14, 3, &palette, 0.25),
            "{style} at 2**40 + 0.25"
        );
    }
}

#[test]
fn every_style_renders_at_tiny_sizes() {
    let palette = named("deep");
    for style in styles() {
        for (w, h) in [(1, 1), (1, 9), (9, 1)] {
            assert_eq!(
                rgb(style, w, h, 17, &palette, 0.0).len(),
                w * h * 3,
                "{style} {w}x{h}"
            );
        }
    }
}

#[test]
fn scenes_stay_well_within_the_work_limit() {
    let palette = named("rainbow");
    let sizes = [
        (1, 1),
        (3, 2),
        (1, 4000),
        (4000, 1),
        (64, 64),
        (640, 480),
        (100, 2000),
        (3840, 2160),
    ];
    for style in styles() {
        for (w, h) in sizes {
            for (seed, phase) in [(1, 0.0), (7, 0.81), (12345, 0.37)] {
                let scene = build_scene(style, w, h, seed, &palette, phase).unwrap();
                let (work, limit) = scene_work(&*scene, w, h, &palette).unwrap();
                assert!(
                    work <= limit / 8.0,
                    "{style} {w}x{h} seed {seed}: work {work} over an eighth of {limit}"
                );
            }
        }
    }
}

/// Mean absolute difference between horizontally adjacent pixels, per channel.
fn roughness(image: &[u8], w: usize, h: usize) -> f64 {
    let mut total = 0u64;
    for row in image.chunks(w * 3) {
        for x in 0..(w - 1) * 3 {
            total += u64::from(row[x].abs_diff(row[x + 3]));
        }
    }
    total as f64 / (h * (w - 1)) as f64
}

#[test]
fn wave_styles_are_not_aliased() {
    let palette = named("deep");
    for style in [
        "plasma",
        "spiral",
        "interference",
        "cymatics",
        "optics",
        "opart",
    ] {
        let coarse = roughness(&rgb(style, 32, 32, 1, &palette, 0.0), 32, 32);
        let fine = roughness(&rgb(style, 128, 128, 1, &palette, 0.0), 128, 128);
        assert!(
            fine < coarse / 2.0,
            "{style}: {fine} at 128 vs {coarse} at 32"
        );
    }
}

#[test]
fn ring_styles_scale_with_the_canvas() {
    let palette = named("deep");
    let background = palette.stop(0);
    let coverage = |style: &str, size: usize| {
        let image = rgb(style, size, size, 4, &palette, 0.0);
        let covered = image.chunks(3).filter(|p| **p != background[..]).count();
        covered as f64 / (size * size) as f64
    };
    for style in ["rings", "gems"] {
        let (small, large) = (coverage(style, 48), coverage(style, 144));
        assert!(small > 0.05, "{style} covers {small} at 48");
        assert!(
            (small - large).abs() < 0.5 * small.max(large),
            "{style}: {small} at 48 vs {large} at 144"
        );
    }
}

#[test]
fn formerly_flat_styles_have_spatial_detail() {
    let palette = named("deep");
    for style in [
        "mandelbrot",
        "petals",
        "starburst",
        "kaleidoscope",
        "horizon",
        "ink",
    ] {
        let image = rgb(style, 64, 48, 1, &palette, 0.0);
        let colors: std::collections::HashSet<&[u8]> = image.chunks(3).collect();
        assert!(colors.len() > 16, "{style} has {} colors", colors.len());
        // A silhouette row can have only two colors; it still needs edges.
        let minimum = if style == "horizon" { 1 } else { 4 };
        let busiest = image
            .chunks(64 * 3)
            .map(|row| {
                row.chunks(3)
                    .collect::<std::collections::HashSet<_>>()
                    .len()
            })
            .max()
            .unwrap();
        assert!(
            busiest > minimum,
            "{style}: at most {busiest} colors in a row"
        );
    }
}
