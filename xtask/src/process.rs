use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

pub fn require_command(command: &str) -> Result<()> {
    let status = if cfg!(windows) {
        Command::new("where")
            .arg(command)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    } else {
        Command::new("sh")
            .args(["-c", "command -v \"$1\" >/dev/null 2>&1", "sh"])
            .arg(command)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    };

    if status
        .with_context(|| format!("检查命令 '{command}' 失败"))?
        .success()
    {
        Ok(())
    } else {
        bail!("错误：缺少命令 '{command}'")
    }
}

pub fn run(command: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(command)
        .args(args)
        .status()
        .with_context(|| format!("执行命令 '{command}' 失败"))?;

    if status.success() {
        Ok(())
    } else {
        bail!("命令执行失败：{} {}", command, args.join(" "))
    }
}

pub fn run_with_env(command: &str, args: &[&str], envs: &[(&str, &str)]) -> Result<()> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    for (key, value) in envs {
        cmd.env(key, value);
    }

    let status = cmd
        .status()
        .with_context(|| format!("执行命令 '{command}' 失败"))?;

    if status.success() {
        Ok(())
    } else {
        bail!("命令执行失败：{} {}", command, args.join(" "))
    }
}

pub fn output(command: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .with_context(|| format!("执行命令 '{command}' 失败"))?;

    if !output.status.success() {
        bail!(
            "命令执行失败：{} {}\n{}",
            command,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
