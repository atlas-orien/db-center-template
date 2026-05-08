use anyhow::Result;

use crate::{cli::MigrateCommand, common::XtaskContext, process};

pub fn run(command: MigrateCommand) -> Result<()> {
    let ctx = XtaskContext::load()?;
    process::require_command("sea-orm-cli")?;

    let migration_dir = ctx.migration_dir.to_string_lossy().to_string();
    let envs = [("DATABASE_URL", ctx.database_url.as_str())];

    match command {
        MigrateCommand::Up => process::run_with_env(
            "sea-orm-cli",
            &["migrate", "up", "-d", &migration_dir],
            &envs,
        ),
        MigrateCommand::Down => process::run_with_env(
            "sea-orm-cli",
            &["migrate", "down", "-d", &migration_dir],
            &envs,
        ),
        MigrateCommand::Fresh => process::run_with_env(
            "sea-orm-cli",
            &["migrate", "fresh", "-d", &migration_dir],
            &envs,
        ),
        MigrateCommand::Refresh => process::run_with_env(
            "sea-orm-cli",
            &["migrate", "refresh", "-d", &migration_dir],
            &envs,
        ),
        MigrateCommand::Reset => process::run_with_env(
            "sea-orm-cli",
            &["migrate", "reset", "-d", &migration_dir],
            &envs,
        ),
        MigrateCommand::Status => process::run_with_env(
            "sea-orm-cli",
            &["migrate", "status", "-d", &migration_dir],
            &envs,
        ),
        MigrateCommand::Generate { name } => process::run_with_env(
            "sea-orm-cli",
            &["migrate", "generate", &name, "-d", &migration_dir],
            &envs,
        ),
    }
}
