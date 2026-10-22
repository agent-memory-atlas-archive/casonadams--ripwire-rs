use ripwire_rs::commands::gate::{
    GateArgs, GateChoice, GateRequest, TopGate, merge_request, select_gate,
};

fn request<'a>() -> GateRequest<'a> {
    GateRequest {
        edit_check: None,
        affected: None,
        quality_baseline: false,
        quality_delta: false,
        clones: false,
    }
}

#[test]
fn test_select_gate_follows_dispatch_precedence() {
    let all = GateRequest {
        edit_check: Some("a"),
        affected: Some("b"),
        quality_baseline: true,
        quality_delta: true,
        clones: true,
    };
    assert_eq!(select_gate(&all), Some(GateChoice::EditCheck("a")));
    let without = |edit, affected, baseline, delta| GateRequest {
        edit_check: edit,
        affected,
        quality_baseline: baseline,
        quality_delta: delta,
        clones: true,
    };
    assert_eq!(
        select_gate(&without(None, Some("b"), true, true)),
        Some(GateChoice::Affected("b"))
    );
    assert_eq!(
        select_gate(&without(None, None, true, true)),
        Some(GateChoice::QualityBaseline)
    );
    assert_eq!(
        select_gate(&without(None, None, false, true)),
        Some(GateChoice::QualityDelta)
    );
    assert_eq!(
        select_gate(&without(None, None, false, false)),
        Some(GateChoice::Clones)
    );
    assert_eq!(select_gate(&request()), None);
}

#[test]
fn test_merge_prefers_subcommand_flags_and_falls_back_to_top_level() {
    let mut args = GateArgs::default();
    let mut top = TopGate::default();
    assert_eq!(merge_request(&args, &top), request());

    top.edit.edit_check = Some("top".into());
    top.baseline.primary.quality_baseline = true;
    top.delta.opts.quality_ack = Some(None);
    let merged = merge_request(&args, &top);
    assert_eq!(merged.edit_check, Some("top"));
    assert!(merged.quality_baseline && merged.quality_delta);

    args.edit_check.edit_check = Some("sub".into());
    args.clones.clones = true;
    let merged = merge_request(&args, &top);
    assert_eq!(merged.edit_check, Some("sub"));
    assert!(merged.clones);
}
