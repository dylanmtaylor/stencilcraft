//! Prints what the reader makes of Silhouette Studio files (format analysis aid):
//! `cargo run -p stencilcraft-studio --example studio_dump -- FILE...`
fn main() -> Result<(), String> {
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
        match stencilcraft_studio::import(&bytes, &path) {
            Ok(got) => {
                println!(
                    "{path}: {:?}, board {} x {} mm, {} shapes",
                    got.version,
                    got.doc.board.width_mm,
                    got.doc.board.height_mm,
                    got.doc.shape_count()
                );
                for (_, s) in got.doc.shapes() {
                    println!("  {:?}", s.path);
                }
                for w in got.warnings {
                    println!("  warning: {w}");
                }
            }
            Err(e) => println!("{e}"),
        }
    }
    Ok(())
}
