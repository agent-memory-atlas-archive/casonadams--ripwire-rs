use ripwire_rs::cli::dispatch::{parse_from, run_cli_args};
use ripwire_rs::commands::gate::{GateChoice, select_gate};

const MISSING: &str = "/nonexistent-ripwire-gate-root";

#[test]
fn test_select_gate_follows_dispatch_precedence() {
    assert_eq!(
        select_gate(Some("a"), Some("b"), true, true, true),
        GateChoice::EditCheck("a")
    );
    assert_eq!(
        select_gate(None, Some("b"), true, true, true),
        GateChoice::Affected("b")
    );
    assert_eq!(
        select_gate(None, None, true, true, true),
        GateChoice::QualityBaseline
    );
    assert_eq!(
        select_gate(None, None, false, true, true),
        GateChoice::QualityDelta
    );
    assert_eq!(
        select_gate(None, None, false, false, true),
        GateChoice::Clones
    );
    assert_eq!(
        select_gate(None, None, false, false, false),
        GateChoice::Nothing
    );
}

#[test]
fn test_gate_subcommand_without_flags_runs_nothing() {
    let cli = parse_from(["ripwire-rs", "gate", MISSING]).unwrap();
    assert!(run_cli_args(cli).is_ok());
}

#[test]
fn test_gate_subcommand_routes_each_gate_to_its_runner_without_panic() {
    for argv in [
        vec!["ripwire-rs", "gate", MISSING, "--edit-check", "f"],
        vec!["ripwire-rs", "gate", MISSING, "--affected", "a.rs"],
        vec!["ripwire-rs", "gate", MISSING, "--quality-baseline"],
        vec!["ripwire-rs", "gate", MISSING, "--quality-delta"],
        vec!["ripwire-rs", "gate", MISSING, "--clones"],
    ] {
        let cli = parse_from(argv).unwrap();
        let _ = run_cli_args(cli);
    }
}
