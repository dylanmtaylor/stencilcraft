//! StencilCraft Single Line: an original single-stroke lettering for pens and engraving,
//! drawn in code for this project (MIT OR Apache-2.0 like the rest of the source; no font
//! file, no third-party glyph data).
//!
//! Glyphs are written on a grid of 20 units to the em: baseline 0, x-height 9, cap and
//! ascender height 14, descender -5. A glyph is a list of strokes in a tiny path language:
//! `M x y` moves, `L x y` draws a line, `E cx cy rx ry start sweep` draws an elliptical arc
//! (degrees, counter-clockwise positive), continuing the stroke when it starts where the pen
//! is. Each glyph's advance is its first number.

use kurbo::{Arc, BezPath, Point, Vec2};

/// Units per em of the glyph grid.
pub(super) const UNITS_PER_EM: f64 = 20.0;

/// Advance of a space, in grid units.
const SPACE: f64 = 6.0;

/// The glyph of `c` (its advance and strokes in grid units), `?` for characters it lacks.
/// Spaces have an advance and no strokes.
pub(super) fn glyph(c: char) -> (f64, BezPath) {
    if c == ' ' || c == '\u{a0}' {
        return (SPACE, BezPath::new());
    }
    if c == '\t' {
        return (SPACE * 4.0, BezPath::new());
    }
    let source = GLYPHS
        .iter()
        .find(|(g, _)| *g == c)
        .or_else(|| GLYPHS.iter().find(|(g, _)| *g == '?'))
        .map_or("6", |(_, s)| *s);
    parse(source).unwrap_or((SPACE, BezPath::new()))
}

/// Parses one glyph's source: its advance, then its strokes. `None` on malformed source.
fn parse(source: &str) -> Option<(f64, BezPath)> {
    // Ops are written glued to their first number ("M1 0 L6 14"), so split a leading letter off.
    let mut tokens = source
        .split_whitespace()
        .flat_map(|t| match t.char_indices().nth(1) {
            Some((i, _)) if t.starts_with(char::is_alphabetic) => [Some(&t[..i]), Some(&t[i..])],
            _ => [Some(t), None],
        })
        .flatten();
    let advance: f64 = tokens.next()?.parse().ok()?;
    let mut path = BezPath::new();
    let mut pen: Option<Point> = None;
    let number = |tokens: &mut dyn Iterator<Item = &str>| -> Option<f64> {
        tokens.next()?.parse::<f64>().ok().filter(|v| v.is_finite())
    };
    while let Some(op) = tokens.next() {
        match op {
            "M" | "L" => {
                let p = Point::new(number(&mut tokens)?, number(&mut tokens)?);
                if op == "M" || pen.is_none() {
                    path.move_to(p);
                } else {
                    path.line_to(p);
                }
                pen = Some(p);
            }
            "E" => {
                let centre = Point::new(number(&mut tokens)?, number(&mut tokens)?);
                let radii = Vec2::new(number(&mut tokens)?, number(&mut tokens)?);
                let start = number(&mut tokens)?.to_radians();
                let sweep = number(&mut tokens)?.to_radians();
                let from = centre + Vec2::new(radii.x * start.cos(), radii.y * start.sin());
                if pen.is_none_or(|p| p.distance(from) > 1e-6) {
                    path.move_to(from);
                }
                let arc = Arc::new(centre, radii, start, sweep, 0.0);
                for el in arc.append_iter(0.01) {
                    path.push(el);
                }
                pen = Some(
                    centre
                        + Vec2::new(
                            radii.x * (start + sweep).cos(),
                            radii.y * (start + sweep).sin(),
                        ),
                );
            }
            _ => return None,
        }
    }
    Some((advance, path))
}

/// The characters drawn, each with its source (advance first).
const GLYPHS: &[(char, &str)] = &[
    ('A', "12 M1 0 L6 14 L11 0 M3 5 L9 5"),
    (
        'B',
        "11 M1 0 L1 14 L6 14 E6 10.75 3.25 3.25 90 -180 M1 7.5 L6.5 7.5 E6.5 3.75 3.75 3.75 90 -180 L1 0",
    ),
    ('C', "13 E7 7 6 7 50 260"),
    ('D', "12.5 M1 0 L1 14 L5 14 E5 7 6 7 90 -180 L1 0"),
    ('E', "10 M9 14 L1 14 L1 0 L9 0 M1 7 L7 7"),
    ('F', "10 M9 14 L1 14 L1 0 M1 7 L7 7"),
    ('G', "14 E7 7 6 7 45 315 L8 7"),
    ('H', "12 M1 0 L1 14 M11 0 L11 14 M1 7 L11 7"),
    ('I', "4 M2 0 L2 14"),
    ('J', "10 M8 14 L8 4 E4.5 4 3.5 4 0 -180"),
    ('K', "11 M1 0 L1 14 M10 14 L1 5 M4 8 L10 0"),
    ('L', "9 M1 14 L1 0 L8 0"),
    ('M', "14 M1 0 L1 14 L7 3 L13 14 L13 0"),
    ('N', "12 M1 0 L1 14 L11 0 L11 14"),
    ('O', "14 E7 7 6 7 0 360"),
    ('P', "11 M1 0 L1 14 L6 14 E6 10.5 3.5 3.5 90 -180 L1 7"),
    ('Q', "14 E7 7 6 7 0 360 M8 3 L13 -1"),
    (
        'R',
        "11 M1 0 L1 14 L6 14 E6 10.5 3.5 3.5 90 -180 L1 7 M5 7 L10 0",
    ),
    ('S', "11 E5.5 10.5 4 3.5 30 240 E5.5 3.5 4.5 3.5 90 -240"),
    ('T', "12 M1 14 L11 14 M6 14 L6 0"),
    ('U', "12 M1 14 L1 5 E6 5 5 5 180 180 L11 14"),
    ('V', "12 M1 14 L6 0 L11 14"),
    ('W', "16 M1 14 L4.5 0 L8 11 L11.5 0 L15 14"),
    ('X', "11 M1 0 L10 14 M1 14 L10 0"),
    ('Y', "12 M1 14 L6 7 L11 14 M6 7 L6 0"),
    ('Z', "11 M1 14 L10 14 L1 0 L10 0"),
    ('a', "10 E5 4.5 4 4.5 0 360 M9 9 L9 0"),
    ('b', "10 M1 14 L1 0 E5 4.5 4 4.5 180 360"),
    ('c', "9.5 E5 4.5 4 4.5 45 270"),
    ('d', "10 E5 4.5 4 4.5 0 360 M9 14 L9 0"),
    ('e', "10 M1 4.5 L9 4.5 E5 4.5 4 4.5 0 315"),
    (
        'f',
        "7 M2.5 0 L2.5 11.5 E5 11.5 2.5 2.5 180 -150 M0.5 9 L6 9",
    ),
    ('g', "10 E5 4.5 4 4.5 0 360 M9 9 L9 -2 E5 -2 4 3 0 -180"),
    ('h', "10 M1 14 L1 0 M1 5 E5 5 4 4 180 -180 L9 0"),
    ('i', "4 M2 0 L2 9 M2 12 L2 12.6"),
    (
        'j',
        "5.5 M3.5 9 L3.5 -2.5 E1.5 -2.5 2 2.5 0 -180 M3.5 12 L3.5 12.6",
    ),
    ('k', "9 M1 14 L1 0 M8 9 L1 3 M3.5 5 L8 0"),
    ('l', "4 M2 14 L2 0"),
    (
        'm',
        "16 M1 0 L1 9 M1 5.5 E4.5 5.5 3.5 3.5 180 -180 L8 0 M8 5.5 E11.5 5.5 3.5 3.5 180 -180 L15 0",
    ),
    ('n', "10 M1 0 L1 9 M1 5 E5 5 4 4 180 -180 L9 0"),
    ('o', "10 E5 4.5 4 4.5 0 360"),
    ('p', "10 M1 9 L1 -5 E5 4.5 4 4.5 180 360"),
    ('q', "10 E5 4.5 4 4.5 0 360 M9 9 L9 -5"),
    ('r', "7 M1 0 L1 9 M1 5 E5 5 4 4 180 -120"),
    (
        's',
        "9 E4.5 6.75 3.5 2.25 30 240 E4.5 2.25 3.5 2.25 90 -240",
    ),
    ('t', "7 M3 14 L3 2 E5 2 2 2 180 135 M0.5 9 L6.5 9"),
    ('u', "10 M1 9 L1 4 E5 4 4 4 180 180 M9 9 L9 0"),
    ('v', "10 M1 9 L5 0 L9 9"),
    ('w', "14 M1 9 L4 0 L7 7 L10 0 L13 9"),
    ('x', "9 M1 0 L8 9 M1 9 L8 0"),
    ('y', "10 M1 9 L5 0 M9 9 L3 -5"),
    ('z', "9 M1 9 L8 9 L1 0 L8 0"),
    ('0', "11 E5.5 7 4.5 7 0 360"),
    ('1', "11 M3 11 L6 14 L6 0 M3 0 L9 0"),
    ('2', "11 E5.5 9.5 4.5 4.5 150 -195 L1 0 L10 0"),
    ('3', "11 E5.5 10.5 4 3.5 150 -240 E5.5 3.5 4.5 3.5 90 -270"),
    ('4', "11 M8 0 L8 14 L1 4 L10 4"),
    ('5', "11 M9.5 14 L2 14 L2.05 7.39 E5.5 4.5 4.5 4.5 140 -300"),
    (
        '6',
        "11 E5.5 4.5 4.5 4.5 0 360 M1 4.5 L1 8 E6 8 5 6 180 -120",
    ),
    ('7', "11 M1 14 L10 14 L4 0"),
    ('8', "11 E5.5 10.5 3.75 3.5 0 360 E5.5 3.5 4.5 3.5 0 360"),
    (
        '9',
        "11 E5.5 9.5 4.5 4.5 0 360 M10 9.5 L10 6 E5 6 5 6 0 -120",
    ),
    ('.', "4 M2 0 L2 0.6"),
    (',', "4 M2.5 0.8 L1 -2.5"),
    (':', "4 M2 0 L2 0.6 M2 8 L2 8.6"),
    (';', "4 M2.5 0.8 L1 -2.5 M2.5 8 L2.5 8.6"),
    ('!', "4 M2 14 L2 4 M2 0 L2 0.6"),
    ('?', "10 E5 10 4 4 150 -240 L5 3.5 M5 0 L5 0.6"),
    ('\'', "3 M1.5 14 L1.5 10"),
    ('"', "5 M1.5 14 L1.5 10 M3.5 14 L3.5 10"),
    ('`', "4 M1 14 L3 11"),
    ('-', "8 M1.5 6 L6.5 6"),
    ('_', "10 M0 -2 L10 -2"),
    ('+', "10 M5 2 L5 10 M1 6 L9 6"),
    ('=', "10 M1 4 L9 4 M1 8 L9 8"),
    ('/', "8 M1 -2 L7 15"),
    ('\\', "8 M1 15 L7 -2"),
    ('|', "4 M2 15 L2 -2"),
    ('(', "7 E8 6 6 10 120 120"),
    (')', "7 E-1 6 6 10 60 -120"),
    ('[', "6 M5 15 L2 15 L2 -2 L5 -2"),
    (']', "6 M1 15 L4 15 L4 -2 L1 -2"),
    (
        '{',
        "7 M6 15 L4.5 15 L3.5 14 L3.5 8 L1.5 6.5 L3.5 5 L3.5 -1 L4.5 -2 L6 -2",
    ),
    (
        '}',
        "7 M1 15 L2.5 15 L3.5 14 L3.5 8 L5.5 6.5 L3.5 5 L3.5 -1 L2.5 -2 L1 -2",
    ),
    ('<', "9 M8 11 L1 6 L8 1"),
    ('>', "9 M1 11 L8 6 L1 1"),
    ('^', "9 M1 9 L4.5 14 L8 9"),
    ('~', "12 E3.5 6 2.5 1.5 180 -180 E8.5 6 2.5 1.5 180 180"),
    ('*', "8 M4 14 L4 8 M1.4 12.5 L6.6 9.5 M1.4 9.5 L6.6 12.5"),
    (
        '#',
        "12 M4 0 L5.5 14 M8.5 0 L10 14 M1.5 4.5 L10.5 4.5 M2 9.5 L11 9.5",
    ),
    ('%', "13 E3.5 11 2.5 3 0 360 M11 14 L2 0 E9.5 3 2.5 3 0 360"),
    (
        '&',
        "12 M11 0 L3.08 9.89 E5 11.5 2.5 2.5 220 -260 L1.8 4.3 E5 3.5 3.3 3.5 165 155 L10 5",
    ),
    (
        '@',
        "15 E7 6 2.5 3 0 360 M9.5 9 L9.5 4 E11.5 4 2 2 180 180 E7.5 6 6 7 15 300",
    ),
    (
        '$',
        "11 E5.5 10.5 4 3.5 30 240 E5.5 3.5 4.5 3.5 90 -240 M5.5 15 L5.5 -1",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_parses_and_draws() {
        for (c, source) in GLYPHS {
            let (advance, path) = parse(source).unwrap_or_else(|| panic!("{c:?} malformed"));
            assert!(advance > 0.0, "{c:?}");
            assert!(!path.elements().is_empty(), "{c:?} draws nothing");
            let bounds = kurbo::Shape::bounding_box(&path);
            assert!(
                bounds.x0 >= -1.5 && bounds.x1 <= advance + 1.0,
                "{c:?} {bounds:?}"
            );
            assert!(bounds.y0 >= -5.5 && bounds.y1 <= 16.0, "{c:?} {bounds:?}");
        }
        // Printable ASCII is covered.
        for c in '!'..='~' {
            assert!(GLYPHS.iter().any(|(g, _)| *g == c), "{c:?} missing");
        }
        assert!(glyph(' ').1.elements().is_empty());
        // Unknown characters draw the question mark.
        assert_eq!(glyph('\u{4e00}').1, glyph('?').1);
    }
}
