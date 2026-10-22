use clap::CommandFactory;
use ripwire_rs::Cli;
use ripwire_rs::commands::{self, SharedOption};
use std::collections::HashSet;

fn windowed(options: &[SharedOption]) -> bool {
    options.iter().any(|option| {
        matches!(
            option,
            SharedOption::Paged | SharedOption::PagedMatches | SharedOption::McpPaged
        )
    })
}

fn budgeted(options: &[SharedOption]) -> bool {
    options
        .iter()
        .any(|option| matches!(option, SharedOption::Budget(_)))
}

fn exposed_params(tool_name: &str) -> Vec<&'static str> {
    commands::tools()
        .iter()
        .find(|tool| tool.name == tool_name)
        .expect("registered tool")
        .params
        .iter()
        .map(|(name, _, _)| *name)
        .collect()
}

#[test]
fn command_names_and_tool_names_are_unique() {
    let mut names = HashSet::new();
    for command in commands::commands().iter().filter(|c| !c.name().is_empty()) {
        assert!(
            names.insert(command.name()),
            "duplicate command {}",
            command.name()
        );
    }
    let mut tools = HashSet::new();
    for tool in commands::tools() {
        assert!(tools.insert(tool.name), "duplicate tool {}", tool.name);
    }
}

#[test]
fn every_required_tool_parameter_is_declared() {
    for tool in commands::tools() {
        for required in tool.required {
            assert!(
                tool.params.iter().any(|(name, _, _)| name == required),
                "{} requires undeclared '{required}'",
                tool.name
            );
        }
    }
}

#[test]
fn tool_parameters_are_declared_once() {
    for tool in commands::tools() {
        let mut names = HashSet::new();
        for (name, _, _) in tool.params.iter() {
            assert!(names.insert(*name), "{} declares {name} twice", tool.name);
        }
    }
}

#[test]
fn tools_of_windowed_commands_expose_limit_and_offset() {
    assert!(exposed_params("flags").contains(&"limit"));
    assert!(!exposed_params("for").contains(&"limit"));
    for command in commands::commands() {
        for tool in command.tools() {
            let params = exposed_params(tool.name);
            let expected = windowed(command.options());
            assert_eq!(params.contains(&"limit"), expected, "{} limit", tool.name);
            assert_eq!(params.contains(&"offset"), expected, "{} offset", tool.name);
        }
    }
}

#[test]
fn tools_expose_budget_tokens_only_where_declared() {
    for command in commands::commands() {
        for tool in command.tools() {
            let expected = budgeted(command.options());
            let params = exposed_params(tool.name);
            assert_eq!(
                params.contains(&"budget_tokens"),
                expected,
                "{} budget_tokens",
                tool.name
            );
        }
    }
}

#[test]
fn plugin_flags_compose_into_one_valid_cli() {
    commands::augment(Cli::command()).debug_assert();
}

#[test]
fn paging_is_honored_only_where_declared() {
    let paged = ripwire_rs::parse_from(["rw", "--limit=1"]).unwrap();
    let unpaged = ripwire_rs::parse_from(["rw"]).unwrap();
    for command in commands::commands().iter().filter(|c| !c.name().is_empty()) {
        let declared = command.options().iter().any(|option| option.is_paged());
        let warning = commands::unhonored_option_warning(&paged, *command);
        assert_eq!(warning.is_none(), declared, "--{}", command.name());
        assert!(commands::unhonored_option_warning(&unpaged, *command).is_none());
    }
}
