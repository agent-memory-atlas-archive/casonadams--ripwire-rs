use super::lint::{LintFinding, LintRuleDesc};
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
        matches!(self.lang, Lang::C | Lang::Cpp | Lang::ObjC | Lang::Cuda)
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

fn check_lines<F>(
    rule: &dyn FileLintRule,
    ctx: &FileLintContext<'_>,
    findings: &mut Vec<LintFinding>,
    mut pred: F,
) where
    F: FnMut(usize, &str) -> Option<&'static str>,
{
    for (idx, &(_, trimmed)) in ctx.code_lines.iter().enumerate() {
        if !trimmed.is_empty()
            && let Some(msg) = pred(idx, trimmed)
        {
            let line_no = (idx + 1) as u32;
            rule.emit(ctx, line_no, ctx.enclosing_at(line_no), msg, findings);
        }
    }
}

struct CStyleCastRule;
impl FileLintRule for CStyleCastRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "c-style-cast",
            severity: "warn",
            category: "style",
            description: "a C-style cast — prefers explicit static_cast/const_cast/reinterpret_cast",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |_, t| {
            [
                "(int)", "(char*)", "(void*)", "(size_t)", "(double)", "(float)",
            ]
            .iter()
            .any(|c| t.contains(c))
            .then_some("C-style cast")
        });
    }
}

struct GotoRule;
impl FileLintRule for GotoRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "goto",
            severity: "warn",
            category: "control-flow",
            description: "a goto statement — cppcoreguidelines-avoid-goto",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
            t.contains("goto ").then_some("goto statement")
        });
    }
}

struct DoWhileRule;
impl FileLintRule for DoWhileRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "do-while",
            severity: "info",
            category: "style",
            description: "a do/while loop shape",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
            (t.starts_with("} while (") || t.starts_with("} while(")).then_some("do/while loop")
        });
    }
}

struct UnsafeCFnRule;
impl FileLintRule for UnsafeCFnRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "unsafe-c-fn",
            severity: "error",
            category: "security",
            description: "a call to an unbounded C string function (strcpy/strcat/sprintf/gets)",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
            ["strcpy(", "strcat(", "sprintf(", "gets("]
                .iter()
                .find(|&&f| t.contains(f))
                .map(|_| "unbounded C string call")
        });
    }
}

struct WeakCryptoRule;
impl FileLintRule for WeakCryptoRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "weak-crypto",
            severity: "error",
            category: "security",
            description: "a call to a broken hash or cipher (MD5/SHA1/MD4/RC4)",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
            ["MD5(", "SHA1(", "MD4(", "RC4("]
                .iter()
                .any(|c| t.contains(c))
                .then_some("weak cryptographic primitive")
        });
    }
}

struct RedundantParensRule;
impl FileLintRule for RedundantParensRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "redundant-parens",
            severity: "info",
            category: "style",
            description: "a doubly-parenthesized expression — readability-redundant-parentheses",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
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
        });
    }
}

struct SuspiciousSemicolonRule;
impl FileLintRule for SuspiciousSemicolonRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "suspicious-semicolon",
            severity: "warn",
            category: "correctness",
            description: "an if-body that is just `;` — bugprone-suspicious-semicolon",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |_, t| {
            ((t.starts_with("if (") || t.starts_with("if("))
                && t.strip_suffix(';')
                    .is_some_and(|s| s.trim_end().ends_with(')')))
            .then_some("if condition followed immediately by ';'")
        });
    }
}

struct TypedefOverUsingRule;
impl FileLintRule for TypedefOverUsingRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "typedef-over-using",
            severity: "info",
            category: "style",
            description: "a C-style typedef struct/union where `using` is preferred",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
            t.contains("typedef struct ")
                .then_some("C-style typedef struct")
        });
    }
}

struct MagicNumberRule;
impl FileLintRule for MagicNumberRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "magic-number",
            severity: "info",
            category: "maintainability",
            description: "a non-trivial numeric literal inside a function body",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |idx, t| {
            let line_no = (idx + 1) as u32;
            let is_decl = [
                "const ",
                "constexpr ",
                "static ",
                "#define",
                "case ",
                "enum ",
            ]
            .iter()
            .any(|p| t.starts_with(p));
            if is_decl || ctx.enclosing_at(line_no) == "<top>" {
                return None;
            }
            t.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .any(|tok| {
                    tok.len() >= 2
                        && tok.bytes().all(|b| b.is_ascii_digit())
                        && !matches!(
                            tok,
                            "10" | "16" | "32" | "64" | "100" | "256" | "1000" | "1024"
                        )
                })
                .then_some("non-trivial numeric literal in function body")
        });
    }
}

struct EmptyCatchRule;
impl FileLintRule for EmptyCatchRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "empty-catch",
            severity: "warn",
            category: "error-masking",
            description: "a catch block with an empty body",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
            (t.contains("catch {}") || t.contains("catch (...) {}")).then_some("empty catch block")
        });
    }
}

struct SelfAssignRule;
impl FileLintRule for SelfAssignRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "self-assign",
            severity: "warn",
            category: "correctness",
            description: "x = x — almost always a copy-paste bug",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
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
        });
    }
}

struct InconsistentReturnRule;
impl FileLintRule for InconsistentReturnRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "inconsistent-return",
            severity: "info",
            category: "maintainability",
            description: "a bare `return;` mixed with a value-returning return in the same function",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        for &(sym, name) in &ctx.syms {
            if sym.kind != SymKind::Function && sym.kind != SymKind::Method {
                continue;
            }
            let Some(body) = ctx
                .code
                .get(sym.sig_end_byte as usize..sym.end_byte as usize)
            else {
                continue;
            };
            if body.contains("return;") && body.contains("return ") {
                self.emit(
                    ctx,
                    sym.line,
                    name,
                    &format!("Function '{name}' mixes bare and value returns"),
                    findings,
                );
            }
        }
    }
}

struct UnreachableCodeRule;
impl FileLintRule for UnreachableCodeRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "unreachable-code",
            severity: "warn",
            category: "correctness",
            description: "a statement after an unconditional return/break/continue/throw/raise",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
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
                self.emit(
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
}

fn strip_digit_suffix(name: &str) -> Option<&str> {
    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    (trimmed.len() < name.len() && !trimmed.is_empty()).then_some(trimmed)
}

struct NamingSeriesRule;
impl FileLintRule for NamingSeriesRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "naming-series",
            severity: "info",
            category: "naming",
            description: "foo1/foo2/... digit-suffix siblings sharing a base name in one scope",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
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
                self.emit(
                    ctx,
                    sym.line,
                    name,
                    &format!("Digit-suffixed series '{base}1/{base}2/...'"),
                    findings,
                );
            }
        }
    }
}

fn sig_return_type<'a>(raw: &'a str, sym: &Symbol) -> Option<&'a str> {
    let sig = raw.get(sym.sig_start_byte as usize..sym.sig_end_byte as usize)?;
    let (_, ret) = sig.rsplit_once("->")?;
    let ret = ret.trim().trim_end_matches('{').trim();
    (!ret.is_empty()).then_some(ret)
}

struct NamingPredicateRule;
impl FileLintRule for NamingPredicateRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "naming-predicate",
            severity: "info",
            category: "naming",
            description: "an is/has/can/should/was-prefixed name whose return type is not bool-like",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        for &(sym, name) in &ctx.syms {
            let short = name.rsplit("::").next().unwrap_or(name);
            if ["is_", "has_", "can_", "should_", "was_"]
                .iter()
                .any(|p| short.starts_with(p))
                && let Some(ret) = sig_return_type(ctx.raw, sym)
                && !ret.contains("bool")
            {
                self.emit(
                    ctx,
                    sym.line,
                    name,
                    &format!("Predicate '{short}' returns '{ret}'"),
                    findings,
                );
            }
        }
    }
}

struct NamingSetterRule;
impl FileLintRule for NamingSetterRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "naming-setter",
            severity: "info",
            category: "naming",
            description: "a set-prefixed name whose return type is not void-like",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        for &(sym, name) in &ctx.syms {
            let short = name.rsplit("::").next().unwrap_or(name);
            if short.starts_with("set_")
                && let Some(ret) = sig_return_type(ctx.raw, sym)
                && ret != "()"
                && !ret.contains("Result<()")
            {
                self.emit(
                    ctx,
                    sym.line,
                    name,
                    &format!("Setter '{short}' returns '{ret}'"),
                    findings,
                );
            }
        }
    }
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

struct NamingConfusableRule;
impl FileLintRule for NamingConfusableRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "naming-confusable",
            severity: "info",
            category: "naming",
            description: "a co-visible pair within edit distance <=2 or reordered tokens",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
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
                self.emit(
                    ctx,
                    sa.line,
                    na,
                    &format!("Confusable names '{sha}' and '{nb}'"),
                    findings,
                );
            }
        }
    }
}

const GENERIC_SUBTOKENS: &[&str] = &[
    "get", "set", "do", "run", "handle", "process", "data", "item", "val", "value", "info", "obj",
    "tmp", "fn", "func", "helper", "util",
];

struct NamingUninformativeRule;
impl FileLintRule for NamingUninformativeRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "naming-uninformative",
            severity: "info",
            category: "naming",
            description: "every split subtoken is corpus-ubiquitous (BM25 idf) on a body past size floor",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        for &(sym, name) in &ctx.syms {
            if (sym.kind == SymKind::Function || sym.kind == SymKind::Method) && sym.loc >= 20 {
                let short = name.rsplit("::").next().unwrap_or(name);
                let parts: Vec<&str> = short.split('_').filter(|s| !s.is_empty()).collect();
                if !parts.is_empty()
                    && parts
                        .iter()
                        .all(|p| GENERIC_SUBTOKENS.contains(&p.to_ascii_lowercase().as_str()))
                {
                    self.emit(
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
}

struct AtomCommaOperatorRule;
impl FileLintRule for AtomCommaOperatorRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "atom-comma-operator",
            severity: "info",
            category: "readability",
            description: "the comma operator inside an expression [Gopstein FSE 2017]",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |_, t| {
            (!t.starts_with("for")
                && t.ends_with(';')
                && !t.contains('(')
                && t.matches('=').count() >= 2
                && t.contains(", "))
            .then_some("comma operator in expression statement")
        });
    }
}

struct AtomEmbeddedCrementRule;
impl FileLintRule for AtomEmbeddedCrementRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "atom-embedded-crement",
            severity: "info",
            category: "readability",
            description: "++/-- evaluated inside a larger expression [Gopstein FSE 2017]",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |_, t| {
            if t.starts_with("for ")
                || t.starts_with("for(")
                || !(t.contains("++") || t.contains("--"))
            {
                return None;
            }
            let bare = t.trim_end_matches(';').trim();
            let standalone = bare
                .strip_prefix("++")
                .or_else(|| bare.strip_prefix("--"))
                .or_else(|| bare.strip_suffix("++"))
                .or_else(|| bare.strip_suffix("--"));
            standalone
                .is_none_or(|s| !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
                .then_some("++/-- embedded inside a larger expression")
        });
    }
}

fn extract_cond(t: &str) -> Option<&str> {
    t.strip_prefix("if (")
        .or_else(|| t.strip_prefix("if("))
        .or_else(|| t.strip_prefix("while ("))
        .or_else(|| t.strip_prefix("while("))?
        .rsplit_once(')')
        .map(|(c, _)| c)
}

struct AtomAssignAsValueRule;
impl FileLintRule for AtomAssignAsValueRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "atom-assign-as-value",
            severity: "info",
            category: "readability",
            description: "an assignment whose VALUE is consumed [Gopstein FSE 2017]",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |_, t| {
            extract_cond(t)
                .is_some_and(|c| c.contains(" = "))
                .then_some("assignment used as condition value")
        });
    }
}

struct AtomNestedTernaryRule;
impl FileLintRule for AtomNestedTernaryRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "atom-nested-ternary",
            severity: "info",
            category: "readability",
            description: "a conditional expression inside a conditional expression [Gopstein FSE 2017]",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
            (t.matches('?').count() >= 2 && t.replace("::", "").matches(':').count() >= 2)
                .then_some("nested conditional ?: expression")
        });
    }
}

struct AtomImplicitPredicateRule;
impl FileLintRule for AtomImplicitPredicateRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "atom-implicit-predicate",
            severity: "info",
            category: "readability",
            description: "arithmetic used where a truth value is expected [Gopstein FSE 2017]",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |_, t| {
            let cond = extract_cond(t)?;
            let has_cmp = ["==", "!=", "<=", ">=", "<", ">", "&&", "||", "!"]
                .iter()
                .any(|op| cond.contains(op));
            (!has_cmp && (cond.contains("strcmp(") || cond.contains(" + ") || cond.contains(" % ")))
                .then_some("arithmetic or strcmp used as truth value")
        });
    }
}

struct AtomOctalLiteralRule;
impl FileLintRule for AtomOctalLiteralRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "atom-octal-literal",
            severity: "info",
            category: "readability",
            description: "a leading-zero integer literal — 0755 is 493, not 755 [Gopstein FSE 2017]",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |_, t| {
            t.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .any(|tok| {
                    tok.len() >= 2
                        && tok.starts_with('0')
                        && tok.bytes().all(|b| (b'0'..=b'7').contains(&b))
                })
                .then_some("leading-zero octal integer literal")
        });
    }
}

struct AtomReversedSubscriptRule;
impl FileLintRule for AtomReversedSubscriptRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "atom-reversed-subscript",
            severity: "info",
            category: "readability",
            description: "1[arr] — legal C, almost never intended [Gopstein FSE 2017]",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |_, t| {
            let b = t.as_bytes();
            (1..b.len())
                .any(|i| {
                    b[i] == b'['
                        && b[i - 1].is_ascii_digit()
                        && (i < 2 || !(b[i - 2].is_ascii_alphanumeric() || b[i - 2] == b'_'))
                })
                .then_some("reversed subscript literal[expr]")
        });
    }
}

struct CacheNodeContainerRule;
impl FileLintRule for CacheNodeContainerRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "cache-node-container",
            severity: "info",
            category: "performance",
            description: "a node-based std container (map/list/set/...) — one heap node per element",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
            ["std::map<", "std::list<", "std::set<"]
                .iter()
                .any(|c| t.contains(c))
                .then_some("node-based std container")
        });
    }
}

struct CacheVectorOfRawPtrRule;
impl FileLintRule for CacheVectorOfRawPtrRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "cache-vector-of-raw-ptr",
            severity: "info",
            category: "performance",
            description: "vector<T*> — contiguous handles, scattered payloads",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |_, t| {
            t.split("vector<")
                .nth(1)
                .and_then(|a| a.split('>').next())
                .is_some_and(|ty| ty.contains('*'))
                .then_some("vector<T*> scatters pointees")
        });
    }
}

struct CacheVectorOfIndirectRule;
impl FileLintRule for CacheVectorOfIndirectRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "cache-vector-of-indirect",
            severity: "info",
            category: "performance",
            description: "vector<unique_ptr/shared_ptr/vector<...>> — an indirection per element",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
            [
                "std::vector<std::unique_ptr<",
                "std::vector<std::shared_ptr<",
            ]
            .iter()
            .any(|c| t.contains(c))
            .then_some("vector of smart pointers")
        });
    }
}

struct CacheHeapAllocInLoopRule;
impl FileLintRule for CacheHeapAllocInLoopRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "cache-heap-alloc-in-loop",
            severity: "info",
            category: "performance",
            description: "new/malloc/calloc/realloc/strdup inside a loop body",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |idx, t| {
            (ctx.loop_lines[idx]
                && ["new ", "malloc(", "calloc(", "realloc(", "strdup("]
                    .iter()
                    .any(|a| t.contains(a)))
            .then_some("heap allocation inside loop")
        });
    }
}

struct CachePointerChaseLoopRule;
impl FileLintRule for CachePointerChaseLoopRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "cache-pointer-chase-loop",
            severity: "info",
            category: "performance",
            description: "p = p->next inside a loop — a serial dependent-load chain",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |idx, t| {
            (ctx.loop_lines[idx] && t.contains('=') && t.contains("->next"))
                .then_some("pointer-chase ->next inside loop")
        });
    }
}

struct CacheGatherSubscriptRule;
impl FileLintRule for CacheGatherSubscriptRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "cache-gather-subscript",
            severity: "info",
            category: "performance",
            description: "a[b[i]] inside a loop — gather/scatter random access",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        check_lines(self, ctx, findings, |idx, t| {
            (ctx.loop_lines[idx]
                && t.split_once('[')
                    .and_then(|(_, r)| r.split_once(']'))
                    .is_some_and(|(inner, _)| inner.contains('[')))
            .then_some("nested gather subscript a[b[i]] in loop")
        });
    }
}

struct CacheSharedPtrByValueRule;
impl FileLintRule for CacheSharedPtrByValueRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "cache-shared-ptr-by-value",
            severity: "info",
            category: "performance",
            description: "a by-value shared_ptr parameter — atomic refcount traffic per call",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        if !ctx.is_c_family() {
            return;
        }
        for &(sym, name) in &ctx.syms {
            if (sym.kind == SymKind::Function || sym.kind == SymKind::Method)
                && let Some(sig) = ctx
                    .code
                    .get(sym.sig_start_byte as usize..sym.sig_end_byte as usize)
                && let Some(after) = sig.split("shared_ptr<").nth(1)
                && let Some(tail) = after.split([',', ')']).next()
                && !tail.contains('&')
                && !tail.contains('*')
            {
                self.emit(
                    ctx,
                    sym.line,
                    name,
                    &format!("Function '{name}' takes shared_ptr by value"),
                    findings,
                );
            }
        }
    }
}

struct CacheManualPrefetchRule;
impl FileLintRule for CacheManualPrefetchRule {
    fn desc(&self) -> LintRuleDesc {
        LintRuleDesc {
            name: "cache-manual-prefetch",
            severity: "info",
            category: "performance",
            description: "an existing _mm_prefetch/__builtin_prefetch call",
        }
    }
    fn check_file(&self, ctx: &FileLintContext<'_>, findings: &mut Vec<LintFinding>) {
        check_lines(self, ctx, findings, |_, t| {
            (t.contains("_mm_prefetch(") || t.contains("__builtin_prefetch("))
                .then_some("manual prefetch intrinsic")
        });
    }
}

/// Complete registry of all 33 file-, scope-, and line-level lint rules.
pub static FILE_LINT_RULES: &[&dyn FileLintRule] = &[
    &CStyleCastRule,
    &GotoRule,
    &DoWhileRule,
    &UnsafeCFnRule,
    &WeakCryptoRule,
    &RedundantParensRule,
    &SuspiciousSemicolonRule,
    &TypedefOverUsingRule,
    &MagicNumberRule,
    &EmptyCatchRule,
    &SelfAssignRule,
    &InconsistentReturnRule,
    &UnreachableCodeRule,
    &NamingSeriesRule,
    &NamingPredicateRule,
    &NamingSetterRule,
    &NamingConfusableRule,
    &NamingUninformativeRule,
    &AtomCommaOperatorRule,
    &AtomEmbeddedCrementRule,
    &AtomAssignAsValueRule,
    &AtomNestedTernaryRule,
    &AtomImplicitPredicateRule,
    &AtomOctalLiteralRule,
    &AtomReversedSubscriptRule,
    &CacheNodeContainerRule,
    &CacheVectorOfRawPtrRule,
    &CacheVectorOfIndirectRule,
    &CacheHeapAllocInLoopRule,
    &CachePointerChaseLoopRule,
    &CacheGatherSubscriptRule,
    &CacheSharedPtrByValueRule,
    &CacheManualPrefetchRule,
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
