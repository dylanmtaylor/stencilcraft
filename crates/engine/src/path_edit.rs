//! Node-level path edits on VectorCraft's anchor model (`PathData`), in millimetres.
//! Each function validates its indices and returns a printable error instead of panicking;
//! the engine wraps them in commands (`crate::Command::Path*`).

use std::collections::BTreeSet;

use kurbo::Point;
use serde::{Deserialize, Serialize};
use vectorcraft_geom::{AnchorKind, PathData, SubPath};

/// Endpoints closer than this (mm) are merged into one node instead of joined by a line.
const COINCIDENT_MM: f64 = 1e-6;

/// One node of a shape's path: its contour (subpath) and position along it, as counted by
/// `query.node-count`. A closed contour does not repeat its first node at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeRef {
    pub contour: usize,
    pub node: usize,
}

/// Which end of an open contour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathEnd {
    Start,
    End,
}

/// Node type for `path.set-node-type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeType {
    /// No handles: a sharp corner.
    Corner,
    /// Collinear handles, a third of the way to each neighbour.
    Smooth,
}

/// Segment type for `path.set-segment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SegmentType {
    Line,
    Curve,
}

/// Axis for `path.average-nodes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeAxis {
    /// Same y.
    Horizontal,
    /// Same x.
    Vertical,
    /// The common centroid.
    Both,
}

/// Stroke join for `path.outline-stroke`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum StrokeJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// Stroke cap for `path.outline-stroke`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum StrokeCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// Node statistics for `query.node-count`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct NodeCount {
    pub shapes: usize,
    pub contours: usize,
    pub nodes: usize,
    /// Nodes with no handles (only straight segments meet there).
    pub linear: usize,
    /// Nodes with at least one handle.
    pub bezier: usize,
}

pub(crate) fn count(path: &PathData, total: &mut NodeCount) {
    total.shapes += 1;
    total.contours += path.subpaths.len();
    for (_, _, anchor) in path.anchors() {
        total.nodes += 1;
        if anchor.has_in() || anchor.has_out() {
            total.bezier += 1;
        } else {
            total.linear += 1;
        }
    }
}

fn contour_mut(path: &mut PathData, contour: usize) -> Result<&mut SubPath, String> {
    let count = path.subpaths.len();
    path.subpaths
        .get_mut(contour)
        .ok_or_else(|| format!("no contour {contour} (path has {count})"))
}

fn check_node(path: &PathData, at: NodeRef) -> Result<(), String> {
    let sp = path.subpaths.get(at.contour).ok_or_else(|| {
        format!(
            "no contour {} (path has {})",
            at.contour,
            path.subpaths.len()
        )
    })?;
    if at.node >= sp.anchors.len() {
        return Err(format!(
            "contour {} has no node {} (it has {})",
            at.contour,
            at.node,
            sp.anchors.len()
        ));
    }
    Ok(())
}

fn check_nodes(path: &PathData, nodes: &[NodeRef]) -> Result<BTreeSet<NodeRef>, String> {
    if nodes.is_empty() {
        return Err("select at least one node".to_string());
    }
    for at in nodes {
        check_node(path, *at)?;
    }
    Ok(nodes.iter().copied().collect())
}

/// Drops contours left with fewer than two nodes; an edit that leaves nothing is an error.
pub(crate) fn finish(path: &mut PathData) -> Result<(), String> {
    path.subpaths.retain(|sp| sp.anchors.len() >= 2);
    if path.subpaths.is_empty() {
        return Err("the edit would leave no path; remove the shape instead".to_string());
    }
    Ok(())
}

/// Break at a node: a closed contour opens there; an open one splits in two.
pub(crate) fn break_at(path: &mut PathData, at: NodeRef) -> Result<(), String> {
    check_node(path, at)?;
    let sp = contour_mut(path, at.contour)?;
    let n = sp.anchors.len();
    if !sp.closed && (at.node == 0 || at.node + 1 == n) {
        return Err(format!(
            "node {} is already an end of open contour {}",
            at.node, at.contour
        ));
    }
    let pieces = sp.cut_at(&BTreeSet::from([at.node]));
    path.subpaths.splice(at.contour..=at.contour, pieces);
    Ok(())
}

/// Close an open contour; coincident ends merge into one node.
pub(crate) fn close(path: &mut PathData, contour: usize) -> Result<(), String> {
    let sp = contour_mut(path, contour)?;
    if sp.closed {
        return Err(format!("contour {contour} is already closed"));
    }
    if sp.anchors.len() < 2 {
        return Err(format!("contour {contour} needs two nodes to close"));
    }
    let closed = vectorcraft_pathops::join(&[PathData::single(sp.clone())], COINCIDENT_MM);
    path.subpaths.splice(contour..=contour, closed.subpaths);
    Ok(())
}

fn end_point(sp: &SubPath, end: PathEnd) -> Option<Point> {
    match end {
        PathEnd::Start => sp.anchors.first(),
        PathEnd::End => sp.anchors.last(),
    }
    .map(|a| a.p)
}

/// The pair of ends (one per contour) closest to each other.
fn nearest_ends(a: &SubPath, b: &SubPath) -> (PathEnd, PathEnd) {
    let mut best = (f64::INFINITY, PathEnd::End, PathEnd::Start);
    for ea in [PathEnd::End, PathEnd::Start] {
        for eb in [PathEnd::Start, PathEnd::End] {
            if let (Some(pa), Some(pb)) = (end_point(a, ea), end_point(b, eb)) {
                let d = pa.distance(pb);
                if d < best.0 {
                    best = (d, ea, eb);
                }
            }
        }
    }
    (best.1, best.2)
}

/// Join open contour `a` to open contour `b` at the given (or nearest) ends: one open contour
/// running through `a` then `b`. Coincident ends merge; otherwise a straight segment joins them.
pub(crate) fn join(
    mut a: SubPath,
    a_end: Option<PathEnd>,
    mut b: SubPath,
    b_end: Option<PathEnd>,
) -> Result<SubPath, String> {
    if a.closed || b.closed {
        return Err("only open contours can be joined".to_string());
    }
    if a.anchors.is_empty() || b.anchors.is_empty() {
        return Err("cannot join an empty contour".to_string());
    }
    let (near_a, near_b) = nearest_ends(&a, &b);
    let (a_end, b_end) = (a_end.unwrap_or(near_a), b_end.unwrap_or(near_b));
    if a_end == PathEnd::Start {
        a.reverse();
    }
    if b_end == PathEnd::End {
        b.reverse();
    }
    let (Some(last), Some(first)) = (a.anchors.last().copied(), b.anchors.first().copied()) else {
        return Err("cannot join an empty contour".to_string());
    };
    if last.p.distance(first.p) <= COINCIDENT_MM {
        b.anchors.remove(0);
        if let Some(l) = a.anchors.last_mut() {
            l.h_out = first.h_out;
            l.kind = AnchorKind::Corner;
        }
    } else {
        if let Some(l) = a.anchors.last_mut() {
            l.h_out = l.p;
        }
        if let Some(f) = b.anchors.first_mut() {
            f.h_in = f.p;
        }
    }
    a.anchors.extend(b.anchors);
    Ok(a)
}

pub(crate) fn average(
    path: &mut PathData,
    nodes: &[NodeRef],
    axis: NodeAxis,
) -> Result<(), String> {
    let selected = check_nodes(path, nodes)?;
    let selection: Vec<(usize, usize)> = selected.iter().map(|r| (r.contour, r.node)).collect();
    let axis = match axis {
        NodeAxis::Horizontal => vectorcraft_pathops::AverageAxis::Horizontal,
        NodeAxis::Vertical => vectorcraft_pathops::AverageAxis::Vertical,
        NodeAxis::Both => vectorcraft_pathops::AverageAxis::Both,
    };
    *path = vectorcraft_pathops::average(path, &selection, axis);
    Ok(())
}

/// Move the nodes to their centroid, then fuse selected neighbours on the same contour into
/// one corner node (keeping the outer handles). Non-adjacent nodes end up coincident.
pub(crate) fn merge(path: &mut PathData, nodes: &[NodeRef]) -> Result<(), String> {
    if nodes.len() < 2 {
        return Err("select at least two nodes to merge".to_string());
    }
    average(path, nodes, NodeAxis::Both)?;
    let selected: BTreeSet<NodeRef> = nodes.iter().copied().collect();
    for (ci, sp) in path.subpaths.iter_mut().enumerate() {
        let is_selected = |i: usize| {
            selected.contains(&NodeRef {
                contour: ci,
                node: i,
            })
        };
        let mut kept: Vec<(vectorcraft_geom::Anchor, bool)> = Vec::with_capacity(sp.anchors.len());
        for (i, anchor) in sp.anchors.iter().enumerate() {
            let sel = is_selected(i);
            if let Some((last, last_sel)) = kept.last_mut()
                && sel
                && *last_sel
                && last.p.distance(anchor.p) <= COINCIDENT_MM
            {
                last.h_out = anchor.h_out;
                last.kind = AnchorKind::Corner;
                continue;
            }
            kept.push((*anchor, sel));
        }
        // A closed contour also fuses across its seam.
        if sp.closed && kept.len() > 1 {
            let first = kept.first().copied();
            let last = kept.last().copied();
            if let (Some((f, true)), Some((l, true))) = (first, last)
                && f.p.distance(l.p) <= COINCIDENT_MM
            {
                kept.pop();
                if let Some((f, _)) = kept.first_mut() {
                    f.h_in = l.h_in;
                    f.kind = AnchorKind::Corner;
                }
            }
        }
        sp.anchors = kept.into_iter().map(|(a, _)| a).collect();
    }
    finish(path)
}

pub(crate) fn delete_nodes(path: &mut PathData, nodes: &[NodeRef]) -> Result<(), String> {
    let selected = check_nodes(path, nodes)?;
    // Highest index first so earlier indices stay valid.
    for at in selected.iter().rev() {
        let sp = contour_mut(path, at.contour)?;
        if sp.anchors.len() <= 2 {
            sp.anchors.clear();
        } else {
            vectorcraft_pathops::remove_anchor(sp, at.node);
        }
    }
    finish(path)
}

pub(crate) fn set_node_type(
    path: &mut PathData,
    nodes: &[NodeRef],
    kind: NodeType,
) -> Result<(), String> {
    let selected = check_nodes(path, nodes)?;
    for at in selected {
        let sp = contour_mut(path, at.contour)?;
        match kind {
            NodeType::Corner => {
                if let Some(anchor) = sp.anchors.get_mut(at.node) {
                    anchor.retract();
                }
            }
            NodeType::Smooth => {
                if !sp.smooth_anchor(at.node) {
                    return Err(format!(
                        "node {} of contour {} has coincident neighbours and no direction to smooth along",
                        at.node, at.contour
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn set_segment(
    path: &mut PathData,
    contour: usize,
    segment: usize,
    kind: SegmentType,
) -> Result<(), String> {
    let sp = contour_mut(path, contour)?;
    let n = sp.anchors.len();
    if segment >= sp.segment_count() {
        return Err(format!(
            "contour {contour} has no segment {segment} (it has {})",
            sp.segment_count()
        ));
    }
    let (i0, i1) = (segment, (segment + 1) % n);
    let (Some(a), Some(b)) = (
        sp.anchors.get(i0).map(|a| a.p),
        sp.anchors.get(i1).map(|a| a.p),
    ) else {
        return Err(format!("contour {contour} has no segment {segment}"));
    };
    let (out, inn) = match kind {
        SegmentType::Line => (a, b),
        SegmentType::Curve if !sp.segment_is_line(segment) => return Ok(()),
        SegmentType::Curve => (a.lerp(b, 1.0 / 3.0), a.lerp(b, 2.0 / 3.0)),
    };
    if let Some(anchor) = sp.anchors.get_mut(i0) {
        anchor.h_out = out;
        if kind == SegmentType::Line {
            anchor.kind = AnchorKind::Corner;
        }
    }
    if let Some(anchor) = sp.anchors.get_mut(i1) {
        anchor.h_in = inn;
        if kind == SegmentType::Line {
            anchor.kind = AnchorKind::Corner;
        }
    }
    Ok(())
}

pub(crate) fn outline_stroke(
    path: &PathData,
    width_mm: f64,
    join: StrokeJoin,
    cap: StrokeCap,
    miter_limit: f64,
) -> PathData {
    let join = match join {
        StrokeJoin::Miter => vectorcraft_pathops::Join::Miter,
        StrokeJoin::Round => vectorcraft_pathops::Join::Round,
        StrokeJoin::Bevel => vectorcraft_pathops::Join::Bevel,
    };
    let cap = match cap {
        StrokeCap::Butt => vectorcraft_pathops::Cap::Butt,
        StrokeCap::Round => vectorcraft_pathops::Cap::Round,
        StrokeCap::Square => vectorcraft_pathops::Cap::Square,
    };
    vectorcraft_pathops::outline_stroke(path, width_mm, cap, join, miter_limit)
}
