mod cli;
mod commands;

use anyhow::{Result, anyhow};
use clap::{CommandFactory, Parser};
use cli_tools::{
    configs::PlatformsConfig,
    languages::{
        LanguageBackend, PythonLanguageBackend, RustLanguageBackend, SwiftLanguageBackend, TypeScriptLanguageBackend,
    },
    release::run_release,
    sync::run_sync,
    types::{Command, Configuration, Language},
};

use crate::{cli::Cli, commands::Commands};

fn run_setup(include_platform_specific: bool) -> Result<()> {
    if include_platform_specific && cfg!(target_vendor = "apple") {
        Command::xcodebuild_first_launch().run()?;
    }

    Command::rustup_setup().run()?;
    Command::rustup_update().run()?;
    Command::rustup_show().run()?;
    Command::uv_setup().run()?;
    Command::pnpm_setup().run()?;

    Ok(())
}

fn run_verify(config: &PlatformsConfig) -> Result<()> {
    for language in config.languages.keys() {
        let backend = language_backend(*language, config.clone())?;
        backend.build(Configuration::Release, vec![config.host_target()?], vec![])?;
    }
    let (output, _) = Command::git_status_porcelain().output()?;
    if !output.is_empty() {
        eprintln!("{output}");
        return Err(anyhow!("The repository has uncommitted changes after building all languages"));
    }
    Ok(())
}

fn language_backend(
    language: Language,
    config: PlatformsConfig,
) -> Result<Box<dyn LanguageBackend>> {
    match language {
        Language::Rust => Ok(Box::new(RustLanguageBackend::new(config))),
        Language::Python => Ok(Box::new(PythonLanguageBackend::new(config))),
        Language::Swift => Ok(Box::new(SwiftLanguageBackend::new(config))),
        Language::TypeScript => Ok(Box::new(TypeScriptLanguageBackend::new(config))),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = PlatformsConfig::load()?;
    let host_target = config.host_target()?;

    match cli.command {
        Some(Commands::Setup {
            include_platform_specific,
        }) => run_setup(include_platform_specific)?,
        Some(Commands::Install {
            language,
        }) => language_backend(language, config)?.install()?,
        Some(Commands::Build {
            language,
            configuration,
            targets,
            capabilities,
        }) => language_backend(language, config)?.build(configuration, targets, capabilities)?,
        Some(Commands::Test {
            language,
        }) => {
            let configuration = Configuration::Release;
            let capabilities = vec![];
            let backend = language_backend(language, config.clone())?;
            if backend.expects_prebuild_for_run() {
                backend.build(configuration, vec![host_target.clone()], capabilities.clone())?;
            }
            backend.test(configuration, host_target.clone(), capabilities)?
        },
        Some(Commands::Example {
            language,
            name,
        }) => {
            let configuration = Configuration::Release;
            let capabilities = vec![];
            let backend = language_backend(language, config.clone())?;
            if backend.expects_prebuild_for_run() {
                backend.build(configuration, vec![host_target.clone()], capabilities.clone())?;
            }
            backend.example(&name, configuration, host_target.clone(), capabilities)?
        },
        Some(Commands::Sync {
            check,
        }) => run_sync(check)?,
        Some(Commands::Verify) => run_verify(&config)?,
        Some(Commands::Release {
            version,
        }) => run_release(&version)?,
        None => {
            let mut cmd = Cli::command();
            cmd.print_help()?;
        },
    }

    Ok(())
}
