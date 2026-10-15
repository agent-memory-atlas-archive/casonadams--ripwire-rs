pub mod args;
pub mod dispatch;
pub mod queries;
pub mod safety;
pub mod stream;
pub mod wrap;

pub use args::*;
pub use dispatch::*;
pub use queries::*;
pub use safety::*;
pub use stream::*;
pub use wrap::*;
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[test]
    fn test_cli_parsing() {
        let help = Cli::try_parse_from(["rw", "--help"]);
        assert!(help.is_err()); // --help exits with help message

        let run = Cli::try_parse_from([
            "rw",
            ".",
            "--top-k",
            "150",
            "--max-tokens",
            "4000",
            "--legend",
            "none",
        ])
        .unwrap();
        assert_eq!(run.path, Some(".".into()));
        assert_eq!(run.top_k, Some(150));
        assert_eq!(run.max_tokens, Some(4000));
        assert_eq!(run.legend, "none");
        let mcp_cmd = Cli::try_parse_from(["rw", "mcp"]).unwrap();
        assert_eq!(mcp_cmd.command, Some(Commands::Mcp(McpArgs::default())));

        let nav_cmd =
            Cli::try_parse_from(["rw", "navigate", "--symbol", "main", "--callers"]).unwrap();
        assert!(matches!(nav_cmd.command, Some(Commands::Navigate(_))));

        let orient_cmd = Cli::try_parse_from(["rw", "orient", "--for", "auth"]).unwrap();
        assert!(matches!(orient_cmd.command, Some(Commands::Orient(_))));
    }

    #[test]
    fn test_gate_dispatch_args_is_active() {
        assert!(!GateDispatchArgs::default().is_active());
        assert!(dispatch_gate_command(".", GateDispatchArgs::default()).is_none());
        let chk = |e, a, b, d| {
            GateDispatchArgs {
                edit_check: e,
                affected: a,
                quality_baseline: b,
                quality_delta: d,
            }
            .is_active()
        };
        assert!(chk(Some("m"), None, false, false) && chk(None, Some("f"), false, false));
        assert!(chk(None, None, true, false) && chk(None, None, false, true));
    }

    #[test]
    fn test_compute_effective_budget() {
        assert_eq!(compute_effective_budget(Some(1024), None, None), Some(1024));
        assert_eq!(compute_effective_budget(None, Some(100), None), Some(400));
        assert_eq!(compute_effective_budget(None, None, Some(50)), Some(200));
        assert_eq!(
            compute_effective_budget(None, None, None),
            Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES)
        );
        assert_eq!(split_endpoint_pair("a,b"), ("a", "b"));
        assert_eq!(split_endpoint_pair("a:b"), ("a", "b"));
        assert_eq!(split_endpoint_pair("a->b"), ("a", "b"));
        assert_eq!(split_endpoint_pair("single"), ("single", ""));
    }

    #[test]
    fn test_cli_modifier_and_verb_group_traits() {
        let cli = Cli::try_parse_from([
            "rw",
            "sub/dir",
            "--limit",
            "10",
            "--offset",
            "5",
            "--top-k",
            "50",
            "--pack-budget",
            "8192",
            "--with-history",
            "--since",
            "6m",
            "--json",
            "--pretty",
            "--legend",
            "none",
            "--callers",
            "main",
        ])
        .unwrap();
        assert_eq!(cli.limit_opt(), Some(10));
        assert_eq!(cli.offset_opt(), Some(5));
        assert_eq!(cli.modifiers.limit_opt(), Some(10));
        assert_eq!(cli.modifiers.offset_opt(), Some(5));
        let pw = cli.page_window(100, 40);
        assert_eq!(pw.begin, 5);
        assert_eq!(pw.end, 15);
        assert_eq!(pw.shown, 10);
        assert!(pw.capped);
        assert!(pw.has_more);

        assert_eq!(cli.pack_budget_opt(), Some(8192));
        assert_eq!(cli.token_budget_opt(), None);
        assert_eq!(cli.max_tokens_opt(), None);
        assert_eq!(cli.top_k_opt(), Some(50));
        assert_eq!(cli.detail_opt(), None);
        assert!(cli.with_history_flag());
        assert_eq!(cli.since_scope_opt(), Some("6m"));
        assert!(cli.is_json_out());
        assert!(cli.is_pretty_out());
        assert!(cli.is_legend_none());
        assert_eq!(cli.target_root(), "sub/dir");
        assert!(cli.nav.is_group_active());
        assert!(!cli.detail.is_group_active());
        assert!(!cli.safety.is_group_active());
        assert!(!cli.arch.is_group_active());
        assert!(!cli.retrieval.is_group_active());
        assert!(!cli.workspace.is_group_active());
    }
}
