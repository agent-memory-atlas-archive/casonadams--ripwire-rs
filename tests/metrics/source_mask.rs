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

#[test]
fn python_division_stays_code_and_multiline_strings_stay_literal() {
    let source =
        "result = total // divisor\nlabel = '''quoted ' # total\nstill total'''\n# total\n";
    let masked = mask_source(source, Lang::Python);
    assert!(masked.code.contains("total // divisor"));
    assert!(!masked.code.contains("still total"));
    assert!(!masked.comments.contains("quoted"));
    assert_eq!(masked.comments.matches("total").count(), 1);
}

#[test]
fn rust_raw_literals_do_not_consume_following_code() {
    let source =
        "fn label<'a>() -> &'a str { r#\"quoted \" // TOKEN\"# }\nfn value() { consume(TOKEN); }\n";
    let masked = mask_source(source, Lang::Rust);
    assert!(masked.code.contains("fn label<'a>() -> &'a str"));
    assert!(!masked.code.contains("quoted"));
    assert!(masked.code.contains("consume(TOKEN)"));
    assert!(!masked.comments.contains("TOKEN"));
}

#[test]
fn interpolated_values_are_code_inside_template_literals() {
    let source = "const label = `TOKEN ${value(TOKEN)} suffix`;\n";
    let masked = mask_source(source, Lang::JavaScript);
    assert!(!masked.code.contains("`TOKEN"));
    assert!(masked.code.contains("value(TOKEN)"));
    assert!(!masked.code.contains("suffix"));
}

#[test]
fn configuration_values_are_literals_while_comments_remain_distinct() {
    for (lang, source) in [
        (Lang::Json, "{\"limit\": 8}\n"),
        (Lang::Toml, "limit = 8 # explanation\n"),
        (Lang::Yaml, "limit: 8 # explanation\n"),
    ] {
        let masked = mask_source(source, lang);
        assert!(masked.code.trim().is_empty(), "{lang:?}: {}", masked.code);
        assert_eq!(
            masked.comments.contains("explanation"),
            source.contains('#')
        );
    }
}
