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
    engine.doc.shapes().map(|s| area(&s.path)).sum()
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
    assert!(
        (total_area(&engine) - 700.0).abs() < 0.1,
        "{}",
        total_area(&engine)
    );
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
        let (o, n) = (orig.bounds().unwrap(), now.bounds().unwrap());
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
    assert_eq!(
        svg_after.matches('M').count() + 1,
        svg_before.matches('M').count()
    );
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
    assert_eq!(
        mats.mats[0].placements.len(),
        2,
        "two copies of one attached piece"
    );
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
        engine
            .doc
            .get_shape(a)
            .unwrap()
            .production
            .fill_color
            .as_deref(),
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
        serde_json::json!({"command": "shapes.set-production", "params": {"ids": [a], "operation": {"kind": "score"}, "material": "Cardstock"}}),
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

#[test]
fn layer_membership_commands_round_trip_through_dot_named_json() {
    let mut engine = Engine::default();
    let a = add(&mut engine, rect_path(0.0, 0.0, 10.0, 10.0));
    let b = add(&mut engine, rect_path(20.0, 0.0, 10.0, 10.0));
    let c = add(&mut engine, rect_path(40.0, 0.0, 10.0, 10.0));
    let orig_layer = engine.doc.layer_of_shape(b).unwrap();
    // layer.add with an operation creates a new operation layer on top.
    run(
        &mut engine,
        serde_json::json!({"command": "layer.add", "params": {"name": "cut", "operation": {"kind": "cut"}}}),
    )
    .unwrap();
    let cut = engine.doc.layers.len() - 1;
    let effect = run(
        &mut engine,
        serde_json::json!({"command": "layer.add-members", "params": {"layer": cut, "ids": [a, b]}}),
    )
    .unwrap();
    assert!(matches!(effect, Effect::LayerMembersChanged { layer, .. } if layer == cut));
    // Additive and idempotent: [b, c] leaves [a, b, c]; another layer keeps b.
    run(
        &mut engine,
        serde_json::json!({"command": "layer.add-members", "params": {"layer": cut, "ids": [b, c]}}),
    )
    .unwrap();
    assert_eq!(engine.doc.layers[cut].members, vec![a, b, c]);
    assert!(engine.doc.layers[orig_layer].members.contains(&b));
    // Geometry invariance: shape paths and bounds are untouched by membership.
    let before: Vec<_> = engine
        .doc
        .shapes()
        .map(|s| (s.id, s.path.clone(), s.bounds()))
        .collect();
    run(
        &mut engine,
        serde_json::json!({"command": "layer.remove-members", "params": {"layer": cut, "ids": [b]}}),
    )
    .unwrap();
    run(
        &mut engine,
        serde_json::json!({"command": "layer.remove-members", "params": {"layer": cut, "ids": [b]}}),
    )
    .unwrap();
    assert_eq!(engine.doc.layers[cut].members, vec![a, c]);
    let after: Vec<_> = engine
        .doc
        .shapes()
        .map(|s| (s.id, s.path.clone(), s.bounds()))
        .collect();
    assert_eq!(before, after);
    // layer.set-operation sets, clears, and validates.
    run(
        &mut engine,
        serde_json::json!({"command": "layer.set-operation", "params": {"index": 0, "operation": {"kind": "engrave"}}}),
    )
    .unwrap();
    assert!(
        engine.doc.layers[0]
            .operation
            .as_ref()
            .is_some_and(|op| matches!(op, stencilcraft_doc::Operation::Engrave(_)))
    );
    run(
        &mut engine,
        serde_json::json!({"command": "layer.set-operation", "params": {"index": 0, "operation": null}}),
    )
    .unwrap();
    assert_eq!(engine.doc.layers[0].operation, None);
    // Undo restores the previous memberships.
    let before_undo = engine.doc.clone();
    engine.run(Command::Undo).unwrap();
    assert_ne!(engine.doc, before_undo);
    while engine.can_undo() {
        engine.undo();
    }
    let fresh = Engine::default();
    let _ = add(&mut Engine::default(), rect_path(0.0, 0.0, 1.0, 1.0));
    assert_eq!(engine.doc.layers.len(), fresh.doc.layers.len());
}

#[test]
fn membership_commands_reject_locked_layers() {
    let mut engine = Engine::default();
    let a = add(&mut engine, rect_path(0.0, 0.0, 10.0, 10.0));
    run(
        &mut engine,
        serde_json::json!({"command": "layer.set-locked", "params": {"index": 0, "locked": true}}),
    )
    .unwrap();
    assert!(
        run(
            &mut engine,
            serde_json::json!({"command": "layer.add-members", "params": {"layer": 0, "ids": [a]}})
        )
        .is_err()
    );
    assert!(
        run(
            &mut engine,
            serde_json::json!({"command": "layer.remove-members", "params": {"layer": 0, "ids": [a]}})
        )
        .is_err()
    );
    assert!(
        run(
            &mut engine,
            serde_json::json!({"command": "layer.set-operation", "params": {"index": 0, "operation": {"kind": "cut"}}})
        )
        .is_err()
    );
}

/// 02-04: the exact dot-named JSON the UI dispatches (operation popup, palette drop,
/// Send to Layer… menu) is additive, idempotent, geometry-invariant, and undoable.
#[test]
fn assignment_json_is_additive_idempotent_geometry_invariant_and_undoable() {
    let mut engine = Engine::default();
    let a = add(&mut engine, rect_path(0.0, 0.0, 10.0, 10.0));
    let b = add(&mut engine, rect_path(20.0, 0.0, 10.0, 10.0));
    let c = add(&mut engine, rect_path(40.0, 0.0, 10.0, 10.0));
    // A second, operation-less layer to assign into.
    run(
        &mut engine,
        serde_json::json!({"command": "layer.add", "params": {"name": "pass2", "operation": null}}),
    )
    .unwrap();
    let layer1 = engine.doc.layers.len() - 1;

    let geometry = |e: &Engine| {
        e.doc
            .shapes()
            .map(|s| (s.id, s.path.clone(), s.bounds()))
            .collect::<Vec<_>>()
    };

    // Additive assign: shape paths and bounds stay byte-equal and the shape's
    // original layer keeps it (D-07).
    let orig_layer = engine.doc.layer_of_shape(a).unwrap();
    let before_doc = engine.doc.clone();
    let before_geo = geometry(&engine);
    run(
        &mut engine,
        serde_json::json!({"command": "layer.add-members", "params": {"layer": layer1, "ids": [a, b]}}),
    )
    .unwrap();
    assert_eq!(geometry(&engine), before_geo);
    assert!(engine.doc.layers[orig_layer].members.contains(&a));
    assert_eq!(engine.doc.layers[layer1].members, vec![a, b]);
    // Undo restores the previous memberships.
    assert!(engine.undo());
    assert_eq!(engine.doc, before_doc);

    // Re-adding the same ids is idempotent.
    run(
        &mut engine,
        serde_json::json!({"command": "layer.add-members", "params": {"layer": layer1, "ids": [a, b]}}),
    )
    .unwrap();
    let mid = engine.doc.clone();
    run(
        &mut engine,
        serde_json::json!({"command": "layer.add-members", "params": {"layer": layer1, "ids": [a, b]}}),
    )
    .unwrap();
    assert_eq!(engine.doc, mid);
    assert!(engine.undo());
    assert_eq!(engine.doc, mid);
    assert!(engine.undo());
    assert_eq!(engine.doc, before_doc);

    // Removing an id that is not a member is a no-op success.
    run(
        &mut engine,
        serde_json::json!({"command": "layer.remove-members", "params": {"layer": layer1, "ids": [c]}}),
    )
    .unwrap();
    assert_eq!(engine.doc, before_doc);

    // set-operation with a Cut operation puts the layer into production_sequence()
    // once it has a member; null clears it again. Undo restores each state.
    run(
        &mut engine,
        serde_json::json!({"command": "layer.set-operation", "params": {"index": layer1, "operation": {"kind": "cut"}}}),
    )
    .unwrap();
    assert!(
        engine.doc.layers[layer1]
            .operation
            .as_ref()
            .is_some_and(|op| matches!(op, stencilcraft_doc::Operation::Cut(_)))
    );
    assert!(
        engine
            .doc
            .production_sequence()
            .any(|(l, _)| l.name == "pass2"),
        "a layer with a Cut operation is a production pass"
    );
    run(
        &mut engine,
        serde_json::json!({"command": "layer.add-members", "params": {"layer": layer1, "ids": [a]}}),
    )
    .unwrap();
    assert!(
        engine
            .doc
            .production_sequence()
            .any(|(l, shapes)| l.name == "pass2" && shapes.iter().any(|s| s.id == a))
    );
    assert!(engine.undo());
    assert!(engine.undo());
    assert_eq!(engine.doc, before_doc);
    run(
        &mut engine,
        serde_json::json!({"command": "layer.set-operation", "params": {"index": layer1, "operation": null}}),
    )
    .unwrap();
    assert_eq!(engine.doc.layers[layer1].operation, None);
    assert!(engine.undo());
    assert_eq!(engine.doc, before_doc);
}
