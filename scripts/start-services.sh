#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 启动 v1.3 个人关系图谱服务端所需的常驻服务
# 运行环境：WSL2 + Ubuntu 24.04
# 说明：
#   - Ollama：AI 模型推理服务
#   - Axum 服务端：HTTP API（默认端口 8790）
#   - Vite 前端：开发服务器（默认端口 1420）
# =============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DATA_DIR="${HOME}/.local/share/relationship-graph"
SERVER_LOG="/tmp/relationship-graph-server.log"
FRONTEND_LOG="/tmp/relationship-graph-frontend.log"
RG_OLLAMA_TIMEOUT_SECS="${RG_OLLAMA_TIMEOUT_SECS:-45}"
RG_OLLAMA_CHAT_TIMEOUT_SECS="${RG_OLLAMA_CHAT_TIMEOUT_SECS:-120}"

mkdir -p "${APP_DATA_DIR}"

echo "==> 启动 Ollama 服务（后台）"
if ! pgrep -x "ollama" >/dev/null; then
  nohup ollama serve >/tmp/ollama.log 2>&1 &
  echo "Ollama 服务已启动，日志：/tmp/ollama.log"
  # 等待服务就绪
  for i in {1..30}; do
    if curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
else
  echo "Ollama 服务已在运行"
fi

echo "==> 检查 Ollama 模型"
ollama list

echo "==> 启动 Axum 服务端（后台，端口 8790）"
if ! lsof -i :8790 >/dev/null 2>&1; then
  cd "${PROJECT_DIR}/server"
  source "${HOME}/.cargo/env"
  nohup env \
    RG_OLLAMA_TIMEOUT_SECS="${RG_OLLAMA_TIMEOUT_SECS}" \
    RG_OLLAMA_CHAT_TIMEOUT_SECS="${RG_OLLAMA_CHAT_TIMEOUT_SECS}" \
    cargo run --release >"${SERVER_LOG}" 2>&1 &
  echo "Axum 服务端已启动，日志：${SERVER_LOG}"
  # 等待服务就绪
  for i in {1..60}; do
    if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
    echo "Axum 服务端已就绪"
  else
    echo "Axum 服务端启动中，请查看日志：${SERVER_LOG}"
  fi
else
  echo "Axum 服务端已在运行（端口 8790）"
fi

echo "==> 启动前端开发服务器（后台，端口 1420）"
if ! lsof -i :1420 >/dev/null 2>&1; then
  if [ ! -d "${PROJECT_DIR}/node_modules" ]; then
    echo "前端依赖未安装，请先运行 npm install 后再启动前端"
    exit 1
  fi
  (
    cd "${PROJECT_DIR}"
    nohup npm run dev >"${FRONTEND_LOG}" 2>&1 &
  )
  echo "前端开发服务器已启动，日志：${FRONTEND_LOG}"
  for i in {1..60}; do
    if curl -s http://localhost:1420 >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  if curl -s http://localhost:1420 >/dev/null 2>&1; then
    echo "前端开发服务器已就绪"
  else
    echo "前端开发服务器启动中，请查看日志：${FRONTEND_LOG}"
  fi
else
  echo "前端开发服务器已在运行（端口 1420）"
fi

echo "==> 服务状态检查完成"

# =============================================================================
# 服务状态汇总
# =============================================================================
echo ""
echo "============================================================"
echo "服务状态汇总："

# Ollama
if pgrep -x "ollama" >/dev/null; then
  echo "  Ollama     - http://localhost:11434  - [运行中]"
else
  echo "  Ollama     - http://localhost:11434  - [未运行]"
fi

# Axum
if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
  echo "  Axum服务端  - http://localhost:8790   - [已就绪]"
else
  echo "  Axum服务端  - http://localhost:8790   - [启动中]"
  echo "  查看日志：${SERVER_LOG}"
fi

# Frontend
if curl -s http://localhost:1420 >/dev/null 2>&1; then
  echo "  前端        - http://localhost:1420   - [已就绪]"
else
  echo "  前端        - http://localhost:1420   - [启动中]"
  echo "  查看日志：${FRONTEND_LOG}"
fi

echo "============================================================"
