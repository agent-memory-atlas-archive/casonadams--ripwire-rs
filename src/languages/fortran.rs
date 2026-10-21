use super::{DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct FortranLanguage;
pub static FORTRAN_LANG: FortranLanguage = FortranLanguage;

const FORTRAN_STD_MEMBERS: &[&str] = &[
    "abs",
    "aimag",
    "aint",
    "anint",
    "ceiling",
    "floor",
    "modulo",
    "mod",
    "real",
    "dble",
    "cmplx",
    "int",
    "nint",
    "conjg",
    "dim",
    "max",
    "min",
    "sign",
    "sqrt",
    "exp",
    "log",
    "log10",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "sinh",
    "cosh",
    "tanh",
    "matmul",
    "dot_product",
    "transpose",
    "sum",
    "product",
    "all",
    "any",
    "count",
    "maxval",
    "minval",
    "maxloc",
    "minloc",
    "size",
    "shape",
    "lbound",
    "ubound",
    "reshape",
    "trim",
    "adjustl",
    "adjustr",
    "index",
    "scan",
    "verify",
    "len",
    "len_trim",
    "repeat",
    "allocated",
    "associated",
    "present",
    "null",
    "c_loc",
    "c_funloc",
    "c_f_pointer",
    "c_associated",
];

const FORTRAN_STD_QUALIFIERS: &[&str] = &[
    "iso_c_binding",
    "iso_fortran_env",
    "ieee_arithmetic",
    "ieee_exceptions",
    "ieee_features",
    "omp_lib",
];

impl LanguageSemantics for FortranLanguage {
    fn tag(&self) -> &'static str {
        "fortran"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["f", "for", "f90", "f95", "f03", "f08"]
    }

    super::impl_grammar_method!(crate tree_sitter_fortran::LANGUAGE, "../queries/fortran/tags.scm");

    fn display_color(&self) -> &'static str {
        "#734f96"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Fortran)
    }
    fn resolve_def_params_and_name(
        &self,
        def_node: Option<tree_sitter::Node<'_>>,
        _source_bytes: &[u8],
        _kind: crate::domain::SymKind,
        orig_name: &str,
    ) -> (u16, String) {
        let params = def_node
            .and_then(|dn| {
                if dn.kind() == "subroutine" || dn.kind() == "function" {
                    dn.child(0).map(crate::gate::contract::extract_param_count)
                } else {
                    Some(crate::gate::contract::extract_param_count(dn))
                }
            })
            .unwrap_or(0);
        (params, orig_name.to_string())
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::DefaultPublic
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_')
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        FORTRAN_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        FORTRAN_STD_QUALIFIERS
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        self.std_member_names()
            .iter()
            .any(|s| s.eq_ignore_ascii_case(name))
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        self.std_qualifier_names()
            .iter()
            .any(|s| s.eq_ignore_ascii_case(qualifier))
    }
}
