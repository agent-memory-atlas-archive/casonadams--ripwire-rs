//! `--legend-dict`: full schema legend dictionary.
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "legend-dict",
    args: LegendDictArgs,
    static: LEGEND_DICT,
    run: report(render),
    /// Emit full schema legend dictionary
    #[arg(long = "legend-dict")]
    legend_dict: bool,
}

pub fn render<W: Write>(
    _ctx: &ExecutionContext<'_>,
    _args: &LegendDictArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    crate::serialize::serialize_legend_dict_xml(out)
}
