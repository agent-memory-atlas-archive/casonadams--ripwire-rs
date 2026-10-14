pub mod analysis_lenses;
pub mod complexity;
pub mod dmm;
pub mod eval;
pub mod hotspots;
pub mod layout;
pub mod lint;
pub mod nonlocal;
pub mod quality_panel;
pub mod readability;

pub use analysis_lenses::{
    classify_identifier_casing, compute_comment_coherence, compute_context_ratio, compute_ensemble,
    serialize_comment_coherence_xml, serialize_context_ratio_xml, serialize_ensemble_xml,
    serialize_executive_report_md, serialize_naming_calibration_xml,
    serialize_naming_consistency_xml,
};
pub use dmm::{DmmProperty, DmmReport, compute_dmm_report, serialize_dmm_xml};
pub use eval::{run_cochange_eval, run_retrieval_eval};
pub use layout::{
    FieldLayout, StructLayoutDef, StructLayoutReport, compute_layout_report,
    serialize_field_affinity_xml, serialize_layout_xml,
};
pub use quality_panel::{
    QualityPanelReport, QualityPanelRow, compute_quality_panel_report, serialize_quality_panel_xml,
};

pub use complexity::{
    ComplexityMetrics, NESTING_BAR, compute_complexity, compute_complexity_from_node,
};
pub use hotspots::{HotspotFile, HotspotsReport, compute_hotspots};
pub use lint::{
    LINT_RULES, LintFinding, LintRuleDesc, run_lint_scan, serialize_lint_catalog,
    serialize_lint_sarif, serialize_lint_xml,
};
pub use nonlocal::{NonlocalFunction, find_nonlocal_state, serialize_nonlocal_state_xml};
pub use readability::{
    ReadabilityMetrics, compute_readability, execute_biggest_first_query, posnett_score,
};
