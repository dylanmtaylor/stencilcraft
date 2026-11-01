//! Tiling: split an oversized artboard into physical sheets.
//!
//! A grid of tiles covers the board; each tile becomes its own document holding
//! the part of every shape that lies on that sheet, translated so the tile's origin
//! is its own sheet origin. Shapes are clipped to the sheet (`clip`), so no cut path
//! runs past the material edge. Overlap (`overlap_mm`) keeps neighbouring tiles
//! sharing a strip, so a shape crossing a boundary is cut on both sheets and the
//! pieces meet on the machine. Alignment marks are still to come (`docs/gaps.md` 3).

mod clip;

use kurbo::Rect;
use stencilcraft_doc::Document;
use stencilcraft_geom::bounds_of_shape;

#[derive(Debug, thiserror::Error)]
pub enum TilingError {
    #[error("tiling: sheet size {w}×{h} mm is not positive")]
    BadSheet { w: f64, h: f64 },
    #[error("tiling: overlap {0} mm must be ≥ 0 and smaller than the sheet")]
    BadOverlap(f64),
    #[error(
        "tiling: {count} sheets exceeds the {limit}-sheet limit (use a bigger sheet or a smaller overlap)"
    )]
    TooManyTiles { count: usize, limit: usize },
    #[error("tiling: {0}")]
    Model(String),
}

/// Hard ceiling on the sheets one tiling may produce. A tiny sheet, or an overlap
/// almost as wide as the sheet, would otherwise ask for millions of sheets and run
/// the machine out of memory before the user notices a bad flag.
pub const MAX_TILES: usize = 4096;

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
    let cols = axis_tiles(board_w, sheet_w, adv_x);
    let rows = axis_tiles(board_h, sheet_h, adv_y);
    // `cols`/`rows` saturate at `usize::MAX` for absurd inputs, so the product is
    // checked before anything is allocated.
    let count = cols.saturating_mul(rows);
    if count > MAX_TILES {
        return Err(TilingError::TooManyTiles {
            count,
            limit: MAX_TILES,
        });
    }
    let mut tiles = Vec::with_capacity(count);
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

/// Sheets needed to span `span` millimetres: the last sheet must still reach the
/// far edge, so tiles that would start past the board are not emitted.
fn axis_tiles(span: f64, sheet: f64, advance: f64) -> usize {
    if span <= sheet || sheet <= 0.0 || advance <= 0.0 {
        return 1;
    }
    let extra = ((span - sheet) / advance).ceil();
    if !extra.is_finite() || extra <= 0.0 {
        return 1;
    }
    // `as` saturates, which is exactly what the caller's limit check wants.
    (extra as usize).saturating_add(1)
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
/// keeps the source kerf settings and contains the visible geometry that lies on
/// the tile, clipped to the tile rectangle and translated so the tile origin is the
/// sheet origin. Hidden layers are skipped, and sheets that end up with nothing to
/// cut are not emitted. Output order is row-major, matching `layout`.
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
                // Only geometry that crosses the sheet edge needs clipping; a shape
                // wholly on the sheet keeps its curves instead of being flattened.
                let on_sheet = b.x0 >= tile.rect.x0 - 1e-9
                    && b.y0 >= tile.rect.y0 - 1e-9
                    && b.x1 <= tile.rect.x1 + 1e-9
                    && b.y1 <= tile.rect.y1 + 1e-9;
                let clipped;
                let source = if on_sheet {
                    &shape.path
                } else {
                    clipped = clip::clip_path_to_rect(&shape.path, tile.rect);
                    &clipped
                };
                if source.elements().is_empty() {
                    continue;
                }
                let mut translated = shape.clone();
                translated.path = transform_path(
                    source,
                    kurbo::Affine::translate((-tile.rect.x0, -tile.rect.y0)),
                );
                piece_layer.shapes.push(translated);
            }
            if !piece_layer.shapes.is_empty() {
                piece.layers.push(piece_layer);
            }
        }
        // A tile the artwork never reaches needs no material: skip the sheet rather
        // than sending an empty cut job to the machine.
        if piece.layers.is_empty() {
            continue;
        }
        out.push(piece);
    }
    Ok(out)
}

/// Convenience: shape count of each exported sheet, in the same order
/// `tile_document` returns them.
pub fn tile_shape_counts(
    doc: &Document,
    sheet_w: f64,
    sheet_h: f64,
    overlap_mm: f64,
) -> Result<Vec<usize>, TilingError> {
    Ok(tile_document(doc, sheet_w, sheet_h, overlap_mm)?
        .iter()
        .map(Document::shape_count)
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
    fn grid_coverage_has_no_gaps_and_overlap_is_bounded() {
        let board_w = 511.0;
        let board_h = 397.0;
        let sheet_w = 230.0;
        let sheet_h = 180.0;
        let overlap = 12.0;
        let tiles = layout(board_w, board_h, sheet_w, sheet_h, overlap).unwrap();
        // Sample the entire board at a 1 mm pitch. Every point must belong to
        // at least one tile; adjacent tile interiors overlap by no more than
        // the requested overlap in either axis.
        for yi in 0..=board_h as usize {
            for xi in 0..=board_w as usize {
                let x = xi as f64;
                let y = yi as f64;
                assert!(
                    tiles.iter().any(|tile| {
                        x >= tile.rect.x0
                            && x <= tile.rect.x1
                            && y >= tile.rect.y0
                            && y <= tile.rect.y1
                    }),
                    "uncovered point ({x}, {y})"
                );
            }
        }
        for a in &tiles {
            for b in &tiles {
                if a.row == b.row && a.col + 1 == b.col {
                    let shared = a.rect.x1.min(b.rect.x1) - a.rect.x0.max(b.rect.x0);
                    assert!(shared >= overlap - 1e-9, "horizontal gap: {shared}");
                    assert!(shared <= overlap + 1e-9, "horizontal overlap: {shared}");
                }
                if a.col == b.col && a.row + 1 == b.row {
                    let shared = a.rect.y1.min(b.rect.y1) - a.rect.y0.max(b.rect.y0);
                    assert!(shared >= overlap - 1e-9, "vertical gap: {shared}");
                    assert!(shared <= overlap + 1e-9, "vertical overlap: {shared}");
                }
            }
        }
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
    fn every_sheet_gets_only_its_part_of_the_traversing_shape() {
        let doc = cross_doc(500.0, 400.0);
        let counts = tile_shape_counts(&doc, 300.0, 300.0, 10.0).unwrap();
        // The diagonal never reaches the top-left sheet, which is therefore not
        // exported at all; the other three sheets each carry one piece of it.
        assert_eq!(counts, vec![1, 1, 1], "counts per sheet");
        let pieces = tile_document(&doc, 300.0, 300.0, 10.0).unwrap();
        assert_eq!(pieces.len(), 3);
        for piece in &pieces {
            assert_eq!(piece.shape_count(), 1);
            assert_eq!(piece.layers[0].kind, CutStyle::Cut);
            // Kerf settings travel with each sheet.
            assert_eq!(piece.kerf, doc.kerf);
        }
    }

    #[test]
    fn clipped_cut_paths_stay_on_their_sheet() {
        // A rectangle far larger than one sheet: every sheet must receive the
        // intersection, never the whole rectangle.
        let mut doc = Document {
            board: Board {
                width_mm: 600.0,
                height_mm: 600.0,
            },
            kerf: stencilcraft_doc::KerfSettings::default(),
            layers: vec![Layer::new("cut", CutStyle::Cut)],
            next_shape_id: 1,
        };
        let mut p = kurbo::BezPath::new();
        p.move_to(Point::new(5.0, 5.0));
        p.line_to(Point::new(595.0, 5.0));
        p.line_to(Point::new(595.0, 595.0));
        p.line_to(Point::new(5.0, 595.0));
        p.close_path();
        doc.push_shape(0, p, CutStyle::Cut).unwrap();

        // 3 × 3 sheets cover the board, but the middle one holds nothing of the
        // outline and must not be exported.
        assert_eq!(layout(600.0, 600.0, 300.0, 300.0, 10.0).unwrap().len(), 9);
        let pieces = tile_document(&doc, 300.0, 300.0, 10.0).unwrap();
        assert_eq!(pieces.len(), 8, "the middle sheet is empty and skipped");
        for piece in &pieces {
            assert_eq!(piece.shape_count(), 1);
            let b = bounds_of_shape(&piece.layers[0].shapes[0].path).unwrap();
            assert!(b.x0 >= -1e-6 && b.y0 >= -1e-6, "off the sheet: {b:?}");
            assert!(
                b.x1 <= piece.board.width_mm + 1e-6 && b.y1 <= piece.board.height_mm + 1e-6,
                "off the sheet: {b:?} on {}×{}",
                piece.board.width_mm,
                piece.board.height_mm
            );
        }
    }

    #[test]
    fn overlap_almost_as_wide_as_the_sheet_still_needs_one_sheet() {
        // 299 mm of overlap on a 300 mm sheet advances 1 mm per tile: counting
        // advances instead of sheets would ask for 90 000 sheets.
        let tiles = layout(300.0, 300.0, 300.0, 300.0, 299.0).unwrap();
        assert_eq!(tiles.len(), 1);
    }

    #[test]
    fn absurd_tilings_are_refused() {
        // A 100 mm sheet with a 99.9 mm overlap advances 0.1 mm: this would want
        // ~100 000 sheets per axis.
        assert!(matches!(
            layout(10_000.0, 10_000.0, 100.0, 100.0, 99.9),
            Err(TilingError::TooManyTiles { .. })
        ));
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
        // The top-right sheet starts at (290, 290) and holds the diagonal from
        // (363, 290) to (499, 399), i.e. roughly (73, 0) to (209, 109) in sheet space.
        let top_right = pieces.last().expect("sheets are never empty");
        assert_eq!(top_right.board.width_mm, 210.0);
        let b = bounds_of_shape(&top_right.layers[0].shapes[0].path).unwrap();
        assert!((b.x0 - 72.6).abs() < 1.0, "shape origin: {b:?}");
        assert!(
            b.y0.abs() < 1.0,
            "shape touches the sheet's bottom edge: {b:?}"
        );
        assert!(b.x1 <= 210.0 + 1e-6 && b.y1 <= 110.0 + 1e-6, "{b:?}");
    }
}
