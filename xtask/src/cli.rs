use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "cargo xtask")]
#[command(about = "Cross-platform automation for db-center-template")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize local project files such as .env.
    Init,
    /// Manage PostgreSQL and database maintenance tasks.
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    /// Run SeaORM migration commands.
    Migrate {
        #[command(subcommand)]
        command: MigrateCommand,
    },
    /// Generate code from database metadata.
    Entity {
        #[command(subcommand)]
        command: EntityCommand,
    },
    /// Refresh the database and regenerate entities.
    FreshDb,
    /// Initialize admin permissions, menus, and built-in admin roles.
    InitPermissions,
    /// Initialize app permissions and the free app role.
    InitAppPermissions,
    /// Bind an auth account to the root admin role.
    InitRoot,
}

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    /// Start the PostgreSQL Docker container and ensure the target database exists.
    Up,
    /// Show PostgreSQL container status.
    Status,
    /// Stop the PostgreSQL container.
    Stop,
    /// Remove the PostgreSQL container.
    Rm,
    /// Ensure the target database exists.
    Init,
    /// Clear the public schema.
    Clear,
    /// Truncate one table and restart identity.
    Truncate {
        /// Table name to truncate.
        table: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum MigrateCommand {
    /// Apply pending migrations.
    Up,
    /// Roll back one migration.
    Down,
    /// Drop all tables and rerun all migrations.
    Fresh,
    /// Roll back all migrations and rerun them.
    Refresh,
    /// Roll back all migrations.
    Reset,
    /// Show migration status.
    Status,
    /// Generate a new migration file.
    Generate {
        /// Migration name, such as create_users.
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum EntityCommand {
    /// Generate SeaORM entities from the current database schema.
    Generate,
}
