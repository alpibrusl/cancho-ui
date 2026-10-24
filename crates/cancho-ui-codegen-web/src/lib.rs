//! cancho-ui-codegen-web — JavaScript + DOM emitter.
//!
//! Phase 1 produces a single ES module that, when loaded in a page
//! with a `<div id="root">`, mounts the (first) component to the
//! root. The emitted code is direct DOM construction — no virtual
//! DOM, no JSX, no framework.
//!
//! The shape is deliberately the smallest JavaScript that is
//! correct, readable, and source-mappable. Phase 2 will:
//! - emit source maps,
//! - split per-component into modules,
//! - wire DOM event handlers,
//! - emit accessibility (ARIA) attributes from the IR,
//! - add a minimal stylesheet (cui-col, cui-row).

mod adapters;
mod controls;
pub mod emit;
mod expr;
mod i18n;
mod lists;
mod map;
mod number;
mod sortlist;
mod stream;
mod view;

pub use emit::{EmitOptions, emit_module, emit_module_with, module_for_file};
