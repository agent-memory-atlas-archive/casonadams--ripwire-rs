//! `--legend-dict`: full schema legend dictionary.
use super::{Plugin, Run};
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct LegendDictArgs {
    /// Emit full schema legend dictionary
    #[arg(long = "legend-dict")]
    pub legend_dict: bool,
}

pub fn render(out: &mut XmlWriter<'_, impl Write>) -> io::Result<()> {
    crate::serialize::serialize_legend_dict_xml(out)
}

fn run(run: &Run<'_>, _args: &LegendDictArgs) -> io::Result<()> {
    run.report(|_, out| render(out))
}

pub static LEGEND_DICT: Plugin<LegendDictArgs> = Plugin {
    name: "legend-dict",
    options: &[],
    selected: |args| args.legend_dict,
    run,
    check: None,
    tools: &[],
};
