use ripwire_rs::navigate::{
    GraphQueryPageOptions, evaluate_verify_claim, execute_graph_query_expr,
    execute_graph_query_expr_paged, execute_pack_task, find_symbols_by_name, navigate_1hop,
    serialize_callers_xml, serialize_impact_with_importers_xml, transitive_impact,
    truncate_body_to_budget,
};
use ripwire_rs::serialize::{GraphGauges, PathLayerExt};
use ripwire_rs::{
    FileInfo, Graph, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
};

#[test]
fn test_navigate_1hop_and_impact() {
    let interner = Interner::new();
    let f0 = FileInfo {
        id: 0,
        path: interner.intern("src/lib.rs"),
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 500,
    };

    let sym_a = Symbol::new(
        0,
        interner.intern("a"),
        interner.intern("c_a"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );
    let sym_b = Symbol::new(
        1,
        interner.intern("b"),
        interner.intern("c_b"),
        SymKind::Function,
        Lang::Rust,
        0,
        5,
    );
    let sym_c = Symbol::new(
        2,
        interner.intern("c"),
        interner.intern("c_c"),
        SymKind::Function,
        Lang::Rust,
        0,
        10,
    );

    let ref_ab = Reference::new(0, interner.intern("b"), 0, 2, Lang::Rust, RefRole::Call);
    let ref_bc = Reference::new(1, interner.intern("c"), 0, 6, Lang::Rust, RefRole::Call);

    let mut ingest = IngestResult::new();
    ingest.files.push(f0);
    ingest.symbols.extend_from_slice(&[sym_a, sym_b, sym_c]);
    ingest.references.extend_from_slice(&[ref_ab, ref_bc]);

    let graph = Graph::build(&ingest, &interner);
    let ranks = vec![0.2, 0.3, 0.5];

    let matched = find_symbols_by_name("b", &ingest, &interner);
    assert_eq!(matched, vec![1]);

    let nav = navigate_1hop(1, &ingest, &graph, &ranks, &interner).unwrap();
    assert_eq!(nav.target.name, "b");
    assert_eq!(nav.callers.len(), 1);
    assert_eq!(nav.callers[0].name, "a");
    assert_eq!(nav.callers[0].sites, vec!["src/lib.rs:2".to_string()]);
    assert_eq!(nav.callees.len(), 1);
    assert_eq!(nav.callees[0].name, "c");

    let mut callers_xml = Vec::new();
    serialize_callers_xml("b", &nav.callers, &mut callers_xml).unwrap();
    assert!(
        String::from_utf8(callers_xml)
            .unwrap()
            .contains("sites_at=\"src/lib.rs:2\"")
    );

    let impact = transitive_impact(2, &ingest, &graph, &ranks, &interner, None);
    assert_eq!(impact.len(), 2);
    assert_eq!(impact[0].name, "b");
    assert_eq!(impact[0].hops, 1);
    assert_eq!(impact[1].name, "a");
    assert_eq!(impact[1].hops, 2);

    let mut imp_xml = Vec::new();
    serialize_impact_with_importers_xml(
        ".",
        "c",
        &impact,
        vec!["src/main.rs"],
        GraphGauges::ZERO_XML_ATTRS,
        1,
        &mut imp_xml,
    )
    .unwrap();
    let imp_str = String::from_utf8(imp_xml).unwrap();
    assert!(imp_str.contains("importers=\"2\""));
    assert!(imp_str.contains("p=\"src/main.rs\""));
}

#[test]
fn test_graph_query_execution() {
    let interner = Interner::new();
    let f0 = FileInfo {
        id: 0,
        path: interner.intern("src/test.rs"),
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 500,
    };
    let sym_a = Symbol::new(
        0,
        interner.intern("foo"),
        interner.intern("c_foo"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );
    let sym_b = Symbol::new(
        1,
        interner.intern("bar"),
        interner.intern("c_bar"),
        SymKind::Function,
        Lang::Rust,
        0,
        5,
    );
    let ref_ab = Reference::new(0, interner.intern("bar"), 0, 2, Lang::Rust, RefRole::Call);

    let mut ingest = IngestResult::new();
    ingest.files.push(f0);
    ingest.symbols.extend_from_slice(&[sym_a, sym_b]);
    ingest.references.extend_from_slice(&[ref_ab]);

    let graph = Graph::build(&ingest, &interner);
    let mut out = Vec::new();
    execute_graph_query_expr(
        "callers(name(\"bar\"))",
        ".",
        &ingest,
        &graph,
        &interner,
        &mut out,
    )
    .unwrap();
    let out_str = String::from_utf8(out).unwrap();
    assert!(out_str.contains("<query schema=\"ripwire.graph-query/v1\""));
    assert!(out_str.contains("count=\"1\""));
    assert!(out_str.contains("n=\"foo\""));

    let mut out = Vec::new();
    execute_graph_query_expr(
        "callees(name(\"foo\"))",
        ".",
        &ingest,
        &graph,
        &interner,
        &mut out,
    )
    .unwrap();
    assert!(String::from_utf8(out).unwrap().contains("n=\"bar\""));

    let mut out = Vec::new();
    execute_graph_query_expr(
        "and(kind(all, fn), file(all, test), fanin(all, 1), cx(all, 10))",
        ".",
        &ingest,
        &graph,
        &interner,
        &mut out,
    )
    .unwrap();
    assert!(String::from_utf8(out).unwrap().contains("n=\"bar\""));

    let mut out = Vec::new();
    execute_graph_query_expr(
        "or(not(name(\"foo\")), callers(\"bar\", 1))",
        ".",
        &ingest,
        &graph,
        &interner,
        &mut out,
    )
    .unwrap();
    assert!(String::from_utf8(out).unwrap().contains("count=\"2\""));
    let mut paged_out = Vec::new();
    execute_graph_query_expr_paged(
        "or(not(name(\"foo\")), callers(\"bar\", 1))",
        ".",
        &ingest,
        &graph,
        &interner,
        GraphQueryPageOptions {
            limit: Some(1),
            offset: Some(0),
        },
        &mut paged_out,
    )
    .unwrap();
    let paged_str = String::from_utf8(paged_out).unwrap();
    assert!(paged_str.contains("shown=\"1\" capped=\"1\" total=\"2\""));

    let long_body = "let x = 1;\n".repeat(400);
    let truncated = truncate_body_to_budget(&long_body, 512);
    assert!(truncated.len() < 600);
    assert!(truncated.contains("truncated to fit pack-task budget"));

    let mut v_out = Vec::new();
    evaluate_verify_claim("calls(foo,bar)", &ingest, &graph, &interner, &mut v_out).unwrap();
    assert!(String::from_utf8(v_out).unwrap().contains("confirmed"));
    let mut v_ref = Vec::new();
    evaluate_verify_claim("calls(bar,foo)", &ingest, &graph, &interner, &mut v_ref).unwrap();
    assert!(String::from_utf8(v_ref).unwrap().contains("refuted"));
    let mut p_out = Vec::new();
    execute_pack_task("search", ".", &ingest, &interner, &mut p_out).unwrap();
    assert!(String::from_utf8(p_out).unwrap().contains("<sigs>"));

    for tag in [
        "function",
        "method",
        "class",
        "struct",
        "interface",
        "section",
        "unknown",
    ] {
        let mut k_out = Vec::new();
        execute_graph_query_expr(
            &format!("kind(all, {tag})"),
            ".",
            &ingest,
            &graph,
            &interner,
            &mut k_out,
        )
        .unwrap();
        assert!(!k_out.is_empty());
    }
    assert_eq!("src/infra/nodekind.h".builtin_layer(), Some("infra"));
    assert_eq!("src/ui/render/renderer.rs".builtin_layer(), Some("render"));
    assert_eq!("packages/ai/src/usage.ts".builtin_layer(), Some("ai"));
    assert_eq!("src/lib.rs".builtin_layer(), None);
    let mut l_out = Vec::new();
    execute_graph_query_expr(
        "layer(all, \"test\")",
        ".",
        &ingest,
        &graph,
        &interner,
        &mut l_out,
    )
    .unwrap();
    assert!(String::from_utf8(l_out).unwrap().contains("count=\"0\""));
}
