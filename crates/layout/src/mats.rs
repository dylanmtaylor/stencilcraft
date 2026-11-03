//! Mat layout (the Prepare screen): which pieces go on which mat, and where.

use std::collections::BTreeMap;

use kurbo::{Affine, BezPath, Rect};
use serde::{Deserialize, Serialize};
use stencilcraft_doc::{Document, GroupKind, Operation};

use crate::machines::{MachineProfile, MarkStyle, machine};
use crate::{Result, check_range, invalid};

const IN: f64 = 25.4;
pub const MAX_COPIES: u32 = 100;
pub const MAX_PIECES: usize = 10_000;
pub const MAX_MATS: usize = 500;
/// Rotation candidates tried per piece when nesting (so the effective step is at least 5°).
const MAX_ROTATIONS: usize = 36;

/// The printed page for Print Then Cut mats, and its registration marks. Defaults: US Letter,
/// marks 0.787 in long, 0.020 in thick, inset 0.625 in from the page edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PageOptions {
    pub width_mm: f64,
    pub height_mm: f64,
    /// Mark layout; the machine's own style when unset.
    pub marks: Option<MarkStyle>,
    pub mark_length_mm: f64,
    pub mark_thickness_mm: f64,
    pub mark_inset_mm: f64,
}

impl Default for PageOptions {
    fn default() -> Self {
        Self {
            width_mm: 8.5 * IN,
            height_mm: 11.0 * IN,
            marks: None,
            mark_length_mm: 0.787 * IN,
            mark_thickness_mm: 0.020 * IN,
            mark_inset_mm: 0.625 * IN,
        }
    }
}

impl PageOptions {
    pub(crate) fn validate(&self) -> Result<()> {
        check_range(self.width_mm, 50.0, 1500.0, "page width_mm")?;
        check_range(self.height_mm, 50.0, 1500.0, "page height_mm")?;
        check_range(self.mark_length_mm, 2.0, 100.0, "mark_length_mm")?;
        check_range(self.mark_thickness_mm, 0.1, 5.0, "mark_thickness_mm")?;
        check_range(
            self.mark_inset_mm,
            self.mark_length_mm / 2.0,
            self.width_mm.min(self.height_mm) / 3.0,
            "mark_inset_mm (at least half the mark length, so the corner square fits)",
        )?;
        Ok(())
    }
}

/// Options of the mat layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MatOptions {
    /// Machine profile id (`production.machines` lists them).
    pub machine: String,
    /// Mat size name (`12x12`, `12x24`, …); the smallest that fits the largest piece when unset.
    pub mat: Option<String>,
    /// Feed from the roll without a mat (Smart Materials, roll feed).
    pub mat_free: bool,
    /// Project copies: every piece is repeated this many times (1–100).
    pub copies: u32,
    /// Gap kept between pieces and copies, in mm (0–100).
    pub spacing_mm: f64,
    /// Material-saving nesting (skyline packing). Off: pieces go down the mat in columns, in
    /// document order.
    pub nest: bool,
    /// Rotation step for nesting in degrees: 0 keeps every piece upright; 90 tries quarter
    /// turns; smaller steps (down to 5) try more angles.
    pub rotation_step_deg: f64,
    /// Mats to mirror, and mats not to mirror, overriding the automatic choice (mirrored when
    /// the material is iron-on or Infusible Ink).
    pub mirror_on: Vec<usize>,
    pub mirror_off: Vec<usize>,
    pub page: PageOptions,
}

impl Default for MatOptions {
    fn default() -> Self {
        Self {
            machine: "cricut-maker".to_string(),
            mat: None,
            mat_free: false,
            copies: 1,
            spacing_mm: 3.0,
            nest: false,
            rotation_step_deg: 90.0,
            mirror_on: Vec::new(),
            mirror_off: Vec::new(),
            page: PageOptions::default(),
        }
    }
}

/// What a mat holds: pieces of one material and colour, cut or printed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct MatKey {
    pub print_then_cut: bool,
    pub material: Option<String>,
    pub color: Option<String>,
}

/// One piece on a mat: a shape, or an Attach group, in one copy.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Placement {
    pub shape_ids: Vec<u64>,
    pub copy: u32,
    pub rotation_deg: f64,
    /// Document → mat map as kurbo affine coefficients `[a, b, c, d, e, f]`.
    pub transform: [f64; 6],
    /// The piece's box on the mat `[x0, y0, x1, y1]`.
    pub bounds: [f64; 4],
}

impl Placement {
    pub fn affine(&self) -> Affine {
        Affine::new(self.transform)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatPlan {
    pub index: usize,
    pub key: MatKey,
    /// The manual mat number that made this mat, if any.
    pub manual: Option<u32>,
    /// Mat size name, `roll` for mat-free, or `page` for a Print Then Cut page.
    pub size: String,
    pub width_mm: f64,
    /// Mat length, or for mat-free the material to feed (at least the roll minimum).
    pub length_mm: f64,
    /// Furthest extent of the pieces down the mat.
    pub used_length_mm: f64,
    pub mirror: bool,
    /// The area pieces may occupy `[x0, y0, x1, y1]`.
    pub area: [f64; 4],
    pub placements: Vec<Placement>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatLayout {
    pub machine: String,
    pub mats: Vec<MatPlan>,
}

struct Piece {
    shape_ids: Vec<u64>,
    copy: u32,
    key: MatKey,
    manual: Option<u32>,
    paths: Vec<BezPath>,
    bounds: Rect,
}

impl Piece {
    fn rotated(&self, deg: f64) -> (Affine, Rect) {
        let rot = Affine::rotate_about(deg.to_radians(), self.bounds.center());
        let mut acc: Option<Rect> = None;
        for p in &self.paths {
            if let Some(b) = stencilcraft_geom::bounds_of_shape(&(rot * p.clone())) {
                acc = Some(acc.map_or(b, |a| a.union(b)));
            }
        }
        (rot, acc.unwrap_or(self.bounds))
    }

    fn label(&self) -> u64 {
        self.shape_ids.first().copied().unwrap_or_default()
    }
}

fn collect_pieces(doc: &Document) -> Vec<Piece> {
    let mut pieces: Vec<Piece> = Vec::new();
    let mut attach_index: BTreeMap<u64, usize> = BTreeMap::new();
    for layer in doc.layers.iter().filter(|l| l.visible) {
        for shape in &layer.shapes {
            if shape.hidden || shape.operation() == Operation::Guide {
                continue;
            }
            let path = shape.cut_path().into_owned();
            let Some(bounds) = stencilcraft_geom::bounds_of_shape(&path) else {
                continue;
            };
            if path.elements().is_empty() {
                continue;
            }
            let printed = shape.operation() == Operation::PrintThenCut
                || shape
                    .production
                    .group
                    .as_ref()
                    .is_some_and(|g| g.kind == GroupKind::Flatten);
            let leads = !matches!(
                shape.operation(),
                Operation::Draw | Operation::Score | Operation::Engrave
            );
            let key = MatKey {
                print_then_cut: printed,
                material: shape.production.material.clone(),
                color: shape
                    .production
                    .fill_color
                    .clone()
                    .or_else(|| shape.production.line_color.clone()),
            };
            let existing = shape
                .production
                .attach
                .and_then(|a| attach_index.get(&a).copied());
            if let Some(piece) = existing.and_then(|i| pieces.get_mut(i)) {
                piece.shape_ids.push(shape.id);
                piece.paths.push(path);
                piece.bounds = piece.bounds.union(bounds);
                piece.manual = piece.manual.or(shape.production.mat);
                // A cut layer decides the mat of the attached draw/score layers; printing wins.
                if printed || leads && !piece.key.print_then_cut {
                    piece.key = MatKey {
                        print_then_cut: printed || piece.key.print_then_cut,
                        ..key
                    };
                }
                continue;
            }
            if let Some(a) = shape.production.attach {
                attach_index.insert(a, pieces.len());
            }
            pieces.push(Piece {
                shape_ids: vec![shape.id],
                copy: 0,
                key,
                manual: shape.production.mat,
                paths: vec![path],
                bounds,
            });
        }
    }
    pieces
}

/// A packing area and its strategy.
struct Packer {
    area: Rect,
    spacing: f64,
    nest: bool,
    angles: Vec<f64>,
}

#[derive(Clone)]
struct Placed {
    piece: usize,
    x: f64,
    y: f64,
    deg: f64,
}

#[derive(Default)]
struct Bin {
    /// Skyline segments `(x, width, height)` measured from the area's origin.
    skyline: Vec<(f64, f64, f64)>,
    /// Column cursor for the plain layout: `(x, y, column width)`.
    column: (f64, f64, f64),
    placed: Vec<Placed>,
}

impl Packer {
    fn new_bin(&self) -> Bin {
        Bin {
            skyline: vec![(0.0, self.area.width() + self.spacing, 0.0)],
            ..Bin::default()
        }
    }

    /// Orientations that fit an empty mat, as `(degrees, width, height)`.
    fn orientations(&self, piece: &Piece) -> Vec<(f64, f64, f64)> {
        self.angles
            .iter()
            .map(|&deg| {
                let (_, b) = piece.rotated(deg);
                (deg, b.width(), b.height())
            })
            .filter(|(_, w, h)| *w <= self.area.width() + 1e-9 && *h <= self.area.height() + 1e-9)
            .collect()
    }

    fn place(&self, bin: &mut Bin, index: usize, piece: &Piece) -> bool {
        let options = self.orientations(piece);
        if self.nest {
            let mut best: Option<(f64, f64, f64, f64, f64)> = None; // top, x, y, deg, w
            for &(deg, w, h) in &options {
                if let Some((x, y)) = skyline_fit(&bin.skyline, w, h, self.area, self.spacing) {
                    let better = best.is_none_or(|(top, bx, ..)| {
                        y + h < top - 1e-9 || ((y + h - top).abs() <= 1e-9 && x < bx)
                    });
                    if better {
                        best = Some((y + h, x, y, deg, w));
                    }
                }
            }
            let Some((top, x, y, deg, w)) = best else {
                return false;
            };
            skyline_add(&mut bin.skyline, x, w + self.spacing, top + self.spacing);
            bin.placed.push(Placed {
                piece: index,
                x,
                y,
                deg,
            });
            return true;
        }
        // Plain layout: down the mat, then the next column. Upright unless only a turn fits.
        let Some(&(deg, w, h)) = options.first() else {
            return false;
        };
        let (cx, cy, cw) = bin.column;
        let (x, y) = if cy + h <= self.area.height() + 1e-9 {
            (cx, cy)
        } else {
            let nx = cx + cw + self.spacing;
            if nx + w > self.area.width() + 1e-9 || cw == 0.0 {
                return false;
            }
            bin.column = (nx, 0.0, 0.0);
            (nx, 0.0)
        };
        if x + w > self.area.width() + 1e-9 {
            return false;
        }
        bin.column = (x, y + h + self.spacing, bin.column.2.max(w));
        bin.placed.push(Placed {
            piece: index,
            x,
            y,
            deg,
        });
        true
    }

    /// Packs `pieces` (indices into `all`) into as many bins as needed.
    fn pack(&self, all: &[Piece], pieces: &[usize]) -> Result<Vec<Bin>> {
        let mut order: Vec<usize> = pieces.to_vec();
        if self.nest {
            // Big pieces first leave the gaps for the small ones.
            order.sort_by(|&a, &b| {
                let area = |i: usize| all.get(i).map_or(0.0, |p| p.bounds.area());
                area(b).total_cmp(&area(a))
            });
        }
        let mut bins: Vec<Bin> = Vec::new();
        for index in order {
            let Some(piece) = all.get(index) else {
                continue;
            };
            if bins.iter_mut().any(|bin| self.place(bin, index, piece)) {
                continue;
            }
            if bins.len() >= MAX_MATS {
                return invalid(format!(
                    "the layout needs more than {MAX_MATS} mats; lower the copies"
                ));
            }
            let mut bin = self.new_bin();
            if !self.place(&mut bin, index, piece) {
                return invalid(format!(
                    "shape {} ({:.1} × {:.1} mm) does not fit the {:.1} × {:.1} mm cut area",
                    piece.label(),
                    piece.bounds.width(),
                    piece.bounds.height(),
                    self.area.width(),
                    self.area.height()
                ));
            }
            bins.push(bin);
        }
        Ok(bins)
    }
}

/// Lowest-then-leftmost spot on the skyline for a `w × h` piece.
fn skyline_fit(
    skyline: &[(f64, f64, f64)],
    w: f64,
    h: f64,
    area: Rect,
    spacing: f64,
) -> Option<(f64, f64)> {
    let mut best: Option<(f64, f64)> = None;
    for &(x, _, _) in skyline {
        if x + w > area.width() + 1e-9 {
            continue;
        }
        let span_end = x + w + spacing;
        let y = skyline
            .iter()
            .filter(|(sx, sw, _)| *sx < span_end - 1e-9 && sx + sw > x + 1e-9)
            .map(|(_, _, sy)| *sy)
            .fold(0.0, f64::max);
        if y + h > area.height() + 1e-9 {
            continue;
        }
        if best.is_none_or(|(bx, by)| y < by - 1e-9 || ((y - by).abs() <= 1e-9 && x < bx)) {
            best = Some((x, y));
        }
    }
    best
}

fn skyline_add(skyline: &mut Vec<(f64, f64, f64)>, x: f64, w: f64, top: f64) {
    let end = x + w;
    let mut next: Vec<(f64, f64, f64)> = Vec::with_capacity(skyline.len() + 2);
    for &(sx, sw, sy) in skyline.iter() {
        let se = sx + sw;
        if se <= x || sx >= end {
            next.push((sx, sw, sy));
            continue;
        }
        if sx < x {
            next.push((sx, x - sx, sy));
        }
        if se > end {
            next.push((end, se - end, sy));
        }
    }
    next.push((x, w, top));
    next.sort_by(|a, b| a.0.total_cmp(&b.0));
    skyline.clear();
    for seg in next {
        match skyline.last_mut() {
            Some(last) if (last.2 - seg.2).abs() < 1e-9 => last.1 += seg.1,
            _ => skyline.push(seg),
        }
    }
}

fn validate(options: &MatOptions) -> Result<&'static MachineProfile> {
    let Some(profile) = machine(&options.machine) else {
        let known: Vec<&str> = crate::MACHINES.iter().map(|m| m.id).collect();
        return invalid(format!(
            "unknown machine {:?}; known: {}",
            options.machine,
            known.join(", ")
        ));
    };
    if !(1..=MAX_COPIES).contains(&options.copies) {
        return invalid(format!(
            "copies must be 1–{MAX_COPIES}, got {}",
            options.copies
        ));
    }
    check_range(options.spacing_mm, 0.0, 100.0, "spacing_mm")?;
    let step = options.rotation_step_deg;
    if step != 0.0 {
        check_range(step, 1.0, 180.0, "rotation_step_deg (0 for none)")?;
    }
    options.page.validate()?;
    Ok(profile)
}

fn angles(step: f64, nest: bool) -> Vec<f64> {
    if !nest {
        // The plain layout keeps pieces upright and turns one only when it fits no other way.
        return vec![0.0, 90.0];
    }
    if step == 0.0 {
        return vec![0.0];
    }
    let step = step.max(180.0 / MAX_ROTATIONS as f64);
    let mut out = Vec::new();
    let mut a = 0.0;
    // Box sizes repeat every half turn.
    while a < 180.0 - 1e-9 && out.len() < MAX_ROTATIONS {
        out.push(a);
        a += step;
    }
    out
}

/// The mat or roll area for ordinary (cut) mats, and its size name and length.
fn cut_area(
    profile: &MachineProfile,
    options: &MatOptions,
    pieces: &[&Piece],
) -> Result<(String, Rect, f64, f64)> {
    if options.mat_free {
        let Some(roll) = &profile.roll else {
            return invalid(format!("{} cannot cut without a mat", profile.name));
        };
        let usable = roll.max_length_mm - roll.lead_mm - roll.trail_mm;
        let area = Rect::new(0.0, roll.lead_mm, roll.width_mm, roll.lead_mm + usable);
        return Ok(("roll".to_string(), area, roll.min_length_mm, roll.trail_mm));
    }
    if profile.mats.is_empty() {
        return invalid(format!(
            "{} cuts from the roll only; set mat_free",
            profile.name
        ));
    }
    if let Some(name) = &options.mat {
        let Some(mat) = profile.mats.iter().find(|m| m.name == name) else {
            let names: Vec<&str> = profile.mats.iter().map(|m| m.name).collect();
            return invalid(format!(
                "{} has no {name} mat; it takes {}",
                profile.name,
                names.join(", ")
            ));
        };
        return Ok((
            mat.name.to_string(),
            Rect::new(0.0, 0.0, mat.width_mm, mat.length_mm),
            0.0,
            0.0,
        ));
    }
    // Only sizes that fit the largest piece (upright or turned) are offered; take the smallest.
    let fits = |w: f64, l: f64| {
        pieces.iter().all(|p| {
            let (bw, bh) = (p.bounds.width(), p.bounds.height());
            (bw <= w + 1e-9 && bh <= l + 1e-9) || (bh <= w + 1e-9 && bw <= l + 1e-9)
        })
    };
    let mat = profile
        .mats
        .iter()
        .find(|m| fits(m.width_mm, m.length_mm))
        .or_else(|| profile.mats.last());
    let Some(mat) = mat else {
        return invalid(format!("{} has no mats", profile.name));
    };
    Ok((
        mat.name.to_string(),
        Rect::new(0.0, 0.0, mat.width_mm, mat.length_mm),
        0.0,
        0.0,
    ))
}

fn auto_mirror(key: &MatKey) -> bool {
    key.material.as_deref().is_some_and(|m| {
        let m = m.to_ascii_lowercase();
        m.contains("iron-on")
            || m.contains("iron on")
            || m.contains("htv")
            || m.contains("infusible")
    })
}

/// Lays the document's pieces onto mats.
pub fn layout_mats(doc: &Document, options: &MatOptions) -> Result<MatLayout> {
    let profile = validate(options)?;
    let base = collect_pieces(doc);
    if base.len().saturating_mul(options.copies as usize) > MAX_PIECES {
        return invalid(format!(
            "{} pieces × {} copies is more than {MAX_PIECES}",
            base.len(),
            options.copies
        ));
    }
    let mut pieces: Vec<Piece> = Vec::new();
    for copy in 0..options.copies {
        for p in &base {
            pieces.push(Piece {
                shape_ids: p.shape_ids.clone(),
                copy,
                key: p.key.clone(),
                manual: p.manual,
                paths: p.paths.clone(),
                bounds: p.bounds,
            });
        }
    }
    let cut_pieces: Vec<&Piece> = pieces.iter().filter(|p| !p.key.print_then_cut).collect();
    let (size, area, min_len, trail) = cut_area(profile, options, &cut_pieces)?;
    let print_area = crate::print::art_area(&options.page, profile);
    let packer = |print: bool| Packer {
        area: if print { print_area } else { area },
        spacing: options.spacing_mm,
        nest: options.nest,
        angles: angles(options.rotation_step_deg, options.nest),
    };

    // Automatic mats, one run per key in order of first appearance; manual pieces wait.
    let mut keys: Vec<MatKey> = Vec::new();
    for p in pieces.iter().filter(|p| p.manual.is_none()) {
        if !keys.contains(&p.key) {
            keys.push(p.key.clone());
        }
    }
    let mut mats: Vec<(MatKey, Option<u32>, Vec<usize>)> = Vec::new();
    for key in keys {
        let members: Vec<usize> = (0..pieces.len())
            .filter(|&i| {
                pieces
                    .get(i)
                    .is_some_and(|p| p.manual.is_none() && p.key == key)
            })
            .collect();
        for bin in packer(key.print_then_cut).pack(&pieces, &members)? {
            mats.push((
                key.clone(),
                None,
                bin.placed.iter().map(|p| p.piece).collect(),
            ));
        }
    }
    // Manual moves: onto an existing mat (re-laid with the newcomers) or a new one.
    let mut manual: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, p) in pieces.iter().enumerate() {
        if let Some(m) = p.manual {
            manual.entry(m).or_default().push(i);
        }
    }
    let auto_count = mats.len();
    for (number, moved) in manual {
        let target = usize::try_from(number).ok().filter(|&n| n < auto_count);
        match target.and_then(|n| mats.get_mut(n)) {
            Some((_, flag, members)) => {
                members.extend(moved);
                *flag = Some(number);
            }
            None => {
                let key = moved
                    .first()
                    .and_then(|&i| pieces.get(i))
                    .map(|p| p.key.clone())
                    .unwrap_or(MatKey {
                        print_then_cut: false,
                        material: None,
                        color: None,
                    });
                mats.push((key, Some(number), moved));
            }
        }
    }

    let mut out: Vec<MatPlan> = Vec::new();
    for (key, manual, members) in mats {
        let packer = packer(key.print_then_cut);
        let bins = packer.pack(&pieces, &members)?;
        if manual.is_some() && bins.len() > 1 {
            return invalid(format!(
                "the pieces moved to mat {} do not fit on one mat",
                manual.unwrap_or_default()
            ));
        }
        for bin in bins {
            let index = out.len();
            let mirror = if options.mirror_on.contains(&index) {
                true
            } else if options.mirror_off.contains(&index) {
                false
            } else {
                auto_mirror(&key)
            };
            let region = packer.area;
            let mut placements = Vec::new();
            let mut used: f64 = 0.0;
            for placed in &bin.placed {
                let Some(piece) = pieces.get(placed.piece) else {
                    continue;
                };
                let (rot, rb) = piece.rotated(placed.deg);
                let (px, py) = (region.x0 + placed.x, region.y0 + placed.y);
                // y-up document → y-down mat: the top of the piece meets the top of its slot.
                let mut map = Affine::translate((px - rb.x0, py + rb.y1))
                    * Affine::scale_non_uniform(1.0, -1.0)
                    * rot;
                let mut bounds = [px, py, px + rb.width(), py + rb.height()];
                if mirror {
                    let flip = Affine::new([-1.0, 0.0, 0.0, 1.0, region.x0 + region.x1, 0.0]);
                    map = flip * map;
                    bounds = [
                        region.x0 + region.x1 - bounds[2],
                        bounds[1],
                        region.x0 + region.x1 - bounds[0],
                        bounds[3],
                    ];
                }
                used = used.max(bounds[3]);
                placements.push(Placement {
                    shape_ids: piece.shape_ids.clone(),
                    copy: piece.copy,
                    rotation_deg: placed.deg,
                    transform: map.as_coeffs(),
                    bounds,
                });
            }
            placements.sort_by(|a, b| a.bounds[1].total_cmp(&b.bounds[1]));
            let (size, width, length) = if key.print_then_cut {
                (
                    "page".to_string(),
                    options.page.width_mm,
                    options.page.height_mm,
                )
            } else if options.mat_free {
                (size.clone(), area.x1, (used + trail).max(min_len))
            } else {
                (size.clone(), area.width(), area.height())
            };
            out.push(MatPlan {
                index,
                key: key.clone(),
                manual,
                size,
                width_mm: width,
                length_mm: length,
                used_length_mm: used,
                mirror,
                area: [region.x0, region.y0, region.x1, region.y1],
                placements,
            });
        }
    }
    Ok(MatLayout {
        machine: profile.id.to_string(),
        mats: out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rect;
    use stencilcraft_doc::CutStyle;

    fn doc_with(rects: &[(f64, f64, f64, f64)]) -> Document {
        let mut doc = Document::default();
        for &(x, y, w, h) in rects {
            doc.push_shape(0, rect(x, y, w, h), CutStyle::Cut).unwrap();
        }
        doc
    }

    fn overlaps(a: &[f64; 4], b: &[f64; 4]) -> bool {
        a[0] < b[2] - 1e-6 && b[0] < a[2] - 1e-6 && a[1] < b[3] - 1e-6 && b[1] < a[3] - 1e-6
    }

    fn check_inside_and_apart(layout: &MatLayout) {
        for mat in &layout.mats {
            for (i, p) in mat.placements.iter().enumerate() {
                assert!(p.bounds[0] >= mat.area[0] - 1e-6 && p.bounds[2] <= mat.area[2] + 1e-6);
                assert!(p.bounds[1] >= mat.area[1] - 1e-6 && p.bounds[3] <= mat.area[3] + 1e-6);
                for q in &mat.placements[i + 1..] {
                    assert!(!overlaps(&p.bounds, &q.bounds), "{p:?} overlaps {q:?}");
                }
            }
        }
    }

    #[test]
    fn layout_keeps_pieces_on_the_mat_and_apart() {
        let doc = doc_with(&[
            (0.0, 0.0, 100.0, 50.0),
            (10.0, 10.0, 80.0, 120.0),
            (0.0, 0.0, 30.0, 30.0),
        ]);
        for nest in [false, true] {
            let options = MatOptions {
                copies: 6,
                nest,
                ..MatOptions::default()
            };
            let layout = layout_mats(&doc, &options).unwrap();
            let total: usize = layout.mats.iter().map(|m| m.placements.len()).sum();
            assert_eq!(total, 18);
            check_inside_and_apart(&layout);
        }
    }

    #[test]
    fn placements_map_shapes_onto_their_boxes() {
        let doc = doc_with(&[(500.0, 500.0, 40.0, 20.0)]);
        let layout = layout_mats(&doc, &MatOptions::default()).unwrap();
        let p = &layout.mats[0].placements[0];
        let mapped = p.affine() * doc.layers[0].shapes[0].path.clone();
        let b = stencilcraft_geom::bounds_of_shape(&mapped).unwrap();
        assert!((b.x0 - p.bounds[0]).abs() < 1e-6 && (b.y1 - p.bounds[3]).abs() < 1e-6);
        assert_eq!(p.bounds, [0.0, 0.0, 40.0, 20.0]);
    }

    #[test]
    fn nesting_uses_less_roll_than_the_plain_layout() {
        // Tall thin strips and small squares: columns waste the width, nesting fills it.
        let mut rects = Vec::new();
        for i in 0..6 {
            rects.push((i as f64 * 50.0, 0.0, 20.0, 150.0));
            rects.push((i as f64 * 50.0, 200.0, 40.0, 40.0));
        }
        let doc = doc_with(&rects);
        let plain = MatOptions {
            mat_free: true,
            ..MatOptions::default()
        };
        let nested = MatOptions {
            nest: true,
            ..plain.clone()
        };
        let a = layout_mats(&doc, &plain).unwrap();
        let b = layout_mats(&doc, &nested).unwrap();
        check_inside_and_apart(&b);
        assert_eq!(a.mats.len(), 1);
        assert_eq!(b.mats.len(), 1);
        assert!(
            b.mats[0].used_length_mm < a.mats[0].used_length_mm,
            "nested {} vs plain {}",
            b.mats[0].used_length_mm,
            a.mats[0].used_length_mm
        );
    }

    #[test]
    fn rotation_steps_fit_a_diagonal_strip() {
        // A 45° strip only fits a narrow roll when nesting may turn it.
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        p.line_to((150.0, 150.0));
        p.line_to((145.0, 155.0));
        p.line_to((-5.0, 5.0));
        p.close_path();
        let mut doc = Document::default();
        doc.push_shape(0, p, CutStyle::Cut).unwrap();
        let options = MatOptions {
            machine: "cricut-joy".to_string(),
            mat_free: true,
            nest: true,
            rotation_step_deg: 45.0,
            ..MatOptions::default()
        };
        let layout = layout_mats(&doc, &options).unwrap();
        assert_eq!(layout.mats[0].placements[0].rotation_deg, 45.0);
        let upright = MatOptions {
            rotation_step_deg: 0.0,
            ..options
        };
        assert!(layout_mats(&doc, &upright).is_err());
    }

    #[test]
    fn mats_split_by_material_and_attach_stays_together() {
        let mut doc = doc_with(&[
            (0.0, 0.0, 10.0, 10.0),
            (50.0, 50.0, 10.0, 10.0),
            (0.0, 0.0, 5.0, 5.0),
        ]);
        doc.layers[0].shapes[0].production.attach = Some(1);
        doc.layers[0].shapes[1].production.attach = Some(1);
        doc.layers[0].shapes[2].production.material = Some("Everyday Iron-On".to_string());
        let layout = layout_mats(&doc, &MatOptions::default()).unwrap();
        assert_eq!(layout.mats.len(), 2);
        assert_eq!(layout.mats[0].placements.len(), 1);
        assert_eq!(layout.mats[0].placements[0].shape_ids, vec![1, 2]);
        // Attached shapes keep their relative offset (60 mm box).
        let b = layout.mats[0].placements[0].bounds;
        assert!((b[2] - b[0] - 60.0).abs() < 1e-6);
        assert!(!layout.mats[0].mirror);
        assert!(layout.mats[1].mirror, "iron-on mirrors automatically");
        let off = MatOptions {
            mirror_off: vec![1],
            mirror_on: vec![0],
            ..MatOptions::default()
        };
        let layout = layout_mats(&doc, &off).unwrap();
        assert!(layout.mats[0].mirror && !layout.mats[1].mirror);
    }

    #[test]
    fn manual_mat_and_size_limits() {
        let mut doc = doc_with(&[(0.0, 0.0, 10.0, 10.0), (0.0, 0.0, 20.0, 20.0)]);
        doc.layers[0].shapes[1].production.mat = Some(5);
        let layout = layout_mats(&doc, &MatOptions::default()).unwrap();
        assert_eq!(layout.mats.len(), 2);
        assert_eq!(layout.mats[1].manual, Some(5));
        doc.layers[0].shapes[1].production.mat = Some(0);
        let layout = layout_mats(&doc, &MatOptions::default()).unwrap();
        assert_eq!(layout.mats.len(), 1);
        assert_eq!(layout.mats[0].placements.len(), 2);

        let big = doc_with(&[(0.0, 0.0, 400.0, 400.0)]);
        let err = layout_mats(&big, &MatOptions::default()).unwrap_err();
        assert!(err.to_string().contains("does not fit"), "{err}");
        let joy = MatOptions {
            machine: "cricut-joy".to_string(),
            ..MatOptions::default()
        };
        let small = doc_with(&[(0.0, 0.0, 100.0, 200.0)]);
        let layout = layout_mats(&small, &joy).unwrap();
        assert_eq!(layout.mats[0].size, "4.5x12");
        assert!(
            layout_mats(
                &small,
                &MatOptions {
                    copies: 0,
                    ..MatOptions::default()
                }
            )
            .is_err()
        );
        assert!(
            layout_mats(
                &small,
                &MatOptions {
                    machine: "x".into(),
                    ..MatOptions::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn mat_free_reports_feed_length_with_roller_margins() {
        let doc = doc_with(&[(0.0, 0.0, 50.0, 30.0)]);
        let options = MatOptions {
            machine: "cricut-joy".to_string(),
            mat_free: true,
            ..MatOptions::default()
        };
        let layout = layout_mats(&doc, &options).unwrap();
        let mat = &layout.mats[0];
        assert_eq!(mat.size, "roll");
        assert!(
            (mat.placements[0].bounds[1] - 25.4).abs() < 1e-9,
            "1 in lead"
        );
        // 25.4 + 30 + 12.7 trail is under the 4 in minimum.
        assert!((mat.length_mm - 101.6).abs() < 1e-9);
    }
}
