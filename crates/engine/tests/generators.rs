//! Generator and effect commands (`generate.*`): JSON params as MCP sends them, one undo
//! step each, tool tags, in-place edits keeping ids, and calm errors that change nothing.
#![allow(clippy::unwrap_used, clippy::panic)]

use kurbo::Shape as _;
use stencilcraft_doc::{CutStyle, Tip, ToolOp};
use stencilcraft_engine::{Command, Effect, Engine};

fn cmd(json: &str) -> Command {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

fn added(e: &mut Engine, json: &str) -> Vec<u64> {
    match e
        .run(cmd(json))
        .unwrap_or_else(|err| panic!("{json}: {err}"))
    {
        Effect::ShapesAdded { ids } => ids,
        Effect::ShapeAdded { id } => vec![id],
        other => panic!("{json}: {other:?}"),
    }
}

fn square(e: &mut Engine, x: f64, size: f64) -> u64 {
    added(
        e,
        &format!(
            r#"{{"command":"shape.add-rect","params":{{"x":{x},"y":0,"width":{size},"height":{size}}}}}"#
        ),
    )[0]
}

/// Runs `json`, checks one undo restores the document exactly and redo repeats it.
fn undoable(e: &mut Engine, json: &str) -> Effect {
    let before = e.doc.clone();
    let effect = e
        .run(cmd(json))
        .unwrap_or_else(|err| panic!("{json}: {err}"));
    let after = e.doc.clone();
    assert_ne!(before, after, "{json} changed nothing");
    assert!(e.undo());
    assert_eq!(e.doc, before, "{json} undo");
    assert!(e.redo());
    assert_eq!(e.doc, after, "{json} redo");
    effect
}

#[test]
fn parametric_shapes_add_and_edit_in_place() {
    let mut e = Engine::default();
    let ids = added(
        &mut e,
        r#"{"command":"generate.shape","params":{"shape":{"kind":"gear","teeth":16},"x":10,"y":10,"width":40,"height":40}}"#,
    );
    let id = ids[0];
    let b = e.doc.get_shape(id).unwrap().bounds().unwrap();
    assert!((b.x0 - 10.0).abs() < 1e-6 && (b.x1 - 50.0).abs() < 1e-6);
    let effect = undoable(
        &mut e,
        &format!(
            r#"{{"command":"generate.shape","params":{{"shape":{{"kind":"heart"}},"x":0,"y":0,"width":20,"height":20,"replace":{id}}}}}"#
        ),
    );
    assert_eq!(effect, Effect::ShapesEdited { ids: vec![id] });
    assert_eq!(e.doc.shape_count(), 1);
    let b = e.doc.get_shape(id).unwrap().bounds().unwrap();
    assert!((b.x1 - 20.0).abs() < 1e-6);
    // Every kind parses with defaults.
    for kind in [
        "heart",
        "arrow",
        "speech-bubble",
        "ring",
        "crescent",
        "gear",
        "spiral",
        "banner",
        "scallop-circle",
        "scallop-square",
        "triangle",
    ] {
        added(
            &mut e,
            &format!(
                r#"{{"command":"generate.shape","params":{{"shape":{{"kind":"{kind}"}},"x":0,"y":0,"width":10,"height":10}}}}"#
            ),
        );
    }
    let before = e.doc.clone();
    for bad in [
        r#"{"command":"generate.shape","params":{"shape":{"kind":"gear","teeth":1},"x":0,"y":0,"width":10,"height":10}}"#,
        r#"{"command":"generate.shape","params":{"shape":{"kind":"heart"},"x":0,"y":0,"width":-1,"height":10}}"#,
        r#"{"command":"generate.shape","params":{"shape":{"kind":"heart"},"x":0,"y":0,"width":1,"height":1,"replace":999}}"#,
    ] {
        assert!(e.run(cmd(bad)).is_err(), "{bad}");
    }
    assert_eq!(e.doc, before);
}

#[test]
fn fills_are_tagged_engrave_paths_and_release_to_plain_paths() {
    let mut e = Engine::default();
    let sq = square(&mut e, 0.0, 10.0);
    let effect = undoable(
        &mut e,
        &format!(
            r#"{{"command":"generate.fill","params":{{"ids":[{sq}],"pattern":"lines","spacing_mm":1,"angle_deg":0,"tool":{{"op":"heat-pen"}}}}}}"#
        ),
    );
    let Effect::ShapesAdded { ids } = effect else {
        panic!("{effect:?}")
    };
    let fill = e.doc.get_shape(ids[0]).unwrap();
    assert_eq!(fill.style, CutStyle::Engrave);
    assert_eq!(fill.tool, Some(ToolOp::HeatPen));
    let lines = fill
        .path
        .elements()
        .iter()
        .filter(|el| matches!(el, kurbo::PathEl::MoveTo(_)))
        .count();
    assert_eq!(lines, 10);
    // The tag survives an unrelated edit that round-trips through the session.
    e.run(Command::ShapeMove {
        id: sq,
        dx_mm: 1.0,
        dy_mm: 0.0,
    })
    .unwrap();
    assert_eq!(e.doc.get_shape(ids[0]).unwrap().tool, Some(ToolOp::HeatPen));
    let released = undoable(
        &mut e,
        &format!(
            r#"{{"command":"generate.release","params":{{"ids":[{}]}}}}"#,
            ids[0]
        ),
    );
    let Effect::ShapesAdded { ids: plain } = released else {
        panic!()
    };
    assert_eq!(plain.len(), 10);
    assert!(
        plain
            .iter()
            .all(|id| e.doc.get_shape(*id).unwrap().tool.is_none())
    );
    assert!(e.doc.get_shape(ids[0]).is_none());
    // Power engraver stipple and a bad spacing.
    added(
        &mut e,
        &format!(
            r#"{{"command":"generate.fill","params":{{"ids":[{sq}],"pattern":"stipple-grid","spacing_mm":2,"dot_mm":0.3,"tool":{{"op":"power-engrave"}}}}}}"#
        ),
    );
    let before = e.doc.clone();
    assert!(
        e.run(cmd(&format!(
            r#"{{"command":"generate.fill","params":{{"ids":[{sq}],"spacing_mm":0}}}}"#
        )))
        .is_err()
    );
    assert_eq!(e.doc, before);
}

#[test]
fn tool_lines_score_and_emboss_convert_outlines() {
    let mut e = Engine::default();
    let sq = square(&mut e, 0.0, 10.0);
    let effect = undoable(
        &mut e,
        &format!(
            r#"{{"command":"generate.tool-lines","params":{{"ids":[{sq}],"tool":{{"op":"emboss","tip":"wide","passes":2}},"fill":{{"pattern":"grid","spacing_mm":2}}}}}}"#
        ),
    );
    let Effect::ShapesAdded { ids } = effect else {
        panic!()
    };
    assert_eq!(ids.len(), 2);
    assert!(e.doc.get_shape(sq).is_none());
    for id in ids {
        let s = e.doc.get_shape(id).unwrap();
        assert_eq!(s.style, CutStyle::Engrave);
        assert_eq!(
            s.tool,
            Some(ToolOp::Emboss {
                tip: Tip::Wide,
                passes: 2,
                deboss: false
            })
        );
    }
    let line = added(
        &mut e,
        r#"{"command":"generate.score-line","params":{"x0":0,"y0":0,"x1":0,"y1":50}}"#,
    );
    assert_eq!(
        e.doc.get_shape(line[0]).unwrap().tool,
        Some(ToolOp::Score {
            tip: Tip::Fine,
            passes: 1
        })
    );
    assert!(
        e.run(cmd(
            r#"{"command":"generate.score-line","params":{"x0":0,"y0":0,"x1":0,"y1":5,"passes":0}}"#
        ))
        .is_err()
    );
}

#[test]
fn rhinestones_punches_puzzle_weed_and_marks() {
    let mut e = Engine::default();
    let sq = square(&mut e, 0.0, 40.0);
    let stones = added(
        &mut e,
        &format!(
            r#"{{"command":"generate.rhinestone","params":{{"ids":[{sq}],"size":"ss10","spacing_mm":1}}}}"#
        ),
    );
    assert_eq!(e.doc.get_shape(stones[0]).unwrap().style, CutStyle::Cut);
    let puzzle = undoable(
        &mut e,
        &format!(r#"{{"command":"generate.puzzle","params":{{"id":{sq},"rows":3,"columns":3}}}}"#),
    );
    assert!(matches!(puzzle, Effect::ShapesAdded { .. }));
    let weed = added(
        &mut e,
        &format!(
            r#"{{"command":"generate.weed","params":{{"ids":[{sq}],"margin_mm":5,"spacing_mm":10}}}}"#
        ),
    );
    assert_eq!(weed.len(), 2);
    let b = e.doc.get_shape(weed[0]).unwrap().bounds().unwrap();
    assert!((b.x0 + 5.0).abs() < 1e-6 && (b.x1 - 45.0).abs() < 1e-6);
    let marks = added(
        &mut e,
        &format!(
            r#"{{"command":"generate.registration","params":{{"ids":[{sq}],"mark":"crosshair"}}}}"#
        ),
    );
    assert_eq!(
        e.doc.get_shape(marks[0]).unwrap().bounds().unwrap().x0,
        -5.0
    );
    let circle = added(
        &mut e,
        r#"{"command":"shape.add-ellipse","params":{"x":100,"y":0,"width":20,"height":20}}"#,
    )[0];
    let punched = undoable(
        &mut e,
        &format!(r#"{{"command":"generate.punch","params":{{"ids":[{circle}],"diameter_mm":1}}}}"#),
    );
    let Effect::ShapesAdded { ids } = punched else {
        panic!()
    };
    let s = e.doc.get_shape(ids[0]).unwrap();
    assert_eq!(s.tool, Some(ToolOp::Punch { diameter_mm: 1.0 }));
    assert!(e.doc.get_shape(circle).is_none());
    assert!(
        e.run(cmd(&format!(
            r#"{{"command":"generate.punch","params":{{"ids":[{}],"diameter_mm":50}}}}"#,
            ids[0]
        )))
        .is_err()
    );
}

#[test]
fn popup_card_cuts_and_scores() {
    let mut e = Engine::default();
    let effect = undoable(
        &mut e,
        r#"{"command":"generate.popup","params":{"width":200,"height":140,"struts":3,"strut_width_mm":15,"base_width_pct":25,"dash_pitch_mm":3}}"#,
    );
    let Effect::ShapesAdded { ids } = effect else {
        panic!()
    };
    assert_eq!(ids.len(), 2);
    assert_eq!(e.doc.get_shape(ids[0]).unwrap().style, CutStyle::Cut);
    let scores = e.doc.get_shape(ids[1]).unwrap();
    assert_eq!(scores.style, CutStyle::Engrave);
    assert!(matches!(scores.tool, Some(ToolOp::Score { .. })));
    assert!(
        e.run(cmd(
            r#"{"command":"generate.popup","params":{"struts":20,"strut_width_mm":20}}"#
        ))
        .is_err()
    );
}

#[test]
fn warp_perforate_and_wavy_edit_in_place() {
    let mut e = Engine::default();
    let sq = square(&mut e, 0.0, 20.0);
    let effect = undoable(
        &mut e,
        &format!(
            r#"{{"command":"generate.warp","params":{{"ids":[{sq}],"style":"bulge","bend":0.5}}}}"#
        ),
    );
    assert_eq!(effect, Effect::ShapesEdited { ids: vec![sq] });
    let b = e.doc.get_shape(sq).unwrap().bounds().unwrap();
    assert!(b.y1 > 24.0, "{b:?}");
    undoable(
        &mut e,
        &format!(
            r#"{{"command":"generate.wavy","params":{{"ids":[{sq}],"amplitude_mm":0.5,"wavelength_mm":4}}}}"#
        ),
    );
    undoable(
        &mut e,
        &format!(
            r#"{{"command":"generate.perforate","params":{{"ids":[{sq}],"cut_mm":2,"gap_mm":1}}}}"#
        ),
    );
    let path = &e.doc.get_shape(sq).unwrap().path;
    assert!(
        !path
            .elements()
            .iter()
            .any(|el| matches!(el, kurbo::PathEl::ClosePath))
    );
    assert_eq!(e.doc.shape_count(), 1);
    let before = e.doc.clone();
    for bad in [
        format!(r#"{{"command":"generate.warp","params":{{"ids":[{sq}],"bend":3}}}}"#),
        format!(
            r#"{{"command":"generate.perforate","params":{{"ids":[{sq}],"cut_mm":0,"gap_mm":1}}}}"#
        ),
        r#"{"command":"generate.wavy","params":{"ids":[999],"amplitude_mm":1,"wavelength_mm":1}}"#
            .to_string(),
    ] {
        assert!(e.run(cmd(&bad)).is_err(), "{bad}");
    }
    assert_eq!(e.doc, before);
    e.run(Command::ShapesSetLocked {
        ids: vec![sq],
        locked: true,
    })
    .unwrap();
    assert!(
        e.run(cmd(&format!(
            r#"{{"command":"generate.warp","params":{{"ids":[{sq}]}}}}"#
        )))
        .is_err()
    );
}

#[test]
fn object_on_path_and_advanced_replicate() {
    let mut e = Engine::default();
    let dot = square(&mut e, 0.0, 2.0);
    let ring = added(
        &mut e,
        r#"{"command":"shape.add-ellipse","params":{"x":50,"y":50,"width":40,"height":40}}"#,
    )[0];
    let effect = undoable(
        &mut e,
        &format!(
            r#"{{"command":"generate.object-on-path","params":{{"ids":[{dot}],"path_id":{ring},"count":12}}}}"#
        ),
    );
    let Effect::ShapesAdded { ids } = effect else {
        panic!()
    };
    assert_eq!(ids.len(), 12);
    for id in &ids {
        let c = e.doc.get_shape(*id).unwrap().bounds().unwrap().center();
        assert!(
            (c.distance(kurbo::Point::new(70.0, 70.0)) - 20.0).abs() < 0.1,
            "{c:?}"
        );
    }
    let effect = undoable(
        &mut e,
        &format!(
            r#"{{"command":"generate.replicate","params":{{"ids":[{dot}],"direction":"angle","angle_deg":90,"count":3,"gap_mm":1}}}}"#
        ),
    );
    let Effect::ShapesAdded { ids } = effect else {
        panic!()
    };
    let ys: Vec<f64> = ids
        .iter()
        .map(|id| e.doc.get_shape(*id).unwrap().bounds().unwrap().y0)
        .collect();
    for (k, y) in ys.iter().enumerate() {
        assert!((y - 3.0 * (k as f64 + 1.0)).abs() < 1e-6, "{ys:?}");
    }
    let on_top = added(
        &mut e,
        &format!(
            r#"{{"command":"generate.replicate","params":{{"ids":[{dot}],"direction":"on-top","count":2}}}}"#
        ),
    );
    for id in on_top {
        assert_eq!(
            e.doc.get_shape(id).unwrap().path,
            e.doc.get_shape(dot).unwrap().path
        );
    }
    assert!(e
        .run(cmd(&format!(r#"{{"command":"generate.object-on-path","params":{{"ids":[{ring}],"path_id":{ring}}}}}"#)))
        .is_err());
    assert!(e
        .run(cmd(&format!(r#"{{"command":"generate.replicate","params":{{"ids":[{dot}],"direction":"left","count":0}}}}"#)))
        .is_err());
    let _ = e.doc.get_shape(dot).unwrap().path.bounding_box();
}
