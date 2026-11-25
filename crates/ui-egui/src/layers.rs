//! Pure helpers behind the Layers panel tree (D-08/D-09/D-12).
//!
//! Everything here is derived from the document on each frame. The Unassigned
//! group and the Pass N badges are never persisted — no layer, enum variant or
//! stored field is ever created for them (Phase 1 D-04, D-09).

use std::collections::BTreeSet;

use stencilcraft_doc::{Document, Operation};

/// Sentinel entry in [`build_layer_rows`]' `open` set that expands the derived
/// Unassigned group (a real layer index can never collide with it).
pub const UNASSIGNED_GROUP: usize = usize::MAX;

/// One row of the layers tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerRow {
    /// A layer header row; `index` is into `Document::layers`.
    Layer { index: usize },
    /// A member shape leaf under an expanded layer group.
    Member { layer: usize, shape: u64 },
    /// The derived Unassigned group header (never a persisted layer, D-09).
    UnassignedGroup,
    /// A zero-membership shape leaf under the Unassigned group, in document order.
    UnassignedMember { shape: u64 },
}

/// 1-based index of `layer_index` in `doc.production_sequence()`; `None` for
/// layers that do not produce (operation-less containers and Guide layers, D-12).
pub fn pass_number(doc: &Document, layer_index: usize) -> Option<usize> {
    if !doc.layers.get(layer_index)?.produces() {
        return None;
    }
    Some(
        doc.layers
            .iter()
            .take(layer_index + 1)
            .filter(|l| l.produces())
            .count(),
    )
}

/// The tree rows for the layers panel: one group row per layer plus member
/// leaves for open groups, then the Unassigned group with the zero-membership
/// shapes (`Document::unassigned_shapes`) in document order.
pub fn build_layer_rows(doc: &Document, open: &BTreeSet<usize>) -> Vec<LayerRow> {
    let mut rows = Vec::new();
    for index in 0..doc.layers.len() {
        rows.push(LayerRow::Layer { index });
        if open.contains(&index)
            && let Some(layer) = doc.layers.get(index)
        {
            for &shape in &layer.members {
                rows.push(LayerRow::Member {
                    layer: index,
                    shape,
                });
            }
        }
    }
    rows.push(LayerRow::UnassignedGroup);
    if open.contains(&UNASSIGNED_GROUP) {
        for shape in doc.unassigned_shapes() {
            rows.push(LayerRow::UnassignedMember { shape: shape.id });
        }
    }
    rows
}

/// The operation a shape effectively runs with and the name of the owning
/// operation layer; `None` for unassigned shapes.
pub fn effective_operation(doc: &Document, shape_id: u64) -> Option<(Operation, String)> {
    doc.layers
        .iter()
        .find(|l| l.operation.is_some() && l.members.contains(&shape_id))
        .and_then(|l| l.operation.clone().map(|op| (op, l.name.clone())))
}

/// Names of every operation layer containing the shape (N:M membership, D-08).
pub fn membership_labels(doc: &Document, shape_id: u64) -> Vec<String> {
    doc.layers
        .iter()
        .filter(|l| l.operation.is_some() && l.members.contains(&shape_id))
        .map(|l| l.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_doc::{Layer, OperationSettings};
    use stencilcraft_testkit::rect_path;

    fn cut() -> Operation {
        Operation::Cut(OperationSettings::default())
    }

    fn engrave() -> Operation {
        Operation::Engrave(OperationSettings::default())
    }

    /// `Document::default()`'s "base" container (index 0) / cut / guide / engrave,
    /// with shape 1 in cut and engrave, shape 2 in base only, shape 3 in no layer.
    fn doc() -> Document {
        let mut doc = Document::default();
        doc.layers.push(Layer::new("cut", Some(cut())));
        doc.layers.push(Layer::new("guide", Some(Operation::Guide)));
        doc.layers.push(Layer::new("engrave", Some(engrave())));
        let one = doc.push_shape(1, rect_path(0.0, 0.0, 10.0, 10.0)).unwrap();
        doc.layers[3].members.push(one.id);
        doc.push_shape(0, rect_path(20.0, 0.0, 10.0, 10.0)).unwrap();
        doc.shapes.push(stencilcraft_doc::Shape {
            id: 3,
            path: rect_path(40.0, 0.0, 10.0, 10.0),
            hidden: false,
            locked: false,
            text: None,
            production: Default::default(),
        });
        doc.next_shape_id = 4;
        doc
    }

    #[test]
    fn pass_number_skips_container_and_guide_layers_and_renumbers_after_reorder() {
        let mut doc = doc();
        assert_eq!(pass_number(&doc, 0), None, "operation-less container");
        assert_eq!(pass_number(&doc, 1), Some(1), "first producing layer");
        assert_eq!(pass_number(&doc, 2), None, "Guide produces nothing (D-17)");
        assert_eq!(pass_number(&doc, 3), Some(2), "second producing layer");
        assert_eq!(pass_number(&doc, 4), None, "out of range");

        doc.layers.swap(1, 3);
        assert_eq!(pass_number(&doc, 1), Some(1), "engrave moved to the front");
        assert_eq!(pass_number(&doc, 3), Some(2), "cut renumbered");
    }

    /// The engine's `layer.move` arm is `remove(from); insert(to)` — this test
    /// applies that exact move (the through-the-engine path is covered by
    /// `reorder_changes_pass_order` in stencilcraft-engine) and asserts the
    /// derived Pass N badges renumber. Adding stencilcraft-engine as a dev-dep
    /// here would force a Cargo.lock re-resolution (T-02-SC: no dependency
    /// changes in this plan), so the move is applied the same way instead.
    #[test]
    fn pass_number_renumbers_after_layer_move() {
        let mut doc = doc();
        assert_eq!(pass_number(&doc, 1), Some(1), "cut is Pass 1");
        assert_eq!(pass_number(&doc, 3), Some(2), "engrave is Pass 2");

        // LayerMove { from: 3, to: 1 } as the engine arm applies it.
        let layer = doc.layers.remove(3);
        doc.layers.insert(1, layer);
        assert_eq!(
            pass_number(&doc, 1),
            Some(1),
            "engrave moved to the front is Pass 1"
        );
        assert_eq!(pass_number(&doc, 2), Some(2), "cut renumbered to Pass 2");
    }

    #[test]
    fn build_layer_rows_lists_exactly_the_unassigned_shapes_and_persists_nothing() {
        let doc = doc();
        let open: BTreeSet<usize> = [1, UNASSIGNED_GROUP].into_iter().collect();
        let rows = build_layer_rows(&doc, &open);
        assert_eq!(
            rows,
            vec![
                LayerRow::Layer { index: 0 },
                LayerRow::Layer { index: 1 },
                LayerRow::Member { layer: 1, shape: 1 },
                LayerRow::Layer { index: 2 },
                LayerRow::Layer { index: 3 },
                LayerRow::UnassignedGroup,
                LayerRow::UnassignedMember { shape: 2 },
                LayerRow::UnassignedMember { shape: 3 },
            ]
        );
        // Shape 2 sits in the operation-less container only, so it is unassigned too.
        assert_eq!(
            doc.unassigned_shapes().map(|s| s.id).collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(
            doc.layers.len(),
            4,
            "no Unassigned layer is ever persisted (D-09)"
        );

        // A collapsed Unassigned group still renders, without its leaves.
        let rows = build_layer_rows(&doc, &BTreeSet::new());
        assert_eq!(rows.last(), Some(&LayerRow::UnassignedGroup));
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r, LayerRow::UnassignedMember { .. }))
        );
    }

    #[test]
    fn effective_operation_resolves_the_owning_layer_and_none_for_unassigned() {
        let doc = doc();
        let (op, source) = effective_operation(&doc, 1).unwrap();
        assert_eq!(op, cut());
        assert_eq!(source, "cut");
        assert_eq!(effective_operation(&doc, 2), None);
        assert_eq!(effective_operation(&doc, 3), None);
        assert_eq!(effective_operation(&doc, 99), None);
    }

    #[test]
    fn membership_labels_lists_every_owning_operation_layer_n_to_m() {
        let doc = doc();
        assert_eq!(
            membership_labels(&doc, 1),
            vec!["cut".to_string(), "engrave".to_string()]
        );
        assert!(membership_labels(&doc, 2).is_empty());
        assert!(membership_labels(&doc, 3).is_empty());
    }
}
