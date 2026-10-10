use clap::Subcommand;
use cli_tools::{
    configs::HOST_TARGET,
    types::{Capability, Configuration, Language},
};

#[derive(Subcommand)]
pub enum Commands {
    /// Install rustup / uv / pnpm and required targets
    Setup {
        #[arg(long)]
        include_platform_specific: bool,
    },
    /// Install tools for a specific language
    Install {
        #[arg(value_enum)]
        language: Language,
    },
    /// Build for a specific language
    Build {
        #[arg(value_enum, default_value = "rust")]
        language: Language,
        #[arg(value_enum, default_value = "release")]
        configuration: Configuration,
        #[arg(long, value_delimiter = ',', default_value = HOST_TARGET)]
        targets: Vec<String>,
        #[arg(long, value_enum, value_delimiter = ',')]
        capabilities: Vec<Capability>,
    },
    /// Run tests for a specific language
    Test {
        #[arg(value_enum)]
        language: Language,
    },
    /// Run an example for a specific language
    Example {
        #[arg(value_enum)]
        language: Language,
        name: String,
    },
    /// Synchronize project files with platforms.toml
    Sync {
        #[arg(long)]
        check: bool,
    },
    /// Verify that the working tree has no uncommitted changes after building each language
    Verify,
    /// Bump version, sync, build all languages, and stage release artifacts
    Release {
        version: String,
    },
}
