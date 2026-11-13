//! `import` and `trace` subcommands: any supported file into a document, and a placed
//! raster image into cut contours. Both act through engine commands so they behave
//! exactly like the app and MCP.

use stencilcraft_doc::{CutStyle, Document};
use stencilcraft_engine::{Command, Effect, Engine};
use stencilcraft_import::ImportOptions;
use stencilcraft_raster::{ColourRange, TraceMode, TraceOptions};

pub const IMPORT_USAGE: &str =
    "usage: stencilcraft-cli import --in FILE --out DOC.stencilcraft [--into DOC.stencilcraft]
       [--fit-page [WxH]] [--no-join] [--join-tolerance MM] [--dpi N]
  FILE: .svg/.svgz .dxf .pdf .eps .ps .ai, or a PNG/JPEG/BMP/GIF/TIFF/WebP image (placed to trace).
  --fit-page scales vector art to the page (default: the target's board), else 1:1.";

pub const TRACE_USAGE: &str =
    "usage: stencilcraft-cli trace --in IMAGE|DOC.stencilcraft --out DOC.stencilcraft [--image ID]
       [--threshold PCT] [--despeckle PX] [--low-pass PX] [--high-pass PX]
       [--hue MIN:MAX] [--min-saturation PCT] [--smoothing PX] [--corner DEG]
       [--mode all|outer|detach] [--remove-background TOL] [--dpi N] [--engrave]
  Traces the placed image (the last one, or --image ID) into closed cut contours.";

fn flag_value<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str, String> {
    args.get(i + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} needs a value"))
}

fn number(args: &[String], i: usize, flag: &str) -> Result<f64, String> {
    let v = flag_value(args, i, flag)?;
    v.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("{flag} {v}: expected a number"))
}

/// Import `input` into `engine`'s document: vector shapes via `shape.add-paths`, images
/// via `image.place`. Returns (shapes added, image ids).
pub fn import_into(
    engine: &mut Engine,
    input: &str,
    options: &ImportOptions,
) -> Result<(usize, Vec<u64>), String> {
    let imported = stencilcraft_import::import_file(input, options).map_err(|e| e.to_string())?;
    let shapes: Vec<_> = imported
        .layers
        .iter()
        .flat_map(|layer| {
            layer
                .shapes
                .iter()
                .map(|shape| (shape.path.clone(), shape.style))
        })
        .collect();
    let count = shapes.len();
    if count > 0 {
        engine
            .run(Command::ShapeAddPaths { shapes })
            .map_err(|e| e.to_string())?;
    }
    let mut ids = Vec::new();
    for image in imported.images {
        if let Effect::ImageAdded { id } = engine
            .run(Command::ImagePlace { image })
            .map_err(|e| e.to_string())?
        {
            ids.push(id);
        }
    }
    Ok((count, ids))
}

pub fn import_command(args: &[String]) -> Result<(), String> {
    let (mut input, mut output, mut into) = (None, None, None);
    let mut options = ImportOptions::default();
    let mut fit_page: Option<Option<(f64, f64)>> = None;
    let mut i = 0;
    while i < args.len() {
        let arg = args.get(i).map(String::as_str).unwrap_or("");
        let mut advance = 2;
        match arg {
            "--in" => input = Some(flag_value(args, i, arg)?.to_string()),
            "--out" => output = Some(flag_value(args, i, arg)?.to_string()),
            "--into" => into = Some(flag_value(args, i, arg)?.to_string()),
            "--fit-page" => match args.get(i + 1).filter(|v| !v.starts_with("--")) {
                Some(size) => {
                    fit_page = Some(Some(
                        crate::pipeline::parse_sheet_size(size).map_err(|e| e.to_string())?,
                    ));
                }
                None => {
                    fit_page = Some(None);
                    advance = 1;
                }
            },
            "--no-join" => {
                options.join = false;
                advance = 1;
            }
            "--join-tolerance" => options.join_tolerance_mm = number(args, i, arg)?,
            "--dpi" => options.dpi = number(args, i, arg)?,
            "-h" | "--help" => {
                println!("{IMPORT_USAGE}");
                return Ok(());
            }
            other => return Err(format!("import: unknown argument {other}")),
        }
        i += advance;
    }
    let input = input.ok_or("import: --in FILE is required")?;
    let output = output.ok_or("import: --out FILE.stencilcraft is required")?;
    let base = match &into {
        Some(path) => stencilcraft_doc::load_from(path).map_err(|e| e.to_string())?,
        None => Document::default(),
    };
    if let Some(page) = fit_page {
        options.fit_to_page = true;
        options.page_mm = page.unwrap_or((base.board.width_mm, base.board.height_mm));
    }
    let doc = if into.is_some() {
        let mut engine = Engine::new(base);
        import_into(&mut engine, &input, &options)?;
        engine.doc
    } else {
        stencilcraft_import::import_file(&input, &options).map_err(|e| e.to_string())?
    };
    stencilcraft_doc::save_to(&doc, &output).map_err(|e| e.to_string())?;
    println!(
        "imported {} shapes and {} images ({:.2} × {:.2} mm): {output}",
        doc.shape_count(),
        doc.images.len(),
        doc.board.width_mm,
        doc.board.height_mm
    );
    Ok(())
}

pub fn trace_command(args: &[String]) -> Result<(), String> {
    let (mut input, mut output, mut image_id) = (None, None, None);
    let mut options = TraceOptions::default();
    let mut background: Option<f64> = None;
    let mut dpi = ImportOptions::default().dpi;
    let mut style = CutStyle::Cut;
    let mut range: Option<ColourRange> = None;
    let mut min_saturation = 0.0;
    let mut i = 0;
    while i < args.len() {
        let arg = args.get(i).map(String::as_str).unwrap_or("");
        let mut advance = 2;
        match arg {
            "--in" => input = Some(flag_value(args, i, arg)?.to_string()),
            "--out" => output = Some(flag_value(args, i, arg)?.to_string()),
            "--image" => {
                let v = flag_value(args, i, arg)?;
                image_id = Some(
                    v.parse::<u64>()
                        .map_err(|_| format!("--image {v}: expected an id"))?,
                );
            }
            "--threshold" => options.threshold = number(args, i, arg)?,
            "--despeckle" => {
                let v = flag_value(args, i, arg)?;
                options.despeckle = v
                    .parse()
                    .map_err(|_| format!("--despeckle {v}: expected pixels"))?;
            }
            "--low-pass" => options.low_pass = number(args, i, arg)?,
            "--high-pass" => options.high_pass = number(args, i, arg)?,
            "--smoothing" => options.smoothing = number(args, i, arg)?,
            "--corner" => options.corner_threshold = number(args, i, arg)?,
            "--min-saturation" => min_saturation = number(args, i, arg)?,
            "--hue" => {
                let v = flag_value(args, i, arg)?;
                let (lo, hi) = v
                    .split_once(':')
                    .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
                    .ok_or_else(|| format!("--hue {v}: expected MIN:MAX degrees"))?;
                range = Some(ColourRange {
                    hue_min: lo,
                    hue_max: hi,
                    min_saturation: 0.0,
                });
            }
            "--mode" => {
                options.mode = match flag_value(args, i, arg)? {
                    "all" | "all-edges" => TraceMode::AllEdges,
                    "outer" | "outer-edge" => TraceMode::OuterEdge,
                    "detach" => TraceMode::Detach,
                    other => return Err(format!("--mode {other}: expected all, outer or detach")),
                };
            }
            "--remove-background" => background = Some(number(args, i, arg)?),
            "--dpi" => dpi = number(args, i, arg)?,
            "--engrave" => {
                style = CutStyle::Engrave;
                advance = 1;
            }
            "-h" | "--help" => {
                println!("{TRACE_USAGE}");
                return Ok(());
            }
            other => return Err(format!("trace: unknown argument {other}")),
        }
        i += advance;
    }
    options.colour_range = range.map(|r| ColourRange {
        min_saturation,
        ..r
    });
    let input = input.ok_or("trace: --in FILE is required")?;
    let output = output.ok_or("trace: --out FILE.stencilcraft is required")?;
    let is_native = std::path::Path::new(&input)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(craft_file_formats::from_extension)
        == Some(craft_file_formats::Format::StencilCraft);
    let mut engine = if is_native {
        Engine::new(stencilcraft_doc::load_from(&input).map_err(|e| e.to_string())?)
    } else {
        let options = ImportOptions {
            dpi,
            ..ImportOptions::default()
        };
        Engine::new(stencilcraft_import::import_file(&input, &options).map_err(|e| e.to_string())?)
    };
    let id = image_id
        .or_else(|| engine.doc.images.last().map(|image| image.id))
        .ok_or("trace: the document has no placed image")?;
    if let Some(tolerance) = background {
        engine
            .run(Command::ImageRemoveBackground { id, tolerance })
            .map_err(|e| e.to_string())?;
    }
    let added = match engine
        .run(Command::ImageTrace { id, options, style })
        .map_err(|e| e.to_string())?
    {
        Effect::ShapesAdded { ids } => ids.len(),
        _ => 0,
    };
    stencilcraft_doc::save_to(&engine.doc, &output).map_err(|e| e.to_string())?;
    println!("traced {added} closed contours from image {id}: {output}");
    Ok(())
}
