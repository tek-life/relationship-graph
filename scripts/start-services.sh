#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 启动 v1.3 个人关系图谱服务端所需的常驻 AI 服务
# 运行环境：WSL2 + Ubuntu 24.04
# =============================================================================

echo "==> 启动 Ollama 服务（后台）"
if ! pgrep -x "ollama" >/dev/null; then
  nohup ollama serve >/tmp/ollama.log 2>&1 &
  echo "Ollama 服务已启动，日志：/tmp/ollama.log"
else
  echo "Ollama 服务已在运行"
fi

echo "==> 检查 Ollama 模型"
ollama list

echo "==> 检查 PostgreSQL 容器"
if docker ps --format '{{.Names}}' | grep -q "^relationship-graph-pg$"; then
  echo "PostgreSQL 容器正在运行"
else
  echo "PostgreSQL 容器未运行，请执行 ./scripts/setup-postgres-docker.sh"
fi
