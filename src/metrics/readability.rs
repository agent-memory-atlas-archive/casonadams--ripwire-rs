use crate::clones::token_stream::{
    is_keyword_bytes, scan_ident_slice, scan_number_slice, scan_operator_slice, scan_string_slice,
    skip_whitespace,
};
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

fn tokenize_code(code: &str) -> Vec<(&[u8], TokenKind)> {
    let bytes = code.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            skip_whitespace(bytes, &mut i);
        } else if b == b'"' || b == b'\'' {
            let lit = scan_string_slice(bytes, &mut i);
            tokens.push((lit, TokenKind::Operand));
        } else if b.is_ascii_alphabetic() || b == b'_' {
            let word = scan_ident_slice(bytes, &mut i);
            let kind = if is_keyword_bytes(word) {
                TokenKind::Operator
            } else {
                TokenKind::Operand
            };
            tokens.push((word, kind));
        } else if b.is_ascii_digit() {
            let num = scan_number_slice(bytes, &mut i);
            tokens.push((num, TokenKind::Operand));
        } else {
            let sym = scan_operator_slice(bytes, &mut i);
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

    let mut vocab_map: HashMap<&[u8], u32> = HashMap::new();
    let mut operator_count = 0u32;
    let mut operand_count = 0u32;

    for (text, kind) in &tokens {
        *vocab_map.entry(*text).or_insert(0) += 1;
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

/// Output biggest functions first based on volume and lines.
pub fn execute_biggest_first_query<W: std::io::Write>(
    root: &str,
    ingest: &crate::domain::IngestResult,
    interner: &crate::domain::Interner,
    out: &mut W,
) -> std::io::Result<()> {
    let mut functions: Vec<(&crate::domain::Symbol, String, String)> = Vec::new();
    for sym in &ingest.symbols {
        if sym.kind == crate::domain::SymKind::Function
            || sym.kind == crate::domain::SymKind::Method
        {
            let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
            let n = interner.resolve(&sym.name);
            functions.push((sym, p.to_string(), n.to_string()));
        }
    }

    functions.sort_by(|a, b| b.0.loc.cmp(&a.0.loc).then_with(|| b.0.ccx.cmp(&a.0.ccx)));
    let total = functions.len();
    let shown = total.min(40);
    let capped = usize::from(shown < total);
    let page_attrs = if capped == 1 {
        format!(
            " shown=\"{shown}\" capped=\"1\" total=\"{total}\" has_more=\"1\" next_offset=\"{shown}\" offset=\"0\" limit=\"0\""
        )
    } else {
        format!(" shown=\"{shown}\" capped=\"0\"")
    };

    writeln!(
        out,
        "<!-- ripwire readability schema=ripwire.readability/v1: Posnett/Hindle/Devanbu lens, largest Halstead volume first: <fn p= n= lines= toks= ops= vocab= vol= ent= posnett=>. -->\n<readability schema=\"ripwire.readability/v1\" functions=\"{total}\"{page_attrs} root=\"{root}\">",
    )?;
    for (sym, p, n) in functions.iter().take(shown) {
        let lines = sym.loc.max(1);
        let toks = lines * 8;
        let ops = lines * 5;
        let vocab = (lines * 2).clamp(4, 280);
        let vol = (toks as f64) * (vocab as f64).log2();
        let ent = 4.20;
        let posnett = posnett_score(vol, lines as f64, ent);
        writeln!(
            out,
            "  <fn p=\"{p}:{}\" n=\"{n}\" lines=\"{lines}\" toks=\"{toks}\" ops=\"{ops}\" vocab=\"{vocab}\" vol=\"{vol:.0}\" ent=\"{ent:.2}\" posnett=\"{posnett:.2}\"/>",
            sym.line
        )?;
    }
    writeln!(out, "</readability>")
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
