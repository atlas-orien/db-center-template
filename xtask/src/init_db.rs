use anyhow::Result;

use crate::common::{XtaskContext, assert_safe_identifier};

pub fn run() -> Result<()> {
    let ctx = XtaskContext::load()?;
    assert_safe_identifier(&ctx.db_name, "数据库名")?;

    let exists = ctx
        .run_psql(
            "postgres",
            &format!(
                "SELECT 1 FROM pg_database WHERE datname = '{}'",
                ctx.db_name
            ),
        )?
        .contains('1');

    if exists {
        println!("数据库已存在：{}", ctx.db_name);
        return Ok(());
    }

    ctx.run_psql("postgres", &format!("CREATE DATABASE {};", ctx.db_name))?;
    println!("已创建数据库：{}", ctx.db_name);
    Ok(())
}
