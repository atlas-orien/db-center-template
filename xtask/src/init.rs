use std::fs;

use anyhow::{Context, Result, bail};

use crate::common::project_root;

pub fn run() -> Result<()> {
    let root = project_root()?;
    let source = root.join(".env.example");
    let target = root.join(".env");

    if !source.exists() {
        bail!("错误：找不到 {}", source.display());
    }

    fs::copy(&source, &target)
        .with_context(|| format!("复制 {} 到 {} 失败", source.display(), target.display()))?;

    println!("已初始化 .env");
    Ok(())
}
