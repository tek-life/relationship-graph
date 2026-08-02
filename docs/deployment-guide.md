# 关系图谱系统 — 部署与运行指南

## 1. 环境准备

### 1.1 系统要求

| 项目 | 最低版本 | 推荐版本 | 说明 |
|------|---------|---------|------|
| **操作系统** | — | — | Linux (Ubuntu 22.04+)、macOS 12+、Windows 10/11 |
| **Rust** | Edition 2021 | Rust 1.75+ | 后端编译 |
| **Node.js** | 18 LTS | 20 LTS | 前端构建 |
| **npm** | 9+ | 10+ | 随 Node.js 一同安装 |
| **内存** | 4 GB | 8 GB+ | 本地 LLM 需要更多内存 |
| **磁盘空间** | 5 GB | 10 GB+ | Rust 编译缓存较大 |

### 1.2 安装 Rust

**Linux / macOS（推荐）：**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**Windows：**

从 <https://rustup.rs/> 下载并运行 `rustup-init.exe`。安装时需要安装 Visual Studio C++ Build Tools（安装器会提示）。

**验证安装：**

```bash
rustc --version
cargo --version
```

预期输出类似：

```
rustc 1.80.0 (xxxxxx 2024-xx-xx)
cargo 1.80.0 (xxxxxx 2024-xx-xx)
```

### 1.3 安装 Node.js 和 npm

**Linux (Ubuntu/Debian)：**

```bash
# 推荐使用 NodeSource 官方源
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs

# 或使用 nvm
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.0/install.sh | bash
nvm install 20
nvm use 20
```

**macOS：**

```bash
# 使用 Homebrew
brew install node@20

# 或使用 nvm
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.0/install.sh | bash
nvm install 20
```

**Windows：**

从 <https://nodejs.org/> 下载 LTS 版本的 MSI 安装包，按向导安装即可。

**验证安装：**

```bash
node --version
npm --version
```

预期输出类似：

```
v20.x.x
10.x.x
```

### 1.4 安装 Ollama（可选，用于本地 LLM）

Ollama 用于在本地运行大语言模型，为自然语言查询（NLQ）提供智能提取能力。如果不安装，系统将自动降级为规则提取模式。

**Linux：**

```bash
curl -fsSL https://ollama.com/install.sh | sh
```

**macOS：**

从 <https://ollama.com/download> 下载 macOS 客户端。

**Windows：**

从 <https://ollama.com/download> 下载 Windows 安装程序。

**安装并验证模型：**

```bash
# 启动 Ollama 服务（Linux 安装后自动启动）
ollama serve &

# 拉取推荐模型（约 4.7GB）
ollama pull qwen2.5:7b

# 验证
ollama list
```

### 1.5 其他依赖

**Linux 编译依赖**（Rust 后端编译 SQLCipher 需要）：

```bash
# Ubuntu/Debian
sudo apt-get install -y build-essential pkg-config libssl-dev

# CentOS/RHEL
sudo yum groupinstall "Development Tools"
sudo yum install -y openssl-devel pkgconfig
```

**macOS：**

确保已安装 Xcode Command Line Tools：

```bash
xcode-select --install
```

**Git：**

```bash
# Linux
sudo apt-get install -y git

# macOS（随 Xcode CLT 安装）
# Windows：从 https://git-scm.com 下载
```

---

## 2. 获取代码

### 2.1 克隆仓库

```bash
git clone <your-repo-url> relationship-graph
cd relationship-graph
```

### 2.2 项目结构说明

```
relationship-graph/
├── server/                 # Rust 后端（Axum HTTP API）
│   ├── src/
│   │   ├── main.rs         # 入口：启动 HTTP 服务
│   │   ├── api/            # HTTP 路由与处理器
│   │   ├── db/             # SQLCipher 加密数据库层
│   │   ├── llm/            # LLM Provider 抽象（Ollama/OpenAI/规则降级）
│   │   ├── security/       # JWT 认证与敏感信息保护
│   │   ├── nlq.rs          # 自然语言查询引擎
│   │   ├── nlq_config.rs   # NLQ 关键词配置加载
│   │   ├── infer.rs        # 关系推断引擎
│   │   ├── state.rs        # 应用状态管理
│   │   └── types.rs        # 共享类型定义
│   ├── config/
│   │   └── nlq_keywords.json  # NLQ 关键词配置文件
│   └── Cargo.toml          # Rust 依赖清单
├── src/                    # React 前端
│   ├── components/         # UI 组件
│   ├── hooks/              # React 自定义 Hooks
│   ├── services/           # API 客户端、Token 管理
│   └── types/              # TypeScript 类型定义
├── src-tauri/              # Tauri 桌面端（可选）
├── package.json            # 前端依赖与脚本
├── vite.config.ts          # Vite 构建配置
├── tsconfig.json           # TypeScript 配置
└── tailwind.config.js      # Tailwind CSS 配置
```

---

## 3. 后端部署

### 3.1 安装 Rust 依赖

```bash
cd server
cargo fetch
cd ..
```

首次运行会下载并编译所有 Rust crate，耗时较长（约 3-10 分钟，取决于网络和机器性能）。

主要依赖：

| Crate | 版本 | 用途 |
|-------|------|------|
| axum | 0.7 | HTTP 框架（含 multipart 支持） |
| tokio | 1 | 异步运行时 |
| tower-http | 0.6 | CORS 和请求追踪中间件 |
| rusqlite | 0.32 | SQLCipher 加密数据库 |
| reqwest | 0.11 | HTTP 客户端（调用 LLM 服务） |
| jsonwebtoken | 9 | JWT 认证 |
| argon2 | 0.5 | 密码哈希 |

### 3.2 环境变量配置（完整列表）

以下环境变量用于控制后端行为。**没有 `.env` 文件**，需要手动设置或在启动命令中指定。

| 变量名 | 说明 | 默认值 | 是否必填 |
|--------|------|--------|----------|
| `RG_PORT` | HTTP 服务监听端口 | `8790` | 否 |
| `RG_DATA_DIR` | 数据目录（数据库、盐值文件存放路径） | 系统数据目录 + `/relationship-graph`（Linux: `~/.local/share/relationship-graph`） | 否 |
| `RG_JWT_SECRET` | JWT 签名密钥（64 位十六进制字符串） | 随机生成（**重启后所有 token 失效**） | **生产环境必填** |
| `RG_LLM_PROVIDER` | LLM Provider 优先级链（逗号分隔） | `ollama,fallback` | 否 |
| `RG_OLLAMA_URL` | Ollama 服务地址 | `http://localhost:11434` | 否 |
| `RG_OLLAMA_MODEL` | Ollama 使用的模型 | `qwen2.5:7b` | 否 |
| `RG_OLLAMA_TIMEOUT_SECS` | Ollama 请求超时时间（秒） | `10` | 否 |
| `RG_OPENAI_API_KEY` | OpenAI API 密钥 | — | 使用 OpenAI 时必填 |
| `RG_OPENAI_BASE_URL` | OpenAI 兼容 API 基础地址 | `https://api.openai.com/v1` | 否 |
| `RG_OPENAI_MODEL` | OpenAI 使用的模型 | `gpt-4o-mini` | 否 |
| `RG_OPENAI_TIMEOUT_SECS` | OpenAI 请求超时时间（秒） | `15` | 否 |
| `RG_NLQ_KEYWORDS_PATH` | NLQ 关键词配置文件路径 | `config/nlq_keywords.json`（相对于后端工作目录） | 否 |
| `RUST_LOG` | 日志级别 | `info` | 否 |

**生成 JWT 密钥（生产环境）：**

```bash
# 生成 64 位十六进制字符串
openssl rand -hex 32
```

将生成的值设置为 `RG_JWT_SECRET`。

**创建环境变量文件（可选但推荐）：**

```bash
cat > server/.env << 'EOF'
RG_PORT=8790
RG_DATA_DIR=./data
RG_JWT_SECRET=你的64位十六进制密钥
RG_LLM_PROVIDER=ollama,fallback
RG_OLLAMA_URL=http://localhost:11434
RG_OLLAMA_MODEL=qwen2.5:7b
RUST_LOG=info
EOF
```

> **注意**：项目不使用 `dotenv` 等自动加载库，`.env` 文件需要手动 `source` 或在启动命令中引用。

### 3.3 编译后端

```bash
cd server

# 开发编译（快速，含调试信息）
cargo build

# 生产编译（优化，编译时间较长）
cargo build --release

cd ..
```

编译产物位于 `server/target/debug/relationship-graph-server` 或 `server/target/release/relationship-graph-server`。

### 3.4 启动后端服务

**方式一：cargo run（开发模式）**

```bash
cd server

# 不设置环境变量（使用默认值）
cargo run

# 或指定环境变量
RG_PORT=8790 RG_JWT_SECRET=$(openssl rand -hex 32) cargo run

# 使用 .env 文件
source .env && cargo run

cd ..
```

**方式二：直接运行编译产物**

```bash
# 开发版
./server/target/debug/relationship-graph-server

# 生产版
./server/target/release/relationship-graph-server
```

**启动成功的标志输出：**

```
INFO server_start addr=0.0.0.0:8790
```

后端监听 `0.0.0.0:8790`，同一局域网内的设备均可通过 `http://<服务器IP>:8790` 访问。

### 3.5 数据库初始化

数据库**无需手动初始化**。首次启动后，当用户通过前端设置主密码时，系统会自动：

1. 在 `RG_DATA_DIR` 目录下创建 `app.db`（SQLCipher 加密数据库）
2. 创建 `salt.hex`（加密盐值文件）
3. 自动执行数据库迁移（创建所有表和索引）

数据库表结构包括：
- `users` — 用户账户
- `persons` — 联系人
- `relationships` — 关系连接
- `interactions` — 互动记录
- `entity_mentions` — 实体提及
- `settings` — 系统设置

---

## 4. 前端部署

### 4.1 安装前端依赖

```bash
npm install
```

主要前端依赖：

| 包名 | 版本 | 用途 |
|------|------|------|
| react | ^18.3.1 | UI 框架 |
| cytoscape | ^3.30.3 | 关系图谱可视化 |
| pinyin-pro | ^3.28.2 | 拼音分组排序 |
| tesseract.js | ^7.0.0 | 图片 OCR 识别 |
| xlsx | ^0.18.5 | Excel 导入 |
| tailwindcss | ^3.4.7 | CSS 框架 |
| vite | ^5.3.5 | 构建工具 |
| typescript | ^5.5.4 | 类型检查 |

### 4.2 环境变量配置

前端通过 Vite 的环境变量机制配置。在项目根目录创建 `.env` 或 `.env.local` 文件：

```bash
cat > .env.local << 'EOF'
# 后端 API 地址（默认自动检测当前页面 hostname）
VITE_API_BASE=http://localhost:8790

# 启用旧版认证模式（可选，兼容旧 Token 方式）
# VITE_LEGACY_AUTH=true
EOF
```

| 变量名 | 说明 | 默认值 | 是否必填 |
|--------|------|--------|----------|
| `VITE_API_BASE` | 后端 API 基础 URL | `http://<当前页面hostname>:8790`（自动检测） | 否 |
| `VITE_LEGACY_AUTH` | 启用旧版 sessionStorage Token 认证 | 未设置（关闭） | 否 |

> **提示**：如果前端和后端运行在同一台机器的开发模式下，默认值即可正常工作。如果需要从其他设备访问（如手机），需要确保 `VITE_API_BASE` 指向后端实际 IP。

### 4.3 开发模式启动

```bash
# 仅启动前端（需要后端已在运行）
npm run dev:frontend
```

前端开发服务器启动后监听 `0.0.0.0:1420`（严格端口模式，端口被占用时会报错而非自动切换）。

访问地址：`http://localhost:1420`

### 4.4 生产构建

```bash
npm run build
```

构建产物输出到 `dist/` 目录，包含纯静态文件，可用任何 Web 服务器托管（Nginx、Caddy 等）。

预览生产构建：

```bash
npm run preview
```

---

## 5. 一键启动（开发环境）

### 5.1 npm run dev 联动启动

项目配置了 `concurrently` 工具，一条命令同时启动前端和后端：

```bash
npm run dev
```

此命令等价于同时执行：

```bash
npm run dev:frontend   # vite（端口 1420）
npm run dev:backend    # cd server && cargo run（端口 8790）
```

- `-k` 参数确保任一进程退出时自动终止另一个
- 终端输出以 `frontend` 和 `backend` 前缀区分

### 5.2 验证服务状态

**检查后端：**

```bash
curl -s http://localhost:8790/api/health 2>/dev/null || echo "后端未响应"
```

> 注意：如果未配置 `/api/health` 端点，也可以用以下方式确认后端正在监听：

```bash
# 检查端口是否被监听
ss -tlnp | grep 8790
# 或
lsof -i :8790
```

**检查前端：**

```bash
curl -s -o /dev/null -w "%{http_code}" http://localhost:1420
```

预期输出 `200`。

---

## 6. 系统配置详解

### 6.1 JWT 密钥配置

系统使用 JWT（JSON Web Token）进行用户认证：

- **Access Token**：有效期 **2 小时**，存储在 `localStorage`
- **Refresh Token**：有效期 **30 天**，存储在 `localStorage`
- 前端在 Access Token 过期时自动使用 Refresh Token 刷新

**生产环境强烈建议设置 `RG_JWT_SECRET`**，否则每次重启后端服务都会生成新的随机密钥，导致所有已登录用户被强制登出。

```bash
# 生成密钥
export RG_JWT_SECRET=$(openssl rand -hex 32)
echo "你的 JWT 密钥: $RG_JWT_SECRET"
```

### 6.2 LLM 服务配置（Ollama / OpenAI / 降级链）

系统采用 **Provider 降级链** 设计，按优先级依次尝试不同的 LLM 服务，某个失败时自动切换到下一个：

```
环境变量: RG_LLM_PROVIDER=ollama,openai,fallback
```

**三种 Provider：**

| Provider | 说明 | 所需配置 |
|----------|------|---------|
| `ollama` | 本地 Ollama 服务 | 需安装 Ollama 并拉取模型 |
| `openai` | OpenAI 兼容 API（也支持其他兼容服务） | 需设置 `RG_OPENAI_API_KEY` |
| `fallback` | 纯正则/规则提取，无需外部服务 | 无 |

**降级逻辑：**

1. 按 `RG_LLM_PROVIDER` 指定的顺序依次尝试
2. 每个 Provider 有 30 秒超时限制
3. 如果某个 Provider 返回空结果或超时，自动尝试下一个
4. `fallback` 始终作为最终保障（如果配置了的话）

**常用配置示例：**

```bash
# 仅使用 Ollama（默认）
RG_LLM_PROVIDER=ollama,fallback

# 仅使用 OpenAI
RG_LLM_PROVIDER=openai,fallback
RG_OPENAI_API_KEY=sk-xxxxx

# Ollama 优先，OpenAI 备选
RG_LLM_PROVIDER=ollama,openai,fallback

# 不使用任何 LLM（纯规则模式）
RG_LLM_PROVIDER=fallback

# 使用兼容 OpenAI 的第三方 API（如 DeepSeek、智谱等）
RG_LLM_PROVIDER=openai,fallback
RG_OPENAI_API_KEY=your-api-key
RG_OPENAI_BASE_URL=https://api.deepseek.com/v1
RG_OPENAI_MODEL=deepseek-chat
```

### 6.3 NLQ 关键词配置

自然语言查询（NLQ）通过关键词匹配识别用户意图。配置文件位于 `server/config/nlq_keywords.json`。

**支持的意图类型：**

| 意图 | 说明 | 权重 | 示例关键词 |
|------|------|------|-----------|
| `create_person` | 添加新联系人 | 1.0 | "新加"、"添加"、"刚认识" |
| `update_person` | 更新联系人信息 | 1.0 | "换了"、"去了"、"跳槽" |
| `add_interaction` | 记录互动 | 1.0 | "聊了"、"开会"、"见了面" |
| `find_path` | 查找关系路径 | 1.2 | "怎么认识"、"通过谁"、"中间人" |
| `search_people` | 搜索联系人 | 0.8 | "找"、"搜索"、"谁是" |

**配置文件路径可通过 `RG_NLQ_KEYWORDS_PATH` 环境变量覆盖：**

```bash
RG_NLQ_KEYWORDS_PATH=/path/to/custom_keywords.json
```

如果文件不存在或格式错误，系统会使用内置的默认关键词配置，不会中断服务。

**热加载**：系统支持运行时替换关键词配置（通过 API 或重启），无需重新编译。

### 6.4 OAuth 第三方登录（预留）

数据库 `users` 表中预留了 `oauth_provider` 和 `oauth_id` 字段，支持未来扩展第三方登录（如微信、GitHub 等）。当前版本尚未实现 OAuth 流程。

---

## 7. 首次使用

### 7.1 访问系统

确保前后端均已启动后，在浏览器中访问：

```
http://localhost:1420
```

如果从局域网其他设备访问：

```
http://<运行前端的机器IP>:1420
```

> **注意**：前端页面加载后会自动连接后端 API（地址由 `VITE_API_BASE` 或当前页面 hostname + 端口 8790 决定）。

### 7.2 数据库初始化（主密码设置）

首次访问时，系统会引导用户设置数据库加密的主密码。该密码用于 SQLCipher 加密，**请妥善保存，丢失后无法恢复数据**。

系统会在数据目录（默认 `~/.local/share/relationship-graph/`）创建：
- `app.db` — 加密的 SQLite 数据库
- `salt.hex` — 加密盐值

### 7.3 注册/登录

系统支持多用户。首个注册的用户即为管理员。

**注册信息：**
- 用户名（必填，唯一）
- 密码（必填）
- 邮箱（可选，唯一）
- 手机号（可选，唯一）
- 显示名称（可选）

**登录方式：**
- 用户名 + 密码
- 邮箱 + 密码
- 手机号 + 密码

### 7.4 新用户引导

登录后系统提供引导向导（OnboardingWizard），帮助用户快速了解：
- 添加联系人
- 建立关系连接
- 记录互动
- 使用自然语言查询

---

## 8. 常见问题排查

### 8.1 后端启动失败

**问题：端口被占用**

```
端口绑定失败
```

排查：

```bash
# 查看占用端口的进程
ss -tlnp | grep 8790
# 或
lsof -i :8790

# 更换端口启动
RG_PORT=8791 cargo run
```

**问题：数据目录无权限**

```
无法定位系统数据目录
```

排查：

```bash
# 指定自定义数据目录
mkdir -p ./data
RG_DATA_DIR=./data cargo run
```

**问题：编译失败 — SQLCipher 相关**

```
error: failed to run custom build command for `libsqlite3-sys`
```

排查：

```bash
# 确保安装了编译依赖
sudo apt-get install -y build-essential pkg-config libssl-dev

# 清理缓存后重新编译
cd server
cargo clean
cargo build
```

### 8.2 前端无法连接后端

**问题：页面打开但显示网络错误**

排查步骤：

```bash
# 1. 确认后端在运行
curl http://localhost:8790/api/auth/me
# 预期：返回 401（而非连接被拒绝）

# 2. 检查 CORS（后端已配置为 CORS 全开放）
curl -v -X OPTIONS http://localhost:8790/api/auth/me \
  -H "Origin: http://localhost:1420" \
  -H "Access-Control-Request-Method: GET"

# 3. 如果从其他设备访问，确认 IP 和端口
# 前端 .env.local 中设置正确的 API 地址
echo "VITE_API_BASE=http://<后端IP>:8790" > .env.local
```

**问题：跨域错误**

后端已配置 `CorsLayer::very_permissive()`，正常情况下不应出现跨域问题。如果出现，检查是否有反向代理在中间拦截了请求。

### 8.3 数据库问题

**问题：数据库无法打开**

排查：

```bash
# 检查数据目录
ls -la ~/.local/share/relationship-graph/
# 或自定义目录
ls -la $RG_DATA_DIR/

# 检查文件权限
chmod 700 ~/.local/share/relationship-graph/

# 如果 salt.hex 丢失但 app.db 存在，数据库将无法打开
# 此时需要重置（会丢失数据）
rm ~/.local/share/relationship-graph/app.db
rm ~/.local/share/relationship-graph/salt.hex
```

**问题：数据库迁移失败**

查看日志中的 `schema_migrate` 相关信息：

```bash
RUST_LOG=db=debug cargo run
```

### 8.4 LLM 服务不可用

**问题：Ollama 连接失败**

排查：

```bash
# 确认 Ollama 在运行
curl http://localhost:11434/api/tags
# 预期：返回模型列表 JSON

# 确认模型已下载
ollama list
# 确认 qwen2.5:7b 在列表中

# 如果 Ollama 未安装，降级到规则模式即可正常使用
RG_LLM_PROVIDER=fallback cargo run
```

**问题：OpenAI API 调用失败**

排查：

```bash
# 验证 API Key
curl -H "Authorization: Bearer $RG_OPENAI_API_KEY" \
  https://api.openai.com/v1/models

# 检查日志
RUST_LOG=llm=debug cargo run
```

**问题：自然语言查询结果不准确**

即使不使用 LLM，系统仍可通过规则降级（`fallback`）模式处理基本查询。但智能提取（公司名、职位等）能力会下降。

### 8.5 语音输入不工作

系统使用浏览器原生 Web Speech API 和/或 Whisper 本地模型进行语音输入。

排查：

```bash
# 1. 确认浏览器支持 Web Speech API
#    Chrome/Edge 支持，Firefox 需要手动启用

# 2. 确认浏览器已授权麦克风权限
#    在地址栏左侧的锁图标中检查权限设置

# 3. 确认 HTTPS（生产环境）或 localhost（开发环境）
#    Web Speech API 在非安全上下文中不可用
```

---

## 9. 生产环境部署建议

### 9.1 安全配置

**必须完成的安全加固：**

```bash
# 1. 设置固定 JWT 密钥
export RG_JWT_SECRET=$(openssl rand -hex 32)

# 2. 限制数据目录权限
chmod 700 /var/lib/relationship-graph

# 3. 使用 HTTPS（通过反向代理实现，见 9.2 节）

# 4. 收紧 CORS — 修改后端代码中的 CorsLayer
#    将 CorsLayer::very_permissive() 改为指定可信域名白名单

# 5. 设置 RUST_LOG 为 warn 或 error，避免敏感信息泄漏到日志
export RUST_LOG=warn
```

### 9.2 反向代理

**Nginx 配置示例：**

```nginx
server {
    listen 443 ssl http2;
    server_name your-domain.com;

    ssl_certificate     /etc/ssl/certs/your-cert.pem;
    ssl_certificate_key /etc/ssl/private/your-key.pem;

    # 前端静态文件
    location / {
        root /var/www/relationship-graph/dist;
        try_files $uri $uri/ /index.html;
    }

    # 后端 API 代理
    location /api/ {
        proxy_pass http://127.0.0.1:8790;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

server {
    listen 80;
    server_name your-domain.com;
    return 301 https://$host$request_uri;
}
```

此时前端 `VITE_API_BASE` 可留空（默认使用当前页面 hostname），因为前后端同域。

### 9.3 数据备份

**备份脚本：**

```bash
#!/bin/bash
BACKUP_DIR="/var/backups/relationship-graph"
DATA_DIR="${RG_DATA_DIR:-$HOME/.local/share/relationship-graph}"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p "$BACKUP_DIR"

# 备份数据库文件（SQLCipher 加密状态）
cp "$DATA_DIR/app.db" "$BACKUP_DIR/app.db.$DATE"
cp "$DATA_DIR/salt.hex" "$BACKUP_DIR/salt.hex.$DATE"

# 清理 30 天前的备份
find "$BACKUP_DIR" -name "app.db.*" -mtime +30 -delete
find "$BACKUP_DIR" -name "salt.hex.*" -mtime +30 -delete

echo "备份完成: $BACKUP_DIR"
```

建议通过 cron 定期执行：

```bash
# 每天凌晨 2 点备份
0 2 * * * /path/to/backup.sh
```

### 9.4 监控与日志

**systemd 服务配置：**

```ini
[Unit]
Description=Relationship Graph Server
After=network.target

[Service]
Type=simple
User=relationship-graph
WorkingDirectory=/opt/relationship-graph/server
Environment=RG_PORT=8790
Environment=RG_DATA_DIR=/var/lib/relationship-graph
Environment=RG_JWT_SECRET=你的密钥
Environment=RG_LLM_PROVIDER=ollama,fallback
Environment=RUST_LOG=info
ExecStart=/opt/relationship-graph/server/target/release/relationship-graph-server
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

**启用并启动服务：**

```bash
sudo systemctl daemon-reload
sudo systemctl enable relationship-graph
sudo systemctl start relationship-graph

# 查看状态
sudo systemctl status relationship-graph

# 查看日志
sudo journalctl -u relationship-graph -f
```

**日志级别说明：**

| 级别 | 说明 |
|------|------|
| `error` | 仅记录错误 |
| `warn` | 记录警告和错误（推荐生产环境） |
| `info` | 记录一般信息（默认） |
| `debug` | 记录调试信息（开发/排查时使用） |
| `trace` | 记录所有追踪信息 |

可通过 `RUST_LOG` 精细控制不同模块的日志级别：

```bash
RUST_LOG=info,db=debug,llm=trace
```
