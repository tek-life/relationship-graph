#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 配置 WSL 使用宿主机代理（用于访问 GitHub / HuggingFace 等）
# 运行环境：WSL2 + Ubuntu 24.04
# 宿主机代理端口说明（根据本项目环境）：
#   7891 = HTTP/HTTPS 代理 兼 SOCKS5 代理
# 用法：
#   source ./scripts/setup-proxy.sh          # 在当前 shell 生效（curl/wget/git）
#   ./scripts/setup-proxy.sh --persist-git   # 额外将代理写入 git 全局配置
# =============================================================================

# WSL2 默认（NAT 模式）下宿主机 IP = 默认路由网关
HOST_IP="${HOST_IP:-$(ip route show default | awk '{print $3}' | head -1)}"
PROXY_PORT="${PROXY_PORT:-7891}"
HTTP_PROXY="http://${HOST_IP}:${PROXY_PORT}"
SOCKS_PROXY="socks5h://${HOST_IP}:${PROXY_PORT}"

echo "==> 使用代理：${HTTP_PROXY} / ${SOCKS_PROXY}"

# 供 curl / wget / 多数 CLI 识别的环境变量
export http_proxy="${HTTP_PROXY}"
export https_proxy="${HTTP_PROXY}"
export all_proxy="${SOCKS_PROXY}"
export HTTP_PROXY="${HTTP_PROXY}"
export HTTPS_PROXY="${HTTP_PROXY}"
export ALL_PROXY="${SOCKS_PROXY}"
# 本地地址不走代理
export no_proxy="localhost,127.0.0.1,::1"
export NO_PROXY="localhost,127.0.0.1,::1"

if [[ "${1:-}" == "--persist-git" ]]; then
  git config --global http.proxy "${HTTP_PROXY}"
  git config --global https.proxy "${HTTP_PROXY}"
  echo "==> 已写入 git 全局代理配置"
  echo "    取消命令：git config --global --unset http.proxy; git config --global --unset https.proxy"
fi

echo "==> 代理已在当前 shell 生效。快速验证："
echo "    curl -I https://github.com"
