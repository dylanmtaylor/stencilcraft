//! `stencilcraft-cli`: the headless driver and agent surface, following the
//! direction of the Crafting Apps CLI (`vectorcraft-cli run … --export out.pdf`).
//!
//! Subcommands:
//! - `run --in FILE [--apply-kerf MM] [--tile-sheet WxH] [--overlap MM] --export OUT[.svg|.dxf|.gcode]`
//! - `import-svg --in FILE.svg --out FILE.stencilcraft` — import SVG into the native document format
//! - `mcp` — JSON-lines control on stdio (see docs/mcp.md)
//! - `make-starter --out FILE` — write the canonical starter document
//! - `cut --in FILE [--media …] [--bbox] [--device PATH]` — machine cut prep
//! - `help`

mod cut;
mod pipeline;
mod run;

use run::RunRequest;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => {
            let req = RunRequest::parse(&args[1..]).map_err(|e| e.to_string())?;
            run::execute(req)
        }
        Some("import-svg") => import_svg_command(&args[1..]),
        Some("mcp") => mcp_loop(),
        Some("cut") => {
            cut::execute(&args[1..])?;
            Ok(())
        }
        Some("make-starter") => {
            let out = args
                .iter()
                .position(|a| a == "--out")
                .and_then(|i| args.get(i + 1))
                .ok_or("make-starter needs --out FILE")?;
            stencilcraft_doc::save_to(&stencilcraft_testkit::starter(), out)
                .map_err(|e| e.to_string())
        }
        None | Some("-h" | "--help" | "help") => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!(
            "unknown command: {other} (try `stencilcraft-cli help`)"
        )),
    }
}

fn mcp_loop() -> Result<(), String> {
    use std::io::{BufRead, Write as _};
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut session = stencilcraft_mcp::Session::default();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| format!("mcp: read: {e}"))?;
        match session.handle_line(&line) {
            Ok(None) => continue,
            Ok(Some(response)) => {
                writeln!(out, "{response}").map_err(|e| format!("mcp: write: {e}"))?;
            }
            Err(e) => return Err(format!("mcp: {e}")),
        }
    }
    Ok(())
}

fn import_svg_command(args: &[String]) -> Result<(), String> {
    let mut input = None;
    let mut output = None;
    let mut i = 0;
    while i < args.len() {
        let arg = args.get(i).map(String::as_str).unwrap_or("");
        match arg {
            "--in" => {
                i += 1;
                input = args.get(i).cloned();
            }
            "--out" => {
                i += 1;
                output = args.get(i).cloned();
            }
            "-h" | "--help" => {
                println!(
                    "usage: stencilcraft-cli import-svg --in FILE.svg --out FILE.stencilcraft"
                );
                return Ok(());
            }
            other => return Err(format!("import-svg: unknown argument {other}")),
        }
        i += 1;
    }
    let input = input.ok_or("import-svg: --in FILE.svg is required")?;
    let output = output.ok_or("import-svg: --out FILE.stencilcraft is required")?;
    let contents =
        std::fs::read_to_string(&input).map_err(|e| format!("{input}: could not read SVG: {e}"))?;
    let doc = stencilcraft_svg::import(&contents, &input).map_err(|e| e.to_string())?;
    stencilcraft_doc::save_to(&doc, &output).map_err(|e| e.to_string())?;
    println!(
        "imported {} shapes on {} layers ({} × {} mm): {output}",
        doc.shape_count(),
        doc.layers.len(),
        doc.board.width_mm,
        doc.board.height_mm
    );
    Ok(())
}

fn print_help() {
    println!("stencilcraft-cli — StencilCraft headless tools");
    println!();
    println!("  run --in FILE [--apply-kerf MM] [--tile-sheet WxH] [--overlap MM]");
    println!("      [--tile-prefix PREFIX] --export OUT");
    println!("      Export is .svg, .dxf or .gcode; with --tile-sheet the tiles go");
    println!("      to PREFIX-00.svg … (tiles export as SVG, open in the app to inspect).",);
    println!("  import-svg --in FILE.svg --out FILE.stencilcraft");
    println!("      Import common SVG shapes/groups/transforms into the native format.");
    println!("  mcp            JSON-lines control protocol on stdio (docs/mcp.md)");
    println!("  cut --in FILE [--media …] [--bbox] [--device PATH]");
    println!("      Encode + send to a cutting machine; dry-run transcript by default.");
    println!("  make-starter --out FILE");
    println!("  help");
}
