use ripwire_rs::commands::context::{ExecutionContext, Paginated, VerbGroup};
use ripwire_rs::commands::{
    around, biggest_first, communities, context_ratio, external_surface, layout,
    naming_calibration, naming_consistency, quality_panel, report, symbol_metrics, tree,
};
use ripwire_rs::commands::{
    around::AroundArgs, biggest_first::BiggestFirstArgs, communities::CommunitiesArgs,
    context_ratio::ContextRatioArgs, external_surface::ExternalSurfaceArgs, layout::LayoutArgs,
    naming_calibration::NamingCalibrationArgs, naming_consistency::NamingConsistencyArgs,
    quality_panel::QualityPanelArgs, symbol_metrics::SymbolMetricsArgs,
};
use ripwire_rs::serialize::XmlWriter;
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, PageRankConfig, ingest_in_memory_files};
use std::collections::BTreeSet;

fn render(
    args: &[&str],
    body: impl FnOnce(&ExecutionContext<'_>, &mut XmlWriter<'_, Vec<u8>>) -> std::io::Result<()>,
) -> std::io::Result<String> {
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
        body(&ctx, &mut writer)?;
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
    let output = render(&["--tree"], |ctx, out| {
        tree::render(ctx, ctx.cli.page_request(), out)
    })
    .unwrap();
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
    let output = render(&["--biggest-first"], |ctx, out| {
        biggest_first::render(
            ctx,
            &ctx.cli.plugins.decode::<BiggestFirstArgs>().unwrap(),
            out,
        )
    })
    .unwrap();
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
        let cli = ripwire_rs::parse_from(args).unwrap();
        let error = ripwire_rs::run_cli_args(cli).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn community_dispatch_separates_an_isolated_definition_from_a_connected_pair() {
    let output = render(&["--communities"], |ctx, out| {
        communities::render(
            ctx,
            &ctx.cli.plugins.decode::<CommunitiesArgs>().unwrap(),
            out,
        )
    })
    .unwrap();
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
    let output = render(&["--around", "entry"], |ctx, out| {
        around::render(ctx, &ctx.cli.plugins.decode::<AroundArgs>().unwrap(), out)
    })
    .unwrap();
    assert_eq!(
        names(&output),
        ["entry", "helper"].map(str::to_owned).into()
    );
}

#[test]
fn external_surface_does_not_reclassify_a_resolved_internal_call() {
    let output = render(&["--external-surface"], |ctx, out| {
        external_surface::render(
            ctx,
            &ctx.cli.plugins.decode::<ExternalSurfaceArgs>().unwrap(),
            out,
        )
    })
    .unwrap();
    let document = roxmltree::Document::parse(&output).unwrap();
    assert!(!document.descendants().any(|node| node.has_tag_name("x")));
}

#[test]
fn naming_votes_count_only_the_selected_language_definitions() {
    let output = render(&["--naming-consistency"], |ctx, out| {
        naming_consistency::render(
            ctx,
            &ctx.cli.plugins.decode::<NamingConsistencyArgs>().unwrap(),
            out,
        )
    })
    .unwrap();
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
    let error = render(&["--layout", "entry"], |ctx, out| {
        layout::render(ctx, &ctx.cli.plugins.decode::<LayoutArgs>().unwrap(), out)
    })
    .unwrap_err();
    assert!(
        error.to_string().contains("no C-family aggregate body"),
        "{error}"
    );
}

#[test]
fn naming_calibration_counts_the_observed_definition_style() {
    let output = render(&["--naming-calibration"], |ctx, out| {
        naming_calibration::render(
            ctx,
            &ctx.cli.plugins.decode::<NamingCalibrationArgs>().unwrap(),
            out,
        )
    })
    .unwrap();
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
    let output = render(&["--report"], report::render).unwrap();
    let rows: Vec<_> = output
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| line.split('|').nth(1).unwrap().trim().trim_matches('`'))
        .collect();
    assert_eq!(rows, ["helper", "entry", "isolated"]);
}

#[test]
fn context_ratio_counts_internal_entities_without_external_context() {
    let output = render(&["--context-ratio"], |ctx, out| {
        context_ratio::render(
            ctx,
            &ctx.cli.plugins.decode::<ContextRatioArgs>().unwrap(),
            out,
        )
    })
    .unwrap();
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
    let output = render(&["--quality-panel"], |ctx, out| {
        quality_panel::render(
            ctx,
            &ctx.cli.plugins.decode::<QualityPanelArgs>().unwrap(),
            out,
        )
    })
    .unwrap();
    let document = roxmltree::Document::parse(&output).unwrap();
    let report = document.root_element();
    assert_eq!(report.attribute("eligible"), Some("3"));
    assert_eq!(report.attribute("ranked"), Some("0"));
}

#[test]
fn metrics_reports_reference_direction_not_just_symbol_presence() {
    let output = render(&["--metrics"], |ctx, out| {
        symbol_metrics::render(
            ctx,
            &ctx.cli.plugins.decode::<SymbolMetricsArgs>().unwrap(),
            out,
        )
    })
    .unwrap();
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

fn selected<A: clap::FromArgMatches + 'static>(
    cli: &ripwire_rs::Cli,
    plugin: &ripwire_rs::commands::Plugin<A>,
) -> bool {
    cli.plugins
        .decode::<A>()
        .is_some_and(|args| (plugin.selected)(&args))
}

#[test]
fn detail_group_is_active_only_for_its_primary_flags() {
    use ripwire_rs::commands::{
        biggest_first, expand, legend_dict, lint, lint_catalog, naming_calibration,
        naming_consistency, naming_locals, nonlocal_state, outline, pack_top_n, report, tree,
    };
    let active = |flag: &str| {
        let cli = ripwire_rs::parse_from(["rw", flag]).unwrap();
        selected(&cli, &expand::EXPAND)
            || selected(&cli, &outline::OUTLINE)
            || selected(&cli, &pack_top_n::PACK_TOP_N)
            || selected(&cli, &tree::TREE)
            || selected(&cli, &biggest_first::BIGGEST_FIRST)
            || selected(&cli, &nonlocal_state::NONLOCAL_STATE)
            || selected(&cli, &lint::LINT)
            || selected(&cli, &lint_catalog::LINT_CATALOG)
            || selected(&cli, &naming_locals::NAMING_LOCALS)
            || selected(&cli, &naming_consistency::NAMING_CONSISTENCY)
            || selected(&cli, &naming_calibration::NAMING_CALIBRATION)
            || selected(&cli, &report::REPORT)
            || selected(&cli, &legend_dict::LEGEND_DICT)
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
        let p = &cli.plugins;
        use ripwire_rs::commands as c;
        [
            (c::owners::OWNERS.selected)(&p.decode().unwrap()),
            (c::stray_content::STRAY_CONTENT.selected)(&p.decode().unwrap()),
            (c::replace_symbol_body::REPLACE_SYMBOL_BODY.selected)(&p.decode().unwrap()),
            (c::insert_before_symbol::INSERT_BEFORE_SYMBOL.selected)(&p.decode().unwrap()),
            (c::insert_after_symbol::INSERT_AFTER_SYMBOL.selected)(&p.decode().unwrap()),
            (c::whereis::WHEREIS.selected)(&p.decode().unwrap()),
            (c::doc_drift::DOC_DRIFT.selected)(&p.decode().unwrap()),
            (c::flags_report::FLAGS.selected)(&p.decode().unwrap()),
            (c::mentions::MENTIONS.selected)(&p.decode().unwrap()),
            (c::doctor::DOCTOR.selected)(&p.decode().unwrap()),
            (c::handoff::HANDOFF.selected)(&p.decode().unwrap()),
            (c::notes::NOTES.selected)(&p.decode().unwrap()),
            (c::note_add::NOTE_ADD.selected)(&p.decode().unwrap()),
            (c::flip::FLIP.selected)(&p.decode().unwrap()),
            (c::plan_lint::PLAN_LINT.selected)(&p.decode().unwrap()),
            (c::skipped::SKIPPED.selected)(&p.decode().unwrap()),
        ]
        .into_iter()
        .any(|on| on)
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

#[test]
fn safety_group_is_active_only_for_its_primary_flags() {
    let active = |flag: &str| {
        let cli = ripwire_rs::parse_from(["rw", flag]).unwrap();
        cli.plugins
            .decode::<ripwire_rs::commands::safety::SafetyArgs>()
            .unwrap()
            .is_group_active()
    };
    for flag in [
        "--test-gate=a",
        "--situ=a",
        "--safe-delete=a",
        "--exercises=a",
        "--pr-context=a",
        "--merge-scout=a",
        "--dmm=a",
        "--plan-lanes",
        "--scan-skills=a",
        "--edit-plan=a",
        "--edit-payload=a",
        "--partition",
        "--seams",
        "--pin-census",
        "--baseline",
        "--baseline-update",
    ] {
        assert!(active(flag), "{flag}");
    }
    for flag in ["--owners", "--doc-drift", "--flags", "--lint-select=a"] {
        assert!(!active(flag), "{flag}");
    }
}

#[test]
fn architecture_group_is_active_only_for_its_primary_flags() {
    use ripwire_rs::commands::{
        arch, around, comment_coherence, communities, context_ratio, dead_code, deps, ensemble,
        external_surface, field_affinity, layout, mermaid, quality_panel, symbol_metrics, zoom,
    };
    let active = |flag: &str| {
        let cli = ripwire_rs::parse_from(["rw", flag]).unwrap();
        selected(&cli, &dead_code::DEAD_CODE)
            || selected(&cli, &communities::COMMUNITIES)
            || selected(&cli, &deps::DEPS)
            || selected(&cli, &around::AROUND)
            || selected(&cli, &external_surface::EXTERNAL_SURFACE)
            || selected(&cli, &arch::ARCH)
            || selected(&cli, &mermaid::MERMAID)
            || selected(&cli, &zoom::ZOOM)
            || selected(&cli, &symbol_metrics::SYMBOL_METRICS)
            || selected(&cli, &quality_panel::QUALITY_PANEL)
            || selected(&cli, &layout::LAYOUT)
            || selected(&cli, &field_affinity::FIELD_AFFINITY)
            || selected(&cli, &context_ratio::CONTEXT_RATIO)
            || selected(&cli, &ensemble::ENSEMBLE)
            || selected(&cli, &comment_coherence::COMMENT_COHERENCE)
    };
    for flag in [
        "--dead-code",
        "--communities",
        "--community=1",
        "--deps",
        "--around=a",
        "--external-surface",
        "--arch=a",
        "--mermaid",
        "--zoom=2",
        "--metrics",
        "--quality-panel",
        "--layout=a",
        "--field-affinity",
        "--context-ratio",
        "--ensemble",
        "--comment-coherence",
    ] {
        assert!(active(flag), "{flag}");
    }
    for flag in [
        "--sarif",
        "--zoom-levels=1",
        "--around-depth=2",
        "--include-builtins",
    ] {
        assert!(!active(flag), "{flag}");
    }
}
