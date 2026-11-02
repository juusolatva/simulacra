//! Palette interpolation, ported from `palette_color()` in `abstract_art.py`.

pub struct Palette {
    stops: Vec<(i64, i64, i64)>,
}

impl Palette {
    pub fn new(stops: Vec<(i64, i64, i64)>) -> Result<Self, String> {
        if stops.is_empty() {
            return Err("palette must contain at least one color stop".into());
        }
        Ok(Self { stops })
    }

    /// Stop `index` as RGB, clamped to 0..=255.
    pub fn stop(&self, index: usize) -> [u8; 3] {
        let (r, g, b) = self.stops[index];
        [channel(r as f64), channel(g as f64), channel(b as f64)]
    }

    /// Map t in [0, 1] to RGB. Channels are truncated like Python's int() and
    /// then clamped to 0..=255, as the PNG writers do.
    #[allow(
        clippy::manual_clamp,
        reason = "clamp() keeps NaN, but Python's max(0.0, min(1.0, t)) maps NaN to 1.0"
    )]
    pub fn color(&self, t: f64) -> [u8; 3] {
        // Python: max(0.0, min(1.0, t)). f64::min/max agree, including NaN -> 1.0.
        let t = t.min(1.0).max(0.0);
        let last = self.stops.len() - 1;
        if t <= 0.0 || last == 0 {
            return self.stop(0);
        }
        if t >= 1.0 {
            return self.stop(last);
        }
        let steps = last as f64;
        for i in 0..last {
            let span = (i + 1) as f64 / steps;
            if t <= span {
                let local = (t - i as f64 / steps) * steps;
                let (a, b) = (self.stops[i], self.stops[i + 1]);
                return [
                    channel(a.0 as f64 + (b.0 - a.0) as f64 * local),
                    channel(a.1 as f64 + (b.1 - a.1) as f64 * local),
                    channel(a.2 as f64 + (b.2 - a.2) as f64 * local),
                ];
            }
        }
        self.stop(last)
    }
}

#[inline]
fn channel(value: f64) -> u8 {
    // `as` truncates toward zero (Python int()) and saturates to 0..=255.
    value as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_and_truncates_like_python() {
        let palette = Palette::new(vec![(13, 19, 49), (27, 42, 94), (61, 90, 153)]).unwrap();
        assert_eq!(palette.color(-1.0), [13, 19, 49]);
        assert_eq!(palette.color(f64::NAN), [61, 90, 153]);
        // Python: palette_color(stops, 0.3) == (21, 32, 76)
        assert_eq!(palette.color(0.3), [21, 32, 76]);
    }
}
