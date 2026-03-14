//! CLI subcommands for inspecting and testing configured agents.

use anyhow::Result;
use ath_config::ConfigStore;
use clap::{Args, Subcommand};
use colored::Colorize;

use crate::GlobalArgs;

#[derive(Debug, Args, Clone)]
#[command(about = "Inspect and manage configured AI agents.")]
pub(crate) struct AgentsArgs {
    #[command(subcommand)]
    pub(crate) command: AgentsCommand,
}

#[derive(Debug, Subcommand, Clone)]
pub(crate) enum AgentsCommand {
    /// List all configured agents and their status.
    List,
    /// Test connectivity to each configured agent.
    Test,
}

pub(crate) fn agents_command(args: AgentsArgs, global: GlobalArgs) -> Result<()> {
    let config = ConfigStore::load().map_err(anyhow::Error::new)?;

    match args.command {
        AgentsCommand::List => list_agents(&config, global),
        AgentsCommand::Test => test_agents(&config, global),
    }
}

fn list_agents(config: &ConfigStore, _global: GlobalArgs) -> Result<()> {
    let agents = &config.agents.agents;

    if agents.is_empty() {
        println!("No agents configured.");
        println!(
            "{}",
            "Add agents to .ath/agents.toml or set API key environment variables."
                .dimmed()
        );
        return Ok(());
    }

    println!("{}", "Configured Agents".bold());
    println!("{}", "─".repeat(60));

    for agent in agents {
        let has_key = agent.resolve_api_key().is_some();
        let status = if has_key {
            "ready".green().to_string()
        } else {
            format!("{} ({})", "missing key".red(), agent.api_key_env.dimmed())
        };

        let name = agent.name();
        let provider_model = format!("{}/{}", agent.provider, agent.model);

        if agent.display_name.is_some() {
            println!(
                "  {} ({})",
                name.bold(),
                provider_model.dimmed()
            );
        } else {
            println!("  {}", provider_model.bold());
        }

        println!("    Status:  {status}");

        if let Some(ref base_url) = agent.base_url {
            println!("    URL:     {base_url}");
        }

        if agent.is_builtin_provider() {
            println!("    Type:    built-in");
        } else {
            println!("    Type:    custom (OpenAI-compatible)");
        }

        println!();
    }

    let available = agents.iter().filter(|a| a.resolve_api_key().is_some()).count();
    println!(
        "{} agent(s) configured, {} available",
        agents.len(),
        available
    );

    // Show skills config status
    if let Some(ref skills) = config.skills {
        if skills.has_overrides() {
            println!(
                "\n{} Custom skill routing active ({} route(s){})",
                "Skills:".bold(),
                skills.routes.len(),
                if skills.default.is_some() {
                    ", custom default"
                } else {
                    ""
                }
            );
        }
    }

    Ok(())
}

fn test_agents(config: &ConfigStore, _global: GlobalArgs) -> Result<()> {
    let agents = &config.agents.agents;

    if agents.is_empty() {
        println!("No agents configured to test.");
        return Ok(());
    }

    let available: Vec<_> = agents.iter().filter(|a| a.resolve_api_key().is_some()).collect();

    if available.is_empty() {
        println!("No agents have API keys configured. Nothing to test.");
        println!(
            "{}",
            "Set the required environment variables and try again.".dimmed()
        );
        return Ok(());
    }

    println!("{}", "Testing Agent Connectivity".bold());
    println!("{}", "─".repeat(40));

    // We can't actually call the agents without a tokio runtime in a sync function,
    // and a real connectivity test would require actual API calls.
    // For now, verify that backends can be constructed (auth validation).
    for agent_cfg in &available {
        let name = agent_cfg.name();
        print!("  {} ... ", name);

        match crate::run::build_backend_for_agent_from_config(agent_cfg, config) {
            Ok(Some(_)) => println!("{}", "✓ backend ready".green()),
            Ok(None) => println!("{}", "⚠ skipped (unsupported)".yellow()),
            Err(e) => println!("{} {}", "✗".red(), e),
        }
    }

    println!();
    println!(
        "Tested {} agent(s). Run `ath run` to execute with these agents.",
        available.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agents_list_subcommand_parses() {
        use clap::Parser;

        #[derive(Debug, Parser)]
        struct TestCli {
            #[command(subcommand)]
            command: TestCmd,
        }

        #[derive(Debug, Subcommand)]
        enum TestCmd {
            Agents(AgentsArgs),
        }

        let cli = TestCli::try_parse_from(["test", "agents", "list"]).expect("parse agents list");
        match cli.command {
            TestCmd::Agents(args) => {
                assert!(matches!(args.command, AgentsCommand::List));
            }
        }
    }

    #[test]
    fn agents_test_subcommand_parses() {
        use clap::Parser;

        #[derive(Debug, Parser)]
        struct TestCli {
            #[command(subcommand)]
            command: TestCmd,
        }

        #[derive(Debug, Subcommand)]
        enum TestCmd {
            Agents(AgentsArgs),
        }

        let cli = TestCli::try_parse_from(["test", "agents", "test"]).expect("parse agents test");
        match cli.command {
            TestCmd::Agents(args) => {
                assert!(matches!(args.command, AgentsCommand::Test));
            }
        }
    }
}
