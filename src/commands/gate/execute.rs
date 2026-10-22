use crate::cli::Cli;
use crate::commands::context::{OutputDialect, load_repo_analysis_for_cli};
use crate::commands::safety::get_working_tree_changed_files;
use std::io::{self, Write};
use std::path::Path;

#[derive(Default, Clone, Copy)]
pub struct GateDispatchArgs<'a> {
    pub edit_check: Option<&'a str>,
    pub affected: Option<&'a str>,
    pub quality_baseline: bool,
    pub quality_delta: bool,
    pub scope: Option<&'a str>,
    pub quality_ack: Option<&'a str>,
    pub ack_only: Option<&'a str>,
    pub show_stale: bool,
    pub allow_dirty: bool,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl GateDispatchArgs<'_> {
    pub fn is_active(&self) -> bool {
        self.edit_check.is_some()
            || self.affected.is_some()
            || self.quality_baseline
            || self.quality_delta
    }
}

pub fn execute_gate_action<W: Write>(
    args: GateDispatchArgs<'_>,
    state: &crate::Workspace,
    out: &mut crate::serialize::XmlWriter<'_, W>,
) -> Option<io::Result<()>> {
    if let Some(sym) = args.edit_check {
        let page = args.limit.map(|l| (args.offset.unwrap_or(0), l));
        return Some(execute_edit_check_with(sym, page, state, out));
    }
    if let Some(spec) = args.affected {
        return Some(execute_affected_with(spec, state, out));
    }
    if args.quality_baseline {
        return Some(execute_quality_baseline_with(state, args.allow_dirty, out));
    }
    if args.quality_delta {
        return Some(run_quality_delta_gate_with_args(args, state, out));
    }
    None
}

pub fn run_quality_delta_gate_with_args<W: Write>(
    args: GateDispatchArgs<'_>,
    state: &crate::Workspace,
    out: &mut crate::serialize::XmlWriter<'_, W>,
) -> io::Result<()> {
    match execute_quality_delta_with_args(args, state, out) {
        Ok(has_failures) if has_failures => {
            out.finish()?;
            std::process::exit(2);
        }
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

fn load_gate_state(
    cli: &Cli,
    root: &str,
    is_delta: bool,
) -> io::Result<crate::Workspace> {
    let (ingest, graph, pr, interner) =
        load_repo_analysis_for_cli(cli, root, is_delta)?;
    let mut state = crate::Workspace::from_analysis(root, ingest, graph, pr, interner);
    state.at = crate::git::history::compute_git_at_stamp(root);
    if is_delta && !get_working_tree_changed_files(root).is_empty() {
        let cutoff = crate::git::history::resolve_cutoff_timestamp(None, 1);
        state.commits = crate::git::history::walk_repository_history_scoped(root, cutoff).ok();
    }
    Ok(state)
}

/// Refuse gate modifiers passed without the verb they shape.
pub fn validate_gate_modifiers(args: &GateDispatchArgs<'_>) -> io::Result<()> {
    if args.allow_dirty && !args.quality_baseline {
        return Err(io::Error::other(
            "ripwire-rs: --allow-dirty modifies --quality-baseline; pass it too",
        ));
    }
    if args.show_stale && !args.quality_delta {
        return Err(io::Error::other(
            "ripwire-rs: --show-stale modifies --quality-delta; pass it too",
        ));
    }
    Ok(())
}

pub fn dispatch_gate_command(
    root: &str,
    args: GateDispatchArgs<'_>,
    cli: &Cli,
) -> Option<io::Result<()>> {
    if let Err(e) = validate_gate_modifiers(&args) {
        return Some(Err(e));
    }
    if !args.is_active() {
        return None;
    }
    let state = match load_gate_state(cli, root, args.quality_delta) {
        Ok(s) => s,
        Err(e) => return Some(Err(e)),
    };
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = OutputDialect::wrap_xml_writer(cli, &mut stdout_lock, false);
    let res = execute_gate_action(args, &state, &mut stdout);
    res.map(|result| result.and_then(|()| stdout.finish()))
}

pub fn execute_edit_check_with<W: Write>(
    symbol: &str,
    page: Option<(usize, usize)>,
    state: &crate::Workspace,
    out: &mut W,
) -> io::Result<()> {
    crate::query::safety::execute_edit_check_with(symbol, page, &state.safety_context(), out)
}

pub fn execute_affected_with<W: Write>(
    specs: &str,
    state: &crate::Workspace,
    out: &mut W,
) -> io::Result<()> {
    crate::query::safety::execute_affected_with(specs, &state.safety_context(), out)
}

pub fn execute_quality_baseline_with<W: Write>(
    state: &crate::Workspace,
    allow_dirty: bool,
    out: &mut W,
) -> io::Result<()> {
    let sources = crate::query::FileSources {
        root: Path::new(&state.root),
    };
    crate::query::safety::execute_quality_baseline_with(
        &state.query_context(&sources),
        allow_dirty,
        out,
    )
}

pub fn execute_quality_delta_with_args<W: Write>(
    args: GateDispatchArgs<'_>,
    state: &crate::Workspace,
    out: &mut W,
) -> io::Result<bool> {
    let sources = crate::query::FileSources {
        root: Path::new(&state.root),
    };
    crate::query::safety::execute_quality_delta_with_args(
        crate::gate::quality_delta::DeltaCliOptions {
            scope: args.scope,
            quality_ack: args.quality_ack,
            ack_only: args.ack_only,
            show_stale: args.show_stale,
        },
        &state.query_context(&sources),
        out,
    )
}
