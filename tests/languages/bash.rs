use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_bash_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "bash",
        files: &[(
            "scripts/deploy.sh",
            Lang::Bash,
            b"#!/usr/bin/env bash\nrun_deploy() {\n  LOCAL_VAR=1\n  build_image\n}\nbuild_image() {\n  echo ok\n}\n",
        )],
        expected_syms: &[
            sym("run_deploy", SymKind::Function, None, true),
            sym("build_image", SymKind::Function, None, true),
        ],
        rejected_syms: &["LOCAL_VAR"],
        expected_edges: &[("run_deploy", "build_image")],
        ..Default::default()
    });
    assert!(!Lang::Bash.semantics().supports_slice());
}

#[test]
fn test_bash_quoted_heredoc_terminator() {
    let cases: [(&str, Option<&str>); 7] = [
        ("python3 - \"$X\" <<'PY' | pbcopy", Some("PY")),
        ("cat <<-\"EOF\"", Some("EOF")),
        ("cat << 'END_SQL'", Some("END_SQL")),
        ("cat <<EOF", None),
        ("python3 -c '", None),
        ("echo $((1 << 2))", None),
        ("cat <<''", None),
    ];
    for (line, want) in cases {
        assert_eq!(
            Lang::Bash.semantics().quoted_heredoc_terminator(line),
            want,
            "{line}"
        );
    }
    assert_eq!(
        Lang::Python
            .semantics()
            .quoted_heredoc_terminator("x <<'PY'"),
        None
    );
}
