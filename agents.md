# 个人关系图谱 —— AI 智能体项目摘要

> 供后续 AI 智能体快速理解项目全貌。信息来源：`docs/superpowers/specs/2026-07-30-personal-relationship-graph-design.md`、`docs/superpowers/plans/2026-07-30-personal-relationship-graph-plan.md` 以及当前代码结构。

---

## 1. 项目目标

构建一款**本地优先、加密存储、端侧智能辅助**的个人关系图谱桌面应用 MVP。

核心能力：
- 维护联系人基础信息、关系链路、互动记录和下一步跟进。
- 通过自然语言完成录入、更新和检索。
- 支持语音转文字录入互动记录。
- 以图谱方式可视化人和人之间的关系。
- 按敏感级别对数据进行脱敏展示和访问控制。

长期定位：以 Web/PWA 为优先形态、可扩展 Windows 增强客户端的个人关系图谱系统；v1.3 设计文档提出“远程服务端 + 本地高敏感库”的分级架构，但当前 MVP 先以 Tauri 本地桌面应用闭环。

---

## 2. 核心技术栈

| 层级 | 选型 |
|---|---|
| 前端框架 | React 18 + TypeScript 5 |
| 构建工具 | Vite 5（开发端口固定 `1420`）|
| 样式 | Tailwind CSS 3 |
| 桌面壳 | Tauri 2（Rust 后端 + React 前端）|
| 后端语言 | Rust |
| 本地数据库 | SQLite + SQLCipher（rusqlite `bundled-sqlcipher`）|
| 密钥管理 | 系统密钥链（keyring crate）+ Argon2id 派生 |
| 端侧 LLM | Ollama（默认模型 `qwen2:7b`，HTTP API `localhost:11434`）|
| 语音转文字 | whisper-cli（Whisper.cpp），需本机安装并放置模型 |
| 图谱可视化 | Cytoscape.js |

---

## 3. 项目目录结构

```
relationship-graph/
├── docs/superpowers/specs/      # 设计文档（v1.3）
├── docs/superpowers/plans/      # 实现计划
├── src/                         # 前端源码
│   ├── components/              # React 组件
│   ├── services/                # Tauri 调用层 + Ollama/Whisper 封装
│   ├── types/                   # TypeScript 类型定义
│   ├── App.tsx / main.tsx       # 应用入口
│   └── index.css                # Tailwind 入口 + 组件样式
├── src-tauri/                   # Rust 后端
│   ├── src/
│   │   ├── commands/            # Tauri Command 暴露给前端
│   │   ├── db/                  # SQLite / SQLCipher 数据层
│   │   ├── security/            # 密钥链 + 敏感级别工具
│   │   ├── types.rs             # 共享结构体
│   │   └── main.rs              # Tauri 启动入口
│   ├── Cargo.toml
│   └── tauri.conf.json
└── package.json / vite.config.ts / tsconfig.json / tailwind.config.js
```

---

## 4. 数据模型

当前数据库（`src-tauri/src/db/schema.rs`）已实现以下表：

### 4.1 `persons` —— 联系人
- `id`, `name`, `aliases`（JSON 数组）, `avatar`, `phone`, `email`
- `company`, `title`, `location`, `background`
- `relationship_strength`: `strong` / `medium` / `weak`
- `resource_tags`（JSON 数组）
- `sensitivity_level`: `low` / `medium` / `high`
- `status`: `active` / `follow-up` / `cold`
- `next_step`, `notes`, `created_at`, `updated_at`

### 4.2 `relationships` —— 关系链路
- `id`, `from_person_id`, `to_person_id`
- `relationship_type`: `introduced` / `colleague` / `friend` / `cooperation` / `other`
- `strength`, `description`, `created_at`

### 4.3 `interactions` —— 互动记录
- `id`, `person_id`, `timestamp`, `content`, `summary`
- `topics`（JSON 数组）, `action_items`（JSON 数组）, `created_at`

### 4.4 `entity_mentions` —— 实体提及（歧义确认）
- `id`, `interaction_id`, `person_id`, `mention_text`, `confidence`, `resolved`

### 4.5 `settings` —— 设置
- `key`, `value`；当前用于保存 salt.hex

设计文档 v1.3 还规划了 `projects`、`reminders`、`import_tasks`、`media_assets`、`audit_logs` 等，当前 MVP 尚未实现。

---

## 5. 主要功能模块与当前实现状态

| 功能模块 | 实现状态 | 关键文件 |
|---|---|---|
| 项目初始化 / Tauri 启动 | 已完成 | `src-tauri/src/main.rs`、`tauri.conf.json` |
| 数据库加密（SQLCipher） | 已完成 | `src-tauri/src/db/crypto.rs`、`commands/security.rs` |
| 账号登录 / 密钥文件自动解锁 | 已完成 | `src/components/PasswordGate.tsx`、`services/auth.ts`、`server/src/api/mod.rs` |
| 联系人 CRUD | 已完成 | `src-tauri/src/db/person.rs`、`commands/person.rs`、`components/PersonForm.tsx` |
| 关系链路 CRUD | 已完成 | `src-tauri/src/db/relationship.rs`、`commands/relationship.rs`、`components/RelationshipForm.tsx` |
| 互动记录 CRUD | 已完成 | `src-tauri/src/db/interaction.rs`、`commands/interaction.rs`、`components/InteractionForm.tsx` |
| 实体提及 / 歧义确认 | 已完成 | `src/components/EntityResolver.tsx` |
| 敏感级别控制 / 脱敏展示 | 已完成 | `src/components/SensitivityGuard.tsx`、`security/sensitivity.rs` |
| 关系图谱可视化 | 已完成 | `src/components/GraphView.tsx`、`commands/graph.rs` |
| 自然语言查询（NLQ） | 已完成初版 | `src/components/NaturalLanguageQuery.tsx`、`commands/nlq.rs` |
| 端侧 LLM 信息抽取 | 已完成 | `src/services/ollama.ts` |
| 语音转文字 | 已完成调用封装 | `src/components/VoiceRecorder.tsx`、`services/whisper.ts`、`commands/voice.rs` |
| 单元测试 | 已完成 Rust 侧基础测试 | `src-tauri/src/db/tests.rs`、`commands/nlq.rs` 内测试 |
| 聊天联网搜索 | 已完成（cloud 通道） | `server/src/llm.rs`（百炼 `enable_search` 透传）、`components/ChatView.tsx` |
| 聊天文档上传注入 | 已完成（前端解析） | `server/src/document.rs`、`components/DocumentAttachButton.tsx` |
| 聊天联系人数据工具调用（方案 B） | 已完成（cloud 通道） | `server/src/data_tools.rs`、`server/src/llm.rs`（Agent 工具循环） |

### 5.1 安全与加密

- WSL 服务端（当前形态）已去掉主密码：SQLCipher 密钥为随机 32 字节，存于数据目录 `db.key`（0600 权限），服务端启动即自动解锁，无人工解锁步骤。
- 全新部署 `POST /api/auth/setup` 生成密钥文件、建库并创建 admin 账号；老库用一次性 `POST /api/auth/migrate`（旧主密码）rekey 迁移。
- 用户体系：邀请制注册、用户名密码登录（Argon2id 哈希），内存 Token（12 小时 TTL）。
- 数据库使用 SQLCipher 加密，路径：`~/.local/share/relationship-graph/app.db`（Linux）。
- 历史 Tauri 原型使用主密码 + Argon2id 派生密钥 + 系统密钥链（keyring），已被上述机制取代。

**用户数据隔离（联系人数据私有，非系统共享）**：

- `persons.owner_id` 归属用户；relationships / interactions / entity_mentions 的归属经由 person 外键 JOIN 派生，不加冗余列。
- 所有数据层函数强制携带 `owner_id` 并在 **SQL 层 WHERE 过滤**（而非应用层后置过滤）；NLQ、图谱、关系推断、Agent 数据工具、Excel 导入均已接入。
- 跨用户访问统一表现为「查无此数据」（404「数据不存在或无权访问」），不泄露存在性；越权写入由数据层抛 `InvalidQuery` 拒绝。
- 数据端点从认证中间件提取 `AuthUser` user_id，取不到一律 401；chat 链路无用户会话时禁用数据工具、降级普通聊天。
- 存量迁移：无 `owner_id` 的联系人回填到首个用户（`users ORDER BY created_at LIMIT 1`），幂等；补列必须先于建索引（`idx_persons_owner` 依赖新列）。

### 5.2 敏感级别访问控制

- `low`：正常展示姓名与详情。
- `medium` / `high`：列表中默认显示代称；真实姓名、电话、邮箱等需点击“确认查看”。
- 图谱中高敏感节点使用红色背景、中敏感使用橙色背景区分。
- NLQ 结果中高敏感联系人默认折叠，需二次确认后展示。

### 5.3 自然语言查询（NLQ）

当前实现为**规则驱动的 QueryIntent 中间层**，不直接让 LLM 生成 SQL：
- 解析地名、资源标签、话题、状态、关系强度、最近未联系天数等关键词。
- 使用参数化 SQL 加载候选人，再在内存中匹配和打分。
- 返回联系人代称/姓名、公司职位、关系强度、状态、上次互动摘要、建议下一步。
- 支持示例查询：
  - “谁在上海做地产，和我关系比较近？”
  - “上次聊过融资的人里，还没跟进的有谁？”
  - “这个懂车帝的投标，谁能帮上忙？”
  - “最近3个月没联系但标记了待跟进的人有哪些？”

### 5.4 语音录入

- 前端当前提供音频文件路径输入框，调用 `transcribe_audio`。
- 后端调用本机 `whisper-cli` 转写中文。
- 模型默认路径：`~/.local/share/relationship-graph/models/ggml-base.bin`。
- 转写结果回填到互动记录表单后，可触发 Ollama 提取人物、话题、待办。

### 5.5 端侧 LLM 信息抽取

- 调用本地 Ollama `qwen2:7b` 模型。
- 输入沟通文本，输出 JSON：`persons`、`topics`、`actionItems`、`summary`。
- 失败时有 fallback，返回空字段 + 内容前 80 字摘要。

### 5.6 聊天联网搜索与文档上传（一期）

- **联网搜索**：仅 cloud 通道生效（`RG_LLM_BACKEND=cloud`），通过百炼 `enable_search` 实现；`ChatRequest.webSearch`（camelCase，可选）控制单次请求开关，env `RG_WEB_SEARCH=off` 为全局总闸（默认允许）；SSE 新增 step stage `web_search`。
- **文档上传**：前端解析 txt / md / pdf（pdfjs-dist）/ docx（mammoth），`.doc` 不支持；以 `ChatRequest.documents: [{fileName, content}]` 随请求提交，服务端 `server/src/document.rs` 按 `RG_DOC_CONTEXT_CHARS`（默认 12000 字）预算尾部截断并附 `[文档内容超长已截断]`；prompt 注入顺序为 角色 → 技能 → 文档 → 问题。
- 二期规划见 §8 待办（MCP rmcp 搜索、后端解析端点、来源角标）。

### 5.7 聊天联系人数据工具调用（方案 B，Function Calling）

解决「基于我的数据生成报告/盘点」类请求拿不到真实联系人数据的问题：模型通过工具自主查库，而非编造模拟数据。

- **工具集**（全部只读，`server/src/data_tools.rs`）：`search_contacts`（地点/关键词/强度/状态/N 天未联系，LIMIT 30）、`get_person_detail`（id 或姓名/代称，含背景、关系链、最近互动）、`list_recent_interactions`（默认 30 天，LIMIT 40）。
- **工具循环引擎**（`server/src/llm.rs` `cloud_agent_stream`）：每轮流式调用携 tools；流式 `ToolCall` 事件累积为 pending_calls → 本轮结束后加锁执行工具（查完即释放）→ 追加 assistant/tool 消息进入下一轮；`AGENT_MAX_TOOL_TURNS = 4` 防不收敛。
- **脱敏在工具层源头完成**：medium/high 返回代称（`sensitivity::display_name`），high 额外标记 `realNameHidden:true`；phone/email 永不输出；模型从源头看不到真名。
- **预算**：单次工具输出按 `RG_TOOL_OUTPUT_BUDGET_CHARS`（默认 8000 字符）整包截断并附提示。
- **开关与降级**：仅 cloud 通道生效；env `RG_CHAT_TOOLS=off` 为全局总闸（默认允许）；建流/调用失败自动降级普通聊天，永不阻断；legacy/rig 通道行为与改造前一致。
- **SSE 事件**：新增 step stage `tool_loop`（已启用提示）与 `tool_call`（正在调用工具 X）；thinking_delta/text_delta/done 契约不变，done 的 usage 取末轮。
- **实测基线**（qwen3.7-plus）：tools + enable_thinking 共存正常；流式 + tools 正常；二轮 role:tool 回传正常。未尽项见 §8.2。

### 5.8 聊天会话历史注入（多轮对话）

解决模型每轮“失忆”：chat / chat-stream 两条链路携带会话历史，支持多轮追问。

- **契约**：`ChatRequest.sessionId`（camelCase，可选，`#[serde(default)]`）；缺省（旧客户端仅传 query）保持单轮行为，向后兼容；前端 `stream.ts` / `useChat.ts` 透传当前会话 id。
- **安全**：携带 sessionId 时前置归属校验（`verify_chat_session_owned`），不匹配/不存在一律 404「数据不存在或无权访问」，不泄露存在性；`GET /api/sessions/:id/messages` 同步修复越权读。
- **历史组装**（`resolve_chat_history`，`server/src/api/mod.rs`）：取严格早于请求时刻的最近消息（排除本轮刚落库的 user 消息，`created_at < request_at`）→ 提取最近 `[对话摘要]` system 消息（窗口内优先，否则补查 DB）→ user/assistant 轮次从新到旧按字符预算累加，预算 `RG_CHAT_HISTORY_CHARS`（默认 8000）；最近一条即使单独超预算也保留。
- **注入顺序**：角色+技能+文档 → 摘要 → 最近历史轮次 → 本轮 query；技能/文档仍拼 system。
- **LLM 层 messages 数组化**：cloud（百炼 messages）/ rig（Ollama 经 rig）/ legacy（Ollama 改 `/api/chat`）三通道均已支持；无历史时行为与改造前逐字节一致。工具循环（`cloud_agent_stream`）同步携带历史。
- **降级与隔离**：历史读取失败降级为空历史继续聊天，永不阻断；日志只记元数据；NLQ 链路（联系人管家）不注入历史。
- **压缩竞态防护**：`AppState.compressing_sessions` per-session 内存标记，并发请求同时越过 50 条阈值时只触发一次压缩，其余跳过（消息已落库）。

---

## 6. 架构设计

### 6.1 当前 MVP 架构（Tauri 本地桌面）

```
React 前端（Vite + Tailwind）
    ↓ Tauri invoke
Rust 后端（Tauri Commands）
    ↓ rusqlite
本地 SQLCipher 加密 SQLite
    ↓ 系统密钥链
Argon2id 派生的数据库密钥
```

### 6.2 设计文档 v1.3 目标架构

```
手机 / Web / PWA 客户端  →  HTTPS  →  远程 Linux 服务端（中低敏感数据）
Windows 增强客户端（Tauri） → 本地 SQLCipher 高敏感数据库 + 远程服务端
```

当前代码尚未拆分出远程服务端，但数据层（`db/`）和命令层（`commands/`）结构清晰，后续可向 REST API / Axum 迁移。

---

## 7. 关键文件索引

| 用途 | 文件路径 |
|---|---|
| 设计文档 | `docs/superpowers/specs/2026-07-30-personal-relationship-graph-design.md` |
| 实现计划 | `docs/superpowers/plans/2026-07-30-personal-relationship-graph-plan.md` |
| 前端入口 | `src/main.tsx`、`src/App.tsx` |
| 前端类型 | `src/types/index.ts` |
| 前端服务层 | `src/services/db.ts`、`src/services/ollama.ts`、`src/services/whisper.ts`、`src/services/security.ts` |
| UI 组件 | `src/components/PersonForm.tsx`、`PersonCard.tsx`、`PersonList.tsx`、`RelationshipForm.tsx`、`InteractionForm.tsx`、`EntityResolver.tsx`、`GraphView.tsx`、`NaturalLanguageQuery.tsx`、`PasswordGate.tsx`、`SensitivityGuard.tsx`、`VoiceRecorder.tsx` |
| Rust 入口 | `src-tauri/src/main.rs` |
| Rust 类型 | `src-tauri/src/types.rs` |
| 数据库 Schema | `src-tauri/src/db/schema.rs` |
| 数据层 | `src-tauri/src/db/person.rs`、`relationship.rs`、`interaction.rs`、`crypto.rs` |
| 命令层 | `src-tauri/src/commands/person.rs`、`relationship.rs`、`interaction.rs`、`graph.rs`、`nlq.rs`、`security.rs`、`voice.rs` |
| 安全工具 | `src-tauri/src/security/keychain.rs`、`sensitivity.rs` |
| 测试 | `src-tauri/src/db/tests.rs` |
| 配置 | `package.json`、`vite.config.ts`、`tsconfig.json`、`tailwind.config.js`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` |

---

## 8. 当前 Open 问题 / 后续待办

根据设计文档 v1.3 和实现计划，以下能力尚未实现或待决策：

1. **远程服务端**：当前为纯本地 Tauri 应用；后续需拆分为 Rust Axum / REST API，支持 Web/PWA 多端访问。
2. **分级存储完整落地**：高敏感数据本地加密、中低敏感数据远程服务端存储、字段级加密。
3. **高敏感数据备份/迁移**：换电脑、损坏、重装系统时的恢复方案。
4. **统一自然语言入口 Intent Router**：当前 NLQ 仅用于查询，录入/更新/路径生成仍需通过表单完成。
5. **项目/机会实体**（`projects` 表）：尚未实现。
6. **主动提醒机制**（`reminders` 表）：关系冷却、跟进、机会匹配、激活建议。
7. **批量导入**：CSV / Excel / vCard / 微信聊天记录导入及字段映射、去重合并。
8. **名片 OCR**：PaddleOCR / Tesseract 集成。
9. **关系推断与确认**：基于规则 + LLM 推断潜在关系，用户确认后写入。
10. **项目决策链路径生成**：从“我”到目标人的关系路径搜索与推荐。
11. **审计日志**（`audit_logs` 表）。
12. **语音录入体验优化**：当前需手动输入音频文件路径；理想流程应直接录音并保存临时文件。
13. **多用户 / 组织权限体系**。
14. **Tauri 能力配置**：当前没有 `src-tauri/capabilities/default.json`，运行 `tauri dev` 前需确认 Tauri 2 权限配置。
15. **admin 忘记密码的恢复手段**：去掉主密码后（密钥文件机制），admin 密码忘记即无法登录且无恢复途径；可后续提供基于 `db.key` 密钥文件的 admin 密码重置命令（如服务端本地 CLI）。
16. **联网搜索 + 文档上传二期**：联网搜索切换为 MCP rmcp 实现（替代百炼 `enable_search` 单一通道）；文档解析下沉到后端端点 `/api/docs/parse`（替代当前前端 pdf/docx/txt 解析，支持 `.doc` 等更多格式）；联网回答附搜索来源角标（引用标注）。

### 8.1 LLM 对话功能缺口清单（2026-08-07 审计）

针对引入云端大模型与提升对话体验的差距审计，按优先级分组如下。

**P0 — 引入云端大模型的前置条件（约 6-9 人天）**

1. **Provider 抽象层**：`server/src/llm.rs` 中 9 个函数硬编码 Ollama 私有格式（`/api/generate` + `format:"json"`），无接口分层；需用 rig（Rust Agent 框架，Apache-2.0，统一 API 覆盖 20+ provider）重构传输层，实现 Ollama / OpenAI 兼容双 provider，按场景绑定不同模型。估 3-4 天。
2. **API Key 安全存储**：`settings` 表在 schema 中存在，但服务端无任何读写函数（空壳）；需实现读写 API + SQLCipher 对称加密存储 + admin 配置页。这是接入云端模型的先决条件。估 1.5-2 天。
3. **会话历史注入**（✅ 已实现 2026-08-08，见 §5.8）：~~`/api/chat` 完全无状态~~——ChatRequest 已增 sessionId，chat/chat-stream 均从 DB 组装 messages 数组（摘要 + 字符预算截取），Ollama 已改 `/api/chat` messages 格式。

**P1 — 对话体验核心（约 7-10 人天）**

4. **流式输出全链路**：当前全部 `stream:false`，reqwest 未启用 stream feature；需后端 Axum SSE 端点 + 前端 ReadableStream 增量渲染 + 停止生成按钮（AbortController）。依赖 provider 层。估 3-5 天。
5. **上下文窗口管理**：全服务端 `num_ctx` / `max_tokens` 零命中（Ollama 默认 4096），无输入截断（`profile_qa` 的 generate_profile 把全部历史一次性拼入，有超窗隐患）；需设置 `num_ctx` 8192-16384、`max_tokens` 上限、历史注入 token 预算。估 1 天。
6. **重试退避与失败显性提示**：无 retry / backoff；抽取失败静默降级为 confidence=0 的空草稿；需指数退避重试（超时 / 5xx 重试 1-2 次）、JSON 解析失败带错误信息纠错重试 1 次、失败时明确报错、前端错误气泡加重试按钮。估 2-3 天。
7. **按场景模型配置与 token 计量**：当前仅环境变量全局单一模型；需模型配置表 + 按场景绑定（chat / extract / summarize）+ admin 配置 + 记录 provider / model / 耗时 / token usage 元数据（不落内容）。依赖 provider 层与 API Key 存储。估 1-2 天。

**P2 — 锦上添花（约 4-6 人天）**

8. **消息重新生成与编辑重发**：依赖流式输出。估 2-3 天。
9. **Markdown 渲染升级**：自研 `MarkdownContent.tsx` 不支持表格 / 图片 / 语法高亮，而大模型高频输出表格；换 react-markdown + remark-gfm。估 0.5 天。
10. **压缩竞态防护**（✅ 已实现 2026-08-08，见 §5.8）：~~两个并发请求同时越过 50 条压缩阈值会重复压缩~~——已加 `AppState.compressing_sessions` per-session 内存标记。
11. **reqwest Client 单例化**：当前每次 LLM 调用新建 TCP 连接，应将 Client 放入 AppState 复用。估 0.5 天。

**已达标项（无需整改）**

- 隐私日志合规：只记录元数据，不落对话内容。
- 并发锁模式基本正确：LLM 调用前已释放 DB 锁。
- 50 条上下文压缩机制已实现。

### 8.2 方案 B（联系人数据工具调用）未尽实现项（2026-08-08）

当前落地的是只读工具 + cloud 通道的最小闭环，以下能力尚未实现：

1. **写工具 / 草稿链路**：三个工具均只读，模型不能经工具新增/修改联系人、录入互动；若后续支持写工具，必须复用 NLQ 草稿 + 用户确认机制（§10 设计原则），禁止工具直写库。
2. **会话历史注入**（✅ 已实现 2026-08-08）：~~工具循环当前为单轮~~——`cloud_agent_stream` 已携带摘要 + 历史轮次（与 §5.8 同源实现）。
3. **rig / legacy 通道工具支持**：工具仅 cloud 通道生效，本地 Ollama 通道无工具能力（qwen2 系列原生 tools 支持弱，短期不做）。
4. **工具调用详情 UI**：前端仅展示 step 文案，未展示工具名/参数/结果摘要与多轮思考过程；后续可把 ToolCall 事件扩展为携带参数的结构化 step。
5. **大规模数据分页**：`search_contacts` LIMIT 30 + 整包 8000 字截断，千级联系人全量盘点仍可能截断；未支持 offset/分页参数与聚合统计类工具（如 count_by_location）。
6. **并行工具调用**：引擎支持单轮多个 ToolCall，但百炼 `parallel_tool_calls` 场景未专项实测。
7. **中间轮 usage 计量**：仅保留末轮 Final 的 usage，多轮累计 token 消耗未统计（与 §8.1 P1-7 token 计量同源）。
8. **同名歧义**：`get_person_detail` 按姓名搜索时取第一个候选人，同名时可能误选；未接入 entity_mentions 歧义确认机制。
9. **工具描述回归基线**：工具 description 措辞直接影响 qwen 工具选择准确率，修改前需实测回归（spike 脚本已删，可用临时实例 + 报告类查询验证）。

---

## 9. 如何运行

```bash
# 1. 安装前端依赖
npm install

# 2. 启动 Tauri 开发模式（需要 Rust 工具链）
npm run tauri dev

# 3. 运行 Rust 测试
cd src-tauri && cargo test
```

语音功能需要：
- 本机安装 `whisper-cli`（Whisper.cpp）。
- 将模型文件放到 `~/.local/share/relationship-graph/models/ggml-base.bin`。

NLQ / 互动记录 AI 提取需要：
- 本机运行 Ollama 服务：`ollama run qwen2:7b`。

---

## 10. 设计原则速览

- **低摩擦维护优先**：主流程是自然语言/语音 → 结构化草稿 → 用户确认；表单仅作兜底和调试。
- **LLM 做解读，不直接做关键写入决策**：LLM 输出草稿，应用层做字段白名单校验，用户确认后写库；NLQ 使用 `QueryIntent` 中间层，禁止直接生成 SQL。
- **分级存储优先**：高敏感本地加密、原则上不上传；中低敏感可上服务端；敏感字段脱敏展示。
- **隐私安全日志**：记录请求元数据、结果数量、敏感级别、二次确认事件；禁止记录密码、token、完整沟通内容、原始名片图片等。

---

## 11. 数字人（@）规则记忆

用于统一首页多数字人路由，避免后续扩展冲突。

### 11.1 注册规则

- 每个数字人必须声明唯一 `id`（英文 snake_case），如 `contact_manager`。
- 每个数字人必须声明唯一 `mention`（如 `@联系人管家`）。
- 可声明 `aliases`，但 alias 不能与其它数字人的 `mention` / `aliases` 重叠。
- 每个数字人必须声明 `routeMode`（当前支持：`relationship` / `chat`）。

### 11.2 解析规则

- 只识别输入开头的第一个 mention：`^\s*@xxx`。
- 本轮只激活一个数字人；多 mention 时以第一个为准。
- 命中数字人后，先移除 mention 前缀，再把正文作为 query。
- mention 后无正文时，不执行 workflow，提示用户补充内容。

### 11.3 路由优先级

`显式 @数字人 > 用户手动选择 > 自动意图分类`

### 11.4 当前默认数字人

- `@联系人管家`（`id=contact_manager`，`routeMode=relationship`）
- `@数字管家` 作为 alias 与其等价。
- 点击头像时自动把 `@联系人管家` 写入首页输入框，行为对齐“微信 @ 某人”。

### 11.5 技能（SKILL）注入范围

- 数字人 SKILL 注入仅作用于 `/api/chat` 与 `/api/chat/stream` 两条链路（请求携带 `agentId` 时注入该数字人的技能文档）。
- 联系人管家（`routeMode=relationship`）走 NLQ 规则链路，其 `extract_*` JSON 抽取不注入技能。

---

## 12. 执行约定记忆

- 每完成一个小功能后，立即执行一次 Git commit。
- 该 commit 流程默认自动进行，不需要再次向用户请示确认。

*最后更新：2026-08-08*
