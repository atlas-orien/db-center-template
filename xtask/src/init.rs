use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

use crate::common::project_root;

pub fn run() -> Result<()> {
    let root = project_root()?;
    copy_required(&root.join(".env.example"), &root.join(".env"))?;

    let config_dir = root.join("config");
    if config_dir.exists() {
        for entry in fs::read_dir(&config_dir)
            .with_context(|| format!("读取目录 {} 失败", config_dir.display()))?
        {
            let entry = entry?;
            let source = entry.path();
            let Some(file_name) = source.file_name().and_then(|name| name.to_str()) else {
                continue;
            };

            let Some(target_name) = file_name.strip_suffix("-example.toml") else {
                continue;
            };

            let target = config_dir.join(format!("{target_name}.toml"));
            copy_required(&source, &target)?;
        }
    }

    println!("本地 env 和 config 已初始化");
    Ok(())
}

fn copy_required(source: &Path, target: &Path) -> Result<()> {
    if !source.exists() {
        bail!("错误：找不到 {}", source.display());
    }

    fs::copy(source, target)
        .with_context(|| format!("复制 {} 到 {} 失败", source.display(), target.display()))?;

    println!("已初始化 {}", target.display());
    Ok(())
}
