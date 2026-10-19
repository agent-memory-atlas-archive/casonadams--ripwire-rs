use super::lint::{LINT_RULES, LintFinding, LintRuleDesc};
use crate::domain::{IngestResult, Interner, Lang, SymKind, Symbol};

pub type SymSpan<'a> = (&'a Symbol, &'a str);

/// Precomputed per-file context passed to every `FileLintRule`.
pub struct FileLintContext<'a> {
    pub path: &'a str,
    pub lang: Lang,
    pub raw: &'a str,
    pub code: &'a str,
    pub code_lines: Vec<(&'a str, &'a str)>,
    pub syms: Vec<SymSpan<'a>>,
    pub loop_lines: Vec<bool>,
}

impl<'a> FileLintContext<'a> {
    #[must_use]
    pub fn new(
        path: &'a str,
        lang: Lang,
        raw: &'a str,
        code: &'a str,
        syms: Vec<SymSpan<'a>>,
    ) -> Self {
        let mut loop_lines = Vec::new();
        let mut loop_brace_depth: Option<i32> = None;
        let mut brace_depth: i32 = 0;
        let mut code_lines = Vec::new();
        for line in code.lines() {
            let trimmed = line.trim();
            let opens_loop = trimmed.starts_with("for ")
                || trimmed.starts_with("for(")
                || trimmed.starts_with("while ")
                || trimmed.starts_with("while(");
            if opens_loop && loop_brace_depth.is_none() {
                loop_brace_depth = Some(brace_depth);
            }
            let in_loop = loop_brace_depth.is_some() && !opens_loop;
            loop_lines.push(in_loop);
            code_lines.push((line, trimmed));
            brace_depth += line.bytes().filter(|&b| b == b'{').count() as i32;
            brace_depth -= line.bytes().filter(|&b| b == b'}').count() as i32;
            if loop_brace_depth.is_some_and(|d| brace_depth <= d) {
                loop_brace_depth = None;
            }
        }
        Self {
            path,
            lang,
            raw,
            code,
            code_lines,
            syms,
            loop_lines,
        }
    }

    #[must_use]
    pub fn is_c_family(&self) -> bool {
        self.lang.semantics().dialect_group() == crate::languages::DialectGroup::CFamily
    }

    #[must_use]
    pub fn enclosing_at(&self, line_1based: u32) -> &'a str {
        self.syms
            .iter()
            .filter(|(s, _)| s.line <= line_1based && line_1based <= s.line.saturating_add(s.loc))
            .min_by_key(|(s, _)| s.loc)
            .map_or("<top>", |(_, n)| *n)
    }
}

/// Trait implemented by every file-, scope-, or line-level lint rule.
pub trait FileLintRule: Sync {
    fn desc(&self) -> LintRuleDesc;
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>);

    fn emit(
        &self,
        ctx: &FileLintContext<'_>,
        line: u32,
        enclosing: &str,
        message: &str,
        findings: &mut Vec<LintFinding>,
    ) {
        let d = self.desc();
        findings.push(LintFinding {
            rule: d.name.to_string(),
            message: message.to_string(),
            path: ctx.path.to_string(),
            line,
            severity: d.severity.to_string(),
            enclosing: enclosing.to_string(),
        });
    }
}

pub enum FileLintKind {
    AllLines(fn(usize, &str, &FileLintContext<'_>) -> Option<&'static str>),
    CFamilyLines(fn(usize, &str, &FileLintContext<'_>) -> Option<&'static str>),
    Custom(fn(&FileLintSpec, &FileLintContext<'_>, &mut Vec<LintFinding>)),
}

pub struct FileLintSpec {
    pub desc: LintRuleDesc,
    pub kind: FileLintKind,
}

impl FileLintRule for FileLintSpec {
    fn desc(&self) -> LintRuleDesc {
        self.desc
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        let line_fn = match self.kind {
            FileLintKind::AllLines(f) => f,
            FileLintKind::CFamilyLines(f) => {
                if !ctx.is_c_family() {
                    return;
                }
                f
            }
            FileLintKind::Custom(custom) => {
                custom(self, ctx, findings);
                return;
            }
        };
        for (idx, &(_, trimmed)) in ctx.code_lines.iter().enumerate() {
            if !trimmed.is_empty()
                && let Some(msg) = line_fn(idx, trimmed, ctx)
            {
                let line_no = (idx + 1) as u32;
                self.emit(ctx, line_no, ctx.enclosing_at(line_no), msg, findings);
            }
        }
    }
}

fn strip_digit_suffix(name: &str) -> Option<&str> {
    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    (trimmed.len() < name.len() && !trimmed.is_empty()).then_some(trimmed)
}

fn sig_return_type<'a>(raw: &'a str, sym: &Symbol) -> Option<&'a str> {
    let sig = raw.get(sym.sig_start_byte as usize..sym.sig_end_byte as usize)?;
    let (_, ret) = sig.rsplit_once("->")?;
    let ret = ret.trim().trim_end_matches('{').trim();
    (!ret.is_empty()).then_some(ret)
}

fn edit_distance_one(a: &str, b: &str) -> bool {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    if ab.len() == bb.len() {
        return ab.iter().zip(bb).filter(|(x, y)| x != y).count() == 1;
    }
    let (short, long) = if ab.len() < bb.len() {
        (ab, bb)
    } else {
        (bb, ab)
    };
    if long.len() != short.len() + 1 {
        return false;
    }
    let i = short
        .iter()
        .zip(long)
        .position(|(x, y)| x != y)
        .unwrap_or(short.len());
    short[i..] == long[i + 1..]
}

fn extract_cond(t: &str) -> Option<&str> {
    let head = t.split('{').next()?;
    head.strip_prefix("if (")
        .or_else(|| head.strip_prefix("if("))
        .or_else(|| head.strip_prefix("while ("))
        .or_else(|| head.strip_prefix("while("))?
        .rsplit_once(')')
        .map(|(c, _)| c)
}

const GENERIC_SUBTOKENS: &[&str] = &[
    "get", "set", "do", "run", "handle", "process", "data", "item", "val", "value", "info", "obj",
    "tmp", "fn", "func", "helper", "util",
];

const C_STYLE_CASTS: &[&str] = &[
    "(int)",
    "(unsigned)",
    "(unsigned ",
    "(long)",
    "(long ",
    "(short)",
    "(char)",
    "(char*)",
    "(const char*)",
    "(void*)",
    "(const void*)",
    "(size_t)",
    "(std::size_t)",
    "(uint32_t)",
    "(uint64_t)",
    "(int32_t)",
    "(int64_t)",
    "(double)",
    "(float)",
];

const ALLOWED_NUM_LITERALS: &[&str] = &[
    "10", "12", "16", "20", "24", "32", "40", "48", "60", "64", "80", "100", "120", "128", "200",
    "256", "500", "512", "1000", "1024", "2048", "4096", "8192", "65536",
];

fn check_inconsistent_return(
    rule: &FileLintSpec,
    ctx: &FileLintContext<'_>,
    findings: &mut Vec<LintFinding>,
) {
    for &(sym, name) in &ctx.syms {
        if sym.is_callable()
            && let Some(body) = ctx
                .code
                .get(sym.sig_end_byte as usize..sym.end_byte as usize)
            && body.contains("return;")
            && body.contains("return ")
        {
            rule.emit(
                ctx,
                sym.line,
                name,
                &format!("Function '{name}' mixes bare and value returns"),
                findings,
            );
        }
    }
}

fn check_unreachable_code(
    rule: &FileLintSpec,
    ctx: &FileLintContext<'_>,
    findings: &mut Vec<LintFinding>,
) {
    let mut prev_jump = false;
    for (idx, &(_, t)) in ctx.code_lines.iter().enumerate() {
        if t.is_empty() {
            continue;
        }
        let closes = ["}", "case ", "default:", "else", "catch", "except"]
            .iter()
            .any(|p| t.starts_with(p))
            || t.ends_with(':');
        if prev_jump && !closes {
            let line_no = (idx + 1) as u32;
            rule.emit(
                ctx,
                line_no,
                ctx.enclosing_at(line_no),
                "statement after unconditional jump",
                findings,
            );
        }
        prev_jump = t == "break;"
            || t == "continue;"
            || t == "return;"
            || (t.starts_with("return ") && t.ends_with(';') && !t.contains("if "))
            || (t.starts_with("throw ") && t.ends_with(';'))
            || (t.starts_with("goto ") && t.ends_with(';'));
    }
}

fn check_naming_series(
    rule: &FileLintSpec,
    ctx: &FileLintContext<'_>,
    findings: &mut Vec<LintFinding>,
) {
    let mut series = std::collections::HashMap::<_, Vec<(&Symbol, &str)>>::new();
    for &(sym, name) in &ctx.syms {
        let short = name.rsplit("::").next().unwrap_or(name);
        if let Some(base) = strip_digit_suffix(short) {
            series
                .entry((sym.scope, base))
                .or_default()
                .push((sym, short));
        }
    }
    for ((_, base), mut group) in series {
        group.sort_by_key(|(_, s)| *s);
        group.dedup_by_key(|(_, s)| *s);
        if group.len() >= 2 {
            let (sym, name) = group[0];
            rule.emit(
                ctx,
                sym.line,
                name,
                &format!("Digit-suffixed series '{base}1/{base}2/...'"),
                findings,
            );
        }
    }
}

fn check_naming_predicate(
    rule: &FileLintSpec,
    ctx: &FileLintContext<'_>,
    findings: &mut Vec<LintFinding>,
) {
    for &(sym, name) in &ctx.syms {
        let short = name.rsplit("::").next().unwrap_or(name);
        if ["is_", "has_", "can_", "should_", "was_"]
            .iter()
            .any(|p| short.starts_with(p))
            && let Some(ret) = sig_return_type(ctx.raw, sym)
            && !ret.contains("bool")
        {
            rule.emit(
                ctx,
                sym.line,
                name,
                &format!("Predicate '{short}' returns '{ret}'"),
                findings,
            );
        }
    }
}

fn check_naming_setter(
    rule: &FileLintSpec,
    ctx: &FileLintContext<'_>,
    findings: &mut Vec<LintFinding>,
) {
    for &(sym, name) in &ctx.syms {
        let short = name.rsplit("::").next().unwrap_or(name);
        if short.starts_with("set_")
            && let Some(ret) = sig_return_type(ctx.raw, sym)
            && ret != "()"
            && !ret.contains("Result<()")
        {
            rule.emit(
                ctx,
                sym.line,
                name,
                &format!("Setter '{short}' returns '{ret}'"),
                findings,
            );
        }
    }
}

fn check_naming_confusable(
    rule: &FileLintSpec,
    ctx: &FileLintContext<'_>,
    findings: &mut Vec<LintFinding>,
) {
    for (i, &(sa, na)) in ctx.syms.iter().enumerate() {
        let sha = na.rsplit("::").next().unwrap_or(na);
        if sha.len() < 4 || strip_digit_suffix(sha).is_some() {
            continue;
        }
        if let Some((_, nb)) = ctx.syms[i + 1..].iter().find(|&&(sb, nb)| {
            let shb = nb.rsplit("::").next().unwrap_or(nb);
            sa.scope == sb.scope
                && shb.len() >= 4
                && strip_digit_suffix(shb).is_none()
                && edit_distance_one(sha, shb)
        }) {
            rule.emit(
                ctx,
                sa.line,
                na,
                &format!("Confusable names '{sha}' and '{nb}'"),
                findings,
            );
        }
    }
}

fn check_naming_uninformative(
    rule: &FileLintSpec,
    ctx: &FileLintContext<'_>,
    findings: &mut Vec<LintFinding>,
) {
    for &(sym, name) in &ctx.syms {
        if sym.is_callable() && sym.loc >= 20 {
            let short = name.rsplit("::").next().unwrap_or(name);
            let parts: Vec<&str> = short.split('_').filter(|s| !s.is_empty()).collect();
            if !parts.is_empty()
                && parts
                    .iter()
                    .all(|p| GENERIC_SUBTOKENS.contains(&p.to_ascii_lowercase().as_str()))
            {
                rule.emit(
                    ctx,
                    sym.line,
                    name,
                    &format!("Function '{short}' uses only generic subtokens"),
                    findings,
                );
            }
        }
    }
}

fn check_shared_ptr_by_value(
    rule: &FileLintSpec,
    ctx: &FileLintContext<'_>,
    findings: &mut Vec<LintFinding>,
) {
    if !ctx.is_c_family() {
        return;
    }
    for &(sym, name) in &ctx.syms {
        if sym.is_callable()
            && let Some(sig) = ctx
                .code
                .get(sym.sig_start_byte as usize..sym.sig_end_byte as usize)
            && let Some(after) = sig.split("shared_ptr<").nth(1)
            && let Some(tail) = after.split([',', ')']).next()
            && !tail.contains('&')
            && !tail.contains('*')
        {
            rule.emit(
                ctx,
                sym.line,
                name,
                &format!("Function '{name}' takes shared_ptr by value"),
                findings,
            );
        }
    }
}

/// Complete registry of all 33 file-, scope-, and line-level lint rules.
pub static FILE_LINT_RULES: &[&dyn FileLintRule] = &[
    &FileLintSpec {
        desc: LINT_RULES[10],
        kind: FileLintKind::CFamilyLines(|_, t, _| {
            C_STYLE_CASTS
                .iter()
                .any(|c| t.contains(c))
                .then_some("C-style cast")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[11],
        kind: FileLintKind::AllLines(|_, t, _| t.contains("goto ").then_some("goto statement")),
    },
    &FileLintSpec {
        desc: LINT_RULES[12],
        kind: FileLintKind::AllLines(|_, t, _| {
            (t.starts_with("} while (") || t.starts_with("} while(")).then_some("do/while loop")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[13],
        kind: FileLintKind::AllLines(|_, t, _| {
            ["strcpy(", "strcat(", "sprintf(", "gets("]
                .iter()
                .any(|f| t.contains(f))
                .then_some("unbounded C string call")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[14],
        kind: FileLintKind::AllLines(|_, t, _| {
            ["MD5(", "SHA1(", "MD4(", "RC4("]
                .iter()
                .any(|c| t.contains(c))
                .then_some("weak cryptographic primitive")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[15],
        kind: FileLintKind::AllLines(|_, t, _| {
            ["if ((", "while ((", "return (("]
                .iter()
                .any(|p| {
                    t.strip_prefix(p)
                        .and_then(|r| r.split_once("))"))
                        .is_some_and(|(inner, _)| {
                            !inner.is_empty()
                                && inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        })
                })
                .then_some("doubly-parenthesized expression")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[16],
        kind: FileLintKind::CFamilyLines(|_, t, _| {
            ((t.starts_with("if (") || t.starts_with("if("))
                && t.strip_suffix(';')
                    .is_some_and(|s| s.trim_end().ends_with(')')))
            .then_some("if condition followed immediately by ';'")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[17],
        kind: FileLintKind::AllLines(|_, t, _| {
            t.contains("typedef struct ")
                .then_some("C-style typedef struct")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[18],
        kind: FileLintKind::CFamilyLines(|idx, t, ctx| {
            let line_no = (idx + 1) as u32;
            let is_decl = ["const ", "constexpr ", "#define", "case ", "enum "]
                .iter()
                .any(|p| t.contains(p));
            if is_decl || ctx.enclosing_at(line_no) == "<top>" {
                return None;
            }
            t.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .any(|tok| {
                    tok.len() >= 2
                        && tok.bytes().all(|b| b.is_ascii_digit())
                        && !ALLOWED_NUM_LITERALS.contains(&tok)
                })
                .then_some("non-trivial numeric literal in function body")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[19],
        kind: FileLintKind::AllLines(|_, t, _| {
            (t.contains("catch {}") || t.contains("catch (...) {}")).then_some("empty catch block")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[20],
        kind: FileLintKind::AllLines(|_, t, _| {
            let (lhs, rhs) = t.strip_suffix(';')?.split_once('=')?;
            if lhs.ends_with(['=', '!', '<', '>', '+', '-', '*', '/', '%', '&', '|', '^'])
                || rhs.starts_with('=')
            {
                return None;
            }
            let (l, r) = (lhs.trim(), rhs.trim());
            (!l.is_empty()
                && l == r
                && l.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.')))
            .then_some("self-assignment x = x")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[21],
        kind: FileLintKind::Custom(check_inconsistent_return),
    },
    &FileLintSpec {
        desc: LINT_RULES[22],
        kind: FileLintKind::Custom(check_unreachable_code),
    },
    &FileLintSpec {
        desc: LINT_RULES[23],
        kind: FileLintKind::Custom(check_naming_series),
    },
    &FileLintSpec {
        desc: LINT_RULES[24],
        kind: FileLintKind::Custom(check_naming_predicate),
    },
    &FileLintSpec {
        desc: LINT_RULES[25],
        kind: FileLintKind::Custom(check_naming_setter),
    },
    &FileLintSpec {
        desc: LINT_RULES[26],
        kind: FileLintKind::Custom(check_naming_confusable),
    },
    &FileLintSpec {
        desc: LINT_RULES[27],
        kind: FileLintKind::Custom(check_naming_uninformative),
    },
    &FileLintSpec {
        desc: LINT_RULES[28],
        kind: FileLintKind::CFamilyLines(|_, t, _| {
            let (first_lhs, _) = t.split_once('=')?;
            (!t.starts_with("for")
                && t.ends_with(';')
                && !t.contains('(')
                && !first_lhs.trim().contains(' ')
                && t.matches('=').count() >= 2
                && t.contains(", "))
            .then_some("comma operator in expression statement")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[29],
        kind: FileLintKind::CFamilyLines(|_, t, _| {
            if t.starts_with("for ")
                || t.starts_with("for(")
                || !(t.contains("++") || t.contains("--"))
            {
                return None;
            }
            let has_outer_expr = t.contains(" = ")
                || t.contains("+ ")
                || t.contains("- ")
                || (t.contains('[') && (t.contains("++]") || t.contains("--]")));
            has_outer_expr.then_some("++/-- embedded inside a larger expression")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[30],
        kind: FileLintKind::CFamilyLines(|_, t, _| {
            extract_cond(t)
                .is_some_and(|c| c.contains(" = "))
                .then_some("assignment used as condition value")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[31],
        kind: FileLintKind::AllLines(|_, t, _| {
            (t.matches('?').count() >= 2 && t.replace("::", "").matches(':').count() >= 2)
                .then_some("nested conditional ?: expression")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[32],
        kind: FileLintKind::CFamilyLines(|_, t, _| {
            let cond = extract_cond(t)?;
            let has_cmp = ["==", "!=", "<=", ">=", "<", ">", "&&", "||", "!"]
                .iter()
                .any(|op| cond.contains(op));
            (!has_cmp && (cond.contains("strcmp(") || cond.contains(" + ") || cond.contains(" % ")))
                .then_some("arithmetic or strcmp used as truth value")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[33],
        kind: FileLintKind::CFamilyLines(|_, t, _| {
            t.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .any(|tok| {
                    tok.len() >= 2
                        && tok.starts_with('0')
                        && tok.bytes().all(|b| (b'0'..=b'7').contains(&b))
                })
                .then_some("leading-zero octal integer literal")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[34],
        kind: FileLintKind::CFamilyLines(|_, t, _| {
            let b = t.as_bytes();
            (1..b.len())
                .any(|i| {
                    b[i] == b'['
                        && b[i - 1].is_ascii_digit()
                        && (i < 2 || !(b[i - 2].is_ascii_alphanumeric() || b[i - 2] == b'_'))
                })
                .then_some("reversed subscript literal[expr]")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[35],
        kind: FileLintKind::AllLines(|_, t, _| {
            ["std::map<", "std::list<", "std::set<"]
                .iter()
                .any(|c| t.contains(c))
                .then_some("node-based std container")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[36],
        kind: FileLintKind::CFamilyLines(|_, t, _| {
            t.split("vector<")
                .nth(1)
                .and_then(|a| a.split('>').next())
                .is_some_and(|ty| ty.contains('*'))
                .then_some("vector<T*> scatters pointees")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[37],
        kind: FileLintKind::AllLines(|_, t, _| {
            [
                "std::vector<std::unique_ptr<",
                "std::vector<std::shared_ptr<",
            ]
            .iter()
            .any(|c| t.contains(c))
            .then_some("vector of smart pointers")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[38],
        kind: FileLintKind::CFamilyLines(|idx, t, ctx| {
            (ctx.loop_lines[idx]
                && ["new ", "malloc(", "calloc(", "realloc(", "strdup("]
                    .iter()
                    .any(|a| t.contains(a)))
            .then_some("heap allocation inside loop")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[39],
        kind: FileLintKind::CFamilyLines(|idx, t, ctx| {
            (ctx.loop_lines[idx] && t.contains('=') && t.contains("->next"))
                .then_some("pointer-chase ->next inside loop")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[40],
        kind: FileLintKind::CFamilyLines(|idx, t, ctx| {
            (ctx.loop_lines[idx]
                && t.split_once('[')
                    .and_then(|(_, r)| r.split_once(']'))
                    .is_some_and(|(inner, _)| inner.contains('[')))
            .then_some("nested gather subscript a[b[i]] in loop")
        }),
    },
    &FileLintSpec {
        desc: LINT_RULES[41],
        kind: FileLintKind::Custom(check_shared_ptr_by_value),
    },
    &FileLintSpec {
        desc: LINT_RULES[42],
        kind: FileLintKind::AllLines(|_, t, _| {
            (t.contains("_mm_prefetch(") || t.contains("__builtin_prefetch("))
                .then_some("manual prefetch intrinsic")
        }),
    },
];

/// Collect `(symbol, resolved_name)` pairs belonging to `file_id`.
#[must_use]
pub fn symbols_in_file<'a>(
    ingest: &'a IngestResult,
    interner: &'a Interner,
    file_id: u32,
) -> Vec<SymSpan<'a>> {
    ingest
        .symbols
        .iter()
        .filter(|s| s.file_id == file_id && s.kind != SymKind::Section)
        .map(|s| (s, interner.resolve(&s.name)))
        .collect()
}
