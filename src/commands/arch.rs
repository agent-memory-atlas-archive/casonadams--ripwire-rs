//! `--arch RULES_FILE`: enforce architectural layering rules; exits 2 when any rule is violated.
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "arch",
    args: ArchArgs,
    static: ARCH,
    run: report(render),
    /// Enforce architectural layering rules from file (exits 2 on violation)
    #[arg(long = "arch", value_name = "RULES_FILE")]
    arch: Option<String>,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &ArchArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let content = std::fs::read_to_string(args.arch.as_deref().unwrap_or_default())?;
    let cfg = crate::gate::arch::ArchConfig::parse(&content);
    let violations =
        crate::gate::arch::check_architecture_rules(&cfg, ctx.ingest, ctx.graph, ctx.interner);
    crate::gate::arch::serialize_arch_xml(ctx.root, &violations, out)?;
    if !violations.is_empty() {
        out.finish()?;
        std::process::exit(2);
    }
    Ok(())
}
