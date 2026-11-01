//! `tile-preview`: every sheet of a tiling laid out on one page.
//!
//! Tiling produces one document per sheet; checking what will be cut on each one
//! otherwise means opening them one at a time. The preview writes a single SVG with
//! every sheet in its own framed cell, scaled down, so the whole job can be looked
//! at (and printed) before any material is used.

use stencilcraft_svg::TilePreviewOptions;
use stencilcraft_tiling::SheetOptions;

use crate::pipeline;

/// Run the subcommand: `tile-preview --in FILE --sheet WxH --out PAGE.svg`.
pub fn execute(args: &[String]) -> Result<(), String> {
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut sheet: Option<(f64, f64)> = None;
    let mut overlap = 0.0_f64;
    let mut columns = TilePreviewOptions::default().columns;
    let mut scale = TilePreviewOptions::default().scale;
    let mut marks = false;
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
            "--sheet" => {
                i += 1;
                let value = args.get(i).ok_or("tile-preview: --sheet needs WxH")?;
                sheet = Some(pipeline::parse_sheet_size(value).map_err(|e| e.to_string())?);
            }
            "--overlap" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("tile-preview: --overlap needs millimetres")?;
                overlap = value
                    .trim()
                    .parse()
                    .map_err(|_| format!("tile-preview: --overlap {value}: expected mm"))?;
            }
            "--columns" => {
                i += 1;
                let value = args.get(i).ok_or("tile-preview: --columns needs a count")?;
                columns = value
                    .trim()
                    .parse()
                    .map_err(|_| format!("tile-preview: --columns {value}: expected a count"))?;
            }
            "--scale" => {
                i += 1;
                let value = args.get(i).ok_or("tile-preview: --scale needs a number")?;
                scale = value
                    .trim()
                    .parse()
                    .map_err(|_| format!("tile-preview: --scale {value}: expected a number"))?;
            }
            "--marks" => marks = true,
            "-h" | "--help" => {
                println!(
                    "usage: stencilcraft-cli tile-preview --in FILE.stencilcraft --sheet WxH --out PAGE.svg"
                );
                println!(
                    "flags: --overlap MM --columns N --scale N --marks (see `run --tile-sheet`)"
                );
                return Ok(());
            }
            other => return Err(format!("tile-preview: unknown argument {other}")),
        }
        i += 1;
    }
    let input = input.ok_or("tile-preview: --in FILE.stencilcraft is required")?;
    let output = output.ok_or("tile-preview: --out PAGE.svg is required")?;
    let (sheet_w, sheet_h) = sheet.ok_or(
        "tile-preview: --sheet WxH is required (the same size you would cut on; see `run --tile-sheet`)",
    )?;
    let doc = stencilcraft_doc::load_from(&input).map_err(|e| e.to_string())?;
    let pieces = stencilcraft_tiling::tile_document_with(
        &doc,
        &SheetOptions {
            sheet_w,
            sheet_h,
            overlap_mm: overlap,
            marks,
        },
    )
    .map_err(|e| e.to_string())?;
    let svg =
        stencilcraft_svg::export_tile_preview(&pieces, &TilePreviewOptions { columns, scale })
            .map_err(|e| e.to_string())?;
    std::fs::write(&output, svg).map_err(|e| format!("{output}: {e}"))?;
    println!(
        "previewed {} sheets on one preview page: {output}",
        pieces.len()
    );
    Ok(())
}
