#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 个人关系图谱 v1.3 服务端一键部署脚本
# 运行环境：WSL2 + Ubuntu 24.04（亦兼容原生 Ubuntu 22.04+）
#
# 说明：
#   - 本脚本会安装系统依赖、开发工具链、AI 服务、数据库及前端/服务端构建依赖。
#   - 若网络受限，可先 source ./scripts/setup-proxy.sh 配置宿主机代理。
#   - whisper.cpp 使用 SSH 协议从 GitHub clone，需先配置好 GitHub SSH key。
#   - 默认使用小参数量模型 qwen2:0.5b，适合 8GB 内存主机。
#
# 用法：
#   cd /home/hfli/relationship-graph
#   ./scripts/deploy.sh
# =============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="relationship-graph"
APP_DATA_DIR="${HOME}/.local/share/${APP_NAME}"
BIN_DIR="${HOME}/.local/bin"
VENV_DIR="${HOME}/.venvs/${APP_NAME}"
MODELS_DIR="${APP_DATA_DIR}/models"
WHISPER_MODEL="${MODELS_DIR}/ggml-base.bin"
OLLAMA_MODEL="${OLLAMA_MODEL:-qwen2:0.5b}"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# 检测运行环境
if [[ "$(uname -s)" != "Linux" ]]; then
  log_error "本脚本仅支持 Linux 环境（WSL2/Ubuntu）"
  exit 1
fi

if ! grep -qiE "ubuntu|debian" /etc/os-release 2>/dev/null; then
  log_warn "未检测到 Ubuntu/Debian 系统，部分 apt 命令可能失效"
fi

mkdir -p "${APP_DATA_DIR}" "${BIN_DIR}" "${MODELS_DIR}"

# =============================================================================
# 1. 系统依赖
# =============================================================================
log_info "步骤 1/11: 安装系统依赖"
"${PROJECT_DIR}/scripts/install-system-deps.sh"

# =============================================================================
# 2. Node.js（推荐 20.x LTS）
# =============================================================================
log_info "步骤 2/11: 检查并安装 Node.js"
if ! command -v node &>/dev/null || [[ "$(node --version | cut -d'v' -f2 | cut -d'.' -f1)" -lt 20 ]]; then
  NODE_VERSION="20.15.1"
  NODE_TARBALL="node-v${NODE_VERSION}-linux-x64.tar.xz"
  NODE_URL="https://nodejs.org/dist/v${NODE_VERSION}/${NODE_TARBALL}"
  TMP_DIR="$(mktemp -d)"
  curl -fL --connect-timeout 15 "${NODE_URL}" | tar -xJf - -C "${TMP_DIR}"
  rm -rf "${HOME}/.local/node"
  mv "${TMP_DIR}/node-v${NODE_VERSION}-linux-x64" "${HOME}/.local/node"
  rm -rf "${TMP_DIR}"
  # 确保 PATH 包含 node
  if ! grep -q '.local/node/bin' "${HOME}/.bashrc"; then
    echo 'export PATH="${HOME}/.local/node/bin:${PATH}"' >> "${HOME}/.bashrc"
  fi
  export PATH="${HOME}/.local/node/bin:${PATH}"
  log_info "Node.js 已安装：$(node --version)"
else
  log_info "Node.js 已满足要求：$(node --version)"
fi

# =============================================================================
# 3. Rust
# =============================================================================
log_info "步骤 3/11: 检查并安装 Rust"
if ! command -v cargo &>/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  # shellcheck source=/dev/null
  source "${HOME}/.cargo/env"
  log_info "Rust 已安装：$(rustc --version)"
else
  # shellcheck source=/dev/null
  source "${HOME}/.cargo/env"
  log_info "Rust 已满足要求：$(rustc --version)"
fi

# =============================================================================
# 4. Python venv + AI/OCR 库
# =============================================================================
log_info "步骤 4/11: 配置 Python 虚拟环境"
if ! command -v python3 &>/dev/null; then
  sudo apt install -y python3 python3-venv python3-pip
fi

if [ ! -d "${VENV_DIR}" ]; then
  python3 -m venv "${VENV_DIR}"
fi

# shellcheck source=/dev/null
source "${VENV_DIR}/bin/activate"

pip install --upgrade pip
pip install faster-whisper pytesseract pillow

if ! grep -q "${VENV_DIR}/bin/activate" "${HOME}/.bashrc"; then
  echo "# ${APP_NAME} Python venv" >> "${HOME}/.bashrc"
  echo "source ${VENV_DIR}/bin/activate" >> "${HOME}/.bashrc"
fi

log_info "Python 虚拟环境已就绪：${VENV_DIR}"

# =============================================================================
# 5. AI 服务：Ollama + Whisper
# =============================================================================
log_info "步骤 5/11: 安装 AI 服务（Ollama、Whisper）"
log_info "若网络受限，请先运行: source ./scripts/setup-proxy.sh"
"${PROJECT_DIR}/scripts/install-ai-deps.sh"

log_info "步骤 5b: 拉取默认 LLM 模型 ${OLLAMA_MODEL}"
if ! ollama list 2>/dev/null | grep -q "^${OLLAMA_MODEL}"; then
  ollama pull "${OLLAMA_MODEL}"
fi
ollama list

# =============================================================================
# 6. Docker + PostgreSQL
# =============================================================================
log_info "步骤 6/11: 启动 PostgreSQL Docker 容器"
if ! command -v docker &>/dev/null; then
  log_error "Docker 未安装。请先运行 install-system-deps.sh 或手动安装 Docker。"
  log_error "安装后执行 'newgrp docker' 使当前用户加入 docker 组，再重新运行本脚本。"
  exit 1
fi
if ! docker ps &>/dev/null; then
  log_warn "Docker 已安装但当前不可用（可能权限不足或 daemon 未启动）。"
  log_warn "请执行 'newgrp docker' 后重新登录，或运行 'sudo systemctl start docker'。"
  log_warn "若处于受限网络，请先为 Docker daemon 配置 HTTP 代理："
  log_warn "  sudo mkdir -p /etc/systemd/system/docker.service.d"
  log_warn "  sudo tee /etc/systemd/system/docker.service.d/http-proxy.conf <<EOF"
  log_warn "  [Service]"
  log_warn "  Environment=\"HTTP_PROXY=http://<host-ip>:7891\""
  log_warn "  Environment=\"HTTPS_PROXY=http://<host-ip>:7891\""
  log_warn "  EOF"
  log_warn "  sudo systemctl daemon-reload && sudo systemctl restart docker"
  exit 1
fi
"${PROJECT_DIR}/scripts/setup-postgres-docker.sh"

# =============================================================================
# 7. 前端 npm 依赖
# =============================================================================
log_info "步骤 7/11: 安装前端 npm 依赖"
cd "${PROJECT_DIR}"

# 配置 npm 使用代理（如环境变量已设置）
if [ -n "${HTTP_PROXY:-}" ]; then
  npm config set proxy "${HTTP_PROXY}" || true
  npm config set https-proxy "${HTTPS_PROXY}" || true
fi

npm install

# 校验关键前端包是否已安装
for pkg in tesseract.js pinyin-pro xlsx regenerator-runtime; do
  if [ ! -f "node_modules/${pkg}/package.json" ]; then
    log_error "前端依赖 ${pkg} 未正确安装，请检查 npm 网络"
    exit 1
  fi
done
log_info "前端依赖安装完成"

# =============================================================================
# 8. 构建 Axum 服务端
# =============================================================================
log_info "步骤 8/11: 构建 Axum 服务端"
cd "${PROJECT_DIR}/server"
cargo build --release

if [ ! -f "${PROJECT_DIR}/server/target/release/relationship-graph-server" ]; then
  log_error "服务端构建失败"
  exit 1
fi
log_info "服务端构建完成"

# =============================================================================
# 9. 启动服务
# =============================================================================
log_info "步骤 9/11: 启动服务"
"${PROJECT_DIR}/scripts/start-services.sh"

# =============================================================================
# 10. 初始化 SQLite 加密数据库
# =============================================================================
log_info "步骤 10/11: 初始化 SQLite 加密数据库"

# 等待 Axum 服务就绪
for i in $(seq 1 60); do
  if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
if ! curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
  log_error "Axum 服务未就绪，无法初始化数据库"
  exit 1
fi

# 获取主密码
if [ -z "${RG_INIT_PASSWORD:-}" ]; then
  read -rsp "请输入主密码（至少8位）: " RG_INIT_PASSWORD
  echo ""
fi

if [ "${#RG_INIT_PASSWORD}" -lt 8 ]; then
  log_error "主密码至少需要 8 个字符"
  exit 1
fi

# 尝试 setup；若已初始化则改用 unlock
SETUP_RESPONSE=$(curl -s -w "\n%{http_code}" -X POST http://localhost:8790/api/auth/setup \
  -H "Content-Type: application/json" \
  -d "{\"password\":\"${RG_INIT_PASSWORD}\"}")
SETUP_HTTP_CODE=$(echo "$SETUP_RESPONSE" | tail -1)
SETUP_BODY=$(echo "$SETUP_RESPONSE" | sed '$d')

if [ "$SETUP_HTTP_CODE" = "200" ]; then
  log_info "数据库初始化成功"
elif [ "$SETUP_HTTP_CODE" = "400" ] || [ "$SETUP_HTTP_CODE" = "409" ]; then
  log_info "数据库已初始化，尝试解锁..."
  UNLOCK_RESPONSE=$(curl -s -w "\n%{http_code}" -X POST http://localhost:8790/api/auth/unlock \
    -H "Content-Type: application/json" \
    -d "{\"password\":\"${RG_INIT_PASSWORD}\"}")
  UNLOCK_HTTP_CODE=$(echo "$UNLOCK_RESPONSE" | tail -1)
  if [ "$UNLOCK_HTTP_CODE" = "200" ]; then
    log_info "数据库解锁成功"
  else
    log_error "数据库解锁失败（HTTP ${UNLOCK_HTTP_CODE}）"
    exit 1
  fi
else
  log_error "数据库初始化失败（HTTP ${SETUP_HTTP_CODE}）: ${SETUP_BODY}"
  exit 1
fi

# =============================================================================
# 11. 导入演示数据
# =============================================================================
log_info "步骤 11/11: 导入演示数据"
if RG_SEED_PASSWORD="$RG_INIT_PASSWORD" node "${PROJECT_DIR}/scripts/seed-demo-data.mjs"; then
  log_info "演示数据导入成功"
else
  log_warn "演示数据导入失败（非致命，可稍后手动运行: RG_SEED_PASSWORD=<密码> node scripts/seed-demo-data.mjs）"
fi

log_info "部署完成！"
echo ""
echo "============================================"
echo "前端访问地址："
echo "  http://$(hostname -I | awk '{print $1}'):1420"
echo ""
echo "Axum API 地址："
echo "  http://$(hostname -I | awk '{print $1}'):8790"
echo ""
echo "常用命令："
echo "  启动前端：npm run dev"
echo "  启动服务端：cd server && cargo run --release"
echo "  查看服务：${PROJECT_DIR}/scripts/start-services.sh"
echo "============================================"

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

# PostgreSQL
if command -v docker &>/dev/null && docker ps --format '{{.Names}}' 2>/dev/null | grep -q "^relationship-graph-pg$"; then
  echo "  PostgreSQL - localhost:5432          - [运行中]"
else
  echo "  PostgreSQL - localhost:5432          - [未运行]"
fi

# Axum
if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
  echo "  Axum服务端  - http://localhost:8790   - [已就绪]"
else
  echo "  Axum服务端  - http://localhost:8790   - [启动中]"
fi

echo "============================================================"
