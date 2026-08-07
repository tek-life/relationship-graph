# scripts/ 部署与运维脚本说明

本目录包含个人智能 AI 平台的部署、启动、重启、依赖安装与测试辅助脚本。

**当前生产形态**：后端 Axum 服务（端口 8790）+ 前端静态资源（Caddy 8080），
LLM 能力**默认使用阿里云百炼云端模型**（`RG_LLM_BACKEND=cloud`），无需本地 Ollama；
Ollama（localhost:11434）仅本地开发走 legacy/rig 通道时需要。

---

## 目录

- [脚本一览](#脚本一览)
- [dev.sh — 本地开发一键启动](#devsh--本地开发一键启动)
- [start-services.sh — 生产/开发服务启动](#start-servicessh--生产开发服务启动)
- [restart-services.sh — 服务重启](#restart-servicessh--服务重启)
- [deploy.sh — 一键部署](#deploysh--一键部署)
- [阿里云 ECS 部署（本地构建 + 上传）](#阿里云-ecs-部署本地构建--上传)
- [install-system-deps.sh — 系统依赖安装](#install-system-depssh--系统依赖安装)
- [install-ai-deps.sh — AI 依赖安装（Whisper）](#install-ai-depssh--ai-依赖安装whisper)
- [setup-proxy.sh 与 Caddyfile — 代理与反向代理](#setup-proxysh-与-caddyfile--代理与反向代理)
- [数据脚本（.mjs）](#数据脚本mjs)
- [云端模型（阿里百炼）配置](#云端模型阿里百炼配置)
- [本地开发（Ollama）](#本地开发ollama)
- [注意事项](#注意事项)

---

## 脚本一览

| 脚本 | 用途 | 典型场景 |
|---|---|---|
| `dev.sh` | 本地开发一键启动（可选 Ollama + cargo run 后端 + Vite） | 日常开发调试 |
| `start-services.sh` | 启动后端 + 前端（生产/开发模式自动检测），默认云端 LLM | 生产启动、被 restart/deploy 复用 |
| `restart-services.sh` | 按端口动态停掉再拉起，可带 `--build` | 后端代码更新后重启 |
| `deploy.sh` | 全新机器一键部署（依赖→构建→启动→初始化数据库→种子数据） | 阿里云 ECS / WSL 首次部署（服务器上编译） |
| `server-init.sh` | ECS 一次性初始化（运行时依赖 + Caddy + systemd 用户级服务） | 阿里云 ECS 部署方案，在服务器上运行 |
| `release.sh` | 本地构建 → rsync 上传 → 远端重启 → 健康检查 | 阿里云 ECS 日常发版，在开发机运行 |
| `install-system-deps.sh` | apt 安装系统级依赖 | 被 deploy.sh 调用，也可单独执行 |
| `install-ai-deps.sh` | 编译安装 whisper-cli + 下载 Whisper 模型 | 语音转写依赖 |
| `setup-proxy.sh` | 为当前 shell 配置宿主机代理 | 网络受限环境（GitHub/HF 下载） |
| `Caddyfile` | Caddy 反向代理配置（静态资源 + /api 转发 + PWA 缓存策略） | 生产前端服务 |
| `seed-demo-data.mjs` | 通过 HTTP API 导入演示数据 | 部署后初始化演示 |
| `generate-test-data.mjs` | 生成 1000 条脏数据 Excel | 导入功能测试 |
| `e2e-import-test.mjs` | 端到端导入链路验证（指向独立测试实例） | 回归测试 |
| `systemd/relationship-graph.service.example` | 8790 后端 systemd 用户级 unit 模板（本机 WSL 与 ECS 同名复用） | 见[注意事项](#注意事项)第 2 条 |

---

## dev.sh — 本地开发一键启动

**用途**：快速拉起开发环境（Ollama 可选 + `cargo run` 后端 + Vite 前端），不启动 Caddy。

**使用场景**：日常开发调试；后端走 `cargo run`（debug 编译更快），前端走 Vite 热更新（1420）。

**LLM 通道选择**（启动后端前在环境里指定）：

- 本地模型：`RG_LLM_BACKEND=legacy`，需要本机 Ollama（脚本会尝试拉起；未安装/未就绪仅提示、不阻断）
- 云端百炼：`RG_LLM_BACKEND=cloud`，无需 Ollama，服务端自动读取 `~/.config/rg-cloud-api-key`

**示例**：

```bash
# 云端通道开发（推荐，无需 Ollama）
RG_LLM_BACKEND=cloud ./scripts/dev.sh

# 本地模型通道开发（需已安装 Ollama 与 qwen2.5:7b）
RG_LLM_BACKEND=legacy ./scripts/dev.sh

# 不指定则随环境/服务端默认
./scripts/dev.sh
```

**关键端口**：后端 8790、Vite 1420、Ollama 11434（可选）。

---

## start-services.sh — 生产/开发服务启动

**用途**：启动全部常驻服务。自动检测运行模式（存在 `dist/` 目录 → 生产模式，用 Caddy/静态服务；否则开发模式，用 Vite）。

**使用场景**：生产环境启动；也被 `restart-services.sh` 与 `deploy.sh` 复用。

**默认行为（云端模式）**：

- 后端启动时导出 `RG_LLM_BACKEND=${RG_LLM_BACKEND:-cloud}` 与 `RG_SKILL_BUDGET_CHARS=${RG_SKILL_BUDGET_CHARS:-8000}`；
- `RG_CLOUD_BASE_URL` / `RG_CLOUD_CHAT_MODEL` / `RG_CLOUD_EXTRACT_MODEL` / `RG_CLOUD_TIMEOUT_SECS` / `RG_CLOUD_API_KEY` / `RG_LLM_CLOUD_FNS` 如环境已设置则**透传**给服务端（均有服务端默认值，无需必填）；
- **跳过 Ollama** 并提示“生产默认使用百炼云端模型，无需 Ollama”；
- 启动前做云端 API Key 存在性轻量检查（env 或 `~/.config/rg-cloud-api-key`），缺失仅警告不阻断，**不回显 Key 内容**。

**关键环境变量**：

| 变量 | 默认 | 说明 |
|---|---|---|
| `RG_LLM_BACKEND` | `cloud` | LLM 通道：`legacy` / `rig` / `cloud` |
| `RG_SKILL_BUDGET_CHARS` | `8000` | 技能注入字符预算 |
| `RG_USE_OLLAMA` | `0` | 设 `1` 时启动 Ollama（仅本地通道需要） |
| `RG_OLLAMA_TIMEOUT_SECS` | `45` | legacy/rig 通道 Ollama 超时 |
| `RG_OLLAMA_CHAT_TIMEOUT_SECS` | `120` | legacy/rig 通道聊天超时 |

**示例**：

```bash
# 生产默认（云端百炼）
./scripts/start-services.sh

# 回退本地 Ollama
RG_LLM_BACKEND=legacy RG_USE_OLLAMA=1 ./scripts/start-services.sh

# 覆盖云端聊天模型（示例）
RG_CLOUD_CHAT_MODEL=qwen-plus ./scripts/start-services.sh
```

**约定（保持不变）**：后端端口 8790；数据目录 `~/.local/share/relationship-graph`（db.key 密钥文件启动即自动解锁，无主密码流程）。
若 8790 已有进程在跑，脚本不会重复启动（先检测端口占用）。

> **提示**：本机（WSL）与 ECS 的 8790 后端已改用 systemd 用户级服务托管（见[注意事项](#注意事项)第 2 条）。脚本检测到 unit 已安装时，**后端段自动委托 systemctl**（运行中不重复启动、未运行则 `systemctl --user start`），不再自行拉起，避免与 systemd 自动拉起竞争。日志位置：systemd 托管时为 `server/server.log`，脚本裸跑时为 `/tmp/relationship-graph-server.log`。

---

## restart-services.sh — 服务重启

**用法**：

```bash
./scripts/restart-services.sh            # 重启 Axum 后端 + 前端（保留 Ollama）
./scripts/restart-services.sh --all      # 连同 Ollama 一起重启（仅本地通道需要）
./scripts/restart-services.sh --build    # 先 cargo build --release 再重启
```

**说明**：

- 停止逻辑**按端口动态查持有 PID**（`lsof -t -i :PORT -sTCP:LISTEN`），不会误杀其它进程；等待最多 10 秒，超时才 `kill -9`；
- `--build` 时先停服务再编译（避免 cargo 覆盖正在运行的二进制导致旧进程跑已删除文件的故障）；
- 停止后委托 `start-services.sh` 重新拉起并做健康检查，因此 `RG_LLM_BACKEND` 等环境变量同样在此生效（默认 cloud）；
- 数据库使用 db.key 密钥文件自动解锁，重启后无需人工解锁；
- LLM 通道环境变量会继承当前 shell，如需回退：`RG_LLM_BACKEND=legacy RG_USE_OLLAMA=1 ./scripts/restart-services.sh --build`。

> **提示**：后端已由 systemd 用户级服务托管时，本脚本的后端段自动改走 `systemctl --user stop`（停止）+ `start-services.sh` 委托 `systemctl --user start`（拉起），不会与 systemd 自动拉起竞争；日常仅重启后端可直接 `systemctl --user restart relationship-graph`。`--build` 换二进制场景：脚本先 systemctl stop → 编译 → 再由 systemctl start 拉起新二进制。

---

## deploy.sh — 一键部署

**用途**：全新机器（Ubuntu 22.04+，WSL2 或阿里云 ECS）一键部署：系统依赖 → Node.js → Rust → Python venv → AI 依赖（Whisper，Ollama 可选）→ 前端构建 → 后端编译 → 启动服务 → 健康检查 → 初始化加密数据库 → 导入演示数据。

**使用场景**：首次部署到阿里云服务器。**部署前请先配置云端 API Key**（见[云端模型配置](#云端模型阿里百炼配置)）。

**关键参数/环境变量**：

| 变量 | 默认 | 说明 |
|---|---|---|
| `RG_LLM_BACKEND` | `cloud` | LLM 通道；`legacy` 时步骤 5b 才拉取本地模型 |
| `RG_USE_OLLAMA` | `0` | 设 `1` 强制拉取本地 Ollama 模型 |
| `OLLAMA_MODEL` | `qwen2.5:7b` | 本地模型名（仅本地通道） |
| `RG_INIT_PASSWORD` | 交互输入 | admin 账号密码（至少 8 位） |
| `HTTP_PROXY`/`HTTPS_PROXY` | - | 存在时自动配置 npm 代理 |

**示例**：

```bash
cd /home/hfli/personal_ai_workspace
# 先配置云端 Key（只需一次）
echo 'sk-xxx' > ~/.config/rg-cloud-api-key && chmod 600 ~/.config/rg-cloud-api-key
# 一键部署（默认云端模型）
RG_INIT_PASSWORD='你的管理员密码' ./scripts/deploy.sh
```

**注意**：脚本会对已初始化实例改用 admin 登录校验（不会重复 setup）；演示数据导入失败不阻断部署。

> 低配 ECS（2C4G）在服务器上 `cargo build --release` 有 OOM 风险，推荐改用下方[阿里云 ECS 部署（本地构建 + 上传）](#阿里云-ecs-部署本地构建--上传)方案。

---

## 阿里云 ECS 部署（本地构建 + 上传）

**思路**：服务器不装 Rust / Node.js / Ollama，只做运行时。产物在本地（WSL2 Ubuntu 24.04，x86_64 与 ECS 架构一致）编译后 rsync 上传；两个服务用 **systemd 用户级 unit** 托管（崩溃自动拉起、开机自启、发版无需 root）。LLM 走百炼云端（`RG_LLM_BACKEND=cloud`）。

**远端目录布局**：

```
~/relationship-graph/{bin,web,logs,Caddyfile}     # 产物与配置（release.sh 上传）
~/.local/share/relationship-graph/                # SQLCipher 数据库 + db.key（勿动）
~/.config/rg-cloud-api-key                        # 百炼 API Key（chmod 600）
~/.config/systemd/user/relationship-graph.service # Axum 后端（8790）
~/.config/systemd/user/rg-caddy.service           # Caddy 前端 + /api 反代（8080）
```

### 步骤 1：服务器一次性初始化（server-init.sh）

```bash
# 本地把脚本传到服务器并执行
scp scripts/server-init.sh <user>@<ip>:~/ && ssh <user>@<ip> bash server-init.sh

# 服务器上配置百炼 API Key（LLM 功能需要）
ssh <user>@<ip> "mkdir -p ~/.config && printf '%s' 'sk-xxx' > ~/.config/rg-cloud-api-key && chmod 600 ~/.config/rg-cloud-api-key"
```

脚本内容：apt 安装运行时依赖（rsync/tesseract 等）→ 官方仓库安装 Caddy → 生成 `~/relationship-graph/Caddyfile` → 创建并 enable 两个 systemd 用户级服务 → 启用 linger（开机自启）。

### 步骤 2：首次发版 + 初始化数据库（release.sh，在本地运行）

```bash
# 前提：已配置 SSH 免密（ssh-copy-id <user>@<ip>）
RG_HOST=<user>@<ip> ./scripts/release.sh

# 首次部署后初始化数据库（创建 admin）
curl -X POST http://<ip>:8080/api/auth/setup \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"<至少8位密码>"}'
```

### release.sh 参数

| 变量 | 默认 | 说明 |
|---|---|---|
| `RG_HOST` | 必填 | SSH 目标，如 `hfli@47.98.x.x` |
| `RG_SSH_PORT` | `22` | SSH 端口 |
| `RG_REMOTE_DIR` | `relationship-graph` | 远端应用目录（相对 home） |
| `SKIP_BACKEND` / `SKIP_FRONTEND` | `0` | 设 `1` 只发前端 / 只发后端 |

流程：`cargo build --release` → `npm run build` → rsync 上传（二进制先传 `.new` 再原子替换）→ `systemctl --user restart` → curl 健康检查（失败自动打印 journalctl 日志）。

### 日常运维

```bash
RG_HOST=<user>@<ip> ./scripts/release.sh              # 每次改动后发版
ssh <user>@<ip> 'systemctl --user status relationship-graph'
ssh <user>@<ip> 'journalctl --user -u relationship-graph -f'   # 跟踪后端日志
```

**前置条件**：阿里云安全组放行 8080/tcp（如需 HTTPS 另放行 80/443 并备案域名）。

---

## install-system-deps.sh — 系统依赖安装

**用途**：apt 安装构建工具链（build-essential/cmake/git 等）、Tauri 桌面壳编译依赖（保留备用）、OCR 依赖（Tesseract 中文简繁体）。需 sudo 权限。

**示例**：

```bash
./scripts/install-system-deps.sh
```

---

## install-ai-deps.sh — AI 依赖安装（Whisper）

**用途**：编译安装 whisper-cli（whisper.cpp，CMake Release）到 `~/.local/bin`，并下载 Whisper base 中文转写模型（默认 hf-mirror.com 镜像，3 次重试）。语音转写功能的依赖。

**关键参数**：

| 变量 | 默认 | 说明 |
|---|---|---|
| `HF_ENDPOINT` | `https://hf-mirror.com` | 模型下载源；官方源设 `https://huggingface.co` |

**示例**：

```bash
source ./scripts/setup-proxy.sh   # 网络受限时先配代理
./scripts/install-ai-deps.sh
HF_ENDPOINT=https://huggingface.co ./scripts/install-ai-deps.sh   # 切官方源
```

> 说明：LLM 能力生产默认走阿里百炼云端，本脚本**不再负责 Ollama 安装**；本地开发如需 Ollama 请自行安装（见[本地开发（Ollama）](#本地开发ollama)）。

---

## setup-proxy.sh 与 Caddyfile — 代理与反向代理

### setup-proxy.sh

**用途**：WSL/受限网络环境下，把宿主机代理（默认 `7891` 端口）导出到当前 shell（curl/wget/git/npm 通用）。必须用 `source` 执行才会在当前 shell 生效。

```bash
source ./scripts/setup-proxy.sh              # 当前 shell 生效
./scripts/setup-proxy.sh --persist-git       # 额外写入 git 全局代理配置
HOST_IP=192.168.1.10 PROXY_PORT=7890 source ./scripts/setup-proxy.sh   # 自定义
```

### Caddyfile

**用途**：生产前端服务 —— 静态资源 + `/api/*` 反向代理到 8790 + SPA fallback + PWA Service Worker 禁缓存 + 静态资源 immutable 长缓存 + JSON 访问日志。

**可移植性**（环境变量，均有默认值，本机可直接用）：

| 变量 | 默认值 | 说明 |
|---|---|---|
| `RG_WEB_ROOT` | `/home/hfli/personal_ai_workspace/dist` | 静态资源根目录；ECS/迁移场景设为 `~/relationship-graph/web` 即可复用同一份 Caddyfile（与 server-init.sh 生成的 ECS 版 Caddyfile 语义一致：同端口、同反代、同缓存策略） |

**实现细节**：

- admin 端点固定 `localhost:2020`（Caddyfile 全局块）：与 apt 安装的系统级 caddy（占用默认 admin 2019）共存，否则启动失败；caddy 2.6 的 `run` 无 `--admin` flag，只能在全局块设置；
- 访问日志路径由 `start-services.sh` 生成配置时替换：`/var/log/relationship-graph` 可写则用之，否则回退 `/tmp/relationship-graph-caddy-access.log`（不再依赖 sudo 建目录）；
- 监听 `:8080`（即 0.0.0.0），后端 8790 同为 0.0.0.0，满足局域网访问前提（WSL2 NAT 可达性见[注意事项](#注意事项)）。

```bash
# 由 start-services.sh 自动拉起（推荐）；手工运行：
caddy run --config /home/hfli/personal_ai_workspace/scripts/Caddyfile
caddy adapt --config scripts/Caddyfile       # 仅校验配置
RG_WEB_ROOT=~/relationship-graph/web caddy run --config scripts/Caddyfile   # 覆盖静态根
```

监听端口默认 8080（Caddyfile 内 `:8080`）；`start-services.sh` 生产模式会自动拉起。

---

## 数据脚本（.mjs）

| 脚本 | 用途 | 示例 |
|---|---|---|
| `seed-demo-data.mjs` | 通过 HTTP API **只新增**演示联系人/互动/关系（可重复运行，已存在跳过），使首页示例 NLQ 有结果 | `RG_SEED_PASSWORD='<管理员密码>' node scripts/seed-demo-data.mjs [baseUrl]` |
| `generate-test-data.mjs` | 生成 1000 条"手搓风格"脏数据 Excel（空姓名/重复行/格式混乱等），输出到 `test-data/` | `node scripts/generate-test-data.mjs` |
| `e2e-import-test.mjs` | 端到端导入验证（解析→映射→清洗→preview→commit），**默认指向 8791 独立测试实例，勿指向正式库** | `node scripts/e2e-import-test.mjs [baseUrl]` |

> seed 脚本密码仅用于登录换 token，不落盘不打印。

---

## 云端模型（阿里百炼）配置

生产默认 LLM 通道为阿里百炼（DashScope OpenAI 兼容端点）。服务端（`server/src/llm.rs`）实现，脚本不内置任何敏感值。

### 环境变量表

| 变量 | 默认值 | 说明 |
|---|---|---|
| `RG_LLM_BACKEND` | 脚本默认 `cloud` | 通道开关：`legacy`（本地 Ollama）/ `rig`（rig 框架）/ `cloud`（百炼全量）。legacy 下可用 `RG_LLM_CLOUD_FNS` 做函数级灰度 |
| `RG_CLOUD_BASE_URL` | `https://dashscope.aliyuncs.com/compatible-mode/v1` | 兼容端点，一般无需改 |
| `RG_CLOUD_CHAT_MODEL` | `qwen3.7-plus` | 聊天/画像模型（流式开思考，SSE 带 thinking_delta） |
| `RG_CLOUD_EXTRACT_MODEL` | `qwen-flash` | 抽取/压缩模型（关思考 + json_object，低延迟低成本） |
| `RG_CLOUD_TIMEOUT_SECS` | `120` | 云端调用超时（秒） |
| `RG_CLOUD_API_KEY` | 无 | API Key。**优先 env**；缺省由服务端自动读取 `~/.config/rg-cloud-api-key` |
| `RG_SKILL_BUDGET_CHARS` | 脚本默认 `8000` | 技能/画像注入总字符预算 |
| `RG_LLM_CLOUD_FNS` | 无 | 仅 `legacy` 通道有效：按函数灰度切云端（逗号分隔函数名） |

### API Key 配置（二选一）

```bash
# 方式一（推荐）：Key 文件，服务端启动时自动读取（支持 BOM 容错）
echo 'sk-xxx' > ~/.config/rg-cloud-api-key && chmod 600 ~/.config/rg-cloud-api-key

# 方式二：环境变量覆盖（优先级更高），随服务启动命令注入
RG_CLOUD_API_KEY='sk-xxx' ./scripts/start-services.sh
```

### 模型分工

- **聊天/画像/技能注入**：`qwen3.7-plus`，流式输出且开启思考（前端可见 thinking_delta 思考流）；
- **信息抽取/上下文压缩等结构化任务**：`qwen-flash`，关闭思考 + `response_format=json_object`，延迟约 1s、成本最低。

### 回退本地模型（应急）

改环境变量重启即回退，无需改代码：

```bash
RG_LLM_BACKEND=legacy RG_USE_OLLAMA=1 ./scripts/restart-services.sh
```

---

## 本地开发（Ollama）

本地开发若不想消耗云端额度或需离线调试，可走本地模型通道：

```bash
# 1. 安装 Ollama 并拉取默认模型
curl -fsSL https://ollama.com/install.sh | sh
ollama pull qwen2.5:7b

# 2. 以 legacy 通道启动开发环境
RG_LLM_BACKEND=legacy ./scripts/dev.sh
```

- `dev.sh` 会自动尝试拉起 Ollama；**未安装/未就绪仅提示不阻断**；
- legacy 通道默认请求 `http://localhost:11434`，超时由 `RG_OLLAMA_TIMEOUT_SECS`（45s）与 `RG_OLLAMA_CHAT_TIMEOUT_SECS`（120s）控制；
- 也可用 `rig` 通道（rig 框架接本地模型）：`RG_LLM_BACKEND=rig`。

---

## 注意事项

1. **API Key 安全**：严禁把 Key 写进任何脚本、配置或提交到仓库；脚本对 Key 只做存在性检查，绝不打印/回显内容。Key 文件请保持 `chmod 600`。
2. **服务托管方式（本机已迁移 systemd）**：本地/WSL 的 8790 后端已由 **systemd 用户级 unit**（`relationship-graph.service`）托管，进程崩溃 3 秒自动拉起、开机/登录后自启（已开 linger），替代旧的 nohup 裸跑。unit 模板：`scripts/systemd/relationship-graph.service.example`（安装到 `~/.config/systemd/user/relationship-graph.service`，需替换 `__PROJECT_DIR__`）。常用命令：
   ```bash
   systemctl --user status  relationship-graph       # 查看状态
   systemctl --user restart relationship-graph       # 重启（发版换二进制后用这个）
   journalctl --user -u relationship-graph -f        # 跟踪日志（服务日志仍追加写 server/server.log）
   ```
   阿里云 ECS 生产环境用 `server-init.sh` + `release.sh` 方案（同名 unit，自动拉起）；如手工排查端口占用，仍须**动态查端口持有 PID**（`ss -ltnp | grep 8790`）再 kill，勿写死 PID。
3. **数据目录勿动**：`~/.local/share/relationship-graph` 含加密数据库与 db.key 密钥文件，更换目录等同丢失全部数据。
4. **先停后编译**：`restart-services.sh --build` 已遵循"先停服务再 cargo build"，手动操作时也请照此，避免旧进程运行已被覆盖删除的二进制。
5. **沙箱/容器内启动陷阱**：在只读挂载的沙箱里启动服务会命中"数据库只读"错误，请在真实文件系统（沙箱外）启动常驻服务。
6. **测试实例隔离**：`e2e-import-test.mjs` 默认打 8791 测试实例，请勿指向 8790 正式库。
7. **WSL2 局域网访问**：后端 8790 与 Caddy 8080 均监听 0.0.0.0，但 WSL2 默认 NAT 模式下局域网其它设备无法直达 WSL 内端口，需在 Windows 宿主机做端口转发（`netsh interface portproxy`）或在 `.wslconfig` 开启 mirrored 网络；本机（Windows 浏览器）访问 `http://localhost:8080` 始终可用。
8. **系统级 Caddy 共存**：若机器上另有 apt 安装的系统级 caddy（监听 :80、admin 2019），本项目 Caddyfile 已将 admin 错开到 2020，两者可共存；如确认不用系统级 caddy 可 `sudo systemctl disable --now caddy` 释放 80 端口。
