use std::io;

/// Print copy-paste integration recipe for coding agents.
pub fn execute_wrap_command(agent: &str, _force: bool) -> io::Result<()> {
    match agent.to_ascii_lowercase().as_str() {
        "claude" => {
            println!(
                r#"# ripwire -> Claude Code (CLI-first)
# RECOMMENDED — this agent can run shell commands, so call the CLI directly. It costs
# nothing until you invoke it, and it reads CLAUDE.md, so the paste block below IS the wiring:
ripwire-rs . --for="<your task>" --token-budget=2000

# ALTERNATIVE — register the MCP server instead, for a warm index across calls:
claude mcp add ripwire -- ripwire-rs --mcp
# verbs the agent can then call mid-task (33 total):
#   read:             analyze, rank_by, find_symbol, find_referencing_symbols, grep, cochange, memory_recall, situational_awareness, mentions, for, lego, owners, fetch_body, batch, flags, doc_drift, slice
#   flagship reflex:  exemplar, quality_delta, quality_baseline, impact, uses, affected, path_between, connect, explore, from_trace, edit_check, whereis, stray_content
#   edit:             replace_symbol_body, insert_before_symbol, insert_after_symbol
"#
            );
            Ok(())
        }
        "cursor" | "windsurf" | "gemini" => {
            println!(
                r#"{{
  "mcpServers": {{
    "ripwire": {{
      "command": "ripwire-rs",
      "args": ["--mcp"]
    }}
  }}
}}"#
            );
            Ok(())
        }
        "codex" => {
            println!(
                r#"# OpenAI Codex (~/.codex/config.toml)
[mcp_servers.ripwire]
command = "ripwire-rs"
args = ["--mcp"]
"#
            );
            Ok(())
        }
        "opencode" => {
            println!(
                r#"{{
  "mcp": {{
    "ripwire": {{
      "command": "ripwire-rs",
      "args": ["--mcp"]
    }}
  }}
}}"#
            );
            Ok(())
        }
        "aider" => {
            println!(
                r#"# aider (repo-map context)
ripwire-rs . --for="<task>" > .ripwire-map.txt
aider --read .ripwire-map.txt
"#
            );
            Ok(())
        }
        other => {
            eprintln!(
                "ripwire wrap: unknown agent '{other}'\n\
ripwire wrap <agent> — print the recipe to wire ripwire into an agent's loop.\n  \
CLI-first:   claude  codex  opencode  openclaw  hermes\n  \
MCP config:  cursor  windsurf  gemini\n  \
repo-map:    aider\n  \
example:     ripwire wrap claude\n  \
--all        detect every installed agent + emit each one's config\n\
# ripwire -> add to your client's MCP config (generic stanza)\n\
{{\n  \
\"mcpServers\": {{\n    \
\"ripwire\": {{ \"command\": \"ripwire-rs\", \"args\": [\"--mcp\"] }}\n  \
}}\n\
}}"
            );
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_wrap_command_recipes() {
        for agent in ["claude", "cursor", "codex", "opencode", "aider"] {
            assert!(execute_wrap_command(agent, false).is_ok());
        }
    }
}
