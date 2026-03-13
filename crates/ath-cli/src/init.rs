use anyhow::Result;
use clap::Args;

use crate::GlobalArgs;

#[derive(Debug, Args, Clone, PartialEq, Eq, Default)]
#[command(about = "Initialize Athena interactively in the current project.")]
pub(crate) struct InitArgs {}

pub(crate) fn init_command(_args: InitArgs, _global: GlobalArgs) -> Result<()> {
    println!("{}", init_placeholder_message());
    Ok(())
}

pub(crate) fn init_placeholder_message() -> &'static str {
    "Interactive setup is not wired yet. `ath init` will guide project setup and offer an example spec during that flow."
}
