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
sudo apt-get install -y build-essential pkg-config libssl-dev cmake

# CentOS/RHEL
sudo yum groupinstall "Development Tools"
sudo yum install -y openssl-devel pkgconfig cmake
```

> **依赖说明：**
> - `pkg-config`：Rust 的 `libsqlite3-sys` / `openssl-sys` 等 crate 在编译时通过 pkg-config 定位系统库的头文件和链接路径，缺失会导致 `custom build command failed` 错误。
> - `cmake`：部分平台下 `libsqlite3-sys`（含 SQLCipher）会从源码编译，cmake 是其构建系统的依赖之一。Linux 下若已安装系统 `libsqlite3` 开发包则可能不需要，但建议预装以避免意外。
> - `libssl-dev` / `openssl-devel`：`reqwest` 和 `tokio-tungstenite` 等 crate 的 TLS 功能依赖 OpenSSL 头文件（调用 OpenAI 等 HTTPS 服务时必需）。

**macOS：**

确保已安装 Xcode Command Line Tools：

```bash
xcode-select --install

# 如需 cmake（Homebrew）
brew install cmake
```

**Git：**

```bash
# Linux
sudo apt-get install -y git

# macOS（随 Xcode CLT 安装）
# Windows：从 https://git-scm.com 下载
```

### 1.6 WSL 环境配置（Windows 开发推荐）

本项目主要在 WSL（Windows Subsystem for Linux）中开发。以下为 WSL2 + Ubuntu 的推荐配置。

**安装 WSL2：**

```powershell
# 在 Windows PowerShell（管理员）中执行
wsl --install -d Ubuntu-24.04
```

重启后设置 Linux 用户名和密码，然后进入 WSL。

**WSL 内安装开发工具链：**

```bash
# 更新包管理器
sudo apt-get update && sudo apt-get upgrade -y

# 安装编译工具与依赖
sudo apt-get install -y build-essential pkg-config libssl-dev cmake git curl

# 安装 Node.js 20（见 1.3 节）
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs

# 安装 Rust（见 1.2 节）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**文件系统性能注意事项：**

> WSL2 跨文件系统访问（如从 Linux 访问 `/mnt/c/...` 下的 Windows 文件）I/O 性能较差，会显著拖慢 `cargo build` 和 `npm install`。**强烈建议将项目放在 WSL 原生文件系统**（如 `~/relationship-graph`），而非挂载的 Windows 路径。

```bash
# 推荐：项目放在 WSL 主目录
cd ~
git clone <your-repo-url> relationship-graph
cd relationship-graph
```

**从 Windows 访问 WSL 服务：**

WSL2 会自动将 Linux 服务监听的端口转发到 Windows。后端监听 `0.0.0.0:8790` 后，在 Windows 浏览器中可直接访问 `http://localhost:8790`，前端开发服务器 `http://localhost:1420` 同理。

```bash
# 在 WSL 中启动服务后，Windows 浏览器访问
# 后端 API:    http://localhost:8790/api/health
# 前端页面:    http://localhost:1420
```

**VS Code 远程开发（推荐）：**

```bash
# 在 WSL 项目目录中执行
code .
```

VS Code 会自动安装 "WSL" 扩展并以远程模式打开项目，获得与本地开发一致的体验（语法高亮、Rust-analyzer、TypeScript 智能提示等）。

**ES Module 注意事项：**

WSL 环境下运行 Node.js 脚本（如 `scripts/` 目录下的 `.mjs` 工具）时，若遇到 `ERR_UNKNOWN_FILE_EXTENSION`，确保 `package.json` 中已配置 `"type": "module"`，或使用 `node --experimental-vm-modules` 启动。

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

数据库**无需手动初始化**。后端启动时，应用状态中的数据库连接为 `None`（未解锁状态），所有受保护 API 会返回 `409 Conflict`（"数据库尚未初始化或解锁"）。数据库的创建与解锁通过认证 API 触发：

#### 3.5.1 首次启动流程（数据库未创建）

1. **检查状态** — 前端调用 `GET /api/auth/state`，返回 `{ initialized: false, unlocked: false }`
2. **设置主密码** — 用户在前端设置主密码（至少 8 字符），前端调用 `POST /api/auth/setup`，请求体 `{ "password": "你的主密码" }`
3. 后端执行：
   - 在 `RG_DATA_DIR` 目录下创建 `app.db`（SQLCipher 加密数据库文件）
   - 生成 16 字节随机盐值，写入 `salt.hex`（十六进制文本）
   - 用主密码 + 盐值通过 Argon2id 派生 32 字节密钥（详见 3.5.3）
   - 以派生密钥打开 SQLCipher 数据库（`PRAGMA key = '<密钥>'`）
   - 执行 `schema::migrate()` 创建全部表和索引（详见 3.5.4）
   - 将连接存入应用状态，标记为已解锁
   - 签发 JWT（`user_id="legacy"`）并返回 token

> 首次设置主密码后，系统会自动签发一个临时 token，但**尚未注册正式用户**。建议立即通过 `POST /api/auth/register` 注册用户以获得持久的 JWT 认证。

#### 3.5.2 后续启动流程（数据库已存在）

1. **检查状态** — `GET /api/auth/state` 返回 `{ initialized: true, unlocked: false }`
2. **解锁数据库** — 调用 `POST /api/auth/unlock`，请求体 `{ "password": "你的主密码" }`
3. 后端执行：
   - 读取 `salt.hex` 获取盐值
   - 用主密码 + 盐值派生密钥
   - 打开 SQLCipher 数据库，执行 `validate_encrypted_db`（查询 `sqlite_master` 验证密钥正确性）
   - 密钥错误时返回 `400 Bad Request`（"主密码不正确"）
   - 执行 `schema::migrate()`（增量迁移，确保老库升级到最新结构）
   - 存入连接，签发 JWT

#### 3.5.3 SQLCipher 加密流程

数据库采用 **SQLCipher** 进行全文件加密，密钥派生使用 **Argon2id** 算法：

```
主密码(用户输入) ──┐
                   ├──► Argon2id ──► 32 字节密钥(十六进制) ──► PRAGMA key
盐值(16字节随机) ──┘                                    │
salt.hex(十六进制文本)                                ▼
                                               SQLCipher AES-256-CBC 加密
```

**Argon2id 参数**（见 `server/src/db/crypto.rs`）：

| 参数 | 值 | 说明 |
|------|-----|------|
| `m_cost`（内存） | 64 MB（65536 KiB） | 密钥派生内存消耗 |
| `t_cost`（迭代） | 3 | 密钥派生轮数 |
| `p_cost`（并行） | 4 | 并行线程数 |
| 输出长度 | 32 字节 | 作为 AES-256 密钥 |

> Argon2id 派生过程计算密集（约需数百毫秒），属正常现象。后端日志会记录 `derive_key_success elapsed_ms=...`。

**数据目录文件说明：**

| 文件 | 说明 | 丢失影响 |
|------|------|---------|
| `app.db` | SQLCipher 加密的 SQLite 数据库 | 丢失全部数据（无法解密恢复） |
| `salt.hex` | 16 字节盐值的十六进制文本 | 即使 `app.db` 存在也无法派生密钥，数据库无法打开 |

> **⚠️ 两个文件缺一不可**。若 `salt.hex` 丢失但 `app.db` 存在，数据库将永久无法解密。

#### 3.5.4 数据库迁移（Append-Only 模式）

系统采用**只增不删**的增量迁移策略（见 `server/src/db/schema.rs`）：

1. **建表阶段** — `migrate()` 函数通过 `CREATE TABLE IF NOT EXISTS` 创建全部表（`persons`、`relationships`、`interactions`、`entity_mentions`、`settings`、`users`）及索引。已存在的表不会重建。

2. **增量列补充** — 多个 `ensure_*_columns` 函数负责老库升级：
   - `ensure_relationship_columns`：为 `relationships` 表补充 `source`、`confidence`、`confirmation_status`、`inference_reason`（v1.3 推断功能）
   - `ensure_relationship_business_columns`：补充 `how_established`、`established_date`、`strength_rating`（v2.0 商业关系扩展）
   - `ensure_person_columns`：为 `persons` 表补充 `school`、`projects`（v1.4 推断规则扩展）
   - `ensure_owner_id_columns`：为 `persons`、`relationships`、`interactions`、`entity_mentions` 补充 `owner_id` 列（多用户支持）

3. **实现原理** — 每个函数先通过 `PRAGMA table_info(<表名>)` 查询已有列，仅对缺失列执行 `ALTER TABLE ... ADD COLUMN`，避免重复添加报错。日志会记录每次新增列操作：`schema_migrate_add_column table=... column=...`。

> **升级安全**：该模式支持从任意历史版本平滑升级，迁移在每次 `setup`/`unlock` 时自动执行，无需手动运行迁移脚本。`ALTER TABLE ADD COLUMN` 是 SQLite 原生支持的兼容操作，不会破坏已有数据。

#### 3.5.5 数据库表结构概览

| 表名 | 用途 | 主要字段 |
|------|------|---------|
| `persons` | 联系人 | id, name, company, title, phone, email, location, school, projects, sensitivity_level, status, owner_id |
| `relationships` | 关系连接 | id, from_person_id, to_person_id, relationship_type, strength, source, confidence, confirmation_status, owner_id |
| `interactions` | 互动记录 | id, person_id, timestamp, content, summary, topics, action_items, owner_id |
| `entity_mentions` | 实体提及 | id, interaction_id, person_id, mention_text, confidence, resolved |
| `settings` | 系统设置 | key, value |
| `users` | 用户账户 | id, username, email, phone, password_hash, oauth_provider, oauth_id, display_name |

### 3.6 API 端点完整列表

后端所有 HTTP 端点均在 `server/src/api/mod.rs` 的 `router()` 函数中定义，分为**公开路由**（无需认证）和**受保护路由**（需 Bearer Token）两类。

#### 3.6.1 公开路由（Public Routes）

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/health` | 健康检查，返回 `{ "status": "ok" }` |
| `GET` | `/api/auth/state` | 查询数据库初始化/解锁状态 |
| `POST` | `/api/auth/setup` | 首次设置主密码，创建加密数据库 |
| `POST` | `/api/auth/unlock` | 用主密码解锁已有数据库 |
| `POST` | `/api/auth/register` | 注册新用户（需先解锁数据库） |
| `POST` | `/api/auth/login` | 用户登录（用户名/邮箱/手机号 + 密码） |
| `POST` | `/api/auth/refresh` | 刷新 Access Token |
| `POST` | `/api/auth/oauth/:provider` | OAuth 第三方登录回调（Mock 模式） |

#### 3.6.2 受保护路由（Protected Routes）

> 以下路由均经过 `require_auth` 中间件，请求头需携带 `Authorization: Bearer <token>`。

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/persons` | 获取联系人列表 |
| `POST` | `/api/persons` | 创建联系人 |
| `GET` | `/api/persons/:id` | 获取单个联系人 |
| `PUT` | `/api/persons/:id` | 更新联系人 |
| `DELETE` | `/api/persons/:id` | 删除联系人 |
| `GET` | `/api/persons/:id/relationships` | 获取该联系人的关系列表 |
| `GET` | `/api/persons/:id/interactions` | 获取该联系人的互动记录 |
| `GET` | `/api/persons-search` | 搜索联系人候选（用于实体解析） |
| `GET` | `/api/relationships` | 获取关系列表 |
| `POST` | `/api/relationships` | 创建关系 |
| `POST` | `/api/relationships/infer` | 触发关系推断 |
| `GET` | `/api/relationships/pending` | 获取待确认的推断关系 |
| `POST` | `/api/relationships/:id/confirmation` | 设置关系确认状态 |
| `POST` | `/api/interactions` | 创建互动记录 |
| `POST` | `/api/entity-mentions` | 创建实体提及 |
| `GET` | `/api/graph` | 获取关系图谱数据（节点 + 边） |
| `POST` | `/api/nlq` | 自然语言查询（单意图） |
| `POST` | `/api/nlq/multi` | 自然语言查询（多意图） |
| `POST` | `/api/nlq/confirm` | 确认 NLQ 草稿结果 |
| `POST` | `/api/import/preview` | 导入预览（Excel 解析） |
| `POST` | `/api/import/persons` | 提交导入（批量创建联系人） |
| `POST` | `/api/voice/transcribe` | 语音转文字（上传音频文件） |
| `POST` | `/api/auth/lock` | 锁定数据库（清除内存连接） |
| `GET` | `/api/auth/me` | 获取当前登录用户信息 |
| `POST` | `/api/admin/reload-keywords` | 热加载 NLQ 关键词配置 |

#### 3.6.3 认证机制

- **JWT 优先**：`require_auth` 中间件优先验证 `Authorization: Bearer <token>` 中的 JWT Access Token
- **旧 Token 兼容**：JWT 验证失败时降级到内存 `TokenStore`（12 小时有效期），向后兼容 `setup`/`unlock` 签发的旧 token
- **用户标识**：JWT 用户通过 `UserId` 提取器传入处理器；旧 token 用户标识为 `"legacy"`

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

#### 5.2.1 基础连通性检查

**检查后端是否在监听：**

```bash
# 方式一：调用健康检查端点
curl -s http://localhost:8790/api/health
# 预期输出: {"status":"ok"}

# 方式二：检查端口监听
ss -tlnp | grep 8790
# 或
lsof -i :8790
```

**检查前端页面是否加载：**

```bash
curl -s -o /dev/null -w "%{http_code}" http://localhost:1420
# 预期输出: 200
```

#### 5.2.2 逐步验证清单

以下是完整的启动验证流程，按顺序执行可确保系统各组件正常工作：

**步骤 1：检查后端健康状态**

```bash
curl -s http://localhost:8790/api/health | jq .
# 预期: {"status":"ok"}
```

**步骤 2：检查数据库初始化状态（首次应为未初始化）**

```bash
curl -s http://localhost:8790/api/auth/state | jq .
# 首次启动预期: {"initialized":false,"unlocked":false}
# 已有数据库预期: {"initialized":true,"unlocked":false}
```

**步骤 3：设置主密码（仅首次）**

```bash
# 仅在 initialized=false 时执行
curl -s -X POST http://localhost:8790/api/auth/setup \
  -H "Content-Type: application/json" \
  -d '{"password":"YourMasterPass123"}' | jq .
# 预期返回: {"token":"<JWT token>"}
```

> 主密码至少 8 个字符。设置后 `app.db` 和 `salt.hex` 会创建在数据目录中。

**步骤 3'：解锁数据库（非首次）**

```bash
# 在 initialized=true 但 unlocked=false 时执行
curl -s -X POST http://localhost:8790/api/auth/unlock \
  -H "Content-Type: application/json" \
  -d '{"password":"YourMasterPass123"}' | jq .
# 预期返回: {"token":"<JWT token>"}
# 密码错误时返回: 400 {"error":"主密码不正确"}
```

**步骤 4：注册用户**

```bash
curl -s -X POST http://localhost:8790/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username":"admin",
    "password":"AdminPass123",
    "email":"admin@example.com",
    "displayName":"管理员"
  }' | jq .
# 预期返回: {"accessToken":"...","refreshToken":"...","user":{...}}
```

> 保存返回的 `accessToken`，后续请求需放入 Authorization 头。用户名至少 2 字符，密码至少 8 字符。

**步骤 5：登录验证**

```bash
curl -s -X POST http://localhost:8790/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"login":"admin","password":"AdminPass123"}' | jq .
# 预期返回: {"accessToken":"...","refreshToken":"...","user":{...}}
# login 字段可为用户名、邮箱或手机号
```

**步骤 6：验证当前用户（Token 有效性）**

```bash
TOKEN="<上一步返回的 accessToken>"

curl -s http://localhost:8790/api/auth/me \
  -H "Authorization: Bearer $TOKEN" | jq .
# 预期返回: {"id":"...","username":"admin","email":"admin@example.com",...}
# Token 无效时返回: 401 {"error":"未登录或会话已过期"}
```

**步骤 7：创建联系人验证**

```bash
curl -s -X POST http://localhost:8790/api/persons \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "name":"张三",
    "company":"示例科技",
    "title":"产品经理",
    "phone":"13800138000",
    "email":"zhangsan@example.com"
  }' | jq .
# 预期返回: {"id":"...","name":"张三",...}
```

**步骤 8：查询关系图谱验证**

```bash
curl -s http://localhost:8790/api/graph \
  -H "Authorization: Bearer $TOKEN" | jq .
# 预期返回: {"nodes":[...],"edges":[...]}
```

**步骤 9：自然语言查询验证**

```bash
curl -s -X POST http://localhost:8790/api/nlq \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"query":"新认识了李四，在腾讯做技术总监"}' | jq .
# 预期返回: NLQ 解析结果（意图 + 提取字段）
# 即使未配置 LLM，fallback 模式也会返回规则提取结果
```

**步骤 10：验证 Ollama LLM 连接（可选）**

```bash
# 确认 Ollama 服务可用
curl -s http://localhost:11434/api/tags | jq '.models[].name'
# 预期: 包含 "qwen2.5:7b"

# 查看 NLQ 调用日志（另开终端）
RUST_LOG=llm=debug cargo run 2>&1 | grep "llm"
```

#### 5.2.3 默认凭据说明

本系统**没有预置默认账号**，首次使用流程如下：

1. **设置主密码**（`POST /api/auth/setup`）— 用于 SQLCipher 数据库加密，与用户登录无关
2. **注册用户**（`POST /api/auth/register`）— 创建可登录的用户账户，用户名和密码自定义
3. **登录**（`POST /api/auth/login`）— 用注册时设置的凭据登录获取 JWT

> 主密码和用户密码是两个独立的密码：
> - **主密码**：用于数据库加密/解锁，丢失后**数据不可恢复**
> - **用户密码**：用于用户登录认证，可通过管理员重置（如果数据库可解锁）

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

### 6.5 系统间通信配置

系统涉及多个组件间的通信，理解数据流向有助于排查网络问题。

#### 6.5.1 前端 → 后端（API 调用）

前端通过 `src/services/token.ts` 中的 `API_BASE` 变量确定后端地址，采用**自动检测机制**：

```typescript
// src/services/token.ts
export const API_BASE: string =
  (import.meta.env.VITE_API_BASE as string | undefined) ??
  `http://${window.location.hostname}:8790`;
```

**检测优先级：**

1. 若设置了环境变量 `VITE_API_BASE`（构建时注入），使用该值
2. 否则使用 `http://<当前页面hostname>:8790`（运行时动态获取）

**这意味着：**

| 访问方式 | 页面 hostname | API_BASE 自动值 |
|---------|-------------|----------------|
| `http://localhost:1420` | localhost | `http://localhost:8790` |
| `http://192.168.1.100:1420` | 192.168.1.100 | `http://192.168.1.100:8790` |
| `https://app.example.com` | app.example.com | `http://app.example.com:8790` |

> **同域部署提示**：若通过 Nginx 反向代理将前后端部署在同一域名下（见 9.2 节），可将 API 请求代理到后端，此时 `API_BASE` 可留空使用默认值，或设为空字符串走相对路径。

**Token 携带方式：** 前端 `api()` 函数自动从 `localStorage` 读取 Access Token，附加到请求头 `Authorization: Bearer <token>`。Token 过期时自动调用 `/api/auth/refresh` 刷新，刷新失败则清除 Token 并提示重新登录。

#### 6.5.2 后端 → Ollama（本地 LLM）

当 `RG_LLM_PROVIDER` 包含 `ollama` 时，后端通过 HTTP 调用本地 Ollama 服务：

```
POST {RG_OLLAMA_URL}/api/generate
Content-Type: application/json

{
  "model": "qwen2.5:7b",
  "prompt": "<提取指令 + 用户输入>",
  "stream": false,
  "options": { "temperature": 0.1 }
}
```

- **默认地址**：`http://localhost:11434`（由 `RG_OLLAMA_URL` 控制）
- **请求超时**：由 `RG_OLLAMA_TIMEOUT_SECS`（默认 10 秒）控制 reqwest 客户端层；降级链另有 30 秒硬超时
- **连接验证**：`curl http://localhost:11434/api/tags`

#### 6.5.3 后端 → OpenAI（云端 LLM）

当 `RG_LLM_PROVIDER` 包含 `openai` 时，后端通过 HTTPS 调用 OpenAI 兼容 API：

```
POST {RG_OPENAI_BASE_URL}/chat/completions
Authorization: Bearer {RG_OPENAI_API_KEY}
Content-Type: application/json

{
  "model": "gpt-4o-mini",
  "messages": [{"role":"system","content":"..."},{"role":"user","content":"..."}],
  "temperature": 0.1
}
```

- **默认地址**：`https://api.openai.com/v1`（由 `RG_OPENAI_BASE_URL` 控制）
- **完整 URL**：`{base_url}/chat/completions`（如 `https://api.openai.com/v1/chat/completions`）
- **请求超时**：由 `RG_OPENAI_TIMEOUT_SECS`（默认 15 秒）控制
- **兼容服务**：可将 `RG_OPENAI_BASE_URL` 指向 DeepSeek、智谱、Moonshot 等兼容 OpenAI 格式的服务

#### 6.5.4 局域网访问配置

后端监听 `0.0.0.0:8790`，局域网内设备可通过服务器 IP 访问。前端开发服务器同样监听 `0.0.0.0:1420`。

**网络拓扑示例：**

```
┌─────────────┐     HTTP:8790      ┌──────────────┐
│  手机浏览器  │ ──────────────────► │  后端服务器   │
│ (192.168.1.x)│                    │ (192.168.1.100)│
└──────┬──────┘                    └──────┬───────┘
       │ HTTP:1420                        │ HTTP:11434
       ▼                                  ▼
┌─────────────┐                   ┌──────────────┐
│  前端 Dev/   │                   │   Ollama     │
│  静态服务器  │                   │  (可选)      │
└─────────────┘                   └──────────────┘
```

**手机访问前端开发服务器：**

1. 确保手机与服务器在同一局域网（同一 WiFi）
2. 在手机浏览器访问 `http://<服务器IP>:1420`
3. 前端会自动用 `window.location.hostname`（即服务器 IP）构建 API 地址 `http://<服务器IP>:8790`

**防火墙放行端口：**

```bash
# Ubuntu (ufw)
sudo ufw allow 8790/tcp   # 后端
sudo ufw allow 1420/tcp   # 前端开发服务器（仅开发环境）

# 或指定来源 IP 范围（更安全）
sudo ufw allow from 192.168.1.0/24 to any port 8790

# CentOS (firewalld)
sudo firewall-cmd --permanent --add-port=8790/tcp
sudo firewall-cmd --reload
```

> **生产环境**：仅开放 443（HTTPS）端口，前端和后端均通过反向代理对外服务，不直接暴露 8790/1420 端口（见 9.2 节）。

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

### 9.3 数据持久化与备份

#### 9.3.1 数据目录文件说明

数据目录（`RG_DATA_DIR`，默认 `~/.local/share/relationship-graph/`）包含以下文件：

| 文件 | 格式 | 说明 |
|------|------|------|
| `app.db` | SQLCipher 加密的 SQLite 二进制 | 全部业务数据的加密存储，未持有密钥时为不可读的乱码 |
| `salt.hex` | 十六进制文本（32 字符） | Argon2id 密钥派生的盐值，与主密码配合生成数据库密钥 |
| `app.db-journal` | 二进制（临时） | SQLite 回滚日志文件，仅在事务进行中存在，事务结束后自动删除 |

> **注意**：`app.db-journal` 是 SQLite 默认的 rollback journal 模式产物（非 WAL 模式）。事务执行时可能出现该文件，正常结束后自动清理。如需启用 WAL 模式以提升并发读性能，可在 `open_encrypted_db` 后添加 `PRAGMA journal_mode=WAL`，但需注意 SQLCipher 对 WAL 的支持版本要求。

#### 9.3.2 SQLCipher 加密原理

数据库文件在磁盘上始终处于**加密状态**。解密过程：

1. 读取 `salt.hex` 获取 16 字节盐值
2. 将用户输入的主密码与盐值输入 Argon2id 算法（64MB 内存 / 3 轮 / 4 线程）
3. 派生 32 字节密钥（十六进制 64 字符）
4. SQLCipher 使用该密钥进行 AES-256-CBC 加密/解密

**关键特性：**
- 主密码从不存储，仅用于密钥派生
- 无法从 `app.db` 或 `salt.hex` 单独恢复数据 — 两者缺一不可
- 密钥仅在后端进程内存中存在，不写入磁盘

#### 9.3.3 主密码丢失后的恢复

> **⚠️ 不可恢复。** 主密码丢失后，**无法解密数据库，数据将永久丢失**。系统不提供密码重置/找回机制。

原因：主密码仅参与 Argon2id 密钥派生，后端不存储主密码本身，也没有"主密码 → 密钥"的逆向路径。这确保即使数据库文件泄露，攻击者也无法解密。

**唯一选项：重置数据库（数据丢失）**

```bash
# 警告：此操作将永久删除所有数据！
DATA_DIR="${RG_DATA_DIR:-$HOME/.local/share/relationship-graph}"
rm -f "$DATA_DIR/app.db" "$DATA_DIR/salt.hex" "$DATA_DIR/app.db-journal"
# 重新启动后端后，可通过 POST /api/auth/setup 重新设置主密码
```

**因此强烈建议：**
- 妥善备份主密码（密码管理器、离线纸质记录）
- 定期备份 `app.db` + `salt.hex` 文件对（见 9.3.4）

#### 9.3.4 数据备份

备份时需**同时备份 `app.db` 和 `salt.hex`**，单独备份任一文件均无法恢复数据。

**备份脚本：**

```bash
#!/bin/bash
BACKUP_DIR="/var/backups/relationship-graph"
DATA_DIR="${RG_DATA_DIR:-$HOME/.local/share/relationship-graph}"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p "$BACKUP_DIR"

# 备份数据库文件（SQLCipher 加密状态，可直接复制）
cp "$DATA_DIR/app.db" "$BACKUP_DIR/app.db.$DATE"
cp "$DATA_DIR/salt.hex" "$BACKUP_DIR/salt.hex.$DATE"

# 清理 30 天前的备份
find "$BACKUP_DIR" -name "app.db.*" -mtime +30 -delete
find "$BACKUP_DIR" -name "salt.hex.*" -mtime +30 -delete

echo "备份完成: $BACKUP_DIR (app.db.$DATE + salt.hex.$DATE)"
```

> 由于 `app.db` 始终处于加密状态，备份文件可直接复制传输，无需额外加密。但建议将备份存储在安全的离线介质或加密存储中。

**通过 cron 定期执行：**

```bash
# 每天凌晨 2 点备份
0 2 * * * /path/to/backup.sh
```

**恢复备份：**

```bash
# 1. 停止后端服务
sudo systemctl stop relationship-graph

# 2. 用备份文件替换当前数据目录
DATA_DIR="${RG_DATA_DIR:-$HOME/.local/share/relationship-graph}"
cp /var/backups/relationship-graph/app.db.20260801_020000 "$DATA_DIR/app.db"
cp /var/backups/relationship-graph/salt.hex.20260801_020000 "$DATA_DIR/salt.hex"

# 3. 重启服务
sudo systemctl start relationship-graph

# 4. 用备份时对应的主密码解锁数据库
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

### 9.5 Docker 部署

#### 9.5.1 Dockerfile 示例

在项目根目录创建 `Dockerfile`：

```dockerfile
# ===== 构建阶段 =====
FROM rust:1.80-bookworm AS backend-builder

WORKDIR /app/server
COPY server/ ./server/
RUN cd server && cargo build --release

FROM node:20-bookworm AS frontend-builder

WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

# ===== 运行阶段 =====
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

# 后端二进制
COPY --from=backend-builder /app/server/target/release/relationship-graph-server /usr/local/bin/

# 前端静态文件
COPY --from=frontend-builder /app/dist /var/www/relationship-graph/dist

# 数据目录
ENV RG_DATA_DIR=/data
RUN mkdir -p /data

EXPOSE 8790

WORKDIR /app/server
COPY server/config ./config

CMD ["relationship-graph-server"]
```

#### 9.5.2 构建与运行

```bash
# 构建镜像
docker build -t relationship-graph .

# 运行容器（挂载数据目录持久化）
docker run -d \
  --name rg-server \
  -p 8790:8790 \
  -v rg-data:/data \
  -e RG_JWT_SECRET=$(openssl rand -hex 32) \
  -e RG_LLM_PROVIDER=ollama,fallback \
  -e RG_OLLAMA_URL=http://host.docker.internal:11434 \
  relationship-graph:latest

# 查看日志
docker logs -f rg-server

# 进入容器
docker exec -it rg-server bash
```

> **Ollama 访问**：容器内无法直接访问宿主机的 `localhost`。使用 `host.docker.internal`（Docker Desktop）或宿主机 IP（Linux: `--add-host=host.docker.internal:host-gateway`）。

#### 9.5.3 docker-compose.yml（含 Nginx + 后端）

```yaml
version: '3.8'
services:
  backend:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "8790:8790"
    volumes:
      - rg-data:/data
      - ./server/config:/app/server/config:ro
    environment:
      RG_DATA_DIR: /data
      RG_JWT_SECRET: ${RG_JWT_SECRET}
      RG_LLM_PROVIDER: ollama,fallback
      RG_OLLAMA_URL: http://host.docker.internal:11434
      RUST_LOG: info
    restart: unless-stopped

  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./dist:/usr/share/nginx/html:ro
      - ./nginx.conf:/etc/nginx/conf.d/default.conf:ro
      - ./certs:/etc/nginx/certs:ro
    depends_on:
      - backend
    restart: unless-stopped

volumes:
  rg-data:
```

### 9.6 多进程管理

除 systemd 外，也可使用 PM2 或 Supervisor 管理后端进程，实现自动重启和日志收集。

#### 9.6.1 PM2（Node 生态）

```bash
# 全局安装 PM2
sudo npm install -g pm2

# 启动后端（ecosystem 配置文件方式）
cat > ecosystem.config.js << 'EOF'
module.exports = {
  apps: [{
    name: 'rg-backend',
    cwd: '/opt/relationship-graph/server',
    script: 'target/release/relationship-graph-server',
    env: {
      RG_PORT: 8790,
      RG_DATA_DIR: '/var/lib/relationship-graph',
      RG_JWT_SECRET: process.env.RG_JWT_SECRET,
      RG_LLM_PROVIDER: 'ollama,fallback',
      RUST_LOG: 'info'
    },
    max_restarts: 10,
    restart_delay: 3000
  }]
};
EOF

pm2 start ecosystem.config.js
pm2 save              # 保存进程列表
pm2 startup           # 开机自启（按提示执行返回的命令）
```

```bash
# 常用命令
pm2 status            # 查看状态
pm2 logs rg-backend   # 查看日志
pm2 restart rg-backend # 重启
pm2 stop rg-backend   # 停止
```

#### 9.6.2 Supervisor

```bash
sudo apt-get install -y supervisor

sudo tee /etc/supervisor/conf.d/relationship-graph.conf << 'EOF'
[program:relationship-graph]
command=/opt/relationship-graph/server/target/release/relationship-graph-server
directory=/opt/relationship-graph/server
environment=RG_PORT="8790",RG_DATA_DIR="/var/lib/relationship-graph",RG_JWT_SECRET="你的密钥",RUST_LOG="info"
user=relationship-graph
autostart=true
autorestart=true
startretries=3
stderr_logfile=/var/log/supervisor/rg-err.log
stdout_logfile=/var/log/supervisor/rg-out.log
EOF

sudo supervisorctl reread
sudo supervisorctl update
sudo supervisorctl start relationship-graph
```

### 9.7 SSL/TLS 证书（Let's Encrypt）

生产环境必须使用 HTTPS。推荐使用 Let's Encrypt 免费证书 + certbot 自动续期。

#### 9.7.1 使用 certbot 获取证书

```bash
# 安装 certbot
sudo apt-get install -y certbot python3-certbot-nginx

# 获取证书（自动修改 Nginx 配置）
sudo certbot --nginx -d your-domain.com

# 或使用 standalone 模式（需先停止 Nginx）
sudo systemctl stop nginx
sudo certbot certonly --standalone -d your-domain.com
sudo systemctl start nginx
```

#### 9.7.2 自动续期

```bash
# 测试续期流程
sudo certbot renew --dry-run

# certbot 安装后自动添加 systemd timer，每天检查两次
# 查看定时任务
sudo systemctl list-timers | grep certbot
```

#### 9.7.3 Nginx SSL 优化配置

```nginx
server {
    listen 443 ssl http2;
    server_name your-domain.com;

    ssl_certificate     /etc/letsencrypt/live/your-domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;

    # SSL 优化
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 1d;

    # ... 其余 location 配置见 9.2 节
}
```

### 9.8 防火墙配置

生产环境应遵循最小暴露原则，仅开放必要端口。

```bash
# ===== UFW (Ubuntu/Debian) =====
sudo ufw default deny incoming
sudo ufw default allow outgoing

# 仅开放 SSH 和 HTTPS
sudo ufw allow 22/tcp           # SSH
sudo ufw allow 443/tcp          # HTTPS

# 如果需要 HTTP（用于 certbot 验证或重定向）
sudo ufw allow 80/tcp

# 启用防火墙
sudo ufw enable

# 查看状态
sudo ufw status verbose
```

```bash
# ===== firewalld (CentOS/RHEL) =====
sudo firewall-cmd --permanent --add-service=ssh
sudo firewall-cmd --permanent --add-service=https
sudo firewall-cmd --permanent --add-service=http
sudo firewall-cmd --reload
```

> **后端端口 8790 不应直接暴露**。通过 Nginx 反向代理 `/api/` 路径到 `127.0.0.1:8790`，外部无需访问 8790。

### 9.9 性能优化建议

#### 9.9.1 后端优化

- **Release 编译**：生产环境必须使用 `cargo build --release`，相比 debug 版本性能提升数倍
- **JWT 密钥固定**：设置 `RG_JWT_SECRET` 避免每次重启重新生成（减少重启开销）
- **日志级别**：生产环境使用 `RUST_LOG=warn`，减少 I/O 开销；排查时临时调为 `debug`
- **Argon2id 参数**：当前 64MB 内存 + 3 轮已在安全与性能间平衡，不建议在生产环境降低参数

```bash
# 生产编译
cd server && cargo build --release

# 启动时使用优化参数
RUST_LOG=warn RG_JWT_SECRET=<固定密钥> \
  ./target/release/relationship-graph-server
```

#### 9.9.2 前端与静态资源优化

**Nginx gzip + 静态资源缓存：**

```nginx
server {
    # ... SSL 配置 ...

    # gzip 压缩
    gzip on;
    gzip_types text/css application/javascript application/json;
    gzip_min_length 1024;

    # 前端静态文件 + 长缓存
    location /assets/ {
        root /var/www/relationship-graph/dist;
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    # 入口 HTML 不缓存（确保用户获取最新版本）
    location / {
        root /var/www/relationship-graph/dist;
        try_files $uri $uri/ /index.html;
        add_header Cache-Control "no-cache";
    }

    # 后端 API 代理（不缓存）
    location /api/ {
        proxy_pass http://127.0.0.1:8790;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

> Vite 构建产物中 `assets/` 目录下的文件名含内容哈希，可安全设置长期缓存。

#### 9.9.3 LLM 性能

- **Ollama 模型选择**：`qwen2.5:7b`（约 4.7GB）平衡了效果与速度；硬件充足可用更大模型
- **超时配置**：调整 `RG_OLLAMA_TIMEOUT_SECS` / `RG_OPENAI_TIMEOUT_SECS` 匹配模型响应速度
- **降级链**：始终在末尾配置 `fallback`，确保 LLM 不可用时系统仍可用

### 9.10 移动端 / 跨设备访问

#### 9.10.1 局域网内手机访问

**开发环境（直接通过 IP 访问）：**

1. 确保手机与运行服务的机器在同一 WiFi 局域网
2. 查询服务器 IP：
   ```bash
   ip addr show | grep "inet " | grep -v 127.0.0.1
   # 例如: 192.168.1.100
   ```
3. 防火墙放行端口：
   ```bash
   sudo ufw allow from 192.168.1.0/24 to any port 8790
   sudo ufw allow from 192.168.1.0/24 to any port 1420
   ```
4. 手机浏览器访问 `http://192.168.1.100:1420`
5. 前端自动检测 hostname，API 请求指向 `http://192.168.1.100:8790`

**生产环境（通过域名 + HTTPS）：**

1. 配置 Nginx 反向代理（见 9.2 节）
2. 手机浏览器访问 `https://your-domain.com`
3. 前后端同域，`API_BASE` 自动使用当前 hostname

#### 9.10.2 HTTPS 与安全上下文要求

> **重要**：浏览器 Web Speech API（语音输入）、Service Worker（PWA）、Geolocation 等**仅在安全上下文中可用**。

| 访问方式 | 是否安全上下文 | Web Speech API |
|---------|-------------|---------------|
| `http://localhost:1420` | ✅ 是（localhost 豁免） | 可用 |
| `http://192.168.1.100:1420` | ❌ 否 | **不可用** |
| `https://your-domain.com` | ✅ 是 | 可用 |

**结论**：若需在手机端使用语音输入等功能，**必须配置 HTTPS**（见 9.7 节）。纯 HTTP 的局域网 IP 访问仅适用于基本功能测试。

#### 9.10.3 PWA 支持

前端已配置为 PWA（渐进式 Web 应用），支持添加到手机主屏幕：

1. 通过 HTTPS 访问应用
2. 浏览器菜单 → "添加到主屏幕"
3. 添加后可像原生应用一样全屏启动，无需浏览器地址栏

> PWA 的 Service Worker 需要安全上下文（HTTPS 或 localhost），纯 HTTP 局域网访问时 Service Worker 不会注册。
