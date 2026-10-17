use super::{
    DefCaptureGateContext, DialectGroup, LanguageSemantics, resolve_relative_or_exact,
    strip_import_quotes,
};
use crate::domain::SymKind;
use std::collections::HashMap;

pub struct CLanguage;
pub struct CppLanguage;
pub struct CudaLanguage;
pub struct MetalLanguage;
pub struct ObjCLanguage;

pub static C_LANG: CLanguage = CLanguage;
pub static CPP_LANG: CppLanguage = CppLanguage;
pub static CUDA_LANG: CudaLanguage = CudaLanguage;
pub static METAL_LANG: MetalLanguage = MetalLanguage;
pub static OBJC_LANG: ObjCLanguage = ObjCLanguage;

fn resolve_c_family_include(
    importer_path: &str,
    raw_target: &str,
    file_index: &HashMap<&str, u32>,
) -> Option<u32> {
    let trimmed = raw_target.trim();
    if trimmed.starts_with('<') {
        return None;
    }
    let clean = strip_import_quotes(trimmed);
    resolve_relative_or_exact(importer_path, clean, file_index, &[".h", ".hpp", ".hh"])
}
fn is_c_family_keyword(name: &str) -> bool {
    matches!(
        name,
        "static_cast"
            | "dynamic_cast"
            | "const_cast"
            | "reinterpret_cast"
            | "sizeof"
            | "alignof"
            | "decltype"
            | "typeid"
            | "static_assert"
            | "noexcept"
    )
}

const TEST_BLOCK_MACRO_NAMES: &[&str] = &[
    "TEST_CASE",
    "TEST_CASE_FIXTURE",
    "TEST_CASE_METHOD",
    "SCENARIO",
    "TEST_SUITE",
];

fn is_screaming_snake(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_uppercase()
        && bytes
            .iter()
            .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

fn matches_child_token(
    ch: tree_sitter::Node<'_>,
    src: &[u8],
    kind_filter: &str,
    allow_anon: bool,
    tokens: &[&str],
) -> bool {
    let named = ch.is_named();
    if (named && ch.kind() != kind_filter) || (!named && !allow_anon) {
        return false;
    }
    ch.utf8_text(src).is_ok_and(|t| tokens.contains(&t))
}

fn has_child_token(
    node: tree_sitter::Node<'_>,
    src: &[u8],
    kind_filter: &str,
    allow_anon: bool,
    tokens: &[&str],
) -> bool {
    (0..node.child_count())
        .filter_map(|i| node.child(i))
        .any(|ch| matches_child_token(ch, src, kind_filter, allow_anon, tokens))
}

fn has_const_qualifier(node: tree_sitter::Node<'_>, src: &[u8]) -> bool {
    has_child_token(
        node,
        src,
        "type_qualifier",
        false,
        &["const", "constexpr", "constinit"],
    )
}

fn has_static_storage(node: tree_sitter::Node<'_>, src: &[u8]) -> bool {
    has_child_token(node, src, "storage_class_specifier", false, &["static"])
}

fn is_bound_by_init_declarator(
    name_node: Option<tree_sitter::Node<'_>>,
    def_node: tree_sitter::Node<'_>,
) -> bool {
    let mut curr = name_node.and_then(|n| n.parent());
    while let Some(n) = curr {
        if n == def_node {
            break;
        }
        if n.kind() == "init_declarator" {
            return true;
        }
        curr = n.parent();
    }
    false
}

fn is_cpp_global_constant_allowed(
    dn: tree_sitter::Node<'_>,
    name_node: Option<tree_sitter::Node<'_>>,
    name: &str,
    src: &[u8],
) -> bool {
    let initialized = is_bound_by_init_declarator(name_node, dn);
    if initialized {
        return is_screaming_snake(name)
            || has_const_qualifier(dn, src)
            || has_child_token(dn, src, "type_qualifier", true, &["__constant__"]);
    }
    has_child_token(dn, src, "type_qualifier", true, &["__constant__"])
        || (is_screaming_snake(name)
            && has_child_token(
                dn,
                src,
                "type_qualifier",
                true,
                &["__device__", "__managed__"],
            ))
}

fn is_c_constant_allowed(ctx: &DefCaptureGateContext<'_>, is_cpp: bool) -> bool {
    let Some(dn) = ctx.def_node else {
        return false;
    };
    if !is_cpp {
        return is_screaming_snake(ctx.name) || has_const_qualifier(dn, ctx.source_bytes);
    }
    if dn.kind() == "field_declaration" {
        return has_static_storage(dn, ctx.source_bytes)
            && has_const_qualifier(dn, ctx.source_bytes);
    }
    is_cpp_global_constant_allowed(dn, ctx.name_node, ctx.name, ctx.source_bytes)
}

fn has_pre_body_error(node: tree_sitter::Node<'_>) -> bool {
    for i in 0..node.child_count() {
        let Some(ch) = node.child(i) else {
            continue;
        };
        if ch.kind() == "compound_statement" {
            break;
        }
        if ch.is_error() || ch.kind() == "ERROR" {
            return true;
        }
    }
    false
}

fn unwrap_declarator_container(fn_decl: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut decl = fn_decl.parent();
    while let Some(p) = decl {
        if matches!(
            p.kind(),
            "pointer_declarator"
                | "reference_declarator"
                | "parenthesized_declarator"
                | "attributed_declarator"
                | "function_declarator"
        ) {
            decl = p.parent();
        } else {
            return Some(p);
        }
    }
    None
}

fn is_invalid_c_callable_scope(fn_decl: tree_sitter::Node<'_>) -> bool {
    if fn_decl.has_error() {
        return true;
    }
    let Some(container) = unwrap_declarator_container(fn_decl) else {
        return true;
    };
    if !matches!(
        container.kind(),
        "function_definition" | "declaration" | "field_declaration" | "template_declaration"
    ) || has_pre_body_error(container)
    {
        return true;
    }
    let mut curr = container.parent();
    while let Some(p) = curr {
        match p.kind() {
            "compound_statement" | "ERROR" | "if_statement" | "for_statement"
            | "while_statement" => return true,
            "field_declaration_list" | "declaration_list" | "translation_unit" => return false,
            _ => curr = p.parent(),
        }
    }
    false
}

fn has_cpp_header_markers(text: &str) -> bool {
    text.contains("namespace ")
        || text.contains("template<")
        || text.contains("template <")
        || text.contains("class ")
        || text.contains("constexpr")
        || text.contains("std::")
        || text.contains("::")
        || text.contains("public:")
        || text.contains("private:")
        || text.contains("noexcept")
}

fn refine_c_header_dialect(
    base_lang: crate::domain::Lang,
    rel_path: &str,
    source_bytes: &[u8],
) -> crate::domain::Lang {
    if !rel_path.ends_with(".h") {
        return base_lang;
    }
    if let Ok(text) = std::str::from_utf8(source_bytes) {
        if text.contains("@interface ") || text.contains("@protocol ") || text.contains("@property")
        {
            return crate::domain::Lang::ObjC;
        }
        if base_lang == crate::domain::Lang::C && !has_cpp_header_markers(text) {
            return crate::domain::Lang::C;
        }
    }
    crate::domain::Lang::Cpp
}

fn is_c_family_def_allowed(ctx: &DefCaptureGateContext<'_>, is_cpp: bool) -> bool {
    match ctx.sub {
        Some("testmacroblock") => TEST_BLOCK_MACRO_NAMES.contains(&ctx.name),
        Some("constant") => is_c_constant_allowed(ctx, is_cpp),
        Some("specialization") => false,
        _ => match ctx.kind {
            SymKind::Function | SymKind::Method => {
                !ctx.def_node.is_some_and(is_invalid_c_callable_scope)
            }
            SymKind::Field => ctx
                .def_node
                .is_none_or(|dn| !has_static_storage(dn, ctx.source_bytes)),
            _ => true,
        },
    }
}

fn resolve_c_family_def_body_and_end<'a>(
    def_node: Option<tree_sitter::Node<'a>>,
    default_end: usize,
) -> (Option<tree_sitter::Node<'a>>, usize) {
    let Some(dn) = def_node else {
        return (None, default_end);
    };
    if dn.kind() == "expression_statement"
        && let Some(sib) = dn.next_named_sibling()
        && sib.kind() == "compound_statement"
    {
        return (Some(sib), sib.end_byte());
    }
    if let Some(parent) = super::find_enclosing_definition_node(dn) {
        return (Some(parent), parent.end_byte());
    }
    (Some(dn), default_end)
}

impl LanguageSemantics for CLanguage {
    fn tag(&self) -> &'static str {
        "c"
    }

    fn display_color(&self) -> &'static str {
        "#555555"
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
        is_c_family_keyword(name)
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        is_c_family_def_allowed(ctx, false)
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

impl LanguageSemantics for CppLanguage {
    fn tag(&self) -> &'static str {
        "cpp"
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
        is_c_family_keyword(name)
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

impl LanguageSemantics for CudaLanguage {
    fn tag(&self) -> &'static str {
        "cuda"
    }

    fn display_color(&self) -> &'static str {
        "#3a87ad"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn supports_c_layout(&self) -> bool {
        true
    }
    fn is_builtin_keyword(&self, name: &str) -> bool {
        is_c_family_keyword(name)
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

impl LanguageSemantics for MetalLanguage {
    fn tag(&self) -> &'static str {
        "metal"
    }

    fn display_color(&self) -> &'static str {
        "#3a87ad"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
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

impl LanguageSemantics for ObjCLanguage {
    fn tag(&self) -> &'static str {
        "m"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn supports_c_layout(&self) -> bool {
        true
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

#[cfg(test)]
mod tests {
    use crate::domain::Lang;
    use crate::ingest::{LanguageRegistry, ingest_in_memory_files};

    #[test]
    fn test_cpp_capture_gates_filter_noise_and_keep_constants() {
        let registry = LanguageRegistry::new().unwrap();
        let interner = crate::domain::Interner::new();
        let cpp_src = br#"
static const int MAX_BUF = 4096;
constexpr int kValidConst = 7;
int mutable_global = 3;
int uninitialized_global;
__constant__ float d_const_table[16];
__device__ int D_DEV_CAP;
__device__ int d_mutable_dev;

struct Widget {
    int count = 0;
    static int s_mutable;
    static constexpr int kClassConst = 42;
};

void runner() {
    std::lock_guard<std::mutex> guard(mtx);
    if (guard.owns_lock()) {}
}

TEST_CASE("widget test") {
    runner();
}
"#;
        let c_src = b"static const int C_CONST = 1;\nint c_mut = 2;\n";
        let ingest = ingest_in_memory_files(
            &[("test.cpp", Lang::Cpp, cpp_src), ("test.c", Lang::C, c_src)],
            &registry,
            &interner,
        );
        let names: Vec<&str> = ingest.symbols.iter().map(|s| &interner[s.name]).collect();
        assert!(names.contains(&"MAX_BUF"));
        assert!(names.contains(&"kValidConst"));
        assert!(names.contains(&"kClassConst"));
        assert!(names.contains(&"count"));
        assert!(names.contains(&"runner"));
        assert!(names.contains(&"TEST_CASE"));
        assert!(names.contains(&"C_CONST"));
        assert!(!names.contains(&"mutable_global"));
        assert!(!names.contains(&"uninitialized_global"));
        assert!(!names.contains(&"s_mutable"));
        assert!(!names.contains(&"guard"));
        assert!(!names.contains(&"c_mut"));
    }

    #[test]
    fn test_header_dialect_refinement_and_error_recovery_guard() {
        let registry = LanguageRegistry::new().unwrap();
        let interner = crate::domain::Interner::new();
        let cpp_hdr =
            b"namespace rw { inline bool kindIs(const char* a, const char* b) { return true; } }\n";
        let objc_hdr = b"@interface Renderer : NSObject\n- (void)draw;\n@end\n";
        let pure_c_hdr = b"int c_header_fn(int a);\n";
        let ingest = ingest_in_memory_files(
            &[
                ("include/nodekind.h", Lang::C, cpp_hdr),
                ("include/Renderer.h", Lang::Cpp, objc_hdr),
                ("include/pure.h", Lang::C, pure_c_hdr),
            ],
            &registry,
            &interner,
        );
        assert_eq!(ingest.files[0].lang, Lang::Cpp);
        assert_eq!(ingest.files[1].lang, Lang::ObjC);
        assert_eq!(ingest.files[2].lang, Lang::C);
        let names: Vec<&str> = ingest.symbols.iter().map(|s| &interner[s.name]).collect();
        assert!(names.contains(&"kindIs"));
    }
}
