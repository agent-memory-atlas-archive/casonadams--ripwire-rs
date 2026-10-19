use crate::domain::Lang;
use std::cell::RefCell;
use tree_sitter::{Language, Parser, Query, QueryCursor};

thread_local! {
    pub static PARSER_STATE: RefCell<(Parser, QueryCursor)> = RefCell::new((
        Parser::new(),
        QueryCursor::new(),
    ));
}

/// Registry of supported Tree-Sitter languages and compiled queries.
#[derive(Default, Clone, Copy)]
pub struct LanguageRegistry;

impl LanguageRegistry {
    /// Initialize registry with all supported 36 languages.
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self)
    }

    /// Initialize registry with Tier-1 languages.
    pub fn new_tier1() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new()
    }

    /// Find compiled language and query for given Lang.
    pub fn get(&self, lang: Lang) -> Option<(&Language, &Query)> {
        lang.semantics().grammar_and_query()
    }
}
