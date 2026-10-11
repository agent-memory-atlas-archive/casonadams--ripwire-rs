use crate::domain::{IngestResult, Interner, NodeId, RefRole};
use crate::gate::contract::{compare_contracts, ContractStatus, SignatureContract};
use crate::graph::Graph;
use std::fmt::Write as FmtWrite;
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
}

impl EditCheckReport {
    /// Format report as dense minified XML conforming to Ripwire schema.
    pub fn write_xml<W: Write>(&self, out: &mut W) -> io::Result<()> {
        writeln!(
            out,
            "<edit-check sym=\"{}\" p=\"{}\" status=\"{}\" callers=\"{}\" incompatible=\"{}\">",
            self.symbol_name,
            self.file_path,
            self.status.as_str(),
            self.total_callers,
            self.incompatible_callers
        )?;
        for c in &self.callers {
            let incomp_attr = if c.is_incompatible {
                " incompatible=\"1\""
            } else {
                ""
            };
            if c.sites.is_empty() {
                writeln!(
                    out,
                    "  <caller sym=\"{}\" p=\"{}:{}\"{}/>",
                    c.caller_name, c.file_path, c.line, incomp_attr
                )?;
            } else {
                writeln!(
                    out,
                    "  <caller sym=\"{}\" p=\"{}:{}\"{}>",
                    c.caller_name, c.file_path, c.line, incomp_attr
                )?;
                for s in &c.sites {
                    writeln!(
                        out,
                        "    <site line=\"{}\" args=\"{}\" required=\"{}\"/>",
                        s.line, s.arg_count, s.required_params
                    )?;
                }
                writeln!(out, "  </caller>")?;
            }
        }
        writeln!(out, "</edit-check>")
    }

    /// Render XML as string.
    #[must_use]
    pub fn to_xml_string(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "<edit-check sym=\"{}\" p=\"{}\" status=\"{}\" callers=\"{}\" incompatible=\"{}\">",
            self.symbol_name,
            self.file_path,
            self.status.as_str(),
            self.total_callers,
            self.incompatible_callers
        );
        for c in &self.callers {
            let incomp_attr = if c.is_incompatible {
                " incompatible=\"1\""
            } else {
                ""
            };
            if c.sites.is_empty() {
                let _ = writeln!(
                    s,
                    "  <caller sym=\"{}\" p=\"{}:{}\"{}/>",
                    c.caller_name, c.file_path, c.line, incomp_attr
                );
            } else {
                let _ = writeln!(
                    s,
                    "  <caller sym=\"{}\" p=\"{}:{}\"{}>",
                    c.caller_name, c.file_path, c.line, incomp_attr
                );
                for site in &c.sites {
                    let _ = writeln!(
                        s,
                        "    <site line=\"{}\" args=\"{}\" required=\"{}\"/>",
                        site.line, site.arg_count, site.required_params
                    );
                }
                let _ = writeln!(s, "  </caller>");
            }
        }
        let _ = writeln!(s, "</edit-check>");
        s
    }
}

fn check_caller_compatibility(
    caller_id: NodeId,
    sym_name: &str,
    target_params: u16,
    ingest: &IngestResult,
    interner: &Interner,
) -> CallerCheckResult {
    let caller_sym = &ingest.symbols[caller_id as usize];
    let caller_name = interner.resolve(&caller_sym.name).to_string();
    let file_path = interner
        .resolve(&ingest.files[caller_sym.file_id as usize].path)
        .to_string();

    let mut sites = Vec::new();
    for r in &ingest.references {
        if r.from_symbol == caller_id && r.role == RefRole::Call {
            let callee = interner.resolve(&r.callee_name);
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
    if focus_sym_id as usize >= ingest.symbols.len() {
        return None;
    }

    let sym = &ingest.symbols[focus_sym_id as usize];
    let sym_name = interner.resolve(&sym.name).to_string();
    let file_path = interner
        .resolve(&ingest.files[sym.file_id as usize].path)
        .to_string();

    let now_contract = SignatureContract {
        name: sym_name.clone(),
        params: sym.params,
        is_public: true,
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FileInfo, Lang, SymKind, Symbol};

    #[test]
    fn test_caller_compatibility_verification() {
        let interner = Interner::new();
        let mut ingest = IngestResult::new();

        let p = interner.intern("src/calc.rs");
        ingest.files.push(FileInfo {
            id: 0,
            path: p,
            lang: Lang::Rust,
            line_count: 50,
            byte_size: 1000,
        });

        // Target function: compute(a, b, c) -> now 3 params
        let target_name = interner.intern("compute");
        let mut target_sym = Symbol::new(
            0,
            target_name,
            target_name,
            SymKind::Function,
            Lang::Rust,
            0,
            10,
        );
        target_sym.params = 3;

        // Caller: runner() -> calls compute with 2 arguments
        let caller_name = interner.intern("runner");
        let caller_sym = Symbol::new(
            1,
            caller_name,
            caller_name,
            SymKind::Function,
            Lang::Rust,
            0,
            20,
        );

        ingest.symbols.push(target_sym);
        ingest.symbols.push(caller_sym);

        let mut ref_call =
            crate::domain::Reference::new(1, target_name, 0, 25, Lang::Rust, RefRole::Call);
        ref_call.arg_count = 2;
        ref_call.arg_count_known = true;
        ingest.references.push(ref_call);

        let graph = Graph::build(&ingest, &interner);

        let baseline = SignatureContract {
            name: "compute".to_string(),
            params: 2,
            is_public: true,
        };

        let report = check_symbol_contract(0, Some(&baseline), &ingest, &graph, &interner).unwrap();
        assert_eq!(report.symbol_name, "compute");
        assert_eq!(report.total_callers, 1);
        assert_eq!(report.incompatible_callers, 1);
        assert!(report.callers[0].is_incompatible);
        assert_eq!(report.callers[0].sites.len(), 1);
        assert_eq!(report.callers[0].sites[0].arg_count, 2);
        assert_eq!(report.callers[0].sites[0].required_params, 3);

        let xml = report.to_xml_string();
        assert!(xml.contains("status=\"contract-change\""));
        assert!(xml.contains("incompatible=\"1\""));
        assert!(xml.contains("<site line=\"25\" args=\"2\" required=\"3\"/>"));
    }
}
