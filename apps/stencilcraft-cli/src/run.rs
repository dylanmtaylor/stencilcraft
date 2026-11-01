//! `run --in FILE [--apply-kerf MM] [--tile-sheet WxH] [--overlap MM]
//!      [--tile-prefix PREFIX] [--tile-marks] --export OUT`

use super::pipeline::{self, PipelineError, Tiling};
use stencilcraft_gcode::GcodeSettings;

pub struct RunRequest {
    pub input: Option<String>,
    pub kerf_mm: Option<f64>,
    pub tiling: Option<Tiling>,
    pub export: Option<String>,
    pub overlap: f64,
    /// `--tile-prefix`, wherever it appears on the command line: the tiling block
    /// itself may be introduced before or after it.
    pub tile_prefix: Option<String>,
    /// `--tile-marks`: stamp registration marks on every sheet.
    pub tile_marks: bool,
    /// Cut profile for `.gcode` exports; other formats ignore it.
    pub gcode: GcodeSettings,
}

impl Default for RunRequest {
    fn default() -> Self {
        Self {
            input: None,
            kerf_mm: None,
            tiling: None,
            export: None,
            overlap: 0.0,
            tile_prefix: None,
            tile_marks: false,
            gcode: GcodeSettings::default(),
        }
    }
}

impl RunRequest {
    /// Hand-rolled POSIX-style parsing, as with the parent binary: plain flags,
    /// no shell escapes, honest errors.
    pub fn parse(args: &[String]) -> Result<Self, PipelineError> {
        let mut req = RunRequest::default();
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            let value = || -> Result<&str, PipelineError> {
                args.get(i + 1)
                    .map(String::as_str)
                    .ok_or_else(|| PipelineError::User(format!("{flag} needs a value")))
            };
            // Most flags take a value; `--tile-marks` and `--help` do not.
            let mut advance = 2;
            match flag {
                "--in" => req.input = Some(value()?.to_string()),
                "--apply-kerf" => {
                    let v = value()?;
                    req.kerf_mm = Some(v.trim().parse().map_err(|_| {
                        PipelineError::User(format!("--apply-kerf {v}: expected a number of mm"))
                    })?);
                }
                "--tile-sheet" => {
                    let (w, h) = pipeline::parse_sheet_size(value()?)?;
                    // Reuse the block if `--overlap`/`--tile-prefix` already made one,
                    // so flags stay order-independent.
                    let overlap = req.overlap;
                    let marks = req.tile_marks;
                    let tiling = req.tiling.get_or_insert_with(|| Tiling {
                        sheet_w: w,
                        sheet_h: h,
                        overlap,
                        prefix: String::new(),
                        marks,
                    });
                    tiling.sheet_w = w;
                    tiling.sheet_h = h;
                    tiling.overlap = overlap;
                }
                "--tile-marks" => {
                    req.tile_marks = true;
                    if let Some(t) = req.tiling.as_mut() {
                        t.marks = true;
                    }
                    advance = 1;
                }
                "--overlap" => {
                    let v = value()?;
                    req.overlap = v
                        .trim()
                        .parse()
                        .map_err(|_| PipelineError::User(format!("--overlap {v}: expected mm")))?;
                    if let Some(t) = req.tiling.as_mut() {
                        t.overlap = req.overlap;
                    }
                }
                "--tile-prefix" => req.tile_prefix = Some(value()?.to_string()),
                "--feed" => {
                    let v = value()?;
                    req.gcode.feed_mm_per_min = v.trim().parse().map_err(|_| {
                        PipelineError::User(format!("--feed {v}: expected mm per minute"))
                    })?;
                }
                "--power" => {
                    let v = value()?;
                    req.gcode.power = parse_count("--power", v)?;
                }
                "--engrave-power" => {
                    let v = value()?;
                    req.gcode.engrave_power = Some(parse_count("--engrave-power", v)?);
                }
                "--passes" => {
                    let v = value()?;
                    req.gcode.passes = parse_count("--passes", v)?;
                }
                "--ramp" => {
                    req.gcode.ramp_power = true;
                    advance = 1;
                }
                "--laser-mode" => {
                    let v = value()?;
                    req.gcode.mode = stencilcraft_gcode::LaserMode::parse(v).ok_or_else(|| {
                        PipelineError::User(format!(
                            "--laser-mode {v}: expected dynamic or constant"
                        ))
                    })?;
                }
                "--export" => req.export = Some(value()?.to_string()),
                "-h" | "--help" => {
                    println!("usage: stencilcraft-cli run --in FILE [flags] --export OUT");
                    println!(
                        "flags: --apply-kerf MM --tile-sheet WxH --overlap MM --tile-prefix PREFIX --tile-marks"
                    );
                    println!(
                        "       --feed MM_PER_MIN --power S --engrave-power S --passes N --ramp --laser-mode dynamic|constant"
                    );
                    std::process::exit(0);
                }
                "--" => {
                    // Remaining tokens are positional: not expected for run.
                    if i + 1 < args.len() {
                        return Err(PipelineError::User(format!(
                            "unexpected positional argument: {}",
                            args[i + 1]
                        )));
                    }
                    break;
                }
                other => {
                    return Err(PipelineError::User(format!(
                        "run: unknown flag {other} (see `stencilcraft-cli help`)"
                    )));
                }
            }
            i += advance;
        }
        if req.input.is_none() {
            return Err(PipelineError::User(
                "run: --in FILE is required".to_string(),
            ));
        }
        if req.export.is_none() {
            return Err(PipelineError::User(
                "run: --export OUT is required".to_string(),
            ));
        }
        if let Some(t) = req.tiling.as_mut() {
            // An explicit `--tile-prefix` wins; otherwise tile names hang off the
            // export path, which is what a bare `--tile-sheet` run expects.
            t.prefix = match req.tile_prefix.clone() {
                Some(prefix) => prefix,
                None => req.export.clone().unwrap_or_else(|| "tile".to_string()),
            };
            t.marks |= req.tile_marks;
        }
        Ok(req)
    }
}

/// Parses a small non-negative integer flag, naming it in the error.
fn parse_count(flag: &str, value: &str) -> Result<u32, PipelineError> {
    value
        .trim()
        .parse()
        .map_err(|_| PipelineError::User(format!("{flag} {value}: expected a whole number")))
}

/// Executes a parsed request: returns the process message or a printable error.
pub fn execute(req: RunRequest) -> Result<(), String> {
    let input = req.input.clone().unwrap_or_default();
    let export = req.export.clone().unwrap_or_default();
    let doc = stencilcraft_doc::load_from(&input).map_err(|e| e.to_string())?;
    let reports =
        pipeline::run_pipeline(&doc, req.kerf_mm, req.tiling.as_ref(), &export, &req.gcode)
            .map_err(|e| e.to_string())?;
    for r in reports {
        println!("{r}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn requires_input_and_export() {
        assert!(RunRequest::parse(&args(&["run", "--in", "a.stencilcraft"])).is_err());
        assert!(RunRequest::parse(&args(&["--export", "out.svg"])).is_err());
        let ok =
            RunRequest::parse(&args(&["--in", "a.stencilcraft", "--export", "out.svg"])).unwrap();
        assert_eq!(ok.input.as_deref(), Some("a.stencilcraft"));
    }

    #[test]
    fn unknown_flag_rejected_calmly() {
        let r = RunRequest::parse(&args(&["--in", "a", "--export", "b", "--what"]));
        assert!(matches!(r, Err(PipelineError::User(ref m)) if m.contains("--what")));
    }

    #[test]
    fn tile_marks_is_a_standalone_flag() {
        let ok = RunRequest::parse(&args(&[
            "--in",
            "a.stencilcraft",
            "--tile-sheet",
            "400x600",
            "--tile-marks",
            "--export",
            "tiles/out",
        ]))
        .unwrap();
        assert!(ok.tiling.expect("tiling present").marks);
        let plain = RunRequest::parse(&args(&[
            "--in",
            "a.stencilcraft",
            "--tile-sheet",
            "400x600",
            "--export",
            "tiles/out",
        ]))
        .unwrap();
        assert!(!plain.tiling.expect("tiling present").marks);
    }

    #[test]
    fn gcode_profile_flags_parse() {
        let ok = RunRequest::parse(&args(&[
            "--in",
            "a.stencilcraft",
            "--export",
            "out.gcode",
            "--feed",
            "1200",
            "--power",
            "800",
            "--engrave-power",
            "150",
            "--passes",
            "3",
            "--ramp",
            "--laser-mode",
            "constant",
        ]))
        .unwrap();
        assert_eq!(ok.gcode.feed_mm_per_min, 1200.0);
        assert_eq!(ok.gcode.power, 800);
        assert_eq!(ok.gcode.engrave_power, Some(150));
        assert_eq!(ok.gcode.passes, 3);
        assert!(ok.gcode.ramp_power);
        assert_eq!(ok.gcode.mode, stencilcraft_gcode::LaserMode::Constant);
        // Defaults otherwise: a bare run keeps the documented profile.
        let plain = RunRequest::parse(&args(&["--in", "a", "--export", "b.gcode"])).unwrap();
        assert_eq!(plain.gcode, GcodeSettings::default());
    }

    #[test]
    fn bad_gcode_profile_flags_are_named() {
        for flags in [
            vec!["--passes", "many"],
            vec!["--power", "-5"],
            vec!["--laser-mode", "blinky"],
            vec!["--feed", "fast"],
        ] {
            let mut argv = args(&["--in", "a", "--export", "b.gcode"]);
            argv.extend(flags.iter().map(|s| s.to_string()));
            let err = match RunRequest::parse(&argv) {
                Ok(_) => panic!("accepted {flags:?}"),
                Err(err) => err.to_string(),
            };
            assert!(err.contains(flags[0]), "{flags:?} -> {err}");
        }
    }

    #[test]
    fn tile_prefix_wins_over_export_path_in_any_order() {
        for prefix_first in [true, false] {
            let flags = if prefix_first {
                args(&[
                    "--in",
                    "a.stencilcraft",
                    "--tile-prefix",
                    "sheets/poster",
                    "--tile-sheet",
                    "400x600",
                    "--export",
                    "out/ignored",
                ])
            } else {
                args(&[
                    "--in",
                    "a.stencilcraft",
                    "--tile-sheet",
                    "400x600",
                    "--export",
                    "out/ignored",
                    "--tile-prefix",
                    "sheets/poster",
                ])
            };
            let ok = RunRequest::parse(&flags).unwrap();
            let tiling = ok.tiling.expect("tiling present");
            assert_eq!(
                tiling.prefix, "sheets/poster",
                "prefix first: {prefix_first}"
            );
            assert_eq!(ok.export.as_deref(), Some("out/ignored"));
        }
    }

    #[test]
    fn tiling_flows_from_flags() {
        let ok = RunRequest::parse(&args(&[
            "--in",
            "a.stencilcraft",
            "--export",
            "tiles/out",
            "--tile-sheet",
            "400x600",
            "--overlap",
            "12.5",
        ]))
        .unwrap();
        let tiling = ok.tiling.expect("tiling present");
        assert_eq!(tiling.sheet_w, 400.0);
        assert_eq!(tiling.sheet_h, 600.0);
        assert_eq!(tiling.overlap, 12.5);
        assert_eq!(tiling.prefix, "tiles/out");
    }
}
