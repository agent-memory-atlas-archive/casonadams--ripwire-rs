use super::c::{
    C_FAMILY_KEYWORDS, CPP_STD_MEMBERS, CPP_STD_QUALIFIERS, blank_c_family_macro_lines,
    is_c_family_def_allowed, refine_c_header_dialect, resolve_c_family_def_body_and_end,
    resolve_c_family_include,
};
use super::{
    ComplexityKinds, DefCaptureGateContext, DialectGroup, ForeignExport, LanguageSemantics,
};
use crate::domain::Lang;
use std::collections::HashMap;

pub struct CppLanguage;
pub static CPP_LANG: CppLanguage = CppLanguage;

const KINDS: ComplexityKinds = ComplexityKinds {
    nesting: &[
        "if_statement",
        "for_statement",
        "for_range_loop",
        "while_statement",
        "do_statement",
        "catch_clause",
        "conditional_expression",
    ],
    jumps: &[
        "break_statement",
        "continue_statement",
        "goto_statement",
        "return_statement",
        "throw_statement",
        "co_return_statement",
    ],
    ..ComplexityKinds::C_STYLE
};

/// Python module macros whose `.def("name", &fn)` calls expose C++ definitions to Python.
const PYTHON_MODULE_MACROS: &[&[u8]] = &[b"PYBIND11_MODULE", b"BOOST_PYTHON_MODULE"];

fn string_literal_text<'a>(node: tree_sitter::Node<'_>, source: &'a [u8]) -> Option<&'a str> {
    (node.kind() == "string_literal")
        .then(|| node.utf8_text(source).ok())??
        .strip_prefix('"')?
        .strip_suffix('"')
}

/// `.def("exported", &target)` / `.def("exported", &Scope::target)` as a Python export.
fn python_def_export(call: tree_sitter::Node<'_>, source: &[u8]) -> Option<ForeignExport> {
    let field = call
        .child_by_field_name("function")?
        .child_by_field_name("field")?;
    if field.utf8_text(source).ok()? != "def" {
        return None;
    }
    let args = call.child_by_field_name("arguments")?;
    let exported = string_literal_text(args.named_child(0)?, source)?;
    let pointer = args.named_child(1)?;
    if pointer.kind() != "pointer_expression" {
        return None;
    }
    let target = pointer.child_by_field_name("argument")?;
    let (target_scope, target_name) = match target.kind() {
        "identifier" => (None, target),
        "qualified_identifier" => (
            target.child_by_field_name("scope"),
            target.child_by_field_name("name")?,
        ),
        _ => return None,
    };
    Some(ForeignExport {
        exported: exported.to_string(),
        target: target_name.utf8_text(source).ok()?.to_string(),
        target_scope: target_scope
            .and_then(|s| s.utf8_text(source).ok())
            .map(str::to_string),
        caller: Lang::Python,
    })
}

fn python_module_exports(root: tree_sitter::Node<'_>, source: &[u8]) -> Vec<ForeignExport> {
    let mut exports = Vec::new();
    let mut cursor = root.walk();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "call_expression"
            && let Some(export) = python_def_export(node, source)
        {
            exports.push(export);
        }
        stack.extend(node.named_children(&mut cursor));
    }
    exports
}

impl LanguageSemantics for CppLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }
    fn foreign_exports(&self, root: tree_sitter::Node<'_>, source: &[u8]) -> Vec<ForeignExport> {
        let declares_module = PYTHON_MODULE_MACROS
            .iter()
            .any(|m| source.windows(m.len()).any(|w| w == *m));
        if declares_module {
            python_module_exports(root, source)
        } else {
            Vec::new()
        }
    }
    fn tag(&self) -> &'static str {
        "cpp"
    }
    super::impl_grammar_method!(crate tree_sitter_cpp::LANGUAGE, "../queries/cpp/tags.scm");

    fn lint_return_type<'a>(&self, sig: &'a str, name: &str) -> Option<&'a str> {
        super::lint_hooks::prefix_return_type(sig, name)
    }

    fn type_declaration_keywords(&self) -> &'static [&'static str] {
        &["typedef", "enum", "using"]
    }

    fn supports_cache_lint(&self) -> bool {
        true
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["h", "cpp", "cc", "cxx", "hpp", "hxx", "hh"]
    }

    fn display_color(&self) -> &'static str {
        "#f34b7d"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn supports_c_layout(&self) -> bool {
        true
    }

    fn refine_dialect(
        &self,
        base_lang: crate::domain::Lang,
        rel_path: &str,
        source_bytes: &[u8],
    ) -> crate::domain::Lang {
        refine_c_header_dialect(base_lang, rel_path, source_bytes)
    }

    fn is_builtin_keyword(&self, name: &str) -> bool {
        C_FAMILY_KEYWORDS.contains(&name)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        CPP_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        CPP_STD_QUALIFIERS
    }

    fn blank_error_macros(&self, source_bytes: &[u8]) -> Option<Vec<u8>> {
        blank_c_family_macro_lines(source_bytes)
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        is_c_family_def_allowed(ctx, true)
    }

    fn resolve_def_body_and_end<'a>(
        &self,
        def_node: Option<tree_sitter::Node<'a>>,
        default_end: usize,
    ) -> (Option<tree_sitter::Node<'a>>, usize) {
        resolve_c_family_def_body_and_end(def_node, default_end)
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        resolve_c_family_include(importer_path, raw_target, file_index)
    }
}
