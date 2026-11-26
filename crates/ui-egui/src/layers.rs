//! Pure helpers behind the Geometry and Operations panels (D-08/D-09).
//!
//! Everything here is derived from the document on each frame: the Unassigned
//! group, pass numbers and operation tags are never persisted into the document
//! — the UI reads `Document.groups` and `Document.operations` directly, with no
//! adapter layer.

use std::collections::BTreeSet;

use stencilcraft_doc::{Document, Group, GroupChild, OperationKind};

/// Sentinel open-set key for the derived Unassigned group (a real group id can
/// never collide with it).
pub const UNASSIGNED_GROUP: u64 = u64::MAX;

/// One row of the geometry tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeoRow {
    /// The ".." navigation row shown while a group is entered.
    Up,
    /// A group header row; `depth` drives the indent.
    Group { id: u64, depth: usize },
    /// A shape leaf inside a group.
    Shape { id: u64, depth: usize },
    /// The derived Unassigned group header (never persisted, D-09).
    UnassignedGroup,
    /// A shape that belongs to no group, listed under Unassigned in document order.
    UnassignedShape { id: u64 },
}

fn group_by_id(doc: &Document, id: u64) -> Option<&Group> {
    doc.groups.iter().find(|g| g.id == id)
}

/// Ids of the groups that are not nested inside another group, in document order.
pub fn top_level_groups(doc: &Document) -> Vec<u64> {
    let nested: BTreeSet<u64> = doc
        .groups
        .iter()
        .flat_map(|g| g.children.iter())
        .filter_map(|child| match child {
            GroupChild::Group(id) => Some(*id),
            GroupChild::Shape(_) => None,
        })
        .collect();
    doc.groups
        .iter()
        .map(|g| g.id)
        .filter(|id| !nested.contains(id))
        .collect()
}

/// Every shape id inside group `id`, nested groups included, in child order.
pub fn group_member_shapes(doc: &Document, id: u64) -> Vec<u64> {
    fn walk(doc: &Document, id: u64, out: &mut Vec<u64>) {
        let Some(group) = group_by_id(doc, id) else {
            return;
        };
        for child in &group.children {
            match child {
                GroupChild::Shape(shape) => {
                    if !out.contains(shape) {
                        out.push(*shape);
                    }
                }
                GroupChild::Group(inner) => walk(doc, *inner, out),
            }
        }
    }
    let mut out = Vec::new();
    walk(doc, id, &mut out);
    out
}

/// Shape ids that belong to no group, in document order.
pub fn ungrouped_shape_ids(doc: &Document) -> Vec<u64> {
    let grouped: BTreeSet<u64> = doc
        .groups
        .iter()
        .flat_map(|g| group_member_shapes(doc, g.id))
        .collect();
    doc.shapes()
        .map(|s| s.id)
        .filter(|id| !grouped.contains(id))
        .collect()
}

fn push_group_rows(
    doc: &Document,
    id: u64,
    depth: usize,
    open: &BTreeSet<u64>,
    rows: &mut Vec<GeoRow>,
) {
    let Some(group) = group_by_id(doc, id) else {
        return;
    };
    rows.push(GeoRow::Group { id, depth });
    if !open.contains(&id) {
        return;
    }
    for child in &group.children {
        match child {
            GroupChild::Group(inner) => push_group_rows(doc, *inner, depth + 1, open, rows),
            GroupChild::Shape(shape) => rows.push(GeoRow::Shape {
                id: *shape,
                depth: depth + 1,
            }),
        }
    }
}

/// The geometry tree rows: when `entered` is a group, its subtree behind an
/// `Up` row; otherwise the top-level groups. The derived Unassigned group with
/// the ungrouped shapes always closes the list.
pub fn build_geometry_rows(
    doc: &Document,
    open: &BTreeSet<u64>,
    entered: Option<u64>,
) -> Vec<GeoRow> {
    let mut rows = Vec::new();
    match entered {
        Some(id) => {
            rows.push(GeoRow::Up);
            if let Some(group) = group_by_id(doc, id) {
                for child in &group.children {
                    match child {
                        GroupChild::Group(inner) => {
                            push_group_rows(doc, *inner, 0, open, &mut rows)
                        }
                        GroupChild::Shape(shape) => rows.push(GeoRow::Shape {
                            id: *shape,
                            depth: 0,
                        }),
                    }
                }
            }
        }
        None => {
            for id in top_level_groups(doc) {
                push_group_rows(doc, id, 0, open, &mut rows);
            }
        }
    }
    rows.push(GeoRow::UnassignedGroup);
    if open.contains(&UNASSIGNED_GROUP) {
        for id in ungrouped_shape_ids(doc) {
            rows.push(GeoRow::UnassignedShape { id });
        }
    }
    rows
}

/// 1-based pass number of operation `op_index` in `Document::operations` order;
/// `None` for Guide/hidden operations (they produce no pass) and out-of-range.
pub fn pass_number(doc: &Document, op_index: usize) -> Option<usize> {
    if !doc.operations.get(op_index)?.is_output() {
        return None;
    }
    Some(
        doc.operations
            .iter()
            .take(op_index + 1)
            .filter(|op| op.is_output())
            .count(),
    )
}

/// Indexes into `Document::operations` of every operation whose members contain
/// `shape_id`, in pass order (N:M membership, D-08).
pub fn operation_tags(doc: &Document, shape_id: u64) -> Vec<usize> {
    doc.operations
        .iter()
        .enumerate()
        .filter(|(_, op)| op.members.contains(&shape_id))
        .map(|(i, _)| i)
        .collect()
}

/// Display icon-name hint for an operation kind in the operations panel.
pub fn icon_name_for_kind(kind: OperationKind) -> Option<&'static str> {
    Some(match kind {
        OperationKind::Cut => "scissors",
        OperationKind::Engrave => "flame",
        OperationKind::Score => "spline",
        OperationKind::Guide => "ruler",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_doc::{Group, Layer, Operation, OperationSettings};
    use stencilcraft_testkit::rect_path;

    fn op(id: u64, kind: OperationKind, members: Vec<u64>) -> Operation {
        Operation {
            id,
            name: format!("op{id}"),
            kind,
            settings: OperationSettings::default(),
            color: None,
            visible: true,
            locked: false,
            members,
            icon: None,
        }
    }

    /// Shapes 1..=4 on a base layer; groups: outer(1) = [shape 1, inner(2)],
    /// inner(2) = [shape 2, shape 3]; shape 4 belongs to no group.
    /// Operations: op 1 = cut over [1, 2], op 2 = guide over [2], op 3 = engrave over [3].
    fn fixture() -> Document {
        let mut doc = Document::default();
        doc.layers.push(Layer::new("base"));
        for i in 1..=4u64 {
            doc.push_shape(0, rect_path(f64::from(i as u32) * 10.0, 0.0, 5.0, 5.0))
                .unwrap();
        }
        doc.groups.push(Group {
            id: 1,
            name: "outer".to_string(),
            children: vec![GroupChild::Shape(1), GroupChild::Group(2)],
        });
        doc.groups.push(Group {
            id: 2,
            name: "inner".to_string(),
            children: vec![GroupChild::Shape(2), GroupChild::Shape(3)],
        });
        doc.operations.push(op(1, OperationKind::Cut, vec![1, 2]));
        doc.operations.push(op(2, OperationKind::Guide, vec![2]));
        doc.operations.push(op(3, OperationKind::Engrave, vec![3]));
        doc
    }

    #[test]
    fn geometry_rows_walk_groups_recursively_and_close_with_unassigned() {
        let doc = fixture();
        let rows = build_geometry_rows(&doc, &BTreeSet::new(), None);
        assert_eq!(
            rows,
            vec![GeoRow::Group { id: 1, depth: 0 }, GeoRow::UnassignedGroup]
        );

        let open: BTreeSet<u64> = [1, 2, UNASSIGNED_GROUP].into_iter().collect();
        let rows = build_geometry_rows(&doc, &open, None);
        assert_eq!(
            rows,
            vec![
                GeoRow::Group { id: 1, depth: 0 },
                GeoRow::Shape { id: 1, depth: 1 },
                GeoRow::Group { id: 2, depth: 1 },
                GeoRow::Shape { id: 2, depth: 2 },
                GeoRow::Shape { id: 3, depth: 2 },
                GeoRow::UnassignedGroup,
                GeoRow::UnassignedShape { id: 4 },
            ]
        );
        // Nothing is persisted: the Unassigned group is derived only.
        assert_eq!(doc.groups.len(), 2);
    }

    #[test]
    fn entering_a_group_shows_its_subtree_behind_an_up_row() {
        let doc = fixture();
        let open: BTreeSet<u64> = [2].into_iter().collect();
        let rows = build_geometry_rows(&doc, &open, Some(1));
        assert_eq!(
            rows,
            vec![
                GeoRow::Up,
                GeoRow::Shape { id: 1, depth: 0 },
                GeoRow::Group { id: 2, depth: 0 },
                GeoRow::Shape { id: 2, depth: 1 },
                GeoRow::Shape { id: 3, depth: 1 },
                GeoRow::UnassignedGroup,
            ]
        );
    }

    #[test]
    fn group_member_shapes_is_recursive_and_unioned() {
        let doc = fixture();
        assert_eq!(group_member_shapes(&doc, 1), vec![1, 2, 3]);
        assert_eq!(group_member_shapes(&doc, 2), vec![2, 3]);
        assert!(group_member_shapes(&doc, 99).is_empty());
        assert_eq!(ungrouped_shape_ids(&doc), vec![4]);
    }

    #[test]
    fn pass_number_counts_only_output_operations_in_order() {
        let doc = fixture();
        assert_eq!(pass_number(&doc, 0), Some(1), "cut is pass 1");
        assert_eq!(pass_number(&doc, 1), None, "guide produces no pass");
        assert_eq!(pass_number(&doc, 2), Some(2), "engrave is pass 2");
        assert_eq!(pass_number(&doc, 3), None, "out of range");

        let mut doc = fixture();
        doc.operations[0].visible = false;
        assert_eq!(
            pass_number(&doc, 0),
            None,
            "hidden operation produces no pass"
        );
        assert_eq!(pass_number(&doc, 2), Some(1), "engrave renumbers to pass 1");
    }

    #[test]
    fn operation_tags_lists_every_operation_of_a_shape_in_pass_order() {
        let doc = fixture();
        assert_eq!(operation_tags(&doc, 1), vec![0]);
        assert_eq!(operation_tags(&doc, 2), vec![0, 1]);
        assert_eq!(operation_tags(&doc, 3), vec![2]);
        assert!(operation_tags(&doc, 4).is_empty());
    }
}
