use ripwire::{
    FileInfo, Graph, IngestResult, Interner, Lang, PageRankResult, RefRole, Reference,
    SerializeOptions, SymKind, Symbol, escape_xml_str, serialize_connect_xml,
    serialize_doc_drift_xml, serialize_flags_xml, serialize_for_xml, serialize_grep_xml,
    serialize_mentions_xml, serialize_owners_xml, serialize_path_between_xml,
    serialize_stray_content_xml, serialize_whereis_xml, serialize_xml_to_string,
};
use serde_json::json;

fn validate_xml_well_formed(xml: &str) {
    let mut tag_stack: Vec<&str> = Vec::new();
    let mut i = 0;
    let bytes = xml.as_bytes();
    let mut saw_root = false;

    while i < bytes.len() {
        if bytes[i] == b'<' {
            let end = xml[i..].find('>').expect("unclosed tag") + i;
            let tag_content = &xml[i + 1..end];
            if let Some(closing_tag) = tag_content.strip_prefix('/') {
                let expected = tag_stack.pop().expect("unexpected closing tag");
                assert_eq!(closing_tag, expected, "closing tag mismatch");
            } else if tag_content.ends_with('/') {
                // Self-closing tag like <c n="..."/>
                assert!(tag_content.starts_with("c "));
            } else {
                let tag_name = tag_content
                    .split_whitespace()
                    .next()
                    .expect("empty tag name");
                if tag_name == "r" {
                    saw_root = true;
                }
                tag_stack.push(tag_name);
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    assert!(tag_stack.is_empty(), "unclosed tags: {:?}", tag_stack);
    assert!(saw_root, "Must emit <r> root tag");
}

#[test]
fn test_streaming_xml_serializer() {
    let interner = Interner::new();

    let f0 = FileInfo {
        id: 0,
        path: interner.intern("src/app & test.rs"),
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1200,
    };
    let f1 = FileInfo {
        id: 1,
        path: interner.intern("src/util.rs"),
        lang: Lang::Rust,
        line_count: 30,
        byte_size: 600,
    };

    let name_main = interner.intern("main");
    let name_helper = interner.intern("compute<T> & log");
    let name_util = interner.intern("init \"core\"");
    let canon = interner.intern("canon");

    let sym_main = Symbol::new(0, name_main, canon, SymKind::Function, Lang::Rust, 0, 10);
    let sym_helper = Symbol::new(1, name_helper, canon, SymKind::Function, Lang::Rust, 0, 20);
    let sym_util = Symbol::new(2, name_util, canon, SymKind::Function, Lang::Rust, 1, 5);

    // main calls helper; helper calls util
    let ref01 = Reference::new(0, name_helper, 0, 11, Lang::Rust, RefRole::Call);
    let ref12 = Reference::new(1, name_util, 0, 21, Lang::Rust, RefRole::Call);

    let mut ingest = IngestResult::new();
    ingest.files.extend_from_slice(&[f0, f1]);
    ingest
        .symbols
        .extend_from_slice(&[sym_main, sym_helper, sym_util]);
    ingest.references.extend_from_slice(&[ref01, ref12]);

    let graph = Graph::build(&ingest, &interner);
    let pr = PageRankResult {
        iterations: 15,
        converged: true,
        ranks: vec![0.20, 0.35, 0.45],
    };

    // ── 1. XML WELL-FORMEDNESS VALIDATION ────────────────────────────────────
    let options = SerializeOptions {
        root: ".".into(),
        top_k: None,
        include_pr: true,
        ..Default::default()
    };

    let xml = serialize_xml_to_string(&ingest, &graph, &pr, &interner, &options);
    validate_xml_well_formed(&xml);

    // ── 2. ESCAPE VALIDATION ─────────────────────────────────────────────────
    assert!(xml.contains("p=\"src/app &amp; test.rs\""));
    assert!(xml.contains("n=\"compute&lt;T&gt; &amp; log\""));
    assert!(xml.contains("n=\"init &quot;core&quot;\""));
    assert!(!xml.contains("&amp;amp;"));
    assert!(!xml.contains("&amp;lt;"));
    assert!(!xml.contains("&amp;gt;"));

    // ── 3. RANK ORDERING & FILE BUCKETING ────────────────────────────────────
    let pos_f1 = xml.find("p=\"src/util.rs\"").unwrap();
    let pos_f0 = xml.find("p=\"src/app &amp; test.rs\"").unwrap();
    assert!(
        pos_f1 < pos_f0,
        "File with highest rank symbol must appear first"
    );

    let pos_helper = xml.find("n=\"compute&lt;T&gt; &amp; log\"").unwrap();
    let pos_main = xml.find("n=\"main\"").unwrap();
    assert!(
        pos_helper < pos_main,
        "Higher-rank symbol must appear before lower-rank symbol in file"
    );

    // ── 4. CALL EDGES ────────────────────────────────────────────────────────
    assert!(
        xml.contains(
            "<s t=\"fn\" n=\"main\" k=\"0.2000\"><c n=\"compute&lt;T&gt; &amp; log\"/></s>"
        )
    );

    // ── 5. NON-CONVERGENCE ATTRIBUTE ─────────────────────────────────────────
    let pr_unconverged = PageRankResult {
        iterations: 100,
        converged: false,
        ranks: vec![0.20, 0.35, 0.45],
    };
    let xml_unconverged =
        serialize_xml_to_string(&ingest, &graph, &pr_unconverged, &interner, &options);
    assert!(xml_unconverged.contains("pr_converged=\"0\""));

    // ── 6. TOP-K FILTERING ───────────────────────────────────────────────────
    let top1_opts = SerializeOptions {
        root: ".".into(),
        top_k: Some(1),
        include_pr: true,
        ..Default::default()
    };
    let xml_top1 = serialize_xml_to_string(&ingest, &graph, &pr, &interner, &top1_opts);
    assert!(xml_top1.contains("p=\"src/util.rs\""));
    assert!(!xml_top1.contains("src/app &amp; test.rs"));

    // ── 7. ZERO-ALLOC ESCAPE TEST ────────────────────────────────────────────
    let clean = "clean_func_name";
    let cow = escape_xml_str(clean);
    assert!(matches!(cow, std::borrow::Cow::Borrowed(_)));
}

#[test]
fn test_query_xml_retrieval_and_git() {
    let mut buf = Vec::new();

    let val_for = json!({
        "task": "authenticate user",
        "symbols": [{"name": "auth", "path": "src/auth.rs", "line": 10, "score": 2.0, "kind": "fn"}]
    });
    serialize_for_xml(&val_for, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("schema=\"ripwire.for/v1\""));
    buf.clear();

    let val_grep = json!({
        "pattern": "login",
        "count": 1,
        "hits": [{"file": "src/main.rs", "line": 42, "enclosing": "main", "text": "login()"}]
    });
    serialize_grep_xml(&val_grep, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("schema=\"ripwire.grep/v1\""));
    buf.clear();

    let val_whereis = json!({
        "symbol": "auth",
        "hits": [{"path": "src/auth.rs", "line": 1, "kind": "def", "text": "fn auth()"}]
    });
    serialize_whereis_xml(&val_whereis, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("schema=\"ripwire.whereis/v1\""));
    buf.clear();

    let val_owners = json!({
        "files": [{"path": "src/auth.rs", "authors": 1, "top_author": "Alice", "top_share": 1.0}]
    });
    serialize_owners_xml(&val_owners, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("schema=\"ripwire.owners/v1\""));
    buf.clear();

    let val_flags = json!({
        "gates": [{"name": "FEATURE_X", "kind": "env", "file": "src/lib.rs", "line": 5}]
    });
    serialize_flags_xml(&val_flags, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("schema=\"ripwire.flags/v1\""));
    buf.clear();

    let val_doc = json!({
        "drift": [{"doc": "docs/arch.md", "line": 12, "referenced": "old_api"}]
    });
    serialize_doc_drift_xml(&val_doc, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("schema=\"ripwire.doc-drift/v1\""));
    buf.clear();

    let val_stray = json!({
        "branches": [{"branch": "feature", "unmerged_commits": 1}]
    });
    serialize_stray_content_xml(&val_stray, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("schema=\"ripwire.stray-content/v1\""));
    buf.clear();

    let val_mentions = json!({
        "symbol": "auth",
        "docs": [{"path": "README.md", "mentions": 3}]
    });
    serialize_mentions_xml(&val_mentions, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("schema=\"ripwire.mentions/v1\""));
}

#[test]
fn test_query_xml_graph_paths() {
    let mut buf = Vec::new();

    let val_connect = json!({
        "symbols": ["a", "b"],
        "edges": [{"from": "a", "to": "b"}]
    });
    serialize_connect_xml(&val_connect, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("schema=\"ripwire.connect/v1\""));
    buf.clear();

    let val_path_reach = json!({
        "from": "a",
        "to": "b",
        "reachable": true,
        "hops": 1,
        "path": ["a", "b"]
    });
    serialize_path_between_xml(&val_path_reach, &mut buf).unwrap();
    let s = String::from_utf8_lossy(&buf);
    assert!(s.contains(
        "<path schema=\"ripwire.path/v1\" from=\"a\" to=\"b\" reachable=\"1\" hops=\"1\">"
    ));
    buf.clear();

    let val_path_unreach = json!({
        "from": "x",
        "to": "y",
        "reachable": false,
        "hops": 0
    });
    serialize_path_between_xml(&val_path_unreach, &mut buf).unwrap();
    let s_unreach = String::from_utf8_lossy(&buf);
    assert!(s_unreach.contains(
        "<path schema=\"ripwire.path/v1\" from=\"x\" to=\"y\" reachable=\"0\" hops=\"0\""
    ));
    assert!(s_unreach.contains("hint="));
}
