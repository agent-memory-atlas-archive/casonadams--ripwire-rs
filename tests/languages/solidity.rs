use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("IVault", SymKind::Interface, None, false),
            sym("Vault", SymKind::Class, None, true),
            sym("deposit", SymKind::Function, Some("Vault"), true),
            sym("recordDeposit", SymKind::Function, Some("Vault"), true),
        ],
        expected_edges: &[("deposit", "recordDeposit")],
        expect_external: true,
        expected_lego: Some(("IVault", "Vault")),
        ..Default::default()
    });
}
