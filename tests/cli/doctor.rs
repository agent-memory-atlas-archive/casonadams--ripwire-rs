use ripwire_rs::cli::doctor::{
    DoctorCheck, DoctorReport, classify_fsmonitor, is_cache_blob_name, probe_cache_artifact,
};

fn sample_report() -> DoctorReport {
    DoctorReport {
        at: "abc123def+dirty".to_string(),
        built_from: "0.1.0".to_string(),
        checks: vec![
            DoctorCheck::new("grammars", true)
                .attr("loaded", 26)
                .attr("expected", 26),
            DoctorCheck::new("cache-dir", false).attr("dir", "/tmp/a&b\"c"),
            DoctorCheck::new("layout", true),
        ],
    }
}

#[test]
fn test_doctor_passed_counts_ok_rows() {
    let report = sample_report();
    assert_eq!(report.passed(), 2);
    assert_eq!(report.checks.len(), 3);
}

#[test]
fn test_doctor_xml_rows_and_counts() {
    let mut out = Vec::new();
    sample_report().write_xml(&mut out).unwrap();
    let xml = String::from_utf8(out).unwrap();
    assert!(xml.starts_with("<!-- ripwire-rs doctor"));
    assert!(xml.contains(
        "<doctor schema=\"ripwire-rs.doctor/v1\" checks=\"3\" passed=\"2\" at=\"abc123def+dirty\" built_from=\"0.1.0\">"
    ));
    assert!(xml.contains("<c n=\"grammars\" ok=\"1\" loaded=\"26\" expected=\"26\"/>"));
    assert!(xml.contains("<c n=\"cache-dir\" ok=\"0\" dir=\"/tmp/a&amp;b&quot;c\"/>"));
    assert!(xml.contains("<c n=\"layout\" ok=\"1\"/>"));
    assert!(xml.trim_end().ends_with("</doctor>"));
    assert_eq!(xml.matches("<c n=\"").count(), 3);
}

#[test]
fn test_doctor_empty_report_passes_vacuously() {
    let report = DoctorReport {
        at: "HEAD".to_string(),
        built_from: "x".to_string(),
        checks: Vec::new(),
    };
    let mut out = Vec::new();
    report.write_xml(&mut out).unwrap();
    let xml = String::from_utf8(out).unwrap();
    assert!(xml.contains("checks=\"0\" passed=\"0\""));
}

#[test]
fn test_classify_fsmonitor() {
    assert_eq!(classify_fsmonitor(None), "unset");
    assert_eq!(classify_fsmonitor(Some("true")), "builtin");
    assert_eq!(classify_fsmonitor(Some("Yes")), "builtin");
    assert_eq!(classify_fsmonitor(Some("false")), "off");
    assert_eq!(classify_fsmonitor(Some("")), "off");
    assert_eq!(
        classify_fsmonitor(Some(".git/hooks/fsmonitor-watchman")),
        "hook"
    );
}

#[test]
fn test_is_cache_blob_name() {
    assert!(is_cache_blob_name("ripwire-rs-cache-1a2b.json"));
    assert!(!is_cache_blob_name("ripwire-rs-cache-1a2b.tmp"));
    assert!(!is_cache_blob_name("ripwire-rs-cache-.json"));
    assert!(!is_cache_blob_name("other.json"));
}

#[test]
fn test_probe_cache_artifact() {
    let current = format!(
        "{{\"version\":{},\"validation_key\":\"k\"}}",
        ripwire_rs::cache::CACHE_VERSION
    );
    assert_eq!(probe_cache_artifact(current.as_bytes()), "ok");
    let stale = format!("{{\"version\":{}}}", ripwire_rs::cache::CACHE_VERSION + 1);
    assert_eq!(probe_cache_artifact(stale.as_bytes()), "cache-version");
    assert_eq!(probe_cache_artifact(&b"not json"[..]), "corrupt");
}
