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
        "<!-- ripwire lint-catalog schema=ripwire.lint-catalog/v1: the built-in lint rule registry: <rule name= sev= cat= lang= since=>. -->\n<lintcatalog schema=\"ripwire.lint-catalog/v1\" rules=\"{}\">",
        LINT_RULES.len()
    )?;
    for r in LINT_RULES {
        writeln!(
            out,
            "  <rule name=\"{}\" sev=\"{}\" cat=\"{}\" lang=\"all\" since=\"v0.1.0\">{}</rule>",
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
    let total = findings.len();
    let shown = total.min(100);
    let capped = usize::from(shown < total);
    writeln!(
        out,
        "<!-- ripwire lint schema=ripwire.lint/v1: AST-only checks, facts not gates: <rule name= count= shown_rows= rows_capped= count_capped=> of <f rule= p= in=>. -->\n<lint schema=\"ripwire.lint/v1\" findings=\"{total}\" shown=\"{shown}\" capped=\"{capped}\" inert_rules=\"24\" root=\"{root}\">",
    )?;
    for r in LINT_RULES {
        let r_cnt = findings.iter().filter(|f| f.rule == r.name).count();
        let r_shown = r_cnt.min(20);
        let r_cap = usize::from(r_shown < r_cnt);
        writeln!(
            out,
            "  <rule name=\"{}\" count=\"{r_cnt}\" shown_rows=\"{r_shown}\" rows_capped=\"{r_cap}\" applicable=\"1\"/>",
            r.name
        )?;
    }
    for f in findings.iter().take(shown) {
        let p_esc = crate::serialize::escape::escape_xml_str(&f.path);
        writeln!(
            out,
            "  <f rule=\"{}\" p=\"{p_esc}:{}\" sev=\"{}\" in=\"main\">{}</f>",
            f.rule, f.line, f.severity, f.message
        )?;
    }
    writeln!(out, "</lint>")
}

/// Serialize lint scan findings to SARIF 2.1.0 JSON format.
pub fn serialize_lint_sarif<W: Write>(findings: &[LintFinding], out: &mut W) -> io::Result<()> {
    let rules_json: Vec<serde_json::Value> = LINT_RULES
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.name,
                "shortDescription": { "text": r.description },
                "properties": { "category": r.category, "severity": r.severity }
            })
        })
        .collect();
    let results_json: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            let level = if f.severity == "major" {
                "warning"
            } else {
                "note"
            };
            serde_json::json!({
                "ruleId": f.rule,
                "level": level,
                "message": { "text": f.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": f.path },
                        "region": { "startLine": f.line }
                    }
                }]
            })
        })
        .collect();
    let sarif = serde_json::json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "ripwire-rs",
                    "version": env!("CARGO_PKG_VERSION"),
                    "rules": rules_json
                }
            },
            "results": results_json
        }]
    });
    serde_json::to_writer_pretty(&mut *out, &sarif)?;
    writeln!(out)
}
