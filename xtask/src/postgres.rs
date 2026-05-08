use std::fs;

use anyhow::{Context, Result};

use crate::{common::XtaskContext, init_db, process};

pub fn up() -> Result<()> {
    let ctx = XtaskContext::load()?;
    process::require_command("docker")?;

    fs::create_dir_all(&ctx.pg_data_dir)
        .with_context(|| format!("创建目录 {} 失败", ctx.pg_data_dir.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&ctx.pg_data_dir, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("设置目录权限 {} 失败", ctx.pg_data_dir.display()))?;
    }

    if ctx.docker_container_running()? {
        println!("PostgreSQL 容器已在运行：{}", ctx.db_container_name);
    } else if ctx.docker_container_exists()? {
        println!("启动已有容器：{}", ctx.db_container_name);
        process::run("docker", &["start", &ctx.db_container_name])?;
    } else {
        println!("创建并启动 PostgreSQL 容器：{}", ctx.db_container_name);
        let port_mapping = format!("{}:5432", ctx.db_port);
        let user_env = format!("POSTGRES_USER={}", ctx.db_user);
        let password_env = format!("POSTGRES_PASSWORD={}", ctx.db_password);
        let volume = format!("{}:/var/lib/postgresql/data", ctx.pg_data_dir.display());

        process::run(
            "docker",
            &[
                "run",
                "-d",
                "--name",
                &ctx.db_container_name,
                "--restart",
                "unless-stopped",
                "-e",
                &user_env,
                "-e",
                &password_env,
                "-e",
                "POSTGRES_DB=postgres",
                "-v",
                &volume,
                "-p",
                &port_mapping,
                &ctx.pg_image,
            ],
        )?;
    }

    ctx.wait_for_postgres()?;
    init_db::run()?;
    println!("PostgreSQL 已就绪");
    println!("容器名: {}", ctx.db_container_name);
    println!("数据库: {}", ctx.db_name);
    println!("地址: {}", ctx.database_url);
    Ok(())
}

pub fn status() -> Result<()> {
    let ctx = XtaskContext::load()?;
    process::require_command("docker")?;

    if ctx.docker_container_running()? {
        let filter = format!("name=^{}$", ctx.db_container_name);
        let output = process::output(
            "docker",
            &[
                "ps",
                "--filter",
                &filter,
                "--format",
                "容器 {{.Names}} 正在运行，端口：{{.Ports}}",
            ],
        )?;
        print!("{output}");
    } else if ctx.docker_container_exists()? {
        let filter = format!("name=^{}$", ctx.db_container_name);
        let output = process::output(
            "docker",
            &[
                "ps",
                "-a",
                "--filter",
                &filter,
                "--format",
                "容器 {{.Names}} 已存在但未运行，状态：{{.Status}}",
            ],
        )?;
        print!("{output}");
    } else {
        println!("容器不存在：{}", ctx.db_container_name);
    }

    Ok(())
}

pub fn stop() -> Result<()> {
    let ctx = XtaskContext::load()?;
    process::require_command("docker")?;

    if ctx.docker_container_running()? {
        process::run("docker", &["stop", &ctx.db_container_name])?;
        println!("已停止容器：{}", ctx.db_container_name);
    } else {
        println!("容器未运行：{}", ctx.db_container_name);
    }

    Ok(())
}

pub fn rm() -> Result<()> {
    let ctx = XtaskContext::load()?;
    process::require_command("docker")?;

    if ctx.docker_container_exists()? {
        process::run("docker", &["rm", "-f", &ctx.db_container_name])?;
        println!("已删除容器：{}", ctx.db_container_name);
    } else {
        println!("容器不存在：{}", ctx.db_container_name);
    }

    Ok(())
}
