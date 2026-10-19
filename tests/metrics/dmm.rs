use ripwire_rs::QualitySnapshot;
use ripwire_rs::metrics::{compute_dmm_report, serialize_dmm_xml};

#[test]
fn test_dmm_calculations() {
    let mut base = QualitySnapshot::default();
    base.loc_by_sym.insert("fn1".into(), 10);
    base.cx_by_sym.insert("fn1".into(), 2);
    base.params_by_sym.insert("fn1".into(), 1);

    let mut target = QualitySnapshot::default();
    target.loc_by_sym.insert("fn1".into(), 10);
    target.cx_by_sym.insert("fn1".into(), 2);
    target.params_by_sym.insert("fn1".into(), 1);

    target.loc_by_sym.insert("god_fn".into(), 100);
    target.cx_by_sym.insert("god_fn".into(), 20);
    target.params_by_sym.insert("god_fn".into(), 10);

    let report = compute_dmm_report(&base, &target, "working-tree", "at");
    assert!(report.dmm < 0.5);
    assert_eq!(report.properties.len(), 3);

    let mut buf = Vec::new();
    serialize_dmm_xml(&report, &mut buf).unwrap();
    let xml = String::from_utf8(buf).unwrap();
    assert!(xml.contains("schema=\"ripwire-rs.dmm/v1\""));

    let mut b2 = QualitySnapshot::default();
    b2.loc_by_sym.insert("f".into(), 12);
    b2.cx_by_sym.insert("f".into(), 3);
    b2.params_by_sym.insert("f".into(), 1);
    let mut t2 = b2.clone();
    t2.cx_by_sym.insert("f".into(), 9);
    let rep2 = compute_dmm_report(&b2, &t2, "working-tree", "at");
    assert_eq!(rep2.properties[1].bad, 24);
    assert_eq!(rep2.properties[1].d_low, -12);
    assert_eq!(rep2.properties[1].d_high, 12);
}
