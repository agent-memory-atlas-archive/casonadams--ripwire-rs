use crate::domain::{IngestResult, Interner, Symbol};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LintRuleDesc {
    pub name: &'static str,
    pub severity: &'static str,
    pub category: &'static str,
    pub description: &'static str,
}

macro_rules! rule_desc {
    ($name:expr, $sev:expr, $cat:expr, $desc:expr) => {
        LintRuleDesc {
            name: $name,
            severity: $sev,
            category: $cat,
            description: $desc,
        }
    };
}

pub const LINT_RULES: &[LintRuleDesc] = &[
    rule_desc!(
        "complexity-spike",
        "major",
        "maintainability",
        "Cognitive complexity exceeds the Sonar Campbell ceiling of 15"
    ),
    rule_desc!(
        "deep-nesting",
        "major",
        "maintainability",
        "Maximum control statement nesting depth exceeds 5 levels"
    ),
    rule_desc!(
        "parameter-spill",
        "minor",
        "design",
        "Parameter count exceeds 5 positional arguments"
    ),
    rule_desc!(
        "verbosity-spike",
        "minor",
        "readability",
        "Function length exceeds 100 non-comment lines of code"
    ),
    rule_desc!(
        "dead-function",
        "major",
        "architecture",
        "Internal linkage function with zero callers in workspace graph"
    ),
    rule_desc!(
        "large-function",
        "info",
        "maintainability",
        "a function body over 80 lines"
    ),
    rule_desc!(
        "naming-short",
        "info",
        "naming",
        "a 1-2 letter Function/Method/Var name — visible far beyond any tiny scope"
    ),
    rule_desc!(
        "naming-wordy",
        "info",
        "naming",
        "more than 5 split tokens in one name"
    ),
    rule_desc!(
        "naming-underscore",
        "info",
        "naming",
        "internal consecutive underscores, or a C-family reserved __x/_X form"
    ),
    rule_desc!(
        "naming-case",
        "info",
        "naming",
        "snake_case and camelCase mixed inside one name"
    ),
    rule_desc!(
        "c-style-cast",
        "warn",
        "style",
        "a C-style cast — prefers explicit static_cast/const_cast/reinterpret_cast"
    ),
    rule_desc!(
        "goto",
        "warn",
        "control-flow",
        "a goto statement — cppcoreguidelines-avoid-goto"
    ),
    rule_desc!("do-while", "info", "style", "a do/while loop shape"),
    rule_desc!(
        "unsafe-c-fn",
        "error",
        "security",
        "a call to an unbounded C string function (strcpy/strcat/sprintf/gets)"
    ),
    rule_desc!(
        "weak-crypto",
        "error",
        "security",
        "a call to a broken hash or cipher (MD5/SHA1/MD4/RC4)"
    ),
    rule_desc!(
        "redundant-parens",
        "info",
        "style",
        "a doubly-parenthesized expression — readability-redundant-parentheses"
    ),
    rule_desc!(
        "suspicious-semicolon",
        "warn",
        "correctness",
        "an if-body that is just `;` — bugprone-suspicious-semicolon"
    ),
    rule_desc!(
        "typedef-over-using",
        "info",
        "style",
        "a C-style typedef struct/union where `using` is preferred"
    ),
    rule_desc!(
        "magic-number",
        "info",
        "maintainability",
        "a non-trivial numeric literal inside a function body"
    ),
    rule_desc!(
        "empty-catch",
        "warn",
        "error-masking",
        "a catch block with an empty body"
    ),
    rule_desc!(
        "self-assign",
        "warn",
        "correctness",
        "x = x — almost always a copy-paste bug"
    ),
    rule_desc!(
        "inconsistent-return",
        "info",
        "maintainability",
        "a bare `return;` mixed with a value-returning return in the same function"
    ),
    rule_desc!(
        "unreachable-code",
        "warn",
        "correctness",
        "a statement after an unconditional return/break/continue/throw/raise"
    ),
    rule_desc!(
        "naming-series",
        "info",
        "naming",
        "foo1/foo2/... digit-suffix siblings sharing a base name in one scope"
    ),
    rule_desc!(
        "naming-predicate",
        "info",
        "naming",
        "an is/has/can/should/was-prefixed name whose return type is not bool-like"
    ),
    rule_desc!(
        "naming-setter",
        "info",
        "naming",
        "a set-prefixed name whose return type is not void-like"
    ),
    rule_desc!(
        "naming-confusable",
        "info",
        "naming",
        "a co-visible pair within edit distance <=2 or reordered tokens"
    ),
    rule_desc!(
        "naming-uninformative",
        "info",
        "naming",
        "every split subtoken is corpus-ubiquitous (BM25 idf) on a body past size floor"
    ),
    rule_desc!(
        "atom-comma-operator",
        "info",
        "readability",
        "the comma operator inside an expression [Gopstein FSE 2017]"
    ),
    rule_desc!(
        "atom-embedded-crement",
        "info",
        "readability",
        "++/-- evaluated inside a larger expression [Gopstein FSE 2017]"
    ),
    rule_desc!(
        "atom-assign-as-value",
        "info",
        "readability",
        "an assignment whose VALUE is consumed [Gopstein FSE 2017]"
    ),
    rule_desc!(
        "atom-nested-ternary",
        "info",
        "readability",
        "a conditional expression inside a conditional expression [Gopstein FSE 2017]"
    ),
    rule_desc!(
        "atom-implicit-predicate",
        "info",
        "readability",
        "arithmetic used where a truth value is expected [Gopstein FSE 2017]"
    ),
    rule_desc!(
        "atom-octal-literal",
        "info",
        "readability",
        "a leading-zero integer literal — 0755 is 493, not 755 [Gopstein FSE 2017]"
    ),
    rule_desc!(
        "atom-reversed-subscript",
        "info",
        "readability",
        "1[arr] — legal C, almost never intended [Gopstein FSE 2017]"
    ),
    rule_desc!(
        "cache-node-container",
        "info",
        "performance",
        "a node-based std container (map/list/set/...) — one heap node per element"
    ),
    rule_desc!(
        "cache-vector-of-raw-ptr",
        "info",
        "performance",
        "vector<T*> — contiguous handles, scattered payloads"
    ),
    rule_desc!(
        "cache-vector-of-indirect",
        "info",
        "performance",
        "vector<unique_ptr/shared_ptr/vector<...>> — an indirection per element"
    ),
    rule_desc!(
        "cache-heap-alloc-in-loop",
        "info",
        "performance",
        "new/malloc/calloc/realloc/strdup inside a loop body"
    ),
    rule_desc!(
        "cache-pointer-chase-loop",
        "info",
        "performance",
        "p = p->next inside a loop — a serial dependent-load chain"
    ),
    rule_desc!(
        "cache-gather-subscript",
        "info",
        "performance",
        "a[b[i]] inside a loop — gather/scatter random access"
    ),
    rule_desc!(
        "cache-shared-ptr-by-value",
        "info",
        "performance",
        "a by-value shared_ptr parameter — atomic refcount traffic per call"
    ),
    rule_desc!(
        "cache-manual-prefetch",
        "info",
        "performance",
        "an existing _mm_prefetch/__builtin_prefetch call"
    ),
];

/// Trait encapsulating a single AST symbol lint rule and its catalog metadata.
pub trait SymbolLintRule: Sync {
    fn desc(&self) -> LintRuleDesc;
    fn check(&self, sym: &Symbol, file: &str, name: &str, graph: &Graph) -> Option<LintFinding>;
}

pub struct SymbolLintSpec {
    pub desc: LintRuleDesc,
    pub check_fn: fn(&Symbol, &str, &str, &Graph) -> Option<String>,
}

impl SymbolLintRule for SymbolLintSpec {
    fn desc(&self) -> LintRuleDesc {
        self.desc
    }
    fn check(&self, sym: &Symbol, file: &str, name: &str, graph: &Graph) -> Option<LintFinding> {
        let message = (self.check_fn)(sym, file, name, graph)?;
        Some(LintFinding {
            rule: self.desc.name.to_string(),
            message,
            path: file.to_string(),
            line: sym.line,
            severity: self.desc.severity.to_string(),
            enclosing: name.to_string(),
        })
    }
}

pub static SYMBOL_LINT_RULES: &[&dyn SymbolLintRule] = &[
    &SymbolLintSpec {
        desc: LINT_RULES[0],
        check_fn: |sym, _, name, _| {
            (sym.ccx > 15).then(|| {
                format!(
                    "Function '{name}' cognitive complexity {} exceeds 15",
                    sym.ccx
                )
            })
        },
    },
    &SymbolLintSpec {
        desc: LINT_RULES[1],
        check_fn: |sym, _, name, _| {
            (sym.max_nest > 5)
                .then(|| format!("Function '{name}' max nesting {} exceeds 5", sym.max_nest))
        },
    },
    &SymbolLintSpec {
        desc: LINT_RULES[2],
        check_fn: |sym, _, name, _| {
            (sym.params > 5).then(|| {
                format!(
                    "Function '{name}' has {} parameters, exceeding 5",
                    sym.params
                )
            })
        },
    },
    &SymbolLintSpec {
        desc: LINT_RULES[3],
        check_fn: |sym, _, name, _| {
            (sym.loc > 100).then(|| format!("Function '{name}' length {} exceeds 100 LOC", sym.loc))
        },
    },
    &SymbolLintSpec {
        desc: LINT_RULES[4],
        check_fn: |sym, file, name, graph| {
            (graph.in_degree(sym.id) == 0 && !crate::gate::affected::is_test_path(file))
                .then(|| format!("Internal function '{name}' has zero incoming callers"))
        },
    },
    &SymbolLintSpec {
        desc: LINT_RULES[5],
        check_fn: |sym, _, name, _| {
            (sym.loc > 80).then(|| format!("Function '{name}' body is {} lines (> 80)", sym.loc))
        },
    },
    &SymbolLintSpec {
        desc: LINT_RULES[6],
        check_fn: |_, _, name, _| {
            let short = name.rsplit("::").next().unwrap_or(name);
            (!short.is_empty() && short.len() <= 2 && short.chars().all(char::is_alphabetic))
                .then(|| format!("Function '{short}' has a 1-2 character name"))
        },
    },
    &SymbolLintSpec {
        desc: LINT_RULES[7],
        check_fn: |_, _, name, _| {
            let short = name.rsplit("::").next().unwrap_or(name);
            let words = short.split('_').filter(|s| !s.is_empty()).count();
            (words > 5).then(|| format!("Function '{short}' has {words} subtokens (> 5)"))
        },
    },
    &SymbolLintSpec {
        desc: LINT_RULES[8],
        check_fn: |_, _, name, _| {
            let short = name.rsplit("::").next().unwrap_or(name);
            short
                .contains("__")
                .then(|| format!("Function '{short}' contains consecutive underscores"))
        },
    },
    &SymbolLintSpec {
        desc: LINT_RULES[9],
        check_fn: |_, _, name, _| {
            let short = name.rsplit("::").next().unwrap_or(name);
            let mixed = short.contains('_')
                && short.chars().any(char::is_lowercase)
                && short.chars().any(char::is_uppercase);
            mixed.then(|| format!("Function '{short}' mixes snake_case and camelCase"))
        },
    },
];

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
        "<!-- ripwire-rs lint-catalog schema=ripwire-rs.lint-catalog/v1: the built-in lint rule registry: <rule name= sev= cat= lang= since=>. -->\n<lintcatalog schema=\"ripwire-rs.lint-catalog/v1\" rules=\"{}\">",
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
        "ripwire-rs.lint/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs lint schema=ripwire-rs.lint/v1: AST-only checks, facts not gates: <rule name= count= shown_rows= rows_capped= count_capped=> of <f rule= p= in=>.",
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
