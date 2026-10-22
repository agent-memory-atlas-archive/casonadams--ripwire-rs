//! Shared vocabulary and helpers language plugins build on; no language-specific logic lives here.

mod complexity_kinds;
mod foreign;
mod syntax_kinds;
pub use complexity_kinds::ComplexityKinds;
pub use foreign::ForeignExport;
pub use syntax_kinds::SyntaxKinds;
pub mod flags;
pub(crate) mod lint;
pub(crate) mod lint_hooks;
pub mod reads;
pub mod source;
