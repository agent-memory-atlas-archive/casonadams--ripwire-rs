use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_cobol_graph_protocol() {
    let code = b"       IDENTIFICATION DIVISION.\n       PROGRAM-ID. PAYROLL.\n       PROCEDURE DIVISION.\n       MAIN-LOGIC.\n           PERFORM CALC-TAX.\n           STOP RUN.\n       CALC-TAX.\n           DISPLAY \"CALCULATING\".\n";
    verify_protocol_case(&GraphProtocolCase {
        label: "cobol",
        files: &[("src/payroll.cbl", Lang::Cobol, code)],
        expected_syms: &[
            sym("MAIN-LOGIC", SymKind::Function, None, false),
            sym("CALC-TAX", SymKind::Function, None, false),
        ],
        expected_edges: &[("MAIN-LOGIC", "CALC-TAX")],
        ..Default::default()
    });
    assert!(
        Lang::Cobol
            .semantics()
            .is_std_or_builtin_member("current-date")
    );
    assert!(
        Lang::Cobol
            .semantics()
            .is_std_or_builtin_qualifier("sysout")
    );
    assert_eq!(Lang::from_extension("cbl"), Lang::Cobol);
    assert_eq!(Lang::from_extension("cob"), Lang::Cobol);
    assert_eq!(Lang::from_extension("cpy"), Lang::Cobol);
}
