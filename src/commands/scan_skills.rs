//! `--scan-skills [DIR]`: security audit scanner for agent skill files; exits 2 on critical, 1 on warnings.
use super::common::safety::report_exit;
use super::common::safety_queries::execute_scan_skills_dispatch;
use super::{Plugin, Run};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ScanSkillsArgs {
    /// Security audit scanner for agent skill files
    #[arg(
        long = "scan-skills",
        visible_alias = "scan-skill",
        num_args = 0..=1,
        default_missing_value = "",
        value_name = "DIR"
    )]
    pub scan_skills: Option<Option<String>>,
}

fn run(run: &Run<'_>, args: &ScanSkillsArgs) -> io::Result<()> {
    let raw = args.scan_skills.clone().flatten().unwrap_or_default();
    report_exit(run, |_ctx, out| execute_scan_skills_dispatch(&raw, out).map(Some))
}

pub static SCAN_SKILLS: Plugin<ScanSkillsArgs> = Plugin {
    name: "scan-skills",
    options: &[],
    selected: |args| args.scan_skills.is_some(),
    run,
    check: None,
    tools: &[],
};
