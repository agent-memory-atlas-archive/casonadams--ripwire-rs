use ripwire_rs::git::history::CommitInfo;
use ripwire_rs::metrics::{
    PanelHistory, PanelInputs, QualityPanelReport, compute_quality_panel_report,
};
use ripwire_rs::serialize::PageRequest;
use ripwire_rs::{
    FileInfo, Graph, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol, ToXmlString,
};

const NOW: i64 = 1_700_000_000;
const WINDOW_START: i64 = NOW - 365 * 86_400;
const PATH: &str = "src/lib.rs";
const NO_HISTORY: &str = "in-memory ingest: no git history";

fn function(interner: &Interner, id: u32, name: &str) -> Symbol {
    Symbol::new(
        id,
        interner.intern(name),
        interner.intern(&format!("{PATH}::{name}")),
        SymKind::Function,
        Lang::Rust,
        0,
        id + 1,
    )
}

fn ingest_for(interner: &Interner, symbols: Vec<Symbol>) -> IngestResult {
    let mut ingest = IngestResult::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern(PATH),
        lang: Lang::Rust,
        line_count: 150,
        byte_size: 3000,
    });
    ingest.symbols = symbols;
    ingest
}

/// `count` commits touching `PATH`, newest first, one day apart, all inside the window.
fn commits_in_window(count: usize) -> Vec<CommitInfo> {
    (0..count)
        .map(|i| CommitInfo {
            id: format!("c{i}"),
            timestamp: NOW - i as i64 * 86_400,
            author: "Ada".to_string(),
            changed_files: vec![PATH.to_string()],
        })
        .collect()
}

fn history(
    ingest: &IngestResult,
    interner: &Interner,
    commits: &[CommitInfo],
) -> Result<PanelHistory, String> {
    Ok(PanelHistory::from_commits(
        ingest,
        interner,
        commits,
        "abc123def".to_string(),
        WINDOW_START,
    ))
}

fn panel(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    preset: Option<&str>,
    history: Result<PanelHistory, String>,
) -> QualityPanelReport {
    compute_quality_panel_report(
        ingest,
        graph,
        interner,
        PanelInputs { preset, history },
        PageRequest::default(),
    )
}

#[test]
fn test_quality_panel_computation() {
    let ingest = IngestResult::new();
    let interner = Interner::new();
    let graph = Graph::build(&ingest, &interner);
    let rep = panel(
        &ingest,
        &graph,
        &interner,
        None,
        Err(NO_HISTORY.to_string()),
    );
    assert_eq!(rep.preset, "default");
    assert_eq!(rep.cut, 2);
    assert!(
        rep.to_xml_string()
            .contains("schema=\"ripwire-rs.quality-panel/v1\"")
    );
}

#[test]
fn test_check_symbol_families() {
    let interner = Interner::new();
    let mut sym = function(&interner, 0, "complex_fn");
    sym.ccx = 20;
    sym.loc = 100;
    sym.max_nest = 5;
    sym.params = 6;
    sym.cx = 15;
    let state_sym = Symbol::new(
        1,
        interner.intern("GLOBAL_STATE"),
        interner.intern("src/lib.rs::GLOBAL_STATE"),
        SymKind::Var,
        Lang::Rust,
        0,
        120,
    );
    let mut ingest = ingest_for(&interner, vec![sym, state_sym]);
    ingest.references.push(Reference::new(
        0,
        interner.intern("GLOBAL_STATE"),
        0,
        10,
        Lang::Rust,
        RefRole::Write,
    ));
    let graph = Graph::build(&ingest, &interner);
    let commits = commits_in_window(14);
    let rep = panel(
        &ingest,
        &graph,
        &interner,
        None,
        history(&ingest, &interner, &commits),
    );
    assert_eq!(rep.rows.len(), 1);
    assert!(rep.rows[0].fam >= 3);
    assert!(rep.rows[0].fired.contains("structural"));
    assert!(rep.rows[0].fired.contains("state"));
    assert!(rep.rows[0].fired.contains("historical"));
}

#[test]
fn test_historical_follows_churn_not_structure() {
    let interner = Interner::new();
    let simple = function(&interner, 0, "handle_request");
    let mut tangled = function(&interner, 1, "tangled_dispatch");
    tangled.ccx = 30;
    tangled.loc = 100;
    tangled.max_nest = 5;
    let ingest = ingest_for(&interner, vec![simple, tangled]);
    let graph = Graph::build(&ingest, &interner);
    let run = |commits: &[CommitInfo]| {
        panel(
            &ingest,
            &graph,
            &interner,
            Some("lenient"),
            history(&ingest, &interner, commits),
        )
    };

    let busy = run(&commits_in_window(14));
    assert_eq!(busy.hranked, 2);

    let quiet = run(&commits_in_window(3));
    assert_eq!(quiet.hranked, 0);
    assert_eq!(quiet.rows.len(), 1);
    assert_eq!(quiet.rows[0].name, "tangled_dispatch");
    assert_eq!(quiet.rows[0].fired, "structural");

    let stale: Vec<CommitInfo> = (0..14)
        .map(|i| CommitInfo {
            id: format!("old{i}"),
            timestamp: WINDOW_START - 1 - i * 86_400,
            author: "Ada".to_string(),
            changed_files: vec![PATH.to_string()],
        })
        .collect();
    assert_eq!(
        run(&stale).hranked,
        0,
        "commits before the window must not count"
    );
}

#[test]
fn test_lexical_family_detects_weak_names_that_parameter_count_cannot() {
    let interner = Interner::new();
    let mut terse = function(&interner, 0, "f");
    terse.params = 0;
    terse.ccx = 1;
    terse.loc = 5;
    let mut many_params = function(&interner, 1, "fetch_order");
    many_params.params = 9;
    many_params.ccx = 1;
    many_params.loc = 5;
    let ingest = ingest_for(&interner, vec![terse, many_params]);
    let graph = Graph::build(&ingest, &interner);
    let rep = panel(
        &ingest,
        &graph,
        &interner,
        Some("lenient"),
        Err(NO_HISTORY.to_string()),
    );
    let terse_row = rep.rows.iter().find(|r| r.name == "f").expect("terse row");
    assert_eq!(terse_row.fired, "lexical");
    let lexical = terse_row
        .evidence
        .iter()
        .find(|e| e.family == "lexical")
        .expect("lexical evidence");
    assert!(terse_row.fired.contains("lexical"));
    assert!(lexical.attrs.contains("name_len=\"1\""));

    let long_row = rep
        .rows
        .iter()
        .find(|r| r.name == "fetch_order")
        .expect("fetch_order row");
    assert!(!long_row.fired.contains("lexical"));
    assert!(long_row.fired.contains("structural"));
}

#[test]
fn test_unavailable_history_is_disclosed_not_substituted() {
    let interner = Interner::new();
    let mut tangled = function(&interner, 0, "tangled_dispatch");
    tangled.ccx = 30;
    tangled.loc = 100;
    tangled.max_nest = 5;
    let ingest = ingest_for(&interner, vec![tangled]);
    let graph = Graph::build(&ingest, &interner);
    let rep = panel(
        &ingest,
        &graph,
        &interner,
        Some("lenient"),
        Err("not a git repository".to_string()),
    );
    assert!(!rep.enabled.contains(&"historical"));
    assert!(
        rep.unavailable
            .iter()
            .any(|(family, why)| *family == "historical" && why.contains("not a git repository"))
    );
    let row = &rep.rows[0];
    assert!(row.evidence.iter().all(|e| e.family != "historical"));
    let xml = rep.to_xml_string();
    assert!(xml.contains("unavailable=\"confusion,historical\""));
    assert!(!xml.contains(" at=\""));
    assert!(!xml.contains(" window=\""));
}

#[test]
fn test_denominators_and_evidence_match_enabled_families() {
    let interner = Interner::new();
    let mut tangled = function(&interner, 0, "tangled_dispatch");
    tangled.ccx = 30;
    tangled.loc = 100;
    tangled.max_nest = 5;
    tangled.params = 6;
    let ingest = ingest_for(&interner, vec![tangled]);
    let graph = Graph::build(&ingest, &interner);
    let commits = commits_in_window(14);
    let rep = panel(
        &ingest,
        &graph,
        &interner,
        Some("lenient"),
        history(&ingest, &interner, &commits),
    );
    // Rust has no C-layout confusion family: structural, lexical, historical, colocation, state.
    assert_eq!(rep.enabled.len(), 5);
    let row = &rep.rows[0];
    assert_eq!(row.of, rep.enabled.len());
    assert_eq!(row.evidence.len(), row.fam);

    let structural = row
        .evidence
        .iter()
        .find(|e| e.family == "structural")
        .expect("structural evidence");
    assert!(row.fired.contains("structural"));
    assert!(structural.attrs.contains("ccx=\"30\""));
    assert!(structural.attrs.contains("loc=\"100\""));
    assert!(structural.attrs.contains("nest=\"5\""));

    let historical = row
        .evidence
        .iter()
        .find(|e| e.family == "historical")
        .expect("historical evidence");
    assert!(row.fired.contains("historical"));
    assert!(historical.attrs.contains("commits=\"14\""));
    assert!(historical.attrs.contains("last=\"2023-11-14\""));

    let xml = rep.to_xml_string();
    assert!(xml.contains("families=\"6\""));
    assert!(xml.contains("enabled_n=\"5\""));
    assert!(xml.contains("at=\"abc123def\""));
}

#[test]
fn test_structural_evidence_carries_humps_and_deep_only_when_informative() {
    let interner = Interner::new();
    let mut deep = function(&interner, 0, "deep_dispatch");
    deep.ccx = 30;
    deep.loc = 100;
    deep.max_nest = 5;
    deep.humps = 3;
    deep.deep = 39;
    let mut flat = function(&interner, 0, "flat_dispatch");
    flat.ccx = 30;
    flat.loc = 100;
    flat.max_nest = 5;
    let ingest = ingest_for(&interner, vec![deep, flat]);
    let graph = Graph::build(&ingest, &interner);
    let rep = panel(
        &ingest,
        &graph,
        &interner,
        Some("lenient"),
        Err("not a git repository".to_string()),
    );
    let attrs_of = |name: &str| {
        let row = rep
            .rows
            .iter()
            .find(|r| r.name == name)
            .expect("ranked row");
        row.evidence
            .iter()
            .find(|e| e.family == "structural")
            .expect("structural evidence")
            .attrs
            .clone()
    };
    let informative = attrs_of("deep_dispatch");
    assert!(informative.contains("humps=\"3\""));
    assert!(informative.contains("deep=\"39\""));
    let quiet = attrs_of("flat_dispatch");
    assert!(!quiet.contains("humps="));
    assert!(!quiet.contains("deep="));
    assert!(quiet.contains("ccx=\"30\""));
}
