//! fieldconfig — user-editable YAML configuration for mock data generation.
//!
//! Module structure:
//!   types.rs    — FieldConfig / FieldKind data structures
//!   infer.rs    — auto-infer FieldConfig from ColumnSchema
//!   serialize.rs — YAML export (with comments) and import
//!   generate.rs  — config-driven value generator

pub mod generate;
pub mod infer;
pub mod serialize;
pub mod types;

pub use infer::MockConfig;
pub use types::{FieldConfig, FieldKind};
