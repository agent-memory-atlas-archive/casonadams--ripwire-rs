use super::{
    DefCaptureGateContext, DialectGroup, LanguageSemantics, VisibilityMode,
    resolve_relative_or_exact, strip_import_quotes,
};
use crate::domain::{Lang, SymKind};
use std::collections::HashMap;

pub struct PythonLanguage;
pub static PYTHON_LANG: PythonLanguage = PythonLanguage;

const PYTHON_STD_MEMBERS: &[&str] = &[
    "append",
    "extend",
    "insert",
    "remove",
    "pop",
    "clear",
    "index",
    "count",
    "sort",
    "reverse",
    "copy",
    "get",
    "keys",
    "values",
    "items",
    "update",
    "setdefault",
    "split",
    "rsplit",
    "splitlines",
    "join",
    "strip",
    "lstrip",
    "rstrip",
    "replace",
    "find",
    "rfind",
    "startswith",
    "endswith",
    "lower",
    "upper",
    "format",
    "encode",
    "decode",
    "read",
    "write",
    "readline",
    "readlines",
    "close",
    "flush",
    "add",
    "discard",
    "loads",
    "dumps",
    "load",
    "dump",
    "match",
    "search",
    "sub",
    "findall",
    "compile",
    "exists",
    "is_file",
    "is_dir",
    "mkdir",
    "unlink",
    "resolve",
    "info",
    "debug",
    "warning",
    "error",
    "exception",
];

const PYTHON_STD_QUALIFIERS: &[&str] = &[
    "os",
    "sys",
    "re",
    "json",
    "math",
    "time",
    "datetime",
    "pathlib",
    "Path",
    "itertools",
    "functools",
    "collections",
    "subprocess",
    "logging",
    "typing",
    "shutil",
    "tempfile",
    "hashlib",
    "base64",
    "urllib",
    "asyncio",
    "pytest",
    "unittest",
    "dict",
    "list",
    "set",
    "str",
    "bytes",
];

const PYTHON_ENUM_BASES: &[&str] = &[
    "Enum",
    "IntEnum",
    "StrEnum",
    "Flag",
    "IntFlag",
    "ReprEnum",
    "Choices",
    "TextChoices",
    "IntegerChoices",
];

fn has_enclosing_method_in_class(node: tree_sitter::Node<'_>) -> bool {
    let mut saw_fn = false;
    let mut curr = node.parent();
    while let Some(up) = curr {
        match up.kind() {
            "function_definition" => saw_fn = true,
            "class_definition" => return saw_fn,
            _ => {}
        }
        curr = up.parent();
    }
    false
}

fn is_py_self_field_capture(
    name_node: Option<tree_sitter::Node<'_>>,
    def_node: Option<tree_sitter::Node<'_>>,
    src: &[u8],
) -> bool {
    let (Some(nn), Some(dn)) = (name_node, def_node) else {
        return false;
    };
    let Some(attr) = nn.parent().filter(|p| p.kind() == "attribute") else {
        return false;
    };
    let is_self = attr
        .child_by_field_name("object")
        .is_some_and(|o| o.kind() == "identifier" && o.utf8_text(src) == Ok("self"));
    is_self && has_enclosing_method_in_class(dn)
}

fn is_py_enum_base_node(base: tree_sitter::Node<'_>, src: &[u8]) -> bool {
    let ident = if base.kind() == "attribute" {
        base.child_by_field_name("attribute")
    } else {
        Some(base)
    };
    let Some(node) = ident.filter(|n| n.kind() == "identifier") else {
        return false;
    };
    node.utf8_text(src)
        .is_ok_and(|name| PYTHON_ENUM_BASES.contains(&name))
}

fn is_py_enum_member_target(name_node: Option<tree_sitter::Node<'_>>, src: &[u8]) -> bool {
    let Some(cls) = name_node
        .and_then(|n| n.parent())
        .and_then(|n| n.parent())
        .and_then(|n| n.parent())
        .and_then(|n| n.parent())
        .filter(|c| c.kind() == "class_definition")
    else {
        return false;
    };
    let Some(bases) = cls.child_by_field_name("superclasses") else {
        return false;
    };
    (0..bases.named_child_count())
        .filter_map(|i| bases.named_child(i))
        .any(|b| is_py_enum_base_node(b, src))
}

impl LanguageSemantics for PythonLanguage {
    fn tag(&self) -> &'static str {
        "py"
    }
    super::impl_grammar_method!(crate tree_sitter_python::LANGUAGE, "../queries/python/tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["py", "pyi"]
    }

    fn display_color(&self) -> &'static str {
        "#3572a5"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Python)
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        if ctx.kind == SymKind::Field {
            return is_py_self_field_capture(ctx.name_node, ctx.def_node, ctx.source_bytes);
        }
        if ctx.sub == Some("enummember") {
            return is_py_enum_member_target(ctx.name_node, ctx.source_bytes);
        }
        true
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        PYTHON_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        PYTHON_STD_QUALIFIERS
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        let clean = strip_import_quotes(raw_target);
        let rel_mod = clean.trim_start_matches('.').replace('.', "/");
        if rel_mod.is_empty() {
            return None;
        }
        resolve_relative_or_exact(
            importer_path,
            &rel_mod,
            file_index,
            &[".py", "/__init__.py"],
        )
    }
}
