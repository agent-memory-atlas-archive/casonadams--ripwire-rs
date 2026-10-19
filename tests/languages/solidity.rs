use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_solidity_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "solidity",
        files: &[(
            "contracts/Vault.sol",
            Lang::Solidity,
            b"interface IVault {}\ncontract Vault is IVault {\n    function deposit() public {\n        abi.encode();\n        recordDeposit();\n    }\n    function recordDeposit() internal {\n        return;\n    }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "IVault", kind: SymKind::Interface, scope: None, has_body_span: false },
            ExpectedSym { name: "Vault", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "deposit", kind: SymKind::Function, scope: Some("Vault"), has_body_span: true },
            ExpectedSym { name: "recordDeposit", kind: SymKind::Function, scope: Some("Vault"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("deposit", "recordDeposit")],
        expect_external: true,
        expected_lego: Some(("IVault", "Vault")),
        expected_import: None,
    });
}
