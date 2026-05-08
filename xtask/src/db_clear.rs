use anyhow::Result;

use crate::common::{XtaskContext, assert_safe_identifier};

pub fn run() -> Result<()> {
    let ctx = XtaskContext::load()?;
    assert_safe_identifier(&ctx.db_user, "数据库用户名")?;

    ctx.run_psql(&ctx.db_name, "DROP SCHEMA IF EXISTS public CASCADE;")?;
    ctx.run_psql(&ctx.db_name, "CREATE SCHEMA public;")?;
    ctx.run_psql(
        &ctx.db_name,
        &format!("GRANT ALL ON SCHEMA public TO {};", ctx.db_user),
    )?;
    ctx.run_psql(&ctx.db_name, "GRANT ALL ON SCHEMA public TO public;")?;

    println!("已清空数据库：{}", ctx.db_name);
    Ok(())
}
