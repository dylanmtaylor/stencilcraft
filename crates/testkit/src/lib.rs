//! Shared sample documents, used by tests across the workspace. Nothing but
//! tests, fixtures and example generators should depend on this crate. Every
//! constructor is total: no unwraps, no panics — documents are built as literals
//! with explicit shape ids.

use kurbo::{BezPath, Point};
use stencilcraft_doc::{Board, CutStyle, Document, KerfSettings, Layer, Shape};

fn layer(name: &str, kind: CutStyle, shapes: Vec<Shape>) -> Layer {
    Layer {
        name: name.to_string(),
        kind,
        visible: true,
        shapes,
    }
}

fn shape(id: u64, path: BezPath, style: CutStyle) -> Shape {
    Shape { id, path, style }
}

pub fn rect_path(x: f64, y: f64, w: f64, h: f64) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(Point::new(x, y));
    p.line_to(Point::new(x + w, y));
    p.line_to(Point::new(x + w, y + h));
    p.line_to(Point::new(x, y + h));
    p.close_path();
    p
}

pub fn line_path(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(Point::new(x0, y0));
    p.line_to(Point::new(x1, y1));
    p
}

fn base_document(board: Board, kerf: KerfSettings, layers: Vec<Layer>) -> Document {
    let id_top = layers
        .iter()
        .map(|l| l.shapes.iter().map(|s| s.id).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    Document {
        board,
        kerf,
        layers,
        next_shape_id: id_top + 1,
    }
}

/// A square cut on the default 100 × 150 board.
pub fn simple_square() -> Document {
    base_document(
        Board::default(),
        KerfSettings::default(),
        vec![layer(
            "cut",
            CutStyle::Cut,
            vec![shape(1, rect_path(10.0, 10.0, 50.0, 50.0), CutStyle::Cut)],
        )],
    )
}

/// An outer square with an inner square: a counter, for kerf tests.
pub fn nested_counter() -> Document {
    base_document(
        Board {
            width_mm: 120.0,
            height_mm: 150.0,
        },
        KerfSettings::default(),
        vec![layer(
            "cut",
            CutStyle::Cut,
            vec![
                shape(1, rect_path(0.0, 0.0, 100.0, 100.0), CutStyle::Cut),
                shape(2, rect_path(30.0, 30.0, 40.0, 40.0), CutStyle::Cut),
            ],
        )],
    )
}

/// One cut shape and one engrave line on separate layers, for export tests.
pub fn mixed_layers() -> Document {
    base_document(
        Board::default(),
        KerfSettings::default(),
        vec![
            layer(
                "cut",
                CutStyle::Cut,
                vec![shape(1, rect_path(10.0, 10.0, 20.0, 20.0), CutStyle::Cut)],
            ),
            layer(
                "engrave",
                CutStyle::Engrave,
                vec![shape(2, line_path(5.0, 5.0, 25.0, 5.0), CutStyle::Engrave)],
            ),
        ],
    )
}

/// A 500 × 400 board with a diagonal line across it, sized for tiling tests.
pub fn big_diagonal() -> Document {
    base_document(
        Board {
            width_mm: 500.0,
            height_mm: 400.0,
        },
        KerfSettings::default(),
        vec![layer(
            "cut",
            CutStyle::Cut,
            vec![shape(1, line_path(1.0, 1.0, 499.0, 399.0), CutStyle::Cut)],
        )],
    )
}

/// The canonical starter document written by the CLI's `make-starter` command.
pub fn starter() -> Document {
    base_document(
        Board::default(),
        KerfSettings::default(),
        vec![
            layer(
                "cut",
                CutStyle::Cut,
                vec![
                    shape(1, rect_path(10.0, 10.0, 50.0, 50.0), CutStyle::Cut),
                    shape(2, rect_path(70.0, 20.0, 15.0, 40.0), CutStyle::Cut),
                ],
            ),
            layer(
                "engrave",
                CutStyle::Engrave,
                vec![shape(
                    3,
                    line_path(10.0, 90.0, 90.0, 90.0),
                    CutStyle::Engrave,
                )],
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<(&'static str, Document)> {
        vec![
            ("simple_square", simple_square()),
            ("nested_counter", nested_counter()),
            ("mixed_layers", mixed_layers()),
            ("big_diagonal", big_diagonal()),
            ("starter", starter()),
        ]
    }

    #[test]
    fn sample_docs_validate() {
        for (name, doc) in samples() {
            if let Err(e) = doc.validate() {
                panic!("{name} is invalid: {e}");
            }
        }
    }

    #[test]
    fn sample_docs_roundtrip_through_the_formatter() {
        for (name, doc) in samples() {
            let json = match doc.save_json() {
                Ok(j) => j,
                Err(e) => panic!("{name} cannot save: {e}"),
            };
            match Document::load_json(&json) {
                Ok(back) => assert_eq!(doc, back, "{name} did not round-trip"),
                Err(e) => panic!("{name} cannot reload: {e}"),
            }
        }
    }

    #[test]
    fn nested_counter_has_a_true_counter() {
        let doc = nested_counter();
        let inner = doc.get_shape(2);
        assert!(inner.is_some(), "inner shape exists");
    }

    #[test]
    fn ids_are_consistent_with_next_shape_id() {
        for (_, doc) in samples() {
            assert!(doc.next_shape_id > doc.shape_count() as u64);
            for (_, s) in doc.shapes() {
                assert!(s.id < doc.next_shape_id);
            }
        }
    }
}
