//! Registration marks for tiled sheets.
//!
//! Each sheet gets the same three corner brackets the print-and-cut sheet uses: a
//! square-ish corner at the top-left plus asymmetric brackets at the top-right and
//! bottom-left, which fix position and orientation of the sheet. Because sheets are
//! overlapping, a sheet that has a neighbour also gets guide ticks at the edge where
//! that neighbour's sheet ends, so the overlap strip can be lined up by eye.
//!
//! Marks are engrave shapes in their own layer: nothing is cut out, but a plotter or
//! printer can score/print them, and they travel with the sheet's SVG.

use kurbo::{BezPath, Point};
use stencilcraft_doc::{CutStyle, Document, Layer};

use crate::{Tile, TilingError};

/// Name of the layer carrying the marks on every sheet.
pub const MARKS_LAYER: &str = "registration";

/// Inset of a bracket's outer corner from the sheet edge, millimetres.
pub const MARK_MARGIN_MM: f64 = 4.0;
/// Length of a bracket arm, millimetres.
pub const MARK_ARM_MM: f64 = 8.0;
/// Thickness of a bracket arm, millimetres.
pub const MARK_STROKE_MM: f64 = 0.6;
/// How far a neighbour-edge guide tick reaches in from the sheet edge, millimetres.
pub const GUIDE_LENGTH_MM: f64 = 5.0;
/// Thickness of a guide tick, millimetres.
pub const GUIDE_STROKE_MM: f64 = 0.4;

/// Add the mark layer to one sheet. `tile` is the sheet's position in the tiling and
/// `overlap_mm` the shared strip, which decides where the guide ticks go.
///
/// Sheets too small to hold the brackets are left unmarked rather than stamped with
/// marks that collide with each other or fall off the material.
pub fn add_marks(piece: &mut Document, tile: &Tile, overlap_mm: f64) -> Result<(), TilingError> {
    let w = piece.board.width_mm;
    let h = piece.board.height_mm;
    if !(w.is_finite() && h.is_finite() && overlap_mm.is_finite()) || w <= 0.0 || h <= 0.0 {
        return Ok(());
    }
    let span = 2.0 * MARK_MARGIN_MM + MARK_ARM_MM;
    if w < span || h < span {
        return Ok(());
    }
    let mut paths = vec![
        bracket(Bracket::TopLeft, w, h),
        bracket(Bracket::TopRight, w, h),
        bracket(Bracket::BottomLeft, w, h),
    ];
    // A guide at `overlap_mm` marks the edge of the neighbouring sheet that this one
    // overlaps: its left neighbour ends there (column guides) and its bottom
    // neighbour ends there (row guides).
    if overlap_mm > 0.0 {
        if tile.col > 0 && overlap_mm + GUIDE_STROKE_MM < w {
            paths.extend(edge_ticks_vertical(overlap_mm, h));
        }
        if tile.row > 0 && overlap_mm + GUIDE_STROKE_MM < h {
            paths.extend(edge_ticks_horizontal(overlap_mm, w));
        }
    }
    let layer_index = piece.layers.len();
    piece
        .layers
        .push(Layer::new(MARKS_LAYER, CutStyle::Engrave));
    for path in paths {
        piece
            .push_shape(layer_index, path, CutStyle::Engrave)
            .map_err(|e| TilingError::Model(e.to_string()))?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Bracket {
    TopLeft,
    TopRight,
    BottomLeft,
}

/// An L-shaped bracket at one corner, arms lying along the two adjacent sheet edges.
fn bracket(corner: Bracket, w: f64, h: f64) -> BezPath {
    let m = MARK_MARGIN_MM;
    let a = MARK_ARM_MM;
    let t = MARK_STROKE_MM;
    // Walk the outside of the L, then back along the inside: one closed polygon.
    let points = match corner {
        Bracket::TopLeft => vec![
            Point::new(m, h - m),
            Point::new(m + a, h - m),
            Point::new(m + a, h - m - t),
            Point::new(m + t, h - m - t),
            Point::new(m + t, h - m - a),
            Point::new(m, h - m - a),
        ],
        Bracket::TopRight => vec![
            Point::new(w - m, h - m),
            Point::new(w - m - a, h - m),
            Point::new(w - m - a, h - m - t),
            Point::new(w - m - t, h - m - t),
            Point::new(w - m - t, h - m - a),
            Point::new(w - m, h - m - a),
        ],
        Bracket::BottomLeft => vec![
            Point::new(m, m),
            Point::new(m + a, m),
            Point::new(m + a, m + t),
            Point::new(m + t, m + t),
            Point::new(m + t, m + a),
            Point::new(m, m + a),
        ],
    };
    polygon(&points)
}

/// Two vertical ticks at `x`, one at the bottom edge and one at the top edge, so the
/// guide survives trimming or loss of either end.
fn edge_ticks_vertical(x: f64, h: f64) -> Vec<BezPath> {
    let half = GUIDE_STROKE_MM * 0.5;
    vec![
        rect_path(x - half, 0.0, x + half, GUIDE_LENGTH_MM),
        rect_path(x - half, h - GUIDE_LENGTH_MM, x + half, h),
    ]
}

/// Two horizontal ticks at `y`, at the left and right edges.
fn edge_ticks_horizontal(y: f64, w: f64) -> Vec<BezPath> {
    let half = GUIDE_STROKE_MM * 0.5;
    vec![
        rect_path(0.0, y - half, GUIDE_LENGTH_MM, y + half),
        rect_path(w - GUIDE_LENGTH_MM, y - half, w, y + half),
    ]
}

fn rect_path(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
    polygon(&[
        Point::new(x0, y0),
        Point::new(x1, y0),
        Point::new(x1, y1),
        Point::new(x0, y1),
    ])
}

fn polygon(points: &[Point]) -> BezPath {
    let mut path = BezPath::new();
    let Some((first, rest)) = points.split_first() else {
        return path;
    };
    path.move_to(*first);
    for p in rest {
        path.line_to(*p);
    }
    path.close_path();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_doc::{Board, KerfSettings};

    fn sheet(w: f64, h: f64) -> Document {
        Document {
            board: Board {
                width_mm: w,
                height_mm: h,
            },
            kerf: KerfSettings::default(),
            layers: Vec::new(),
            next_shape_id: 1,
        }
    }

    fn tile(col: usize, row: usize) -> Tile {
        Tile {
            col,
            row,
            rect: kurbo::Rect::new(0.0, 0.0, 100.0, 100.0),
        }
    }

    #[test]
    fn first_sheet_gets_three_brackets_inside_its_edges() {
        let mut piece = sheet(200.0, 150.0);
        add_marks(&mut piece, &tile(0, 0), 10.0).unwrap();
        let marks = &piece.layers[0];
        assert_eq!(marks.name, MARKS_LAYER);
        assert_eq!(marks.kind, CutStyle::Engrave);
        // Three brackets, no guide ticks on the first sheet.
        assert_eq!(marks.shapes.len(), 3);
        for shape in &marks.shapes {
            let b = shape.bounds().unwrap();
            assert!(b.x0 >= MARK_MARGIN_MM - 1e-9, "{b:?}");
            assert!(b.y0 >= MARK_MARGIN_MM - 1e-9, "{b:?}");
            assert!(b.x1 <= 200.0 - MARK_MARGIN_MM + 1e-9, "{b:?}");
            assert!(b.y1 <= 150.0 - MARK_MARGIN_MM + 1e-9, "{b:?}");
        }
    }

    #[test]
    fn later_sheets_get_guides_where_neighbours_end() {
        let mut piece = sheet(200.0, 150.0);
        add_marks(&mut piece, &tile(1, 1), 10.0).unwrap();
        let marks = &piece.layers[0];
        // Three brackets plus four guide ticks (two per shared edge).
        assert_eq!(marks.shapes.len(), 7);
        let near_x = marks
            .shapes
            .iter()
            .any(|s| s.bounds().is_some_and(|b| (b.x0 - 9.8).abs() < 0.01));
        assert!(near_x, "a vertical guide sits at the overlap distance");
        let near_y = marks
            .shapes
            .iter()
            .any(|s| s.bounds().is_some_and(|b| (b.y0 - 9.8).abs() < 0.01));
        assert!(near_y, "a horizontal guide sits at the overlap distance");
    }

    #[test]
    fn tiny_sheets_are_left_unmarked() {
        let mut piece = sheet(12.0, 12.0);
        add_marks(&mut piece, &tile(0, 0), 2.0).unwrap();
        assert!(piece.layers.is_empty(), "no marks fit on a 12 mm sheet");
    }
}
