use ripwire_rs::DEFAULT_PACK_BUDGET_BYTES;
use ripwire_rs::cli::{GateDispatchArgs, compute_effective_budget, validate_gate_modifiers};
use ripwire_rs::query::graph::split_endpoint_pair;

#[test]
fn test_gate_modifiers_refused_without_their_verb() {
    let lone_dirty = GateDispatchArgs {
        allow_dirty: true,
        ..Default::default()
    };
    assert!(validate_gate_modifiers(&lone_dirty).is_err());
    let lone_stale = GateDispatchArgs {
        show_stale: true,
        ..Default::default()
    };
    assert!(validate_gate_modifiers(&lone_stale).is_err());
    let paired = GateDispatchArgs {
        quality_baseline: true,
        allow_dirty: true,
        quality_delta: true,
        show_stale: true,
        ..Default::default()
    };
    assert!(validate_gate_modifiers(&paired).is_ok());
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
