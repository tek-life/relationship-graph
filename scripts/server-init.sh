#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# 个人关系图谱 - 阿里云 ECS 一次性初始化脚本
# 运行环境：目标服务器（Ubuntu 24.04 LTS），只需运行一次
#
# 设计目标：
#   - 服务器上不安装 Rust / Node.js / Ollama（产物由本地构建后 rsync 上传）
#   - LLM 走阿里百炼云端模型（RG_LLM_BACKEND=cloud），API Key 由服务端
#     自动读取 ~/.config/rg-cloud-api-key
#   - 两个服务均用 systemd **用户级** unit 托管（无需 root 发版）：
#       relationship-graph.service  → Axum 后端（8790）
#       rg-caddy.service            → Caddy 前端 + /api 反代（8080）
#
# 远端目录布局：
#   ~/relationship-graph/
#   ├── bin/relationship-graph-server   （release.sh 上传）
#   ├── web/                            （release.sh 上传 dist/ 内容）
#   ├── Caddyfile                       （本脚本生成）
#   └── logs/                           （Caddy 访问日志）
#   ~/.local/share/relationship-graph/  （SQLCipher 数据库 + db.key，首次 setup 生成）
#
# 用法（在 ECS 上执行）：
#   scp scripts/server-init.sh <user>@<ip>:~/ && ssh <user>@<ip> bash server-init.sh
# =============================================================================

APP_DIR="${HOME}/relationship-graph"
LOG_DIR="${APP_DIR}/logs"
CADDY_PORT="${RG_CADDY_PORT:-8080}"

echo "==> 步骤 1/5: 安装基础运行时依赖"
sudo apt update
sudo apt install -y \
  curl \
  ca-certificates \
  rsync \
  gnupg \
  lsof

echo "==> 步骤 1b: 安装 OCR 依赖（Tesseract，可选但默认安装）"
sudo apt install -y \
  tesseract-ocr \
  tesseract-ocr-chi-sim \
  tesseract-ocr-chi-tra || echo "  [WARN] Tesseract 安装失败，名片 OCR 功能不可用（不影响其他功能）"

echo "==> 步骤 2/5: 安装 Caddy"
if command -v caddy >/dev/null 2>&1; then
  echo "  Caddy 已安装：$(caddy version)"
else
  # 官方 apt 仓库（cloudsmith）
  sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
    | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
    | sudo tee /etc/apt/sources.list.d/caddy-stable.list
  sudo apt update
  sudo apt install -y caddy
  echo "  Caddy 安装完成：$(caddy version)"
fi
# apt 版 Caddy 自带的系统级 unit 使用 /etc/caddy/Caddyfile，与本方案无关，禁用避免占用 80 端口
sudo systemctl disable --now caddy 2>/dev/null || true

echo "==> 步骤 3/5: 创建目录与 Caddyfile"
mkdir -p "${APP_DIR}/bin" "${APP_DIR}/web" "${LOG_DIR}"

cat > "${APP_DIR}/Caddyfile" <<EOF
# 由 server-init.sh 生成：前端静态 + /api 反代（端口 ${CADDY_PORT}）
:${CADDY_PORT} {
    root * ${APP_DIR}/web

    # handle 块保证 /api/* 优先且排他走反代（try_files 默认先于 reverse_proxy
    # 执行，平铺写法会把 /api/xxx 重写成 /index.html 后落 file_server 返回 405）
    handle /api/* {
        reverse_proxy localhost:8790
    }

    # 静态文件 + SPA fallback（与 /api 块互斥）
    handle {
        try_files {path} /index.html
        file_server
    }

    # PWA Service Worker / manifest 不缓存
    @sw path /sw.js
    header @sw Cache-Control "no-cache"
    @manifest path /manifest.json
    header @manifest Cache-Control "no-cache"

    # 带 hash 的静态资源长缓存
    @assets path /assets/*
    header @assets Cache-Control "public, max-age=31536000, immutable"

    header {
        X-Content-Type-Options "nosniff"
        X-Frame-Options "DENY"
        X-XSS-Protection "1; mode=block"
        Referrer-Policy "strict-origin-when-cross-origin"
    }

    log {
        output file ${LOG_DIR}/caddy-access.log
        format json
    }
}
EOF
echo "  已生成 ${APP_DIR}/Caddyfile"

echo "==> 步骤 4/5: 配置 systemd 用户级服务"
mkdir -p "${HOME}/.config/systemd/user"

cat > "${HOME}/.config/systemd/user/relationship-graph.service" <<EOF
[Unit]
Description=Relationship Graph Axum Server (port 8790)
After=network-online.target

[Service]
ExecStart=${APP_DIR}/bin/relationship-graph-server
WorkingDirectory=${APP_DIR}
Environment=RG_PORT=8790
Environment=RG_LLM_BACKEND=cloud
Environment=RG_SKILL_BUDGET_CHARS=8000
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
EOF

cat > "${HOME}/.config/systemd/user/rg-caddy.service" <<EOF
[Unit]
Description=Caddy reverse proxy for Relationship Graph (port ${CADDY_PORT})
After=network-online.target

[Service]
ExecStart=/usr/bin/caddy run --config ${APP_DIR}/Caddyfile
WorkingDirectory=${APP_DIR}
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
EOF

systemctl --user daemon-reload
systemctl --user enable relationship-graph.service rg-caddy.service
echo "  已创建并 enable 两个用户级服务（首次发版后由 release.sh 启动）"

# linger：让用户级服务在未登录时也运行、开机自启（需要一次 sudo）
if loginctl show-user "${USER}" 2>/dev/null | grep -q 'Linger=yes'; then
  echo "  linger 已启用"
else
  echo "  启用 linger（需要 sudo 密码）..."
  sudo loginctl enable-linger "${USER}"
fi

echo "==> 步骤 5/5: 配置百炼 API Key 提示"
if [ -f "${HOME}/.config/rg-cloud-api-key" ]; then
  echo "  已检测到 ~/.config/rg-cloud-api-key（内容不回显）"
else
  echo "  [待办] 尚未配置百炼 API Key，请执行："
  echo "    mkdir -p ~/.config && printf '%s' 'sk-xxxxxxxx' > ~/.config/rg-cloud-api-key && chmod 600 ~/.config/rg-cloud-api-key"
fi

echo ""
echo "============================================"
echo "服务器初始化完成！后续步骤："
echo "  1. 确认阿里云安全组已放行 ${CADDY_PORT}/tcp（如需 HTTPS 另放行 80/443）"
echo "  2. 配置百炼 API Key（见上方提示，LLM 对话功能需要）"
echo "  3. 在本地（开发机）运行首次发版："
echo "       RG_HOST=<user>@<ip> ./scripts/release.sh"
echo "  4. 首次部署后初始化数据库（创建 admin 账号）："
echo "       curl -X POST http://<ip>:${CADDY_PORT}/api/auth/setup \\"
echo "         -H 'Content-Type: application/json' \\"
echo "         -d '{\"username\":\"admin\",\"password\":\"<至少8位密码>\"}'"
echo "============================================"
