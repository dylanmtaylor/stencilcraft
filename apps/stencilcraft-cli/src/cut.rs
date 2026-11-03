//! `stencilcraft-cli cut` — encode a document for any registry machine.
//!
//! Dry-run by default: validates every setting against the machine's
//! capabilities and prints a one-command-per-line transcript without touching
//! hardware. `--out FILE` writes the exact byte stream to a file. Only
//! `--device DEV` sends: `usb[:BUS-ADDR]` (`--features usb`), a serial path
//! (`--features serial`), `tcp:HOST:PORT`, `file:PATH` or `stdout`. Also
//! `machines` (registry, capabilities) and `devices` (USB discovery, probing).

use stencilcraft_doc::Document;
use stencilcraft_geom as geom;
use stencilcraft_machine as machine;
use stencilcraft_machine::{
    Capabilities, CutJob, CutProgram, CutSettings, DeviceRegistry, MachineDriver, Mat, Media,
    PlotOptions, Registration, ToolKind, TransportSpec,
};

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

fn user(e: impl core::fmt::Display) -> CutError {
    CutError::User(format!("cut: {e}"))
}

#[derive(Debug, Clone, Default)]
pub struct CutRequest {
    pub input: Option<String>,
    /// Media preset name prefix or ID (default: vinyl sticker).
    pub media: Option<String>,
    /// Force steps or laser power, per the machine's `force.unit`.
    pub pressure: Option<u32>,
    /// Speed steps or mm/min, per the machine's `speed.unit`.
    pub speed: Option<u32>,
    pub pen: bool,
    /// `None` = the machine's largest documented mat.
    pub mat: Option<Mat>,
    /// Plot a bounding-box preview rectangle instead of the design.
    pub bbox_only: bool,
    pub model_name: Option<String>,
    /// Extra machine table (same shape as the built-in `machines.toml`).
    pub machines_file: Option<String>,
    /// Real send target; see `TransportSpec::parse`.
    pub device: Option<String>,
    /// Write the encoded stream to this file instead of printing it.
    pub out: Option<String>,
    pub track_enhancing: bool,
    pub tool: Option<ToolKind>,
    pub holder: Option<u8>,
    pub depth: Option<u8>,
    pub passes: Option<u8>,
    pub overcut_mm: Option<f64>,
    pub media_size: Option<(f64, f64)>,
    pub registration: Option<Registration>,
}

impl CutRequest {
    pub fn parse(args: &[String]) -> Result<CutRequest, CutError> {
        let mut req = CutRequest::default();
        let mut regmark_search = false;
        let mut regmark_quad = false;
        let mut i = 0;
        while let Some(arg) = args.get(i) {
            let mut take = || -> Result<&str, CutError> {
                i += 1;
                args.get(i)
                    .map(String::as_str)
                    .ok_or_else(|| user(format!("{arg} needs a value")))
            };
            match arg.as_str() {
                "--in" => req.input = Some(take()?.to_string()),
                "--media" => req.media = Some(take()?.to_string()),
                "--pressure" | "--force" => req.pressure = Some(num(arg, take()?)?),
                "--speed" => req.speed = Some(num(arg, take()?)?),
                "--depth" => req.depth = Some(num(arg, take()?)?),
                "--holder" => req.holder = Some(num(arg, take()?)?),
                "--passes" => req.passes = Some(num(arg, take()?)?),
                "--overcut" => req.overcut_mm = Some(num(arg, take()?)?),
                "--mat" => {
                    let v = take()?;
                    req.mat = Some(Mat::parse(v).ok_or_else(|| {
                        user(format!(
                            "--mat {v}: use none, 12x12, 12x24, 8x12, 15x15, 24x24 or 8.5x12"
                        ))
                    })?);
                }
                "--tool" => {
                    let v = take()?;
                    req.tool = Some(ToolKind::parse(v).ok_or_else(|| {
                        user(format!(
                            "--tool {v}: use autoblade, ratchet, deep-cut, kraft, rotary, \
                             punch, pen, emboss, deboss, etch, drag-blade, knife-blade, \
                             foil, score, perforate, engrave or laser"
                        ))
                    })?);
                }
                "--media-size" => req.media_size = Some(pair(arg, take()?, 'x')?),
                "--regmarks" => {
                    let v = take()?;
                    let parts: Vec<f64> = v
                        .split(',')
                        .map(|p| p.trim().parse::<f64>())
                        .collect::<Result<_, _>>()
                        .map_err(|_| user(format!("--regmarks {v}: expected W,L,X,Y in mm")))?;
                    let [w, l, x, y] = parts.as_slice() else {
                        return Err(user(format!("--regmarks {v}: expected W,L,X,Y in mm")));
                    };
                    req.registration = Some(Registration {
                        width_mm: *w,
                        length_mm: *l,
                        origin_right_mm: *x,
                        origin_down_mm: *y,
                        search: false,
                        quad: false,
                    });
                }
                "--regmark-search" => regmark_search = true,
                "--regmark-quad" => regmark_quad = true,
                "--model" | "--machine" => req.model_name = Some(take()?.to_string()),
                "--machines" => req.machines_file = Some(take()?.to_string()),
                "--device" => req.device = Some(take()?.to_string()),
                "--out" => req.out = Some(take()?.to_string()),
                "--pen" => req.pen = true,
                "--track-enhancing" => req.track_enhancing = true,
                "--bbox" => req.bbox_only = true,
                "-h" | "--help" => {
                    print_cut_help();
                    return Err(CutError::HelpPrinted);
                }
                other => {
                    return Err(user(format!(
                        "unknown flag {other} (see `stencilcraft-cli cut --help`)"
                    )));
                }
            }
            i += 1;
        }
        if let Some(reg) = req.registration.as_mut() {
            reg.search = regmark_search || regmark_quad;
            reg.quad = regmark_quad;
        } else if regmark_search || regmark_quad {
            return Err(user("--regmark-search needs --regmarks W,L,X,Y"));
        }
        if req.input.is_none() {
            return Err(user("--in FILE is required"));
        }
        Ok(req)
    }
}

fn num<T: core::str::FromStr>(flag: &str, v: &str) -> Result<T, CutError> {
    v.trim()
        .parse()
        .map_err(|_| user(format!("{flag} {v}: expected a number")))
}

fn pair(flag: &str, v: &str, sep: char) -> Result<(f64, f64), CutError> {
    let (a, b) = v
        .split_once(sep)
        .ok_or_else(|| user(format!("{flag} {v}: expected A{sep}B")))?;
    Ok((num(flag, a)?, num(flag, b)?))
}

/// The built-in registry, plus a user table when given.
pub fn registry(machines_file: Option<&str>) -> Result<DeviceRegistry, CutError> {
    if let Some(e) = DeviceRegistry::builtin_error() {
        return Err(user(e));
    }
    match machines_file {
        None => Ok(DeviceRegistry::builtin().clone()),
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| user(format!("{path}: {e}")))?;
            DeviceRegistry::builtin()
                .with_overrides(&text)
                .map_err(user)
        }
    }
}

/// Resolve a machine by registry id (`cameo-4-pro`) or firmware name (`CAMEO 4 PRO`).
pub fn resolve_machine(reg: &DeviceRegistry, name: &str) -> Result<Capabilities, CutError> {
    reg.by_id(name)
        .or_else(|| reg.by_firmware(name))
        .cloned()
        .ok_or_else(|| {
            user(format!(
                "--model {name}: unknown machine (see `stencilcraft-cli machines`)"
            ))
        })
}

/// Resolve a media preset by name prefix or numeric id.
pub fn resolve_media(spec: Option<&str>) -> Result<Media, CutError> {
    let Some(spec) = spec else {
        return Media::by_id(102).ok_or_else(|| user("default vinyl preset is missing"));
    };
    let lower = spec.to_ascii_lowercase();
    machine::MEDIA_PRESETS
        .iter()
        .copied()
        .find(|m| m.name.to_ascii_lowercase().starts_with(&lower))
        .or_else(|| spec.trim().parse::<u16>().ok().and_then(Media::by_id))
        .ok_or_else(|| {
            user(format!(
                "--media {spec}: no preset matches (try `vinyl`, `cardstock`, \
                 an ID from `--help`, or `custom` with --force/--speed)"
            ))
        })
}

/// Build the vendor-neutral job for a machine from the request and paths.
pub fn build_job(
    req: &CutRequest,
    caps: &Capabilities,
    paths: &[Vec<(f64, f64)>],
) -> Result<(Media, CutJob), CutError> {
    let media = resolve_media(req.media.as_deref())?;
    let mut s = CutSettings::for_caps(caps, &media);
    s.pen = req.pen;
    s.track_enhancing = req.track_enhancing;
    if let Some(mat) = req.mat {
        s.mat = mat;
    }
    if req.depth.is_some() {
        s.depth = req.depth;
    }
    s.tool = req.tool;
    s.toolholder = req.holder.unwrap_or(1);
    s.passes = req.passes.unwrap_or(1);
    s.overcut_mm = req.overcut_mm.unwrap_or(0.0);
    s.media_size_mm = req.media_size;
    s.registration = req.registration;
    let mut job = s.to_job(paths, &PlotOptions::default(), req.bbox_only);
    // Force/speed go straight into the job: their units are machine-specific
    // and may exceed the Silhouette step range (laser power 0-1000, mm/min).
    for op in &mut job.operations {
        if req.pressure.is_some() {
            op.settings.force = req.pressure;
        }
        if req.speed.is_some() {
            op.settings.speed = req.speed;
        }
    }
    Ok((media, job))
}

/// Flatten every visible shape into machine paths (document mm coordinates:
/// x right, y down).
pub fn flatten_document(doc: &Document) -> Vec<Vec<(f64, f64)>> {
    let mut paths = Vec::new();
    for layer in doc.layers.iter().filter(|l| l.visible) {
        for shape in layer.shapes.iter().filter(|shape| !shape.hidden) {
            let polylines = geom::path_to_polylines(&shape.path, geom::DEFAULT_FLATTEN_STEPS);
            paths.extend(machine::paths_from_polylines(&polylines));
        }
    }
    paths
}

/// Validate and encode the request for its machine.
pub fn encode(
    req: &CutRequest,
    doc: &Document,
) -> Result<(CutProgram, Media, CutJob, Capabilities), CutError> {
    let reg = registry(req.machines_file.as_deref())?;
    let caps = resolve_machine(&reg, req.model_name.as_deref().unwrap_or("cameo-4-pro"))?;
    let paths = flatten_document(doc);
    if paths.is_empty() {
        return Err(user("document has no visible shapes to cut"));
    }
    let (media, job) = build_job(req, &caps, &paths)?;
    let program = machine::dry_run(&caps, &job).map_err(user)?;
    Ok((program, media, job, caps))
}

/// Run the cut request; returns lines to print.
pub fn run(req: &CutRequest, doc: &Document) -> Result<Vec<String>, CutError> {
    let (program, media, job, caps) = encode(req, doc)?;
    let mut out = header(&program, &media, &caps, &job);
    if let Some(path) = &req.out {
        std::fs::write(path, program.to_stream()).map_err(|e| user(format!("{path}: {e}")))?;
        out.push(format!(
            "wrote {} bytes to {path}",
            program.to_stream().len()
        ));
    }
    if let Some(device) = &req.device {
        // The deliberate hardware step: the driver validates again before sending.
        let transport = TransportSpec::parse(device)
            .and_then(|spec| spec.open())
            .map_err(user)?;
        let mut driver = MachineDriver::new(caps, transport);
        let mut wait = || std::thread::sleep(core::time::Duration::from_millis(2000));
        let report = driver
            .send(&job, &mut machine::WaitFn(&mut wait))
            .map_err(user)?;
        let _ = driver.close();
        out.push(format!("sent {} bytes to {device}", report.bytes));
        if let Some(v) = report.version {
            out.push(format!("device firmware: {v}"));
        }
    } else if req.out.is_none() {
        out.push("program:".to_string());
        out.extend(program.transcript());
    }
    Ok(out)
}

fn header(program: &CutProgram, media: &Media, caps: &Capabilities, job: &CutJob) -> Vec<String> {
    let mut out = vec![format!(
        "dry run: encoded {} commands (~{} bytes) for {} [{}]",
        program.commands.len(),
        program.to_stream().len(),
        caps.name,
        caps.protocol.id()
    )];
    if let Some(op) = job.operations.first() {
        out.push(format!(
            "media: {} (ID {}), force: {:?}, speed: {:?}, mat: {}, holder: {}, tool: {}, passes: {}",
            media.name,
            media.id,
            op.settings.force,
            op.settings.speed,
            job.mat.id(),
            op.holder,
            op.tool.map_or("(as loaded)", ToolKind::id),
            op.settings.passes
        ));
    }
    if let Some(bb) = program.bbox_mm {
        out.push(format!(
            "cut area: {:.1} x {:.1} mm — use --bbox for a preview rectangle",
            bb.3 - bb.1,
            bb.2 - bb.0
        ));
    }
    out
}

fn print_cut_help() {
    println!("usage: stencilcraft-cli cut --in FILE [flags]");
    println!();
    println!("  --model ID        registry id (default cameo-4-pro; see `machines`)");
    println!("  --machines FILE   extra machine table (machines.toml format)");
    println!("  --media NAME|ID   preset prefix or ID (vinyl, cardstock, 102, …)");
    println!("  --force N         force steps / laser power (--pressure is an alias)");
    println!("  --speed N         speed steps or mm/min, per the machine");
    println!("  --tool TOOL       autoblade, ratchet, deep-cut, kraft, rotary, punch, pen, …");
    println!("  --holder N        tool holder 1 or 2 (two-holder machines)");
    println!("  --depth N         AutoBlade depth 0..10 (holder 1)");
    println!("  --passes N        cut each path N times (1..10)");
    println!("  --overcut MM      line-segment overcut at sharp corners (0..2 mm)");
    println!("  --mat MAT         none, 12x12, 12x24, 8x12, 15x15, 24x24, 8.5x12");
    println!("  --media-size WxL  loaded media in mm (required where the cut width is unknown)");
    println!("  --regmarks W,L,X,Y  print & cut: mark spacing and first-mark position, mm");
    println!("  --regmark-search  automatic optical search (TB123); --regmark-quad for 4 marks");
    println!("  --pen             pen mode");
    println!("  --track-enhancing roller cycles (needs force >= 19)");
    println!("  --bbox            preview rectangle instead of the design");
    println!("  --out FILE        write the encoded stream to FILE (no hardware)");
    println!("  --device DEV      REAL SEND: usb[:BUS-ADDR] (--features usb),");
    println!("                    /dev/ttyUSB0[@BAUD] or serial:PATH (--features serial),");
    println!("                    tcp:HOST:PORT, file:PATH, stdout");
    println!("Default: dry run — transcript printed, one command per line. Values outside");
    println!("the machine's documented limits are rejected, never clamped.");
    println!();
    println!("Media presets (ID: name — force/speed):");
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
    for line in run(&req, &doc).map_err(|e| e.to_string())? {
        println!("{line}");
    }
    Ok(())
}

/// `machines [--json] [--machines FILE] [show ID]`: the registry.
pub fn machines_command(args: &[String]) -> Result<(), String> {
    let mut json = false;
    let mut file = None;
    let mut show = None;
    let mut i = 0;
    while let Some(a) = args.get(i) {
        match a.as_str() {
            "--json" => json = true,
            "--machines" => {
                i += 1;
                file = Some(
                    args.get(i)
                        .ok_or("machines: --machines needs a file")?
                        .clone(),
                );
            }
            "show" => {
                i += 1;
                show = Some(args.get(i).ok_or("machines: show needs an id")?.clone());
            }
            other => return Err(format!("machines: unknown argument {other}")),
        }
        i += 1;
    }
    let reg = registry(file.as_deref()).map_err(|e| e.to_string())?;
    if let Some(id) = show {
        let caps = resolve_machine(&reg, &id).map_err(|e| e.to_string())?;
        let v = serde_json::to_value(&caps).map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if json {
        let v = serde_json::to_value(reg.machines()).map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let mm = |v: Option<f64>| v.map_or("?".to_string(), |v| format!("{v}"));
    let range = |r: &Option<machine::Range>| {
        r.as_ref()
            .map_or("panel".to_string(), |r| format!("{}-{}", r.min, r.max))
    };
    println!(
        "{:<22} {:<18} {:<12} {:<10} {:>7} {:>7} {:>4} {:>7} {:>7}",
        "id", "protocol", "support", "usb", "width", "length", "hold", "force", "speed"
    );
    for s in reg.machines() {
        println!(
            "{:<22} {:<18} {:<12} {:<10} {:>7} {:>7} {:>4} {:>7} {:>7}",
            s.id,
            s.protocol.id(),
            format!("{:?}", s.support).to_ascii_lowercase(),
            s.usb.as_deref().unwrap_or("?"),
            mm(s.cut_width_mm.or(s.media_width_mm)),
            mm(s.cut_length_mm),
            s.tool_holders.map_or("?".to_string(), |h| h.to_string()),
            range(&s.force),
            range(&s.speed),
        );
    }
    println!("? = not publicly documented. `machines show ID` prints full capabilities.");
    Ok(())
}

/// `devices [--status] [--probe DEV]`: USB discovery (needs `--features usb`)
/// and read-only probing of any port. Probing and status queries never move
/// the head.
pub fn devices_command(args: &[String]) -> Result<(), String> {
    let reg = registry(None).map_err(|e| e.to_string())?;
    if let Some(i) = args.iter().position(|a| a == "--probe") {
        let dev = args.get(i + 1).ok_or("devices: --probe needs a device")?;
        let mut t = TransportSpec::parse(dev)
            .and_then(|s| s.open())
            .map_err(|e| e.to_string())?;
        let found = reg.probe(t.as_mut()).map_err(|e| e.to_string())?;
        println!(
            "{dev}: {}",
            found.map_or("no known machine answered", |c| c.id.as_str())
        );
        return Ok(());
    }
    usb_devices(&reg, args.iter().any(|a| a == "--status"))
}

#[cfg(feature = "usb")]
fn usb_devices(reg: &DeviceRegistry, status: bool) -> Result<(), String> {
    let all = machine::list_usb_devices().map_err(|e| e.to_string())?;
    if all.is_empty() {
        println!("no cutter-vendor USB devices found");
    }
    for dev in all {
        let caps = reg.by_usb(dev.vendor_id, dev.product_id);
        println!(
            "usb:{} {:04x}:{:04x} {} {}",
            dev.location,
            dev.vendor_id,
            dev.product_id,
            caps.map_or("(not in registry)", |c| c.id.as_str()),
            dev.product.as_deref().unwrap_or("")
        );
        if status && let Some(caps) = caps {
            let t = TransportSpec::Usb(Some(dev.location.clone()))
                .open()
                .map_err(|e| e.to_string())?;
            let mut driver = MachineDriver::new(caps.clone(), t);
            match driver.status() {
                Ok(s) => println!("  status: {s:?}"),
                Err(e) => println!("  status: {e}"),
            }
        }
    }
    Ok(())
}

#[cfg(not(feature = "usb"))]
fn usb_devices(_reg: &DeviceRegistry, _status: bool) -> Result<(), String> {
    Err(
        "devices needs USB support: cargo run -p stencilcraft-cli --features usb -- devices \
         (or probe a port: devices --probe /dev/ttyUSB0)"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_testkit::starter;

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn flag_parsing_covers_request_surface() {
        let req = CutRequest::parse(&args(&[
            "--in",
            "a.stencilcraft",
            "--media",
            "cardstock",
            "--pressure",
            "19",
            "--speed",
            "8",
            "--pen",
            "--bbox",
            "--tool",
            "autoblade",
            "--holder",
            "2",
            "--depth",
            "3",
            "--passes",
            "2",
            "--overcut",
            "0.5",
            "--media-size",
            "300x500",
            "--regmarks",
            "180,230,15,20",
            "--regmark-search",
        ]))
        .expect("parses");
        assert_eq!(req.input.as_deref(), Some("a.stencilcraft"));
        assert_eq!(req.media.as_deref(), Some("cardstock"));
        assert_eq!((req.pressure, req.speed), (Some(19), Some(8)));
        assert!(req.pen && req.bbox_only);
        assert_eq!(req.tool, Some(ToolKind::AutoBlade));
        assert_eq!(
            (req.holder, req.depth, req.passes),
            (Some(2), Some(3), Some(2))
        );
        assert_eq!(req.overcut_mm, Some(0.5));
        assert_eq!(req.media_size, Some((300.0, 500.0)));
        let reg = req.registration.expect("regmarks");
        assert!(reg.search && !reg.quad);
        assert_eq!(reg.width_mm, 180.0);
    }

    #[test]
    fn bad_flags_are_calm_errors() {
        assert!(
            CutRequest::parse(&[])
                .expect_err("no --in")
                .to_string()
                .contains("--in FILE is required")
        );
        assert!(CutRequest::parse(&args(&["--in", "a", "--tool", "warp"])).is_err());
        assert!(CutRequest::parse(&args(&["--in", "a", "--regmarks", "1,2"])).is_err());
        assert!(CutRequest::parse(&args(&["--in", "a", "--regmark-search"])).is_err());
        assert!(CutRequest::parse(&args(&["--in", "a", "--speed"])).is_err());
    }

    #[test]
    fn media_specs_resolve() {
        assert_eq!(resolve_media(Some("vinyl")).expect("vinyl").id, 102);
        assert_eq!(resolve_media(Some("102")).expect("by id").id, 102);
        assert!(resolve_media(Some("unobtanium")).is_err());
    }

    #[test]
    fn machines_resolve_by_id_or_firmware_name() {
        let reg = registry(None).expect("registry");
        assert_eq!(resolve_machine(&reg, "cameo-3").expect("id").id, "cameo-3");
        assert_eq!(
            resolve_machine(&reg, "CAMEO 4 PRO").expect("fw").id,
            "cameo-4-pro"
        );
        assert!(resolve_machine(&reg, "warp-plotter").is_err());
    }

    #[test]
    fn starter_document_flattens_with_outline_paths() {
        let paths = flatten_document(&starter());
        assert!(!paths.is_empty(), "starter doc should yield cut paths");
        assert!(paths.iter().all(|p| p.len() >= 2));
    }

    #[test]
    fn encode_returns_cameo4pro_by_default() {
        let (_, _, job, caps) = encode(&CutRequest::default(), &starter()).expect("encodes");
        assert_eq!(caps.id, "cameo-4-pro");
        assert_eq!(job.mat, Mat::CameoPro24x24);
    }

    #[test]
    fn dry_run_transcript_starts_with_header_and_setup() {
        let lines = run(&CutRequest::default(), &starter()).expect("dry runs");
        assert!(lines[0].starts_with("dry run: encoded"));
        assert!(lines.iter().any(|l| l == "J1"));
        assert!(lines.iter().any(|l| l.starts_with("FX")));
    }

    #[test]
    fn out_of_range_force_is_rejected_not_clamped() {
        let req = CutRequest {
            pressure: Some(34),
            ..CutRequest::default()
        };
        let err = run(&req, &starter()).expect_err("force 34");
        assert!(err.to_string().contains("force 34 outside 1..=33"));
    }

    #[test]
    fn cricut_is_a_slot_not_a_driver() {
        let req = CutRequest {
            model_name: Some("cricut-maker-3".to_string()),
            ..CutRequest::default()
        };
        let err = run(&req, &starter()).expect_err("unsupported");
        assert!(err.to_string().contains("not implemented"), "{err}");
    }

    #[test]
    fn generic_hpgl_spools_to_a_file_device() {
        let path =
            std::env::temp_dir().join(format!("stencilcraft-cut-{}.plt", std::process::id()));
        let req = CutRequest {
            model_name: Some("generic-hpgl".to_string()),
            media_size: Some((1000.0, 1000.0)),
            mat: Some(Mat::None),
            device: Some(format!("file:{}", path.display())),
            ..CutRequest::default()
        };
        let lines = run(&req, &starter()).expect("hpgl");
        assert!(lines.iter().any(|l| l.starts_with("sent ")));
        let bytes = std::fs::read(&path).expect("written");
        assert!(bytes.starts_with(b"IN;\nSP1;\n"));
        let _ = std::fs::remove_file(&path);
    }
}
