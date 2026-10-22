//! Shared vocabulary and helpers language plugins build on; no language-specific logic lives here.

mod complexity_kinds;
mod foreign;
pub mod keywords;
mod syntax_kinds;
pub use complexity_kinds::ComplexityKinds;
pub use foreign::ForeignExport;
pub use syntax_kinds::SyntaxKinds;
pub mod flags;
pub(crate) mod lint;
pub(crate) mod lint_hooks;
pub mod reads;
pub mod source;

/// Families of lint rules a language opts into; a rule fires on every language declaring its group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintGroup {
    Cast,
    Goto,
    DoWhile,
    Insecure,
    Parens,
    CFamily,
    SelfAssign,
    NativeOnly,
    Unreachable,
    Naming,
}
