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
}
