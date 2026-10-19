use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_ruby_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "ruby",
        files: &[(
            "lib/job.rb",
            Lang::Ruby,
            b"class BaseJob\nend\nclass MailJob < BaseJob\n  MAX_RETRY = 3\n  AliasType = String\n  def perform\n    File.read('a')\n    deliver()\n  end\n  def deliver\n    true\n  end\nend\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "MailJob", kind: SymKind::Class, scope: None, has_body_span: false },
            ExpectedSym { name: "MAX_RETRY", kind: SymKind::Var, scope: Some("MailJob"), has_body_span: false },
            ExpectedSym { name: "perform", kind: SymKind::Method, scope: Some("MailJob"), has_body_span: true },
            ExpectedSym { name: "deliver", kind: SymKind::Method, scope: Some("MailJob"), has_body_span: true },
        ],
        rejected_syms: &["AliasType"],
        expected_edges: &[("perform", "deliver")],
        expect_external: true,
        expected_lego: Some(("BaseJob", "MailJob")),
        expected_import: None,
    });
    assert!(Lang::Ruby.semantics().is_std_or_builtin_member("each"));
}
