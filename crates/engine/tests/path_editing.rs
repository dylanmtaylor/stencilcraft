//! Path editing, compound, pathfinder extras, flags and queries: geometry, node counts,
//! undo, and calm errors on bad input.

use kurbo::{BezPath, ParamCurveNearest, PathEl, Point, Shape as _};
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::{
    BooleanOperation, Command, Effect, Engine, NodeAxis, NodeCount, NodeRef, NodeType, PathEnd,
    SegmentType, StrokeCap, StrokeJoin,
};

fn polyline(points: &[(f64, f64)], closed: bool) -> BezPath {
    let mut p = BezPath::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        if i == 0 {
            p.move_to((x, y));
        } else {
            p.line_to((x, y));
        }
    }
    if closed {
        p.close_path();
    }
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

fn rect(engine: &mut Engine, x: f64, y: f64, w: f64, h: f64) -> u64 {
    match engine
        .run(Command::ShapeAddRect {
            x,
            y,
            width: w,
            height: h,
        })
        .unwrap()
    {
        Effect::ShapeAdded { id } => id,
        other => panic!("unexpected {other:?}"),
    }
}

fn count(engine: &mut Engine, ids: &[u64]) -> NodeCount {
    match engine
        .run(Command::QueryNodeCount { ids: ids.to_vec() })
        .unwrap()
    {
        Effect::NodeCount(c) => c,
        other => panic!("unexpected {other:?}"),
    }
}

fn path_of(engine: &Engine, id: u64) -> BezPath {
    engine.doc.get_shape(id).unwrap().path.clone()
}

fn closes(path: &BezPath) -> usize {
    path.elements()
        .iter()
        .filter(|e| matches!(e, PathEl::ClosePath))
        .count()
}

fn n(contour: usize, node: usize) -> NodeRef {
    NodeRef { contour, node }
}

/// Runs `command`, then checks undo puts the document back exactly.
fn run_undoable(engine: &mut Engine, command: Command) -> Effect {
    let before = engine.doc.clone();
    let effect = engine.run(command).unwrap();
    assert_ne!(engine.doc, before, "command changed nothing");
    engine.run(Command::Undo).unwrap();
    assert_eq!(engine.doc, before, "undo did not restore");
    engine.run(Command::Redo).unwrap();
    effect
}

#[test]
fn break_closed_opens_and_open_splits() {
    let mut e = Engine::default();
    let square = add(
        &mut e,
        polyline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true),
    );
    run_undoable(
        &mut e,
        Command::PathBreak {
            id: square,
            contour: 0,
            node: 2,
        },
    );
    let c = count(&mut e, &[square]);
    // Opening a closed contour repeats the break node at both ends.
    assert_eq!((c.contours, c.nodes), (1, 5));
    assert_eq!(closes(&path_of(&e, square)), 0);

    let line = add(
        &mut e,
        polyline(&[(0.0, 20.0), (5.0, 20.0), (10.0, 20.0)], false),
    );
    e.run(Command::PathBreak {
        id: line,
        contour: 0,
        node: 1,
    })
    .unwrap();
    assert_eq!(count(&mut e, &[line]).contours, 2);
    // An open end cannot be broken, nor can a node that is not there.
    for (contour, node) in [(0, 0), (0, 9), (5, 0)] {
        assert!(
            e.run(Command::PathBreak {
                id: line,
                contour,
                node
            })
            .is_err()
        );
    }
    assert!(
        e.run(Command::PathBreak {
            id: 999,
            contour: 0,
            node: 1
        })
        .is_err()
    );
}

#[test]
fn join_nearest_and_specified_ends() {
    let mut e = Engine::default();
    let a = add(&mut e, polyline(&[(0.0, 0.0), (10.0, 0.0)], false));
    let b = add(&mut e, polyline(&[(20.0, 0.0), (10.0, 0.0)], false));
    run_undoable(
        &mut e,
        Command::PathJoin {
            a,
            a_contour: 0,
            b,
            b_contour: 0,
            a_end: None,
            b_end: None,
        },
    );
    // Coincident ends merge: three nodes, b consumed.
    assert!(e.doc.get_shape(b).is_none());
    assert_eq!(count(&mut e, &[a]).nodes, 3);
    let bounds = path_of(&e, a).bounding_box();
    assert_eq!((bounds.x0, bounds.x1), (0.0, 20.0));

    let c = add(&mut e, polyline(&[(0.0, 30.0), (10.0, 30.0)], false));
    let d = add(&mut e, polyline(&[(0.0, 40.0), (10.0, 40.0)], false));
    e.run(Command::PathJoin {
        a: c,
        a_contour: 0,
        b: d,
        b_contour: 0,
        a_end: Some(PathEnd::Start),
        b_end: Some(PathEnd::Start),
    })
    .unwrap();
    // Start-to-start: a connecting line, four nodes running (10,30)→(0,30)→(0,40)→(10,40).
    let p = path_of(&e, c);
    assert_eq!(count(&mut e, &[c]).nodes, 4);
    assert_eq!(p.elements()[0], PathEl::MoveTo(Point::new(10.0, 30.0)));
    assert_eq!(p.elements()[3], PathEl::LineTo(Point::new(10.0, 40.0)));

    let closed = rect(&mut e, 50.0, 50.0, 5.0, 5.0);
    assert!(
        e.run(Command::PathJoin {
            a: c,
            a_contour: 0,
            b: closed,
            b_contour: 0,
            a_end: None,
            b_end: None
        })
        .is_err()
    );
    assert!(
        e.run(Command::PathJoin {
            a: c,
            a_contour: 0,
            b: c,
            b_contour: 0,
            a_end: None,
            b_end: None
        })
        .is_err()
    );
}

#[test]
fn close_adds_segment_or_merges_ends() {
    let mut e = Engine::default();
    let u = add(
        &mut e,
        polyline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], false),
    );
    run_undoable(&mut e, Command::PathClose { id: u, contour: 0 });
    assert_eq!(closes(&path_of(&e, u)), 1);
    assert_eq!(count(&mut e, &[u]).nodes, 3);
    assert!(e.run(Command::PathClose { id: u, contour: 0 }).is_err());

    let ring = add(
        &mut e,
        polyline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 0.0)], false),
    );
    e.run(Command::PathClose {
        id: ring,
        contour: 0,
    })
    .unwrap();
    assert_eq!(count(&mut e, &[ring]).nodes, 3);
}

#[test]
fn average_and_merge_nodes() {
    let mut e = Engine::default();
    let id = add(
        &mut e,
        polyline(&[(0.0, 0.0), (10.0, 2.0), (20.0, 4.0), (30.0, 0.0)], false),
    );
    run_undoable(
        &mut e,
        Command::PathAverageNodes {
            id,
            nodes: vec![n(0, 1), n(0, 2)],
            axis: NodeAxis::Horizontal,
        },
    );
    let p = path_of(&e, id);
    assert_eq!(p.elements()[1], PathEl::LineTo(Point::new(10.0, 3.0)));
    assert_eq!(p.elements()[2], PathEl::LineTo(Point::new(20.0, 3.0)));

    run_undoable(
        &mut e,
        Command::PathMergeNodes {
            id,
            nodes: vec![n(0, 1), n(0, 2)],
        },
    );
    assert_eq!(count(&mut e, &[id]).nodes, 3);
    assert_eq!(
        path_of(&e, id).elements()[1],
        PathEl::LineTo(Point::new(15.0, 3.0))
    );
    assert!(
        e.run(Command::PathMergeNodes {
            id,
            nodes: vec![n(0, 1)]
        })
        .is_err()
    );
    assert!(
        e.run(Command::PathAverageNodes {
            id,
            nodes: vec![n(0, 40)],
            axis: NodeAxis::Both
        })
        .is_err()
    );
}

#[test]
fn delete_nodes_keeps_shape_or_refuses_to_empty_it() {
    let mut e = Engine::default();
    let id = rect(&mut e, 0.0, 0.0, 10.0, 10.0);
    run_undoable(
        &mut e,
        Command::PathDeleteNodes {
            id,
            nodes: vec![n(0, 1)],
        },
    );
    let c = count(&mut e, &[id]);
    assert_eq!(c.nodes, 3);
    // Triangle (0,0) (10,10) (0,10): half the square.
    assert!((path_of(&e, id).area().abs() - 50.0).abs() < 1e-9);
    assert!(
        e.run(Command::PathDeleteNodes {
            id,
            nodes: vec![n(0, 0), n(0, 1), n(0, 2)]
        })
        .is_err()
    );
    assert!(
        e.run(Command::PathDeleteNodes { id, nodes: vec![] })
            .is_err()
    );
}

#[test]
fn convert_nodes_and_segments() {
    let mut e = Engine::default();
    let id = rect(&mut e, 0.0, 0.0, 10.0, 10.0);
    assert_eq!(count(&mut e, &[id]).bezier, 0);
    run_undoable(
        &mut e,
        Command::PathSetNodeType {
            id,
            nodes: vec![n(0, 1)],
            kind: NodeType::Smooth,
        },
    );
    let c = count(&mut e, &[id]);
    assert_eq!((c.bezier, c.linear), (1, 3));
    e.run(Command::PathSetNodeType {
        id,
        nodes: vec![n(0, 1)],
        kind: NodeType::Corner,
    })
    .unwrap();
    assert_eq!(count(&mut e, &[id]).bezier, 0);

    run_undoable(
        &mut e,
        Command::PathSetSegment {
            id,
            contour: 0,
            segment: 0,
            kind: SegmentType::Curve,
        },
    );
    assert_eq!(count(&mut e, &[id]).bezier, 2);
    // The curve's handles sit on the old line, so the outline is unchanged.
    assert!((path_of(&e, id).area().abs() - 100.0).abs() < 1e-9);
    e.run(Command::PathSetSegment {
        id,
        contour: 0,
        segment: 0,
        kind: SegmentType::Line,
    })
    .unwrap();
    assert_eq!(count(&mut e, &[id]).bezier, 0);
    assert!(
        e.run(Command::PathSetSegment {
            id,
            contour: 0,
            segment: 4,
            kind: SegmentType::Line
        })
        .is_err()
    );
}

/// Largest distance from points along `a` to the curve `b`.
fn deviation(a: &BezPath, b: &BezPath) -> f64 {
    let segs: Vec<_> = b.segments().collect();
    let mut worst: f64 = 0.0;
    for seg in a.segments() {
        for i in 0..=50 {
            let p = kurbo::ParamCurve::eval(&seg, i as f64 / 50.0);
            let d = segs
                .iter()
                .map(|s| s.nearest(p, 1e-9).distance_sq.sqrt())
                .fold(f64::INFINITY, f64::min);
            worst = worst.max(d);
        }
    }
    worst
}

#[test]
fn simplify_reduces_nodes_within_tolerance() {
    let mut e = Engine::default();
    // A densely sampled sine wave: 201 nodes.
    let points: Vec<(f64, f64)> = (0..=200)
        .map(|i| {
            let x = i as f64 * 0.5;
            (x, 20.0 + 10.0 * (x / 10.0).sin())
        })
        .collect();
    let original = polyline(&points, false);
    let id = add(&mut e, original.clone());
    let before = count(&mut e, &[id]).nodes;
    let tolerance = 0.1;
    run_undoable(
        &mut e,
        Command::PathSimplify {
            ids: vec![id],
            tolerance_mm: tolerance,
        },
    );
    let after = count(&mut e, &[id]).nodes;
    assert!(after * 5 < before, "{before} -> {after} nodes");
    let simplified = path_of(&e, id);
    let d = deviation(&simplified, &original).max(deviation(&original, &simplified));
    assert!(
        d <= tolerance * 1.5,
        "deviation {d} over tolerance {tolerance}"
    );
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            e.run(Command::PathSimplify {
                ids: vec![id],
                tolerance_mm: bad
            })
            .is_err()
        );
    }
}

#[test]
fn outline_stroke_makes_a_closed_band() {
    let mut e = Engine::default();
    let id = add(&mut e, polyline(&[(0.0, 10.0), (40.0, 10.0)], false));
    run_undoable(
        &mut e,
        Command::PathOutlineStroke {
            ids: vec![id],
            width_mm: 2.0,
            join: StrokeJoin::Miter,
            cap: StrokeCap::Butt,
            miter_limit: None,
        },
    );
    let p = path_of(&e, id);
    assert_eq!(closes(&p), 1);
    assert!((p.area().abs() - 80.0).abs() < 1e-6, "{}", p.area());
    let b = p.bounding_box();
    assert!((b.y0 - 9.0).abs() < 1e-6 && (b.y1 - 11.0).abs() < 1e-6);

    let square_cap = add(&mut e, polyline(&[(0.0, 30.0), (40.0, 30.0)], false));
    e.run(Command::PathOutlineStroke {
        ids: vec![square_cap],
        width_mm: 2.0,
        join: StrokeJoin::Round,
        cap: StrokeCap::Square,
        miter_limit: None,
    })
    .unwrap();
    // Square caps extend half the width past each end: 42 × 2.
    assert!((path_of(&e, square_cap).area().abs() - 84.0).abs() < 1e-6);
    for width in [0.0, -1.0, f64::NAN] {
        assert!(
            e.run(Command::PathOutlineStroke {
                ids: vec![id],
                width_mm: width,
                join: StrokeJoin::Bevel,
                cap: StrokeCap::Round,
                miter_limit: None
            })
            .is_err()
        );
    }
}

#[test]
fn star_converts_to_plain_path_with_same_geometry() {
    let mut e = Engine::default();
    let Effect::ShapeAdded { id } = e
        .run(Command::ShapeAddStar {
            cx: 50.0,
            cy: 50.0,
            radius: 20.0,
            inner_radius: 8.0,
            points: 5,
            rotation: 0.0,
        })
        .unwrap()
    else {
        panic!("no star")
    };
    let before = path_of(&e, id);
    e.run(Command::ShapesToPath { ids: vec![id] }).unwrap();
    let after = path_of(&e, id);
    assert_eq!(count(&mut e, &[id]).nodes, 10);
    assert!((before.area() - after.area()).abs() < 1e-6);
    e.run(Command::Undo).unwrap();
    assert_eq!(path_of(&e, id), before);
    assert!(e.run(Command::ShapesToPath { ids: vec![] }).is_err());
    assert!(e.run(Command::ShapesToPath { ids: vec![77] }).is_err());
}

#[test]
fn make_and_release_compound() {
    let mut e = Engine::default();
    let outer = rect(&mut e, 0.0, 0.0, 30.0, 30.0);
    let inner = rect(&mut e, 10.0, 10.0, 10.0, 10.0);
    let other = rect(&mut e, 50.0, 0.0, 5.0, 5.0);
    run_undoable(
        &mut e,
        Command::ShapesMakeCompound {
            ids: vec![inner, outer],
        },
    );
    assert_eq!(e.doc.shape_count(), 2);
    assert!(e.doc.get_shape(inner).is_none());
    let c = count(&mut e, &[outer]);
    assert_eq!((c.shapes, c.contours, c.nodes), (1, 2, 8));

    let effect = run_undoable(&mut e, Command::ShapesReleaseCompound { ids: vec![outer] });
    let Effect::ShapesAdded { ids } = effect else {
        panic!("{effect:?}")
    };
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], outer);
    assert_eq!(e.doc.shape_count(), 3);
    assert_eq!(count(&mut e, &[]).contours, 3);
    assert!(
        e.run(Command::ShapesReleaseCompound { ids: vec![other] })
            .is_err()
    );
    assert!(
        e.run(Command::ShapesMakeCompound { ids: vec![other] })
            .is_err()
    );
    assert!(
        e.run(Command::ShapesMakeCompound {
            ids: vec![other, 404]
        })
        .is_err()
    );
}

#[test]
fn subtract_all_removes_every_front_shape_from_the_bottom_one() {
    let mut e = Engine::default();
    let base = rect(&mut e, 0.0, 0.0, 30.0, 10.0);
    let a = rect(&mut e, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&mut e, 20.0, 0.0, 10.0, 10.0);
    e.run(Command::ShapePathfinder {
        ids: vec![base, a, b],
        operation: BooleanOperation::SubtractAll,
    })
    .unwrap();
    assert_eq!(e.doc.shape_count(), 1);
    let (_, shape) = e.doc.shapes().next().unwrap();
    assert!((shape.path.area().abs() - 100.0).abs() < 1e-6);
    let b = shape.bounds().unwrap();
    assert!((b.x0 - 10.0).abs() < 1e-6 && (b.x1 - 20.0).abs() < 1e-6);
    e.run(Command::Undo).unwrap();
    assert_eq!(e.doc.shape_count(), 3);
}

#[test]
fn locked_shapes_reject_edits_and_hidden_ones_leave_selections() {
    let mut e = Engine::default();
    let a = rect(&mut e, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&mut e, 20.0, 0.0, 10.0, 10.0);
    let line = add(&mut e, polyline(&[(0.0, 50.0), (9.0, 50.0)], false));
    run_undoable(
        &mut e,
        Command::ShapesSetLocked {
            ids: vec![a],
            locked: true,
        },
    );
    assert!(
        e.run(Command::ShapeMove {
            id: a,
            dx_mm: 1.0,
            dy_mm: 0.0
        })
        .is_err()
    );
    assert!(e.run(Command::ShapeRemove { id: a }).is_err());
    // Flags survive an edit that round-trips through the VectorCraft session.
    e.run(Command::ShapeMove {
        id: b,
        dx_mm: 1.0,
        dy_mm: 0.0,
    })
    .unwrap();
    assert!(e.doc.get_shape(a).unwrap().locked);

    let selection = |e: &mut Engine, c: Command| match e.run(c).unwrap() {
        Effect::Selection { ids } => ids,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        selection(&mut e, Command::SelectSameFill { id: b }),
        vec![b]
    );
    assert_eq!(
        selection(&mut e, Command::SelectSameStroke { id: b }),
        vec![b, line]
    );
    assert_eq!(
        selection(&mut e, Command::SelectInverse { ids: vec![b] }),
        vec![line]
    );

    e.run(Command::ShapesSetHidden {
        ids: vec![line],
        hidden: true,
    })
    .unwrap();
    assert_eq!(
        selection(&mut e, Command::SelectInverse { ids: vec![] }),
        vec![b]
    );
    // Queries do not enter the history.
    let undo_before = e.can_undo();
    e.run(Command::QueryNodeCount { ids: vec![] }).unwrap();
    assert_eq!(e.can_undo(), undo_before);
    e.run(Command::ShapesSetLocked {
        ids: vec![a],
        locked: false,
    })
    .unwrap();
    e.run(Command::ShapeRemove { id: a }).unwrap();
    assert!(
        e.run(Command::ShapesSetHidden {
            ids: vec![999],
            hidden: true
        })
        .is_err()
    );
    assert!(e.run(Command::SelectSameFill { id: 999 }).is_err());
}

#[test]
fn node_count_reports_linear_and_bezier_nodes() {
    let mut e = Engine::default();
    rect(&mut e, 0.0, 0.0, 10.0, 10.0);
    let Effect::ShapeAdded { id: circle } = e
        .run(Command::ShapeAddEllipse {
            x: 20.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        })
        .unwrap()
    else {
        panic!("no ellipse")
    };
    let all = count(&mut e, &[]);
    assert_eq!((all.shapes, all.contours), (2, 2));
    let c = count(&mut e, &[circle]);
    assert_eq!(c.linear, 0);
    assert_eq!(c.bezier, c.nodes);
    assert_eq!(all.linear, 4);
    assert!(e.run(Command::QueryNodeCount { ids: vec![12345] }).is_err());
}

#[test]
fn new_commands_parse_from_json() {
    let cases = [
        r#"{"command":"path.break","params":{"id":1,"contour":0,"node":2}}"#,
        r#"{"command":"path.join","params":{"a":1,"b":2}}"#,
        r#"{"command":"path.join","params":{"a":1,"b":2,"a_end":"start","b_end":"end"}}"#,
        r#"{"command":"path.close","params":{"id":1}}"#,
        r#"{"command":"path.average-nodes","params":{"id":1,"nodes":[{"contour":0,"node":1}],"axis":"both"}}"#,
        r#"{"command":"path.merge-nodes","params":{"id":1,"nodes":[]}}"#,
        r#"{"command":"path.delete-nodes","params":{"id":1,"nodes":[]}}"#,
        r#"{"command":"path.set-node-type","params":{"id":1,"nodes":[],"kind":"smooth"}}"#,
        r#"{"command":"path.set-segment","params":{"id":1,"contour":0,"segment":0,"kind":"line"}}"#,
        r#"{"command":"path.simplify","params":{"ids":[1],"tolerance_mm":0.1}}"#,
        r#"{"command":"path.outline-stroke","params":{"ids":[1],"width_mm":1,"join":"round","cap":"square"}}"#,
        r#"{"command":"shapes.to-path","params":{"ids":[1]}}"#,
        r#"{"command":"shapes.make-compound","params":{"ids":[1,2]}}"#,
        r#"{"command":"shapes.release-compound","params":{"ids":[1]}}"#,
        r#"{"command":"shapes.set-locked","params":{"ids":[1],"locked":true}}"#,
        r#"{"command":"shapes.set-hidden","params":{"ids":[1],"hidden":true}}"#,
        r#"{"command":"query.node-count","params":{}}"#,
        r#"{"command":"select.same-fill","params":{"id":1}}"#,
        r#"{"command":"select.same-stroke","params":{"id":1}}"#,
        r#"{"command":"select.inverse","params":{"ids":[1]}}"#,
        r#"{"command":"shape.pathfinder","params":{"ids":[1,2],"operation":"subtract-all"}}"#,
    ];
    for case in cases {
        serde_json::from_str::<Command>(case).unwrap_or_else(|e| panic!("{case}: {e}"));
    }
}
