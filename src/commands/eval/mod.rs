//! Evaluation harnesses: `--eval` and `--eval-retrieval`. `--eval-mined`, `--eval-skills` and
//! `--eval-stray` are parsed only.
use super::{Command, SharedOption};
use crate::cli::Cli;
use crate::commands::context::load_repo_analysis_for_cli;
use clap::{Args, Command as ClapCommand};
use std::io;

/// Top-level evaluation flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct EvalFlags {
    /// Run co-change recall vs lexical rankers self-evaluation
    #[arg(long = "eval")]
    pub eval: bool,
    /// Known-item retrieval benchmark (MRR, recall@1/5/10)
    #[arg(long = "eval-retrieval")]
    pub eval_retrieval: bool,
    /// Mined git commit benchmark evaluation
    #[arg(long = "eval-mined")]
    pub eval_mined: bool,
    /// Agent skill effectiveness evaluation
    #[arg(long = "eval-skills")]
    pub eval_skills: bool,
    /// Cross-branch stray content evaluation
    #[arg(long = "eval-stray")]
    pub eval_stray: bool,
    /// Score name-based call resolution against a SCIP index FILE (precision, recall, sampled disagreements)
    #[arg(long = "eval-scip", value_name = "FILE")]
    pub eval_scip: Option<String>,
    /// Sampled disagreements per verdict and reason for --eval-scip
    #[arg(long = "eval-samples", value_name = "N", default_value_t = 10)]
    pub eval_samples: usize,
}

pub struct Eval;
pub static EVAL: Eval = Eval;

impl Command for Eval {
    fn name(&self) -> &'static str {
        "eval"
    }

    fn options(&self) -> &'static [SharedOption] {
        &[]
    }

    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        EvalFlags::augment_args(cmd)
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let flags = cli.plugins.decode::<EvalFlags>()?;
        if flags.eval {
            let mut stdout = io::stdout().lock();
            return Some(crate::metrics::run_cochange_eval(&mut stdout));
        }
        if flags.eval_retrieval {
            return Some((|| {
                let (ingest, _graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
                let mut stdout = io::stdout().lock();
                crate::metrics::run_retrieval_eval_for_repo(
                    &ingest,
                    &interner,
                    &pr.ranks,
                    &mut stdout,
                )
            })());
        }
        flags
            .eval_scip
            .as_deref()
            .map(|path| run_scip_eval(cli, root, path, flags.eval_samples))
    }
}

fn run_scip_eval(cli: &Cli, root: &str, path: &str, samples: usize) -> io::Result<()> {
    if cli.scip.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ripwire-rs: --eval-scip scores resolution without SCIP; drop --scip",
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| io::Error::new(e.kind(), format!("ripwire-rs: --eval-scip {path}: {e}")))?;
    let (ingest, _graph, _pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
    let (sites, documented) = crate::resolve::scip::indexed_sites(&bytes, &ingest, &interner)
        .map_err(|e| io::Error::other(format!("ripwire-rs: --eval-scip {path}: {e}")))?;
    let report = crate::metrics::scip_eval::evaluate_against_scip(
        &ingest,
        &interner,
        &sites,
        &documented,
    );
    crate::metrics::scip_eval::serialize_scip_eval_xml(
        &report,
        &ingest,
        &interner,
        samples,
        &mut io::stdout().lock(),
    )
}
