use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use std::io::{self, Write};

pub const DEFAULT_ARCH_BASELINE: &str = ".ripwire_arch_baseline";

/// A declared architectural rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchRule {
    Deny { from: String, to: String },
    Allow { from: String, to: String },
}

/// A parsed architecture rules configuration.
#[derive(Debug, Clone, Default)]
pub struct ArchConfig {
    pub layers: Vec<(String, Vec<String>)>, // (layer_name, [substr1, substr2...])
    pub rules: Vec<ArchRule>,
}

/// A detected layering rule violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchViolation {
    pub from_layer: String,
    pub to_layer: String,
    pub from_file: String,
    pub to_file: String,
    pub caller: String,
    pub callee: String,
}

impl ArchConfig {
    pub fn parse(content: &str) -> Self {
        let mut layers = Vec::new();
        let mut rules = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            if tokens.len() >= 3 && tokens[0].eq_ignore_ascii_case("layer") && tokens[2] == "=" {
                let name = tokens[1].to_string();
                let substrs = tokens[3..].iter().map(|s| s.to_string()).collect();
                layers.push((name, substrs));
            } else if tokens.len() >= 4
                && (tokens[0].eq_ignore_ascii_case("deny")
                    || tokens[0].eq_ignore_ascii_case("allow"))
                && tokens[2] == "->"
            {
                let from = tokens[1].to_string();
                let to = tokens[3].to_string();
                if tokens[0].eq_ignore_ascii_case("deny") {
                    rules.push(ArchRule::Deny { from, to });
                } else {
                    rules.push(ArchRule::Allow { from, to });
                }
            }
        }
        Self { layers, rules }
    }

    pub fn assign_layer(&self, file_path: &str) -> Option<&str> {
        for (name, substrs) in &self.layers {
            for sub in substrs {
                if matches_layer_pattern(file_path, sub) {
                    return Some(name.as_str());
                }
            }
        }
        None
    }
}

fn matches_layer_pattern(file_path: &str, pat: &str) -> bool {
    if let Some(ext) = pat.strip_prefix("*.") {
        return file_path.ends_with(&format!(".{ext}"));
    }
    let cleaned = pat
        .trim_start_matches("**/")
        .trim_start_matches("./")
        .trim_end_matches("/**")
        .trim_end_matches("/*")
        .trim_end_matches('*');
    !cleaned.is_empty() && file_path.contains(cleaned)
}
struct EdgeEndpoint<'a> {
    file: &'a str,
    layer: &'a str,
    sym: &'a str,
}
#[inline]
fn layer_matches(pattern: &str, layer: &str) -> bool {
    pattern == "*" || pattern == layer
}

fn check_edge_violation(
    from: EdgeEndpoint<'_>,
    to: EdgeEndpoint<'_>,
    rules: &[ArchRule],
    violations: &mut Vec<ArchViolation>,
) {
    if from.layer == to.layer {
        return;
    }

    for rule in rules {
        if let ArchRule::Deny {
            from: r_from,
            to: r_to,
        } = rule
            && layer_matches(r_from, from.layer)
            && layer_matches(r_to, to.layer)
        {
            violations.push(ArchViolation {
                from_layer: from.layer.to_string(),
                to_layer: to.layer.to_string(),
                from_file: from.file.to_string(),
                to_file: to.file.to_string(),
                caller: from.sym.to_string(),
                callee: to.sym.to_string(),
            });
        }
    }
}
/// Evaluate architectural layering rules against the indexed call graph.
#[must_use]
pub fn check_architecture_rules(
    cfg: &ArchConfig,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> Vec<ArchViolation> {
    let mut violations = Vec::new();

    for sym in &ingest.symbols {
        let from_file = ingest.sym_file_path(sym, interner);
        let Some(from_layer) = cfg.assign_layer(from_file) else {
            continue;
        };

        for (target_id, _) in graph.callees(sym.id) {
            let Some(target) = ingest.sym(target_id) else {
                continue;
            };
            let to_file = ingest.sym_file_path(target, interner);
            let Some(to_layer) = cfg.assign_layer(to_file) else {
                continue;
            };

            let from_ep = EdgeEndpoint {
                file: from_file,
                layer: from_layer,
                sym: &interner[sym.name],
            };
            let to_ep = EdgeEndpoint {
                file: to_file,
                layer: to_layer,
                sym: &interner[target.name],
            };
            check_edge_violation(from_ep, to_ep, &cfg.rules, &mut violations);
        }
    }
    violations
}

pub struct ArchReport<'a> {
    pub violations: &'a [ArchViolation],
}

impl crate::serialize::XmlReport for ArchReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "arch"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.arch/v1"
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        write!(out, " violations=\"{}\"", self.violations.len())
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for v in self.violations {
            writeln!(
                out,
                "  <violation from=\"{}\" to=\"{}\" p=\"{}\" tp=\"{}\" caller=\"{}\" callee=\"{}\"/>",
                v.from_layer, v.to_layer, v.from_file, v.to_file, v.caller, v.callee
            )?;
        }
        Ok(())
    }
}

/// Serialize architecture violation report to canonical XML format.
pub fn serialize_arch_xml<W: Write>(
    root: &str,
    violations: &[ArchViolation],
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    ArchReport { violations }.write_xml_report(root, out)
}
