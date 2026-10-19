use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("MailJob", SymKind::Class, None, false),
            sym("MAX_RETRY", SymKind::Var, Some("MailJob"), false),
            sym("perform", SymKind::Method, Some("MailJob"), true),
            sym("deliver", SymKind::Method, Some("MailJob"), true),
        ],
        rejected_syms: &["AliasType"],
        expected_edges: &[("perform", "deliver")],
        expect_external: true,
        expected_lego: Some(("BaseJob", "MailJob")),
        ..Default::default()
    });
    assert!(Lang::Ruby.semantics().is_std_or_builtin_member("each"));
}
