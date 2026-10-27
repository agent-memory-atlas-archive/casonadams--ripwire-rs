use super::harness::{GraphProtocolCase, owner_qualified_callees, sym, verify_protocol_case};
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

#[test]
fn ruby_trailing_hash_pairs_are_one_argument() {
    let source = "class Fake\n  def self.build_env(uri = \"\", opts = {})\n    1\n  end\nend\n\
def build\n  Fake.build_env(\"/\", \"HOST\" => \"example.org\", \"PORT\" => \"8080\")\nend\n\
def build_kw\n  Fake.build_env(\"/\", method: \"POST\", input: \"x\")\nend\n";
    let callees =
        |caller| owner_qualified_callees(&[("lib/fake.rb", Lang::Ruby, source.as_bytes())], caller);
    assert_eq!(callees("build"), ["Fake.build_env"]);
    assert_eq!(callees("build_kw"), ["Fake.build_env"]);
}

#[test]
fn ruby_receivers_built_with_new_pick_that_class_method() {
    let source = "module Web\n  class FakeRequest\n    def get(uri)\n      1\n    end\n  end\n  class Router\n    def get(uri)\n      2\n    end\n  end\n  class Logger\n    def write(line)\n      3\n    end\n  end\n  class Sink\n    def write(line)\n      4\n    end\n  end\nend\n\
class App\n  def initialize\n    @log = Web::Logger.new\n  end\n  def via_chain\n    Web::FakeRequest.new(self).get(\"/\")\n  end\n  def via_local\n    fake = Web::FakeRequest.new(self)\n    fake.get(\"/\")\n  end\n  def via_ivar\n    @log.write(\"x\")\n  end\nend\n";
    let callees =
        |caller| owner_qualified_callees(&[("lib/app.rb", Lang::Ruby, source.as_bytes())], caller);
    assert_eq!(callees("via_chain"), ["FakeRequest.get"]);
    assert_eq!(callees("via_local"), ["FakeRequest.get"]);
    assert_eq!(callees("via_ivar"), ["Logger.write"]);
}
