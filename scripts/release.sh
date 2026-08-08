#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 个人关系图谱 - 本地构建 + 上传发版脚本（在开发机/WSL2 上运行）
#
# 流程：
#   1. 本地 cargo build --release（后端二进制）
#   2. 本地 npm run build（前端 dist/）
#   3. rsync 增量上传到 ECS（~/relationship-graph/{bin,web}）
#   4. 远端重启 systemd 用户级服务（无需 root）
#   5. 健康检查（8790 后端 + 80 前端）
#
# 前提：
#   - 服务器已运行过一次 scripts/server-init.sh
#   - 本地与服务器均为 x86_64 Linux（WSL2 Ubuntu 24.04 → ECS Ubuntu 24.04 可直接复用产物）
#   - 已配置好到服务器的 SSH（推荐密钥免密登录）
#
# 用法：
#   RG_HOST=<user>@<ip> ./scripts/release.sh
#
# 可选环境变量：
#   RG_HOST        必填，SSH 目标，如 hfli@47.98.xx.xx 或 root@47.98.xx.xx
#   RG_SSH_PORT    SSH 端口，默认 22
#   RG_REMOTE_DIR  远端应用目录（相对 home），默认 relationship-graph
#   SKIP_BACKEND=1 跳过后端构建与上传（仅发前端）
#   SKIP_FRONTEND=1 跳过前端构建与上传（仅发后端）
# =============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REMOTE_DIR="${RG_REMOTE_DIR:-relationship-graph}"
SSH_PORT="${RG_SSH_PORT:-22}"
SSH_OPTS=(-o BatchMode=yes -o ConnectTimeout=10 -p "${SSH_PORT}")

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# ---- 0. 参数与环境检查 -------------------------------------------------------
if [ -z "${RG_HOST:-}" ]; then
  log_error "请设置 RG_HOST，例如：RG_HOST=hfli@47.98.1.2 ./scripts/release.sh"
  exit 1
fi

SSH="ssh ${SSH_OPTS[*]} ${RG_HOST}"
RSYNC_SSH="ssh ${SSH_OPTS[*]}"

log_info "目标服务器：${RG_HOST}（端口 ${SSH_PORT}，远端目录 ~/${REMOTE_DIR}）"

if ! ${SSH} 'echo ok' >/dev/null 2>&1; then
  log_error "SSH 连接失败（已启用 BatchMode，不接受交互输入密码）。"
  log_error "请先配置免密登录：ssh-copy-id -p ${SSH_PORT} ${RG_HOST}"
  exit 1
fi
log_info "SSH 连接正常"

REMOTE_OS=$(${SSH} '. /etc/os-release && echo "${ID} ${VERSION_ID}"' || echo "unknown")
log_info "远端系统：${REMOTE_OS}"
case "${REMOTE_OS}" in
  ubuntu*|debian*) ;;
  *) log_warn "远端不是 Ubuntu/Debian，二进制可能不兼容，继续执行" ;;
esac

if [ "${SKIP_BACKEND:-0}" != "1" ]; then
  command -v cargo >/dev/null || { log_error "本地未安装 cargo"; exit 1; }
fi
if [ "${SKIP_FRONTEND:-0}" != "1" ]; then
  command -v npm >/dev/null || { log_error "本地未安装 npm"; exit 1; }
fi

# ---- 1. 构建后端 -------------------------------------------------------------
SERVER_BIN="${PROJECT_DIR}/server/target/release/relationship-graph-server"
if [ "${SKIP_BACKEND:-0}" != "1" ]; then
  log_info "步骤 1/5: 编译后端（cargo build --release）"
  (cd "${PROJECT_DIR}/server" && cargo build --release)
  [ -f "${SERVER_BIN}" ] || { log_error "后端二进制不存在：${SERVER_BIN}"; exit 1; }
else
  log_info "步骤 1/5: 跳过后端构建（SKIP_BACKEND=1）"
fi

# ---- 2. 构建前端 -------------------------------------------------------------
if [ "${SKIP_FRONTEND:-0}" != "1" ]; then
  log_info "步骤 2/5: 构建前端（npm run build）"
  (cd "${PROJECT_DIR}" && npm run build)
  [ -f "${PROJECT_DIR}/dist/index.html" ] || { log_error "前端构建失败：dist/index.html 不存在"; exit 1; }
else
  log_info "步骤 2/5: 跳过前端构建（SKIP_FRONTEND=1）"
fi

# ---- 3. 上传产物 -------------------------------------------------------------
log_info "步骤 3/5: rsync 上传产物"
${SSH} "mkdir -p ${REMOTE_DIR}/bin ${REMOTE_DIR}/web"

if [ "${SKIP_BACKEND:-0}" != "1" ]; then
  # 先传到临时文件再原子替换，避免正在运行的进程读到半个二进制
  rsync -az -e "${RSYNC_SSH}" "${SERVER_BIN}" \
    "${RG_HOST}:${REMOTE_DIR}/bin/relationship-graph-server.new"
  ${SSH} "mv ${REMOTE_DIR}/bin/relationship-graph-server.new ${REMOTE_DIR}/bin/relationship-graph-server && chmod +x ${REMOTE_DIR}/bin/relationship-graph-server"
  log_info "  后端二进制已上传"
fi

if [ "${SKIP_FRONTEND:-0}" != "1" ]; then
  rsync -az --delete -e "${RSYNC_SSH}" "${PROJECT_DIR}/dist/" "${RG_HOST}:${REMOTE_DIR}/web/"
  log_info "  前端 dist/ 已上传"
fi

# ---- 4. 重启远端服务 ---------------------------------------------------------
log_info "步骤 4/5: 重启远端服务（systemd 用户级）"
if [ "${SKIP_BACKEND:-0}" != "1" ]; then
  if ${SSH} "systemctl --user is-active --quiet relationship-graph"; then
    ${SSH} "systemctl --user restart relationship-graph"
    log_info "  relationship-graph 已重启"
  else
    ${SSH} "systemctl --user daemon-reload && systemctl --user start relationship-graph"
    log_info "  relationship-graph 已启动（首次）"
  fi
fi
if ! ${SSH} "systemctl --user is-active --quiet rg-caddy"; then
  ${SSH} "systemctl --user daemon-reload && systemctl --user start rg-caddy"
  log_info "  rg-caddy 已启动（首次）"
else
  log_info "  rg-caddy 运行中（静态文件已由 rsync 更新，无需重启）"
fi

# ---- 5. 健康检查 -------------------------------------------------------------
log_info "步骤 5/5: 健康检查"
HEALTH_OK=false
for i in $(seq 1 30); do
  if ${SSH} "curl -sf http://localhost:8790/api/health" >/dev/null 2>&1; then
    HEALTH_OK=true
    break
  fi
  sleep 1
done

if [ "${HEALTH_OK}" = "true" ]; then
  log_info "后端健康检查通过（:8790/api/health）"
else
  log_error "后端健康检查失败，最近日志："
  ${SSH} "journalctl --user -u relationship-graph -n 30 --no-pager" || true
  exit 1
fi

# 前端监听端口：从远端 Caddyfile 监听行读取（默认 80）
WEB_PORT=$(${SSH} "grep -oE '^:[0-9]+' ${REMOTE_DIR}/Caddyfile 2>/dev/null | head -1 | tr -dc '0-9'")
if [ -z "${WEB_PORT}" ]; then
  WEB_PORT=80
fi

if ${SSH} "curl -sf http://localhost:${WEB_PORT}" >/dev/null 2>&1; then
  log_info "前端健康检查通过（:${WEB_PORT}）"
else
  log_warn "前端健康检查未通过，请检查：ssh ${RG_HOST} 'journalctl --user -u rg-caddy -n 30 --no-pager'"
fi

SERVER_IP="${RG_HOST#*@}"
# 默认端口 80 时 URL 不带端口后缀
WEB_SUFFIX=":${WEB_PORT}"
if [ "${WEB_PORT}" = "80" ]; then
  WEB_SUFFIX=""
fi
echo ""
echo "============================================"
echo "发版完成！"
echo "  访问地址：http://${SERVER_IP}${WEB_SUFFIX}"
echo "  API 地址：http://${SERVER_IP}${WEB_SUFFIX}/api（反代至 8790）"
echo ""
echo "首次部署？执行一次数据库初始化（创建 admin 账号）："
echo "  curl -X POST http://${SERVER_IP}${WEB_SUFFIX}/api/auth/setup \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"username\":\"admin\",\"password\":\"<至少8位密码>\"}'"
echo "============================================"
