//! `--plan-lint FILE`: lint the structure of a PLAN file.
use super::{Plugin, Run};
use crate::serialize::XmlEscapeExt;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct PlanLintArgs {
    /// Lint structure of a PLAN file
    #[arg(long = "plan-lint", value_name = "FILE")]
    pub plan_lint: Option<String>,
}

pub static PLAN_LINT: Plugin<PlanLintArgs> = Plugin {
    name: "plan-lint",
    options: &[],
    selected: |args| args.plan_lint.is_some(),
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &PlanLintArgs) -> io::Result<()> {
    let plan_file = args.plan_lint.as_deref().unwrap_or_default();
    run.report(|_, out| {
        let content = std::fs::read_to_string(plan_file).unwrap_or_default();
        render_plan_lint_xml(plan_file, &content, out)
    })
}

pub fn render_plan_lint_xml<W: Write>(
    plan_file: &str,
    content: &str,
    out: &mut W,
) -> io::Result<()> {
    let headings = content.lines().filter(|l| l.starts_with('#')).count();
    let tasks = content.lines().filter(|l| l.contains("- [")).count();
    writeln!(
        out,
        "<plan-lint schema=\"ripwire-rs.plan-lint/v1\" file=\"{}\" headings=\"{headings}\" tasks=\"{tasks}\" ok=\"{}\"/>",
        plan_file.xml_escaped(),
        usize::from(headings > 0)
    )
}
