use anyhow::Result;

use crate::common::XtaskContext;

pub fn run() -> Result<()> {
    let ctx = XtaskContext::load()?;

    println!("初始化基础权限节点...");
    ctx.run_psql(
        &ctx.db_name,
        "TRUNCATE admin_menus, admin_role_permissions, admin_permissions RESTART IDENTITY CASCADE;",
    )?;

    let permissions = [
        ("dashboard", "Dashboard", "", 100, "group"),
        ("accounts", "Accounts", "", 200, "group"),
        (
            "accounts:admin_users",
            "Admin Users",
            "accounts",
            210,
            "group",
        ),
        ("accounts:app_users", "App Users", "accounts", 220, "group"),
        ("access_control", "Access Control", "", 300, "group"),
        (
            "access_control:roles",
            "Roles",
            "access_control",
            310,
            "group",
        ),
        (
            "access_control:role_permissions",
            "Role Permissions",
            "access_control",
            320,
            "group",
        ),
        (
            "access_control:app_roles",
            "App Roles",
            "access_control",
            330,
            "group",
        ),
        (
            "access_control:app_role_permissions",
            "App Role Permissions",
            "access_control",
            340,
            "group",
        ),
    ];

    for (code, name, parent_code, sort, kind) in permissions {
        upsert_permission(&ctx, code, name, parent_code, sort, kind)?;
    }

    println!("初始化基础菜单...");
    for (name, code, parent_code, permission_code, sort_hint) in [
        ("Dashboard", "dashboard", "", "dashboard", "100"),
        ("Accounts", "accounts", "", "accounts", "200"),
        (
            "Admin Users",
            "accounts:admin_users",
            "accounts",
            "accounts:admin_users",
            "210",
        ),
        (
            "App Users",
            "accounts:app_users",
            "accounts",
            "accounts:app_users",
            "220",
        ),
        (
            "Access Control",
            "access_control",
            "",
            "access_control",
            "300",
        ),
        (
            "Roles",
            "access_control:roles",
            "access_control",
            "access_control:roles",
            "310",
        ),
        (
            "Role Permissions",
            "access_control:role_permissions",
            "access_control",
            "access_control:role_permissions",
            "320",
        ),
        (
            "App Roles",
            "access_control:app_roles",
            "access_control",
            "access_control:app_roles",
            "330",
        ),
        (
            "App Role Permissions",
            "access_control:app_role_permissions",
            "access_control",
            "access_control:app_role_permissions",
            "340",
        ),
    ] {
        upsert_menu(&ctx, name, code, parent_code, permission_code, sort_hint)?;
    }

    println!("初始化后台基础角色...");
    upsert_role(&ctx, "admin", "超级管理员")?;
    upsert_role(&ctx, "support", "客服")?;

    grant_role_permissions(
        &ctx,
        "admin",
        &[
            "dashboard",
            "accounts",
            "accounts:admin_users",
            "accounts:app_users",
            "access_control",
            "access_control:roles",
            "access_control:role_permissions",
            "access_control:app_roles",
            "access_control:app_role_permissions",
        ],
    )?;
    grant_role_permissions(
        &ctx,
        "support",
        &[
            "dashboard",
            "accounts:app_users",
            "access_control:app_roles",
            "access_control:app_role_permissions",
        ],
    )?;

    let permission_count = count_in(
        &ctx,
        "admin_permissions",
        &permissions.map(|(code, _, _, _, _)| code),
    )?;
    let menu_count = count_in_column(
        &ctx,
        "admin_menus",
        "permission_code",
        &permissions.map(|(code, _, _, _, _)| code),
    )?;
    let role_count = count_in(&ctx, "admin_roles", &["admin", "support"])?;
    let role_permission_count = ctx
        .run_psql(
            &ctx.db_name,
            "SELECT COUNT(*) FROM admin_role_permissions rp JOIN admin_roles r ON r.id = \
             rp.role_id WHERE r.code IN ('admin', 'support');",
        )?
        .trim()
        .to_owned();

    println!("基础权限初始化完成");
    println!("admin_permissions: {permission_count}");
    println!("admin_menus: {menu_count}");
    println!("admin_roles: {role_count}");
    println!("admin_role_permissions: {role_permission_count}");
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
    let parent_sql = nullable_string(parent_code);
    ctx.run_psql(
        &ctx.db_name,
        &format!(
            "INSERT INTO admin_permissions (code, name, parent_code, sort, kind)
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

fn upsert_menu(
    ctx: &XtaskContext,
    name: &str,
    code: &str,
    parent_code: &str,
    permission_code: &str,
    _sort_hint: &str,
) -> Result<()> {
    let parent_id_sql = if parent_code.is_empty() {
        "NULL".to_owned()
    } else {
        format!("(SELECT id FROM admin_menus WHERE permission_code = '{parent_code}' LIMIT 1)")
    };

    ctx.run_psql(
        &ctx.db_name,
        &format!(
            "UPDATE admin_menus
             SET name = '{name}',
                 parent_id = {parent_id_sql},
                 permission_code = '{permission_code}'
             WHERE permission_code = '{code}'
                OR (
                  permission_code IS NULL
                  AND name = '{name}'
                  AND parent_id IS NOT DISTINCT FROM {parent_id_sql}
                );

             INSERT INTO admin_menus (name, parent_id, permission_code)
             SELECT '{name}', {parent_id_sql}, '{permission_code}'
             WHERE NOT EXISTS (
               SELECT 1 FROM admin_menus WHERE permission_code = '{code}'
             );"
        ),
    )?;
    Ok(())
}

fn upsert_role(ctx: &XtaskContext, code: &str, name: &str) -> Result<()> {
    ctx.run_psql(
        &ctx.db_name,
        &format!(
            "INSERT INTO admin_roles (code, name)
             VALUES ('{code}', '{name}')
             ON CONFLICT (code) DO NOTHING;"
        ),
    )?;
    Ok(())
}

fn grant_role_permissions(ctx: &XtaskContext, role_code: &str, permissions: &[&str]) -> Result<()> {
    let values = permissions
        .iter()
        .map(|code| format!("('{code}')"))
        .collect::<Vec<_>>()
        .join(",");

    ctx.run_psql(
        &ctx.db_name,
        &format!(
            "WITH role_row AS (
               SELECT id FROM admin_roles WHERE code = '{role_code}'
             ),
             permission_rows AS (
               SELECT id
               FROM admin_permissions
               WHERE code IN (
                 SELECT code FROM (VALUES {values}) AS permission_codes(code)
               )
             )
             INSERT INTO admin_role_permissions (role_id, permission_id)
             SELECT role_row.id, permission_rows.id
             FROM role_row
             CROSS JOIN permission_rows
             ON CONFLICT (role_id, permission_id) DO NOTHING;"
        ),
    )?;
    Ok(())
}

fn count_in(ctx: &XtaskContext, table: &str, codes: &[&str]) -> Result<String> {
    count_in_column(ctx, table, "code", codes)
}

fn count_in_column(
    ctx: &XtaskContext,
    table: &str,
    column: &str,
    codes: &[&str],
) -> Result<String> {
    let quoted = codes
        .iter()
        .map(|code| format!("'{code}'"))
        .collect::<Vec<_>>()
        .join(",");
    Ok(ctx
        .run_psql(
            &ctx.db_name,
            &format!("SELECT COUNT(*) FROM {table} WHERE {column} IN ({quoted});"),
        )?
        .trim()
        .to_owned())
}

fn nullable_string(value: &str) -> String {
    if value.is_empty() {
        "NULL".to_owned()
    } else {
        format!("'{value}'")
    }
}
