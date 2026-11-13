//! App integrations around StencilCraft's shared document model.
//!
//! Native document types, validation, geometry helpers and `.stencilcraft` encoding are owned by
//! `craft-stencilcraft-document`. This crate keeps application library/recovery features, built-in
//! templates, and dispatch to other supported file formats.

pub mod format;
pub mod library;
pub mod recovery;
pub mod templates;

pub use craft_stencilcraft_document::*;
