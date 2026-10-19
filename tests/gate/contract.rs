use ripwire_rs::gate::contract::{
    ContractStatus, SignatureContract, check_node_visibility, compare_contracts,
    extract_param_count, parse_head_signature_match,
};
use ripwire_rs::{Lang, LanguageRegistry};
use streaming_iterator::StreamingIterator;

#[test]
fn test_compare_contracts_scenarios() {
    let baseline = SignatureContract {
        name: "fetch".to_string(),
        params: 2,
        is_public: false,
    };

    let same = SignatureContract {
        name: "fetch".to_string(),
        params: 2,
        is_public: false,
    };
    assert_eq!(
        compare_contracts(&same, Some(&baseline)),
        ContractStatus::Unchanged
    );

    let arity_broken = SignatureContract {
        name: "fetch".to_string(),
        params: 3,
        is_public: false,
    };
    assert_eq!(
        compare_contracts(&arity_broken, Some(&baseline)),
        ContractStatus::ContractChange {
            param_changed: true,
            public_changed: false,
            was_params: 2,
            now_params: 3,
            was_public: false,
            now_public: false,
        }
    );

    let pub_changed = SignatureContract {
        name: "fetch".to_string(),
        params: 2,
        is_public: true,
    };
    assert_eq!(
        compare_contracts(&pub_changed, Some(&baseline)),
        ContractStatus::ContractChange {
            param_changed: false,
            public_changed: true,
            was_params: 2,
            now_params: 2,
            was_public: false,
            now_public: true,
        }
    );

    assert_eq!(compare_contracts(&same, None), ContractStatus::NewSymbol);
}

fn parse_rust_fn_stats(code: &[u8]) -> (u16, bool) {
    let reg = LanguageRegistry::new().unwrap();
    let (ts_lang, _) = reg.get(Lang::Rust).unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(ts_lang).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let fn_node = tree.root_node().child(0).unwrap();
    (
        extract_param_count(fn_node),
        check_node_visibility(fn_node, code, Lang::Rust),
    )
}

#[test]
fn test_extract_param_and_visibility() {
    assert_eq!(
        parse_rust_fn_stats(b"pub fn example(a: i32, b: i32) {}"),
        (2, true)
    );
}

#[test]
fn test_extract_param_count_ignores_generic_type_parameters() {
    let code = b"pub fn collect_with_rank_and_limit<E, H>(&self, rank: Option<u32>, limit: Option<usize>, heartbeat: H) {}";
    assert_eq!(parse_rust_fn_stats(code).0, 4);
}

#[test]
fn test_dart_private_method_visibility_and_named_params() {
    let dart_code = br#"
class SwapRecord {
  SwapRecord(this.a, this.b, {required this.c, this.d, this.e});
  Future<void> run() async {
    await _prepare("first", y: 2);
  }
  Future<void> _prepare(String x, {int y = 1, bool z = false}) async {}
}
void makeSwap() {
  SwapRecord(1, 2, c: 3, d: 4);
}
"#;
    let reg = LanguageRegistry::new().unwrap();
    let (ts_lang, query) = reg.get(Lang::Dart).unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(ts_lang).unwrap();
    let tree = parser.parse(dart_code, None).unwrap();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), dart_code.as_slice());

    let mut found_prepare = false;
    let mut found_ctor = false;
    while let Some(m) = matches.next() {
        let Some((name, params, is_pub)) =
            parse_head_signature_match(m, query, dart_code, Lang::Dart)
        else {
            continue;
        };
        if name == "_prepare" {
            found_prepare = true;
            assert!(!is_pub, "private Dart method _prepare must not be public");
            assert_eq!(params, 3, "1 positional + 2 named parameters");
        } else if name == "SwapRecord" && params > 0 {
            found_ctor = true;
            assert!(is_pub);
            assert_eq!(params, 5, "2 positional + 3 named parameters");
        }
    }
    assert!(found_prepare);
    assert!(found_ctor);
}
