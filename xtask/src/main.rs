mod app_permissions;
mod cli;
mod common;
mod db_clear;
mod db_truncate;
mod entity_generate;
mod fresh_db;
mod init;
mod init_db;
mod init_permissions;
mod init_root;
mod migrate;
mod postgres;
mod process;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command, DbCommand, EntityCommand, MigrateCommand};

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run()
}

impl Cli {
    fn run(self) -> Result<()> {
        match self.command {
            Command::Init | Command::InitEnv => init::run(),
            Command::Db { command } => command.run(),
            Command::Migrate { command } => command.run(),
            Command::Entity { command } => command.run(),
            Command::FreshDb => fresh_db::run(),
            Command::InitPermissions => init_permissions::run(),
            Command::InitAppPermissions => app_permissions::run(),
            Command::InitRoot => init_root::run(),
        }
    }
}

impl DbCommand {
    fn run(self) -> Result<()> {
        match self {
            DbCommand::Up => postgres::up(),
            DbCommand::Status => postgres::status(),
            DbCommand::Stop => postgres::stop(),
            DbCommand::Rm => postgres::rm(),
            DbCommand::Init => init_db::run(),
            DbCommand::Clear => db_clear::run(),
            DbCommand::Truncate { table } => db_truncate::run(&table),
        }
    }
}

impl MigrateCommand {
    fn run(self) -> Result<()> {
        migrate::run(self)
    }
}

impl EntityCommand {
    fn run(self) -> Result<()> {
        match self {
            EntityCommand::Generate => entity_generate::run(),
        }
    }
}
