use ripwire_rs::cli::VerbGroup;
use ripwire_rs::commands::retrieval::{RetrievalArgs, execute_retrieval_dispatch};
use ripwire_rs::query::commands::QueryContext;
use ripwire_rs::{
    Graph, IngestResult, Interner, Lang, LanguageRegistry, PageRankConfig, PageRankResult,
};

const FLOW_RS: &[u8] =
    b"pub fn entry() { bridge(); }\nfn bridge() { leaf(); }\nfn leaf() {}\nfn isolated() {}\n";
const OTHER_RS: &[u8] = b"fn other() {}\n";

static FIXTURE_SOURCES: &[(&str, &[u8])] = &[("src/flow.rs", FLOW_RS), ("other/flow.rs", OTHER_RS)];

fn fixture() -> (IngestResult, Graph, Interner) {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ripwire_rs::ingest_in_memory_files(
        &[
            ("src/flow.rs", Lang::Rust, FLOW_RS),
            ("other/flow.rs", Lang::Rust, OTHER_RS),
        ],
        &registry,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    (ingest, graph, interner)
}

fn context<'a>(
    ingest: &'a IngestResult,
    graph: &'a Graph,
    pr: &'a PageRankResult,
    interner: &'a Interner,
) -> QueryContext<'a> {
    QueryContext {
        root: ":memory:",
        ingest,
        graph,
        pr,
        interner,
        commits: None,
        at: None,
        sources: &FIXTURE_SOURCES,
    }
}

fn query(args: RetrievalArgs) -> std::io::Result<String> {
    let (ingest, graph, interner) = fixture();
    let ranks = graph.page_rank(&PageRankConfig::default());
    let mut output = Vec::new();
    execute_retrieval_dispatch(
        &args,
        &context(&ingest, &graph, &ranks, &interner),
        None,
        &mut output,
    )?;
    Ok(String::from_utf8(output).unwrap())
}

#[test]
fn directed_paths_distinguish_transitive_reverse_and_disconnected_calls() {
    for (pair, reachable, hops) in [
        ("entry -> leaf", "1", "2"),
        ("leaf,entry", "0", "0"),
        ("entry:isolated", "0", "0"),
    ] {
        let xml = query(RetrievalArgs {
            path_between: Some(pair.into()),
            ..Default::default()
        })
        .unwrap();
        assert!(xml.contains(&format!("reachable=\"{reachable}\"")), "{xml}");
        assert!(xml.contains(&format!("hops=\"{hops}\"")), "{xml}");
        if reachable == "1" {
            assert!(xml.contains("bridge"), "{xml}");
            assert!(!xml.contains("isolated"), "{xml}");
        }
    }
}

#[test]
fn connect_includes_intermediate_callers_but_not_disconnected_definitions() {
    let xml = query(RetrievalArgs {
        connect: Some(" entry , leaf ".into()),
        connect_radius: Some(2),
        path_between: Some("absent,leaf".into()),
        ..Default::default()
    })
    .unwrap();
    assert!(xml.contains("<t n=\"entry\""), "{xml}");
    assert!(xml.contains("<t n=\"leaf\""), "{xml}");
    assert!(xml.contains("<s n=\"bridge\""), "{xml}");
    assert!(xml.contains("<e f=\"entry\" t=\"bridge\""), "{xml}");
    assert!(xml.contains("<e f=\"bridge\" t=\"leaf\""), "{xml}");
    assert!(!xml.contains("isolated"), "{xml}");
}

#[test]
fn missing_graph_endpoints_fail_before_emitting_partial_results() {
    let (ingest, graph, interner) = fixture();
    let ranks = graph.page_rank(&PageRankConfig::default());
    for args in [
        RetrievalArgs {
            connect: Some("entry,absent".into()),
            ..Default::default()
        },
        RetrievalArgs {
            path_between: Some("absent,leaf".into()),
            ..Default::default()
        },
        RetrievalArgs {
            path_between: Some("entry,absent".into()),
            ..Default::default()
        },
        RetrievalArgs {
            lego: Some("absent".into()),
            ..Default::default()
        },
    ] {
        let mut output = Vec::new();
        let error = execute_retrieval_dispatch(
            &args,
            &context(&ingest, &graph, &ranks, &interner),
            None,
            &mut output,
        )
        .unwrap_err();
        assert!(error.to_string().contains("absent"), "{error}");
        assert!(output.is_empty());
    }
}

#[test]
fn location_resolution_rejects_ambiguous_suffixes_and_out_of_definition_lines() {
    let xml = query(RetrievalArgs {
        at: Some("src/flow.rs:2".into()),
        verify: Some("calls(leaf,entry)".into()),
        ..Default::default()
    })
    .unwrap();
    assert!(xml.contains("sym=\"bridge\""), "{xml}");
    assert!(xml.contains("l=\"2\""), "{xml}");
    assert!(!xml.contains("verdict="), "{xml}");
    for spec in ["flow.rs:1", "src/flow.rs:99", "missing.rs:1"] {
        assert!(
            query(RetrievalArgs {
                at: Some(spec.into()),
                ..Default::default()
            })
            .is_err(),
            "{spec}"
        );
    }
}

#[test]
fn call_claims_report_transitive_witnesses_without_claiming_absence_proves_non_calls() {
    for (claim, verdict) in [
        ("calls(entry,bridge)", "confirmed"),
        ("calls(entry,leaf)", "confirmed"),
        ("calls(leaf,entry)", "not-established"),
        ("calls(absent,leaf)", "not-established"),
    ] {
        let xml = query(RetrievalArgs {
            verify: Some(claim.into()),
            ..Default::default()
        })
        .unwrap();
        assert!(xml.contains(&format!("verdict=\"{verdict}\"")), "{xml}");
    }
}

#[test]
fn task_section_errors_are_atomic_even_with_signatures_only() {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ripwire_rs::ingest_in_memory_files(&[], &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let ranks = graph.page_rank(&PageRankConfig::default());
    for sections in ["lego,lego", "compose,", "unknown"] {
        let args = RetrievalArgs {
            task_for: Some("locate parser".into()),
            signatures_only: true,
            auto_bodies: true,
            sections: Some(sections.into()),
            ..Default::default()
        };
        let mut output = Vec::new();
        let error = execute_retrieval_dispatch(
            &args,
            &context(&ingest, &graph, &ranks, &interner),
            None,
            &mut output,
        )
        .unwrap_err();
        assert!(error.to_string().contains("--sections="), "{error}");
        assert!(output.is_empty());
    }
}

#[test]
fn signature_pack_preserves_indexed_definitions_and_resolved_callees() {
    let xml = query(RetrievalArgs {
        pack_signatures: true,
        path_between: Some("absent,leaf".into()),
        ..Default::default()
    })
    .unwrap();
    for name in ["entry", "bridge", "leaf", "isolated", "other"] {
        assert!(xml.contains(&format!("<s t=\"fn\" n=\"{name}\"")), "{xml}");
    }
    assert!(xml.contains("<c n=\"bridge\""), "{xml}");
    assert!(xml.contains("<c n=\"leaf\""), "{xml}");
    assert!(xml.contains("p=\"src/flow.rs\""), "{xml}");
    assert!(xml.contains("p=\"other/flow.rs\""), "{xml}");
}

#[test]
fn quoted_search_literals_preserve_payload_and_do_not_capture_possessives() {
    use ripwire_rs::query::task_route::extract_quoted_task_literal;
    for (task, expected) in [
        ("find parser's failure", None),
        ("find parser's 'bad_token'", Some("bad_token")),
        ("find 'two words'", None),
        ("find \"two words & <tag>\"", Some("two words & <tag>")),
        ("find 'first' then \"second\"", Some("second")),
        ("find \"unfinished", None),
    ] {
        assert_eq!(extract_quoted_task_literal(task), expected, "{task}");
    }
}

#[test]
fn advanced_retrieval_and_pattern_search_execute_over_context_sources() {
    let pat_xml = query(RetrievalArgs {
        pattern: Some("bridge".into()),
        ..Default::default()
    })
    .unwrap();
    assert!(pat_xml.contains("bridge"), "{pat_xml}");
    assert!(pat_xml.contains("src/flow.rs"), "{pat_xml}");

    let ast_xml = query(RetrievalArgs {
        ast_match: Some("(function_item name: (identifier) @name)".into()),
        ..Default::default()
    })
    .unwrap();
    let report = roxmltree::Document::parse(&ast_xml).unwrap();
    assert_eq!(report.root_element().attribute("hits"), Some("5"));
    assert_eq!(report.root_element().attribute("files"), Some("2"));

    let refetch_xml = query(RetrievalArgs {
        refetch: Some("entry".into()),
        ..Default::default()
    })
    .unwrap();
    assert!(refetch_xml.contains("bridge();"), "{refetch_xml}");

    let trace_xml = query(RetrievalArgs {
        from_trace: Some("src/flow.rs:2".into()),
        ..Default::default()
    })
    .unwrap();
    let report = roxmltree::Document::parse(&trace_xml).unwrap();
    let suspect = report
        .descendants()
        .find(|node| node.has_tag_name("suspect"))
        .unwrap();
    assert_eq!(suspect.attribute("n"), Some("bridge"));
    assert_eq!(suspect.attribute("p"), Some("src/flow.rs"));
    assert_eq!(suspect.attribute("l"), Some("2"));

    let pack_xml = query(RetrievalArgs {
        pack_task: Some("inspect bridge".into()),
        ..Default::default()
    })
    .unwrap();
    assert!(pack_xml.contains("bridge"), "{pack_xml}");
    assert!(pack_xml.contains("<sigs>"), "{pack_xml}");

    let gq_xml = query(RetrievalArgs {
        graph_query: Some("callees(entry)".into()),
        ..Default::default()
    })
    .unwrap();
    let report = roxmltree::Document::parse(&gq_xml).unwrap();
    let callees: Vec<_> = report
        .descendants()
        .filter(|node| node.has_tag_name("s"))
        .map(|node| (node.attribute("n"), node.attribute("p")))
        .collect();
    assert_eq!(callees, [(Some("bridge"), Some("src/flow.rs:2"))]);

    let ex_xml = query(RetrievalArgs {
        exemplar: Some("entry".into()),
        ..Default::default()
    })
    .unwrap();
    let report = roxmltree::Document::parse(&ex_xml).unwrap();
    assert_eq!(report.root_element().attribute("n"), Some("entry"));
    let body = report
        .descendants()
        .find(|node| node.has_tag_name("b"))
        .unwrap();
    assert!(body.text().unwrap().contains("bridge();"));
    let callees: Vec<_> = body
        .descendants()
        .filter(|node| node.has_tag_name("c"))
        .map(|node| (node.attribute("n"), node.attribute("l")))
        .collect();
    assert_eq!(callees, [(Some("bridge"), Some("2"))]);
}

#[test]
fn retrieval_group_is_active_only_for_its_primary_flags() {
    let active = |flag: &str| {
        let cli = ripwire_rs::parse_from(["rw", flag]).unwrap();
        cli.plugins
            .decode::<RetrievalArgs>()
            .unwrap()
            .is_group_active()
    };
    for flag in [
        "--for=a",
        "--grep=a",
        "--connect=a,b",
        "--path-between=a,b",
        "--exemplar=a",
        "--lego=a",
        "--recall=a",
        "--from-trace=a",
        "--at=a",
        "--verify=a",
        "--pack-task=a",
        "--graph-query=a",
        "--match=a",
        "--pattern=a",
        "--regex=a",
        "--help-task=a",
        "--refetch=a",
        "--pack-signatures",
    ] {
        assert!(active(flag), "{flag}");
    }
    for flag in ["--adaptive", "--no-route", "--refetch="] {
        assert!(!active(flag), "{flag}");
    }
}
