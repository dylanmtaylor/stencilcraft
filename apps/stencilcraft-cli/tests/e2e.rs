//! End-to-end tests: drive the built `stencilcraft-cli` binary the way a user
//! does, against the committed `examples/` documents. Everything here must
//! exit cleanly — a failing operation reports an error, never a panic.

// Test-only file: helpers may fail loudly like tests do (clippy.toml allows
// unwrap/expect in tests; the lint needs the attribute escape here because
// helpers are not #[test] annotated). This file never ships.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::ok_expect)]

use std::path::PathBuf;
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_stencilcraft-cli"))
}

fn ws_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .ok()
        .expect("workspace root exists")
}

fn example(name: &str) -> String {
    ws_root()
        .join("examples")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn fresh_out_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("stencilcraft-e2e");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).ok().expect("create temp dir");
    dir
}

fn expect_exit0(out: &Output, stdout: &str) {
    assert!(
        out.status.success(),
        "exit {:?}\nstdout: {stdout}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn expect_exit1(out: &Output, label: &str) {
    assert!(!out.status.success(), "{label}: expected a non-zero exit");
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !all.to_ascii_lowercase().contains("panic"),
        "{label}: rejected with a panic: {all}"
    );
}

#[test]
fn run_exports_svg_from_the_starter_example() {
    let dir = fresh_out_dir();
    let target = dir.join("starter-out.svg");
    let mut cmd = bin();
    cmd.args([
        "run",
        "--in",
        &example("starter.stencilcraft"),
        "--export",
        &target.to_string_lossy(),
    ]);
    let out = cmd.output().ok().expect("child spawns");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    expect_exit0(&out, &stdout);
    let svg = std::fs::read_to_string(&target).ok().expect("svg written");
    assert!(svg.contains("<svg"), "svg root present: {svg:.120}");
    assert!(svg.contains("path"), "cut paths exported");
    assert!(svg.len() > 50, "nontrivial document: len {}", svg.len());
}

#[test]
fn run_kerf_example_reports_applied_kerf() {
    let dir = fresh_out_dir();
    let target = dir.join("frame-out.svg");
    let mut cmd = bin();
    cmd.args([
        "run",
        "--in",
        &example("stencil-frame.stencilcraft"),
        "--export",
        &target.to_string_lossy(),
    ]);
    let out = cmd.output().ok().expect("child spawns");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    expect_exit0(&out, &stdout);
    assert!(stdout.contains("0.200 mm"), "kerf reported: {stdout}");
    assert!(target.is_file(), "svg written");
}

#[test]
fn run_tiles_the_poster_into_sheets() {
    let dir = fresh_out_dir();
    let base = dir.join("poster-tile");
    let mut cmd = bin();
    cmd.args([
        "run",
        "--in",
        &example("poster.stencilcraft"),
        "--tile-sheet",
        "400x600",
        "--overlap",
        "10",
        "--export",
        &base.to_string_lossy(),
    ]);
    let out = cmd.output().ok().expect("child spawns");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    expect_exit0(&out, &stdout);
    assert!(stdout.contains("exported 2 tiles"), "tile count: {stdout}");
    let base_str = base.to_string_lossy().into_owned();
    for suffix in ["-00.svg", "-01.svg"] {
        let p = std::path::PathBuf::from(format!("{base_str}{suffix}"));
        assert!(p.is_file(), "tile file {} exists", p.display());
    }
}

#[test]
fn cut_dry_run_prints_a_valid_transcript() {
    let mut cmd = bin();
    cmd.args(["cut", "--in", &example("starter.stencilcraft")]);
    let out = cmd.output().ok().expect("child spawns");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    expect_exit0(&out, &stdout);
    assert!(stdout.contains("dry run: encoded"), "header: {stdout:.100}");
    let program = stdout.split("program:\n").nth(1).unwrap_or("");
    assert!(!program.is_empty(), "transcript has program:");
    let lines: Vec<&str> = program.lines().collect();
    assert_eq!(lines.first(), Some(&"J1"), "setup starts with tool select");
    assert!(
        lines.iter().any(|l| l.starts_with("FX")),
        "tool selection appears"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("TG")),
        "mat registration appears"
    );
    assert!(lines.iter().any(|l| l.starts_with('M')), "move commands");
    assert!(lines.iter().any(|l| l.starts_with('D')), "draw commands");
    assert!(
        lines.iter().any(|l| l.starts_with("SO")),
        "ends by parking the head"
    );
    for line in &lines {
        assert!(
            line.chars()
                .next()
                .map(|c| { c.is_ascii_uppercase() || c.is_ascii_digit() || c == '!' || c == '\\' })
                .unwrap_or(true),
            "command line shape: {line:?}"
        );
    }
}

#[test]
fn cut_rejects_unknown_media_calmly() {
    let mut cmd = bin();
    cmd.args([
        "cut",
        "--in",
        &example("starter.stencilcraft"),
        "--media",
        "adamantium",
    ]);
    let out = cmd.output().ok().expect("child spawns");
    expect_exit1(&out, "cut --media");
    assert!(!out.status.success(), "unknown media must fail");
}

#[test]
fn cut_device_without_serial_is_a_calm_error() {
    let mut cmd = bin();
    cmd.args([
        "cut",
        "--in",
        &example("starter.stencilcraft"),
        "--device",
        "/dev/definitely-not-a-tty",
    ]);
    let out = cmd.output().ok().expect("child spawns");
    expect_exit1(&out, "cut --device");
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        all.contains("needs serial support") || all.contains("serial"),
        "calm feature message: {all}"
    );
}

#[test]
fn absent_input_reports_and_fails_the_pipeline() {
    let mut cmd = bin();
    cmd.args([
        "run",
        "--in",
        "/definitely/absent.stencilcraft",
        "--export",
        "x.svg",
    ]);
    let out = cmd.output().ok().expect("child spawns");
    expect_exit1(&out, "run --in absent");
}

#[test]
fn make_starter_writes_a_reloadable_document() {
    let dir = fresh_out_dir();
    let target = dir.join("made.stencilcraft");
    let mut cmd = bin();
    cmd.args(["make-starter", "--out", &target.to_string_lossy()]);
    let out = cmd.output().ok().expect("child spawns");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    expect_exit0(&out, &stdout);
    assert!(target.is_file(), "starter written");
    let json = std::fs::read_to_string(&target).ok().expect("read back");
    assert!(json.contains("stencilcraft"), "format header present");
}

#[test]
fn unknown_flag_fails_with_a_message_naming_the_flag() {
    let mut cmd = bin();
    cmd.args(["run", "--wednesday"]);
    let out = cmd.output().ok().expect("child spawns");
    expect_exit1(&out, "run --wednesday");
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(all.contains("--wednesday"), "flag named: {all}");
}
