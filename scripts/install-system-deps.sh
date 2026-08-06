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

echo "==> 系统依赖安装完成"
