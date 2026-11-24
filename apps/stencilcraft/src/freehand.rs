//! Freehand and Smooth Freehand drawing, following Silhouette Studio: the stroke follows the
//! pointer until release and closes when it ends on its start. Freehand keeps the stroke's
//! shape (dropping points that add nothing); Smooth Freehand simplifies it further and passes a
//! smooth curve through what is left, so it has no sharp angles.

use stencilcraft_geom::{BezPath, Point};

use crate::click_draw::{curve_through, polyline};

/// The most samples one stroke keeps, so a long scribble stays bounded.
pub const MAX_SAMPLES: usize = 20_000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stroke {
    pub points: Vec<Point>,
}

impl Stroke {
    /// Add a pointer sample, skipping ones closer than `min_step` (mm) to the last.
    pub fn push(&mut self, at: Point, min_step: f64) {
        if !(at.x.is_finite() && at.y.is_finite()) || self.points.len() >= MAX_SAMPLES {
            return;
        }
        let min_step = if min_step.is_finite() {
            min_step.max(0.0)
        } else {
            0.0
        };
        if self
            .points
            .last()
            .is_some_and(|last| (at - *last).hypot() < min_step)
        {
            return;
        }
        self.points.push(at);
    }

    /// The finished path: `tolerance` (mm) is how far the result may stray from the samples;
    /// the stroke closes when its end is within `close_radius` (mm) of its start. `None` when
    /// the stroke is too short to be a line.
    pub fn finish(&self, smooth: bool, tolerance: f64, close_radius: f64) -> Option<BezPath> {
        let (first, last) = (self.points.first()?, self.points.last()?);
        let closed = self.points.len() >= 4 && (*last - *first).hypot() <= close_radius;
        let mut points = self.points.clone();
        if closed {
            // The end lands on the start: drop the trailing samples sitting on it so the
            // close segment joins them without a tiny hook.
            while points.len() > 3
                && points
                    .last()
                    .is_some_and(|p| (*p - *first).hypot() <= close_radius)
            {
                points.pop();
            }
        }
        let tolerance = if tolerance.is_finite() {
            tolerance.max(0.0)
        } else {
            0.0
        };
        let tolerance = if smooth { tolerance * 8.0 } else { tolerance };
        let mut kept = simplify(&points, tolerance);
        // Simplifying always keeps the first sample; on a closed loop that start is just a
        // seam, so drop it when the loop passes close enough without it.
        if closed
            && kept.len() > 4
            && let (Some(start), Some(before), Some(after)) =
                (kept.first(), kept.last(), kept.get(1))
            && distance_to_segment(*start, *before, *after) <= tolerance
        {
            kept.remove(0);
        }
        if kept.len() < 2 {
            return None;
        }
        Some(if smooth {
            curve_through(&kept, closed && kept.len() >= 3)
        } else {
            polyline(&kept, closed && kept.len() >= 3)
        })
    }
}

/// Ramer–Douglas–Peucker: the fewest points (keeping both ends) whose polyline stays within
/// `tolerance` of the original. Iterative, so a long stroke cannot overflow the stack.
pub fn simplify(points: &[Point], tolerance: f64) -> Vec<Point> {
    let n = points.len();
    if n <= 2 {
        return points.to_vec();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut spans = vec![(0usize, n - 1)];
    while let Some((a, b)) = spans.pop() {
        let (Some(pa), Some(pb)) = (points.get(a), points.get(b)) else {
            continue;
        };
        let mut farthest = None;
        let mut worst = tolerance;
        for (i, p) in points.iter().enumerate().take(b).skip(a + 1) {
            let d = distance_to_segment(*p, *pa, *pb);
            if d > worst {
                worst = d;
                farthest = Some(i);
            }
        }
        if let Some(i) = farthest {
            if let Some(flag) = keep.get_mut(i) {
                *flag = true;
            }
            spans.push((a, i));
            spans.push((i, b));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let len2 = ab.hypot2();
    if len2 == 0.0 {
        return (p - a).hypot();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).hypot()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(points: impl IntoIterator<Item = (f64, f64)>) -> Stroke {
        let mut s = Stroke::default();
        for (x, y) in points {
            s.push(Point::new(x, y), 0.0);
        }
        s
    }

    #[test]
    fn a_straight_scribble_simplifies_to_its_ends() {
        let s = stroke((0..=100).map(|i| (f64::from(i), 0.001 * f64::from(i % 2))));
        let path = s.finish(false, 0.1, 1.0).expect("line");
        assert_eq!(path.elements().len(), 2, "{path:?}");
    }

    #[test]
    fn ending_on_the_start_closes_the_shape() {
        let square = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.3, 0.2),
        ];
        let path = stroke(square).finish(false, 0.1, 1.0).expect("square");
        assert!(
            path.elements()
                .last()
                .is_some_and(|e| e.end_point().is_none()),
            "closed"
        );
        let open = stroke(square.iter().copied().take(4))
            .finish(false, 0.1, 1.0)
            .expect("open");
        assert!(
            open.elements()
                .last()
                .is_some_and(|e| e.end_point().is_some()),
            "open"
        );
    }

    #[test]
    fn smooth_freehand_is_made_of_curves_through_kept_points() {
        let zigzag = (0..=40).map(|i| {
            let x = f64::from(i);
            (x, if i % 10 < 5 { x % 5.0 } else { 5.0 - x % 5.0 })
        });
        let path = stroke(zigzag).finish(true, 0.1, 0.5).expect("smooth");
        assert!(path.segments().count() >= 2);
        for segment in path.segments() {
            let cubic = segment.to_cubic();
            assert!(
                cubic.p1 != cubic.p0 || cubic.p2 != cubic.p3,
                "every segment is a curve with handles: {cubic:?}"
            );
        }
    }

    #[test]
    fn samples_are_bounded_and_hostile_points_are_dropped() {
        let mut s = Stroke::default();
        s.push(Point::new(f64::NAN, 0.0), 0.0);
        s.push(Point::new(0.0, 0.0), f64::NAN);
        s.push(Point::new(0.0, 0.1), 1.0);
        assert_eq!(s.points.len(), 1, "NaN and too-close samples skipped");
        for i in 0..(MAX_SAMPLES + 50) {
            s.push(Point::new(i as f64, 0.0), 0.0);
        }
        assert_eq!(s.points.len(), MAX_SAMPLES);
        assert!(Stroke::default().finish(true, 0.1, 1.0).is_none());
        assert!(stroke([(1.0, 1.0)]).finish(false, 0.1, 1.0).is_none());
        let _ = s.finish(true, f64::INFINITY, f64::NAN);
    }
}
