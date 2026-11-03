//! Pop-up card generator: a card spread with a vertical centre fold and box struts across
//! it. Each strut is two parallel cuts across the fold; scoring the fold outside the struts,
//! the struts' own (reversed) fold and the two lines joining the cut ends makes the strut
//! pop forward when the card opens to 90°.

use kurbo::{BezPath, Point, Rect, Shape};
use serde::{Deserialize, Serialize};

use crate::{GenError, MAX_ITEMS, MAX_MM, Result, check_positive, check_range};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PopupParams {
    /// The opened card spread (millimetres, y up); the fold runs up its middle.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Number of struts (1–50), spread evenly up the fold.
    pub struts: u32,
    /// Strut width along the fold, in millimetres.
    pub strut_width_mm: f64,
    /// How far each strut reaches from the fold, as a percentage of half the card width
    /// (5–95).
    pub base_width_pct: f64,
    /// Dash pitch for the score lines: dash and gap each half of it; 0 scores solid lines.
    pub dash_pitch_mm: f64,
    /// Also cut the card outline.
    pub outline: bool,
}

impl Default for PopupParams {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 140.0,
            struts: 2,
            strut_width_mm: 20.0,
            base_width_pct: 30.0,
            dash_pitch_mm: 0.0,
            outline: true,
        }
    }
}

/// Cut lines and score lines.
pub struct Popup {
    pub cuts: BezPath,
    pub scores: BezPath,
}

pub fn popup(p: &PopupParams) -> Result<Popup> {
    check_range(p.x, -MAX_MM, MAX_MM, "x")?;
    check_range(p.y, -MAX_MM, MAX_MM, "y")?;
    let w = check_positive(p.width, MAX_MM, "width")?;
    let h = check_positive(p.height, MAX_MM, "height")?;
    let sw = check_positive(p.strut_width_mm, MAX_MM, "strut_width_mm")?;
    let pct = check_range(p.base_width_pct, 5.0, 95.0, "base_width_pct")?;
    let dash = check_range(p.dash_pitch_mm, 0.0, MAX_MM, "dash_pitch_mm")?;
    if !(1..=50).contains(&p.struts) {
        return Err(GenError("struts must be between 1 and 50".into()));
    }
    let n = p.struts as f64;
    let gap = (h - n * sw) / (n + 1.0);
    if gap <= 0.0 {
        return Err(GenError(format!(
            "{} struts {sw} mm wide do not fit a {h} mm fold",
            p.struts
        )));
    }
    let fold_x = p.x + w / 2.0;
    let depth = w / 2.0 * pct / 100.0;
    let mut cuts = BezPath::new();
    let mut scores = BezPath::new();
    if p.outline {
        cuts.extend(Rect::new(p.x, p.y, p.x + w, p.y + h).path_elements(0.1));
    }
    let mut score = |a: Point, b: Point| -> Result<()> { dashed(&mut scores, a, b, dash) };
    let mut y = p.y;
    for k in 0..p.struts {
        let y0 = p.y + gap + k as f64 * (sw + gap);
        let y1 = y0 + sw;
        // The card fold below this strut.
        score(Point::new(fold_x, y), Point::new(fold_x, y0))?;
        for cy in [y0, y1] {
            cuts.move_to((fold_x - depth, cy));
            cuts.line_to((fold_x + depth, cy));
        }
        // The strut's fold (reversed in use) and its two base folds.
        score(
            Point::new(fold_x - depth, y0),
            Point::new(fold_x - depth, y1),
        )?;
        score(Point::new(fold_x, y0), Point::new(fold_x, y1))?;
        score(
            Point::new(fold_x + depth, y0),
            Point::new(fold_x + depth, y1),
        )?;
        y = y1;
    }
    score(Point::new(fold_x, y), Point::new(fold_x, p.y + h))?;
    Ok(Popup { cuts, scores })
}

/// A line, or dashes along it (dash = gap = pitch / 2, centred on the line).
fn dashed(out: &mut BezPath, a: Point, b: Point, pitch: f64) -> Result<()> {
    let len = a.distance(b);
    if len <= 0.0 {
        return Ok(());
    }
    if pitch <= 0.0 || pitch >= len {
        out.move_to(a);
        out.line_to(b);
        return Ok(());
    }
    let n = (len / pitch).floor();
    if n > MAX_ITEMS as f64 {
        return Err(GenError(format!(
            "more than {MAX_ITEMS} dashes; raise dash_pitch_mm"
        )));
    }
    let start = (len - (n * pitch - pitch / 2.0)) / 2.0;
    for k in 0..n as usize {
        let s = start + k as f64 * pitch;
        out.move_to(a.lerp(b, s / len));
        out.line_to(a.lerp(b, (s + pitch / 2.0) / len));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flatten;

    #[test]
    fn default_card_has_two_struts() {
        let card = popup(&PopupParams::default()).unwrap();
        let cuts = flatten(&card.cuts).unwrap();
        // Outline + two cuts per strut.
        assert_eq!(cuts.len(), 5);
        assert!(cuts[0].closed);
        for c in &cuts[1..] {
            assert!(!c.closed);
            assert!((c.points[0].x - 70.0).abs() < 1e-9 && (c.points[1].x - 130.0).abs() < 1e-9);
        }
        // Fold pieces (struts + 1) and three folds per strut.
        assert_eq!(flatten(&card.scores).unwrap().len(), 3 + 6);
        // Everything sits on the card.
        let b = card.scores.bounding_box();
        assert!(b.x0 >= 0.0 && b.x1 <= 200.0 && b.y0 >= 0.0 && b.y1 <= 140.0);
    }

    #[test]
    fn dashed_scores_and_fit_checks() {
        let p = PopupParams {
            dash_pitch_mm: 4.0,
            struts: 1,
            outline: false,
            ..PopupParams::default()
        };
        let card = popup(&p).unwrap();
        for d in flatten(&card.scores).unwrap() {
            let l = crate::polyline_length(&d.points, false);
            assert!((l - 2.0).abs() < 1e-9, "{l}");
        }
        let p = PopupParams {
            struts: 10,
            strut_width_mm: 20.0,
            ..PopupParams::default()
        };
        assert!(popup(&p).is_err());
        let p = PopupParams {
            base_width_pct: 100.0,
            ..PopupParams::default()
        };
        assert!(popup(&p).is_err());
    }
}
