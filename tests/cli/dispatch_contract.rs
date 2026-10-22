use clap::Parser;
use ripwire_rs::cli::{Cli, CliVerb, ExecutionContext, VerbGroup};
use ripwire_rs::commands::detail::DetailArgs;
use ripwire_rs::serialize::XmlWriter;
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, PageRankConfig, ingest_in_memory_files};
use std::collections::BTreeSet;

fn render(args: &[&str], detail: bool) -> std::io::Result<String> {
    let cli = ripwire_rs::parse_from(std::iter::once("rw").chain(args.iter().copied())).unwrap();
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[(
            "src/lib.rs",
            Lang::Rust,
            b"fn entry() { helper(); }\nfn helper() {}\nfn isolated() {}\n".as_slice(),
        )],
        &registry,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&PageRankConfig::default());
    let ctx = ExecutionContext {
        cli: &cli,
        root: ":memory:",
        ingest: &ingest,
        graph: &graph,
        pr: &pr,
        interner: &interner,
    };
    let mut output = Vec::new();
    {
        let mut writer = XmlWriter::new(&mut output, true);
        if detail {
            cli.plugins
                .decode::<DetailArgs>()
                .unwrap()
                .execute_verb(&ctx, &mut writer)?;
        } else {
            cli.arch.execute_verb(&ctx, &mut writer)?;
        }
        writer.finish()?;
    }
    Ok(String::from_utf8(output).unwrap())
}

fn names(xml: &str) -> BTreeSet<String> {
    roxmltree::Document::parse(xml)
        .unwrap()
        .descendants()
        .filter_map(|node| node.attribute("n"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn file_tree_retains_definitions_without_inventing_call_edges() {
    let output = render(&["--tree"], true).unwrap();
    assert_eq!(
        names(&output),
        ["entry", "helper", "isolated"].map(str::to_owned).into()
    );
    let document = roxmltree::Document::parse(&output).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("p") == Some("src/lib.rs"))
    );
}

#[test]
fn readability_inventory_does_not_lose_unreachable_functions() {
    let output = render(&["--biggest-first"], true).unwrap();
    assert_eq!(
        names(&output),
        ["entry", "helper", "isolated"].map(str::to_owned).into()
    );
}

#[test]
fn invalid_slice_requests_fail_before_any_repository_acquisition() {
    for (args, message) in [
        (
            vec!["rw", "--slice", "entry:value", "--slice-flow", "sideways"],
            "invalid slice flow",
        ),
        (vec!["rw", "slice"], "slice target is required"),
    ] {
        let cli = Cli::try_parse_from(args).unwrap();
        let error = ripwire_rs::run_cli_args(cli).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn community_dispatch_separates_an_isolated_definition_from_a_connected_pair() {
    let output = render(&["--communities"], false).unwrap();
    let document = roxmltree::Document::parse(&output).unwrap();
    let summary = document
        .descendants()
        .find(|node| node.has_tag_name("communities"))
        .unwrap();
    assert_eq!(summary.attribute("isolated_source"), Some("1"));
    let reported = names(&output);
    assert!(reported.contains("entry"));
    assert!(reported.contains("helper"));
}

#[test]
fn ego_graph_dispatch_excludes_disconnected_functions() {
    let output = render(&["--around", "entry"], false).unwrap();
    assert_eq!(
        names(&output),
        ["entry", "helper"].map(str::to_owned).into()
    );
}

#[test]
fn external_surface_does_not_reclassify_a_resolved_internal_call() {
    let output = render(&["--external-surface"], false).unwrap();
    let document = roxmltree::Document::parse(&output).unwrap();
    assert!(!document.descendants().any(|node| node.has_tag_name("x")));
}

#[test]
fn naming_votes_count_only_the_selected_language_definitions() {
    let output = render(&["--naming-consistency"], true).unwrap();
    let document = roxmltree::Document::parse(&output).unwrap();
    let votes: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("vote"))
        .map(|node| {
            (
                node.attribute("kind"),
                node.attribute("style"),
                node.attribute("count"),
            )
        })
        .collect();
    assert_eq!(votes, [(Some("fn"), Some("lower"), Some("3"))]);
}

#[test]
fn c_layout_rejects_a_rust_function_before_source_loading() {
    let error = render(&["--layout", "entry"], false).unwrap_err();
    assert!(
        error.to_string().contains("no C-family aggregate body"),
        "{error}"
    );
}

#[test]
fn naming_calibration_counts_the_observed_definition_style() {
    let output = render(&["--naming-calibration"], true).unwrap();
    let document = roxmltree::Document::parse(&output).unwrap();
    let styles: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .map(|node| (node.attribute("name"), node.attribute("count")))
        .collect();
    assert_eq!(styles, [(Some("lower"), Some("3"))]);
}

#[test]
fn executive_report_ranks_the_reached_helper_ahead_of_uncalled_entries() {
    let output = render(&["--report"], true).unwrap();
    let rows: Vec<_> = output
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| line.split('|').nth(1).unwrap().trim().trim_matches('`'))
        .collect();
    assert_eq!(rows, ["helper", "entry", "isolated"]);
}

#[test]
fn context_ratio_counts_internal_entities_without_external_context() {
    let output = render(&["--context-ratio"], false).unwrap();
    let document = roxmltree::Document::parse(&output).unwrap();
    assert_eq!(document.root_element().attribute("units"), Some("3"));
    let row = document
        .descendants()
        .find(|node| node.has_tag_name("s"))
        .unwrap();
    assert_eq!(row.attribute("n"), Some("entry"));
    assert_eq!(row.attribute("ents"), Some("1"));
    assert_eq!(row.attribute("ents_out"), Some("0"));
}

#[test]
fn quality_panel_does_not_rank_a_small_healthy_call_graph_as_a_problem() {
    let output = render(&["--quality-panel"], false).unwrap();
    let document = roxmltree::Document::parse(&output).unwrap();
    let report = document.root_element();
    assert_eq!(report.attribute("eligible"), Some("3"));
    assert_eq!(report.attribute("ranked"), Some("0"));
}

#[test]
fn metrics_reports_reference_direction_not_just_symbol_presence() {
    let output = render(&["--metrics"], false).unwrap();
    let document = roxmltree::Document::parse(&output).unwrap();
    let helper = document
        .descendants()
        .find(|node| node.attribute("n") == Some("helper"))
        .unwrap();
    let entry = document
        .descendants()
        .find(|node| node.attribute("n") == Some("entry"))
        .unwrap();
    assert_eq!(helper.attribute("in"), Some("1"));
    assert_eq!(helper.attribute("out"), Some("0"));
    assert_eq!(entry.attribute("in"), Some("0"));
    assert_eq!(entry.attribute("out"), Some("1"));
}

#[test]
fn detail_group_is_active_only_for_its_primary_flags() {
    let active = |flag: &str| {
        let cli = ripwire_rs::parse_from(["rw", flag]).unwrap();
        cli.plugins
            .decode::<DetailArgs>()
            .unwrap()
            .is_group_active()
    };
    for flag in [
        "--expand=a",
        "--outline=a",
        "--pack-top-n=1",
        "--tree",
        "--biggest-first",
        "--nonlocal-state",
        "--lint",
        "--lint-catalog",
        "--naming-locals",
        "--naming-consistency",
        "--naming-calibration",
        "--report",
        "--legend-dict",
    ] {
        assert!(active(flag), "{flag}");
    }
    for flag in [
        "--lint-select=a",
        "--lint-ignore=a",
        "--lint-max-per-rule=1",
    ] {
        assert!(!active(flag), "{flag}");
    }
}

#[test]
fn workspace_group_is_active_only_for_its_primary_flags() {
    let active = |flag: &str| {
        let cli = ripwire_rs::parse_from(["rw", flag]).unwrap();
        cli.plugins
            .decode::<ripwire_rs::commands::workspace::WorkspaceArgs>()
            .unwrap()
            .is_group_active()
    };
    for flag in [
        "--whereis=a",
        "--mentions=a",
        "--note-add=a",
        "--flip=a",
        "--plan-lint=a",
        "--owners",
        "--doc-drift",
        "--flags",
        "--stray-content",
        "--doctor",
        "--handoff",
        "--notes",
        "--skipped",
        "--replace-symbol-body=a:b",
        "--insert-before-symbol=a:b",
        "--insert-after-symbol=a:b",
    ] {
        assert!(active(flag), "{flag}");
    }
    for flag in [
        "--whereis-full-text",
        "--whereis-listing=defs",
        "--gateability",
        "--abi",
    ] {
        assert!(!active(flag), "{flag}");
    }
}
