//! `--plan-lint FILE`: lint the structure of a PLAN file.
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::serialize::{XmlEscapeExt, XmlWriter};
use std::io::{self, Write};

report_command! {
    name: "plan-lint",
    args: PlanLintArgs,
    static: PLAN_LINT,
    run: report(render),
    /// Lint structure of a PLAN file
    #[arg(long = "plan-lint", value_name = "FILE")]
    plan_lint: Option<String>,
}

fn render<W: Write>(
    _ctx: &ExecutionContext<'_>,
    args: &PlanLintArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let plan_file = args.plan_lint.as_deref().unwrap_or_default();
    let content = std::fs::read_to_string(plan_file).unwrap_or_default();
    render_plan_lint_xml(plan_file, &content, out)
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
