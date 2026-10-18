use crate::domain::{IngestResult, Interner};
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
    pub enclosing: String,
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
    LintRuleDesc {
        name: "large-function",
        severity: "info",
        category: "maintainability",
        description: "a function body over 80 lines",
    },
    LintRuleDesc {
        name: "naming-short",
        severity: "info",
        category: "naming",
        description: "a 1-2 letter Function/Method/Var name — visible far beyond any tiny scope",
    },
    LintRuleDesc {
        name: "naming-wordy",
        severity: "info",
        category: "naming",
        description: "more than 5 split tokens in one name",
    },
    LintRuleDesc {
        name: "naming-underscore",
        severity: "info",
        category: "naming",
        description: "internal consecutive underscores, or a C-family reserved __x/_X form",
    },
    LintRuleDesc {
        name: "naming-case",
        severity: "info",
        category: "naming",
        description: "snake_case and camelCase mixed inside one name",
    },
    LintRuleDesc {
        name: "c-style-cast",
        severity: "warn",
        category: "style",
        description: "a C-style cast — prefers explicit static_cast/const_cast/reinterpret_cast",
    },
    LintRuleDesc {
        name: "goto",
        severity: "warn",
        category: "control-flow",
        description: "a goto statement — cppcoreguidelines-avoid-goto",
    },
    LintRuleDesc {
        name: "do-while",
        severity: "info",
        category: "style",
        description: "a do/while loop shape",
    },
    LintRuleDesc {
        name: "unsafe-c-fn",
        severity: "error",
        category: "security",
        description: "a call to an unbounded C string function (strcpy/strcat/sprintf/gets)",
    },
    LintRuleDesc {
        name: "weak-crypto",
        severity: "error",
        category: "security",
        description: "a call to a broken hash or cipher (MD5/SHA1/MD4/RC4)",
    },
    LintRuleDesc {
        name: "redundant-parens",
        severity: "info",
        category: "style",
        description: "a doubly-parenthesized expression — readability-redundant-parentheses",
    },
    LintRuleDesc {
        name: "suspicious-semicolon",
        severity: "warn",
        category: "correctness",
        description: "an if-body that is just `;` — bugprone-suspicious-semicolon",
    },
    LintRuleDesc {
        name: "typedef-over-using",
        severity: "info",
        category: "style",
        description: "a C-style typedef struct/union where `using` is preferred",
    },
    LintRuleDesc {
        name: "magic-number",
        severity: "info",
        category: "maintainability",
        description: "a non-trivial numeric literal inside a function body",
    },
    LintRuleDesc {
        name: "empty-catch",
        severity: "warn",
        category: "error-masking",
        description: "a catch block with an empty body",
    },
    LintRuleDesc {
        name: "self-assign",
        severity: "warn",
        category: "correctness",
        description: "x = x — almost always a copy-paste bug",
    },
    LintRuleDesc {
        name: "inconsistent-return",
        severity: "info",
        category: "maintainability",
        description: "a bare `return;` mixed with a value-returning return in the same function",
    },
    LintRuleDesc {
        name: "unreachable-code",
        severity: "warn",
        category: "correctness",
        description: "a statement after an unconditional return/break/continue/throw/raise",
    },
    LintRuleDesc {
        name: "naming-series",
        severity: "info",
        category: "naming",
        description: "foo1/foo2/... digit-suffix siblings sharing a base name in one scope",
    },
    LintRuleDesc {
        name: "naming-predicate",
        severity: "info",
        category: "naming",
        description: "an is/has/can/should/was-prefixed name whose return type is not bool-like",
    },
    LintRuleDesc {
        name: "naming-setter",
        severity: "info",
        category: "naming",
        description: "a set-prefixed name whose return type is not void-like",
    },
    LintRuleDesc {
        name: "naming-confusable",
        severity: "info",
        category: "naming",
        description: "a co-visible pair within edit distance <=2 or reordered tokens",
    },
    LintRuleDesc {
        name: "naming-uninformative",
        severity: "info",
        category: "naming",
        description: "every split subtoken is corpus-ubiquitous (BM25 idf) on a body past size floor",
    },
    LintRuleDesc {
        name: "atom-comma-operator",
        severity: "info",
        category: "readability",
        description: "the comma operator inside an expression [Gopstein FSE 2017]",
    },
    LintRuleDesc {
        name: "atom-embedded-crement",
        severity: "info",
        category: "readability",
        description: "++/-- evaluated inside a larger expression [Gopstein FSE 2017]",
    },
    LintRuleDesc {
        name: "atom-assign-as-value",
        severity: "info",
        category: "readability",
        description: "an assignment whose VALUE is consumed [Gopstein FSE 2017]",
    },
    LintRuleDesc {
        name: "atom-nested-ternary",
        severity: "info",
        category: "readability",
        description: "a conditional expression inside a conditional expression [Gopstein FSE 2017]",
    },
    LintRuleDesc {
        name: "atom-implicit-predicate",
        severity: "info",
        category: "readability",
        description: "arithmetic used where a truth value is expected [Gopstein FSE 2017]",
    },
    LintRuleDesc {
        name: "atom-octal-literal",
        severity: "info",
        category: "readability",
        description: "a leading-zero integer literal — 0755 is 493, not 755 [Gopstein FSE 2017]",
    },
    LintRuleDesc {
        name: "atom-reversed-subscript",
        severity: "info",
        category: "readability",
        description: "1[arr] — legal C, almost never intended [Gopstein FSE 2017]",
    },
    LintRuleDesc {
        name: "cache-node-container",
        severity: "info",
        category: "performance",
        description: "a node-based std container (map/list/set/...) — one heap node per element",
    },
    LintRuleDesc {
        name: "cache-vector-of-raw-ptr",
        severity: "info",
        category: "performance",
        description: "vector<T*> — contiguous handles, scattered payloads",
    },
    LintRuleDesc {
        name: "cache-vector-of-indirect",
        severity: "info",
        category: "performance",
        description: "vector<unique_ptr/shared_ptr/vector<...>> — an indirection per element",
    },
    LintRuleDesc {
        name: "cache-heap-alloc-in-loop",
        severity: "info",
        category: "performance",
        description: "new/malloc/calloc/realloc/strdup inside a loop body",
    },
    LintRuleDesc {
        name: "cache-pointer-chase-loop",
        severity: "info",
        category: "performance",
        description: "p = p->next inside a loop — a serial dependent-load chain",
    },
    LintRuleDesc {
        name: "cache-gather-subscript",
        severity: "info",
        category: "performance",
        description: "a[b[i]] inside a loop — gather/scatter random access",
    },
    LintRuleDesc {
        name: "cache-shared-ptr-by-value",
        severity: "info",
        category: "performance",
        description: "a by-value shared_ptr parameter — atomic refcount traffic per call",
    },
    LintRuleDesc {
        name: "cache-manual-prefetch",
        severity: "info",
        category: "performance",
        description: "an existing _mm_prefetch/__builtin_prefetch call",
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
            enclosing: name.to_string(),
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
            enclosing: name.to_string(),
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
            enclosing: name.to_string(),
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
            enclosing: name.to_string(),
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
                enclosing: name.to_string(),
            }
        })
    }
}

struct LargeFunctionRule;
impl SymbolLintRule for LargeFunctionRule {
    fn desc(&self) -> LintRuleDesc {
        LINT_RULES[5].clone()
    }
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        _graph: &Graph,
    ) -> Option<LintFinding> {
        (sym.loc > 80).then(|| LintFinding {
            rule: "large-function".to_string(),
            message: format!("Function '{name}' body is {} lines (> 80)", sym.loc),
            path: file.to_string(),
            line: sym.line,
            severity: "info".to_string(),
            enclosing: name.to_string(),
        })
    }
}

struct NamingShortRule;
impl SymbolLintRule for NamingShortRule {
    fn desc(&self) -> LintRuleDesc {
        LINT_RULES[6].clone()
    }
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        _graph: &Graph,
    ) -> Option<LintFinding> {
        let short = name.rsplit("::").next().unwrap_or(name);
        (!short.is_empty() && short.len() <= 2 && short.chars().all(char::is_alphabetic)).then(
            || LintFinding {
                rule: "naming-short".to_string(),
                message: format!("Function '{short}' has a 1-2 character name"),
                path: file.to_string(),
                line: sym.line,
                severity: "info".to_string(),
                enclosing: name.to_string(),
            },
        )
    }
}

struct NamingWordyRule;
impl SymbolLintRule for NamingWordyRule {
    fn desc(&self) -> LintRuleDesc {
        LINT_RULES[7].clone()
    }
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        _graph: &Graph,
    ) -> Option<LintFinding> {
        let short = name.rsplit("::").next().unwrap_or(name);
        let words = short.split('_').filter(|s| !s.is_empty()).count();
        (words > 5).then(|| LintFinding {
            rule: "naming-wordy".to_string(),
            message: format!("Function '{short}' has {words} subtokens (> 5)"),
            path: file.to_string(),
            line: sym.line,
            severity: "info".to_string(),
            enclosing: name.to_string(),
        })
    }
}

struct NamingUnderscoreRule;
impl SymbolLintRule for NamingUnderscoreRule {
    fn desc(&self) -> LintRuleDesc {
        LINT_RULES[8].clone()
    }
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        _graph: &Graph,
    ) -> Option<LintFinding> {
        let short = name.rsplit("::").next().unwrap_or(name);
        short.contains("__").then(|| LintFinding {
            rule: "naming-underscore".to_string(),
            message: format!("Function '{short}' contains consecutive underscores"),
            path: file.to_string(),
            line: sym.line,
            severity: "info".to_string(),
            enclosing: name.to_string(),
        })
    }
}

struct NamingCaseRule;
impl SymbolLintRule for NamingCaseRule {
    fn desc(&self) -> LintRuleDesc {
        LINT_RULES[9].clone()
    }
    fn check(
        &self,
        sym: &crate::domain::Symbol,
        file: &str,
        name: &str,
        _graph: &Graph,
    ) -> Option<LintFinding> {
        let short = name.rsplit("::").next().unwrap_or(name);
        let mixed = short.contains('_')
            && short.chars().any(char::is_lowercase)
            && short.chars().any(char::is_uppercase);
        mixed.then(|| LintFinding {
            rule: "naming-case".to_string(),
            message: format!("Function '{short}' mixes snake_case and camelCase"),
            path: file.to_string(),
            line: sym.line,
            severity: "info".to_string(),
            enclosing: name.to_string(),
        })
    }
}

pub static SYMBOL_LINT_RULES: &[&dyn SymbolLintRule] = &[
    &ComplexitySpikeRule,
    &DeepNestingRule,
    &ParameterSpillRule,
    &VerbositySpikeRule,
    &DeadFunctionRule,
    &LargeFunctionRule,
    &NamingShortRule,
    &NamingWordyRule,
    &NamingUnderscoreRule,
    &NamingCaseRule,
];

/// Scan the workspace for AST code smells across all symbols.
#[must_use]
pub fn run_lint_scan(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> Vec<LintFinding> {
    run_lint_scan_in_root(".", ingest, graph, interner)
}

pub fn scan_file_rules(
    path: &str,
    raw_content: &str,
    lang: crate::domain::Lang,
    syms: Vec<super::lint_rules::SymSpan<'_>>,
    findings: &mut Vec<LintFinding>,
) {
    let masked = super::source_mask::mask_source(raw_content, lang);
    let ctx = super::lint_rules::FileLintContext::new(path, lang, raw_content, &masked.code, syms);
    for rule in super::lint_rules::FILE_LINT_RULES {
        rule.check_file(&ctx, findings);
    }
}

fn run_symbol_lint_rules(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    findings: &mut Vec<LintFinding>,
) {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    for sym in &ingest.symbols {
        if sym.is_callable() && !view.is_test_symbol(sym) {
            let p = view.sym_path(sym);
            let n = view.sym_name(sym);
            findings.extend(
                SYMBOL_LINT_RULES
                    .iter()
                    .filter_map(|rule| rule.check(sym, p, n, graph)),
            );
        }
    }
}

#[must_use]
pub fn run_lint_scan_in_root(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    run_symbol_lint_rules(ingest, graph, interner, &mut findings);
    let root_path = std::path::Path::new(root);
    for fi in &ingest.files {
        if !fi.lang.semantics().is_prose_document() {
            let p = interner.resolve(&fi.path);
            if let Ok(content) = crate::domain::read_rel_string(root_path, p) {
                let syms = super::lint_rules::symbols_in_file(ingest, interner, fi.id);
                scan_file_rules(p, &content, fi.lang, syms, &mut findings);
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
        LINT_RULES.len()
    )?;
    for r in LINT_RULES {
        writeln!(
            out,
            "  <rule name=\"{}\" sev=\"{}\" cat=\"{}\" lang=\"all\" since=\"v0.1.0\">{}</rule>",
            r.name,
            r.severity,
            r.category,
            crate::serialize::XmlEscaped(r.description)
        )?;
    }
    writeln!(out, "</lintcatalog>")
}

pub struct LintReport<'a> {
    pub findings: &'a [LintFinding],
}

impl LintReport<'_> {
    fn capped_rule_names(&self) -> Vec<&'static str> {
        if self.findings.len() < 5000 {
            return Vec::new();
        }
        LINT_RULES
            .iter()
            .filter(|r| self.findings.iter().filter(|f| f.rule == r.name).count() >= 500)
            .map(|r| r.name)
            .collect()
    }
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
        let active_rules = LINT_RULES
            .iter()
            .filter(|r| self.findings.iter().any(|f| f.rule == r.name))
            .count();
        let inert_rules = LINT_RULES.len().saturating_sub(active_rules);
        let capped_rules = self.capped_rule_names();
        write!(
            w,
            " findings=\"{}\" shown=\"{}\" capped=\"{}\" inert_rules=\"{inert_rules}\"",
            pw.total,
            pw.shown,
            usize::from(pw.capped)
        )?;
        if !capped_rules.is_empty() {
            write!(
                w,
                " counts_floor=\"1\" findings_capped=\"1\" findings_next=\"--lint --lint-select={} --lint-max-per-rule=50000\"",
                capped_rules.join(",")
            )?;
        }
        Ok(())
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let shown = self.findings.len().min(100);
        let capped_rules = self.capped_rule_names();
        for r in LINT_RULES {
            let r_cnt = self.findings.iter().filter(|f| f.rule == r.name).count();
            if r_cnt == 0 && !SYMBOL_LINT_RULES.iter().any(|sr| sr.desc().name == r.name) {
                continue;
            }
            let r_shown = r_cnt.min(20);
            let r_cap = usize::from(r_shown < r_cnt);
            let count_cap_attr = if capped_rules.contains(&r.name) {
                " count_capped=\"1\""
            } else {
                ""
            };
            writeln!(
                w,
                "  <rule name=\"{}\" count=\"{r_cnt}\" shown_rows=\"{r_shown}\" rows_capped=\"{r_cap}\"{count_cap_attr} applicable=\"1\"/>",
                r.name
            )?;
        }
        for f in self.findings.iter().take(shown) {
            use crate::serialize::XmlEscapeExt;
            writeln!(
                w,
                "  <f rule=\"{}\" p=\"{}:{}\" sev=\"{}\" in=\"{}\">{}</f>",
                f.rule,
                f.path.xml_escaped(),
                f.line,
                f.severity,
                f.enclosing.xml_escaped(),
                f.message.xml_escaped()
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
