use super::lint::{LINT_RULES, LintFinding, LintScan};
use crate::serialize::{PageRequest, XmlEscapeExt};
use std::io::{self, Write};

const DEFAULT_BYTE_BUDGET: usize = 100_000;

fn default_cap(findings: &[LintFinding]) -> usize {
    let mut bytes = 0;
    findings
        .iter()
        .take_while(|f| {
            bytes += f.path.len() + f.rule.len() + f.enclosing.len() + f.message.len() + 80;
            bytes <= DEFAULT_BYTE_BUDGET
        })
        .count()
}

fn selection_attrs(scan: &LintScan) -> String {
    if scan.options.select.is_empty() && scan.options.ignore.is_empty() {
        return String::new();
    }
    let mut attrs = format!(" selected=\"{} of {}\"", scan.stats.len(), LINT_RULES.len());
    for (name, values) in [
        ("select", &scan.options.select),
        ("ignore", &scan.options.ignore),
    ] {
        if !values.is_empty() {
            attrs.push_str(&format!(" {name}=\"{}\"", values.join(",").xml_escaped()));
        }
    }
    attrs
}

fn count_attrs(scan: &LintScan) -> String {
    let capped: Vec<&str> = scan
        .stats
        .iter()
        .filter(|s| s.capped)
        .map(|s| s.rule.desc.name)
        .collect();
    let mut attrs = String::new();
    if !capped.is_empty() {
        attrs.push_str(&format!(" counts_floor=\"1\" findings_capped=\"1\" findings_next=\"--lint --lint-select={} --lint-max-per-rule=50000\"", capped.join(",")));
    }
    let inert = scan.stats.iter().filter(|s| !s.applicable).count();
    if inert > 0 {
        attrs.push_str(&format!(" inert_rules=\"{inert}\""));
    }
    attrs
}

pub fn serialize_lint_xml<W: Write>(
    root: &str,
    scan: &LintScan,
    page: PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let cap = if page.limit.is_some() || page.offset.is_some() {
        0
    } else {
        default_cap(&scan.findings)
    };
    let window = page.window(scan.findings.len(), cap);
    let shown = &scan.findings[window.begin..window.end];
    writeln!(
        out,
        "<!-- ripwire-rs lint schema=ripwire-rs.lint/v1: AST-only checks, facts not gates: <rule name= count= shown_rows= rows_capped= count_capped=> of <f rule= p= in=>. window: shown= capped= total= has_more= next_offset= offset= limit=. rows_capped=: 1 = cut. root=: p= relative to it. findings=N: findings over the printed rules; a floor when findings_capped=1. -->"
    )?;
    writeln!(
        out,
        "<lint schema=\"ripwire-rs.lint/v1\" findings=\"{}\"{}{}{} root=\"{}\">",
        scan.findings.len(),
        window.xml_attrs(),
        selection_attrs(scan),
        count_attrs(scan),
        root.xml_escaped()
    )?;
    for stat in &scan.stats {
        let count = shown
            .iter()
            .filter(|f| f.rule == stat.rule.desc.name)
            .count();
        let capped = if stat.capped {
            " count_capped=\"1\""
        } else {
            ""
        };
        let applicable = if stat.applicable {
            ""
        } else {
            " applicable=\"0\""
        };
        writeln!(
            out,
            "  <rule name=\"{}\" count=\"{}\" shown_rows=\"{count}\" rows_capped=\"{}\"{capped}{applicable}/>",
            stat.rule.desc.name,
            stat.count,
            usize::from(count < stat.count)
        )?;
    }
    for f in shown {
        writeln!(
            out,
            "  <f rule=\"{}\" p=\"{}:{}\" in=\"{}\">{}</f>",
            f.rule,
            f.path.xml_escaped(),
            f.line,
            f.enclosing.xml_escaped(),
            f.message.xml_escaped()
        )?;
    }
    writeln!(out, "</lint>")
}

pub fn serialize_lint_sarif<W: Write>(scan: &LintScan, out: &mut W) -> io::Result<()> {
    let rules: Vec<_> = scan
        .stats
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.rule.desc.name,
                "shortDescription": {"text": s.rule.desc.description},
                "defaultConfiguration": {"level": match s.rule.desc.severity {
                    "info" => "note",
                    "error" => "error",
                    _ => "warning",
                }},
            })
        })
        .collect();
    let results: Vec<_> = scan.findings.iter().map(|f| serde_json::json!({
        "ruleId": f.rule, "message": {"text": f.message},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": f.path}, "region": {"startLine": f.line}}}]
    })).collect();
    serde_json::to_writer_pretty(out, &serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json", "version": "2.1.0",
        "runs": [{"tool": {"driver": {"name": "ripwire-rs", "rules": rules}}, "results": results}]
    })).map_err(io::Error::other)
}
