use anyhow::Result;

use crate::{entity_generate, migrate};

pub fn run() -> Result<()> {
    migrate::run(crate::cli::MigrateCommand::Refresh)?;
    entity_generate::run()?;
    println!("数据库已 refresh，entity 已重新生成");
    Ok(())
}
