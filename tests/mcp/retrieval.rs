use ripwire_rs::mcp::retrieval::{
    ForTaskOptions, call_exemplar, call_for_task, call_for_task_with_options, call_lego,
    collect_query_file_hits, compute_adaptive_cut, match_pattern_on_text, parse_for_sections,
};
use ripwire_rs::{
    FileInfo, Graph, IngestResult, Interner, Lang, LanguageRegistry, PageRankConfig, RefRole,
    Reference, SymKind, Symbol, ingest_in_memory_files, serialize_exemplar_xml, serialize_for_xml,
};
use serde_json::{Value, json};

#[test]
fn test_parse_for_sections_is_a_closed_set() {
    assert_eq!(parse_for_sections("lego").unwrap(), vec!["lego"]);
    assert_eq!(
        parse_for_sections("compose, lego").unwrap(),
        vec!["compose", "lego"]
    );
    assert!(
        parse_for_sections("lego,lego")
            .unwrap_err()
            .contains("named twice")
    );
    assert!(
        parse_for_sections("legos")
            .unwrap_err()
            .contains("unknown section")
    );
    assert!(parse_for_sections("").is_err());
}

#[test]
fn test_for_lego_section_stub_by_default_full_on_request() {
    let mut val = json!({
        "task": "parse flags",
        "symbols": [],
        "lego": [{
            "name": "VerbGroup",
            "path": "src/cli/args.rs",
            "line": 30,
            "implementors": [
                { "name": "TopNavArgs", "path": "src/cli/args.rs", "line": 122 },
                { "name": "TopDetailArgs", "path": "src/cli/args.rs", "line": 142 }
            ]
        }]
    });
    let render = |v: &Value| {
        let mut out = Vec::new();
        serialize_for_xml(v, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    };
    let stub = render(&val);
    assert!(stub.contains(
        "<lego total=\"1\" shown=\"0\" capped=\"1\" next=\"--for=&apos;parse flags&apos; --sections=lego\"/>"
    ));
    assert!(!stub.contains("<iface "));

    val["sections"] = json!(["lego"]);
    let full = render(&val);
    assert!(full.contains("<lego><iface n=\"VerbGroup\" p=\"src/cli/args.rs:30\" implementors=\"2\"><impl n=\"TopNavArgs\" p=\"src/cli/args.rs:122\"/>"));
    assert!(!full.contains("<lego total="));

    val["lego"] = json!([]);
    assert!(!render(&val).contains("<lego"));
    let mut cval = json!({
        "task": "parse flags",
        "symbols": [],
        "compose": [{ "name": "graph", "owner": "NavContext", "rel": "uses" }],
        "hops": [{ "name": "run", "path": "src/main.rs", "line": 10, "total": 1, "calls": [{ "name": "init", "line": 2 }] }]
    });
    let cstub = render(&cval);
    assert!(cstub.contains("<compose total=\"1\" shown=\"0\" capped=\"1\""));
    assert!(cstub.contains("<hops shown=\"1\" total=\"1\" capped=\"0\"><h l=\"10\" p=\"src/main.rs\" n=\"run\"><calls total=\"1\"><c n=\"init\" l=\"2\"/></calls></h></hops>"));
    cval["sections"] = json!(["compose"]);
    let cfull = render(&cval);
    assert!(
        cfull.contains(
            "<compose><field name=\"graph\" owner=\"NavContext\" rel=\"uses\"/></compose>"
        )
    );
}

#[test]
fn test_call_exemplar_roles() {
    let interner = Interner::new();
    let ingest = IngestResult::new();
    for role in [
        "fn",
        "method",
        "class",
        "struct",
        "interface",
        "section",
        "other",
    ] {
        let res = call_exemplar(&ingest, &interner, &[], role, None);
        assert!(res.get("error").is_some());
    }
    let mut ing = IngestResult::new();
    ing.files.push(FileInfo {
        id: 0,
        path: interner.intern("src/lib.rs"),
        lang: Lang::Rust,
        line_count: 20,
        byte_size: 100,
    });
    let mut s0 = Symbol::new(
        0,
        interner.intern("caller_fn"),
        interner.intern("src/lib.rs::caller_fn"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );
    s0.end_byte = 50;
    ing.symbols.push(s0);
    ing.symbols.push(Symbol::new(
        1,
        interner.intern("callee_fn"),
        interner.intern("src/lib.rs::callee_fn"),
        SymKind::Method,
        Lang::Rust,
        0,
        10,
    ));
    ing.references.push(Reference::new(
        0,
        interner.intern("callee_fn"),
        0,
        2,
        Lang::Rust,
        RefRole::Call,
    ));
    let g = Graph::build(&ing, &interner);
    let ex = call_exemplar(&ing, &interner, &[0.8, 0.2], "fn", Some(&g));
    assert_eq!(ex["calls"].as_array().unwrap().len(), 1);
    let mut xml_buf = Vec::new();
    serialize_exemplar_xml(".", &ex, &mut xml_buf).unwrap();
    assert!(
        String::from_utf8(xml_buf)
            .unwrap()
            .contains("<calls total=\"1\">")
    );
}

#[test]
fn test_call_for_task_with_detail() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    let sym = Symbol::new(
        0,
        interner.intern("test_fn"),
        interner.intern("c_test_fn"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );
    ingest.symbols.push(sym);
    let res = call_for_task(&ingest, &interner, &[1.0], "test fn", None, Some(1), false);
    assert_eq!(res["count"], 1);
    assert_eq!(res["detail"], 1);

    for i in 1..6 {
        ingest.symbols.push(Symbol::new(
            i,
            interner.intern(&format!("session_secret_{i}")),
            interner.intern(&format!("c_{i}")),
            SymKind::Function,
            Lang::Rust,
            0,
            i + 1,
        ));
    }
    ingest.symbols.push(Symbol::new(
        6,
        interner.intern("lsp_manager"),
        interner.intern("c_lsp"),
        SymKind::Struct,
        Lang::Rust,
        0,
        10,
    ));
    let ranks = vec![1.0; ingest.symbols.len()];
    let div_res = call_for_task(
        &ingest,
        &interner,
        &ranks,
        "lsp manager session secret",
        None,
        None,
        false,
    );
    let top_names: Vec<&str> = div_res["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .take(5)
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(top_names.contains(&"lsp_manager"));
}

#[test]
fn test_compute_adaptive_cut() {
    assert_eq!(compute_adaptive_cut(&[10.0, 5.0], 5, 10), (2, 2, 0.0));
    let scores = [10.0, 9.0, 8.0, 2.0, 1.0];
    let (k, shown, gap) = compute_adaptive_cut(&scores, 1, 5);
    assert_eq!(k, 3);
    assert_eq!(shown, 3);
    assert!(gap >= 0.25);
    let no_drop = [10.0, 9.9, 9.8, 9.7];
    let (k_all, _, _) = compute_adaptive_cut(&no_drop, 1, 4);
    assert_eq!(k_all, 4);
}

#[test]
fn test_pattern_and_ast_match() {
    let matches = match_pattern_on_text(
        "pub class Foo { bar(); }\nfn other() {}",
        &["class", "{", "}"],
    );
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0, 1);

    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let code = b"pub struct Widget {}\npub fn build() {}";
    let ts_code = b"export const materializeUrl = async (rawPath: string): Promise<string> => {\n  const x = rawPath.trim();\n  return x;\n};\n";
    let mut ingest = ingest_in_memory_files(
        &[
            ("src/lib.rs", Lang::Rust, code),
            ("src/url.ts", Lang::TypeScript, ts_code),
        ],
        &reg,
        &interner,
    );
    let hits = collect_query_file_hits(
        &ingest.files[0],
        code,
        "(struct_item name: (type_identifier) @name)",
        &ingest,
        &interner,
    )
    .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["text"], "Widget");

    let arrow_sym = ingest
        .symbols
        .iter()
        .find(|s| interner.resolve(&s.name) == "materializeUrl")
        .expect("materializeUrl");
    assert!(arrow_sym.has_body());
    assert!(
        !arrow_sym
            .extract_signature(ts_code, "materializeUrl")
            .contains("rawPath.trim()")
    );

    let next_id = ingest.symbols.len() as u32;
    ingest.symbols.push(
        Symbol::new(
            next_id,
            interner.intern("count"),
            interner.intern("src/lib.rs::Widget::count"),
            SymKind::Field,
            Lang::Rust,
            0,
            1,
        )
        .with_scope(interner.intern("Widget")),
    );
    ingest.references.push(Reference::new(
        0,
        interner.intern("build"),
        0,
        1,
        Lang::Rust,
        RefRole::Call,
    ));
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&PageRankConfig::default());
    let res = call_for_task_with_options(
        &ingest,
        &interner,
        &pr.ranks,
        "Widget build",
        ForTaskOptions {
            root: ".",
            graph: Some(&graph),
            budget_tokens: None,
            detail: Some(1),
            adaptive: false,
        },
    );
    assert!(!res["compose"].as_array().unwrap().is_empty());
    assert!(!res["hops"].as_array().unwrap().is_empty());
}

#[test]
fn test_camel_case_and_derivational_stem_scoring() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("packages/core/src/ast/loader.ts"),
        lang: Lang::TypeScript,
        line_count: 20,
        byte_size: 200,
    });
    ingest.files.push(FileInfo {
        id: 1,
        path: interner.intern("packages/core/src/controllers/tree-controller.ts"),
        lang: Lang::TypeScript,
        line_count: 20,
        byte_size: 200,
    });
    ingest.symbols.push(Symbol::new(
        0,
        interner.intern("discoverAndLoadASTNodes"),
        interner.intern("discoverAndLoadASTNodes"),
        SymKind::Function,
        Lang::TypeScript,
        0,
        1,
    ));
    ingest.symbols.push(Symbol::new(
        1,
        interner.intern("reloadTrees"),
        interner.intern("reloadTrees"),
        SymKind::Function,
        Lang::TypeScript,
        1,
        1,
    ));
    let res = call_for_task(
        &ingest,
        &interner,
        &[0.001, 0.001],
        "ast node cache tree discovery validation resolution",
        None,
        None,
        false,
    );
    let syms = res["symbols"].as_array().unwrap();
    assert_eq!(syms[0]["name"], "discoverAndLoadASTNodes");
}

#[test]
fn test_call_lego_includes_out_of_line_methods_for_unique_class() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("src/infra/Diagnostics.h"),
        lang: Lang::Cpp,
        line_count: 50,
        byte_size: 500,
    });
    ingest.files.push(FileInfo {
        id: 1,
        path: interner.intern("src/infra/diagnostics.cpp"),
        lang: Lang::Cpp,
        line_count: 50,
        byte_size: 500,
    });
    let cls_spur = interner.intern("ConsoleLog");
    ingest.symbols.push(Symbol::new(
        0,
        cls_spur,
        interner.intern("src/infra/Diagnostics.h::ConsoleLog"),
        SymKind::Class,
        Lang::Cpp,
        0,
        10,
    ));
    ingest.symbols.push(
        Symbol::new(
            1,
            interner.intern("handlePanic"),
            interner.intern("src/infra/Diagnostics.h::ConsoleLog::handlePanic"),
            SymKind::Function,
            Lang::Cpp,
            0,
            15,
        )
        .with_scope(cls_spur),
    );
    ingest.symbols.push(
        Symbol::new(
            2,
            interner.intern("handleAssert"),
            interner.intern("src/infra/diagnostics.cpp::ConsoleLog::handleAssert"),
            SymKind::Method,
            Lang::Cpp,
            1,
            25,
        )
        .with_scope(cls_spur),
    );
    let lego = call_lego(&ingest, &interner, "ConsoleLog");
    let methods = lego["methods"].as_array().unwrap();
    assert_eq!(methods.len(), 2);
}
