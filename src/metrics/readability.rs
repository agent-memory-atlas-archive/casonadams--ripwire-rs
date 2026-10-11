use std::collections::HashMap;

pub const POSNETT_INTERCEPT: f64 = 8.87;
pub const POSNETT_VOLUME: f64 = -0.033;
pub const POSNETT_LINES: f64 = 0.40;
pub const POSNETT_ENTROPY: f64 = -1.5;
pub const POSNETT_Z_CLAMP: f64 = 40.0;

/// Readability metrics computed for a symbol or function definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadabilityMetrics {
    pub token_count: u32,
    pub operator_count: u32,
    pub operand_count: u32,
    pub vocabulary_count: u32,
    pub volume: f64,
    pub entropy: f64,
    pub lines: u32,
    pub posnett: f64,
}

/// Compute the Posnett/Hindle/Devanbu (MSR 2011) readability score.
#[must_use]
pub fn posnett_score(volume: f64, lines: f64, entropy: f64) -> f64 {
    let z = POSNETT_INTERCEPT
        + POSNETT_VOLUME * volume
        + POSNETT_LINES * lines
        + POSNETT_ENTROPY * entropy;
    let clamped = z.clamp(-POSNETT_Z_CLAMP, POSNETT_Z_CLAMP);
    1.0 / (1.0 + (-clamped).exp())
}

#[derive(Debug, PartialEq, Eq)]
enum TokenKind {
    Operator,
    Operand,
}

fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "fn" | "function"
            | "def"
            | "func"
            | "let"
            | "mut"
            | "var"
            | "const"
            | "if"
            | "else"
            | "for"
            | "while"
            | "loop"
            | "match"
            | "switch"
            | "case"
            | "return"
            | "break"
            | "continue"
            | "struct"
            | "class"
            | "enum"
            | "impl"
            | "trait"
            | "interface"
            | "pub"
            | "public"
            | "private"
            | "protected"
            | "import"
            | "use"
            | "from"
            | "as"
            | "type"
            | "async"
            | "await"
            | "try"
            | "catch"
            | "throw"
    )
}

fn scan_string_literal(chars: &[char], i: &mut usize) -> String {
    let quote = chars[*i];
    let mut s = String::new();
    s.push(quote);
    *i += 1;
    while *i < chars.len() {
        let c = chars[*i];
        s.push(c);
        *i += 1;
        if c == '\\' && *i < chars.len() {
            s.push(chars[*i]);
            *i += 1;
        } else if c == quote {
            break;
        }
    }
    s
}

fn scan_word(chars: &[char], i: &mut usize) -> String {
    let mut s = String::new();
    while *i < chars.len() && (chars[*i].is_alphanumeric() || chars[*i] == '_') {
        s.push(chars[*i]);
        *i += 1;
    }
    s
}

fn scan_symbol_token(chars: &[char], i: &mut usize) -> String {
    let multi_ops = [
        "==", "!=", "<=", ">=", "&&", "||", "::", "->", "=>", "+=", "-=",
    ];
    if *i + 1 < chars.len() {
        let pair: String = chars[*i..=*i + 1].iter().collect();
        if multi_ops.contains(&pair.as_str()) {
            *i += 2;
            return pair;
        }
    }
    let s = chars[*i].to_string();
    *i += 1;
    s
}

fn tokenize_code(code: &str) -> Vec<(String, TokenKind)> {
    let chars: Vec<char> = code.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' || c == '\'' {
            let lit = scan_string_literal(&chars, &mut i);
            tokens.push((lit, TokenKind::Operand));
        } else if c.is_alphabetic() || c == '_' {
            let word = scan_word(&chars, &mut i);
            let kind = if is_keyword(&word) {
                TokenKind::Operator
            } else {
                TokenKind::Operand
            };
            tokens.push((word, kind));
        } else if c.is_ascii_digit() {
            let num = scan_word(&chars, &mut i);
            tokens.push((num, TokenKind::Operand));
        } else {
            let sym = scan_symbol_token(&chars, &mut i);
            tokens.push((sym, TokenKind::Operator));
        }
    }
    tokens
}

fn calculate_entropy(frequencies: &[u32], total: f64) -> f64 {
    if total <= 0.0 {
        return 0.0;
    }
    let mut entropy = 0.0;
    for &freq in frequencies {
        let p = (freq as f64) / total;
        if p > 0.0 {
            entropy -= p * p.log2();
        }
    }
    entropy
}

/// Compute Halstead Volume, Token Entropy, and Posnett Readability score.
#[must_use]
pub fn compute_readability(code: &str) -> ReadabilityMetrics {
    let tokens = tokenize_code(code);
    let token_count = tokens.len() as u32;
    let lines = (code.lines().count() as u32).max(1);

    if tokens.is_empty() {
        return ReadabilityMetrics {
            token_count: 0,
            operator_count: 0,
            operand_count: 0,
            vocabulary_count: 0,
            volume: 0.0,
            entropy: 0.0,
            lines,
            posnett: posnett_score(0.0, lines as f64, 0.0),
        };
    }

    let mut vocab_map: HashMap<String, u32> = HashMap::new();
    let mut operator_count = 0u32;
    let mut operand_count = 0u32;

    for (text, kind) in &tokens {
        *vocab_map.entry(text.clone()).or_insert(0) += 1;
        match kind {
            TokenKind::Operator => operator_count += 1,
            TokenKind::Operand => operand_count += 1,
        }
    }

    let vocabulary_count = vocab_map.len() as u32;
    let volume = if vocabulary_count > 1 {
        (token_count as f64) * (vocabulary_count as f64).log2()
    } else {
        0.0
    };

    let frequencies: Vec<u32> = vocab_map.into_values().collect();
    let entropy = calculate_entropy(&frequencies, token_count as f64);
    let posnett = posnett_score(volume, lines as f64, entropy);

    ReadabilityMetrics {
        token_count,
        operator_count,
        operand_count,
        vocabulary_count,
        volume,
        entropy,
        lines,
        posnett,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_posnett_score_calculation() {
        let p = posnett_score(100.0, 10.0, 3.5);
        assert!((0.0..=1.0).contains(&p));
    }

    #[test]
    fn test_readability_metrics_computation() {
        let code = r#"
pub fn factorial(n: u64) -> u64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}
"#;
        let m = compute_readability(code);
        assert!(m.token_count > 10);
        assert!(m.operator_count > 0);
        assert!(m.operand_count > 0);
        assert!(m.vocabulary_count > 5);
        assert!(m.volume > 0.0);
        assert!(m.entropy > 0.0);
        assert_eq!(m.lines, 8);
        assert!((0.0..=1.0).contains(&m.posnett));
    }
}
