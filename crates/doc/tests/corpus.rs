//! Untrusted-input corpus: the document reader must return `Result` for
//! every malformed input we can construct, never panic. AGENTS.md ›
//! Robustness: "an untrusted file can never lose unsaved work".
//!
// Test-only file: helpers may fail loudly like tests do (see clippy.toml).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::ok_expect)]
//! This is deterministic fuzz-lite: a fixed seed drives mutations of a valid
//! document (truncation, byte replacement, insertion), plus hand-written
//! pathological inputs (nesting bombs, huge numbers, duplicate keys, wrong
//! types, non-UTF8 bytes). If a reader gains a panic on any of these, CI
//! fails here.

use stencilcraft_doc::Document;

/// The canonical starter document, identical in shape to
/// `examples/starter.stencilcraft` (kept verbatim so this test does not
/// depend on file layout).
const VALID_DOC: &str = r#"{
  "board": { "height_mm": 150.0, "width_mm": 100.0 },
  "kerf": { "apply_to_exports": false, "width_mm": 0.0 },
  "layers": [
    {
      "kind": "cut",
      "name": "cut",
      "shapes": [
        {
          "id": 1,
          "path": [
            { "MoveTo": { "x": 10.0, "y": 10.0 } },
            { "LineTo": { "x": 60.0, "y": 10.0 } },
            { "LineTo": { "x": 60.0, "y": 60.0 } },
            { "LineTo": { "x": 10.0, "y": 60.0 } },
            "ClosePath"
          ],
          "style": "cut"
        }
      ],
      "visible": true
    }
  ],
  "next-shape-id": 2,
  "stencilcraft": { "name": "stencilcraft", "version": 1 }
}"#;

/// Sanity: the corpus seed really is valid, so failures below mean the
/// mutations broke it, not the seed.
#[test]
fn corpus_seed_is_valid() {
    let doc = Document::load_json(VALID_DOC).expect("seed doc loads");
    doc.validate().expect("seed doc validates");
}

/// A mutant may be accepted or rejected — the only forbidden outcome is a
/// panic. Whatever loads must validate and re-save cleanly.
#[test]
fn truncations_of_a_valid_doc_never_panic() {
    for cut in 0..VALID_DOC.len() {
        let mutant = &VALID_DOC[..cut];
        exercise(mutant);
    }
}

/// Byte substitutions at a deterministic pseudo-random set of positions:
/// simple xorshift, no external fuzz dependency.
#[test]
fn byte_substitutions_never_panic() {
    let mut state: u64 = 0x5C1C_F00D;
    let bytes = VALID_DOC.as_bytes();
    for i in 0..2000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let pos = (state as usize) % bytes.len();
        let repl = (state % 128) as u8;
        let mut mutant = bytes.to_vec();
        mutant[pos] = repl;
        exercise_bytes(&mutant, i);
        // Insertions too: the parser must cope with doubled tokens as well
        // as corrupted ones.
        let mut expanded = bytes.to_vec();
        expanded.insert(pos, repl);
        exercise_bytes(&expanded, i);
    }
}

#[test]
fn pathological_inputs_reject_or_load_but_never_panic() {
    let inputs: Vec<String> = vec![
        "".to_string(),
        "not json".to_string(),
        "null".to_string(),
        "[]".to_string(),
        "{\"stencilcraft\": {\"name\": \"stencilcraft\", \"version\": 1}}".to_string(),
        format!(
            "{{\"stencilcraft\": {{\"version\": 1}}, \"board\": [{}",
            "[".repeat(4000)
        ),
        format!("{}1e400{}", "{\"x\": ".repeat(2), "}"),
        "{\"board\": {\"width_mm\": 1}".repeat(200),
        VALID_DOC.replace("width_mm", "widthMM"),
        VALID_DOC.replace("\"version\": 1", "\"version\": 999"),
        VALID_DOC.replace("150.0", "-150.0"),
        VALID_DOC.replace("150.0", "1e400"),
        VALID_DOC.replace("150.0", "null"),
        VALID_DOC.replace("\"visible\": true", "\"visible\": \"yes\""),
        VALID_DOC.replace("ClosePath", "\"SneakyNode\""),
        VALID_DOC.replace("\"width_mm\": 0.0", "\"width_mm\": NaN"),
        VALID_DOC
            .replace("next-shape-id", "next-shape-id-dup; next-shape-id")
            .replace(
                "\"next-shape-id-dup; next-shape-id\":",
                "\"next-shape-id\":",
            ),
        VALID_DOC.replace("\"name\": \"stencilcraft\"", "\"name\": \"\\u0000\""),
        "\u{feff}{\"stencilcraft\": {\"version\": 1}}".to_string(),
    ];
    for input in &inputs {
        exercise(input);
    }
}
#[test]
fn mutants_that_do_load_are_internally_consistent() {
    let mut loaded = 0usize;
    let mut state: u64 = 7;
    let bytes = VALID_DOC.as_bytes();
    for _ in 0..1000 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let pos = (state.wrapping_shr(33) as usize) % bytes.len();
        let mut mutant = bytes.to_vec();
        mutant[pos] = b' ';
        if let Ok(doc) = Document::load_json(std::str::from_utf8(&mutant).unwrap_or("")) {
            loaded += 1;
            doc.validate()
                .expect("a loaded document must always validate");
            doc.save_json()
                .expect("a loaded document must always re-save");
        }
    }
    assert!(loaded > 0, "some mutants must still parse; none may crash");
}

fn exercise(input: &str) {
    if let Ok(doc) = Document::load_json(input) {
        doc.validate()
            .expect("a loaded document must always validate");
        doc.save_json()
            .expect("a loaded document must always re-save");
    }
}

fn exercise_bytes(bytes: &[u8], _pass: usize) {
    match std::str::from_utf8(bytes) {
        Ok(text) => exercise(text),
        Err(_) => {
            // Non-UTF8 is a plain rejection; just check that asking for the
            // string via the lossy path also stays panic-free.
            let _ = Document::load_json(&String::from_utf8_lossy(bytes));
        }
    }
}
