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

/// Trait encapsulating a single AST symbol lint rule and its catalog metadata.
pub trait SymbolLintRule: Sync {
    fn desc(&self) -> LintRuleDesc;
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        graph: &Graph,
    ) -> Option<LintFinding>;
}

struct ComplexitySpikeRule;
impl SymbolLintRule for ComplexitySpikeRule {
    fn desc(&self) -> LintRuleDesc {
        LINT_RULES[0].clone()
    }
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        _graph: &Graph,
    ) -> Option<LintFinding> {
        (sym.ccx > 15).then(|| LintFinding {
            rule: "complexity-spike".to_string(),
            message: format!(
                "Function '{name}' cognitive complexity {} exceeds 15",
                sym.ccx
            ),
            path: file.to_string(),
            line: sym.line,
            severity: "major".to_string(),
        })
    }
}

struct DeepNestingRule;
impl SymbolLintRule for DeepNestingRule {
    fn desc(&self) -> LintRuleDesc {
        LINT_RULES[1].clone()
    }
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        _graph: &Graph,
    ) -> Option<LintFinding> {
        (sym.max_nest > 5).then(|| LintFinding {
            rule: "deep-nesting".to_string(),
            message: format!("Function '{name}' max nesting {} exceeds 5", sym.max_nest),
            path: file.to_string(),
            line: sym.line,
            severity: "major".to_string(),
        })
    }
}

struct ParameterSpillRule;
impl SymbolLintRule for ParameterSpillRule {
    fn desc(&self) -> LintRuleDesc {
        LINT_RULES[2].clone()
    }
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        _graph: &Graph,
    ) -> Option<LintFinding> {
        (sym.params > 5).then(|| LintFinding {
            rule: "parameter-spill".to_string(),
            message: format!(
                "Function '{name}' has {} parameters, exceeding 5",
                sym.params
            ),
            path: file.to_string(),
            line: sym.line,
            severity: "minor".to_string(),
        })
    }
}

struct VerbositySpikeRule;
impl SymbolLintRule for VerbositySpikeRule {
    fn desc(&self) -> LintRuleDesc {
        LINT_RULES[3].clone()
    }
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        _graph: &Graph,
    ) -> Option<LintFinding> {
        (sym.loc > 100).then(|| LintFinding {
            rule: "verbosity-spike".to_string(),
            message: format!("Function '{name}' length {} exceeds 100 LOC", sym.loc),
            path: file.to_string(),
            line: sym.line,
            severity: "minor".to_string(),
        })
    }
}

struct DeadFunctionRule;
impl SymbolLintRule for DeadFunctionRule {
    fn desc(&self) -> LintRuleDesc {
        LINT_RULES[4].clone()
    }
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        graph: &Graph,
    ) -> Option<LintFinding> {
        (graph.in_degree(sym.id) == 0 && !crate::gate::affected::is_test_path(file)).then(|| {
            LintFinding {
                rule: "dead-function".to_string(),
                message: format!("Internal function '{name}' has zero incoming callers"),
                path: file.to_string(),
                line: sym.line,
                severity: "major".to_string(),
            }
        })
    }
}

pub static SYMBOL_LINT_RULES: &[&dyn SymbolLintRule] = &[
    &ComplexitySpikeRule,
    &DeepNestingRule,
    &ParameterSpillRule,
    &VerbositySpikeRule,
    &DeadFunctionRule,
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
        for rule in SYMBOL_LINT_RULES {
            if let Some(finding) = rule.check(sym, p, n, graph) {
                findings.push(finding);
            }
        }
    }
    findings
}

/// Serialize lint catalog to canonical format.
pub fn serialize_lint_catalog<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "<!-- ripwire lint-catalog schema=ripwire.lint-catalog/v1: the built-in lint rule registry: <rule name= sev= cat= lang= since=>. -->\n<lintcatalog schema=\"ripwire.lint-catalog/v1\" rules=\"{}\">",
        SYMBOL_LINT_RULES.len()
    )?;
    for rule in SYMBOL_LINT_RULES {
        let r = rule.desc();
        writeln!(
            out,
            "  <rule name=\"{}\" sev=\"{}\" cat=\"{}\" lang=\"all\" since=\"v0.1.0\">{}</rule>",
            r.name, r.severity, r.category, r.description
        )?;
    }
    writeln!(out, "</lintcatalog>")
}

pub struct LintReport<'a> {
    pub findings: &'a [LintFinding],
}

impl crate::serialize::XmlReport for LintReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "lint"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.lint/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire lint schema=ripwire.lint/v1: AST-only checks, facts not gates: <rule name= count= shown_rows= rows_capped= count_capped=> of <f rule= p= in=>.",
        )
    }
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let pw = crate::serialize::PageWindow::compute(self.findings.len(), 100, None, None);
        write!(
            w,
            " findings=\"{}\" shown=\"{}\" capped=\"{}\" inert_rules=\"24\"",
            pw.total,
            pw.shown,
            usize::from(pw.capped)
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let shown = self.findings.len().min(100);
        for rule in SYMBOL_LINT_RULES {
            let r = rule.desc();
            let r_cnt = self.findings.iter().filter(|f| f.rule == r.name).count();
            let r_shown = r_cnt.min(20);
            let r_cap = usize::from(r_shown < r_cnt);
            writeln!(
                w,
                "  <rule name=\"{}\" count=\"{r_cnt}\" shown_rows=\"{r_shown}\" rows_capped=\"{r_cap}\" applicable=\"1\"/>",
                r.name
            )?;
        }
        for f in self.findings.iter().take(shown) {
            let p_esc = crate::serialize::escape::escape_xml_str(&f.path);
            writeln!(
                w,
                "  <f rule=\"{}\" p=\"{p_esc}:{}\" sev=\"{}\" in=\"main\">{}</f>",
                f.rule, f.line, f.severity, f.message
            )?;
        }
        Ok(())
    }
}

/// Serialize lint scan findings to canonical XML format.
pub fn serialize_lint_xml<W: Write>(
    root: &str,
    findings: &[LintFinding],
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    LintReport { findings }.write_xml_report(root, out)
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
