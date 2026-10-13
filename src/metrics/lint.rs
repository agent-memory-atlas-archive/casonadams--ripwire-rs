use crate::domain::{IngestResult, Interner, SymKind};
use crate::graph::Graph;
use std::io::{self, Write};

/// An emitted code smell or lint finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintFinding {
    pub rule: String,
    pub message: String,
    pub path: String,
    pub line: u32,
    pub severity: String,
}

/// Catalog entry describing an AST code smell rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintRuleDesc {
    pub name: &'static str,
    pub severity: &'static str,
    pub category: &'static str,
    pub description: &'static str,
}

pub const LINT_RULES: &[LintRuleDesc] = &[
    LintRuleDesc {
        name: "complexity-spike",
        severity: "major",
        category: "maintainability",
        description: "Cognitive complexity exceeds the Sonar Campbell ceiling of 15",
    },
    LintRuleDesc {
        name: "deep-nesting",
        severity: "major",
        category: "maintainability",
        description: "Maximum control statement nesting depth exceeds 5 levels",
    },
    LintRuleDesc {
        name: "parameter-spill",
        severity: "minor",
        category: "design",
        description: "Parameter count exceeds 5 positional arguments",
    },
    LintRuleDesc {
        name: "verbosity-spike",
        severity: "minor",
        category: "readability",
        description: "Function length exceeds 100 non-comment lines of code",
    },
    LintRuleDesc {
        name: "dead-function",
        severity: "major",
        category: "architecture",
        description: "Internal linkage function with zero callers in workspace graph",
    },
];

/// Scan the workspace for AST code smells across all symbols.
#[must_use]
pub fn run_lint_scan(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();

    for sym in &ingest.symbols {
        if sym.kind != SymKind::Function && sym.kind != SymKind::Method {
            continue;
        }

        let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
        let n = interner.resolve(&sym.name);

        if sym.ccx > 15 {
            findings.push(LintFinding {
                rule: "complexity-spike".to_string(),
                message: format!("Function '{n}' cognitive complexity {} exceeds 15", sym.ccx),
                path: p.to_string(),
                line: sym.line,
                severity: "major".to_string(),
            });
        }

        if sym.max_nest > 5 {
            findings.push(LintFinding {
                rule: "deep-nesting".to_string(),
                message: format!("Function '{n}' max nesting {} exceeds 5", sym.max_nest),
                path: p.to_string(),
                line: sym.line,
                severity: "major".to_string(),
            });
        }

        if sym.params > 5 {
            findings.push(LintFinding {
                rule: "parameter-spill".to_string(),
                message: format!("Function '{n}' has {} parameters, exceeding 5", sym.params),
                path: p.to_string(),
                line: sym.line,
                severity: "minor".to_string(),
            });
        }

        if graph.in_degree(sym.id) == 0 && !crate::gate::affected::is_test_path(p) {
            findings.push(LintFinding {
                rule: "dead-function".to_string(),
                message: format!("Internal function '{n}' has zero incoming callers"),
                path: p.to_string(),
                line: sym.line,
                severity: "major".to_string(),
            });
        }
    }
    findings
}

/// Serialize lint catalog to canonical format.
pub fn serialize_lint_catalog<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "<lintcatalog schema=\"ripwire.lint-catalog/v1\" rules=\"{}\">",
        LINT_RULES.len()
    )?;
    for r in LINT_RULES {
        writeln!(
            out,
            "  <rule name=\"{}\" sev=\"{}\" cat=\"{}\">{}</rule>",
            r.name, r.severity, r.category, r.description
        )?;
    }
    writeln!(out, "</lintcatalog>")
}

/// Serialize lint scan findings to canonical XML format.
pub fn serialize_lint_xml<W: Write>(
    root: &str,
    findings: &[LintFinding],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<lint schema=\"ripwire.lint/v1\" root=\"{root}\" findings=\"{}\">",
        findings.len()
    )?;
    for f in findings {
        writeln!(
            out,
            "  <f rule=\"{}\" p=\"{}:{}\" sev=\"{}\">{}</f>",
            f.rule, f.path, f.line, f.severity, f.message
        )?;
    }
    writeln!(out, "</lint>")
}
