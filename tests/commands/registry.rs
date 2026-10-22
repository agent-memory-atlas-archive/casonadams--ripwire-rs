use clap::CommandFactory;
use ripwire_rs::Cli;
use ripwire_rs::commands::{self, SharedOption};
use std::collections::HashSet;

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
fn plugin_flags_compose_into_one_valid_cli() {
    commands::augment(Cli::command()).debug_assert();
}

#[test]
fn paging_is_honored_only_where_declared() {
    let paged = ripwire_rs::parse_from(["rw", "--limit=1"]).unwrap();
    let unpaged = ripwire_rs::parse_from(["rw"]).unwrap();
    for command in commands::commands().iter().filter(|c| !c.name().is_empty()) {
        let declared = command.options().contains(&SharedOption::Paged);
        let warning = commands::unhonored_option_warning(&paged, *command);
        assert_eq!(warning.is_none(), declared, "--{}", command.name());
        assert!(commands::unhonored_option_warning(&unpaged, *command).is_none());
    }
}
