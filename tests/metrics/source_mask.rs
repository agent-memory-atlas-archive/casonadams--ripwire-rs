use ripwire_rs::Lang;
use ripwire_rs::metrics::source_mask::mask_source;

#[test]
fn test_mask_separates_code_comments_and_literals_per_line() {
    let src = "let a = \"strcpy(\"; // TODO: fix\nstrcpy(x); /* catch {} */\n";
    let m = mask_source(src, Lang::C);
    assert_eq!(m.code.len(), src.len());
    let code: Vec<&str> = m.code.lines().collect();
    let comments: Vec<&str> = m.comments.lines().collect();
    assert_eq!(code.len(), 2);
    assert!(!code[0].contains("strcpy("));
    assert!(!code[0].contains("TODO"));
    assert!(comments[0].contains("TODO: fix"));
    assert!(code[1].contains("strcpy(x)"));
    assert!(!code[1].contains("catch"));
    assert!(comments[1].contains("catch {}"));
}

#[test]
fn test_mask_rust_lifetimes_stay_code_and_chars_are_literals() {
    let src = "fn f<'a>(s: &'a str) -> char { 'x' } // todo!(\nlet q = '\\'';\n";
    let m = mask_source(src, Lang::Rust);
    assert!(m.code.contains("fn f<'a>(s: &'a str)"));
    assert!(!m.code.contains("'x'"));
    assert!(!m.code.contains("todo!("));
    assert!(m.comments.contains("todo!("));
    assert!(!m.code.lines().nth(1).unwrap().contains('\''));
}

#[test]
fn test_mask_hash_comments_only_for_hash_languages() {
    let py = mask_source("x = 1  # FIXME: later\n", Lang::Python);
    assert!(py.comments.contains("FIXME: later"));
    let c = mask_source("#include <a.h>\n", Lang::C);
    assert!(c.code.contains("#include"));
}
