//! `stencilcraft-cli cut` — send a document to a cutting machine.
//!
//! Dry-run by default: encodes the full Graphtec command stream and prints a
//! readable transcript without touching hardware. A real send needs
//! `--device PATH` and a CLI built with `--features stencilcraft-cli/serial`.

use stencilcraft_doc::Document;
use stencilcraft_geom as geom;
use stencilcraft_machine as machine;
use stencilcraft_machine::{CutProgram, CutSettings, Mat, Media, Model, PlotOptions};

/// Errors from the cut subcommand; printed to stderr as the process message.
#[derive(Debug)]
pub enum CutError {
    /// Something the user did; message printed verbatim.
    User(String),
    /// `--help` was handled (help text already printed).
    HelpPrinted,
}

impl core::fmt::Display for CutError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CutError::User(msg) => write!(f, "{msg}"),
            CutError::HelpPrinted => write!(f, "help printed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CutRequest {
    pub input: Option<String>,
    /// Media preset name prefix or ID (default: vinyl sticker).
    pub media: Option<String>,
    pub pressure: Option<u8>,
    pub speed: Option<u8>,
    pub pen: bool,
    pub mat: Option<Mat>,
    /// Plot a bounding-box preview rectangle instead of the design.
    pub bbox_only: bool,
    pub model_name: Option<String>,
    /// Serial device path for a real send (needs the `serial` feature).
    pub device: Option<String>,
    pub track_enhancing: bool,
}

impl Default for CutRequest {
    fn default() -> Self {
        CutRequest {
            input: None,
            media: None,
            pressure: None,
            speed: None,
            pen: false,
            mat: Some(Mat::CameoPro24x24),
            bbox_only: false,
            model_name: None,
            device: None,
            track_enhancing: false,
        }
    }
}

impl CutRequest {
    pub fn parse(args: &[String]) -> Result<CutRequest, CutError> {
        let mut req = CutRequest::default();
        let mut i = 0;
        while i < args.len() {
            let advance = match args[i].as_str() {
                "--in" => {
                    req.input = Some(value(args, i, "--in")?.to_string());
                    2
                }
                "--media" => {
                    req.media = Some(value(args, i, "--media")?.to_string());
                    2
                }
                "--pressure" => {
                    let v = value(args, i, "--pressure")?;
                    req.pressure = Some(v.trim().parse().map_err(|_| {
                        CutError::User(format!("--pressure {v}: expected a count"))
                    })?);
                    2
                }
                "--speed" => {
                    let v = value(args, i, "--speed")?;
                    req.speed =
                        Some(v.trim().parse().map_err(|_| {
                            CutError::User(format!("--speed {v}: expected a number"))
                        })?);
                    2
                }
                "--mat" => {
                    req.mat = Some(parse_mat(value(args, i, "--mat")?)?);
                    2
                }
                "--model" => {
                    req.model_name = Some(value(args, i, "--model")?.to_string());
                    2
                }
                "--device" => {
                    req.device = Some(value(args, i, "--device")?.to_string());
                    2
                }
                "--pen" => {
                    req.pen = true;
                    1
                }
                "--track-enhancing" => {
                    req.track_enhancing = true;
                    1
                }
                "--bbox" => {
                    req.bbox_only = true;
                    1
                }
                "-h" | "--help" => {
                    print_cut_help();
                    return Err(CutError::HelpPrinted);
                }
                other => {
                    return Err(CutError::User(format!(
                        "cut: unknown flag {other} (see `stencilcraft-cli cut --help`)"
                    )));
                }
            };
            i += advance;
        }
        if req.input.is_none() {
            return Err(CutError::User("cut: --in FILE is required".to_string()));
        }
        Ok(req)
    }
}

fn value<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str, CutError> {
    args.get(i + 1)
        .map(String::as_str)
        .ok_or_else(|| CutError::User(format!("cut: {flag} needs a value")))
}

fn parse_mat(s: &str) -> Result<Mat, CutError> {
    match s.to_ascii_lowercase().as_str() {
        "none" => Ok(Mat::None),
        "12x12" => Ok(Mat::Cameo12x12),
        "12x24" => Ok(Mat::Cameo12x24),
        "8x12" | "portrait8x12" => Ok(Mat::Portrait8x12),
        "15x15" => Ok(Mat::CameoPlus15x15),
        "24x24" => Ok(Mat::CameoPro24x24),
        "8.5x12" => Ok(Mat::Portrait8_5x12),
        other => Err(CutError::User(format!(
            "cut: --mat {other}: use none, 12x12, 12x24, 8x12, 15x15, 24x24 or 8.5x12"
        ))),
    }
}

fn parse_model(name: &str) -> Result<Model, CutError> {
    Model::from_version(&name.replace('-', " ")).ok_or_else(|| {
        CutError::User(format!(
            "cut: --model {name}: unknown model. Known: cameo-4, cameo-4-plus, \
                 cameo-4-pro, cameo-5, cameo-5-plus, cameo-pro-mk2, cameo-3, \
                 portrait-3, portrait-4"
        ))
    })
}

/// Resolve the effective media + overrides into settings.
pub fn resolve_settings(req: &CutRequest) -> Result<(Media, CutSettings), CutError> {
    let media = match &req.media {
        Some(spec) => {
            let by_name = machine::MEDIA_PRESETS.iter().copied().find(|m| {
                m.name
                    .to_ascii_lowercase()
                    .starts_with(&spec.to_ascii_lowercase())
            });
            let found = by_name.or_else(|| {
                spec.trim()
                    .parse::<u16>()
                    .ok()
                    .and_then(machine::Media::by_id)
            });
            found.ok_or_else(|| {
                CutError::User(format!(
                    "cut: --media {spec}: no preset matches (try `vinyl`, `cardstock`, \
                     an ID from `--help`, or `custom` with --pressure/--speed)"
                ))
            })?
        }
        None => machine::Media::by_id(102)
            .ok_or_else(|| CutError::User("cut: default vinyl preset is missing".to_string()))?,
    };
    let mut settings = CutSettings::from_media(&media);
    settings.pen = req.pen;
    settings.track_enhancing = req.track_enhancing;
    if let Some(mat) = req.mat {
        settings.mat = mat;
    }
    if let Some(p) = req.pressure {
        settings.pressure = Some(p);
    }
    if let Some(s) = req.speed {
        settings.speed = Some(s);
    }
    Ok((media, settings))
}

/// Flatten every visible shape into machine paths (document mm coordinates:
/// x right, y down).
pub fn flatten_document(doc: &Document) -> Vec<Vec<(f64, f64)>> {
    let mut paths = Vec::new();
    for layer in &doc.layers {
        if !layer.visible {
            continue;
        }
        for shape in &layer.shapes {
            for pl in geom::path_to_polylines(&shape.path, geom::DEFAULT_FLATTEN_STEPS) {
                if pl.points.len() < 2 {
                    continue;
                }
                let mut path: Vec<(f64, f64)> = pl.points.iter().map(|p| (p.x, p.y)).collect();
                if pl.closed
                    && let Some(first) = pl.points.first()
                {
                    path.push((first.x, first.y));
                }
                paths.push(path);
            }
        }
    }
    paths
}

/// Encode the cut program for this request and document.
pub fn encode(
    req: &CutRequest,
    doc: &Document,
) -> Result<(CutProgram, Media, CutSettings, Model), CutError> {
    let model = match &req.model_name {
        Some(name) => parse_model(name)?,
        None => Model::Cameo4Pro,
    };
    let (media, settings) = resolve_settings(req)?;
    let paths = flatten_document(doc);
    if paths.is_empty() {
        return Err(CutError::User(
            "cut: document has no visible shapes to cut".to_string(),
        ));
    }
    let program = machine::encode_cut(
        model,
        &settings,
        &paths,
        &PlotOptions::default(),
        req.bbox_only,
    );
    Ok((program, media, settings, model))
}

/// Run the cut request; returns lines to print.
pub fn run(req: &CutRequest, doc: &Document) -> Result<Vec<String>, CutError> {
    let (program, media, settings, model) = encode(req, doc)?;
    let stream_len = program.to_stream().len();

    if let Some(device) = &req.device {
        real_send(&program, device, &media, model)?;
        let mut out = vec![format!("sent {stream_len} bytes to {device}")];
        out.push(format!(
            "model: {model:?}, media: {} (ID {}), mat: {:?}",
            media.name, media.id, settings.mat
        ));
        if let Some(bb) = program.bbox_mm {
            out.push(format!(
                "cut area: {:.1} x {:.1} mm",
                bb.3 - bb.1,
                bb.2 - bb.0
            ));
        }
        Ok(out)
    } else {
        Ok(dry_transcript(&program, &media, model, &settings))
    }
}

/// Actually write the program to a serial device: init escape, version query
/// with confirmation, ready-poll, then the stream. Everything returns; nothing
/// panics.
#[cfg(feature = "serial")]
fn real_send(
    program: &CutProgram,
    device: &str,
    media: &Media,
    model: Model,
) -> Result<(), CutError> {
    use machine::Transport as _;
    let mut transport =
        machine::SerialTransport::open(device, 9600).map_err(|e| CutError::User(e.to_string()))?;
    // Palaver: initialize, then ask who we are talking to.
    transport
        .write(machine::ESC_INIT.as_bytes())
        .map_err(|e| CutError::User(e.to_string()))?;
    transport
        .write(b"FG\x03")
        .map_err(|e| CutError::User(e.to_string()))?;
    let mut buf = [0u8; 64];
    let n = transport
        .read(&mut buf)
        .map_err(|e| CutError::User(e.to_string()))?;
    let version: String = String::from_utf8_lossy(&buf[..n])
        .trim_end_matches('\u{3}')
        .trim()
        .to_string();
    if let Some(actual) = Model::from_version(&version)
        && actual != model
    {
        return Err(CutError::User(format!(
            "cut: device reports {version:?} ({actual:?}) but --model was {model:?}. \
             Please align your --model argument."
        )));
    }
    // Poll status until ready.
    for _ in 0..15 {
        transport
            .write(machine::ESC_STATUS.as_bytes())
            .map_err(|e| CutError::User(e.to_string()))?;
        let n = transport
            .read(&mut buf)
            .map_err(|e| CutError::User(e.to_string()))?;
        match machine::parse_status(&buf[..n]) {
            machine::DeviceStatus::Ready => break,
            machine::DeviceStatus::Unloaded => {
                return Err(CutError::User(
                    "cut: media not loaded — load material and retry".to_string(),
                ));
            }
            // Moving / unknown: poll again after a pause.
            _ => std::thread::sleep(core::time::Duration::from_millis(2000)),
        }
    }
    machine::send_program(&mut transport, program).map_err(|e| CutError::User(e.to_string()))
}

/// Without the serial feature a real send cannot happen; say so calmly.
#[cfg(not(feature = "serial"))]
fn real_send(
    program: &CutProgram,
    device: &str,
    media: &Media,
    model: Model,
) -> Result<(), CutError> {
    let _ = (program, device, media, model);
    Err(CutError::User(
        "cut: --device needs serial support: \
         cargo run --features stencilcraft-cli/serial"
            .to_string(),
    ))
}

fn dry_transcript(
    program: &CutProgram,
    media: &Media,
    model: Model,
    settings: &CutSettings,
) -> Vec<String> {
    let mut out = Vec::new();
    out.push(format!(
        "dry run: encoded {} commands (~{} bytes) for {model:?}",
        program.commands.len(),
        program.to_stream().len()
    ));
    out.push(format!(
        "media: {} (ID {}), pressure: {:?}, speed: {:?}, mat: {:?}",
        media.name, media.id, settings.pressure, settings.speed, settings.mat
    ));
    if let Some(bb) = program.bbox_mm {
        out.push(format!(
            "cut area: {:.1} x {:.1} mm — use --bbox for a preview rectangle",
            bb.3 - bb.1,
            bb.2 - bb.0
        ));
    }
    out.push("program:".to_string());
    for command in &program.commands {
        out.push(command.to_ascii());
    }
    out
}

fn print_cut_help() {
    println!("usage: stencilcraft-cli cut --in FILE [flags]");
    println!();
    println!("  --media NAME|ID   preset prefix or ID (vinyl, cardstock, 102, …)");
    println!("  --pressure N      override pressure (1..33)");
    println!("  --speed N         override speed (1..30 on Cameo 4)");
    println!("  --mat MAT         none, 12x12, 12x24, 8x12, 15x15, 24x24, 8.5x12");
    println!("  --model MODEL     cameo-4-pro (default), cameo-4, portrait-4, …");
    println!("  --pen             pen mode");
    println!("  --track-enhancing roller cycles (needs pressure >= 19)");
    println!("  --bbox            preview rectangle instead of the design");
    println!("  --device PATH     real send over serial (needs the serial feature)");
    println!("Default: dry run — transcript printed, one command per line.");
    println!();
    println!("Media presets (ID: name — pressure/speed):");
    for m in machine::MEDIA_PRESETS {
        match (m.pressure, m.speed) {
            (Some(p), Some(s)) => println!("  {:>3}: {} — {p}/{s}", m.id, m.name),
            _ => println!("  {:>3}: {} — custom", m.id, m.name),
        }
    }
}

/// Entry point called from main.
pub fn execute(args: &[String]) -> Result<(), String> {
    let req = match CutRequest::parse(args) {
        Ok(req) => req,
        Err(CutError::HelpPrinted) => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    let input = req.input.clone().unwrap_or_default();
    let doc = stencilcraft_doc::load_from(&input).map_err(|e| e.to_string())?;
    let lines = run(&req, &doc).map_err(|e| e.to_string())?;
    for line in lines {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_testkit::starter;

    #[test]
    fn flag_parsing_covers_request_surface() {
        let args = vec![
            "--in".to_string(),
            "a.stencilcraft".to_string(),
            "--media".to_string(),
            "cardstock".to_string(),
            "--pressure".to_string(),
            "19".to_string(),
            "--speed".to_string(),
            "8".to_string(),
            "--pen".to_string(),
            "--bbox".to_string(),
        ];
        let req = CutRequest::parse(&args).expect("parses");
        assert_eq!(req.input.as_deref(), Some("a.stencilcraft"));
        assert_eq!(req.media.as_deref(), Some("cardstock"));
        assert_eq!(req.pressure, Some(19));
        assert_eq!(req.speed, Some(8));
        assert!(req.pen);
        assert!(req.bbox_only);
    }

    #[test]
    fn missing_input_is_a_calm_error() {
        let req = CutRequest::parse(&[]).expect_err("no --in given");
        assert!(req.to_string().contains("--in FILE is required"));
    }

    #[test]
    fn media_specs_resolve() {
        let req = CutRequest {
            media: Some("vinyl".to_string()),
            ..CutRequest::default()
        };
        let (media, settings) = resolve_settings(&req).expect("vinyl");
        assert_eq!(media.id, 102);
        assert_eq!(settings.pressure, Some(10));
        let req = CutRequest {
            media: Some("102".to_string()),
            ..CutRequest::default()
        };
        let (media, _) = resolve_settings(&req).expect("by id");
        assert_eq!(media.id, 102);
        let req = CutRequest {
            media: Some("unobtanium".to_string()),
            ..CutRequest::default()
        };
        assert!(resolve_settings(&req).is_err());
    }

    #[test]
    fn mat_parses() {
        assert_eq!(parse_mat("24x24").expect("pro mat"), Mat::CameoPro24x24);
        assert_eq!(
            parse_mat("8.5x12").expect("portrait mat"),
            Mat::Portrait8_5x12
        );
        assert!(parse_mat("11x11").is_err());
    }

    #[test]
    fn starter_document_flattens_with_outline_paths() {
        let doc = starter();
        let paths = flatten_document(&doc);
        assert!(!paths.is_empty(), "starter doc should yield cut paths");
        for path in paths {
            assert!(path.len() >= 2, "each path has at least two points");
        }
    }

    #[test]
    fn encode_returns_cameo4pro_by_default() {
        let doc = starter();
        let req = CutRequest::default();
        let (_, _, settings, model) = encode(&req, &doc).expect("encodes");
        assert_eq!(model, Model::Cameo4Pro);
        assert_eq!(settings.mat, Mat::CameoPro24x24);
    }

    #[test]
    fn dry_run_transcript_starts_with_header_and_setup() {
        let doc = starter();
        let req = CutRequest::default();
        let lines = run(&req, &doc).expect("dry runs");
        assert!(lines[0].starts_with("dry run: encoded"));
        assert!(lines.iter().any(|l| l.starts_with("dry run: encoded")));
        // The program section includes the tool select and media pressure.
        assert!(lines.iter().any(|l| l == "J1"));
        assert!(lines.iter().any(|l| l.starts_with("FX")));
    }
}
