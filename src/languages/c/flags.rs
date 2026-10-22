//! C-family feature flags: `#ifndef` settings gates, `#if` conditionals and CMake options.
use crate::languages::common::flags::{FlagFact, env_facts};
use crate::languages::LanguageSemantics;

/// A C-preprocessor settings gate: `#ifndef NAME` followed by `#define NAME VALUE` (an include guard has no value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsGate {
    pub name: String,
    pub default: String,
    pub dark: bool,
    pub define_idx: usize,
}

fn ifndef_name(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("#ifndef ")?.trim_start();
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let name = &rest[..end];
    (!name.is_empty()).then_some(name)
}

#[must_use]
pub fn extract_settings_gate(lines: &[&str], idx: usize) -> Option<SettingsGate> {
    let name = ifndef_name(lines[idx])?;
    lines
        .iter()
        .enumerate()
        .skip(idx + 1)
        .take(3)
        .find_map(|(define_idx, line)| {
            let mut parts = line.trim().strip_prefix("#define ")?.split_whitespace();
            (parts.next()? == name).then_some(())?;
            let value = parts.next()?;
            Some(SettingsGate {
                name: name.to_string(),
                default: value.to_string(),
                dark: value == "0" || value.eq_ignore_ascii_case("false"),
                define_idx,
            })
        })
}

fn measure_preproc_region_loc(lines: &[&str], start_idx: usize) -> usize {
    let mut depth = 1usize;
    for (offset, &line) in lines.iter().skip(start_idx + 1).enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#if") {
            depth += 1;
        } else if trimmed.starts_with("#endif") {
            depth -= 1;
            if depth == 0 {
                return offset;
            }
        }
    }
    0
}

/// One identifier used in a `#if` / `#ifdef` / `#elif` condition, with the lines its region spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalUse {
    pub name: String,
    pub line_idx: usize,
    pub loc: usize,
}

#[must_use]
pub fn conditional_uses(lines: &[&str]) -> Vec<ConditionalUse> {
    let mut uses = Vec::new();
    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let Some(expr) = ["#if ", "#ifdef ", "#elif "]
            .iter()
            .find_map(|p| trimmed.strip_prefix(p))
        else {
            continue;
        };
        let loc = measure_preproc_region_loc(lines, line_idx);
        uses.extend(
            expr.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|t| {
                    t.starts_with(|c: char| c.is_alphabetic() || c == '_') && *t != "defined"
                })
                .map(|t| ConditionalUse {
                    name: t.to_string(),
                    line_idx,
                    loc,
                }),
        );
    }
    uses
}

pub fn extract_cmake_option(line: &str, path: &str) -> Option<(String, String, bool)> {
    if !path.ends_with(".cmake") && !path.ends_with("CMakeLists.txt") {
        return None;
    }
    let rest = line.trim().strip_prefix("option(")?;
    let inner = rest.trim_end_matches(')').trim();
    let name: String = inner
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    let is_on = inner
        .split_whitespace()
        .last()
        .is_some_and(|t| t.eq_ignore_ascii_case("ON"));
    let def_val = if is_on { "ON" } else { "OFF" };
    Some((name, def_val.to_string(), !is_on))
}

/// Flag facts of a C-family source file or of the CMake manifests that configure its build.
pub fn facts(lang: &impl LanguageSemantics, lines: &[&str], path: &str) -> Vec<FlagFact> {
    if path.ends_with(".cmake") || path.ends_with("CMakeLists.txt") {
        return lines
            .iter()
            .enumerate()
            .filter_map(|(line, text)| {
                let (name, default, dark) = extract_cmake_option(text, path)?;
                Some(FlagFact::Option { kind: "cmake", name, default, dark, line })
            })
            .collect();
    }
    let mut facts = env_facts(lines, lang.uses_hash_comments(), lang.has_lifetime_tick_syntax(), lang.env_reads());
    for line in 0..lines.len() {
        if let Some(gate) = extract_settings_gate(lines, line) {
            facts.push(FlagFact::Setting {
                name: gate.name,
                default: gate.default,
                dark: gate.dark,
                line,
                define_line: gate.define_idx,
            });
        }
    }
    facts.extend(conditional_uses(lines).into_iter().map(|u| FlagFact::Conditional {
        name: u.name,
        line: u.line_idx,
        loc: u.loc,
    }));
    facts
}

/// The hooks every C-family plugin shares: flag harvesting, header files and reserved identifiers.
macro_rules! c_family_hooks {
    () => {
        fn env_reads(&self) -> &'static [$crate::languages::common::flags::EnvRead] {
            &[$crate::languages::common::flags::EnvRead::Call("getenv")]
        }

        fn flag_facts(&self, lines: &[&str], path: &str) -> Vec<$crate::languages::common::flags::FlagFact> {
            $crate::languages::c::flags::facts(self, lines, path)
        }

        fn is_header_file(&self, path: &str) -> bool {
            matches!(
                std::path::Path::new(path).extension().and_then(|e| e.to_str()),
                Some("h" | "hpp" | "hh" | "hxx" | "cuh")
            )
        }

        fn is_reserved_identifier(&self, name: &str) -> bool {
            name.starts_with("__")
                || name.strip_prefix('_').is_some_and(|rest| rest.starts_with(char::is_uppercase))
        }
    };
}
pub(crate) use c_family_hooks;
