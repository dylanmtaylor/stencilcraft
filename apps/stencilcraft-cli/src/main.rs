//! `stencilcraft-cli`: the headless driver and agent surface, following the
//! direction of the Crafting Apps CLI (`vectorcraft-cli run … --export out.pdf`).
//!
//! Subcommands:
//! - `run --in FILE [--apply-kerf MM] [--tile-sheet WxH] [--overlap MM] --export OUT[.svg|.dxf|.gcode]`
//! - `mcp` — JSON-lines control on stdio (see docs/mcp.md)
//! - `make-starter --out FILE` — write the canonical starter document
//! - `help`

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
        Some("mcp") => mcp_loop(),
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

fn print_help() {
    println!("stencilcraft-cli — StencilCraft headless tools");
    println!();
    println!("  run --in FILE [--apply-kerf MM] [--tile-sheet WxH] [--overlap MM]");
    println!("      [--tile-prefix PREFIX] --export OUT");
    println!("      Export is .svg, .dxf or .gcode; with --tile-sheet the tiles go");
    println!("      to PREFIX-00.svg … (tiles export as SVG, open in the app to inspect).",);
    println!("  mcp            JSON-lines control protocol on stdio (docs/mcp.md)");
    println!("  make-starter --out FILE");
    println!("  help");
}
