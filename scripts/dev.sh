#!/usr/bin/env bash
set -e

# =============================================================================
# 个人关系图谱 - 轻量开发启动脚本（仅用于开发模式）
#
# 用途：快速启动开发环境（Ollama + Axum 后端 + Vite 前端）
# 用法：./scripts/dev.sh
#
# 注意：本脚本仅用于开发模式，不启动 Caddy 反向代理。
#       生产部署请使用 ./scripts/deploy.sh + ./scripts/start-services.sh
#
# 启动的服务：
#   1. Ollama（如果未运行）
#   2. Axum 后端（cargo run，非 release 模式，编译更快）
#   3. Vite 前端开发服务器（npm run dev，端口 1420）
# =============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[✓]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[!]${NC} $*"; }
log_step() { echo -e "${BLUE}[→]${NC} $*"; }

echo ""
echo "=========================================="
echo "  个人关系图谱 - 开发环境启动"
echo "  （仅开发模式，不启动 Caddy 反向代理）"
echo "=========================================="
echo ""

# =============================================================================
# 1. 启动 Ollama 服务
# =============================================================================
log_step "检查 Ollama 服务..."
if pgrep -x "ollama" >/dev/null; then
  log_info "Ollama 已在运行"
else
  log_info "启动 Ollama 服务..."
  nohup ollama serve >/tmp/ollama.log 2>&1 &
  # 等待就绪
  for i in {1..15}; do
    if curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  if curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
    log_info "Ollama 服务已启动"
  else
    log_warn "Ollama 启动中，请稍后检查（日志：/tmp/ollama.log）"
  fi
fi

# =============================================================================
# 2. 启动 Axum 后端（开发模式：cargo run，非 release）
# =============================================================================
log_step "检查 Axum 后端服务..."
if lsof -i :8790 >/dev/null 2>&1; then
  log_info "Axum 后端已在运行（端口 8790）"
else
  log_info "启动 Axum 后端（cargo run，开发模式）..."
  cd "${PROJECT_DIR}/server"
  source "${HOME}/.cargo/env" 2>/dev/null || true
  nohup cargo run >/tmp/relationship-graph-server.log 2>&1 &
  # 等待就绪
  for i in {1..30}; do
    if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
    log_info "Axum 后端已启动"
  else
    log_warn "Axum 后端启动中（日志：/tmp/relationship-graph-server.log）"
  fi
fi

# =============================================================================
# 3. 启动 Vite 前端开发服务器（不启动 Caddy）
# =============================================================================
log_step "检查 Vite 前端服务..."
if lsof -i :1420 >/dev/null 2>&1; then
  log_info "Vite 前端已在运行（端口 1420）"
else
  if [ ! -d "${PROJECT_DIR}/node_modules" ]; then
    log_warn "前端依赖未安装，先运行 npm install..."
    cd "${PROJECT_DIR}"
    npm install
  fi
  log_info "启动 Vite 前端..."
  cd "${PROJECT_DIR}"
  nohup npm run dev >/tmp/relationship-graph-frontend.log 2>&1 &
  # 等待就绪
  for i in {1..30}; do
    if curl -s http://localhost:1420 >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  if curl -s http://localhost:1420 >/dev/null 2>&1; then
    log_info "Vite 前端已启动"
  else
    log_warn "Vite 前端启动中（日志：/tmp/relationship-graph-frontend.log）"
  fi
fi

log_info "开发模式不启动 Caddy 反向代理（生产部署请使用 ./scripts/deploy.sh）"

# =============================================================================
# 状态汇总
# =============================================================================
echo ""
echo "=========================================="
echo "  服务状态（开发模式）"
echo "=========================================="

if pgrep -x "ollama" >/dev/null; then
  echo "  Ollama      http://localhost:11434   [运行中]"
else
  echo "  Ollama      http://localhost:11434   [未运行]"
fi

if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
  echo "  Axum 后端   http://localhost:8790    [已就绪]"
else
  echo "  Axum 后端   http://localhost:8790    [启动中]"
fi

if curl -s http://localhost:1420 >/dev/null 2>&1; then
  echo "  Vite 前端   http://localhost:1420    [已就绪]"
else
  echo "  Vite 前端   http://localhost:1420    [启动中]"
fi

echo "  Caddy       （开发模式不启动）"
echo "=========================================="
echo ""
echo "开发环境启动完成！"
echo "  前端访问：http://localhost:1420"
echo "  API 地址：http://localhost:8790"
echo "  健康检查：curl http://localhost:8790/api/health"
echo ""
