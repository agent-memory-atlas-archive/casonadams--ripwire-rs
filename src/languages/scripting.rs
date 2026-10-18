use super::{
    DefCaptureGateContext, DialectGroup, LanguageSemantics, VisibilityMode,
    resolve_relative_or_exact, strip_import_quotes,
};
use crate::domain::{Lang, SymKind};
use std::collections::HashMap;

pub struct PythonLanguage;
pub struct RubyLanguage;
pub struct LuaLanguage;
pub struct BashLanguage;
pub struct GDScriptLanguage;

pub static PYTHON_LANG: PythonLanguage = PythonLanguage;
pub static RUBY_LANG: RubyLanguage = RubyLanguage;
pub static LUA_LANG: LuaLanguage = LuaLanguage;
pub static BASH_LANG: BashLanguage = BashLanguage;
pub static GDSCRIPT_LANG: GDScriptLanguage = GDScriptLanguage;

impl LanguageSemantics for PythonLanguage {
    fn tag(&self) -> &'static str {
        "py"
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
        is_python_def_allowed(ctx)
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_python_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_python_std_qualifier_name(qualifier)
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

impl LanguageSemantics for RubyLanguage {
    fn tag(&self) -> &'static str {
        "rb"
    }

    fn display_color(&self) -> &'static str {
        "#701516"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Ruby)
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
        ctx.sub != Some("constant") || super::is_screaming_snake(ctx.name)
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_ruby_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_ruby_std_qualifier_name(qualifier)
    }
}

impl LanguageSemantics for LuaLanguage {
    fn tag(&self) -> &'static str {
        "lua"
    }

    fn display_color(&self) -> &'static str {
        "#000080"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Lua)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_lua_std_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_lua_std_qualifier_name(qualifier)
    }
}

impl LanguageSemantics for BashLanguage {
    fn tag(&self) -> &'static str {
        "sh"
    }

    fn display_color(&self) -> &'static str {
        "#89e051"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Bash)
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn supports_slice(&self) -> bool {
        false
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }
}

impl LanguageSemantics for GDScriptLanguage {
    fn tag(&self) -> &'static str {
        "gd"
    }

    fn display_color(&self) -> &'static str {
        "#355570"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::GDScript)
    }

    fn is_dependency_capable(&self) -> bool {
        false
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

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_gdscript_std_member_name(name)
    }
}

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
    node.utf8_text(src).is_ok_and(|name| {
        matches!(
            name,
            "Enum"
                | "IntEnum"
                | "StrEnum"
                | "Flag"
                | "IntFlag"
                | "ReprEnum"
                | "Choices"
                | "TextChoices"
                | "IntegerChoices"
        )
    })
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

fn is_python_def_allowed(ctx: &DefCaptureGateContext<'_>) -> bool {
    if ctx.kind == SymKind::Field {
        return is_py_self_field_capture(ctx.name_node, ctx.def_node, ctx.source_bytes);
    }
    if ctx.sub == Some("enummember") {
        return is_py_enum_member_target(ctx.name_node, ctx.source_bytes);
    }
    true
}

fn is_python_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "append"
            | "extend"
            | "insert"
            | "remove"
            | "pop"
            | "clear"
            | "index"
            | "count"
            | "sort"
            | "reverse"
            | "copy"
            | "get"
            | "keys"
            | "values"
            | "items"
            | "update"
            | "setdefault"
            | "split"
            | "rsplit"
            | "splitlines"
            | "join"
            | "strip"
            | "lstrip"
            | "rstrip"
            | "replace"
            | "find"
            | "rfind"
            | "startswith"
            | "endswith"
            | "lower"
            | "upper"
            | "format"
            | "encode"
            | "decode"
            | "read"
            | "write"
            | "readline"
            | "readlines"
            | "close"
            | "flush"
            | "add"
            | "discard"
            | "loads"
            | "dumps"
            | "load"
            | "dump"
            | "match"
            | "search"
            | "sub"
            | "findall"
            | "compile"
            | "exists"
            | "is_file"
            | "is_dir"
            | "mkdir"
            | "unlink"
            | "resolve"
            | "info"
            | "debug"
            | "warning"
            | "error"
            | "exception"
    )
}

fn is_python_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "os" | "sys"
            | "re"
            | "json"
            | "math"
            | "time"
            | "datetime"
            | "pathlib"
            | "Path"
            | "itertools"
            | "functools"
            | "collections"
            | "subprocess"
            | "logging"
            | "typing"
            | "shutil"
            | "tempfile"
            | "hashlib"
            | "base64"
            | "urllib"
            | "asyncio"
            | "pytest"
            | "unittest"
            | "dict"
            | "list"
            | "set"
            | "str"
            | "bytes"
    )
}

fn is_ruby_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "new"
            | "each"
            | "map"
            | "select"
            | "reject"
            | "find"
            | "detect"
            | "collect"
            | "inject"
            | "reduce"
            | "flat_map"
            | "compact"
            | "flatten"
            | "uniq"
            | "sort"
            | "sort_by"
            | "reverse"
            | "first"
            | "last"
            | "take"
            | "drop"
            | "count"
            | "length"
            | "size"
            | "empty?"
            | "include?"
            | "any?"
            | "all?"
            | "none?"
            | "nil?"
            | "to_s"
            | "to_i"
            | "to_f"
            | "to_a"
            | "to_h"
            | "to_sym"
            | "to_json"
            | "inspect"
            | "freeze"
            | "dup"
            | "clone"
            | "keys"
            | "values"
            | "fetch"
            | "delete"
            | "merge"
            | "slice"
            | "strip"
            | "chomp"
            | "gsub"
            | "sub"
            | "split"
            | "join"
            | "downcase"
            | "upcase"
            | "start_with?"
            | "end_with?"
            | "match"
            | "match?"
    )
}

fn is_ruby_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "File"
            | "Dir"
            | "IO"
            | "JSON"
            | "YAML"
            | "Time"
            | "Date"
            | "DateTime"
            | "Regexp"
            | "Kernel"
            | "Object"
            | "Array"
            | "Hash"
            | "String"
            | "Integer"
            | "Float"
            | "Symbol"
            | "Proc"
            | "Thread"
            | "Mutex"
            | "ENV"
            | "Math"
            | "URI"
    )
}

fn is_lua_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "insert"
            | "remove"
            | "concat"
            | "sort"
            | "unpack"
            | "pack"
            | "sub"
            | "find"
            | "match"
            | "gmatch"
            | "gsub"
            | "rep"
            | "reverse"
            | "lower"
            | "upper"
            | "len"
            | "byte"
            | "char"
            | "format"
            | "floor"
            | "ceil"
            | "max"
            | "min"
            | "abs"
            | "sqrt"
            | "open"
            | "close"
            | "read"
            | "write"
            | "lines"
            | "flush"
    )
}

fn is_lua_std_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "table"
            | "string"
            | "math"
            | "io"
            | "os"
            | "coroutine"
            | "package"
            | "debug"
            | "utf8"
            | "bit"
            | "vim"
    )
}

fn is_gdscript_std_member_name(name: &str) -> bool {
    matches!(
        name,
        "append"
            | "erase"
            | "find"
            | "has"
            | "is_empty"
            | "size"
            | "clear"
            | "duplicate"
            | "keys"
            | "values"
            | "get"
            | "set"
            | "get_node"
            | "add_child"
            | "remove_child"
            | "queue_free"
            | "instantiate"
            | "connect"
            | "emit_signal"
    )
}
