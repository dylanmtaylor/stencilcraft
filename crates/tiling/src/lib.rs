//! Tiling: split an oversized artboard into physical sheets.
//!
//! A grid of tiles covers the board; each tile becomes its own document holding
//! every shape intersecting it, translated so the tile's origin is its own sheet
//! origin. Overlap (`overlap_mm`) keeps tiles sharing edges joined, so a shape
//! crossing a tile boundary appears on both sheets and the two pieces meet on the
//! machine. Later milestones replace overlap with true clipping plus alignment
//! marks (`docs/gaps.md` 3).

use kurbo::Rect;
use stencilcraft_doc::Document;
use stencilcraft_geom::bounds_of_shape;

#[derive(Debug, thiserror::Error)]
pub enum TilingError {
    #[error("tiling: sheet size {w}×{h} mm is not positive")]
    BadSheet { w: f64, h: f64 },
    #[error("tiling: overlap {0} mm must be ≥ 0 and smaller than the sheet")]
    BadOverlap(f64),
    #[error("tiling: {0}")]
    Model(String),
}

/// One physical sheet of a tiling.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    /// Tile position in columns/rows, counted from the bottom-left of the board.
    pub col: usize,
    pub row: usize,
    /// The tile's rectangle in board coordinates, millimetres.
    pub rect: Rect,
}

/// Compute the tiling of a `board_w` × `board_h` millimetre board onto sheets
/// `sheet_w` × `sheet_h` millimetres with `overlap_mm` shared between neighbours.
pub fn layout(
    board_w: f64,
    board_h: f64,
    sheet_w: f64,
    sheet_h: f64,
    overlap_mm: f64,
) -> Result<Vec<Tile>, TilingError> {
    if !board_w.is_finite()
        || !board_h.is_finite()
        || !sheet_w.is_finite()
        || !sheet_h.is_finite()
        || !overlap_mm.is_finite()
    {
        return Err(TilingError::Model(
            "tiling: dimensions must all be finite".to_string(),
        ));
    }
    if sheet_w <= 0.0 || sheet_h <= 0.0 {
        return Err(TilingError::BadSheet {
            w: sheet_w,
            h: sheet_h,
        });
    }
    if overlap_mm < 0.0 || overlap_mm >= sheet_w.min(sheet_h) {
        return Err(TilingError::BadOverlap(overlap_mm));
    }
    // Net advance per tile: the sheet minus the strip it shares with its left
    // (resp. bottom) neighbour.
    let adv_x = sheet_w - overlap_mm;
    let adv_y = sheet_h - overlap_mm;
    let cols = ((board_w / adv_x).ceil() as usize).max(1);
    let rows = ((board_h / adv_y).ceil() as usize).max(1);
    let mut tiles = Vec::with_capacity(cols * rows);
    for row in 0..rows {
        for col in 0..cols {
            let x0 = col as f64 * adv_x;
            let y0 = row as f64 * adv_y;
            // Right/top tiles may be narrower than a full sheet: clamp against the
            // board so nothing past the sheet lands on the machine.
            let x1 = (x0 + sheet_w).min(board_w);
            let y1 = (y0 + sheet_h).min(board_h);
            tiles.push(Tile {
                col,
                row,
                rect: Rect::new(x0, y0, x1, y1),
            });
        }
    }
    Ok(tiles)
}

/// Do the two rectangles share interior area? A shape partly covering a tile
/// must appear on that tile too, so the test is strict overlap of open boxes.
fn rects_touch(a: kurbo::Rect, b: kurbo::Rect) -> bool {
    a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1
}

/// Apply an affine transform to a path element-wise: the element grammar stays
/// explicit, and this is the one place a future transform special-case belongs.
fn transform_path(path: &kurbo::BezPath, affine: kurbo::Affine) -> kurbo::BezPath {
    let mut out = kurbo::BezPath::new();
    for elem in path.elements() {
        match *elem {
            kurbo::PathEl::MoveTo(p) => out.move_to(affine * p),
            kurbo::PathEl::LineTo(p) => out.line_to(affine * p),
            kurbo::PathEl::QuadTo(c, p) => out.quad_to(affine * c, affine * p),
            kurbo::PathEl::CurveTo(c1, c2, p) => out.curve_to(affine * c1, affine * c2, affine * p),
            kurbo::PathEl::ClosePath => out.close_path(),
        }
    }
    out
}

/// Tile a whole document: one output document per physical sheet. Each output
/// keeps the source kerf settings and contains every visible shape whose bounds
/// intersect the tile, translated so the tile origin is the sheet origin.
/// Hidden layers are skipped. Output order is row-major, matching `layout`.
pub fn tile_document(
    doc: &Document,
    sheet_w: f64,
    sheet_h: f64,
    overlap_mm: f64,
) -> Result<Vec<Document>, TilingError> {
    if doc.shape_count() == 0 {
        return Err(TilingError::Model(
            "tiling: nothing to tile, document has no shapes".to_string(),
        ));
    }
    let tiles = layout(
        doc.board.width_mm,
        doc.board.height_mm,
        sheet_w,
        sheet_h,
        overlap_mm,
    )?;
    let mut out = Vec::with_capacity(tiles.len());
    for tile in tiles {
        let mut piece = Document {
            board: stencilcraft_doc::Board {
                width_mm: tile.rect.width(),
                height_mm: tile.rect.height(),
            },
            kerf: doc.kerf.clone(),
            layers: Vec::new(),
            next_shape_id: doc.next_shape_id,
        };
        for layer in &doc.layers {
            if !layer.visible {
                continue;
            }
            let mut piece_layer = stencilcraft_doc::Layer::new(layer.name.clone(), layer.kind);
            for shape in &layer.shapes {
                let Some(b) = bounds_of_shape(&shape.path) else {
                    continue;
                };
                if !rects_touch(b, tile.rect) {
                    continue;
                }
                let mut translated = shape.clone();
                translated.path = transform_path(
                    &shape.path,
                    kurbo::Affine::translate((-tile.rect.x0, -tile.rect.y0)),
                );
                piece_layer.shapes.push(translated);
            }
            if !piece_layer.shapes.is_empty() {
                piece.layers.push(piece_layer);
            }
        }
        if piece.layers.is_empty() {
            return Err(TilingError::Model(format!(
                "tiling: sheet {}×{} holds no shapes",
                tile.col, tile.row
            )));
        }
        out.push(piece);
    }
    Ok(out)
}

/// Convenience: shape count per tile in row-major order (used by the CLI's tile
/// summary and tests).
pub fn tile_shape_counts(
    doc: &Document,
    sheet_w: f64,
    sheet_h: f64,
    overlap_mm: f64,
) -> Result<Vec<usize>, TilingError> {
    let tiles = layout(
        doc.board.width_mm,
        doc.board.height_mm,
        sheet_w,
        sheet_h,
        overlap_mm,
    )?;
    Ok(tiles
        .iter()
        .map(|tile| {
            doc.layers
                .iter()
                .filter(|l| l.visible)
                .map(|l| {
                    l.shapes
                        .iter()
                        .filter(|s| {
                            bounds_of_shape(&s.path).is_some_and(|b| rects_touch(b, tile.rect))
                        })
                        .count()
                })
                .sum()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_doc::{Board, CutStyle, Layer};
    use stencilcraft_geom::Point;

    fn cross_doc(board_w: f64, board_h: f64) -> Document {
        let mut doc = Document {
            board: Board {
                width_mm: board_w,
                height_mm: board_h,
            },
            kerf: stencilcraft_doc::KerfSettings::default(),
            layers: vec![Layer::new("cut", CutStyle::Cut)],
            next_shape_id: 1,
        };
        // A plus-shaped set of lines along the diagonals of the board: these
        // intersect every corner tile of the grid.
        let mut p = kurbo::BezPath::new();
        p.move_to(Point::new(1.0, 1.0));
        p.line_to(Point::new(board_w - 1.0, board_h - 1.0));
        doc.push_shape(0, p, CutStyle::Cut).unwrap();
        doc
    }

    #[test]
    fn layout_counts_tiles() {
        let tiles = layout(500.0, 400.0, 300.0, 300.0, 10.0).unwrap();
        let cols = tiles.iter().map(|t| t.col).max().unwrap() + 1;
        let rows = tiles.iter().map(|t| t.row).max().unwrap() + 1;
        assert_eq!(tiles.len(), cols * rows);
        assert_eq!(cols, 2, "500 mm on 290 mm advances needs 2 columns");
        assert_eq!(rows, 2, "400 mm on 290 mm advances needs 2 rows");
    }

    #[test]
    fn no_overlap_when_sheet_covers_board() {
        let tiles = layout(200.0, 200.0, 300.0, 300.0, 5.0).unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].rect, Rect::new(0.0, 0.0, 200.0, 200.0));
    }

    #[test]
    fn last_tile_clamps_to_board() {
        let tiles = layout(500.0, 400.0, 300.0, 300.0, 10.0).unwrap();
        let max_x = tiles.iter().map(|t| t.rect.x1).fold(0.0_f64, f64::max);
        let max_y = tiles.iter().map(|t| t.rect.y1).fold(0.0_f64, f64::max);
        assert_eq!(max_x, 500.0);
        assert_eq!(max_y, 400.0);
    }

    #[test]
    fn bad_sheet_rejected() {
        assert!(matches!(
            layout(100.0, 100.0, 0.0, 300.0, 1.0),
            Err(TilingError::BadSheet { .. })
        ));
        assert!(matches!(
            layout(100.0, 100.0, 300.0, 300.0, 500.0),
            Err(TilingError::BadOverlap(_))
        ));
    }

    #[test]
    fn every_tile_gets_the_traversing_shape() {
        let doc = cross_doc(500.0, 400.0);
        let counts = tile_shape_counts(&doc, 300.0, 300.0, 10.0).unwrap();
        assert_eq!(counts.len(), 4);
        assert!(
            counts.iter().all(|c| *c == 1),
            "diagonal crosses every tile: {counts:?}"
        );
        let pieces = tile_document(&doc, 300.0, 300.0, 10.0).unwrap();
        assert_eq!(pieces.len(), 4);
        for piece in &pieces {
            assert_eq!(piece.shape_count(), 1);
            assert_eq!(piece.layers[0].kind, CutStyle::Cut);
            // Kerf settings travel with each sheet.
            assert_eq!(piece.kerf, doc.kerf);
        }
    }

    #[test]
    fn empty_document_is_an_error() {
        let doc = Document::default();
        assert!(matches!(
            tile_document(&doc, 300.0, 300.0, 5.0),
            Err(TilingError::Model(_))
        ));
    }

    #[test]
    fn tile_origin_translates_shapes_to_sheet_space() {
        let doc = cross_doc(500.0, 400.0);
        let pieces = tile_document(&doc, 300.0, 300.0, 10.0).unwrap();
        // The top-right tile starts at (290, 290); its shape must sit at
        // roughly (−289, −289) in sheet space.
        let top_right_shape = &pieces[3].layers[0].shapes[0];
        let b = bounds_of_shape(&top_right_shape.path).unwrap();
        assert!(b.x0.abs() - 289.0 < 1.0, "shape origin: {b:?}");
    }
}
