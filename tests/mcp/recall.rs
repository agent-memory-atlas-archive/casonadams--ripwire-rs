use ripwire_rs::mcp::recall::{
    DocFormat, RecallInput, RecallOptions, csv_to_text, doc_format, html_to_text, is_heading,
    query_terms, recall, split_sections,
};
use ripwire_rs::serialize_recall_text;

fn md(path: &str, text: &str) -> RecallInput {
    RecallInput {
        path: path.to_string(),
        format: DocFormat::Markdown,
        text: text.to_string(),
    }
}

fn render(inputs: &[RecallInput], query: &str, opts: RecallOptions) -> String {
    let val = recall(inputs, query, opts).to_json(query);
    let mut buf = Vec::new();
    serialize_recall_text(query, &val, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

fn numbered_lines(n: usize) -> String {
    (1..=n)
        .map(|i| format!("line{i:02} zebra alpha beta gamma delta\n"))
        .collect()
}

#[test]
fn test_split_sections_ignores_fenced_headings_and_preamble() {
    let text = "pre\n# A\nx\n```\n# not a heading\n```\n~~~\n## nor this\n~~~\n## B\ny\n";
    let spans: Vec<(usize, usize)> = split_sections(text)
        .iter()
        .map(|s| (s.start_line, s.end_line))
        .collect();
    assert_eq!(spans, vec![(2, 9), (10, 11)]);
    let sections = split_sections(text);
    assert_eq!(
        &text[sections[1].start_byte..sections[1].end_byte],
        "## B\ny\n"
    );
    assert!(split_sections("no headings\n```\n# fenced\n```\n").is_empty());
}

#[test]
fn test_heading_syntax() {
    assert!(is_heading("# Title\n"));
    assert!(is_heading("  ## indented\n"));
    assert!(is_heading("####### seven\n"));
    assert!(is_heading("#\n"));
    assert!(!is_heading("#NoSpace\n"));
    assert!(!is_heading("    # code block\n"));
    assert!(!is_heading("text # not\n"));
}

#[test]
fn test_doc_format_filter() {
    for p in [
        "a.md",
        "A.MD",
        "b.markdown",
        "c.mdx",
        "d.rst",
        "e.adoc",
        "f.org",
        "g.html",
        "h.htm",
        "i.csv",
        "j.ipynb",
    ] {
        assert!(doc_format(p).is_some(), "{p} should be a document");
    }
    for p in [
        "notes.txt",
        "docs/map.json",
        "docs/schema.sql",
        "run.log",
        "spec.pdf",
        "doc_tool.py",
        "README",
    ] {
        assert!(doc_format(p).is_none(), "{p} is not a document");
    }
    assert_eq!(doc_format("docs/x.md"), Some(DocFormat::Markdown));
    assert_eq!(doc_format("docs/x.rst"), Some(DocFormat::Plain));
}

#[test]
fn test_query_terms_split_like_documents() {
    assert_eq!(
        query_terms("snake_word camelCase Snake word2x"),
        vec!["snake", "word", "camel", "case", "word2x"]
    );
}

#[test]
fn test_html_and_csv_rendering() {
    let html = "<html><head><title>T zebra</title><style>.x{}</style><script>var zebra=1;</script></head>\
<body><!-- hidden --><h1>Head</h1><p>one &amp; two&#33;</p><pre>code\n  more</pre></body></html>\n";
    assert_eq!(html_to_text(html), "T zebra Head one & two! code\nmore");
    let csv = "name,\"note, quoted\",count\nx,1,2\ny,,4\nz,5,6\nw,7,8\n";
    assert_eq!(
        csv_to_text(csv),
        "CSV columns: name note, quoted count\nrows: 4\nx 1 2\ny  4\nz 5 6\n"
    );
}

#[test]
fn test_bm25_prefers_rare_terms() {
    let inputs = vec![
        md("a.md", "quokka filler\n"),
        md("b.md", "zebra filler\n"),
        md("c.md", "zebra other\n"),
        md("d.md", "zebra more\n"),
        md("e.md", "unrelated text\n"),
    ];
    let out = recall(&inputs, "zebra quokka", RecallOptions::default());
    assert_eq!(out.documents, 5);
    assert_eq!(out.relevant, 4);
    assert_eq!(out.docs[0].path, "a.md");
    assert!(out.docs[0].relevance > out.docs[1].relevance);
    let order: Vec<&str> = out.docs.iter().map(|d| d.path.as_str()).collect();
    assert_eq!(order, vec!["a.md", "b.md", "c.md", "d.md"]);
}

#[test]
fn test_path_tokens_and_headings_weigh_in() {
    let inputs = vec![
        md("zebra.md", "nothing here at all\n"),
        md("h.md", "# quokka title\nalpha beta gamma\n"),
        md("b.md", "# alpha title\nquokka beta gamma\n"),
        md("f.md", "filler words only\n"),
    ];
    let out = recall(&inputs, "zebra quokka", RecallOptions::default());
    let order: Vec<&str> = out.docs.iter().map(|d| d.path.as_str()).collect();
    assert_eq!(order, vec!["zebra.md", "h.md", "b.md"]);
}

#[test]
fn test_serves_matching_sections_only() {
    let text = "pre zebra\n# Top\nfiller\n## A\nzebra in body\n```\n# zebra fenced\n```\n## B\nnothing\n## Zebra heading\nplain\n";
    let out = recall(&[md("s.md", text)], "zebra", RecallOptions::default());
    let doc = &out.docs[0];
    let note = doc.sections.as_ref().unwrap();
    assert_eq!((note.selected, note.matched, note.total), (2, 2, 4));
    assert_eq!(note.spans, vec![(4, 8), (11, 12)]);
    assert_eq!(
        doc.body,
        "## A\nzebra in body\n```\n# zebra fenced\n```\n## Zebra heading\nplain\n"
    );
    assert!(doc.truncated.is_none());
    assert_eq!(out.share_bytes, None);
}

#[test]
fn test_preamble_only_match_serves_whole_doc() {
    let text = "pre zebra\n# Top\nfiller\n";
    let out = recall(&[md("a.md", text)], "zebra", RecallOptions::default());
    assert_eq!(out.relevant, 1);
    assert!(out.docs[0].sections.is_none());
    assert_eq!(out.docs[0].body, text);
}

#[test]
fn test_budget_truncates_headless_doc_front_first() {
    let body = numbered_lines(40);
    let inputs = vec![md("one.md", &body)];
    let opts = RecallOptions {
        top_k: 8,
        max_tokens: 300,
    };
    let out = recall(&inputs, "zebra", opts);
    let doc = &out.docs[0];
    let (shown, of) = doc.truncated.unwrap();
    assert_eq!(of, body.len());
    assert!(shown <= out.budget_bytes && shown % 36 == 0);
    assert!(doc.body.ends_with("delta\n…"));
    assert_eq!(out.share_bytes, Some(out.budget_bytes));
    assert_eq!(out.truncated(), 1);
    let text = render(&inputs, "zebra", opts);
    assert!(text.contains(&format!("[truncated: {shown} of 1440 bytes]")));
    assert!(text.contains(" capped=1 truncated=1 over_ceiling=1 max_tokens=300 share_bytes=637 "));
}

#[test]
fn test_budget_drops_lower_ranked_sections() {
    let filler = "filler words to pad the section out\n".repeat(10);
    let text =
        format!("# One zebra\n{filler}## Two zebra zebra\n{filler}## Three\nzebra\n{filler}");
    let inputs = vec![md("s.md", &text)];
    let opts = RecallOptions {
        top_k: 8,
        max_tokens: 300,
    };
    let out = recall(&inputs, "zebra", opts);
    let note = out.docs[0].sections.as_ref().unwrap();
    assert_eq!((note.selected, note.matched, note.total), (1, 3, 3));
    assert_eq!(note.spans, vec![(12, 22)]);
    let rendered = render(&inputs, "zebra", opts);
    assert!(rendered.contains(
        "[sections: 1 of 3 selected (3 in doc), section-granular; whole doc 1126 B; lines=\"12-22\"; dropped_by_budget=2]"
    ));
}

#[test]
fn test_budget_omits_docs_that_cannot_get_a_slice() {
    let big = numbered_lines(60);
    let inputs = vec![md("a.md", &big), md("b.md", &big)];
    let opts = RecallOptions {
        top_k: 8,
        max_tokens: 150,
    };
    let out = recall(&inputs, "zebra", opts);
    assert_eq!(out.docs.len(), 1);
    assert_eq!(out.omitted_budget, 1);
    let text = render(&inputs, "zebra", opts);
    assert!(text.contains(
        "(capped: 1 of 2 relevant document files omitted — raise --max-tokens/budget_tokens or narrow the query for 1 more (~318-byte budget))"
    ));
}

#[test]
fn test_top_k_cap_and_header_shape() {
    let inputs = vec![
        md("a.md", "zebra\n"),
        md("b.md", "zebra two\n"),
        md("c.md", "zebra three words\n"),
    ];
    let opts = RecallOptions {
        top_k: 1,
        max_tokens: 8000,
    };
    let text = render(&inputs, "zebra", opts);
    assert!(text.starts_with(
        "ripwire-rs recall — \"zebra\" — 3 relevant of 3 document files, best-first — total=3 shown=1 capped=1 max_tokens=8000 est_tokens="
    ));
    assert!(text.contains("\n\n━━ a.md  (relevance "));
    assert!(text.contains(") ━━\nzebra\n\n\n(capped: 2 of 3 relevant document files omitted — raise --top-k/top_k (default 8) for 2 more)\n"));
    let est: usize = text
        .split("est_tokens=")
        .nth(1)
        .and_then(|s| s.split('\n').next())
        .and_then(|s| s.parse().ok())
        .unwrap();
    assert_eq!(est, (text.len() * 2).div_ceil(5));
}

#[test]
fn test_generated_docs_rank_last() {
    let inputs = vec![
        md(
            "g.md",
            "# Gen\nGenerated by tool. Do not edit.\nzebra zebra zebra\n",
        ),
        md(
            "n.md",
            "# Plain\nonce zebra here among many other filler words\n",
        ),
    ];
    let out = recall(&inputs, "zebra", RecallOptions::default());
    assert_eq!(out.generated_demoted, 1);
    assert_eq!(out.docs[0].path, "n.md");
    assert!(out.docs[1].generated);
    let text = render(&inputs, "zebra", RecallOptions::default());
    assert!(text.contains(" generated_demoted=1 "));
    assert!(text.contains("━━ g.md  (relevance "));
    assert!(
        text.contains(" ━━  [generated_demoted: marker]  [sections: 1 of 1, section-granular;")
    );
}

#[test]
fn test_no_relevant_documents() {
    let inputs = vec![md("a.md", "alpha\n")];
    let text = render(&inputs, "xyzzy", RecallOptions::default());
    assert!(text.starts_with(
        "ripwire-rs recall — \"xyzzy\" — 0 relevant of 1 document files, best-first — total=0 shown=0 capped=0 max_tokens=8000 est_tokens="
    ));
    assert!(text.ends_with("\n\n(no relevant documents — try different terms)\n"));
}
