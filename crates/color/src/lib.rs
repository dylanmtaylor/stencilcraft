//! Colors for StencilCraft: the single place color text and color math are
//! defined, so SVG import/export, engine styles and UI tokens never hand-roll
//! their own parsing.
//!
//! Scope is deliberately small and reusable: parse the CSS forms a cutting
//! workflow meets (hex, `rgb()`, the 16 CSS basic keywords), format back to a
//! canonical lowercase hex string, and do the two bits of math callers
//! actually need (perceived luminance, linear blend). Alpha is out of scope
//! until a consumer needs it — cut paths have no opacity.
//!
//! Per the workspace rules this crate is panic-free and wasm-buildable: it
//! depends only on `serde` and `thiserror`. Grammar follows
//! <https://www.w3.org/TR/css-color-4/> (subset).

use serde::{Deserialize, Serialize};

/// An 8-bit sRGB color, no alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b }
    }

    /// Canonical CSS form: lowercase `#rrggbb`.
    pub fn to_css(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Perceived (relative) luminance in `[0, 1]`, the WCAG weighting of
    /// sRGB channels. 0 is black, 1 is white.
    pub fn luminance(&self) -> f32 {
        let r = channel_value(self.r);
        let g = channel_value(self.g);
        let b = channel_value(self.b);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// Blend from `self` to `other` by `t` in `[0, 1]`: `t = 0.5` is a
    /// plain midpoint, not perceptually flat. `t` outside the range is
    /// clamped, so UI dials can pass anything.
    pub fn blend(self, other: Color, t: f32) -> Color {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Color {
            r: lerp_channel(self.r, other.r, t),
            g: lerp_channel(self.g, other.g, t),
            b: lerp_channel(self.b, other.b, t),
        }
    }
}

fn channel_value(v: u8) -> f32 {
    let c = f32::from(v) / 255.0;
    // WCAG 2.x "relative luminance" is plain channel weighting; css-color-4
    // applies the sRGB transfer function first. We use the WCAG form, which
    // is what palette contrast helpers want.
    c
}

fn lerp_channel(a: u8, b: u8, t: f32) -> u8 {
    let l = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
    (l + 0.5).clamp(0.0, 255.0) as u8
}

/// Why a color string could not be parsed. `Display` names the offending
/// input so callers can echo it without re-wrapping.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid color {input:?}")]
pub struct ColorParseError {
    input: String,
}

/// Parse a CSS color string: the 16 basic keywords, `#rgb`, `#rrggbb`
/// (case-insensitive, optional whitespace tolerated), or
/// `rgb(r, g, b)` with decimal integers 0–255. Comma- or
/// space-separated channels are both accepted.
///
/// Any other form — percentages, hsl, color(), alpha variants — is an error,
/// not a guess: silent color loss is worse than a reported failure.
pub fn parse_color(input: &str) -> Result<Color, ColorParseError> {
    let err = || ColorParseError {
        input: input.to_string(),
    };
    // Cap input length before work: a caller feeding untrusted text cannot
    // make this quadratic.
    if input.len() > 64 {
        return Err(err());
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(err());
    }
    if trimmed.starts_with('#') {
        return parse_hex_body(trimmed);
    }
    if let Some(body) = trimmed.strip_prefix("rgb(") {
        return parse_rgb_fn(body, trimmed);
    }
    // Longest keyword is "digits"? No: "teal"/"aqua"/"lime"/"gray"/"olive"/"navy" are all ≤ 5... silver = 6.
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "black" => Ok(Color::new(0, 0, 0)),
        "white" => Ok(Color::new(255, 255, 255)),
        "red" => Ok(Color::new(255, 0, 0)),
        "lime" => Ok(Color::new(0, 255, 0)),
        "blue" => Ok(Color::new(0, 0, 255)),
        "yellow" => Ok(Color::new(255, 255, 0)),
        "aqua" | "cyan" => Ok(Color::new(0, 255, 255)),
        "magenta" | "fuchsia" => Ok(Color::new(255, 0, 255)),
        "silver" => Ok(Color::new(192, 192, 192)),
        "gray" | "grey" => Ok(Color::new(128, 128, 128)),
        "maroon" => Ok(Color::new(128, 0, 0)),
        "olive" => Ok(Color::new(128, 128, 0)),
        "green" => Ok(Color::new(0, 128, 0)),
        "purple" => Ok(Color::new(128, 0, 128)),
        "navy" => Ok(Color::new(0, 0, 128)),
        "teal" => Ok(Color::new(0, 128, 128)),
        _ => Err(err()),
    }
}

fn parse_hex_body(trimmed: &str) -> Result<Color, ColorParseError> {
    let err = || ColorParseError {
        input: trimmed.to_string(),
    };
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed).trim();
    if digits.len() == 3 {
        let mut chans = [0u8; 3];
        for (i, ch) in digits.chars().enumerate() {
            let hex = ch.to_digit(16).ok_or_else(err)?;
            chans[i] = (hex as u8) * 17;
        }
        return Ok(Color::new(chans[0], chans[1], chans[2]));
    }
    if digits.len() == 6 {
        // Pairs of hex digits; reject non-hex explicitly.
        let mut chans = [0u8; 3];
        for (i, pair) in digits.as_bytes().chunks_exact(2).enumerate() {
            if !pair.iter().all(|b| b.is_ascii_hexdigit()) {
                return Err(err());
            }
            let text = std::str::from_utf8(pair).map_err(|_| err())?;
            // Validated length 6 → exactly three pairs; get_mut avoids an index edge.
            *chans.get_mut(i).ok_or_else(err)? = pairs_to_byte(text)?;
        }
        return Ok(Color::new(chans[0], chans[1], chans[2]));
    }
    Err(err())
}

/// A `rgb(...)` body: `digits.approx?` — here strictly decimal integers with
/// optional surrounding whitespace, separated by commas or spaces, closed by
/// an optional final `)` which must be balanced if present.
fn parse_rgb_fn(body_after: &str, whole: &str) -> Result<Color, ColorParseError> {
    let err = || ColorParseError {
        input: whole.to_string(),
    };
    let (body, close_ok) = match body_after.trim().strip_suffix(')') {
        Some(b) => (b, true),
        None => (body_after.trim(), false),
    };
    // A `)` must be present somewhere, else "rgb(" with no close is a typo form like "rgb 1,2,3" — reject both.
    if !close_ok && !whole.contains(')') {
        return Err(err());
    }
    if body.is_empty() {
        return Err(err());
    }
    let channel_cap = |s: &str, count: usize| -> Result<u8, ColorParseError> {
        if count > 3 {
            return Err(err());
        }
        s.trim().parse::<u8>().map_err(|_| ColorParseError {
            input: whole.to_string(),
        })
    };
    if body.contains(',') {
        let mut count = 0;
        let mut chans = [0u8; 3];
        for part in body.split(',') {
            let v = channel_cap(part, count)?;
            if count < 3 {
                chans[count] = v;
            }
            count += 1;
        }
        if count != 3 {
            return Err(err());
        }
        return Ok(Color::new(chans[0], chans[1], chans[2]));
    }
    // Space-separated: split once into at most 4 parts; extra = error.
    let mut parts = body.split_whitespace();
    let mut chans = [0u8; 3];
    for slot in &mut chans {
        let p = parts.next().ok_or_else(err)?;
        if p.bytes().any(|b| !b.is_ascii_digit()) {
            return Err(err());
        }
        *slot = p.parse::<u8>().map_err(|_| err())?;
    }
    if parts.next().is_some() {
        return Err(err());
    }
    Ok(Color::new(chans[0], chans[1], chans[2]))
}

/// Two ASCII hex digits → byte. The caller validates the slice length.
fn pairs_to_byte(pair: &str) -> Result<u8, ColorParseError> {
    let mut digits = pair.chars();
    let mut value = 0u8;
    for _ in 0..2 {
        let ch = digits.next().ok_or(ColorParseError {
            input: pair.to_string(),
        })?;
        value = (value << 4) | hex_digit(ch)?;
    }
    Ok(value)
}

fn hex_digit(c: char) -> Result<u8, ColorParseError> {
    c.to_digit(16).map(|d| d as u8).ok_or(ColorParseError {
        input: c.to_string(),
    })
}

impl std::str::FromStr for Color {
    type Err = ColorParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_color(s)
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_css())
    }
}

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_css())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        parse_color(&s).map_err(|_| serde::de::Error::custom(s))
    }
}

#[cfg(test)]
mod tests {
    // Test-only: helpers may fail loudly like tests do (see clippy.toml — the
    // lint needs the attribute escape because helpers are not #[test]).
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::ok_expect)]

    use super::*;

    fn c(input: &str) -> Color {
        parse_color(input).ok().expect("test input should parse")
    }

    #[test]
    fn parses_hex_forms() {
        assert_eq!(c("#e8573f"), Color::new(0xe8, 0x57, 0x3f));
        assert_eq!(c("#E8573F"), Color::new(0xe8, 0x57, 0x3f));
        assert_eq!(c(" #000000 "), Color::new(0, 0, 0));
        assert_eq!(c("#fff"), Color::new(255, 255, 255));
        assert_eq!(c("#000"), Color::new(0, 0, 0));
        assert_eq!(c("#abc"), Color::new(0xaa, 0xbb, 0xcc));
    }

    #[test]
    fn parses_css_keywords() {
        assert_eq!(c("black"), Color::new(0, 0, 0));
        assert_eq!(c("WHITE"), Color::new(255, 255, 255));
        assert_eq!(c("Silver"), Color::new(192, 192, 192));
        assert_eq!(c("cyan"), Color::new(0, 255, 255));
        // css-color-4 keeps both spellings for the two aliases.
        assert_eq!(c("magenta"), c("fuchsia"));
        assert_eq!(c("gray"), c("grey"));
    }

    #[test]
    fn parses_rgb_function() {
        assert_eq!(c("rgb(232, 87, 63)"), Color::new(232, 87, 63));
        assert_eq!(c("rgb(232,87,63)"), Color::new(232, 87, 63));
        assert_eq!(c("rgb(232 87 63)"), Color::new(232, 87, 63));
        // 0–255 only; out-of-range integers are a parse error, not a clamp.
        assert!(parse_color("rgb(256, 0, 0)").is_err());
        assert!(parse_color("rgb(-1, 0, 0)").is_err());
        assert!(parse_color("rgb(1 2)").is_err());
        assert!(parse_color("rgb(1, 2, 3, 4)").is_err());
    }

    #[test]
    fn rejects_incomplete_and_unsupported_forms() {
        assert!(parse_color("").is_err());
        assert!(parse_color("#").is_err());
        assert!(parse_color("#ff").is_err());
        assert!(parse_color("#fffff").is_err());
        assert!(parse_color("#fffffff").is_err());
        assert!(parse_color("transparent").is_err());
        assert!(parse_color("rgba(1, 2, 3, 0.5)").is_err());
        assert!(parse_color("hsl(120, 50%, 50%)").is_err());
        assert!(parse_color("rgb(50%, 0%, 0%)").is_err());
        assert!(parse_color("rgb(1, 2").is_err());
        // Nothing from a too-long string is parsed.
        assert!(parse_color(&"k".repeat(65)).is_err());
    }

    #[test]
    fn resolves_fromstr_and_display() {
        let color: Color = "#3f88e8".parse().ok().expect("hex parses");
        assert_eq!(color.to_string(), "#3f88e8");
        // Display is canonical lowercase six-digit.
        assert_eq!(c("#ABC").to_string(), "#aabbcc");
    }

    #[test]
    fn luminance_is_zero_to_one() {
        assert_eq!(c("#000000").luminance(), 0.0);
        assert_eq!(c("#ffffff").luminance(), 1.0);
        let mid = c("#7f7f7f").luminance();
        assert!((0.2..0.7).contains(&mid), "middle gray ≈ {mid}");
        // Refined weighting only shifts one step, never flips extremes.
        assert!(c("#e8573f").luminance() < c("#ffffff").luminance());
    }

    #[test]
    fn blend_interpolates_and_clamps() {
        let black = c("#000000");
        let white = c("#ffffff");
        assert_eq!(black.blend(white, 0.0), black);
        assert_eq!(black.blend(white, 1.0), white);
        assert_eq!(black.blend(white, 0.5), c("#808080"));
        // Out-of-range and non-finite t are clamped, never wrap.
        assert_eq!(black.blend(white, 5.0), white);
        assert_eq!(black.blend(white, f32::NAN), black);
    }

    #[test]
    fn serializes_as_hex_string() {
        // Round-trip through the JSON form the document will one day use.
        let json =
            serde_json::to_string(&c("#e8573f")).expect("color has no serializer failure path");
        assert_eq!(json, "\"#e8573f\"");
        let back: Color = serde_json::from_str(&json).expect("hex string deserializes");
        assert_eq!(back, c("#e8573f"));
        assert!(serde_json::from_str::<Color>("\"nope\"").is_err());
    }
}
