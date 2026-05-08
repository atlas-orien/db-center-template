use anyhow::{Result, bail};

use crate::{common::XtaskContext, process};

pub fn run() -> Result<()> {
    let ctx = XtaskContext::load()?;
    process::require_command("sea-orm-cli")?;

    let table_count = ctx
        .run_psql(
            &ctx.db_name,
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' AND \
             table_type = 'BASE TABLE';",
        )?
        .trim()
        .parse::<usize>()
        .unwrap_or(0);

    if table_count == 0 {
        bail!(
            "错误：当前数据库没有任何表结构，无法生成 entity\n请先执行迁移或先创建表，再运行此命令"
        );
    }

    let entity_dir = ctx.entity_dir.to_string_lossy().to_string();
    process::run_with_env(
        "sea-orm-cli",
        &[
            "generate",
            "entity",
            "-o",
            &entity_dir,
            "--with-serde",
            "both",
            "--date-time-crate",
            "time",
        ],
        &[("DATABASE_URL", ctx.database_url.as_str())],
    )?;

    println!("entity 已生成到：{}", ctx.entity_dir.display());
    println!("此操作不会修改数据库结构");
    Ok(())
}
