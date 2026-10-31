//! `cargo xtask {ci, layers, fmt, clippy, test, help}` — the quality gates.
//!
//! `ci` runs the whole gate set: rustfmt --check, clippy -D warnings across the
//! workspace, tests, and the layering check. Every step is a plain `cargo`
//! invocation; no external tools, no nightly.

mod version;

use std::path::{Path, PathBuf};
use std::process::Command;

/// Workspace root (parent of the xtask crate).
pub fn root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "xtask has no parent directory".to_string())
}

/// A `cargo` command (the one running xtask, else `cargo` on the PATH) run in the workspace root.
pub fn cargo() -> Result<Command, String> {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.current_dir(root()?);
    Ok(c)
}

/// Prints `what` and runs `cmd`, which must exit successfully.
pub fn run(mut cmd: Command, what: &str) -> Result<(), String> {
    eprintln!("$ {what}");
    let status = cmd
        .status()
        .map_err(|e| format!("{what}: failed to spawn: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what}: exited with {status}"))
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
    let Some(task) = args.first().map(String::as_str) else {
        help();
        return Ok(());
    };
    match task {
        "ci" => ci(),
        "layers" => layers(),
        "fmt" => cargo_ok(&["cargo", "fmt"]),
        "clippy" => clippy(),
        "test" => cargo_ok(&["cargo", "test", "--workspace"]),
        "version" => version::run(&root()?, &rest),
        "-h" | "--help" | "help" => {
            help();
            Ok(())
        }
        other => Err(format!(
            "xtask: unknown task {other} (methods: ci, layers, fmt, clippy, test, version)"
        )),
    }
}

fn ci() -> Result<(), String> {
    cargo_ok(&["cargo", "fmt", "--all", "--", "--check"])?;
    clippy()?;
    cargo_ok(&["cargo", "test", "--workspace"])?;
    layers()?;
    println!("xtask ci: all gates passed");
    Ok(())
}

fn clippy() -> Result<(), String> {
    cargo_ok(&[
        "cargo",
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])
}

fn cargo_ok(argv: &[&str]) -> Result<(), String> {
    let (first, rest) = argv
        .split_first()
        .ok_or_else(|| "xtask: no command given".to_string())?;
    let status = Command::new(first)
        .args(rest)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("xtask: {first} failed: {status}"))
    }
}

/// Layering: nothing below `crates/ui-egui` may depend on the UI world
/// (egui/eframe/winit/rfd); exports never depend on the engine; check by reading
/// manifests — cheap, honest, and refactor-safe enough while the tree is small.
fn layers() -> Result<(), String> {
    let mut fails: Vec<String> = Vec::new();
    for manifest in [
        "crates/svg",
        "crates/dxf",
        "crates/gcode",
        "crates/tiling",
        "crates/kerf",
        "crates/geom",
        "crates/doc",
        "crates/engine",
        "crates/mcp",
        "crates/testkit",
        "crates/machine",
    ] {
        let text = std::fs::read_to_string(format!("{manifest}/Cargo.toml"))
            .map_err(|e| format!("xtask layers: {manifest}: {e}"))?;
        for forbidden in ["egui", "eframe", "winit", "rfd", "stencilcraft-ui-egui"] {
            if manifestation_has_ui_dep(&text, forbidden) {
                fails.push(format!(
                    "{manifest} depends on {forbidden} (layering violation)"
                ));
            }
        }
        // Exports must not pull in the engine (keep them independent serializers).
        if ["crates/svg", "crates/dxf", "crates/gcode"].contains(&manifest)
            && text.contains("stencilcraft-engine")
        {
            fails.push(format!(
                "{manifest}: an export crate depends on the engine (export layering violation)"
            ));
        }
    }
    if fails.is_empty() {
        println!("xtask layers: ok");
        Ok(())
    } else {
        Err(format!(
            "xtask layers: {}\n  - {}",
            fails.len(),
            fails.join("\n  - ")
        ))
    }
}

fn manifestation_has_ui_dep(manifest: &str, forbidden: &str) -> bool {
    // Look at [dependencies] only (dev-deps may use UI crate test helpers).
    let mut in_dependencies = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependencies = trimmed == "[dependencies]";
            continue;
        }
        if in_dependencies && trimmed.contains(forbidden) {
            return true;
        }
    }
    false
}

fn help() {
    println!("cargo xtask — StencilCraft automation");
    println!("  ci      fmt --check + clippy + tests + layers");
    println!("  layers  layering checks (nothing below ui-egui touches egui)");
    println!("  fmt     cargo fmt --all");
    println!("  clippy  cargo clippy --workspace --all-targets");
    println!("  test    cargo test --workspace");
    println!("  version show or set the workspace version");
}
