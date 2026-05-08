use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use url::Url;

use crate::process;

#[derive(Debug, Clone)]
pub struct XtaskContext {
    pub project_root: PathBuf,
    pub migration_dir: PathBuf,
    pub entity_dir: PathBuf,
    pub database_url: String,
    pub db_container_name: String,
    pub pg_image: String,
    pub pg_data_dir: PathBuf,
    pub db_user: String,
    pub db_password: String,
    pub db_host: String,
    pub db_port: u16,
    pub db_name: String,
}

impl XtaskContext {
    pub fn load() -> Result<Self> {
        let project_root = project_root()?;

        let original_database_url = env::var("DATABASE_URL").ok();
        let original_app_database_url = env::var("APP_DATABASE_URL").ok();

        let env_path = project_root.join(".env");
        if env_path.exists() {
            dotenvy::from_path_override(&env_path)
                .with_context(|| format!("读取 {} 失败", env_path.display()))?;
        }

        let app_database_url = original_app_database_url
            .or_else(|| env::var("APP_DATABASE_URL").ok())
            .unwrap_or_else(|| "postgres://postgres:123456@localhost:15432/app".to_owned());
        let database_url = original_database_url
            .or_else(|| env::var("DATABASE_URL").ok())
            .unwrap_or(app_database_url);

        let parsed = parse_database_url(&database_url)?;

        Ok(Self {
            migration_dir: project_root.join("crates/migration"),
            entity_dir: project_root.join("crates/repo/src/entity"),
            db_container_name: env::var("DB_CONTAINER_NAME")
                .unwrap_or_else(|_| "postgres".to_owned()),
            pg_image: env::var("PG_IMAGE").unwrap_or_else(|_| "postgres:16".to_owned()),
            pg_data_dir: env::var("PG_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home_dir().join("db/db-center-template-postgres")),
            project_root,
            database_url,
            db_user: parsed.user,
            db_password: parsed.password,
            db_host: parsed.host,
            db_port: parsed.port,
            db_name: parsed.database,
        })
    }

    pub fn run_psql(&self, database: &str, sql: &str) -> Result<String> {
        if self.docker_container_exists()? {
            return process::output(
                "docker",
                &[
                    "exec",
                    "-i",
                    &self.db_container_name,
                    "psql",
                    "-v",
                    "ON_ERROR_STOP=1",
                    "-U",
                    &self.db_user,
                    "-d",
                    database,
                    "-tAc",
                    sql,
                ],
            );
        }

        if command_exists("psql") {
            let target_url = format!(
                "postgres://{}:{}@{}:{}/{}",
                self.db_user, self.db_password, self.db_host, self.db_port, database
            );
            return process::output("psql", &[&target_url, "-v", "ON_ERROR_STOP=1", "-tAc", sql]);
        }

        bail!("错误：既找不到 Docker 容器，也找不到本地 psql，无法执行 SQL")
    }

    pub fn docker_container_exists(&self) -> Result<bool> {
        if !command_exists("docker") {
            return Ok(false);
        }

        let names = process::output("docker", &["ps", "-a", "--format", "{{.Names}}"])?;
        Ok(names.lines().any(|name| name == self.db_container_name))
    }

    pub fn docker_container_running(&self) -> Result<bool> {
        if !command_exists("docker") {
            return Ok(false);
        }

        let names = process::output("docker", &["ps", "--format", "{{.Names}}"])?;
        Ok(names.lines().any(|name| name == self.db_container_name))
    }

    pub fn wait_for_postgres(&self) -> Result<()> {
        for _ in 0 .. 30 {
            let status = Command::new("docker")
                .args([
                    "exec",
                    &self.db_container_name,
                    "pg_isready",
                    "-U",
                    &self.db_user,
                    "-d",
                    "postgres",
                ])
                .status()
                .context("检查 PostgreSQL 状态失败")?;

            if status.success() {
                return Ok(());
            }

            thread::sleep(Duration::from_secs(1));
        }

        bail!("错误：PostgreSQL 启动超时")
    }
}

#[derive(Debug)]
struct ParsedDatabaseUrl {
    user: String,
    password: String,
    host: String,
    port: u16,
    database: String,
}

fn parse_database_url(database_url: &str) -> Result<ParsedDatabaseUrl> {
    let url = Url::parse(database_url).context("DATABASE_URL 不是合法 URL")?;
    let scheme = url.scheme();
    if scheme != "postgres" && scheme != "postgresql" {
        bail!("DATABASE_URL 必须使用 postgres:// 或 postgresql://")
    }

    let database = url.path().trim_start_matches('/').to_owned();
    if database.is_empty() {
        bail!("DATABASE_URL 缺少数据库名")
    }

    Ok(ParsedDatabaseUrl {
        user: url.username().to_owned(),
        password: url.password().unwrap_or("").to_owned(),
        host: url.host_str().unwrap_or("localhost").to_owned(),
        port: url.port().unwrap_or(5432),
        database,
    })
}

pub fn project_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .context("无法解析项目根目录")
}

pub fn command_exists(command: &str) -> bool {
    process::require_command(command).is_ok()
}

pub fn assert_safe_identifier(value: &str, label: &str) -> Result<()> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        bail!("错误：{label} 不能为空")
    };

    if !(first == '_' || first.is_ascii_alphabetic())
        || chars.any(|ch| !(ch == '_' || ch.is_ascii_alphanumeric()))
    {
        bail!("错误：{label} '{value}' 非法，只允许字母、数字和下划线，且不能以数字开头")
    }

    Ok(())
}

pub fn sql_escape(value: &str) -> String {
    value.replace('\'', "''")
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
