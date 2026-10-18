use ripwire_rs::metrics::lint::{SYMBOL_LINT_RULES, scan_file_rules};
use ripwire_rs::metrics::lint_rules::FILE_LINT_RULES;
use ripwire_rs::metrics::{LINT_RULES, LintFinding, serialize_lint_xml};
use ripwire_rs::{Interner, Lang, SymKind, Symbol};
use std::collections::HashSet;

#[test]
fn test_every_catalog_rule_is_backed_by_a_trait_evaluator() {
    let mut trait_names: Vec<&str> = SYMBOL_LINT_RULES
        .iter()
        .map(|r| r.desc().name)
        .chain(FILE_LINT_RULES.iter().map(|r| r.desc().name))
        .collect();
    let mut catalog_names: Vec<&str> = LINT_RULES.iter().map(|r| r.name).collect();
    assert_eq!(trait_names.len(), catalog_names.len());
    trait_names.sort_unstable();
    catalog_names.sort_unstable();
    assert_eq!(trait_names, catalog_names);
}

#[test]
fn test_lint_report_capped_rule_names() {
    let findings: Vec<LintFinding> = (0..5000)
        .map(|i| LintFinding {
            rule: "magic-number".to_string(),
            message: "magic".to_string(),
            path: "src/a.rs".to_string(),
            line: i,
            severity: "info".to_string(),
            enclosing: "main".to_string(),
        })
        .collect();
    let mut buf = Vec::new();
    serialize_lint_xml(".", &findings, &mut buf).unwrap();
    let xml = String::from_utf8(buf).unwrap();
    assert!(xml.contains("findings_capped=\"1\""));
    assert!(
        xml.contains(
            "findings_next=\"--lint --lint-select=magic-number --lint-max-per-rule=50000\""
        )
    );
    assert!(xml.contains("count_capped=\"1\""));
}

#[test]
fn test_line_patterns_ignore_literals_and_comments_and_keep_line_numbers() {
    let src =
        "const char* s = \"strcpy(\"; // goto done\n/* std::map<int,int> */\nstrcpy(dst, s);\n";
    let mut findings = Vec::new();
    scan_file_rules("src/a.c", src, Lang::C, Vec::new(), &mut findings);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "unsafe-c-fn");
    assert_eq!(findings[0].line, 3);
    assert_eq!(findings[0].enclosing, "<top>");
}

#[test]
fn test_file_lint_rules_fire_across_categories() {
    let cpp_src = "\
typedef struct Foo { int x; } Foo;\n\
void work(std::shared_ptr<int> sp) {\n\
    int a = (int)y;\n\
    if ((a)) ;\n\
    if (a = 1) { return; }\n\
    if (strcmp(s, t)) { return 42; }\n\
    a = a;\n\
    int oct = 0755;\n\
    int rev = 1[arr];\n\
    int tern = a ? b : c ? d : e;\n\
    int crem = a++ + 1;\n\
    a = 1, b = 2;\n\
    std::vector<int*> raw;\n\
    std::vector<std::unique_ptr<int>> ind;\n\
    std::map<int, int> mp;\n\
    for (int i = 0; i < 10; ++i) {\n\
        int* p = new int(1);\n\
        n = n->next;\n\
        int g = arr[idx[i]];\n\
    }\n\
    do {\n\
        _mm_prefetch(p, 0);\n\
    } while (0);\n\
    try { MD5(buf); } catch {}\n\
    goto done;\n\
    a = 99;\n\
done:\n\
    return;\n\
}\n";
    let interner = Interner::new();
    let mut sym = Symbol::new(
        0,
        interner.intern("work"),
        interner.intern("src/a.cpp::work"),
        SymKind::Function,
        Lang::Cpp,
        0,
        2,
    );
    sym.loc = 25;
    sym.sig_start_byte = 35;
    sym.sig_end_byte = 70;
    sym.end_byte = cpp_src.len() as u32;
    let mut findings = Vec::new();
    scan_file_rules(
        "src/a.cpp",
        cpp_src,
        Lang::Cpp,
        vec![(&sym, "work")],
        &mut findings,
    );
    let fired: HashSet<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
    for expected in [
        "typedef-over-using",
        "c-style-cast",
        "redundant-parens",
        "suspicious-semicolon",
        "atom-assign-as-value",
        "atom-implicit-predicate",
        "self-assign",
        "atom-octal-literal",
        "atom-reversed-subscript",
        "atom-nested-ternary",
        "atom-embedded-crement",
        "atom-comma-operator",
        "cache-vector-of-raw-ptr",
        "cache-vector-of-indirect",
        "cache-node-container",
        "cache-heap-alloc-in-loop",
        "cache-pointer-chase-loop",
        "cache-gather-subscript",
        "cache-shared-ptr-by-value",
        "do-while",
        "cache-manual-prefetch",
        "weak-crypto",
        "empty-catch",
        "goto",
        "unreachable-code",
        "inconsistent-return",
        "magic-number",
    ] {
        assert!(fired.contains(expected), "missing rule {expected}");
    }
}

#[test]
fn test_naming_file_lint_rules() {
    let rs_src = "fn is_ready() -> u32 { 1 }\nfn set_mode() -> u32 { 1 }\nfn handle_data() {\n}\n";
    let interner = Interner::new();
    let mk = |id, name: &str, line, loc, s0, s1, end| {
        let mut s = Symbol::new(
            id,
            interner.intern(name),
            interner.intern(name),
            SymKind::Function,
            Lang::Rust,
            0,
            line,
        );
        s.loc = loc;
        s.sig_start_byte = s0;
        s.sig_end_byte = s1;
        s.end_byte = end;
        s
    };
    let s1 = mk(0, "is_ready", 1, 1, 0, 20, 26);
    let s2 = mk(1, "set_mode", 2, 1, 27, 47, 53);
    let s3 = mk(2, "handle_data", 3, 25, 54, 70, 75);
    let s4 = mk(3, "step1", 4, 1, 0, 0, 0);
    let s5 = mk(4, "step2", 5, 1, 0, 0, 0);
    let s6 = mk(5, "parse_node", 6, 1, 0, 0, 0);
    let s7 = mk(6, "parse_mode", 7, 1, 0, 0, 0);
    let mut findings = Vec::new();
    scan_file_rules(
        "src/a.rs",
        rs_src,
        Lang::Rust,
        vec![
            (&s1, "is_ready"),
            (&s2, "set_mode"),
            (&s3, "handle_data"),
            (&s4, "step1"),
            (&s5, "step2"),
            (&s6, "parse_node"),
            (&s7, "parse_mode"),
        ],
        &mut findings,
    );
    let fired: HashSet<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
    for expected in [
        "naming-predicate",
        "naming-setter",
        "naming-uninformative",
        "naming-series",
        "naming-confusable",
    ] {
        assert!(fired.contains(expected), "missing rule {expected}");
    }
}
