//! Effect math: the color of one lamp at one instant. Pure and deterministic, so the
//! frontend preview (`src/features/lighting/effect.ts`, a line-by-line port) and the devices
//! show the same thing. Both test suites check the shared vectors in
//! `src/features/lighting/effect-vectors.json`.
//!
//! Conventions:
//! - `position` is the lamp's place along the device (or along all devices when synced), 0 at
//!   the left edge and 1 at the right edge. Without a measured layout, lamps are spread
//!   evenly by index (`default_position`).
//! - One cycle of a dynamic effect lasts `12_000 / speed` ms (`cycle_ms`).
//! - Colors are interpolated in sRGB and rounded at the end (half away from zero).

use models::control::lighting::{LightingColor, LightingDirection, LightingEffect};

/// One full cycle of a dynamic effect at `speed` (clamped to 1–10), in milliseconds.
pub fn cycle_ms(speed: u8) -> f64 {
    12_000.0 / f64::from(speed.clamp(1, 10))
}

/// Evenly spread position of lamp `index` of `count` (0.5 for a single lamp).
pub fn default_position(index: u32, count: u32) -> f64 {
    if count <= 1 {
        0.5
    } else {
        f64::from(index.min(count - 1)) / f64::from(count - 1)
    }
}

/// Fractional part in [0, 1), also for negative values.
fn fract(x: f64) -> f64 {
    x - x.floor()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rgbf {
    r: f64,
    g: f64,
    b: f64,
}

impl Rgbf {
    fn from(c: LightingColor) -> Self {
        Self {
            r: f64::from(c.r),
            g: f64::from(c.g),
            b: f64::from(c.b),
        }
    }

    fn scale(self, k: f64) -> Self {
        Self {
            r: self.r * k,
            g: self.g * k,
            b: self.b * k,
        }
    }

    fn lerp(self, other: Self, t: f64) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }

    fn round(self) -> LightingColor {
        let ch = |v: f64| v.round().clamp(0.0, 255.0) as u8;
        LightingColor::rgb(ch(self.r), ch(self.g), ch(self.b))
    }
}

/// HSV (all in [0, 1]) to sRGB 0–255.
fn hsv(h: f64, s: f64, v: f64) -> Rgbf {
    let h6 = fract(h) * 6.0;
    let sector = h6.floor();
    let f = h6 - sector;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match sector as i64 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Rgbf {
        r: r * 255.0,
        g: g * 255.0,
        b: b * 255.0,
    }
}

/// Palette lookup at `u` in [0, 1): `cyclic` wraps from the last color back to the first
/// (moving effects), otherwise the colors span 0..=1 end to end (gradient).
fn palette(colors: &[LightingColor], u: f64, cyclic: bool) -> Rgbf {
    match colors {
        [] => Rgbf::from(LightingColor::BLACK),
        [only] => Rgbf::from(*only),
        _ => {
            let n = colors.len();
            let segments = if cyclic { n } else { n - 1 };
            let x = if cyclic {
                fract(u) * segments as f64
            } else {
                u.clamp(0.0, 1.0) * segments as f64
            };
            let i = (x.floor() as usize).min(segments - 1);
            let t = x - i as f64;
            let a = Rgbf::from(colors[i]);
            let b = Rgbf::from(colors[(i + 1) % n]);
            a.lerp(b, t)
        }
    }
}

/// The color of lamp `lamp_index` (of `lamp_count`) at `t_ms` after the effect started.
/// `lamp_position` overrides the evenly spread position (see the module docs).
pub fn color_at(
    effect: &LightingEffect,
    t_ms: u64,
    lamp_index: u32,
    lamp_count: u32,
    lamp_position: Option<f64>,
) -> LightingColor {
    let x = lamp_position.unwrap_or_else(|| default_position(lamp_index, lamp_count));
    let t = t_ms as f64;
    match effect {
        LightingEffect::Off => LightingColor::BLACK,
        LightingEffect::Static { color } => *color,
        LightingEffect::Breathing { color, period } => {
            let period = f64::from((*period).max(1));
            let phase = fract(t / period);
            let k = (1.0 - (2.0 * std::f64::consts::PI * phase).cos()) / 2.0;
            Rgbf::from(*color).scale(k).round()
        }
        LightingEffect::Rainbow { speed } => hsv(t / cycle_ms(*speed) + x, 1.0, 1.0).round(),
        LightingEffect::Wave {
            colors,
            speed,
            direction,
        } => {
            let shift = t / cycle_ms(*speed);
            let u = match direction {
                LightingDirection::LeftToRight => x - shift,
                LightingDirection::RightToLeft => x + shift,
            };
            palette(colors, u, true).round()
        }
        LightingEffect::Gradient { colors } => palette(colors, x, false).round(),
        LightingEffect::ColorCycle { colors, speed } => {
            palette(colors, t / cycle_ms(*speed), true).round()
        }
    }
}

/// Light level 0–1 of a single-color (white) light: the effect's brightness envelope, its
/// color ignored (Off 0, Static 1, Breathing the breath; other effects show steady).
pub fn level_at(effect: &LightingEffect, t_ms: u64) -> f64 {
    let white = match effect {
        LightingEffect::Off => return 0.0,
        LightingEffect::Breathing { period, .. } => LightingEffect::Breathing {
            color: LightingColor::WHITE,
            period: *period,
        },
        _ => return 1.0,
    };
    f64::from(color_at(&white, t_ms, 0, 1, None).r) / 255.0
}

/// Scales a color by a brightness of 0–100.
pub fn dim(color: LightingColor, brightness: u8) -> LightingColor {
    if brightness >= 100 {
        return color;
    }
    Rgbf::from(color)
        .scale(f64::from(brightness) / 100.0)
        .round()
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::control::lighting::LightingColor as C;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Vector {
        effect: LightingEffect,
        t: u64,
        index: u32,
        count: u32,
        position: Option<f64>,
        expected: [u8; 3],
    }

    const VECTORS: &str = include_str!("../../../../src/features/lighting/effect-vectors.json");

    #[test]
    fn shared_vectors_match() {
        let vectors: Vec<Vector> = serde_json::from_str(VECTORS).expect("vectors parse");
        assert!(vectors.len() >= 30);
        for (i, v) in vectors.iter().enumerate() {
            let c = color_at(&v.effect, v.t, v.index, v.count, v.position);
            assert_eq!([c.r, c.g, c.b], v.expected, "vector {i}: {:?}", v.effect);
        }
    }

    #[test]
    fn basic_effects() {
        assert_eq!(color_at(&LightingEffect::Off, 5, 0, 1, None), C::BLACK);
        let red = C::rgb(255, 0, 0);
        assert_eq!(
            color_at(&LightingEffect::Static { color: red }, 999, 3, 10, None),
            red
        );
        let b = LightingEffect::Breathing {
            color: red,
            period: 4000,
        };
        assert_eq!(color_at(&b, 0, 0, 1, None), C::BLACK);
        assert_eq!(color_at(&b, 2000, 0, 1, None), red);
        assert_eq!(color_at(&b, 4000, 0, 1, None), C::BLACK);
        assert!((level_at(&b, 2000) - 1.0).abs() < 1e-9);
        assert_eq!(level_at(&LightingEffect::Off, 0), 0.0);
        assert_eq!(level_at(&LightingEffect::Rainbow { speed: 3 }, 0), 1.0);
    }

    #[test]
    fn rainbow_spreads_hue_across_lamps_and_time() {
        let r = LightingEffect::Rainbow { speed: 1 };
        assert_eq!(color_at(&r, 0, 0, 2, Some(0.0)), C::rgb(255, 0, 0));
        // A third of a cycle later the first lamp is green.
        assert_eq!(color_at(&r, 4000, 0, 2, Some(0.0)), C::rgb(0, 255, 0));
        assert_eq!(color_at(&r, 0, 0, 2, Some(2.0 / 3.0)), C::rgb(0, 0, 255));
    }

    #[test]
    fn gradient_spans_end_to_end_and_wave_moves() {
        let colors = vec![C::rgb(0, 0, 0), C::rgb(200, 100, 50)];
        let g = LightingEffect::Gradient {
            colors: colors.clone(),
        };
        assert_eq!(color_at(&g, 0, 0, 3, None), C::rgb(0, 0, 0));
        assert_eq!(color_at(&g, 0, 1, 3, None), C::rgb(100, 50, 25));
        assert_eq!(color_at(&g, 0, 2, 3, None), C::rgb(200, 100, 50));
        let w = |direction| LightingEffect::Wave {
            colors: colors.clone(),
            speed: 10,
            direction,
        };
        // Moving right: what lamp x shows now, lamp x + d shows d of a cycle later.
        let ltr = w(LightingDirection::LeftToRight);
        assert_eq!(
            color_at(&ltr, 0, 0, 1, Some(0.2)),
            color_at(&ltr, 360, 0, 1, Some(0.5))
        );
        let rtl = w(LightingDirection::RightToLeft);
        assert_eq!(
            color_at(&rtl, 0, 0, 1, Some(0.5)),
            color_at(&rtl, 360, 0, 1, Some(0.2))
        );
    }

    #[test]
    fn dimming_and_positions() {
        assert_eq!(dim(C::rgb(200, 100, 0), 50), C::rgb(100, 50, 0));
        assert_eq!(dim(C::rgb(200, 100, 0), 0), C::BLACK);
        assert_eq!(default_position(0, 1), 0.5);
        assert_eq!(default_position(4, 5), 1.0);
        assert_eq!(cycle_ms(0), 12_000.0);
    }

    /// Regenerates the shared vectors (run once after changing the math, then commit):
    /// `cargo test -p control write_effect_vectors -- --ignored`
    #[test]
    #[ignore = "writes the shared fixture"]
    fn write_effect_vectors() {
        let red = C::rgb(255, 0, 0);
        let azure = C::rgb(78, 165, 255);
        let pal = vec![C::rgb(255, 59, 107), C::rgb(255, 179, 64), azure];
        let effects = vec![
            LightingEffect::Off,
            LightingEffect::Static { color: azure },
            LightingEffect::Breathing {
                color: azure,
                period: 3000,
            },
            LightingEffect::Breathing {
                color: red,
                period: 7000,
            },
            LightingEffect::Rainbow { speed: 1 },
            LightingEffect::Rainbow { speed: 7 },
            LightingEffect::Wave {
                colors: pal.clone(),
                speed: 4,
                direction: LightingDirection::LeftToRight,
            },
            LightingEffect::Wave {
                colors: pal.clone(),
                speed: 9,
                direction: LightingDirection::RightToLeft,
            },
            LightingEffect::Gradient {
                colors: pal.clone(),
            },
            LightingEffect::Gradient {
                colors: vec![C::rgb(10, 20, 30)],
            },
            LightingEffect::ColorCycle {
                colors: pal,
                speed: 3,
            },
        ];
        let samples: [(u64, u32, u32, Option<f64>); 6] = [
            (0, 0, 1, None),
            (250, 2, 5, None),
            (1337, 7, 12, None),
            (4100, 0, 3, Some(0.83)),
            (9_999, 11, 24, None),
            (123_457, 3, 4, Some(0.1)),
        ];
        let mut out = Vec::new();
        for e in &effects {
            for (t, index, count, position) in samples {
                let c = color_at(e, t, index, count, position);
                out.push(serde_json::json!({
                    "effect": e, "t": t, "index": index, "count": count,
                    "position": position, "expected": [c.r, c.g, c.b],
                }));
            }
        }
        // One vector per line: small diffs, easy to read.
        let lines: Vec<String> = out.iter().map(|v| format!("  {v}")).collect();
        let text = format!("[\n{}\n]", lines.join(",\n"));
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src/features/lighting/effect-vectors.json"
        );
        std::fs::write(path, text + "\n").expect("write");
    }
}
