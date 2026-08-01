#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 安装 v1.3 个人关系图谱服务端所需的 AI 服务依赖
# 运行环境：WSL2 + Ubuntu 24.04
# 注意：
#   1. whisper.cpp 优先使用 SSH 协议 clone，失败后回退 HTTPS
#   2. 若 WSL 需走宿主机代理访问 GitHub：先 source ./scripts/setup-proxy.sh
#   3. 模型下载默认使用 hf-mirror.com 镜像（huggingface.co 常不可达）
#      如需切回官方源：HF_ENDPOINT=https://huggingface.co ./install-ai-deps.sh
#   4. whisper.cpp 新版使用 CMake 构建，需先安装 cmake（见 install-system-deps.sh）
#   5. whisper-cli 安装到 ~/.local/bin，无需 sudo（确保该目录在 PATH 中）
# =============================================================================

APP_DATA_DIR="${HOME}/.local/share/relationship-graph"
MODELS_DIR="${APP_DATA_DIR}/models"
WHISPER_MODEL="${MODELS_DIR}/ggml-base.bin"
HF_ENDPOINT="${HF_ENDPOINT:-https://hf-mirror.com}"
BIN_DIR="${HOME}/.local/bin"

mkdir -p "${MODELS_DIR}" "${BIN_DIR}"


echo "==> 编译安装 whisper-cli（Whisper.cpp）"
WHISPER_SRC_DIR="${APP_DATA_DIR}/whisper.cpp"
if [ ! -d "${WHISPER_SRC_DIR}" ]; then
  if ! git clone --depth 1 git@github.com:ggerganov/whisper.cpp.git "${WHISPER_SRC_DIR}" 2>/dev/null; then
    echo "SSH克隆失败，尝试HTTPS..."
    git clone --depth 1 https://github.com/ggerganov/whisper.cpp.git "${WHISPER_SRC_DIR}"
  fi
fi
cd "${WHISPER_SRC_DIR}"
git pull --ff-only || true
cmake -B build -DCMAKE_BUILD_TYPE=Release
if ! cmake --build build --config Release -j"$(nproc)"; then
  echo "[ERROR] whisper.cpp 编译失败，请检查 cmake 和编译器版本"
  exit 1
fi
install "${WHISPER_SRC_DIR}/build/bin/whisper-cli" "${BIN_DIR}/whisper-cli"
echo "whisper-cli 已安装到：${BIN_DIR}/whisper-cli"

echo "==> 下载 Whisper base 模型（来源：${HF_ENDPOINT}）"
if [ ! -f "${WHISPER_MODEL}" ]; then
  RETRY_COUNT=0
  MAX_RETRIES=3
  while [ "$RETRY_COUNT" -lt "$MAX_RETRIES" ]; do
    if curl -fL --connect-timeout 15 -o "${WHISPER_MODEL}" \
      "${HF_ENDPOINT}/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"; then
      break
    fi
    RETRY_COUNT=$((RETRY_COUNT + 1))
    if [ "$RETRY_COUNT" -lt "$MAX_RETRIES" ]; then
      echo "[WARN] 模型下载失败（第 ${RETRY_COUNT} 次），5秒后重试..."
      sleep 5
      rm -f "${WHISPER_MODEL}"
    else
      echo "[ERROR] 模型下载失败，已重试 ${MAX_RETRIES} 次"
      exit 1
    fi
  done
else
  echo "模型已存在，跳过：${WHISPER_MODEL}"
fi

# 检查 ~/.local/bin 是否在 PATH 中
if [[ ":${PATH}:" != *":${BIN_DIR}:"* ]]; then
  echo ""
  echo "[提示] ${BIN_DIR} 不在当前 PATH 中，请添加到 ~/.bashrc："
  echo "  export PATH=\"${BIN_DIR}:\${PATH}\""
fi

echo "==> AI 依赖安装完成"
echo "模型路径："
echo "  Ollama 模型：由 ollama 管理"
echo "  Whisper 模型：${WHISPER_MODEL}"
