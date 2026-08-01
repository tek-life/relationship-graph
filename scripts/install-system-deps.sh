#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 安装 v1.3 个人关系图谱服务端所需的系统级依赖
# 运行环境：WSL2 + Ubuntu 24.04
# 注意：本脚本需要 sudo 权限
# =============================================================================

echo "==> 更新 apt 索引"
sudo apt update

echo "==> 安装基础构建工具"
sudo apt install -y \
  build-essential \
  cmake \
  curl \
  wget \
  git \
  file \
  pkg-config \
  libssl-dev \
  libsqlite3-dev \
  ca-certificates \
  gnupg \
  lsb-release

echo "==> 安装 Tauri 桌面壳编译依赖（保留，以备后续构建 Windows 客户端）"
sudo apt install -y \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libwebkit2gtk-4.1-dev

echo "==> 安装 OCR 依赖（Tesseract）"
sudo apt install -y \
  tesseract-ocr \
  libtesseract-dev \
  tesseract-ocr-chi-sim \
  tesseract-ocr-chi-tra

echo "==> 安装 PostgreSQL 客户端（服务端使用 Docker 运行）"
sudo apt install -y postgresql-client

echo "==> 安装 Docker Engine 与 Docker Compose 插件"
if ! command -v docker &>/dev/null; then
  sudo install -m 0755 -d /etc/apt/keyrings
  curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
  echo \
    "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
    https://download.docker.com/linux/ubuntu \
    $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list >/dev/null
  sudo apt update
  sudo apt install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
  sudo usermod -aG docker "$USER"
  echo "提示：已将当前用户加入 docker 组。请退出并重新登录 WSL，或运行 'newgrp docker' 使权限生效。"
else
  echo "Docker 已安装，跳过"
fi

echo "==> 系统依赖安装完成"
echo "建议执行："
echo "  newgrp docker"
echo "  docker run hello-world"
