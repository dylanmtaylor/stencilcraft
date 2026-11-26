//! The Glyphs panel's pictures: the framework's [`GlyphsPanel`] over the fonts text uses.
//! Each cell is the engine's outline of a character, filled (or stroked, for the single-line
//! family) into a white image whose alpha is the glyph's coverage.
//!
//! [`GlyphsPanel`]: craft_font_manager::glyphs::GlyphsPanel

use craft_font_manager::glyphs::{CATEGORIES, GlyphCell, GlyphSource};
use eframe::egui::ColorImage;
use stencilcraft_engine::text::{GlyphPreview, glyph_chars, glyph_preview};

/// The largest cell picture drawn, in pixels a side.
const MAX_PX: u32 = 512;

/// The fonts text uses, as glyph cells.
pub struct AppGlyphs;

/// Whether `c` belongs in the panel's `category` (one of [`CATEGORIES`]).
fn in_category(c: char, category: &str) -> bool {
    let code = u32::from(c);
    match category {
        "Punctuation" => {
            (c.is_ascii_punctuation() && !"$+<=>^`|~".contains(c))
                || "¡¿«»‹›·".contains(c)
                || (0x2010..=0x2027).contains(&code)
        }
        "Numbers" => c.is_numeric(),
        "Currency" => "$¢£¤¥".contains(c) || (0x20A0..=0x20CF).contains(&code),
        "Symbols" => {
            "©®°§¶†‡•".contains(c)
                || ((0x2100..=0x2BFF).contains(&code) && !(0x2190..=0x22FF).contains(&code))
        }
        "Math Symbols" => "+−×÷±<=>¬".contains(c) || (0x2200..=0x22FF).contains(&code),
        "Arrows" => (0x2190..=0x21FF).contains(&code),
        "Latin" => c.is_alphabetic() && code < 0x250,
        "Greek" => (0x370..=0x3FF).contains(&code),
        "Cyrillic" => (0x400..=0x52F).contains(&code),
        "Letterlike Symbols" => (0x2100..=0x214F).contains(&code),
        "Superscripts and Subscripts" => "¹²³".contains(c) || (0x2070..=0x209F).contains(&code),
        "Ornaments" => (0x2766..=0x2767).contains(&code) || code == 0x2042,
        _ => false,
    }
}

/// `glyph` drawn into a `px`-pixel square: the em is 0.8 of the side, the baseline at 0.78,
/// centred on its advance. White, with the coverage in alpha.
fn rasterize(glyph: &GlyphPreview, px: u32) -> Option<ColorImage> {
    let px = px.clamp(1, MAX_PX);
    let side = px as f32;
    let em = f64::from(side) * 0.8;
    let dx = (f64::from(side) - glyph.advance * em) / 2.0;
    let baseline = f64::from(side) * 0.78;
    let mut builder = tiny_skia::PathBuilder::new();
    let at = |p: kurbo::Point| ((p.x * em + dx) as f32, (p.y * em + baseline) as f32);
    for el in glyph.outline.elements() {
        match *el {
            kurbo::PathEl::MoveTo(p) => {
                let (x, y) = at(p);
                builder.move_to(x, y);
            }
            kurbo::PathEl::LineTo(p) => {
                let (x, y) = at(p);
                builder.line_to(x, y);
            }
            kurbo::PathEl::QuadTo(a, p) => {
                let ((ax, ay), (x, y)) = (at(a), at(p));
                builder.quad_to(ax, ay, x, y);
            }
            kurbo::PathEl::CurveTo(a, b, p) => {
                let ((ax, ay), (bx, by), (x, y)) = (at(a), at(b), at(p));
                builder.cubic_to(ax, ay, bx, by, x, y);
            }
            kurbo::PathEl::ClosePath => builder.close(),
        }
    }
    let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
    if let Some(path) = builder.finish() {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(tiny_skia::Color::WHITE);
        paint.anti_alias = true;
        if glyph.stroked {
            let stroke = tiny_skia::Stroke {
                width: (side / 24.0).max(1.0),
                line_cap: tiny_skia::LineCap::Round,
                line_join: tiny_skia::LineJoin::Round,
                ..tiny_skia::Stroke::default()
            };
            pixmap.stroke_path(
                &path,
                &paint,
                &stroke,
                tiny_skia::Transform::identity(),
                None,
            );
        } else {
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    // Premultiplied white: every channel is the coverage.
    Some(ColorImage::from_rgba_premultiplied(
        [px as usize, px as usize],
        pixmap.data(),
    ))
}

impl GlyphSource for AppGlyphs {
    fn cells(&self, family: &str, style: &str, category: &str) -> Vec<GlyphCell> {
        let whole = category == CATEGORIES.first().copied().unwrap_or("Entire Font");
        glyph_chars(family, style)
            .into_iter()
            .filter(|c| whole || in_category(*c, category))
            .map(GlyphCell::Char)
            .collect()
    }

    fn glyph(&self, family: &str, style: &str, cell: GlyphCell, px: u32) -> Option<ColorImage> {
        let GlyphCell::Char(c) = cell else {
            return None;
        };
        rasterize(&glyph_preview(family, style, c)?, px)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_engine::text::SINGLE_LINE_FAMILY;

    #[test]
    fn single_line_cells_draw_and_filter_by_category() {
        let all = AppGlyphs.cells(SINGLE_LINE_FAMILY, "Regular", "Entire Font");
        assert!(all.contains(&GlyphCell::Char('A')));
        let numbers = AppGlyphs.cells(SINGLE_LINE_FAMILY, "Regular", "Numbers");
        assert!(!numbers.is_empty());
        assert!(
            numbers
                .iter()
                .all(|c| matches!(c, GlyphCell::Char(d) if d.is_numeric()))
        );
        let image = AppGlyphs
            .glyph(SINGLE_LINE_FAMILY, "Regular", GlyphCell::Char('A'), 48)
            .expect("drawn");
        assert_eq!(image.size, [48, 48]);
        assert!(image.pixels.iter().any(|p| p.a() > 0), "ink");
        assert!(image.pixels.iter().any(|p| p.a() == 0), "room round it");
        // A hostile size is capped, a missing glyph is none.
        let big = AppGlyphs.glyph(
            SINGLE_LINE_FAMILY,
            "Regular",
            GlyphCell::Char('A'),
            u32::MAX,
        );
        assert_eq!(big.map(|i| i.size), Some([512, 512]));
        assert!(
            AppGlyphs
                .glyph(SINGLE_LINE_FAMILY, "Regular", GlyphCell::Glyph(3), 48)
                .is_none()
        );
    }
}
