//! Silhouette Studio reader: decoding checks on documents we build ourselves from
//! the layout in docs/formats/silhouette-studio.md, plus a malformed-input corpus
//! (every truncation, deterministic bit flips) that must never panic.
//!
//! Files saved by Silhouette Studio itself are not committed (they carry vendor
//! preset text and a thumbnail). Put them under `corpora/silhouette/` at the
//! workspace root and the corpus tests mutate them too.

// Test-only file: helpers may fail loudly like tests do (see clippy.toml).
#![allow(clippy::unwrap_used)]

use stencilcraft_studio::{StudioError, Version, import};

fn block(payload: &[u8]) -> Vec<u8> {
    let mut out = vec![1];
    out.extend_from_slice(&(payload.len() as u32 + 4).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// One object outline in the page-coordinate (compact) layout the reader reads.
fn path_record(contours: &[(bool, &[(f32, f32)])]) -> Vec<u8> {
    let mut outer = (contours.len() as u32 - 1).to_le_bytes().to_vec();
    for &(closed, points) in contours {
        let mut c = vec![u8::from(closed)];
        c.extend_from_slice(&(points.len() as u32 - 1).to_le_bytes());
        for &(x, y) in points {
            c.extend_from_slice(&x.to_le_bytes());
            c.extend_from_slice(&y.to_le_bytes());
            c.extend_from_slice(&[0, 0]);
        }
        outer.extend_from_slice(&(c.len() as u32 + 4).to_le_bytes());
        outer.extend(c);
    }
    block(&outer)
}

/// A v2 body: header, root record holding a filler record, the page record and paths.
fn v2(page: (f32, f32), paths: &[&[(f32, f32)]]) -> Vec<u8> {
    let records: Vec<_> = paths.iter().map(|p| path_record(&[(true, p)])).collect();
    v2_records(page, &records)
}

fn v2_records(page: (f32, f32), records: &[Vec<u8>]) -> Vec<u8> {
    let mut page_payload = vec![0u8; 0x29];
    page_payload.extend_from_slice(&page.0.to_le_bytes());
    page_payload.extend_from_slice(&page.1.to_le_bytes());
    let mut root = block(&[0u8; 8]);
    root.extend(block(&page_payload));
    for r in records {
        root.extend_from_slice(r);
    }
    let mut out = b"silhouette04;".to_vec();
    out.extend(block(&root));
    out
}

fn v3(body_v2: &[u8]) -> Vec<u8> {
    let mut body = body_v2.to_vec();
    body[..13].copy_from_slice(b"silhouette05;");
    let thumb = b"not-a-real-thumbnail";
    let mut out = b"silhouette05;".to_vec();
    out.resize(1024, 0);
    out[0x10..0x18].copy_from_slice(&(body.len() as u64).to_le_bytes());
    out[0x18..0x20].copy_from_slice(&(thumb.len() as u64).to_le_bytes());
    out.extend(body);
    out.extend_from_slice(thumb);
    out
}

const RECT: &[(f32, f32)] = &[
    (62.38, 58.21),
    (62.38, 169.35),
    (201.3, 169.35),
    (201.3, 58.21),
];

fn bounds(bytes: &[u8]) -> (f64, f64, f64, f64) {
    let doc = import(bytes, "t").unwrap().doc;
    let r = doc.layers[0].bounds().unwrap();
    (r.x0, r.y0, r.x1, r.y1)
}

#[test]
fn v2_rectangle_decodes_in_page_millimetres() {
    let file = v2((304.8, 304.8), &[RECT]);
    let got = import(&file, "t").unwrap();
    assert_eq!(got.version, Version::V2);
    assert_eq!(got.doc.shape_count(), 1);
    assert!((got.doc.board.width_mm - 304.8).abs() < 1e-3);
    let (x0, y0, x1, y1) = bounds(&file);
    // y-up: file y 58.21..169.35 from the top of a 304.8 mm page.
    assert!((x0 - 62.38).abs() < 1e-3 && (y0 - (304.8 - 169.35)).abs() < 1e-3);
    assert!((x1 - 201.3).abs() < 1e-3 && (y1 - (304.8 - 58.21)).abs() < 1e-3);
}

#[test]
fn v3_container_reads_section_zero() {
    let file = v3(&v2(
        (215.9, 279.4),
        &[RECT, &[(1.0, 1.0), (2.0, 1.0), (2.0, 2.0)]],
    ));
    let got = import(&file, "t").unwrap();
    assert_eq!(got.version, Version::V3);
    assert_eq!(got.doc.shape_count(), 2);
    assert!((got.doc.board.height_mm - 279.4).abs() < 1e-3);
}

#[test]
fn open_paths_stay_open_and_compound_contours_stay_one_shape() {
    let line = path_record(&[(false, &[(25.4, 25.4), (76.2, 50.8)])]);
    let compound = path_record(&[(true, RECT), (true, &[(1.0, 1.0), (2.0, 1.0), (2.0, 2.0)])]);
    let doc = import(&v2_records((304.8, 304.8), &[line, compound]), "t")
        .unwrap()
        .doc;
    assert_eq!(doc.shape_count(), 2);
    let shapes = &doc.layers[0].shapes;
    let closes = |i: usize| {
        shapes[i]
            .path
            .elements()
            .iter()
            .filter(|e| matches!(e, kurbo::PathEl::ClosePath))
            .count()
    };
    assert_eq!(closes(0), 0, "a line is open");
    assert_eq!(closes(1), 2, "both contours of the compound path close");
}

/// An object style as Silhouette writes it: flag, line colour as B, G, R, A, 8 bytes,
/// then the cut-action record (`01 L`, name "cut").
fn style(rgb: [u8; 3]) -> Vec<u8> {
    let mut out = vec![1];
    for c in [rgb[2], rgb[1], rgb[0], 255] {
        out.extend_from_slice(&(f32::from(c) / 255.0).to_le_bytes());
    }
    out.extend_from_slice(&[0; 8]);
    let mut action = 3u32.to_le_bytes().to_vec();
    action.extend_from_slice(b"cut");
    out.extend(block(&action));
    out
}

#[test]
fn line_colours_become_one_layer_each() {
    let ochre = [0xb8, 0x7c, 0x1b];
    let blue = [0, 0, 0xff];
    let mut records = Vec::new();
    for rgb in [ochre, blue, ochre] {
        let mut object = style(rgb);
        object.extend(path_record(&[(true, RECT)]));
        records.push(object);
    }
    let doc = import(&v2_records((304.8, 304.8), &records), "t")
        .unwrap()
        .doc;
    let layers: Vec<_> = doc
        .layers
        .iter()
        .map(|l| (l.color.clone().unwrap_or_default(), l.shapes.len()))
        .collect();
    assert_eq!(
        layers,
        [("#b87c1b".to_string(), 2), ("#0000ff".to_string(), 1)]
    );
}

#[test]
fn empty_document_imports_with_no_shapes() {
    let got = import(&v2((304.8, 304.8), &[]), "t").unwrap();
    assert_eq!(got.doc.shape_count(), 0);
}

#[test]
fn unknown_body_in_a_v3_container_is_rejected_as_protected() {
    let mut file = v3(&v2((304.8, 304.8), &[RECT]));
    for b in &mut file[1024..1024 + 40] {
        *b ^= 0x5a;
    }
    assert!(matches!(import(&file, "t"), Err(StudioError::Protected(_))));
}

#[test]
fn foreign_and_unsupported_headers_are_rejected() {
    assert!(matches!(
        import(b"<svg/>", "t"),
        Err(StudioError::NotStudio(_))
    ));
    assert!(matches!(
        import(b"silhouette03;\x01\x04\0\0\0", "t"),
        Err(StudioError::Unsupported { .. })
    ));
}

#[test]
fn bad_coordinates_are_never_imported() {
    let file = v2((304.8, 304.8), &[&[(f32::NAN, 0.0), (1.0, 1.0)], RECT]);
    let got = import(&file, "t").unwrap();
    // A non-finite coordinate fails the record check: the path is not a match.
    assert_eq!(got.doc.shape_count(), 1);
}

fn seeds() -> Vec<Vec<u8>> {
    let mut out = vec![
        v2((304.8, 304.8), &[RECT]),
        v3(&v2((304.8, 304.8), &[RECT, RECT])),
    ];
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpora/silhouette");
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if stencilcraft_studio::is_studio_path(&entry.path().to_string_lossy()) {
                out.push(std::fs::read(entry.path()).unwrap());
            }
        }
    }
    out
}

fn exercise(bytes: &[u8]) {
    if let Ok(got) = import(bytes, "mutant") {
        got.doc.validate().unwrap();
    }
}

#[test]
fn every_truncation_never_panics() {
    for seed in seeds() {
        // Long real files: every byte near the structure, then a stride.
        let step = (seed.len() / 4000).max(1);
        for cut in (0..seed.len()).step_by(step) {
            exercise(&seed[..cut]);
        }
    }
}

#[test]
fn bit_flips_never_panic() {
    let mut state: u64 = 0x5111_0E77;
    for seed in seeds() {
        for _ in 0..4000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let mut mutant = seed.clone();
            let pos = (state as usize) % mutant.len();
            mutant[pos] ^= 1 << (state >> 61);
            exercise(&mutant);
        }
        // Every byte of the structural prefix, one bit each.
        for pos in 0..seed.len().min(4096) {
            let mut mutant = seed.clone();
            mutant[pos] ^= 1 << (pos % 8);
            exercise(&mutant);
        }
    }
}

/// Samples checked on screen in Silhouette Studio before saving: name, shape count,
/// and the bounding box X, Y, W, H in inches from the Transform panel.
/// Samples checked on screen: page (in), shape count, and the bounds the app showed
/// (X, Y from the page top, W, H in inches; `None` where only the count was checked).
type Verified = (&'static str, (f64, f64), usize, Option<[f64; 4]>);
const VERIFIED: &[Verified] = &[
    ("empty-v3.studio3", (12.0, 12.0), 0, None),
    (
        "rect-1in-at-1-1",
        (12.0, 12.0),
        1,
        Some([1.0, 1.0, 1.0, 1.0]),
    ),
    (
        "rect-1in-at-2-1",
        (12.0, 12.0),
        1,
        Some([2.0, 1.0, 1.0, 1.0]),
    ),
    (
        "rect-2x1in-at-1-1",
        (12.0, 12.0),
        1,
        Some([1.0, 1.0, 2.0, 1.0]),
    ),
    (
        "ellipse-2x1in-at-1-1",
        (12.0, 12.0),
        1,
        Some([1.0, 1.0, 2.0, 1.0]),
    ),
    // 2026-11-03 samples (SS 5.0.539, Tiny11 VM).
    (
        "rect-1in-rot45",
        (12.0, 12.0),
        1,
        Some([0.793, 0.793, 1.414, 1.414]),
    ),
    (
        "rect-1in-line-b87c1b-fill-54a923",
        (12.0, 12.0),
        1,
        Some([1.0, 1.0, 1.0, 1.0]),
    ),
    (
        "line-1-1-to-3-2",
        (12.0, 12.0),
        1,
        Some([1.0, 1.0, 2.0, 1.0]),
    ),
    ("page-10x7in-empty", (10.0, 7.0), 0, None),
    (
        "two-rects",
        (10.0, 7.0),
        2,
        Some([2.097, 0.706, 1.924, 0.7]),
    ),
    (
        "two-rects-grouped",
        (10.0, 7.0),
        2,
        Some([2.097, 0.706, 1.924, 0.7]),
    ),
    (
        "two-rects-compound",
        (10.0, 7.0),
        1,
        Some([2.097, 0.706, 1.924, 0.7]),
    ),
    ("text-A-arial-72pt", (10.0, 7.0), 1, None),
];

/// The flip, on an asymmetric sample: 1 in from the left and 1 in from the TOP.
#[test]
fn rect_1in_at_1_1_is_one_inch_from_the_page_top_in_y_up() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpora/silhouette");
    let Ok(bytes) = std::fs::read(format!("{dir}/rect-1in-at-1-1.studio3")) else {
        return;
    };
    let doc = import(&bytes, "rect").unwrap().doc;
    let r = doc.layers[0].bounds().unwrap();
    assert!((r.x0 - 25.4).abs() < 0.05, "{r:?}");
    assert!((r.y1 - (304.8 - 25.4)).abs() < 0.05, "top edge {r:?}");
    assert!((r.y0 - (304.8 - 50.8)).abs() < 0.05, "bottom edge {r:?}");
}

#[test]
fn real_samples_decode_when_present() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpora/silhouette");
    for &(stem, (page_w, page_h), shapes, bounds) in VERIFIED {
        let names = if stem.contains('.') {
            vec![stem.to_string()]
        } else {
            vec![format!("{stem}.studio"), format!("{stem}.studio3")]
        };
        for name in names {
            let Ok(bytes) = std::fs::read(format!("{dir}/{name}")) else {
                continue;
            };
            let got = import(&bytes, &name).unwrap();
            assert!(
                (got.doc.board.width_mm - page_w * 25.4).abs() < 1e-3,
                "{name}"
            );
            assert!(
                (got.doc.board.height_mm - page_h * 25.4).abs() < 1e-3,
                "{name}"
            );
            assert_eq!(got.doc.shape_count(), shapes, "{name}");
            if let Some(hex) = stem.strip_prefix("rect-1in-line-").and_then(|s| s.get(..6)) {
                let colours: Vec<_> = got
                    .doc
                    .layers
                    .iter()
                    .filter_map(|l| l.color.clone())
                    .collect();
                assert_eq!(colours, [format!("#{hex}")], "{name}");
            }
            let (Some([x, y, w, h]), Some(r)) = (bounds, got.doc.layers[0].bounds()) else {
                continue;
            };
            // Documents are y-up: Silhouette's Y (from the page top) is page height - y1.
            let top = got.doc.board.height_mm - r.y1;
            let got_in = [r.x0, top, r.width(), r.height()].map(|v| v / 25.4);
            for (g, want) in got_in.iter().zip([x, y, w, h]) {
                assert!((g - want).abs() < 0.002, "{name}: {got_in:?}");
            }
        }
    }
}
