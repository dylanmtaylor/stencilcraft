//! Golden SVG exports under `tests/golden/`: the exact bytes of every export this crate
//! writes. Regenerate with `UPDATE_GOLDEN=1 cargo test -p stencilcraft-svg --test golden`,
//! then review the diff by hand.

#![allow(clippy::unwrap_used)] // helpers outside #[test] fail loudly, like tests

use stencilcraft_doc::{CutStyle, Document, Group, GroupChild, Layer};
use stencilcraft_svg::{TilePreviewOptions, export, export_print_and_cut, export_tile_preview};
use stencilcraft_testkit::{line_path, rect_path, starter};

/// Nested groups, a group split by stacking order, a curve, a hidden shape, a hidden layer
/// and a template layer.
fn grouped() -> Document {
    let mut doc = Document::default();
    doc.layers.push(Layer::new("engrave", CutStyle::Engrave));
    doc.layers.push(Layer::new("hidden", CutStyle::Cut));
    doc.layers.push(Layer::new("guide", CutStyle::Cut));
    doc.layers[2].visible = false;
    doc.layers[3].template = true;
    let mut curve = kurbo::BezPath::new();
    curve.move_to((5.0, 5.0));
    curve.curve_to((10.5, 0.125), (20.0, 1.0 / 3.0), (25.0, 5.0));
    curve.quad_to((30.0, 12.0), (5.0, 5.0));
    curve.close_path();
    let a = doc
        .push_shape(0, rect_path(10.0, 10.0, 20.0, 20.0), CutStyle::Cut)
        .unwrap()
        .id;
    let b = doc.push_shape(0, curve, CutStyle::Cut).unwrap().id;
    let c = doc
        .push_shape(0, rect_path(40.0, 40.0, 5.0, 5.0), CutStyle::Cut)
        .unwrap()
        .id;
    let d = doc
        .push_shape(0, rect_path(-12.5, 60.0, 5.0, 5.0), CutStyle::Engrave)
        .unwrap()
        .id;
    let hidden = doc
        .push_shape(0, rect_path(50.0, 50.0, 5.0, 5.0), CutStyle::Cut)
        .unwrap()
        .id;
    doc.layers[0]
        .shapes
        .iter_mut()
        .for_each(|s| s.hidden = s.id == hidden);
    doc.push_shape(1, line_path(5.0, 90.0, 95.0, 90.0), CutStyle::Engrave)
        .unwrap();
    doc.push_shape(2, rect_path(1.0, 1.0, 2.0, 2.0), CutStyle::Cut)
        .unwrap();
    doc.push_shape(3, rect_path(0.0, 0.0, 300.0, 300.0), CutStyle::Cut)
        .unwrap();
    doc.groups = vec![
        Group {
            id: 1,
            name: String::new(),
            children: vec![
                GroupChild::Shape(a),
                GroupChild::Group(2),
                GroupChild::Shape(d),
            ],
        },
        Group {
            id: 2,
            name: "inner".to_string(),
            children: vec![GroupChild::Shape(b)],
        },
    ];
    let _ = c;
    doc
}

fn check(name: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(expected, actual, "{}", path.display());
}

#[test]
fn exports_match_their_golden_files() {
    check("export-starter.svg", &export(&starter()).unwrap());
    check("export-grouped.svg", &export(&grouped()).unwrap());
    check("export-empty.svg", &export(&Document::default()).unwrap());
    let mut small = starter();
    small.board.width_mm = 60.5;
    check(
        "tile-preview.svg",
        &export_tile_preview(
            &[starter(), grouped(), small],
            &TilePreviewOptions {
                columns: 2,
                scale: 0.25,
            },
        )
        .unwrap(),
    );
    check(
        "print-and-cut.svg",
        &export_print_and_cut(&starter()).unwrap(),
    );
}
