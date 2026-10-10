use clap::Parser;

use crate::commands::Commands;

#[derive(Parser)]
#[command(name = "xgrammar-tools", bin_name = "xgrammar-tools")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}
