# xtask

`xtask` 是当前 Rust workspace 的项目自动化 CLI。

以后优先使用它，不要直接调用 `scripts/*.sh`。它用 Rust 编写，并通过
Cargo 运行，所以 Windows、macOS、Linux、CI 和 AI 编码工具都可以使用同一套入口，
不需要额外依赖 Bash、Make、Just、Node 或 Python。

## 为什么使用 xtask

旧的 `scripts/*.sh` 更适合 Linux/macOS，在 Windows 原生环境下不稳定。

这个项目本来就需要 Rust 和 Cargo，所以把自动化脚本收敛到一个 Rust CLI：

```bash
cargo xtask <command>
```

第一次运行时 Cargo 会自动编译 `xtask`。不需要手动把它安装到全局 `bin` 目录。

## 配置

`xtask` 会读取仓库根目录下的 `.env`。

本地开发示例：

```env
APP_DATABASE_URL=postgres://postgres:123456@localhost:15433/test
DATABASE_URL=postgres://postgres:123456@localhost:15433/test
DB_CONTAINER_NAME=postgres
PG_IMAGE=postgres:16
```

说明：

- `DATABASE_URL` 用于 migration 和 entity 生成。
- 本地开发时，`APP_DATABASE_URL` 通常应该和 `DATABASE_URL` 指向同一个数据库。
- `DB_CONTAINER_NAME` 默认是 `postgres`。
- 如果本机 `15432` 端口已经被旧容器占用，可以改用其它宿主机端口，例如 `15433`。
  修改端口时，两个数据库 URL 都要一起改。

## 常用流程

初始化 `.env`：

```bash
cargo xtask init-env
```

用 Docker 启动 PostgreSQL，并确保目标数据库存在：

```bash
cargo xtask db up
```

执行数据库迁移：

```bash
cargo xtask migrate up
```

根据当前数据库表结构生成 SeaORM entity：

```bash
cargo xtask entity generate
```

初始化内置权限：

```bash
cargo xtask init-permissions
cargo xtask init-app-permissions
```

把 auth 用户绑定为后台 root：

```bash
cargo xtask init-root
```

查看可用命令：

```bash
cargo xtask --help
cargo xtask db --help
cargo xtask migrate --help
```

## 命令说明

### 项目初始化

```bash
cargo xtask init-env
```

把 `.env.example` 复制为 `.env`，并把 `config/*-example.toml` 复制为对应的
`config/*.toml`。

### 数据库

```bash
cargo xtask db up
cargo xtask db status
cargo xtask db stop
cargo xtask db rm
cargo xtask db init
cargo xtask db clear
cargo xtask db truncate <table>
```

说明：

- `db up` 会启动 Docker PostgreSQL 容器，并按 `.env` 中的数据库名创建目标数据库。
- `db clear` 会删除并重建 `public` schema，是破坏性操作。
- `db truncate <table>` 会清空指定表并重置自增，是破坏性操作。

### 数据库迁移

```bash
cargo xtask migrate up
cargo xtask migrate down
cargo xtask migrate fresh
cargo xtask migrate refresh
cargo xtask migrate reset
cargo xtask migrate status
cargo xtask migrate generate <name>
```

这些命令会包装 `sea-orm-cli migrate`，并把 `xtask` 解析后的 `DATABASE_URL`
传给子进程，避免 migration 连接到错误数据库。

### 生成 Entity

```bash
cargo xtask entity generate
```

根据当前连接数据库里的所有表生成 SeaORM entity 文件。

注意：`sea-orm-cli generate entity` 扫描的是数据库里的表，不是 migration 文件。
如果当前连接的数据库里有旧表或无关表，也会生成对应的 entity。

### 重建数据库并生成 Entity

```bash
cargo xtask fresh-db
```

执行顺序：

1. `cargo xtask migrate refresh`
2. `cargo xtask entity generate`

这是破坏性操作。

### 权限初始化

```bash
cargo xtask init-permissions
cargo xtask init-app-permissions
cargo xtask init-root
```

说明：

- `init-permissions` 初始化后台权限、菜单和内置后台角色。
- `init-app-permissions` 初始化普通用户权限和 `free` 角色。
- `init-root` 会登录 `auth` 服务，并把该 auth 用户绑定到本地 `root` 后台角色。

`init-root` 会从 `config/services.toml` 读取 `jwt_verify.url`。

## 旧脚本映射

旧的 shell 脚本会暂时保留作为兼容参考，但新的自动化流程应该使用 `cargo xtask`。

| 旧命令 | 新命令 |
| --- | --- |
| `./scripts/init.sh` | `cargo xtask init-env` |
| `./scripts/postgres.sh up` | `cargo xtask db up` |
| `./scripts/postgres.sh status` | `cargo xtask db status` |
| `./scripts/postgres.sh stop` | `cargo xtask db stop` |
| `./scripts/postgres.sh rm` | `cargo xtask db rm` |
| `./scripts/init_db.sh` | `cargo xtask db init` |
| `./scripts/clear_db.sh` | `cargo xtask db clear` |
| `./scripts/truncate_table.sh <table>` | `cargo xtask db truncate <table>` |
| `./scripts/migrate.sh up` | `cargo xtask migrate up` |
| `./scripts/migrate.sh down` | `cargo xtask migrate down` |
| `./scripts/migrate.sh fresh` | `cargo xtask migrate fresh` |
| `./scripts/migrate.sh refresh` | `cargo xtask migrate refresh` |
| `./scripts/migrate.sh reset` | `cargo xtask migrate reset` |
| `./scripts/migrate.sh status` | `cargo xtask migrate status` |
| `./scripts/migrate.sh generate <name>` | `cargo xtask migrate generate <name>` |
| `./scripts/generate_entity.sh` | `cargo xtask entity generate` |
| `./scripts/fresh_db.sh` | `cargo xtask fresh-db` |
| `./scripts/init_permissions.sh` | `cargo xtask init-permissions` |
| `./scripts/init_app_permissions.sh` | `cargo xtask init-app-permissions` |
| `./scripts/init_root.sh` | `cargo xtask init-root` |
