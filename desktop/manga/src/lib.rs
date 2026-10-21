//! The manga engine: source definitions, the six questions a reader asks of a source, and the
//! means to test a definition against recorded pages.

pub mod check;
pub mod date;
pub mod def;
pub mod eval;
pub mod fetch;
pub mod model;
pub mod repo;
pub mod source;
pub mod store;
pub mod template;
pub mod theme;

pub use def::SourceDef;
pub use source::Source;
