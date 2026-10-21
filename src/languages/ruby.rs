use super::{DefCaptureGateContext, DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct RubyLanguage;
pub static RUBY_LANG: RubyLanguage = RubyLanguage;

const RUBY_STD_MEMBERS: &[&str] = &[
    "new",
    "each",
    "map",
    "select",
    "reject",
    "find",
    "detect",
    "collect",
    "inject",
    "reduce",
    "flat_map",
    "compact",
    "flatten",
    "uniq",
    "sort",
    "sort_by",
    "reverse",
    "first",
    "last",
    "take",
    "drop",
    "count",
    "length",
    "size",
    "empty?",
    "include?",
    "any?",
    "all?",
    "none?",
    "nil?",
    "to_s",
    "to_i",
    "to_f",
    "to_a",
    "to_h",
    "to_sym",
    "to_json",
    "inspect",
    "freeze",
    "dup",
    "clone",
    "keys",
    "values",
    "fetch",
    "delete",
    "merge",
    "slice",
    "strip",
    "chomp",
    "gsub",
    "sub",
    "split",
    "join",
    "downcase",
    "upcase",
    "start_with?",
    "end_with?",
    "match",
    "match?",
];

const RUBY_STD_QUALIFIERS: &[&str] = &[
    "File", "Dir", "IO", "JSON", "YAML", "Time", "Date", "DateTime", "Regexp", "Kernel", "Object",
    "Array", "Hash", "String", "Integer", "Float", "Symbol", "Proc", "Thread", "Mutex", "ENV",
    "Math", "URI",
];

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

    fn parameters_shadow_calls(&self) -> bool {
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

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        ctx.sub != Some("constant") || super::is_screaming_snake(ctx.name)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        RUBY_STD_MEMBERS
    }
    super::impl_grammar_method!(crate tree_sitter_ruby::LANGUAGE, "../queries/ruby/tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["rb", "rake", "jbuilder"]
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        RUBY_STD_QUALIFIERS
    }
}
