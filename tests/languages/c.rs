use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Interner, Lang, LanguageRegistry, SymKind, ingest_in_memory_files};
use std::collections::HashMap;

#[test]
fn test_c_graph_protocol_and_header_refinement() {
    verify_protocol_case(&GraphProtocolCase {
        label: "c",
        files: &[(
            "src/core.c",
            Lang::C,
            b"#include \"core.h\"\n#define EMPTY_FN(x)\nstatic const int MAX_CONN = 16;\nint plain_global = 0;\nvoid start_server(void) {\n    accept_loop();\n}\nvoid accept_loop(void) {\n    return;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "MAX_CONN", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "start_server", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "accept_loop", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["EMPTY_FN", "plain_global"],
        expected_edges: &[("start_server", "accept_loop")],
        expect_external: false,
        expected_lego: None,
        expected_import: Some("core.h"),
    });

    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let cpp_hdr =
        b"namespace rw { inline bool kindIs(const char* a, const char* b) { return true; } }\n";
    let objc_hdr = b"@interface Renderer : NSObject\n- (void)draw;\n@end\n";
    let pure_c_hdr = b"int c_header_fn(int a);\n";
    let ingest = ingest_in_memory_files(
        &[
            ("include/util.h", Lang::Cpp, cpp_hdr),
            ("include/objc.h", Lang::Cpp, objc_hdr),
            ("include/pure.h", Lang::C, pure_c_hdr),
        ],
        &registry,
        &interner,
    );
    assert_eq!(ingest.files[0].lang, Lang::Cpp);
    assert_eq!(ingest.files[1].lang, Lang::ObjC);
    assert_eq!(ingest.files[2].lang, Lang::C);

    let mut file_index = HashMap::new();
    file_index.insert("include/api.h", 7);
    let c_sem = Lang::C.semantics();
    assert_eq!(
        c_sem.resolve_import_target("src/main.c", "<stdio.h>", &file_index),
        None
    );
    assert_eq!(
        c_sem.resolve_import_target("src/main.c", "\"include/api.h\"", &file_index),
        Some(7)
    );
}
