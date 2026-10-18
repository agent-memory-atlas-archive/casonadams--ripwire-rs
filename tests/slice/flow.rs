use ripwire_rs::{OccRole, SliceFlowDir, VariableOcc, compute_dataflow_slice};

#[test]
fn test_flow_forward_and_backward() {
    let occs = vec![
        VariableOcc {
            line: 1,
            stmt_line: 1,
            start_byte: 0,
            end_byte: 1,
            role: OccRole::Decl,
            var_name: "x".into(),
        },
        VariableOcc {
            line: 3,
            stmt_line: 3,
            start_byte: 10,
            end_byte: 11,
            role: OccRole::Read,
            var_name: "x".into(),
        },
        VariableOcc {
            line: 3,
            stmt_line: 3,
            start_byte: 12,
            end_byte: 13,
            role: OccRole::Assign,
            var_name: "y".into(),
        },
        VariableOcc {
            line: 5,
            stmt_line: 5,
            start_byte: 20,
            end_byte: 21,
            role: OccRole::Read,
            var_name: "y".into(),
        },
    ];

    let fwd_steps = compute_dataflow_slice(&occs, "x", SliceFlowDir::Fwd, 8);
    assert_eq!(fwd_steps[0].line, 1);
    assert_eq!(fwd_steps[0].var_name, "x");
    assert_eq!(fwd_steps[1].line, 3);

    let back_steps = compute_dataflow_slice(&occs, "y", SliceFlowDir::Back, 8);
    assert!(back_steps.iter().any(|s| s.line == 3 && s.var_name == "y"));
}
