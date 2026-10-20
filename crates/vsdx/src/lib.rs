//! vsdx — reader/writer library for the Visio 2013+ VSDX family (.vsdx/.vstx/.vssx).
//!
//! Format basis: reverse-engineering artifacts in the repository's
//! `Analysis/` directory (OPC container + `visio/document.xml`,
//! `visio/pages/*.xml`, `visio/masters/*.xml` parts; ShapeSheet
//! cell/section model with `V`/`U`/`F` attributes and master
//! inheritance). This crate is a clean-room implementation of those
//! observations; it contains no Microsoft content.
//!
//! v0.2 adds the write path: [`write::save`] serializes a [`Document`]
//! back to an OPC package, preserving everything the model retained
//! (unknown parts verbatim, element order, all attributes, raw markup).

pub mod formula;
pub mod model;
pub mod opc;
pub mod parse;
pub mod write;

pub use model::{Cell, Document, Master, Page, Row, Section, Shape, Sheet};
pub use opc::Package;
pub use write::{canonical_form, save, save_with};
