//! Production commands: Slice, Weld, Attach, Flatten, Contour, Combine, Create Sticker,
//! production settings and the layout/job/print queries, each through the JSON envelope.
#![allow(clippy::unwrap_used, clippy::panic)]

use kurbo::BezPath;
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::{Command, Effect, Engine, ProductionEffect};
use vectorcraft_geom::{FillRule, PathData};

fn rect_path(x: f64, y: f64, w: f64, h: f64) -> BezPath {
    let mut p = BezPath::new();
    p.move_to((x, y));
    p.line_to((x + w, y));
    p.line_to((x + w, y + h));
    p.line_to((x, y + h));
    p.close_path();
    p
}

fn add(engine: &mut Engine, path: BezPath) -> u64 {
    match engine
        .run(Command::ShapeAddPath {
            path,
            style: CutStyle::Cut,
        })
        .unwrap()
    {
        Effect::ShapeAdded { id } => id,
        other => panic!("unexpected {other:?}"),
    }
}

fn run(engine: &mut Engine, json: serde_json::Value) -> Result<Effect, String> {
    let command: Command = serde_json::from_value(json).map_err(|e| e.to_string())?;
    engine.run(command).map_err(|e| e.to_string())
}

fn area(path: &BezPath) -> f64 {
    vectorcraft_pathops::area(&PathData::from_bezpath(path), FillRule::EvenOdd)
}

fn total_area(engine: &Engine) -> f64 {
    engine.doc.shapes().map(|(_, s)| area(&s.path)).sum()
}

#[test]
fn slice_makes_three_pieces_whose_areas_sum_to_the_union() {
    let mut engine = Engine::default();
    let a = add(&mut engine, rect_path(0.0, 0.0, 20.0, 20.0));
    let b = add(&mut engine, rect_path(10.0, 10.0, 20.0, 20.0));
    let before = engine.doc.clone();
    let effect = run(
        &mut engine,
        serde_json::json!({"command": "shapes.slice", "params": {"a": a, "b": b}}),
    )
    .unwrap();
    let Effect::ShapesAdded { ids } = effect else {
        panic!("{effect:?}")
    };
    assert_eq!(ids.len(), 3);
    assert_eq!(engine.doc.shape_count(), 3);
    // 400 + 400 − 100 overlap.
    assert!((total_area(&engine) - 700.0).abs() < 0.1, "{}", total_area(&engine));
    assert!(engine.doc.get_shape(a).is_none());
    assert!(engine.undo());
    assert_eq!(engine.doc, before);

    // A bar across a square splits the square in two: four pieces.
    let mut engine = Engine::default();
    let a = add(&mut engine, rect_path(0.0, 0.0, 30.0, 30.0));
    let b = add(&mut engine, rect_path(-5.0, 10.0, 40.0, 10.0));
    let Effect::ShapesAdded { ids } = run(
        &mut engine,
        serde_json::json!({"command": "shapes.slice", "params": {"a": a, "b": b}}),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(ids.len(), 5, "two square halves, two bar ends, the overlap");
    assert!((total_area(&engine) - (900.0 + 100.0)).abs() < 0.1);

    let mut engine = Engine::default();
    let a = add(&mut engine, rect_path(0.0, 0.0, 5.0, 5.0));
    let b = add(&mut engine, rect_path(50.0, 0.0, 5.0, 5.0));
    let err = run(
        &mut engine,
        serde_json::json!({"command": "shapes.slice", "params": {"a": a, "b": b}}),
    )
    .unwrap_err();
    assert!(err.contains("do not overlap"), "{err}");
    assert!(!engine.can_undo() || engine.doc.shape_count() == 2);
}

#[test]
fn weld_joins_and_combine_is_reversible() {
    let mut engine = Engine::default();
    let a = add(&mut engine, rect_path(0.0, 0.0, 20.0, 20.0));
    let b = add(&mut engine, rect_path(10.0, 0.0, 20.0, 20.0));
    let original = engine.doc.clone();

    run(
        &mut engine,
        serde_json::json!({"command": "shapes.weld", "params": {"ids": [a, b]}}),
    )
    .unwrap();
    assert_eq!(engine.doc.shape_count(), 1);
    assert!((total_area(&engine) - 600.0).abs() < 0.1);
    assert!(engine.undo());

    let Effect::ShapeAdded { id } = run(
        &mut engine,
        serde_json::json!({"command": "shapes.combine", "params": {"ids": [a, b], "operation": "subtract"}}),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(engine.doc.shape_count(), 1);
    assert!((total_area(&engine) - 200.0).abs() < 0.1);
    // Change the operation in place, then move the group: members follow.
    run(
        &mut engine,
        serde_json::json!({"command": "shape.set-combine", "params": {"id": id, "operation": "intersect"}}),
    )
    .unwrap();
    assert!((total_area(&engine) - 200.0).abs() < 0.1);
    run(
        &mut engine,
        serde_json::json!({"command": "shapes.set-combine", "params": {}}),
    )
    .unwrap_err();
    engine
        .run(Command::ShapeMove {
            id,
            dx_mm: 5.0,
            dy_mm: 7.0,
        })
        .unwrap();
    let Effect::ShapesAdded { ids } = run(
        &mut engine,
        serde_json::json!({"command": "shapes.uncombine", "params": {"id": id}}),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(ids, vec![a, b]);
    for (orig, now) in original.shapes().zip(engine.doc.shapes()) {
        let (o, n) = (orig.1.bounds().unwrap(), now.1.bounds().unwrap());
        assert!((n.x0 - o.x0 - 5.0).abs() < 1e-6 && (n.y0 - o.y0 - 7.0).abs() < 1e-6);
    }
    // Undo everything: the starting document comes back.
    while engine.can_undo() && engine.doc != original {
        engine.undo();
    }
    assert_eq!(engine.doc, original);
}

#[test]
fn contour_hide_changes_exports_only() {
    let mut engine = Engine::default();
    let mut o = rect_path(0.0, 0.0, 30.0, 30.0);
    o.extend(rect_path(10.0, 10.0, 10.0, 10.0).iter());
    let id = add(&mut engine, o);
    let path_before = engine.doc.get_shape(id).unwrap().path.clone();
    let svg_before = stencilcraft_svg::export(&engine.doc).unwrap();
    run(
        &mut engine,
        serde_json::json!({"command": "shape.contour", "params": {"id": id, "contours": [1], "hidden": true}}),
    )
    .unwrap();
    let shape = engine.doc.get_shape(id).unwrap();
    assert_eq!(shape.path, path_before, "the path keeps the contour");
    assert_eq!(shape.cut_path().elements().len(), 5);
    let svg_after = stencilcraft_svg::export(&engine.doc).unwrap();
    assert_ne!(svg_before, svg_after);
    assert_eq!(svg_after.matches('M').count() + 1, svg_before.matches('M').count());
    let job = match run(
        &mut engine,
        serde_json::json!({"command": "production.job", "params": {}}),
    )
    .unwrap()
    {
        Effect::Production(ProductionEffect::Job(job)) => job,
        other => panic!("{other:?}"),
    };
    assert_eq!(job.mats[0].ops.len(), 1);
    // Showing it again restores the export.
    run(
        &mut engine,
        serde_json::json!({"command": "shape.contour", "params": {"id": id, "contours": [1], "hidden": false}}),
    )
    .unwrap();
    assert_eq!(stencilcraft_svg::export(&engine.doc).unwrap(), svg_before);
    assert!(
        run(
            &mut engine,
            serde_json::json!({"command": "shape.contour", "params": {"id": id, "contours": [7], "hidden": true}}),
        )
        .is_err()
    );
}

#[test]
fn attach_flatten_sticker_and_settings() {
    let mut engine = Engine::default();
    let a = add(&mut engine, rect_path(0.0, 0.0, 10.0, 10.0));
    let b = add(&mut engine, rect_path(40.0, 0.0, 10.0, 10.0));
    run(
        &mut engine,
        serde_json::json!({"command": "shapes.attach", "params": {"ids": [a, b]}}),
    )
    .unwrap();
    let mats = match run(
        &mut engine,
        serde_json::json!({"command": "production.mats", "params": {"copies": 2}}),
    )
    .unwrap()
    {
        Effect::Production(ProductionEffect::Mats(m)) => m,
        other => panic!("{other:?}"),
    };
    assert_eq!(mats.mats[0].placements.len(), 2, "two copies of one attached piece");
    run(
        &mut engine,
        serde_json::json!({"command": "shapes.detach", "params": {"ids": [a]}}),
    )
    .unwrap();
    assert!(engine.doc.get_shape(b).unwrap().production.attach.is_none());

    run(
        &mut engine,
        serde_json::json!({"command": "shapes.set-production", "params": {"ids": [a], "fill_color": "red", "material": "Sticker Paper"}}),
    )
    .unwrap();
    assert_eq!(
        engine.doc.get_shape(a).unwrap().production.fill_color.as_deref(),
        Some("#ff0000")
    );
    assert!(
        run(
            &mut engine,
            serde_json::json!({"command": "shapes.set-production", "params": {"ids": [a], "line_color": "not a colour"}}),
        )
        .is_err()
    );

    let Effect::ShapeAdded { id: sticker } = run(
        &mut engine,
        serde_json::json!({"command": "shapes.create-sticker", "params": {"ids": [a, b], "border_mm": 2.0, "cut": "die-cut"}}),
    )
    .unwrap() else {
        panic!()
    };
    let s = engine.doc.get_shape(sticker).unwrap();
    let bounds = s.bounds().unwrap();
    assert!((bounds.x0 + 2.0).abs() < 0.05 && (bounds.x1 - 52.0).abs() < 0.05);
    assert_eq!(engine.doc.shape_count(), 1);
    let sheets = match run(
        &mut engine,
        serde_json::json!({"command": "production.print-sheet", "params": {}}),
    )
    .unwrap()
    {
        Effect::Production(ProductionEffect::PrintSheets(s)) => s,
        other => panic!("{other:?}"),
    };
    assert_eq!(sheets[0].items.iter().filter(|i| !i.bleed).count(), 2);
    let Effect::ShapesAdded { ids } = run(
        &mut engine,
        serde_json::json!({"command": "shapes.unflatten", "params": {"id": sticker}}),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(ids, vec![a, b]);

    let Effect::ShapeAdded { id: flat } = run(
        &mut engine,
        serde_json::json!({"command": "shapes.flatten", "params": {"ids": [a, b]}}),
    )
    .unwrap() else {
        panic!()
    };
    run(
        &mut engine,
        serde_json::json!({"command": "shapes.move-to-mat", "params": {"ids": [flat], "mat": 3}}),
    )
    .unwrap();
    assert_eq!(engine.doc.get_shape(flat).unwrap().production.mat, Some(3));
    engine
        .run(Command::ShapesSetLocked {
            ids: vec![flat],
            locked: true,
        })
        .unwrap();
    assert!(
        run(
            &mut engine,
            serde_json::json!({"command": "shapes.unflatten", "params": {"id": flat}}),
        )
        .unwrap_err()
        .contains("locked")
    );
    let machines = run(
        &mut engine,
        serde_json::json!({"command": "production.machines", "params": {}}),
    )
    .unwrap();
    assert!(matches!(machines, Effect::Production(ProductionEffect::Machines(m)) if m.len() > 5));
}

#[test]
fn production_metadata_survives_vectorcraft_edits_and_saves() {
    let mut engine = Engine::default();
    let a = add(&mut engine, rect_path(0.0, 0.0, 10.0, 10.0));
    run(
        &mut engine,
        serde_json::json!({"command": "shapes.set-production", "params": {"ids": [a], "operation": "score", "material": "Cardstock"}}),
    )
    .unwrap();
    engine
        .run(Command::ShapeMove {
            id: a,
            dx_mm: 1.0,
            dy_mm: 0.0,
        })
        .unwrap();
    let p = &engine.doc.get_shape(a).unwrap().production;
    assert_eq!(p.material.as_deref(), Some("Cardstock"));
    let json = engine.doc.save_json().unwrap();
    let back = stencilcraft_doc::Document::load_json(&json).unwrap();
    assert_eq!(back, engine.doc);
}
