use ripwire_rs::metrics::nonlocal::{
    NonlocalFunction, StateCellAccess, serialize_nonlocal_state_xml,
};

#[test]
fn test_serialize_nonlocal_state_rows_and_cells() {
    let funcs = vec![NonlocalFunction {
        sym_name: "mutate_global".to_string(),
        path: "src/state.rs".to_string(),
        line: 10,
        writes: 1,
        reads: 1,
        cells: vec![
            StateCellAccess {
                cell_name: "GLOBAL_COUNTER".to_string(),
                path: "src/state.rs".to_string(),
                line: 3,
                is_write: true,
            },
            StateCellAccess {
                cell_name: "CONFIG".to_string(),
                path: "src/state.rs".to_string(),
                line: 4,
                is_write: false,
            },
        ],
    }];
    let mut out = Vec::new();
    serialize_nonlocal_state_xml(".", &funcs, &mut out).unwrap();
    let xml = String::from_utf8(out).unwrap();
    assert!(xml.contains("cells=\"2\" functions=\"1\""));
    assert!(xml.contains("<cell n=\"GLOBAL_COUNTER\" p=\"src/state.rs:3\" dir=\"w\"/>"));
    assert!(xml.contains("<cell n=\"CONFIG\" p=\"src/state.rs:4\" dir=\"r\"/>"));
}
