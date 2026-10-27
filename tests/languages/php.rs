use super::harness::{GraphProtocolCase, owner_qualified_callees, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_php_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "php",
        files: &[(
            "src/Auth.php",
            Lang::Php,
            b"<?php\nuse App\\Contracts\\Guard;\ninterface Guard {}\nclass TokenGuard implements Guard {\n    public function check() {\n        verifyToken();\n    }\n}\nfunction verifyToken() {\n    return true;\n}\n",
        )],
        expected_syms: &[
            sym("Guard", SymKind::Interface, None, true),
            sym("TokenGuard", SymKind::Class, None, true),
            sym("check", SymKind::Method, Some("TokenGuard"), true),
            sym("verifyToken", SymKind::Function, None, true),
        ],
        expected_edges: &[("check", "verifyToken")],
        expected_lego: Some(("Guard", "TokenGuard")),
        expected_import: Some("Guard"),
        ..Default::default()
    });
    assert!(Lang::Php.semantics().is_std_or_builtin_member("explode"));
}

const PHP_STORES: &str = "<?php\nclass DiskStore { public function open() { return 1; } }\n\
class MemoryStore { public function open() { return 2; } }\n\
class ReadFailed { public static function forPath($p) { return new self(); } }\n\
class WriteFailed { public static function forPath($p) { return new self(); } }\n";

const PHP_SERVICE: &str = "<?php\nclass Service {\n  private DiskStore $store;\n\
  public function viaThisField() { $this->store->open(); }\n\
  public function viaParam(MemoryStore $memory) { $memory->open(); }\n\
  public function viaLocal() { $local = new DiskStore(); $local->open(); }\n\
  public function viaStatic() { throw WriteFailed::forPath('p'); }\n}\n";

#[test]
fn php_typed_receivers_and_static_calls_pick_the_named_class_method() {
    let callees = |caller| {
        owner_qualified_callees(
            &[
                ("src/Stores.php", Lang::Php, PHP_STORES.as_bytes()),
                ("src/Service.php", Lang::Php, PHP_SERVICE.as_bytes()),
            ],
            caller,
        )
    };
    assert_eq!(callees("viaThisField"), ["DiskStore.open"]);
    assert_eq!(callees("viaParam"), ["MemoryStore.open"]);
    assert!(callees("viaLocal").contains(&"DiskStore.open".to_string()));
    assert!(!callees("viaLocal").contains(&"MemoryStore.open".to_string()));
    assert_eq!(callees("viaStatic"), ["WriteFailed.forPath"]);
}
