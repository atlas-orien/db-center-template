use anyhow::Result;

use crate::common::XtaskContext;

pub fn run() -> Result<()> {
    let ctx = XtaskContext::load()?;

    println!("初始化普通用户基础权限节点...");
    ctx.run_psql(
        &ctx.db_name,
        "INSERT INTO app_roles (code, name)
         VALUES ('free', '免费')
         ON CONFLICT (code) DO NOTHING;

         DELETE FROM app_role_permissions;
         DELETE FROM app_permissions;",
    )?;

    for (code, name, parent_code, sort, kind) in [
        ("profile", "个人中心", "", 100, "group"),
        ("profile:view", "查看个人资料", "profile", 110, "action"),
        ("profile:update", "更新个人资料", "profile", 120, "action"),
    ] {
        upsert_permission(&ctx, code, name, parent_code, sort, kind)?;
    }

    let permission_count = ctx
        .run_psql(
            &ctx.db_name,
            "SELECT COUNT(*)
             FROM app_permissions
             WHERE code IN ('profile', 'profile:view', 'profile:update');",
        )?
        .trim()
        .to_owned();

    println!("普通用户基础权限初始化完成");
    println!("app_roles: free");
    println!("app_permissions: {permission_count}");
    Ok(())
}

fn upsert_permission(
    ctx: &XtaskContext,
    code: &str,
    name: &str,
    parent_code: &str,
    sort: i32,
    kind: &str,
) -> Result<()> {
    let parent_sql = if parent_code.is_empty() {
        "NULL".to_owned()
    } else {
        format!("'{parent_code}'")
    };

    ctx.run_psql(
        &ctx.db_name,
        &format!(
            "INSERT INTO app_permissions (code, name, parent_code, sort, kind)
             VALUES ('{code}', '{name}', {parent_sql}, {sort}, '{kind}')
             ON CONFLICT (code) DO UPDATE
             SET name = EXCLUDED.name,
                 parent_code = EXCLUDED.parent_code,
                 sort = EXCLUDED.sort,
                 kind = EXCLUDED.kind;"
        ),
    )?;
    Ok(())
}
