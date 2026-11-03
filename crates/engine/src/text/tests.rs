use super::*;
use kurbo::{ParamCurveNearest as _, Shape as _};
use vectorcraft_text::test_fonts::{VARIABLE_FAMILY, variable_font};

/// A font built in code from VectorCraft's test fonts (no font file here), loaded once.
fn font() -> FontSpec {
    static LOADED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    LOADED.get_or_init(|| {
        let bytes = variable_font().expect("test font builds");
        load_font_bytes(FontDb::global(), bytes).expect("test font loads");
    });
    FontSpec {
        family: VARIABLE_FAMILY.to_string(),
        style: "Regular".to_string(),
        file: None,
    }
}

fn pen() -> FontSpec {
    FontSpec {
        family: SINGLE_LINE_FAMILY.to_string(),
        style: "Regular".to_string(),
        file: None,
    }
}

fn spec(text: &str, font: FontSpec) -> TextSpec {
    TextSpec {
        text: text.to_string(),
        font,
        size: 72.0,
        x: 10.0,
        y: 20.0,
        alignment: TextAlign::Left,
        letter_spacing: 0.0,
        line_spacing: 100.0,
        kerning: true,
        vertical: false,
        weld: false,
        flow: TextFlow::Straight,
        letters: Vec::new(),
        transform: IDENTITY_TRANSFORM,
    }
}

fn contours(path: &BezPath) -> usize {
    path.elements()
        .iter()
        .filter(|e| matches!(e, PathEl::MoveTo(_)))
        .count()
}

fn add(engine: &mut Engine, spec: TextSpec) -> u64 {
    match engine
        .run(Command::TextAdd(TextAdd { spec, style: None }))
        .unwrap()
    {
        Effect::ShapeAdded { id } => id,
        other => panic!("{other:?}"),
    }
}

#[test]
fn outline_counts_for_known_strings() {
    // Source Sans glyphs: H, E, L one contour each; A, O and o two; B three; b two.
    for (text, expected) in [("HELLO", 6), ("Bob", 7), ("A B", 5)] {
        let path = layout(&spec(text, font()), "test").unwrap();
        assert_eq!(contours(&path), expected, "{text}");
    }
    // Single-line strokes: H is three strokes, I one; space draws nothing.
    let path = layout(&spec("HI I", pen()), "test").unwrap();
    assert_eq!(contours(&path), 5);
    assert!(
        path.elements()
            .iter()
            .all(|e| !matches!(e, PathEl::ClosePath))
    );
}

#[test]
fn spacing_changes_widths_monotonically() {
    for f in [font(), pen()] {
        let mut last = 0.0;
        for spacing in [-20.0, 0.0, 10.0, 50.0, 200.0] {
            let s = TextSpec {
                letter_spacing: spacing,
                ..spec("Wave", f.clone())
            };
            let w = layout(&s, "test").unwrap().bounding_box().width();
            assert!(w > last, "{} {spacing}: {w} after {last}", f.family);
            last = w;
        }
        let mut last = 0.0;
        for lines in [50.0, 100.0, 150.0] {
            let s = TextSpec {
                line_spacing: lines,
                ..spec("ab\ncd\nef", f.clone())
            };
            let h = layout(&s, "test").unwrap().bounding_box().height();
            assert!(h > last, "{} {lines}: {h} after {last}", f.family);
            last = h;
        }
    }
}

#[test]
fn weld_merges_overlapping_letters() {
    // At this spacing Source Sans 3's two n outlines overlap. Welding removes the seam.
    let tight = TextSpec {
        letter_spacing: -20.0,
        ..spec("nn", font())
    };
    assert_eq!(contours(&layout(&tight, "test").unwrap()), 2);
    let welded = TextSpec {
        weld: true,
        ..tight
    };
    assert_eq!(contours(&layout(&welded, "test").unwrap()), 1);

    // Two overlapping O outlines merge externally while retaining both counters.
    let overlapping_o = TextSpec {
        letter_spacing: -20.0,
        ..spec("OO", font())
    };
    assert_eq!(contours(&layout(&overlapping_o, "test").unwrap()), 4);
    let welded_o = TextSpec {
        weld: true,
        ..overlapping_o
    };
    assert_eq!(contours(&layout(&welded_o, "test").unwrap()), 3);
}

#[test]
fn kerning_alignment_and_vertical() {
    let width = |s: &TextSpec| layout(s, "test").unwrap().bounding_box().width();
    let kerned = spec("AVAVAV", font());
    let plain = TextSpec {
        kerning: false,
        ..kerned.clone()
    };
    assert!(width(&kerned) < width(&plain));
    for f in [font(), pen()] {
        let left = layout(&spec("Mid", f.clone()), "test")
            .unwrap()
            .bounding_box();
        for (align, expect) in [(TextAlign::Centre, 0.5), (TextAlign::Right, 1.0)] {
            let b = layout(
                &TextSpec {
                    alignment: align,
                    ..spec("Mid", f.clone())
                },
                "test",
            )
            .unwrap()
            .bounding_box();
            let shift = left.x0 - b.x0;
            assert!(
                (shift - expect * left.width()).abs() < 0.2 * left.width(),
                "{align:?}"
            );
        }
        let tall = layout(
            &TextSpec {
                vertical: true,
                ..spec("ABCD", f.clone())
            },
            "test",
        )
        .unwrap()
        .bounding_box();
        assert!(tall.height() > tall.width() * 2.0, "{} {tall:?}", f.family);
    }
    // Justify stretches the short line to the long one.
    let j = TextSpec {
        alignment: TextAlign::Justify,
        ..spec("ab\nabcdef\nab", pen())
    };
    let glyphs = straight(&j, "test").unwrap();
    let line_width = |l: usize| {
        let xs: Vec<f64> = glyphs
            .iter()
            .filter(|g| g.line == l)
            .map(|g| g.origin.x + g.advance)
            .collect();
        xs.iter().copied().fold(f64::MIN, f64::max)
    };
    assert!((line_width(0) - line_width(1)).abs() < 1e-9);
    assert!(line_width(2) < line_width(1));
}

fn distance_to(path: &BezPath, p: Point) -> f64 {
    path.segments()
        .map(|s| s.nearest(p, 1e-9).distance_sq.sqrt())
        .fold(f64::MAX, f64::min)
}

#[test]
fn text_on_a_path_keeps_letters_on_the_path() {
    let mut wave = BezPath::new();
    wave.move_to((0.0, 0.0));
    wave.curve_to((40.0, 60.0), (80.0, -60.0), (120.0, 0.0));
    for f in [font(), pen()] {
        for (flip, offset) in [(false, 0.0), (true, 0.0), (false, 2.0)] {
            let s = TextSpec {
                flow: TextFlow::OnPath {
                    path: wave.clone(),
                    shape: None,
                    start: 0.1,
                    offset_mm: offset,
                    flip,
                },
                size: 24.0,
                ..spec("Following", f.clone())
            };
            let glyphs = straight(&s, "test").unwrap();
            for (g, xf) in glyphs.iter().zip(placements(&s, &glyphs)) {
                // The baseline point under the letter's middle lies on the path.
                let foot = xf * Point::new(g.origin.x + g.advance / 2.0, 0.0);
                let d = distance_to(&wave, foot);
                assert!((d - offset).abs() < 0.05, "{} flip {flip}: {d}", f.family);
                // The letter is upright to the path: its up vector is the path's normal.
                assert!(((xf * Point::new(0.0, 1.0)) - (xf * Point::ZERO)).hypot() > 0.99);
            }
        }
    }
}

#[test]
fn curve_bends_one_line_round_a_circle() {
    for diameter in [80.0, -80.0] {
        let s = TextSpec {
            flow: TextFlow::Curve {
                diameter_mm: diameter,
            },
            alignment: TextAlign::Centre,
            ..spec("Rainbow", font())
        };
        let glyphs = straight(&s, "test").unwrap();
        let r = 40.0;
        let lo = glyphs.iter().map(|g| g.origin.x).fold(f64::MAX, f64::min);
        let hi = glyphs
            .iter()
            .map(|g| g.origin.x + g.advance)
            .fold(f64::MIN, f64::max);
        assert!(((lo + hi) / 2.0).abs() < 1.0, "centred on the anchor");
        let centre = Point::new(s.x + (lo + hi) / 2.0, s.y - diameter.signum() * r);
        for (g, xf) in glyphs.iter().zip(placements(&s, &glyphs)) {
            let foot = xf * Point::new(g.origin.x + g.advance / 2.0, 0.0);
            assert!((foot.distance(centre) - r).abs() < 1e-6);
        }
        let b = layout(&s, "test").unwrap().bounding_box();
        assert!(b.width() > 0.0);
    }
    // Curve needs one line.
    let two = TextSpec {
        flow: TextFlow::Curve { diameter_mm: 50.0 },
        ..spec("a\nb", font())
    };
    assert!(prepare(&Engine::default(), two, "test").is_err());
}

#[test]
fn font_loader_survives_a_corpus_of_broken_fonts() {
    let good = variable_font().unwrap();
    let db = FontDb::with_font_dirs(Vec::new());
    db.set_system_fallback(false);
    assert!(load_font_bytes(&db, Vec::new()).is_err());
    assert!(load_font_bytes(&db, b"not a font at all".to_vec()).is_err());
    assert!(load_font_bytes(&db, vec![0; 4096]).is_err());
    let mut corpus: Vec<Vec<u8>> = Vec::new();
    // Truncations, including inside the table directory and the glyph data.
    for cut in [
        4,
        12,
        64,
        200,
        1000,
        good.len() / 3,
        good.len() / 2,
        good.len() - 1,
    ] {
        corpus.push(good.get(..cut).unwrap().to_vec());
    }
    // Byte flips spread through the file (a fixed LCG: reproducible).
    let mut seed: u64 = 0x5eed;
    for _ in 0..48 {
        let mut bytes = good.clone();
        for _ in 0..8 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let i = (seed >> 33) as usize % bytes.len();
            bytes[i] ^= (seed >> 17) as u8 | 1;
        }
        corpus.push(bytes);
    }
    for (n, bytes) in corpus.into_iter().enumerate() {
        let local = FontDb::with_font_dirs(Vec::new());
        local.set_system_fallback(false);
        if let Ok(families) = load_font_bytes(&local, bytes) {
            // Whatever loads must also lay out without crashing.
            let family = families.first().cloned().unwrap_or_default();
            let mut object = TextObject::point(
                Point::ZERO,
                "Corpus 123",
                CharStyle {
                    font_family: family,
                    ..CharStyle::default()
                },
            );
            object.vertical = n % 2 == 1;
            let _ = vectorcraft_text::layout(&local, &object);
        }
    }
    // Font files: missing, a folder, and one that is not a font.
    assert!(load_font_file(&db, "/definitely/not/here.ttf").is_err());
    assert!(load_font_file(&db, env!("CARGO_MANIFEST_DIR")).is_err());
    assert!(load_font_file(&db, concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).is_err());
}

#[test]
fn font_files_load_by_path() {
    let dir = std::env::temp_dir().join(format!("stencilcraft-text-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("varitest.ttf");
    std::fs::write(&file, variable_font().unwrap()).unwrap();
    let mut engine = Engine::default();
    let id = add(
        &mut engine,
        spec(
            "File",
            FontSpec {
                family: String::new(),
                style: "Regular".to_string(),
                file: Some(file.to_string_lossy().into_owned()),
            },
        ),
    );
    let text = engine.doc.get_shape(id).unwrap().text.clone().unwrap();
    assert_eq!(text.font.family, VARIABLE_FAMILY);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn live_text_survives_moves_edits_undo_and_save() {
    let mut engine = Engine::default();
    let id = add(&mut engine, spec("Edit me", font()));
    let before = engine.doc.get_shape(id).unwrap().path.bounding_box();
    engine
        .run(Command::ShapeMove {
            id,
            dx_mm: 5.0,
            dy_mm: -3.0,
        })
        .unwrap();
    engine
        .run(Command::ShapeRotate { id, degrees: 30.0 })
        .unwrap();
    let turned = engine.doc.get_shape(id).unwrap().path.bounding_box();
    // Editing the words keeps the move and the turn.
    engine
        .run(Command::TextEdit(TextEdit {
            id,
            text: Some("Edit me".to_string()),
            ..TextEdit::default()
        }))
        .unwrap();
    let after = engine.doc.get_shape(id).unwrap();
    let b = after.path.bounding_box();
    assert!(
        (b.center() - turned.center()).hypot() < 0.01,
        "{b:?} {turned:?}"
    );
    assert!(after.text.as_ref().unwrap().transform != IDENTITY_TRANSFORM);
    engine
        .run(Command::TextEdit(TextEdit {
            id,
            text: Some("Edited text".to_string()),
            size: Some(36.0),
            ..TextEdit::default()
        }))
        .unwrap();
    assert_eq!(
        engine
            .doc
            .get_shape(id)
            .unwrap()
            .text
            .as_ref()
            .unwrap()
            .text,
        "Edited text"
    );
    // A plain move folds into x, y.
    let mut plain = Engine::default();
    let p = add(&mut plain, spec("Move", font()));
    plain
        .run(Command::ShapeMove {
            id: p,
            dx_mm: 7.0,
            dy_mm: 1.0,
        })
        .unwrap();
    plain
        .run(Command::TextEdit(TextEdit {
            id: p,
            letter_spacing: Some(5.0),
            ..TextEdit::default()
        }))
        .unwrap();
    let t = plain.doc.get_shape(p).unwrap().text.clone().unwrap();
    assert!((t.x - 17.0).abs() < 1e-6 && (t.y - 21.0).abs() < 1e-6);
    assert_eq!(t.transform, IDENTITY_TRANSFORM);

    // Undo goes back through the edits to the first layout.
    engine.run(Command::Undo).unwrap();
    engine.run(Command::Undo).unwrap();
    engine.run(Command::Undo).unwrap();
    engine.run(Command::Undo).unwrap();
    let back = engine.doc.get_shape(id).unwrap().path.bounding_box();
    assert!((back.x0 - before.x0).abs() < 1e-6);
    // Text objects round-trip through the file format.
    let json = engine.doc.save_json().unwrap();
    let loaded = stencilcraft_doc::Document::load_json(&json).unwrap();
    assert_eq!(
        loaded.get_shape(id).unwrap().text,
        engine.doc.get_shape(id).unwrap().text
    );

    // A node edit makes the text no longer match: text.edit refuses instead of losing it.
    let path = engine.doc.get_shape(id).unwrap().path.clone();
    let mut edited = path.clone();
    edited.line_to((500.0, 500.0));
    engine
        .run(Command::ShapeSetPath { id, path: edited })
        .unwrap();
    assert!(
        engine
            .run(Command::TextEdit(TextEdit {
                id,
                size: Some(20.0),
                ..TextEdit::default()
            }))
            .is_err()
    );
    engine
        .run(Command::TextToOutlines { ids: vec![id] })
        .unwrap();
    assert!(engine.doc.get_shape(id).unwrap().text.is_none());
    assert!(
        engine
            .run(Command::TextToOutlines { ids: vec![id] })
            .is_err()
    );
}

#[test]
fn ungroup_to_letters_and_lines_keeps_everything_in_place() {
    for f in [font(), pen()] {
        let mut engine = Engine::default();
        let mut s = spec("Ab c\nde", f.clone());
        s.letters = vec![LetterOffset {
            index: 1,
            dx_mm: 1.0,
            dy_mm: 2.0,
            rotate_deg: 15.0,
        }];
        let id = add(&mut engine, s);
        engine
            .run(Command::ShapeRotate { id, degrees: 10.0 })
            .unwrap();
        let whole = engine.doc.get_shape(id).unwrap().path.clone();
        let Effect::ShapesAdded { ids } = engine
            .run(Command::TextUngroup {
                id,
                to: UngroupTo::Lines,
            })
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], id);
        let joined = |engine: &Engine, ids: &[u64]| {
            let mut all = BezPath::new();
            for i in ids {
                all.extend(engine.doc.get_shape(*i).unwrap().path.iter());
            }
            all
        };
        let lines = joined(&engine, &ids);
        assert!(
            infer_affine(&whole, &lines)
                .is_some_and(|a| (a * Point::new(3.0, 4.0)).distance(Point::new(3.0, 4.0)) < 1e-3)
        );
        let Effect::ShapesAdded { ids: letters } = engine
            .run(Command::TextUngroup {
                id,
                to: UngroupTo::Letters,
            })
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(letters.len(), 3, "{}", f.family);
        let mut all = letters.clone();
        all.extend(&ids[1..]);
        let pieces = joined(&engine, &all);
        let (mut p, mut q) = (anchor_points(&whole), anchor_points(&pieces));
        let key = |a: &Point, b: &Point| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y));
        p.sort_by(key);
        q.sort_by(key);
        assert_eq!(p.len(), q.len());
        for (a, b) in p.iter().zip(&q) {
            assert!(a.distance(*b) < 1e-3, "{a:?} {b:?}");
        }
        // Each letter is live text of its own.
        let one = engine
            .doc
            .get_shape(letters[1])
            .unwrap()
            .text
            .clone()
            .unwrap();
        assert_eq!(one.text, "b");
        assert!(
            engine
                .run(Command::TextUngroup {
                    id: letters[1],
                    to: UngroupTo::Letters
                })
                .is_err()
        );
        engine.run(Command::Undo).unwrap();
        engine.run(Command::Undo).unwrap();
        assert_eq!(
            engine
                .doc
                .get_shape(id)
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .text,
            "Ab c\nde"
        );
    }
}

#[test]
fn on_path_from_a_shape_and_monogram_and_fonts() {
    let mut engine = Engine::default();
    let circle = engine
        .run(Command::ShapeAddEllipse {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 80.0,
        })
        .unwrap();
    let Effect::ShapeAdded { id: circle } = circle else {
        panic!()
    };
    let mut s = spec("Round and round", font());
    s.flow = TextFlow::OnPath {
        path: BezPath::new(),
        shape: Some(circle),
        start: 0.25,
        offset_mm: 1.0,
        flip: false,
    };
    let id = add(&mut engine, s);
    let text = engine.doc.get_shape(id).unwrap().text.clone().unwrap();
    assert!(matches!(&text.flow, TextFlow::OnPath { path, .. } if !path.elements().is_empty()));

    let Effect::ShapesAdded { ids } = engine
        .run(Command::TextMonogram(TextMonogram {
            initials: "ABC".to_string(),
            font: font(),
            size: 72.0,
            x: 50.0,
            y: 50.0,
            frame: MonogramFrame::Scallop,
            padding_mm: 3.0,
        }))
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(ids.len(), 2);
    let letters = engine.doc.get_shape(ids[0]).unwrap().path.bounding_box();
    let frame = engine.doc.get_shape(ids[1]).unwrap().path.bounding_box();
    assert!((letters.center() - Point::new(50.0, 50.0)).hypot() < 1e-6);
    assert!(frame.contains(letters.origin()) && frame.width() > letters.width());

    let undo = engine.can_undo();
    let Effect::Fonts { families } = engine
        .run(Command::TextFonts {
            query: String::new(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(engine.can_undo(), undo);
    assert!(families.first().is_some_and(|f| f.single_line));
    assert!(families.iter().any(|f| f.family == VARIABLE_FAMILY));
    let Effect::Fonts { families } = engine
        .run(Command::TextFonts {
            query: "single".to_string(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(
        families
            .iter()
            .all(|f| f.family.to_lowercase().contains("single"))
    );
}

#[test]
fn bad_text_input_is_refused_calmly() {
    let mut engine = Engine::default();
    let bad = [
        TextSpec {
            size: f64::NAN,
            ..spec("x", font())
        },
        TextSpec {
            size: -1.0,
            ..spec("x", font())
        },
        spec("   ", font()),
        spec(&"x".repeat(MAX_TEXT_BYTES + 1), font()),
        TextSpec {
            font: FontSpec {
                family: "No Such Family 1234".to_string(),
                style: "Regular".to_string(),
                file: None,
            },
            ..spec("x", font())
        },
        TextSpec {
            x: f64::INFINITY,
            ..spec("x", font())
        },
        TextSpec {
            transform: [0.0; 6],
            ..spec("x", font())
        },
        TextSpec {
            flow: TextFlow::OnPath {
                path: BezPath::new(),
                shape: Some(999),
                start: 0.0,
                offset_mm: 0.0,
                flip: false,
            },
            ..spec("x", font())
        },
        TextSpec {
            letters: vec![LetterOffset {
                index: 0,
                dx_mm: f64::NAN,
                ..LetterOffset::default()
            }],
            ..spec("x", font())
        },
    ];
    for s in bad {
        assert!(
            engine
                .run(Command::TextAdd(TextAdd {
                    spec: s.clone(),
                    style: None
                }))
                .is_err(),
            "{s:?}"
        );
    }
    assert_eq!(engine.doc.shape_count(), 0);
    assert!(!engine.can_undo());
    // A plain shape is not live text.
    let Effect::ShapeAdded { id } = engine
        .run(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 5.0,
            height: 5.0,
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(
        engine
            .run(Command::TextEdit(TextEdit {
                id,
                ..TextEdit::default()
            }))
            .is_err()
    );
    // Commands parse from the control protocol's JSON.
    let c: Command = serde_json::from_value(serde_json::json!({
        "command": "text.add",
        "params": {"text": "Hi", "font": {"family": SINGLE_LINE_FAMILY}, "size": 36, "x": 1, "y": 2,
                   "alignment": "centre", "flow": {"kind": "curve", "diameter_mm": 60}}
    }))
    .unwrap();
    let Effect::ShapeAdded { id } = engine.run(c).unwrap() else {
        panic!()
    };
    assert_eq!(engine.doc.get_shape(id).unwrap().style, CutStyle::Engrave);
}
