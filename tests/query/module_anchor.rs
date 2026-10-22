use ripwire_rs::query::retrieval::{ForTaskOptions, call_for_task_with_options};
use ripwire_rs::{IngestResult, Interner, Lang, SymKind, Symbol};

/// Ranks a function that the name-exact route would otherwise lose to, plus one `search` declaration
/// of `kind`/`lang`, and returns the route and the first ranked symbol's name.
fn route_for_search_declaration(kind: SymKind, lang: Lang) -> (String, String) {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    let function = Symbol::new(
        0,
        interner.intern("call_pattern_search"),
        interner.intern("call_pattern_search"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );
    let declaration = Symbol::new(
        1,
        interner.intern("search"),
        interner.intern("search"),
        kind,
        lang,
        0,
        1,
    );
    ingest.symbols.push(function);
    ingest.symbols.push(declaration);
    let res = call_for_task_with_options(
        &ingest,
        &interner,
        &[1.0, 0.0],
        "search",
        ForTaskOptions::default(),
    );
    (
        res["route"].as_str().unwrap().to_string(),
        res["symbols"][0]["name"].as_str().unwrap().to_string(),
    )
}

#[test]
fn test_rust_module_declaration_is_the_exact_name_anchor() {
    let (route, first) = route_for_search_declaration(SymKind::Section, Lang::Rust);
    assert_eq!(route, "name-exact(search)");
    assert_eq!(first, "search");
}

#[test]
fn test_markdown_heading_named_like_the_query_is_not_an_anchor() {
    let (route, first) = route_for_search_declaration(SymKind::Section, Lang::Markdown);
    assert_ne!(route, "name-exact(search)");
    assert_eq!(first, "call_pattern_search");
}
