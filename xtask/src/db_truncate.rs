use anyhow::{Result, bail};

use crate::common::{XtaskContext, assert_safe_identifier};

pub fn run(table: &str) -> Result<()> {
    if table.is_empty() {
        bail!("错误：需要表名。用法：cargo xtask db truncate your_table");
    }

    let ctx = XtaskContext::load()?;
    assert_safe_identifier(table, "表名")?;

    let exists = ctx
        .run_psql(
            &ctx.db_name,
            &format!("SELECT to_regclass('public.{table}') IS NOT NULL;"),
        )?
        .contains('t');

    if !exists {
        bail!("错误：表不存在：{table}");
    }

    ctx.run_psql(
        &ctx.db_name,
        &format!("TRUNCATE TABLE {table} RESTART IDENTITY CASCADE;"),
    )?;
    println!("已清空表：{table}");
    Ok(())
}
