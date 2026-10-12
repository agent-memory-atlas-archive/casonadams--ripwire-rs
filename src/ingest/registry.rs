use crate::domain::Lang;
use std::cell::RefCell;
use std::sync::{Arc, LazyLock};
use tree_sitter::{Language, Parser, Query, QueryCursor};

unsafe extern "C" {
    fn tree_sitter_elixir() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_dart() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_kotlin() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_ruby() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_php() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_lua() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_gdscript() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_json() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_toml() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_yaml() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_markdown() -> *const tree_sitter::ffi::TSLanguage;
}

macro_rules! lang_entry {
    ($lang:expr, $query_src:expr) => {
        LazyLock::new(|| {
            let ts: Language = $lang;
            let q = Query::new(&ts, $query_src).expect("valid query");
            (ts, Arc::new(q))
        })
    };
}

thread_local! {
    pub static PARSER_STATE: RefCell<(Parser, QueryCursor)> = RefCell::new((
        Parser::new(),
        QueryCursor::new(),
    ));
}

static LANG_TABLE: [LazyLock<(Language, Arc<Query>)>; 27] = [
    lang_entry!(
        tree_sitter_rust::LANGUAGE.into(),
        include_str!("../queries/rust/tags.scm")
    ),
    lang_entry!(
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        include_str!("../queries/typescript/tags.scm")
    ),
    lang_entry!(
        tree_sitter_javascript::LANGUAGE.into(),
        include_str!("../queries/javascript/tags.scm")
    ),
    lang_entry!(
        tree_sitter_python::LANGUAGE.into(),
        include_str!("../queries/python/tags.scm")
    ),
    lang_entry!(
        tree_sitter_go::LANGUAGE.into(),
        include_str!("../queries/go/tags.scm")
    ),
    lang_entry!(
        tree_sitter_c::LANGUAGE.into(),
        include_str!("../queries/c/tags.scm")
    ),
    lang_entry!(
        tree_sitter_cpp::LANGUAGE.into(),
        include_str!("../queries/cpp/tags.scm")
    ),
    lang_entry!(
        tree_sitter_cuda::LANGUAGE.into(),
        include_str!("../queries/cuda/tags.scm")
    ),
    lang_entry!(
        tree_sitter_cpp::LANGUAGE.into(),
        include_str!("../queries/cpp/tags.scm")
    ),
    lang_entry!(
        tree_sitter_java::LANGUAGE.into(),
        include_str!("../queries/java/tags.scm")
    ),
    lang_entry!(
        tree_sitter_c_sharp::LANGUAGE.into(),
        include_str!("../queries/csharp/tags.scm")
    ),
    lang_entry!(
        tree_sitter_swift::LANGUAGE.into(),
        include_str!("../queries/swift/tags.scm")
    ),
    lang_entry!(
        tree_sitter_bash::LANGUAGE.into(),
        include_str!("../queries/bash/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_ruby()) },
        include_str!("../queries/ruby/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_php()) },
        include_str!("../queries/php/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_lua()) },
        include_str!("../queries/lua/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_elixir()) },
        include_str!("../queries/elixir/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_dart()) },
        include_str!("../queries/dart/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_kotlin()) },
        include_str!("../queries/kotlin/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_gdscript()) },
        include_str!("../queries/gdscript/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_json()) },
        include_str!("../queries/json/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_toml()) },
        include_str!("../queries/toml/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_yaml()) },
        include_str!("../queries/yaml/tags.scm")
    ),
    lang_entry!(unsafe { Language::from_raw(tree_sitter_markdown()) }, ""),
    lang_entry!(
        tree_sitter_zig::LANGUAGE.into(),
        include_str!("../queries/zig/tags.scm")
    ),
    lang_entry!(
        tree_sitter_solidity::LANGUAGE.into(),
        include_str!("../queries/solidity/tags.scm")
    ),
    lang_entry!(
        tree_sitter_hcl::LANGUAGE.into(),
        include_str!("../queries/hcl/tags.scm")
    ),
];

/// Registry of supported Tree-Sitter languages and compiled queries.
#[derive(Default, Clone, Copy)]
pub struct LanguageRegistry;

impl LanguageRegistry {
    /// Initialize registry with all supported 24 languages.
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self)
    }

    /// Initialize registry with Tier-1 languages.
    pub fn new_tier1() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new()
    }

    /// Find compiled language and query for given Lang.
    pub fn get(&self, lang: Lang) -> Option<(&Language, &Query)> {
        let idx = lang as usize;
        if idx < LANG_TABLE.len() {
            let (ts, q) = &*LANG_TABLE[idx];
            Some((ts, q.as_ref()))
        } else {
            None
        }
    }
}
