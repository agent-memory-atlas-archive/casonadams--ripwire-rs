use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};
use std::collections::HashMap;

#[test]
fn test_rust_graph_protocol_and_imports() {
    verify_protocol_case(&GraphProtocolCase {
        label: "rust",
        files: &[(
            "src/lib.rs",
            Lang::Rust,
            b"use crate::util::helper;\npub trait Runner { fn execute(&self); }\npub struct Engine;\nimpl Runner for Engine {\n    pub fn execute(&self) {\n        let local_noise = 1;\n        println!(\"{}\", local_noise);\n        std::mem::drop(local_noise);\n        step();\n    }\n}\npub fn step() {}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Runner", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "Engine", kind: SymKind::Class, scope: None, has_body_span: false },
            ExpectedSym { name: "execute", kind: SymKind::Method, scope: Some("Engine"), has_body_span: true },
            ExpectedSym { name: "step", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["local_noise"],
        expected_edges: &[("execute", "step")],
        expect_external: true,
        expected_lego: Some(("Runner", "Engine")),
        expected_import: Some("crate::util::helper"),
    });

    let mut file_index = HashMap::new();
    file_index.insert("crates/core/src/sub/worker.rs", 4);
    file_index.insert("crates/core/src/common.rs", 5);
    let sem = Lang::Rust.semantics();
    assert_eq!(
        sem.resolve_import_target(
            "crates/core/src/lib.rs",
            "crate::sub::worker::Run",
            &file_index
        ),
        Some(4)
    );
    assert_eq!(
        sem.resolve_import_target("crates/core/src/sub/mod.rs", "self::worker", &file_index),
        Some(4)
    );
    assert_eq!(
        sem.resolve_import_target("crates/core/src/sub/mod.rs", "super::common", &file_index),
        Some(5)
    );
    assert_eq!(
        sem.resolve_import_target("crates/cli/src/main.rs", "core::common::Item", &file_index),
        Some(5)
    );
    assert!(sem.is_std_or_builtin_qualifier("std"));
    assert!(sem.is_std_or_builtin_member("unwrap"));
    assert!(sem.is_entrypoint_symbol("main", None, SymKind::Function, "src/main.rs"));
    assert!(!sem.is_entrypoint_symbol("main", None, SymKind::Function, "examples/demo.rs"));
}
