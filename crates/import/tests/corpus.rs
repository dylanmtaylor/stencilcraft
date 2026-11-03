//! Untrusted-input corpus for every importer: deterministic mutations (truncation,
//! byte substitution, insertion) of a valid file plus hand-written pathological
//! inputs. Accepting or rejecting is fine; panicking is not, and whatever loads must
//! validate and re-save.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::Cursor;

use stencilcraft_doc::{CutStyle, Document};
use stencilcraft_import::{ImportOptions, import_bytes};

fn exercise(bytes: &[u8], name: &str) {
    for options in [
        ImportOptions::default(),
        ImportOptions {
            fit_to_page: true,
            join: false,
            ..ImportOptions::default()
        },
    ] {
        if let Ok(doc) = import_bytes(bytes, name, &options) {
            doc.validate().expect("an imported document validates");
            doc.save_json().expect("an imported document re-saves");
        }
    }
}

/// xorshift-driven mutants: `count` substitutions and insertions, plus truncations
/// every `stride` bytes.
fn mutate(seed: &[u8], name: &str, count: usize, stride: usize) {
    for cut in (0..seed.len()).step_by(stride.max(1)) {
        exercise(&seed[..cut], name);
    }
    let mut state: u64 = 0x5C1C_F00D;
    for _ in 0..count {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let pos = (state as usize) % seed.len();
        let byte = (state >> 40) as u8;
        let mut replaced = seed.to_vec();
        replaced[pos] = byte;
        exercise(&replaced, name);
        let mut inserted = seed.to_vec();
        inserted.insert(pos, byte);
        exercise(&inserted, name);
    }
}

fn sample_doc() -> Document {
    let mut doc = Document::default();
    let mut square = kurbo::BezPath::new();
    square.move_to((10.0, 10.0));
    square.line_to((40.0, 10.0));
    square.line_to((40.0, 30.0));
    square.line_to((10.0, 30.0));
    square.close_path();
    doc.push_shape(0, square, CutStyle::Cut).unwrap();
    let mut tri = kurbo::BezPath::new();
    tri.move_to((50.0, 50.0));
    tri.line_to((80.0, 50.0));
    tri.line_to((65.0, 90.0));
    tri.close_path();
    doc.push_shape(0, tri, CutStyle::Cut).unwrap();
    doc
}

const DXF_ENTITIES: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nLINE\n8\n0\n10\n10\n20\n10\n11\n10\n21\n0\n\
0\nLINE\n8\n0\n10\n10\n20\n10\n11\n0\n21\n10\n\
0\nLINE\n8\n0\n10\n0\n20\n10\n11\n0\n21\n0\n\
0\nCIRCLE\n8\n0\n10\n30\n20\n5\n40\n5\n\
0\nARC\n8\n0\n10\n50\n20\n5\n40\n5\n50\n0\n51\n180\n\
0\nELLIPSE\n8\n0\n10\n70\n20\n5\n11\n6\n21\n0\n40\n0.5\n41\n0\n42\n6.283185307\n\
0\nPOLYLINE\n8\n0\n66\n1\n70\n1\n0\nVERTEX\n8\n0\n10\n0\n20\n20\n0\nVERTEX\n8\n0\n10\n5\n20\n25\n0\nVERTEX\n8\n0\n10\n0\n20\n30\n0\nSEQEND\n\
0\nSPLINE\n8\n0\n70\n8\n71\n3\n72\n8\n73\n4\n40\n0\n40\n0\n40\n0\n40\n0\n40\n1\n40\n1\n40\n1\n40\n1\n\
10\n20\n20\n20\n10\n25\n20\n30\n10\n30\n20\n20\n10\n35\n20\n30\n\
0\nENDSEC\n0\nEOF\n";

#[test]
fn dxf_reads_every_entity_and_joins_lines() {
    let doc = import_bytes(DXF_ENTITIES.as_bytes(), "e.dxf", &ImportOptions::default()).unwrap();
    // Four LINEs join into one closed square; circle, arc, ellipse, polyline, spline.
    assert_eq!(doc.shape_count(), 6, "{:#?}", doc.layers[0].shapes.len());
    let loose = import_bytes(
        DXF_ENTITIES.as_bytes(),
        "e.dxf",
        &ImportOptions {
            join: false,
            ..ImportOptions::default()
        },
    )
    .unwrap();
    assert_eq!(loose.shape_count(), 9);
    let fitted = import_bytes(
        DXF_ENTITIES.as_bytes(),
        "e.dxf",
        &ImportOptions {
            fit_to_page: true,
            page_mm: (200.0, 200.0),
            ..ImportOptions::default()
        },
    )
    .unwrap();
    let bounds = stencilcraft_import::art_bounds(&fitted).unwrap();
    assert!(
        (bounds.width() - 200.0).abs() < 1e-6 || (bounds.height() - 200.0).abs() < 1e-6,
        "{bounds:?}"
    );
    assert_eq!(fitted.board.width_mm, 200.0);
}

#[test]
fn dxf_round_trips_our_exporter() {
    let doc = sample_doc();
    let dxf = stencilcraft_dxf::export(&doc).unwrap();
    let back = import_bytes(dxf.as_bytes(), "rt.dxf", &ImportOptions::default()).unwrap();
    assert_eq!(back.shape_count(), 2);
    let size = |d: &Document| {
        let mut sizes: Vec<(i64, i64)> = d
            .layers
            .iter()
            .flat_map(|l| l.shapes.iter())
            .map(|s| {
                let b = s.bounds().unwrap();
                (
                    (b.width() * 1000.0).round() as i64,
                    (b.height() * 1000.0).round() as i64,
                )
            })
            .collect();
        sizes.sort();
        sizes
    };
    assert_eq!(size(&back), size(&doc));
    for shape in back.layers.iter().flat_map(|l| l.shapes.iter()) {
        assert!(matches!(
            shape.path.elements().last(),
            Some(kurbo::PathEl::ClosePath)
        ));
    }
}

#[test]
fn pdf_round_trips_our_exporter() {
    let pdf = stencilcraft_pdf::export(&sample_doc()).unwrap();
    let back = import_bytes(&pdf, "rt.pdf", &ImportOptions::default()).unwrap();
    assert_eq!(back.shape_count(), 2);
    assert!(
        (back.board.width_mm - 100.0).abs() < 0.01,
        "{}",
        back.board.width_mm
    );
    let square = back.layers[0]
        .shapes
        .iter()
        .filter_map(|s| s.bounds())
        .find(|b| (b.width() - 30.0).abs() < 0.01)
        .unwrap();
    assert!(
        (square.x0 - 10.0).abs() < 0.01 && (square.y0 - 10.0).abs() < 0.01,
        "{square:?}"
    );
}

const EPS: &str = "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 100 100\n\
newpath 10 10 moveto 90 10 lineto 90 90 lineto 10 90 lineto closepath stroke\n\
newpath 50 50 20 0 360 arc closepath stroke\nshowpage\n%%EOF\n";

#[test]
fn eps_paths_are_read() {
    let doc = import_bytes(EPS.as_bytes(), "a.eps", &ImportOptions::default()).unwrap();
    assert_eq!(doc.shape_count(), 2);
    assert!((doc.board.width_mm - 100.0 * 25.4 / 72.0).abs() < 0.01);
}

#[test]
fn raster_imports_place_an_image() {
    let pixels = image::RgbaImage::from_pixel(96, 48, image::Rgba([0, 0, 0, 255]));
    let mut png = Cursor::new(Vec::new());
    pixels.write_to(&mut png, image::ImageFormat::Png).unwrap();
    let doc = import_bytes(png.get_ref(), "dir/photo.png", &ImportOptions::default()).unwrap();
    assert_eq!(doc.images.len(), 1);
    assert_eq!(doc.images[0].name, "photo.png");
    assert!((doc.board.width_mm - 25.4).abs() < 1e-9);
}

#[test]
fn dxf_corpus_never_panics() {
    mutate(DXF_ENTITIES.as_bytes(), "m.dxf", 1500, 7);
    let pathological = [
        String::new(),
        "0\nSECTION\n".to_string(),
        "0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\n1e400\n20\n0\n40\n5\n0\nENDSEC\n0\nEOF\n"
            .to_string(),
        "0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\nNaN\n20\n0\n40\n-5\n0\nENDSEC\n0\nEOF\n"
            .to_string(),
        format!(
            "0\nSECTION\n2\nENTITIES\n{}0\nENDSEC\n0\nEOF\n",
            "0\nLWPOLYLINE\n90\n4000000000\n".repeat(100)
        ),
        format!(
            "0\nSECTION\n2\nENTITIES\n0\nSPLINE\n71\n999999\n72\n999999\n{}0\nENDSEC\n0\nEOF\n",
            "40\n0\n".repeat(1000)
        ),
        "AutoCAD Binary DXF\r\n\x1a\0".to_string(),
        "AC1018 dwg".to_string(),
    ];
    for input in &pathological {
        exercise(input.as_bytes(), "p.dxf");
    }
}

#[test]
fn pdf_corpus_never_panics() {
    let pdf = stencilcraft_pdf::export(&sample_doc()).unwrap();
    mutate(&pdf, "m.pdf", 300, 13);
    for input in [
        &b"%PDF-1.4\n"[..],
        b"%PDF-1.4\n1 0 obj << /Type /Catalog /Pages 1 0 R >> endobj trailer << /Root 1 0 R >>",
        b"%PDF",
    ] {
        exercise(input, "p.pdf");
    }
}

#[test]
fn eps_corpus_never_panics() {
    mutate(EPS.as_bytes(), "m.eps", 400, 5);
    let loops = "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 10 10\n{ 1 } loop\n";
    let recursion = "%!PS\n/f { f } def f\n";
    let huge = "%!PS\n0 0 moveto 1 1 1000000000 { pop 1 1 rlineto } for stroke\n";
    for input in [
        loops,
        recursion,
        huge,
        "%!",
        "%!PS\n9999999999999999999999 9e999 moveto\n",
    ] {
        exercise(input.as_bytes(), "p.eps");
    }
}

#[test]
fn raster_corpus_never_panics() {
    let pixels = image::RgbaImage::from_fn(12, 9, |x, y| {
        image::Rgba([(x * 20) as u8, (y * 25) as u8, 128, 255])
    });
    for (format, name) in [
        (image::ImageFormat::Png, "m.png"),
        (image::ImageFormat::Gif, "m.gif"),
        (image::ImageFormat::Tiff, "m.tiff"),
        (image::ImageFormat::WebP, "m.webp"),
        (image::ImageFormat::Jpeg, "m.jpg"),
        (image::ImageFormat::Bmp, "m.bmp"),
    ] {
        let mut out = Cursor::new(Vec::new());
        let rgb = image::DynamicImage::ImageRgba8(pixels.clone()).to_rgb8();
        match format {
            image::ImageFormat::Jpeg | image::ImageFormat::Bmp => {
                rgb.write_to(&mut out, format).unwrap()
            }
            _ => pixels.write_to(&mut out, format).unwrap(),
        }
        mutate(out.get_ref(), name, 300, 3);
    }
}

#[test]
fn svg_and_unknown_inputs_never_panic() {
    exercise(b"<svg xmlns='http://www.w3.org/2000/svg'><path d='M0 0 L10 0 L10 10'/><path d='M10 10 L0 0'/></svg>", "a.svg");
    exercise(&[0xff, 0xfe, 0x00], "a.svg");
    exercise(b"random", "a.unknown");
    exercise(b"\0\0\0\x18ftypheic", "a.heic");
}
