#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 使用 Docker 启动 v1.3 个人关系图谱的 PostgreSQL 数据库
# 运行环境：WSL2 + Ubuntu 24.04
# 前提：
#   1. 已安装 Docker 并加入 docker 组
#   2. 若网络受限，需先为 Docker daemon 配置 HTTP 代理
#      （参考 scripts/setup-proxy.sh 探测宿主机代理，再写入 /etc/systemd/system/docker.service.d/http-proxy.conf）
# =============================================================================

CONTAINER_NAME="relationship-graph-pg"
DB_NAME="${DB_NAME:-relationship_graph}"
DB_USER="${DB_USER:-rguser}"
DB_PASS="${DB_PASS:-rgpass}"
DB_PORT="${DB_PORT:-5432}"

echo "==> 拉取 PostgreSQL 17 镜像"
docker pull postgres:17-alpine

echo "==> 停止并删除旧容器（如果存在）"
docker rm -f "${CONTAINER_NAME}" 2>/dev/null || true

echo "==> 启动 PostgreSQL 容器"
docker run -d \
  --name "${CONTAINER_NAME}" \
  -e POSTGRES_DB="${DB_NAME}" \
  -e POSTGRES_USER="${DB_USER}" \
  -e POSTGRES_PASSWORD="${DB_PASS}" \
  -p "${DB_PORT}:5432" \
  -v relationship-graph-pg-data:/var/lib/postgresql/data \
  --restart unless-stopped \
  postgres:17-alpine

echo "==> 等待数据库就绪"
until docker exec "${CONTAINER_NAME}" pg_isready -U "${DB_USER}" -d "${DB_NAME}" >/dev/null 2>&1; do
  sleep 1
done

echo "==> PostgreSQL 已启动"
echo "连接字符串：postgresql://${DB_USER}:${DB_PASS}@localhost:${DB_PORT}/${DB_NAME}"
