# Legacy Scripts

The shell scripts in this directory are kept as compatibility references.

Use `cargo xtask` for new work:

```bash
cargo xtask --help
```

Why:

- `scripts/*.sh` require a Unix-like shell.
- Windows users may not have Bash.
- AI agents and CI should use one cross-platform command surface.
- This Rust workspace already requires Cargo, so `cargo xtask` has no extra
  runtime dependency.

## Mapping

| Shell script | Replacement |
| --- | --- |
| `./scripts/init.sh` | `cargo xtask init` |
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

## Notes

The legacy scripts still read `.env` and use `DB_CONTAINER_NAME`, whose default
is `postgres`.

Prefer updating `xtask` when adding new automation.
