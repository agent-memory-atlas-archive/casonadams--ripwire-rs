pub(crate) const DEFINITION_MODIFIERS: &[&str] = &[
    "static",
    "inline",
    "virtual",
    "constexpr",
    "public",
    "private",
    "protected",
    "final",
    "abstract",
    "override",
    "synchronized",
    "native",
    "extern",
    "friend",
    "explicit",
    "const",
    "volatile",
    "unsafe",
    "async",
    "new",
    "sealed",
    "readonly",
    "internal",
    "partial",
    "default",
    "transient",
];

fn trim_return_type(ret: &str) -> Option<&str> {
    let ret = ret.split(" where ").next().unwrap_or(ret);
    let ret = ret.trim().trim_end_matches([':', '{', ';']).trim();
    let ret = ret.trim_end_matches("=>").trim();
    (!ret.is_empty()).then_some(ret)
}

fn find_at_depth_zero(sig: &str, pat: &str) -> Option<usize> {
    let bytes = sig.as_bytes();
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            _ if depth == 0 && bytes[i..].starts_with(pat.as_bytes()) => return Some(i),
            _ => {}
        }
    }
    None
}

pub(crate) fn arrow_return_type(sig: &str) -> Option<&str> {
    let at = find_at_depth_zero(sig, "->")?;
    trim_return_type(&sig[at + 2..])
}

pub(crate) fn colon_return_type(sig: &str) -> Option<&str> {
    let open = sig.find('(')?;
    let mut depth = 0i32;
    for (i, c) in sig[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let rest = sig[open + i + 1..].trim_start().strip_prefix(':')?;
                    return trim_return_type(rest);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn strip_qualifiers(mut prefix: &str) -> &str {
    prefix = prefix.trim_end();
    while let Some(head) = prefix.strip_suffix("::") {
        prefix = head.trim_end_matches(is_ident_char).trim_end();
    }
    prefix
}

fn single_type_word(prefix: &str) -> Option<&str> {
    let mut words = prefix
        .split_whitespace()
        .filter(|w| !DEFINITION_MODIFIERS.contains(w) && !w.starts_with('@'));
    let ty = words.next()?;
    (words.next().is_none() && ty.chars().all(is_ident_char)).then_some(ty)
}

pub(crate) fn prefix_return_type<'a>(sig: &'a str, name: &str) -> Option<&'a str> {
    let prev_is_ident = |at: usize| sig[..at].chars().next_back().is_some_and(is_ident_char);
    let mut from = 0;
    while let Some(rel) = sig[from..].find(name) {
        let at = from + rel;
        let end = at + name.len();
        let before_ok = !prev_is_ident(at);
        if before_ok && sig[end..].trim_start().starts_with('(') {
            return single_type_word(strip_qualifiers(&sig[..at]));
        }
        from = end;
    }
    None
}

pub(crate) fn declares_type(decl: &str, keywords: &[&str]) -> bool {
    decl.split_whitespace()
        .find(|w| {
            !DEFINITION_MODIFIERS.contains(w)
                && !matches!(*w, "export" | "declare" | "pub" | "abstract")
                && !w.starts_with("pub(")
        })
        .is_some_and(|w| keywords.contains(&w))
}

pub(crate) fn constant_initializer(node: tree_sitter::Node<'_>, source: &str) -> bool {
    let mut parent = node.parent();
    while let Some(p) = parent {
        if matches!(p.kind(), "declaration" | "field_declaration") {
            return source[p.byte_range()]
                .split_whitespace()
                .any(|w| matches!(w, "const" | "constexpr"));
        }
        if matches!(p.kind(), "compound_statement" | "function_definition") {
            return false;
        }
        parent = p.parent();
    }
    false
}
