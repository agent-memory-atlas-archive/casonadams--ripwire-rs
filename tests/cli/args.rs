use ripwire_rs::DEFAULT_PACK_BUDGET_BYTES;
use ripwire_rs::cli::compute_effective_budget;
use ripwire_rs::commands::common::gate::validate_gate_modifiers;
use ripwire_rs::query::graph::split_endpoint_pair;

#[test]
fn test_gate_modifiers_refused_without_their_verb() {
    assert!(validate_gate_modifiers(true, false, false, false).is_err());
    assert!(validate_gate_modifiers(false, false, true, false).is_err());
    assert!(validate_gate_modifiers(true, true, true, true).is_ok());
}

#[test]
fn test_compute_effective_budget() {
    assert_eq!(compute_effective_budget(Some(1024), None, None), Some(1024));
    assert_eq!(compute_effective_budget(None, Some(100), None), Some(400));
    assert_eq!(compute_effective_budget(None, None, Some(50)), Some(200));
    assert_eq!(
        compute_effective_budget(None, None, None),
        Some(DEFAULT_PACK_BUDGET_BYTES)
    );
    assert_eq!(split_endpoint_pair("a,b"), ("a", "b"));
    assert_eq!(split_endpoint_pair("a:b"), ("a", "b"));
    assert_eq!(split_endpoint_pair("a->b"), ("a", "b"));
    assert_eq!(split_endpoint_pair("single"), ("single", ""));
}
