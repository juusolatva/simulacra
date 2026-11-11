//! The built-in palettes, the same as `PALETTES` in `simulacra.py` (a Python test
//! compares them through the adapter). Stops are sRGB, from the first color
//! (the background in many styles) to the last.

/// One color stop as `(r, g, b)`, each 0 to 255.
pub type Stop = (i64, i64, i64);

/// A built-in palette: its name and its stops.
pub type NamedPalette = (&'static str, &'static [Stop]);

/// `(name, stops)` for every built-in palette, sorted by name.
pub const PALETTES: &[NamedPalette] = &[
    (
        "candy",
        &[
            (28, 10, 44),
            (150, 40, 150),
            (240, 90, 160),
            (120, 220, 220),
            (255, 240, 150),
        ],
    ),
    (
        "crimson",
        &[
            (12, 2, 4),
            (64, 6, 16),
            (140, 14, 30),
            (214, 48, 52),
            (255, 196, 184),
        ],
    ),
    (
        "deep",
        &[
            (13, 19, 49),
            (27, 42, 94),
            (61, 90, 153),
            (111, 163, 206),
            (168, 208, 230),
        ],
    ),
    (
        "ember",
        &[
            (26, 12, 8),
            (64, 28, 18),
            (130, 56, 30),
            (216, 102, 48),
            (255, 178, 102),
        ],
    ),
    (
        "forest",
        &[
            (11, 31, 18),
            (20, 61, 36),
            (47, 107, 60),
            (85, 160, 106),
            (142, 201, 160),
        ],
    ),
    (
        "gold",
        &[
            (18, 12, 2),
            (72, 48, 8),
            (152, 102, 26),
            (226, 158, 52),
            (255, 222, 140),
        ],
    ),
    (
        "jewel",
        &[
            (10, 6, 24),
            (20, 70, 160),
            (14, 140, 90),
            (170, 40, 140),
            (250, 190, 50),
        ],
    ),
    (
        "lagoon",
        &[
            (2, 16, 20),
            (8, 48, 56),
            (20, 96, 102),
            (56, 160, 156),
            (176, 230, 220),
        ],
    ),
    (
        "neon",
        &[
            (10, 0, 20),
            (40, 0, 60),
            (90, 10, 90),
            (180, 20, 160),
            (255, 110, 220),
        ],
    ),
    (
        "noir",
        &[
            (8, 8, 10),
            (37, 40, 48),
            (88, 94, 106),
            (168, 176, 188),
            (244, 246, 250),
        ],
    ),
    (
        "ocean",
        &[
            (8, 16, 38),
            (16, 42, 72),
            (32, 72, 106),
            (56, 108, 140),
            (96, 148, 178),
        ],
    ),
    (
        "patina",
        &[
            (16, 12, 10),
            (92, 48, 28),
            (178, 104, 62),
            (70, 150, 130),
            (196, 232, 214),
        ],
    ),
    (
        "phosphor",
        &[
            (2, 8, 4),
            (12, 54, 20),
            (30, 136, 56),
            (82, 230, 118),
            (220, 255, 228),
        ],
    ),
    (
        "rainbow",
        &[
            (12, 6, 28),
            (88, 20, 168),
            (30, 96, 230),
            (24, 190, 96),
            (250, 224, 40),
            (250, 120, 24),
            (236, 40, 56),
        ],
    ),
    (
        "risoprint",
        &[
            (24, 16, 36),
            (0, 130, 138),
            (230, 50, 120),
            (244, 180, 40),
            (248, 242, 230),
        ],
    ),
    (
        "sepia",
        &[
            (20, 14, 10),
            (66, 48, 34),
            (128, 96, 66),
            (194, 162, 122),
            (242, 228, 202),
        ],
    ),
    (
        "sherbet",
        &[
            (36, 8, 52),
            (160, 30, 120),
            (250, 90, 80),
            (255, 170, 60),
            (255, 245, 190),
        ],
    ),
    (
        "sunset",
        &[
            (26, 5, 51),
            (93, 31, 63),
            (168, 50, 74),
            (232, 99, 92),
            (249, 168, 39),
        ],
    ),
    (
        "thermal",
        &[
            (0, 0, 4),
            (66, 10, 104),
            (188, 55, 84),
            (249, 142, 9),
            (252, 255, 164),
        ],
    ),
    (
        "tropical",
        &[
            (4, 24, 34),
            (0, 110, 120),
            (30, 190, 150),
            (190, 230, 70),
            (255, 90, 130),
        ],
    ),
    (
        "tundra",
        &[
            (14, 22, 24),
            (44, 68, 66),
            (95, 122, 118),
            (156, 184, 182),
            (228, 238, 238),
        ],
    ),
    (
        "ultraviolet",
        &[
            (8, 2, 18),
            (28, 8, 48),
            (64, 18, 92),
            (120, 36, 150),
            (200, 140, 255),
        ],
    ),
    (
        "vaporwave",
        &[
            (20, 8, 48),
            (100, 40, 180),
            (240, 70, 190),
            (80, 220, 240),
            (255, 230, 245),
        ],
    ),
    (
        "watercolor",
        &[
            (36, 43, 63),
            (91, 119, 153),
            (164, 190, 202),
            (204, 157, 159),
            (246, 237, 219),
        ],
    ),
];

/// The stops of the built-in palette `name`, if there is one.
#[must_use]
pub fn palette_stops(name: &str) -> Option<&'static [Stop]> {
    PALETTES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, stops)| *stops)
}

#[cfg(test)]
mod tests {
    use super::{PALETTES, palette_stops};

    #[test]
    fn palettes_are_sorted_unique_and_have_five_stops_from_dark() {
        for pair in PALETTES.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} before {}", pair[0].0, pair[1].0);
        }
        for (name, stops) in PALETTES {
            assert!(stops.len() >= 5, "{name} has {} stops", stops.len());
            // Many styles take their background from the first stop.
            let (r, g, b) = stops[0];
            assert!(r.max(g).max(b) < 64, "{name} starts at {:?}", stops[0]);
            assert!(
                stops
                    .iter()
                    .all(|&(r, g, b)| [r, g, b].iter().all(|c| (0..=255).contains(c))),
                "{name}"
            );
        }
        assert!(palette_stops("deep").is_some());
        assert!(palette_stops("nosuch").is_none());
    }
}
