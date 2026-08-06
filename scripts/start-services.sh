#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 启动 v1.3 个人关系图谱服务端所需的常驻服务
# 运行环境：WSL2 + Ubuntu 24.04
#
# 说明：
#   - 自动检测运行模式：存在 dist/ 目录则为生产模式，否则为开发模式
#   - Ollama：AI 模型推理服务（端口 11434）
#   - Axum 服务端：HTTP API（端口 8790，cargo run --release）
#   - 前端（生产模式）：Caddy 反向代理（端口 8080）或 Python http.server（端口 8000）兜底
#   - 前端（开发模式）：Vite 开发服务器（端口 1420）
# =============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DATA_DIR="${HOME}/.local/share/relationship-graph"
SERVER_LOG="/tmp/relationship-graph-server.log"
FRONTEND_LOG="/tmp/relationship-graph-frontend.log"
CADDY_LOG="/tmp/relationship-graph-caddy.log"
RG_OLLAMA_TIMEOUT_SECS="${RG_OLLAMA_TIMEOUT_SECS:-45}"
RG_OLLAMA_CHAT_TIMEOUT_SECS="${RG_OLLAMA_CHAT_TIMEOUT_SECS:-120}"

# 运行模式检测
if [ -d "${PROJECT_DIR}/dist" ]; then
  RUN_MODE="production"
else
  RUN_MODE="development"
fi

mkdir -p "${APP_DATA_DIR}"

echo ""
echo "=========================================="
echo "  个人关系图谱 - 服务启动（${RUN_MODE} 模式）"
echo "=========================================="
echo ""

# =============================================================================
# 1. 启动 Ollama 服务
# =============================================================================
echo "==> 启动 Ollama 服务（后台，端口 11434）"
if ! pgrep -x "ollama" >/dev/null; then
  nohup ollama serve >/tmp/ollama.log 2>&1 &
  echo "  Ollama 服务已启动，日志：/tmp/ollama.log"
  # 等待服务就绪
  for i in {1..30}; do
    if curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  if curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
    echo "  Ollama 服务已就绪"
  else
    echo "  [WARN] Ollama 启动中，请稍后检查"
  fi
else
  echo "  Ollama 服务已在运行"
fi

echo "  当前模型列表："
ollama list 2>/dev/null || echo "  （无法获取模型列表）"

# =============================================================================
# 2. 启动 Axum 服务端
# =============================================================================
echo ""
echo "==> 启动 Axum 服务端（后台，端口 8790）"
if ! lsof -i :8790 >/dev/null 2>&1; then
  cd "${PROJECT_DIR}/server"
  source "${HOME}/.cargo/env" 2>/dev/null || true

  # 优先使用编译后的二进制，否则用 cargo run --release
  SERVER_BIN="${PROJECT_DIR}/server/target/release/relationship-graph-server"
  if [ -f "${SERVER_BIN}" ]; then
    nohup env \
      RG_OLLAMA_TIMEOUT_SECS="${RG_OLLAMA_TIMEOUT_SECS}" \
      RG_OLLAMA_CHAT_TIMEOUT_SECS="${RG_OLLAMA_CHAT_TIMEOUT_SECS}" \
      "${SERVER_BIN}" >"${SERVER_LOG}" 2>&1 &
    echo "  Axum 服务端已启动（二进制），日志：${SERVER_LOG}"
  else
    nohup env \
      RG_OLLAMA_TIMEOUT_SECS="${RG_OLLAMA_TIMEOUT_SECS}" \
      RG_OLLAMA_CHAT_TIMEOUT_SECS="${RG_OLLAMA_CHAT_TIMEOUT_SECS}" \
      cargo run --release >"${SERVER_LOG}" 2>&1 &
    echo "  Axum 服务端已启动（cargo run --release），日志：${SERVER_LOG}"
  fi

  cd "${PROJECT_DIR}"

  # 等待服务就绪
  for i in {1..60}; do
    if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
    echo "  Axum 服务端已就绪"
  else
    echo "  [WARN] Axum 服务端启动中，请查看日志：${SERVER_LOG}"
  fi
else
  echo "  Axum 服务端已在运行（端口 8790）"
fi

# =============================================================================
# 3. 启动前端服务
# =============================================================================
echo ""
if [ "${RUN_MODE}" = "production" ]; then
  echo "==> 启动前端静态文件服务（生产模式，dist/ 目录）"

  # 优先使用 Caddy 反向代理
  if command -v caddy &>/dev/null; then
    if ! lsof -i :8080 >/dev/null 2>&1; then
      # 确保日志目录存在
      sudo mkdir -p /var/log/relationship-graph 2>/dev/null || true
      nohup caddy run --config "${PROJECT_DIR}/scripts/Caddyfile" >"${CADDY_LOG}" 2>&1 &
      echo "  Caddy 反向代理已启动（端口 8080），日志：${CADDY_LOG}"
      # 等待就绪
      for i in {1..30}; do
        if curl -s http://localhost:8080 >/dev/null 2>&1; then
          break
        fi
        sleep 1
      done
      if curl -s http://localhost:8080 >/dev/null 2>&1; then
        echo "  Caddy 前端服务已就绪（http://localhost:8080）"
      else
        echo "  [WARN] Caddy 启动中，请查看日志：${CADDY_LOG}"
      fi
    else
      echo "  Caddy 已在运行（端口 8080）"
    fi
  else
    echo "  [提示] 未安装 Caddy，使用 Python http.server 作为静态文件服务兜底"
    if ! lsof -i :8000 >/dev/null 2>&1; then
      cd "${PROJECT_DIR}/dist"
      nohup python3 -m http.server 8000 >"${FRONTEND_LOG}" 2>&1 &
      cd "${PROJECT_DIR}"
      echo "  Python http.server 已启动（端口 8000），日志：${FRONTEND_LOG}"
      # 等待就绪
      for i in {1..15}; do
        if curl -s http://localhost:8000 >/dev/null 2>&1; then
          break
        fi
        sleep 1
      done
      if curl -s http://localhost:8000 >/dev/null 2>&1; then
        echo "  前端静态服务已就绪（http://localhost:8000）"
      else
        echo "  [WARN] Python http.server 启动中，请查看日志：${FRONTEND_LOG}"
      fi
    else
      echo "  Python http.server 已在运行（端口 8000）"
    fi
    echo "  [建议] 安装 Caddy 获得更好的生产体验：sudo apt install caddy"
  fi
else
  echo "==> 启动前端开发服务器（开发模式，Vite，端口 1420）"
  if ! lsof -i :1420 >/dev/null 2>&1; then
    if [ ! -d "${PROJECT_DIR}/node_modules" ]; then
      echo "  [ERROR] 前端依赖未安装，请先运行 npm install"
      exit 1
    fi
    (
      cd "${PROJECT_DIR}"
      nohup npm run dev >"${FRONTEND_LOG}" 2>&1 &
    )
    echo "  Vite 开发服务器已启动，日志：${FRONTEND_LOG}"
    for i in {1..60}; do
      if curl -s http://localhost:1420 >/dev/null 2>&1; then
        break
      fi
      sleep 1
    done
    if curl -s http://localhost:1420 >/dev/null 2>&1; then
      echo "  Vite 开发服务器已就绪"
    else
      echo "  [WARN] Vite 开发服务器启动中，请查看日志：${FRONTEND_LOG}"
    fi
  else
    echo "  Vite 开发服务器已在运行（端口 1420）"
  fi
fi

# =============================================================================
# 4. 健康检查
# =============================================================================
echo ""
echo "==> 健康检查"

# Axum 后端健康检查
if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
  echo "  Axum 后端健康检查：通过"
else
  echo "  Axum 后端健康检查：失败（请查看日志 ${SERVER_LOG}）"
fi

# 前端健康检查
if [ "${RUN_MODE}" = "production" ]; then
  if curl -s http://localhost:8080 >/dev/null 2>&1; then
    echo "  前端（Caddy）健康检查：通过"
  elif curl -s http://localhost:8000 >/dev/null 2>&1; then
    echo "  前端（http.server）健康检查：通过"
  else
    echo "  前端健康检查：未检测到服务"
  fi
else
  if curl -s http://localhost:1420 >/dev/null 2>&1; then
    echo "  前端（Vite）健康检查：通过"
  else
    echo "  前端健康检查：失败（请查看日志 ${FRONTEND_LOG}）"
  fi
fi

# =============================================================================
# 服务状态汇总
# =============================================================================
echo ""
echo "============================================================"
echo "服务状态汇总（${RUN_MODE} 模式）："

# Ollama
if pgrep -x "ollama" >/dev/null; then
  echo "  Ollama       - http://localhost:11434  - [运行中]"
else
  echo "  Ollama       - http://localhost:11434  - [未运行]"
fi

# Axum
if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
  echo "  Axum 服务端  - http://localhost:8790   - [已就绪]"
else
  echo "  Axum 服务端  - http://localhost:8790   - [启动中]"
  echo "    查看日志：${SERVER_LOG}"
fi

# 前端
if [ "${RUN_MODE}" = "production" ]; then
  if curl -s http://localhost:8080 >/dev/null 2>&1; then
    echo "  前端(Caddy)  - http://localhost:8080   - [已就绪]"
  elif curl -s http://localhost:8000 >/dev/null 2>&1; then
    echo "  前端(http)   - http://localhost:8000   - [已就绪]"
  else
    echo "  前端静态服务 -                          - [未运行]"
  fi
else
  if curl -s http://localhost:1420 >/dev/null 2>&1; then
    echo "  前端(Vite)   - http://localhost:1420   - [已就绪]"
  else
    echo "  前端(Vite)   - http://localhost:1420   - [启动中]"
    echo "    查看日志：${FRONTEND_LOG}"
  fi
fi

echo ""
echo "  API 健康检查：curl http://localhost:8790/api/health"
echo "============================================================"
