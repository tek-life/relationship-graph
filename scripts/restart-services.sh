#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 重启个人智能 AI 平台各常驻服务
# 运行环境：WSL2 + Ubuntu 24.04
#
# 用法：
#   ./scripts/restart-services.sh            # 重启 Axum 后端 + 前端（保留 Ollama）
#   ./scripts/restart-services.sh --all      # 连同 Ollama 一起重启
#   ./scripts/restart-services.sh --build    # 先重新编译后端（cargo build --release）再重启
#
# 说明：
#   - 停止逻辑按端口精确匹配，不会误杀其它进程
#   - Ollama 默认不重启（模型常驻内存，重启会触发重新加载）
#   - 停止后委托 start-services.sh 完成拉起与健康检查
#   - 注意：后端重启会使数据库回到锁定状态，需在页面上重新输入主密码解锁
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

# =============================================================================
# 1. 停止现有服务
# =============================================================================
echo "==> 停止现有服务"

# Axum 后端（8790）
stop_by_port 8790 "Axum 服务端"

# 前端：开发模式 Vite（1420）/ 生产模式 Caddy（8080）或 http.server（8000）
stop_by_port 1420 "Vite 开发服务器"
stop_by_port 8080 "Caddy 反向代理"
stop_by_port 8000 "Python http.server"

# Ollama（默认保留）
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
