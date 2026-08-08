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
#   - 停止逻辑按命令行精确匹配本项目进程（pgrep -f）并以 ss 校验端口释放，
#     非 root 下 lsof 无法列出 :80 等端口持有者，故不依赖 lsof
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

# 按命令行精确匹配停止服务：stop_by_pattern <进程命令行匹配模式> <服务名> <监听端口>
# 说明：非 root 下 lsof/ss 均可能无法列出 :80 等端口的持有 PID（实测 ss -tlnp
# 对 caddy 的 :80 监听也不显示 pid），因此以 pgrep -f 精确匹配本项目进程的
# 命令行作为主手段；模式均包含项目专有标识（Caddyfile 文件名 / 二进制全路径 /
# 项目目录路径 / http.server --directory 参数），不会误杀其它进程。
# 停止后再用 ss 校验端口是否已释放。
# 注意：模式均以 ^ 锚定命令行开头，避免 pgrep -f 误匹配包含模式文本的其它命令行。
stop_by_pattern() {
  local pattern="$1"
  local name="$2"
  local port="$3"
  local pids
  pids="$(pgrep -f "${pattern}" 2>/dev/null || true)"
  if [ -z "${pids}" ]; then
    echo "  ${name}（端口 ${port}）：未在运行"
    return
  fi
  echo "  停止 ${name}（端口 ${port}，PID: ${pids//$'\n'/ }）"
  # shellcheck disable=SC2086
  kill ${pids} 2>/dev/null || true
  # 最多等待 10 秒确认退出（进程全部消失且端口不再有可见持有者）
  for _ in {1..10}; do
    if ! pgrep -f "${pattern}" >/dev/null 2>&1 \
      && ! ss -tlnp 2>/dev/null | grep -Eq "[:.]${port}[[:space:]]"; then
      echo "  ${name} 已停止"
      return
    fi
    sleep 1
  done
  echo "  [WARN] ${name} 未在 10 秒内退出，执行强制终止"
  pids="$(pgrep -f "${pattern}" 2>/dev/null || true)"
  if [ -n "${pids}" ]; then
    # shellcheck disable=SC2086
    kill -9 ${pids} 2>/dev/null || true
  fi
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
  # 精确匹配本项目 release 二进制全路径（^ 锚定），不会误杀其它进程
  stop_by_pattern "^${PROJECT_DIR}/server/target/release/relationship-graph-server" "Axum 服务端" 8790
fi

# 前端：开发模式 Vite（1420）/ 生产模式 Caddy（RG_WEB_PORT，默认 80）或 http.server（8000）
# Vite 模式限定本项目 node_modules 路径（npm run dev 实际命令行为
# node ${PROJECT_DIR}/node_modules/.bin/vite），只匹配本项目的 vite dev server；
# 生产环境前端由 Caddy 服务 dist/ 产物，vite 仅开发模式使用，未运行时不命中
stop_by_pattern "^node ${PROJECT_DIR}/node_modules/.*vite" "Vite 开发服务器" 1420
stop_by_pattern "^caddy run --config /tmp/relationship-graph.Caddyfile" "Caddy 反向代理" "${RG_WEB_PORT:-80}"
# http.server 启动时带 --directory ${PROJECT_DIR}/dist（见 start-services.sh），
# 以该参数作为项目专有标识；该方式仅为未安装 Caddy 时的兜底，现役默认走 Caddy
stop_by_pattern "^python3 -m http\\.server 8000 --directory ${PROJECT_DIR}/dist" "Python http.server" 8000

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
