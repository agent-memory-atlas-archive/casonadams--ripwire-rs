use super::{
    DialectGroup, LanguageSemantics, VisibilityMode, resolve_relative_or_exact, strip_import_quotes,
};
use crate::domain::Lang;
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

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
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
        )
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
}
