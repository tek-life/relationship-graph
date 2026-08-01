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
| 主密码 / 密钥链解锁 | 已完成 | `src/components/PasswordGate.tsx`、`services/security.ts`、`security/keychain.rs` |
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

### 5.1 安全与加密

- 首次启动要求设置主密码，使用 Argon2id 派生 32 字节密钥。
- Salt 保存到应用数据目录的 `salt.hex`。
- 派生后的数据库密钥存储到系统密钥链（keyring）。
- 数据库使用 SQLCipher 加密，路径：`~/.local/share/relationship-graph/app.db`（Linux）。
- 启动时优先尝试从密钥链自动解锁，否则要求输入主密码。

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

*最后更新：2026-07-31*
