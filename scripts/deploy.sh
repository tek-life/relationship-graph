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
#   - 默认使用 qwen2.5:7b（可通过环境变量 OLLAMA_MODEL 覆盖）。
#
# 用法：
#   cd /home/hfli/personal_ai_workspace
#   ./scripts/deploy.sh
#
# 部署流程：
#   1.  系统依赖安装
#   2.  Node.js 安装
#   3.  Rust 工具链
#   4.  Python 虚拟环境
#   5.  AI 依赖安装（Ollama + Whisper）
#   6.  前端 npm 依赖安装
#   7.  前端生产构建（npm run build → dist/）
#   8.  后端编译（cargo build --release）
#   9.  启动服务（后端 + 前端静态/Caddy 反向代理）
#   10. 健康检查（Axum + 前端）
#   11. 初始化 SQLite 加密数据库
#   12. 导入演示数据
# =============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="relationship-graph"
APP_DATA_DIR="${HOME}/.local/share/${APP_NAME}"
BIN_DIR="${HOME}/.local/bin"
VENV_DIR="${HOME}/.venvs/${APP_NAME}"
MODELS_DIR="${APP_DATA_DIR}/models"
WHISPER_MODEL="${MODELS_DIR}/ggml-base.bin"
OLLAMA_MODEL="${OLLAMA_MODEL:-qwen2.5:7b}"

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
log_info "步骤 1/12: 安装系统依赖"
"${PROJECT_DIR}/scripts/install-system-deps.sh"

# =============================================================================
# 2. Node.js（推荐 20.x LTS）
# =============================================================================
log_info "步骤 2/12: 检查并安装 Node.js"
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
log_info "步骤 3/12: 检查并安装 Rust"
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
log_info "步骤 4/12: 配置 Python 虚拟环境"
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
log_info "步骤 5/12: 安装 AI 服务（Ollama、Whisper）"
log_info "若网络受限，请先运行: source ./scripts/setup-proxy.sh"
"${PROJECT_DIR}/scripts/install-ai-deps.sh"

log_info "步骤 5b: 拉取默认 LLM 模型 ${OLLAMA_MODEL}"
if ! ollama list 2>/dev/null | grep -q "^${OLLAMA_MODEL}"; then
  ollama pull "${OLLAMA_MODEL}"
fi
ollama list

# =============================================================================
# 6. 前端 npm 依赖
# =============================================================================
log_info "步骤 6/12: 安装前端 npm 依赖"
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
# 7. 前端生产构建（npm run build → dist/）
# =============================================================================
log_info "步骤 7/12: 前端生产构建"
cd "${PROJECT_DIR}"
npm run build

# 校验构建产物
if [ ! -d "${PROJECT_DIR}/dist" ] || [ ! -f "${PROJECT_DIR}/dist/index.html" ]; then
  log_error "前端构建失败：dist/ 目录或 index.html 不存在"
  exit 1
fi
log_info "前端生产构建完成，输出目录：${PROJECT_DIR}/dist"

# 校验 PWA 资源是否已复制到 dist/（manifest.json 和 sw.js 在 public/ 中，Vite 构建会自动复制）
for pwa_file in manifest.json sw.js; do
  if [ ! -f "${PROJECT_DIR}/dist/${pwa_file}" ]; then
    log_warn "PWA 资源 ${pwa_file} 未出现在 dist/ 中，PWA 功能可能受限"
  fi
done

# =============================================================================
# 8. 构建 Axum 服务端
# =============================================================================
log_info "步骤 8/12: 构建 Axum 服务端"
cd "${PROJECT_DIR}/server"
cargo build --release

if [ ! -f "${PROJECT_DIR}/server/target/release/relationship-graph-server" ]; then
  log_error "服务端构建失败"
  exit 1
fi
log_info "服务端构建完成"

# =============================================================================
# 9. 启动服务（后端 + 前端静态/Caddy 反向代理）
# =============================================================================
log_info "步骤 9/12: 启动服务"
"${PROJECT_DIR}/scripts/start-services.sh"

# =============================================================================
# 10. 健康检查
# =============================================================================
log_info "步骤 10/12: 健康检查"

# 等待 Axum 服务就绪
HEALTH_OK=false
for i in $(seq 1 60); do
  if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
    HEALTH_OK=true
    break
  fi
  sleep 1
done

if [ "${HEALTH_OK}" = "true" ]; then
  log_info "Axum 后端健康检查通过（http://localhost:8790/api/health）"
else
  log_error "Axum 后端健康检查失败"
  exit 1
fi

# 检查前端服务（生产模式检查 8080 端口 / Caddy，开发模式检查 1420 端口 / Vite）
if [ -d "${PROJECT_DIR}/dist" ]; then
  # 生产模式：前端由 Caddy（8080）或 Python http.server（8000）提供服务
  if curl -s http://localhost:8080 >/dev/null 2>&1; then
    log_info "前端（Caddy）健康检查通过（http://localhost:8080）"
  elif curl -s http://localhost:8000 >/dev/null 2>&1; then
    log_info "前端（Python http.server）健康检查通过（http://localhost:8000）"
  else
    log_warn "前端静态服务未检测到，请手动检查 Caddy 或 Python http.server 是否已启动"
  fi
else
  # 开发模式：Vite 开发服务器
  if curl -s http://localhost:1420 >/dev/null 2>&1; then
    log_info "前端（Vite）健康检查通过（http://localhost:1420）"
  else
    log_warn "前端开发服务器未就绪，请查看日志：/tmp/relationship-graph-frontend.log"
  fi
fi

# =============================================================================
# 11. 初始化 SQLite 加密数据库
# =============================================================================
log_info "步骤 11/12: 初始化 SQLite 加密数据库"

# 获取管理员密码（setup 时主密码即 admin 账号密码；迁移后统一账号登录）
if [ -z "${RG_INIT_PASSWORD:-}" ]; then
  read -rsp "请输入管理员密码（至少8位）: " RG_INIT_PASSWORD
  echo ""
fi

if [ "${#RG_INIT_PASSWORD}" -lt 8 ]; then
  log_error "密码至少需要 8 个字符"
  exit 1
fi

# 尝试 setup（全新部署会生成密钥文件并创建 admin）；若已初始化则改用 admin 登录
SETUP_RESPONSE=$(curl -s -w "\n%{http_code}" -X POST http://localhost:8790/api/auth/setup \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"admin\",\"password\":\"${RG_INIT_PASSWORD}\"}")
SETUP_HTTP_CODE=$(echo "$SETUP_RESPONSE" | tail -1)
SETUP_BODY=$(echo "$SETUP_RESPONSE" | sed '$d')

if [ "$SETUP_HTTP_CODE" = "200" ]; then
  log_info "数据库初始化成功（已创建 admin 账号）"
elif [ "$SETUP_HTTP_CODE" = "400" ] || [ "$SETUP_HTTP_CODE" = "409" ]; then
  log_info "数据库已初始化，尝试 admin 登录..."
  LOGIN_RESPONSE=$(curl -s -w "\n%{http_code}" -X POST http://localhost:8790/api/auth/login \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"admin\",\"password\":\"${RG_INIT_PASSWORD}\"}")
  LOGIN_HTTP_CODE=$(echo "$LOGIN_RESPONSE" | tail -1)
  if [ "$LOGIN_HTTP_CODE" = "200" ]; then
    log_info "admin 登录成功"
  else
    log_error "admin 登录失败（HTTP ${LOGIN_HTTP_CODE}）"
    exit 1
  fi
else
  log_error "数据库初始化失败（HTTP ${SETUP_HTTP_CODE}）: ${SETUP_BODY}"
  exit 1
fi

# =============================================================================
# 12. 导入演示数据
# =============================================================================
log_info "步骤 12/12: 导入演示数据"
if RG_SEED_PASSWORD="$RG_INIT_PASSWORD" node "${PROJECT_DIR}/scripts/seed-demo-data.mjs"; then
  log_info "演示数据导入成功"
else
  log_warn "演示数据导入失败（非致命，可稍后手动运行: RG_SEED_PASSWORD=<密码> node scripts/seed-demo-data.mjs）"
fi

log_info "部署完成！"
echo ""
echo "============================================"
echo "访问地址："
if [ -d "${PROJECT_DIR}/dist" ]; then
  echo "  生产模式（Caddy/反向代理）：http://$(hostname -I | awk '{print $1}'):8080"
  echo "  前端构建产物目录：${PROJECT_DIR}/dist"
else
  echo "  开发模式（Vite）：http://$(hostname -I | awk '{print $1}'):1420"
fi
echo ""
echo "Axum API 地址："
echo "  http://$(hostname -I | awk '{print $1}'):8790"
echo "  健康检查：http://localhost:8790/api/health"
echo ""
echo "常用命令："
echo "  生产部署：./scripts/deploy.sh"
echo "  启动服务：./scripts/start-services.sh"
echo "  开发环境：./scripts/dev.sh"
echo "  Caddy 代理：caddy run --config ${PROJECT_DIR}/scripts/Caddyfile"
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

# Axum
if curl -s http://localhost:8790/api/health >/dev/null 2>&1; then
  echo "  Axum服务端  - http://localhost:8790   - [已就绪]"
else
  echo "  Axum服务端  - http://localhost:8790   - [启动中]"
fi

# 前端（生产模式检查 8080/8000，开发模式检查 1420）
if [ -d "${PROJECT_DIR}/dist" ]; then
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
  fi
fi

echo "============================================================"
