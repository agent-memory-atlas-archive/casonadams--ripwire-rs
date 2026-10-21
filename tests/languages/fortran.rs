use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_fortran_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "fortran",
        files: &[(
            "src/math.f90",
            Lang::Fortran,
            b"module math_ops\ncontains\n  subroutine compute(x)\n    integer :: x\n    call helper(x)\n  end subroutine compute\n  subroutine helper(y)\n    integer :: y\n  end subroutine helper\nend module math_ops\n",
        )],
        expected_syms: &[
            sym("math_ops", SymKind::Section, None, false),
            sym("compute", SymKind::Function, None, false),
            sym("helper", SymKind::Function, None, false),
        ],
        expected_edges: &[("compute", "helper")],
        ..Default::default()
    });
    assert!(Lang::Fortran.semantics().is_std_or_builtin_member("matmul"));
    assert!(
        Lang::Fortran
            .semantics()
            .is_std_or_builtin_qualifier("iso_c_binding")
    );
    assert_eq!(Lang::from_extension("f90"), Lang::Fortran);
    assert_eq!(Lang::from_extension("f"), Lang::Fortran);
}
