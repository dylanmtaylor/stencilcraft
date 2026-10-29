//! `run --in FILE [--apply-kerf MM] [--tile-sheet WxH] [--overlap MM]
//!      [--tile-prefix PREFIX] --export OUT`

use super::pipeline::{self, PipelineError, Tiling};
use stencilcraft_gcode::GcodeSettings;

pub struct RunRequest {
    pub input: Option<String>,
    pub kerf_mm: Option<f64>,
    pub tiling: Option<Tiling>,
    pub export: Option<String>,
    pub overlap: f64,
}

impl Default for RunRequest {
    fn default() -> Self {
        Self {
            input: None,
            kerf_mm: None,
            tiling: None,
            export: None,
            overlap: 0.0,
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
            let advance = 2;
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
                    let prefix = req.export.clone().unwrap_or_else(|| "tile".to_string());
                    req.tiling = Some(Tiling {
                        overlap: req.overlap,
                        prefix,
                        sheet_w: w,
                        sheet_h: h,
                    });
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
                "--tile-prefix" => {
                    let v = value()?.to_string();
                    if let Some(t) = req.tiling.as_mut() {
                        t.prefix = v;
                    }
                }
                "--export" => req.export = Some(value()?.to_string()),
                "-h" | "--help" => {
                    println!("usage: stencilcraft-cli run --in FILE [flags] --export OUT");
                    println!(
                        "flags: --apply-kerf MM --tile-sheet WxH --overlap MM --tile-prefix PREFIX"
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
            t.prefix = req.export.clone().unwrap_or_else(|| "tile".to_string());
        }
        Ok(req)
    }
}

/// Executes a parsed request: returns the process message or a printable error.
pub fn execute(req: RunRequest) -> Result<(), String> {
    let input = req.input.clone().unwrap_or_default();
    let export = req.export.clone().unwrap_or_default();
    let doc = stencilcraft_doc::load_from(&input).map_err(|e| e.to_string())?;
    let reports = pipeline::run_pipeline(
        &doc,
        req.kerf_mm,
        req.tiling.as_ref(),
        &export,
        &GcodeSettings::default(),
    )
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
