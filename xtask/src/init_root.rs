use std::{
    env,
    io::{self, Write},
};

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};

use crate::common::{XtaskContext, sql_escape};

pub fn run() -> Result<()> {
    let ctx = XtaskContext::load()?;
    let config_path = env::var("CONFIG_PATH")
        .map(Into::into)
        .unwrap_or_else(|_| ctx.project_root.join("config/services.toml"));

    if !config_path.exists() {
        bail!("错误：找不到配置文件 {}", config_path.display());
    }

    let jwt_verify_url = read_jwt_verify_url(&config_path)?;
    let auth_base_url = jwt_verify_url
        .strip_suffix("/internal/jwt_verify_config")
        .unwrap_or(&jwt_verify_url)
        .to_owned();
    let login_url = format!("{auth_base_url}/auth/session/login");
    let me_url = format!("{auth_base_url}/auth/user/me");

    let identifier = env_or_prompt("ROOT_IDENTIFIER", "请输入 root 账号（用户名或邮箱）: ")?;
    if identifier.is_empty() {
        bail!("错误：账号不能为空");
    }

    let password = env_or_prompt_password("ROOT_PASSWORD", "请输入 root 密码: ")?;
    if password.is_empty() {
        bail!("错误：密码不能为空");
    }

    let client = reqwest::blocking::Client::new();
    let login_response: Value = client
        .post(&login_url)
        .json(&json!({
            "identifier": identifier,
            "password": password,
        }))
        .send()
        .context("登录 auth 失败")?
        .error_for_status()
        .context("auth 登录返回错误状态")?
        .json()
        .context("解析登录响应失败")?;

    let access_token = find_string_field(&login_response, "accessToken")
        .context("错误：登录响应中缺少 accessToken")?;

    let me_response: Value = client
        .get(&me_url)
        .bearer_auth(&access_token)
        .send()
        .context("读取 auth 用户信息失败")?
        .error_for_status()
        .context("auth 用户信息返回错误状态")?
        .json()
        .context("解析 auth 用户信息失败")?;

    let payload = me_response.get("data").unwrap_or(&me_response);
    let mut user_id = payload
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let display_id = payload
        .get("display_user_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(&user_id)
        .to_owned();
    let display_name = payload
        .get("display_name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(&identifier)
        .to_owned();

    if user_id.is_empty() {
        user_id = decode_sub_from_jwt(&access_token)?;
    }

    if user_id.is_empty() {
        bail!("错误：无法从 auth access token 中解析 user_id");
    }

    let role_id = ctx
        .run_psql(
            &ctx.db_name,
            "WITH upserted AS (
               INSERT INTO admin_roles (name, code)
               VALUES ('Root', 'root')
               ON CONFLICT (code) DO NOTHING
               RETURNING id
             )
             SELECT id FROM upserted
             UNION ALL
             SELECT id FROM admin_roles WHERE code = 'root'
             LIMIT 1;",
        )?
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then_some(trimmed.to_owned())
        })
        .context("错误：初始化 root 角色失败")?;

    ctx.run_psql(
        &ctx.db_name,
        &format!(
            "INSERT INTO admin_users (user_id, display_id, display_name, remark, status)
             VALUES (
               '{user_id}',
               '{}',
               '{}',
               NULL,
               'enabled'
             )
             ON CONFLICT (user_id) DO UPDATE
             SET display_id = EXCLUDED.display_id,
                 display_name = EXCLUDED.display_name,
                 remark = EXCLUDED.remark,
                 status = EXCLUDED.status;",
            sql_escape(&display_id),
            sql_escape(&display_name)
        ),
    )?;

    ctx.run_psql(
        &ctx.db_name,
        &format!(
            "INSERT INTO admin_user_roles (user_id, role_id)
             SELECT '{user_id}'::uuid, id
             FROM admin_roles
             WHERE code = 'root'
             ON CONFLICT (user_id, role_id) DO NOTHING;"
        ),
    )?;

    ctx.run_psql(
        &ctx.db_name,
        &format!("DELETE FROM admin_role_permissions WHERE role_id = {role_id};"),
    )?;

    let user_role_count = ctx
        .run_psql(
            &ctx.db_name,
            &format!(
                "SELECT COUNT(*)
                 FROM admin_user_roles
                 WHERE user_id = '{user_id}'::uuid
                   AND role_id = {role_id};"
            ),
        )?
        .trim()
        .to_owned();

    if user_role_count != "1" {
        bail!("错误：root 用户角色绑定失败");
    }

    println!("root 初始化完成");
    println!("auth user_id: {user_id}");
    println!("display_id: {display_id}");
    println!("display_name: {display_name}");
    println!("role_id: {role_id}");
    println!("auth login url: {login_url}");
    Ok(())
}

fn read_jwt_verify_url(path: &std::path::Path) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("读取配置文件 {} 失败", path.display()))?;

    let mut in_jwt_verify = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_jwt_verify = line == "[jwt_verify]";
            continue;
        }

        if !in_jwt_verify || !line.starts_with("url") {
            continue;
        }

        let Some((_, value)) = line.split_once('=') else {
            continue;
        };

        let value = value.trim().trim_matches('"');
        if !value.is_empty() {
            return Ok(value.to_owned());
        }
    }

    bail!("错误：config/services.toml 中缺少 jwt_verify.url")
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_owned())
}

fn env_or_prompt(var_name: &str, label: &str) -> Result<String> {
    match env::var(var_name) {
        Ok(value) if !value.trim().is_empty() => Ok(value.trim().to_owned()),
        _ => prompt(label),
    }
}

fn env_or_prompt_password(var_name: &str, label: &str) -> Result<String> {
    match env::var(var_name) {
        Ok(value) if !value.trim().is_empty() => Ok(value.trim().to_owned()),
        _ => prompt_password(label),
    }
}

fn prompt_password(label: &str) -> Result<String> {
    print!("{label}");
    io::stdout().flush()?;

    #[cfg(unix)]
    {
        use std::process::Command;
        let interactive = std::io::IsTerminal::is_terminal(&io::stdin());
        if interactive {
            let _ = Command::new("stty").arg("-echo").status();
        }
        let value = prompt("")?;
        if interactive {
            let _ = Command::new("stty").arg("echo").status();
            println!();
        }
        Ok(value)
    }

    #[cfg(not(unix))]
    {
        prompt("")
    }
}

fn find_string_field(value: &Value, field: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(text) = map.get(field).and_then(Value::as_str) {
                return Some(text.to_owned());
            }

            map.values()
                .find_map(|nested| find_string_field(nested, field))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|nested| find_string_field(nested, field)),
        _ => None,
    }
}

fn decode_sub_from_jwt(token: &str) -> Result<String> {
    let payload = token.split('.').nth(1).context("错误：非法 JWT 格式")?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .context("错误：JWT payload base64 解码失败")?;
    let value: Value =
        serde_json::from_slice(&decoded).context("错误：JWT payload JSON 解析失败")?;

    value
        .get("sub")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .context("错误：JWT payload 缺少 sub")
}
