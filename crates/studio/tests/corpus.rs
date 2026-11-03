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

fn path_record(points: &[(f32, f32)]) -> Vec<u8> {
    let mut list = vec![1];
    list.extend_from_slice(&(points.len() as u32 - 1).to_le_bytes());
    for (i, &(x, y)) in points.iter().enumerate() {
        let mut p = Vec::new();
        p.extend_from_slice(&x.to_le_bytes());
        p.extend_from_slice(&y.to_le_bytes());
        p.extend_from_slice(&[0, 0, u8::from(i == 0), 0, 0]);
        list.extend(block(&p));
    }
    let mut outer = 0u32.to_le_bytes().to_vec();
    outer.extend(block(&list));
    let mut out = block(&outer);
    for v in [1f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// A v2 body: header, root record holding a filler record, the page record and paths.
fn v2(page: (f32, f32), paths: &[&[(f32, f32)]]) -> Vec<u8> {
    let mut page_payload = vec![0u8; 0x29];
    page_payload.extend_from_slice(&page.0.to_le_bytes());
    page_payload.extend_from_slice(&page.1.to_le_bytes());
    let mut root = block(&[0u8; 8]);
    root.extend(block(&page_payload));
    for p in paths {
        root.extend(path_record(p));
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
const VERIFIED: &[(&str, usize, [f64; 4])] = &[
    ("empty-v3.studio3", 0, [0.0; 4]),
    ("rect-1in-at-1-1", 1, [1.0, 1.0, 1.0, 1.0]),
    ("rect-1in-at-2-1", 1, [2.0, 1.0, 1.0, 1.0]),
    ("rect-2x1in-at-1-1", 1, [1.0, 1.0, 2.0, 1.0]),
    ("ellipse-2x1in-at-1-1", 1, [1.0, 1.0, 2.0, 1.0]),
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
    for &(stem, shapes, [x, y, w, h]) in VERIFIED {
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
            assert!((got.doc.board.width_mm - 304.8).abs() < 1e-3, "{name}");
            assert!((got.doc.board.height_mm - 304.8).abs() < 1e-3, "{name}");
            assert_eq!(got.doc.shape_count(), shapes, "{name}");
            let Some(r) = got.doc.layers[0].bounds() else {
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
