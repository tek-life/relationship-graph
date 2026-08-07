#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 重启个人智能 AI 平台各常驻服务
# 运行环境：Ubuntu 24.04（WSL2 / 阿里云 ECS 均适用）
#
# 用法：
#   ./scripts/restart-services.sh            # 重启 Axum 后端 + 前端（保留 Ollama）
#   ./scripts/restart-services.sh --all      # 连同 Ollama 一起重启（仅本地开发通道需要）
#   ./scripts/restart-services.sh --build    # 先重新编译后端（cargo build --release）再重启
#
# 说明：
#   - 生产默认 LLM 通道为阿里百炼云端（RG_LLM_BACKEND=cloud），无需 Ollama；
#     回退本地模型可设 RG_LLM_BACKEND=legacy（配合 RG_USE_OLLAMA=1）
#   - 停止逻辑按端口动态查持有 PID，不会误杀其它进程
#   - Ollama 默认不重启（本地模型常驻内存，重启会触发重新加载）
#   - 停止后委托 start-services.sh 完成拉起与健康检查
#   - 数据库使用 db.key 密钥文件自动解锁，重启后无需人工解锁
# =============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WITH_OLLAMA=0
DO_BUILD=0

for arg in "$@"; do
  case "${arg}" in
    --all) WITH_OLLAMA=1 ;;
    --build) DO_BUILD=1 ;;
    *)
      echo "未知参数：${arg}"
      echo "用法：$0 [--all] [--build]"
      exit 1
      ;;
  esac
done

# 按端口停止进程：stop_by_port <端口> <服务名>
stop_by_port() {
  local port="$1"
  local name="$2"
  local pids
  pids="$(lsof -t -i ":${port}" -sTCP:LISTEN 2>/dev/null || true)"
  if [ -z "${pids}" ]; then
    echo "  ${name}（端口 ${port}）：未在运行"
    return
  fi
  echo "  停止 ${name}（端口 ${port}，PID: ${pids//$'\n'/ }）"
  # shellcheck disable=SC2086
  kill ${pids} 2>/dev/null || true
  # 最多等待 10 秒确认退出
  for _ in {1..10}; do
    if ! lsof -i ":${port}" -sTCP:LISTEN >/dev/null 2>&1; then
      echo "  ${name} 已停止"
      return
    fi
    sleep 1
  done
  echo "  [WARN] ${name} 未在 10 秒内退出，执行强制终止"
  # shellcheck disable=SC2086
  kill -9 ${pids} 2>/dev/null || true
}

echo ""
echo "=========================================="
echo "  个人智能 AI 平台 - 服务重启"
echo "=========================================="
echo ""

# systemd 用户级服务检测：本机（WSL）与 ECS 的 8790 后端已由 systemd 托管，
# kill 会被 Restart=always 立即拉起、与脚本竞争，因此后端段改走
# systemctl stop/start（前端段仍按端口停起，逻辑不变）
if systemctl --user is-enabled relationship-graph >/dev/null 2>&1; then
  echo "[提示] 后端由 systemd 用户级服务托管：停止/拉起改走 systemctl，"
  echo "       日常仅重启后端也可直接：systemctl --user restart relationship-graph"
  echo ""
fi

# =============================================================================
# 1. 停止现有服务
# =============================================================================
echo "==> 停止现有服务"

# Axum 后端（8790）：systemd 托管时走 systemctl stop（kill 会被立即拉起，产生竞争）；
# 停止后由 start-services.sh 委托 systemctl start 重新拉起
if systemctl --user is-enabled relationship-graph >/dev/null 2>&1; then
  echo "  停止 Axum 服务端（systemd 托管：systemctl --user stop）"
  systemctl --user stop relationship-graph || true
  echo "  Axum 服务端已停止（将由 start-services.sh 通过 systemctl 拉起）"
else
  stop_by_port 8790 "Axum 服务端"
fi

# 前端：开发模式 Vite（1420）/ 生产模式 Caddy（8080）或 http.server（8000）
stop_by_port 1420 "Vite 开发服务器"
stop_by_port 8080 "Caddy 反向代理"
stop_by_port 8000 "Python http.server"

# Ollama（默认保留；云端模式下 Ollama 本就不参与 LLM 链路）
if [ "${WITH_OLLAMA}" -eq 1 ]; then
  if pgrep -x "ollama" >/dev/null; then
    echo "  停止 Ollama 服务"
    pkill -x ollama || true
    for _ in {1..10}; do
      pgrep -x "ollama" >/dev/null || break
      sleep 1
    done
    echo "  Ollama 已停止"
  else
    echo "  Ollama：未在运行"
  fi
else
  echo "  Ollama：保留运行（如需一起重启请加 --all）"
fi

# =============================================================================
# 2. 可选：重新编译后端
# 必须在停止 Axum 服务端之后编译：cargo 会覆盖删除正在运行的二进制文件，
# 若先编译后停止，旧进程会继续运行已被删除的旧二进制（历史故障根因）
# =============================================================================
if [ "${DO_BUILD}" -eq 1 ]; then
  echo ""
  echo "==> 重新编译后端（cargo build --release）"
  (
    cd "${PROJECT_DIR}/server"
    source "${HOME}/.cargo/env" 2>/dev/null || true
    cargo build --release
  )
  echo "  编译完成"
fi

# =============================================================================
# 3. 重新拉起所有服务（复用 start-services.sh 的启动与健康检查逻辑）
# =============================================================================
echo ""
echo "==> 重新拉起服务"
exec "${PROJECT_DIR}/scripts/start-services.sh"
