use ripwire::{
    FileInfo, Graph, IngestResult, Interner, Lang, PageRankResult, RefRole, Reference,
    SerializeOptions, SymKind, Symbol, escape_xml_str, serialize_xml_to_string,
};

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
