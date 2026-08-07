#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 启动个人智能 AI 平台服务端所需的常驻服务
# 运行环境：Ubuntu 24.04（WSL2 / 阿里云 ECS 均适用）
#
# 说明：
#   - 自动检测运行模式：存在 dist/ 目录则为生产模式，否则为开发模式
#   - LLM 通道：**默认使用阿里百炼云端模型**（RG_LLM_BACKEND=cloud），无需本地 Ollama
#   - Ollama（端口 11434）仅本地开发 legacy/rig 通道需要，生产默认跳过；
#     如需启动请显式设置 RG_USE_OLLAMA=1
#   - Axum 服务端：HTTP API（端口 8790，优先使用 release 二进制）
#   - 前端（生产模式）：Caddy 反向代理（端口 8080）或 Python http.server（端口 8000）兜底
#   - 前端（开发模式）：Vite 开发服务器（端口 1420）
#
# 云端模型（阿里百炼）可选覆盖项（均有服务端默认值，无需必填）：
#   RG_LLM_BACKEND            通道开关 legacy|rig|cloud（本脚本默认 cloud）
#   RG_CLOUD_BASE_URL         兼容端点（默认 dashscope compatible-mode/v1）
#   RG_CLOUD_CHAT_MODEL       聊天模型（默认 qwen3.7-plus，开思考）
#   RG_CLOUD_EXTRACT_MODEL    抽取模型（默认 qwen-flash，关思考）
#   RG_CLOUD_TIMEOUT_SECS     云端调用超时（默认 120）
#   RG_CLOUD_API_KEY          云端 API Key（优先 env；缺省由服务端自动读取
#                             ~/.config/rg-cloud-api-key，脚本不传 Key、严禁打印）
#   RG_SKILL_BUDGET_CHARS     技能注入预算（本脚本默认 8000）
# =============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DATA_DIR="${HOME}/.local/share/relationship-graph"
SERVER_LOG="/tmp/relationship-graph-server.log"
FRONTEND_LOG="/tmp/relationship-graph-frontend.log"
CADDY_LOG="/tmp/relationship-graph-caddy.log"
# legacy/rig 通道使用的 Ollama 超时（云端模式无效）
RG_OLLAMA_TIMEOUT_SECS="${RG_OLLAMA_TIMEOUT_SECS:-45}"
RG_OLLAMA_CHAT_TIMEOUT_SECS="${RG_OLLAMA_CHAT_TIMEOUT_SECS:-120}"
# LLM 通道：生产默认云端（百炼）；回退本地 Ollama 可设 RG_LLM_BACKEND=legacy
RG_LLM_BACKEND="${RG_LLM_BACKEND:-cloud}"
RG_SKILL_BUDGET_CHARS="${RG_SKILL_BUDGET_CHARS:-8000}"
# Ollama 开关：云端模式默认跳过；RG_USE_OLLAMA=1 时启动（仅本地开发需要）
RG_USE_OLLAMA="${RG_USE_OLLAMA:-0}"

# 运行模式检测
if [ -d "${PROJECT_DIR}/dist" ]; then
  RUN_MODE="production"
else
  RUN_MODE="development"
fi

mkdir -p "${APP_DATA_DIR}"

# systemd 用户级服务检测提示：本机（WSL）与 ECS 均已用 systemd user unit
# 托管 8790 后端，日常运维建议改用 systemctl（仅提示，不改变脚本逻辑）
if systemctl --user is-enabled relationship-graph >/dev/null 2>&1; then
  echo "[提示] 检测到 systemd 用户级服务 relationship-graph 已安装，后端建议用 systemctl 管理："
  echo "       systemctl --user status/restart relationship-graph"
  echo "       journalctl --user -u relationship-graph -f"
  echo "       unit 模板见 scripts/systemd/relationship-graph.service.example"
fi

echo ""
echo "=========================================="
echo "  个人关系图谱 - 服务启动（${RUN_MODE} 模式）"
echo "=========================================="
echo ""

# =============================================================================
# 1. Ollama 服务（可选）
#    生产默认使用阿里百炼云端模型，无需 Ollama；仅当 RG_USE_OLLAMA=1
#    且走 legacy/rig 本地通道时才启动。
# =============================================================================
echo "==> Ollama 服务（可选，端口 11434）"
if [ "${RG_USE_OLLAMA}" = "1" ]; then
  if ! command -v ollama >/dev/null 2>&1; then
    echo "  [WARN] 已设置 RG_USE_OLLAMA=1 但未安装 ollama，跳过启动"
  elif ! pgrep -x "ollama" >/dev/null; then
    nohup ollama serve >/tmp/ollama.log 2>&1 &
    echo "  Ollama 服务已启动，日志：/tmp/ollama.log"
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
else
  echo "  跳过 Ollama：生产默认使用阿里百炼云端模型（RG_LLM_BACKEND=${RG_LLM_BACKEND}），无需 Ollama"
  echo "  如需本地模型（legacy/rig 通道），请以 RG_USE_OLLAMA=1 RG_LLM_BACKEND=legacy 运行"
fi

# 云端模式：API Key 可用性轻量检查（仅提示，不阻断；严禁打印 Key 内容）
if [ "${RG_LLM_BACKEND}" = "cloud" ] || [ "${RG_LLM_BACKEND}" = "rig" ]; then
  if [ -n "${RG_CLOUD_API_KEY:-}" ]; then
    echo "  云端 API Key：已设置 RG_CLOUD_API_KEY 环境变量（内容不回显）"
  elif [ -f "${HOME}/.config/rg-cloud-api-key" ]; then
    echo "  云端 API Key：将使用 ${HOME}/.config/rg-cloud-api-key（服务端自动读取）"
  else
    echo "  [WARN] 未检测到云端 API Key（env RG_CLOUD_API_KEY 或文件 ~/.config/rg-cloud-api-key）"
    echo "         云端 LLM 调用将失败。配置方法见 scripts/README.md「云端模型（阿里百炼）配置」"
  fi
fi

# =============================================================================
# 2. 启动 Axum 服务端
# =============================================================================
echo ""
echo "==> 启动 Axum 服务端（后台，端口 8790）"
if ! lsof -i :8790 >/dev/null 2>&1; then
  cd "${PROJECT_DIR}/server"
  source "${HOME}/.cargo/env" 2>/dev/null || true

  # 云端通道可选覆盖项：如环境已设置则透传给服务端（均有服务端默认值）。
  # RG_CLOUD_API_KEY 通常不必在此设置——服务端缺省自动读取
  # ~/.config/rg-cloud-api-key。任何情况下严禁在脚本或日志中打印 Key。
  CLOUD_ENV=()
  [ -n "${RG_CLOUD_BASE_URL:-}" ] && CLOUD_ENV+=(RG_CLOUD_BASE_URL="${RG_CLOUD_BASE_URL}")
  [ -n "${RG_CLOUD_CHAT_MODEL:-}" ] && CLOUD_ENV+=(RG_CLOUD_CHAT_MODEL="${RG_CLOUD_CHAT_MODEL}")
  [ -n "${RG_CLOUD_EXTRACT_MODEL:-}" ] && CLOUD_ENV+=(RG_CLOUD_EXTRACT_MODEL="${RG_CLOUD_EXTRACT_MODEL}")
  [ -n "${RG_CLOUD_TIMEOUT_SECS:-}" ] && CLOUD_ENV+=(RG_CLOUD_TIMEOUT_SECS="${RG_CLOUD_TIMEOUT_SECS}")
  [ -n "${RG_CLOUD_API_KEY:-}" ] && CLOUD_ENV+=(RG_CLOUD_API_KEY="${RG_CLOUD_API_KEY}")
  # legacy 通道函数级灰度（仅 RG_LLM_BACKEND=legacy 时有效）
  [ -n "${RG_LLM_CLOUD_FNS:-}" ] && CLOUD_ENV+=(RG_LLM_CLOUD_FNS="${RG_LLM_CLOUD_FNS}")

  echo "  LLM 通道：RG_LLM_BACKEND=${RG_LLM_BACKEND}（技能预算 RG_SKILL_BUDGET_CHARS=${RG_SKILL_BUDGET_CHARS}）"

  # 优先使用编译后的二进制，否则用 cargo run --release
  SERVER_BIN="${PROJECT_DIR}/server/target/release/relationship-graph-server"
  if [ -f "${SERVER_BIN}" ]; then
    nohup env \
      RG_LLM_BACKEND="${RG_LLM_BACKEND}" \
      RG_SKILL_BUDGET_CHARS="${RG_SKILL_BUDGET_CHARS}" \
      RG_OLLAMA_TIMEOUT_SECS="${RG_OLLAMA_TIMEOUT_SECS}" \
      RG_OLLAMA_CHAT_TIMEOUT_SECS="${RG_OLLAMA_CHAT_TIMEOUT_SECS}" \
      "${CLOUD_ENV[@]}" \
      "${SERVER_BIN}" >>"${SERVER_LOG}" 2>&1 &
    echo "  Axum 服务端已启动（二进制），日志：${SERVER_LOG}"
  else
    nohup env \
      RG_LLM_BACKEND="${RG_LLM_BACKEND}" \
      RG_SKILL_BUDGET_CHARS="${RG_SKILL_BUDGET_CHARS}" \
      RG_OLLAMA_TIMEOUT_SECS="${RG_OLLAMA_TIMEOUT_SECS}" \
      RG_OLLAMA_CHAT_TIMEOUT_SECS="${RG_OLLAMA_CHAT_TIMEOUT_SECS}" \
      "${CLOUD_ENV[@]}" \
      cargo run --release >>"${SERVER_LOG}" 2>&1 &
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
elif [ "${RG_USE_OLLAMA}" = "1" ]; then
  echo "  Ollama       - http://localhost:11434  - [未运行]"
else
  echo "  Ollama       - （云端模式，未启用；LLM 通道 RG_LLM_BACKEND=${RG_LLM_BACKEND}）"
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
