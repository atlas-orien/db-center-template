#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MIGRATION_DIR="$PROJECT_ROOT/crates/migration"
ENTITY_DIR="$PROJECT_ROOT/crates/repo/src/entity"

load_env() {
  local env_database_url="${DATABASE_URL:-}"
  local env_app_database_url="${APP_DATABASE_URL:-}"

  if [ -f "$PROJECT_ROOT/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$PROJECT_ROOT/.env"
    set +a
  fi

  if [ -n "$env_database_url" ]; then
    DATABASE_URL="$env_database_url"
  fi
  if [ -n "$env_app_database_url" ]; then
    APP_DATABASE_URL="$env_app_database_url"
  fi

  : "${APP_DATABASE_URL:=postgres://postgres:123456@localhost:15432/app}"
  : "${DATABASE_URL:=$APP_DATABASE_URL}"
  : "${DB_CONTAINER_NAME:=postgres}"
  : "${PG_IMAGE:=postgres:16}"
  : "${PG_DATA_DIR:=$HOME/db/db-center-template-postgres}"

  export DATABASE_URL
}

parse_database_url() {
  local url="${DATABASE_URL#postgres://}"
  url="${url#postgresql://}"

  local auth_part="${url%%@*}"
  local host_db_part="${url#*@}"
  local host_port="${host_db_part%%/*}"

  DB_USER="${auth_part%%:*}"
  DB_PASSWORD="${auth_part#*:}"
  DB_NAME="${host_db_part#*/}"
  DB_NAME="${DB_NAME%%\?*}"

  if [[ "$host_port" == *:* ]]; then
    DB_HOST="${host_port%%:*}"
    DB_PORT="${host_port##*:}"
  else
    DB_HOST="$host_port"
    DB_PORT="5432"
  fi
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "错误：缺少命令 '$1'"
    exit 1
  fi
}

require_sea_orm_cli() {
  require_command sea-orm-cli
}

assert_safe_identifier() {
  local value="$1"
  local label="$2"
  if [[ ! "$value" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
    echo "错误：${label} '${value}' 非法，只允许字母、数字和下划线，且不能以数字开头"
    exit 1
  fi
}

docker_container_exists() {
  if ! command -v docker >/dev/null 2>&1; then
    return 1
  fi
  docker ps -a --format '{{.Names}}' | grep -qx "$DB_CONTAINER_NAME"
}

docker_container_running() {
  if ! command -v docker >/dev/null 2>&1; then
    return 1
  fi
  docker ps --format '{{.Names}}' | grep -qx "$DB_CONTAINER_NAME"
}

wait_for_postgres() {
  local retries=30
  while (( retries > 0 )); do
    if docker exec "$DB_CONTAINER_NAME" pg_isready -U "$DB_USER" -d postgres >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
    retries=$((retries - 1))
  done

  echo "错误：PostgreSQL 启动超时"
  exit 1
}

run_psql() {
  local database="$1"
  local sql="$2"

  if docker_container_exists; then
    docker exec -i "$DB_CONTAINER_NAME" \
      psql -v ON_ERROR_STOP=1 -U "$DB_USER" -d "$database" -tAc "$sql"
    return 0
  fi

  if command -v psql >/dev/null 2>&1; then
    local target_url="postgres://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${database}"
    psql "$target_url" -v ON_ERROR_STOP=1 -tAc "$sql"
    return 0
  fi

  echo "错误：既找不到 Docker 容器，也找不到本地 psql，无法执行 SQL"
  exit 1
}
