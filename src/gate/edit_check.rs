use crate::domain::{IngestResult, Interner, NodeId, RefRole};
use crate::gate::contract::{ContractStatus, SignatureContract, compare_contracts};
use crate::graph::Graph;
use std::io::{self, Write};

/// Detailed site information for an incompatible call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompatibleCallSite {
    pub line: u32,
    pub arg_count: u16,
    pub required_params: u16,
}

/// Verification result for a single 1-hop caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerCheckResult {
    pub caller_id: NodeId,
    pub caller_name: String,
    pub file_path: String,
    pub line: u32,
    pub is_incompatible: bool,
    pub sites: Vec<IncompatibleCallSite>,
}

/// Comprehensive contract and 1-hop caller compatibility report.
#[derive(Debug, Clone, PartialEq)]
pub struct EditCheckReport {
    pub symbol_name: String,
    pub file_path: String,
    pub status: ContractStatus,
    pub total_callers: usize,
    pub incompatible_callers: usize,
    pub callers: Vec<CallerCheckResult>,
    pub gauges: crate::serialize::GraphGauges,
    pub at_stamp: String,
}

impl EditCheckReport {
    #[must_use]
    fn is_verdict_only_fold(&self) -> bool {
        self.at_stamp != "HEAD"
            && !self.at_stamp.ends_with("+dirty")
            && self.status == ContractStatus::Unchanged
            && self.incompatible_callers == 0
            && self.total_callers > 0
    }
}
impl crate::serialize::XmlReport for EditCheckReport {
    fn schema_tag(&self) -> &'static str {
        "edit-check"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.edit-check/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some("ripwire edit-check schema=ripwire.edit-check/v1: sym='s contract NOW vs HEAD.")
    }
    fn graph_gauges(&self) -> Option<crate::serialize::GraphGauges> {
        Some(self.gauges)
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let est_tokens = 60 + self.callers.len() * 24;
        let rows_next = if self.is_verdict_only_fold() {
            format!(
                " rows_next=\"--edit-check={} --limit={}\"",
                self.symbol_name, self.total_callers
            )
        } else {
            String::new()
        };
        write!(
            out,
            " sym=\"{}\" t=\"fn\" p=\"{}\" status=\"{}\" defs=\"1\" callers=\"{}\" incompatible=\"{}\" at=\"{}\" root=\".\"{rows_next} next=\"--test-gate={}\" est_tokens=\"{est_tokens}\"",
            self.symbol_name,
            self.file_path,
            self.status,
            self.total_callers,
            self.incompatible_callers,
            self.at_stamp,
            self.file_path
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        if self.is_verdict_only_fold() {
            return Ok(());
        }
        for c in &self.callers {
            write_caller_row(c, out)?;
        }
        Ok(())
    }
}

fn write_caller_row<W: Write>(c: &CallerCheckResult, out: &mut W) -> io::Result<()> {
    let incomp_attr = if c.is_incompatible {
        " incompatible=\"1\""
    } else {
        ""
    };
    if c.sites.is_empty() {
        return writeln!(
            out,
            "  <c n=\"{}\" p=\"{}:{}\"{}/>",
            c.caller_name, c.file_path, c.line, incomp_attr
        );
    }
    writeln!(
        out,
        "  <c n=\"{}\" p=\"{}:{}\"{}>",
        c.caller_name, c.file_path, c.line, incomp_attr
    )?;
    for s in &c.sites {
        writeln!(
            out,
            "    <site line=\"{}\" args=\"{}\" required=\"{}\"/>",
            s.line, s.arg_count, s.required_params
        )?;
    }
    writeln!(out, "  </c>")
}

fn check_caller_compatibility(
    caller_id: NodeId,
    sym_name: &str,
    target_params: u16,
    ingest: &IngestResult,
    interner: &Interner,
) -> CallerCheckResult {
    let caller_sym = &ingest[caller_id];
    let caller_name = interner[caller_sym.name].to_string();
    let file_path = ingest.sym_file_path(caller_sym, interner).to_string();

    let mut sites = Vec::new();
    for r in &ingest.references {
        if r.from_symbol == caller_id && r.role == RefRole::Call {
            let callee = &interner[r.callee_name];
            if callee == sym_name && r.arg_count_known && r.arg_count != target_params {
                sites.push(IncompatibleCallSite {
                    line: r.line,
                    arg_count: r.arg_count,
                    required_params: target_params,
                });
            }
        }
    }

    let is_incompatible = !sites.is_empty();
    CallerCheckResult {
        caller_id,
        caller_name,
        file_path,
        line: caller_sym.line,
        is_incompatible,
        sites,
    }
}

/// Verify a symbol's contract change and check 1-hop caller compatibility.
#[must_use]
pub fn check_symbol_contract(
    focus_sym_id: NodeId,
    head_baseline: Option<&SignatureContract>,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> Option<EditCheckReport> {
    let sym = ingest.sym(focus_sym_id)?;
    let sym_name = interner[sym.name].to_string();
    let file_path = ingest.sym_file_path(sym, interner).to_string();

    let is_public = match head_baseline {
        Some(base)
            if sym.lang.semantics().visibility_mode()
                == crate::languages::VisibilityMode::ExplicitModifier =>
        {
            base.is_public
        }
        _ => crate::gate::contract::is_symbol_name_public(&sym_name, sym.lang),
    };
    let now_contract = SignatureContract {
        name: sym_name.clone(),
        params: sym.params,
        is_public,
    };
    let status = compare_contracts(&now_contract, head_baseline);

    let caller_nodes: Vec<NodeId> = graph.callers(focus_sym_id).map(|(c, _)| c).collect();
    let total_callers = caller_nodes.len();

    let mut callers = Vec::with_capacity(total_callers);
    let mut incompatible_callers = 0usize;

    for caller_id in caller_nodes {
        let res = check_caller_compatibility(caller_id, &sym_name, sym.params, ingest, interner);
        if res.is_incompatible {
            incompatible_callers += 1;
        }
        callers.push(res);
    }

    callers.sort_by(|a, b| {
        b.is_incompatible
            .cmp(&a.is_incompatible)
            .then_with(|| a.caller_name.cmp(&b.caller_name))
            .then_with(|| a.file_path.cmp(&b.file_path))
    });

    Some(EditCheckReport {
        symbol_name: sym_name,
        file_path,
        status,
        total_callers,
        incompatible_callers,
        callers,
        gauges: graph.gauges(),
        at_stamp: "HEAD".to_string(),
    })
}
