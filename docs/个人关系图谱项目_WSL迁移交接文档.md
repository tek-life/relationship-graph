# 个人关系图谱项目 · WSL 迁移交接文档

> 用途：把当前项目迁移到 WSL 或新项目时，帮助后续开发快速了解历史背景、设计决策、代码现状、已知问题和下一步工作。  
> 当前说明：本次会话的文件系统中未能直接读取到原 `relationship-graph` 项目目录，本文档基于此前会话记录、已交付设计文档内容和项目历史总结整理。迁移后应以新仓库实际代码为准再做一次核对。

## 1. 项目一句话说明

这是一个“个人关系图谱联系人管理 App”，用于维护联系人资料、认识背景、介绍关系、沟通记录、后续跟进和项目机会，并通过自然语言查询或语音输入快速调用这些关系信息。

典型问题包括：

- “谁在上海做地产，和我关系比较近？”
- “上次聊过融资的人里，还没跟进的有谁？”
- “这个懂车帝的投标，谁能帮上忙？”
- “最近 3 个月没联系但标记了‘待跟进’的人有哪些？”

系统希望返回：匹配联系人、上次互动摘要、当前状态、建议下一步，以及必要时的关系路径。

## 2. 历史需求演进

### 2.1 最初目标

最初目标是做一个本地运行的联系人关系图谱工具，记录：

- 姓名、电话、邮箱。
- 经谁介绍认识。
- 和对方聊过什么。
- 后续多次沟通内容。
- 关系强度、资源标签、当前状态、下一步跟进。

### 2.2 MVP 原型方向

早期选择先做一个 Windows 本地最小原型，技术方向为：

- Tauri + React + TypeScript + Vite。
- Rust Tauri Commands 作为本地后端调用方式。
- SQLite / SQLCipher 本地加密数据库。
- 系统密钥链保存派生密钥。
- Ollama 本地大模型做信息抽取、摘要、标签提取。
- Whisper / whisper.cpp 做语音转文字。
- Cytoscape.js 做关系图谱可视化。

### 2.3 v1.2 / v1.3 设计方向

后续需求升级为“分级存储 + 远程服务端 + 多端访问”：

- 中低敏感数据：远程 Linux 服务端存储，手机和 Windows 都可访问。
- 高敏感数据：原则上本地加密保存，不上传服务端。
- 手机端：优先访问中低敏感数据。
- Windows 增强客户端：既访问远程服务端，也访问本地高敏感数据库。
- 是否允许高敏感数据加密备份、跨设备迁移，目前在设计文档中标记为 Open。

当前最新设计文档版本为 v1.3，核心变化是：

- 从纯本地 Tauri 原型，升级为“远程服务端 + 本地高敏感库”的分级架构。
- 增加统一自然语言入口。
- 明确 LLM 做解读，不直接做关键写入决策。
- 增加项目/机会实体。
- 增加关系推断、用户确认和关系路径生成。
- 增加 CSV / Excel / vCard / 微信聊天记录导入设计。
- 将主动提醒机制提升为 P1。
- 增加 Open 问题章节。
- 增加代码调整影响章节。

## 3. 已有核心文档

原项目中曾整理过以下核心文档。迁移时建议一并带入新项目：

```text
docs/superpowers/specs/需求文档_v1.2.md
docs/superpowers/specs/2026-07-30-personal-relationship-graph-design.md
docs/superpowers/specs/2026-07-30-personal-relationship-graph-design.html
docs/superpowers/plans/2026-07-30-personal-relationship-graph-plan.md
docs/superpowers/plans/2026-07-30-personal-relationship-graph-plan.html
```

其中：

- `需求文档_v1.2.md`：偏产品/业务需求，描述分级存储、远程服务端、手机访问、互动维护方式、导入方式、提醒机制等。
- `2026-07-30-personal-relationship-graph-design.md`：技术设计文档，已升级到 v1.3。
- `2026-07-30-personal-relationship-graph-design.html`：v1.3 设计文档 HTML 版本。
- `2026-07-30-personal-relationship-graph-plan.md`：早期可执行开发计划。
- `2026-07-30-personal-relationship-graph-plan.html`：开发计划 HTML 版本。

## 4. 当前代码原型的历史状态

> 说明：以下是历史会话中完成过的原型状态，迁移到 WSL 后需要以实际仓库重新验证。

### 4.1 前端

历史技术栈：

- React。
- TypeScript。
- Vite。
- Tailwind CSS。
- Cytoscape.js。
- Tauri 前端 API。

历史主要页面和组件包括：

- `App.tsx`：主界面，包含联系人、图谱、AI 查询等 tab。
- `PersonForm.tsx`：联系人录入。
- `PersonCard.tsx`：联系人名片视图。
- `RelationshipForm.tsx`：关系录入。
- `InteractionForm.tsx`：互动记录录入。
- `GraphView.tsx`：关系图谱展示。
- `NaturalLanguageQuery.tsx`：自然语言查询入口。
- `VoiceRecorder.tsx`：语音输入入口。
- `PasswordGate.tsx`：数据库解锁入口。
- `SensitivityGuard.tsx`：敏感信息访问控制。

### 4.2 Rust / Tauri 后端

历史后端形态是 Tauri Commands，不是独立 HTTP Web Service。

历史模块包括：

```text
src-tauri/src/main.rs
src-tauri/src/commands/security.rs
src-tauri/src/commands/person.rs
src-tauri/src/commands/relationship.rs
src-tauri/src/commands/interaction.rs
src-tauri/src/commands/graph.rs
src-tauri/src/commands/nlq.rs
src-tauri/src/commands/voice.rs
src-tauri/src/db/schema.rs
src-tauri/src/db/crypto.rs
src-tauri/src/db/person.rs
src-tauri/src/db/relationship.rs
src-tauri/src/db/interaction.rs
src-tauri/src/security/keychain.rs
src-tauri/src/security/sensitivity.rs
src-tauri/src/types.rs
```

历史 Tauri 配置要点：

```text
beforeDevCommand: npm run dev
beforeBuildCommand: npm run build
devUrl: http://localhost:1420
frontendDist: ../dist
bundle targets: nsis
```

注意：浏览器直接访问 Vite dev server 只能看到前端页面，不能完整调用 Tauri 后端命令。

### 4.3 数据库与安全

历史设计：

- 本地 SQLite / SQLCipher 加密数据库。
- Argon2id 从主密码派生数据库密钥。
- salt 存在本地。
- 派生密钥可保存到系统密钥链。
- 数据库丢失时，若没有主密码/密钥，应无法直接读取敏感信息。

历史功能：

- 初始化加密数据库。
- 手动输入主密码解锁。
- 从 keychain 自动解锁。
- 忘记本机保存密钥。
- 敏感级别：低 / 中 / 高。
- 高敏感数据默认脱敏，高敏感详情查看需要二次确认。

## 5. 已实现或设计过的数据模型

### 5.1 Person 联系人

核心字段：

- `id`
- `name`
- `aliases`
- `phone`
- `email`
- `company`
- `title`
- `location`
- `background`
- `relationship_strength`
- `resource_tags`
- `sensitivity_level`
- `status`
- `next_step`
- `notes`
- `created_at`
- `updated_at`

v1.3 设计建议补充：

- `introduction_channel`
- `introduced_by_person_id`
- `core_resources`
- `my_value_to_them`
- `confidence_level`
- `verification_suggestion`

### 5.2 Relationship 关系

核心字段：

- `id`
- `from_person_id`
- `to_person_id`
- `type`
- `strength`
- `description`
- `created_at`

v1.3 设计建议补充：

- `source`: manual / inferred / imported。
- `confidence`: 推断置信度。
- `confirmation_status`: confirmed / pending / rejected。
- `inference_reason`: 推断依据。

图谱展示建议：

- 实线：已确认关系。
- 虚线：系统推断关系。
- 灰色 / 待确认：需要用户确认的关系。
- 隐藏：用户否认过的推断关系。

### 5.3 Interaction 互动记录

核心字段：

- `id`
- `person_id`
- `timestamp`
- `content`
- `summary`
- `topics`
- `action_items`
- `created_at`

v1.3 设计建议补充：

- `source`: text / audio / image / import / chat。
- `sensitivity_level`: 默认继承联系人敏感级别。

注意：高敏感互动记录是否上传服务端，目前标记为 Open。当前倾向是默认本地保存，不上传。

### 5.4 EntityMention 实体提及

用于处理“老张”“王总”等模糊称呼。

核心字段：

- `id`
- `interaction_id`
- `person_id`
- `mention_text`
- `confidence`
- `resolved`

如果匹配到多个候选人，需要用户确认。

### 5.5 Project / Opportunity 项目机会

v1.3 新增设计，用于支持“这个项目谁能帮上忙”“通过谁能联系到目标人”。

建议字段：

- `id`
- `name`
- `description`
- `tags`
- `target_people`
- `related_person_ids`
- `status`
- `sensitivity_level`
- `next_step`
- `created_at`
- `updated_at`

### 5.6 ImportTask 导入任务

用于语音、名片、微信聊天记录、CSV、Excel、vCard 等导入。

建议字段：

- `id`
- `import_type`: audio / business_card / chat_text / csv / excel / vcard / llm_extract。
- `status`: pending / running / succeeded / failed / needs_confirmation。
- `input_asset_id`
- `source_filename`
- `field_mapping`
- `result_json`
- `error_summary`
- `created_at`
- `updated_at`

Excel 字段和数据库字段不匹配时，推荐走字段映射流程：

1. 读取表头和前几行样例。
2. 自动猜测字段映射，例如“手机”“联系电话”映射到 `phone`。
3. 未识别字段让用户选择：自定义字段 / 备注 / 忽略。
4. 用户确认字段映射。
5. 做格式校验和重复检测。
6. 展示导入预览。
7. 用户确认后批量写入。
8. 保存字段映射模板，便于复用。

### 5.7 Reminder 主动提醒

v1.3 中主动提醒被提升为 P1。

提醒类型：

- 关系冷却提醒。
- 跟进提醒。
- 新机会匹配。
- 关系激活建议。

## 6. 自然语言与 LLM 设计原则

### 6.1 统一自然语言入口

长期核心交互不是表单，而是自然语言 / 语音输入。

用户输入一句话后，系统判断意图：

- `create_person`: 新建联系人。
- `update_person`: 修改联系人字段。
- `add_interaction`: 追加互动记录。
- `search_people`: 查询联系人。
- `find_path`: 查找关系路径。
- `create_project`: 新建项目或机会。
- `import_record`: 导入聊天记录或文件。

第一版可以保留表单，但表单定位是：

- 调试和补数据。
- 精确修正 AI 草稿。
- LLM 不可用时兜底。
- 高级编辑。

### 6.2 LLM 的角色

需要 LLM 做：

- 自然语言理解。
- 字段抽取。
- 摘要生成。
- 话题和待办提取。
- 模糊代称识别。
- 名片 OCR 后的结构化整理。
- 查询结果解释。

但 LLM 不应直接做：

- 任意 SQL 生成。
- 未经确认的关键写入。
- 绕过字段白名单或权限控制。

### 6.3 NLQ 查询设计

自然语言查询不应让 LLM 直接转 SQL。

推荐链路：

```text
用户问题
→ QueryIntent 中间表示
→ 字段白名单 / 枚举 / limit / sort 校验
→ 参数化 SQL 或安全查询构造器
→ Rust / 服务层过滤和重排
→ LLM 只做结果解释和摘要
```

历史上已将 NLQ 从“关键词规则直查”升级为 QueryIntent 管线。

已覆盖的示例测试意图：

- “谁在上海做地产，和我关系比较近？”
- “上次聊过融资的人里，还没跟进的有谁？”
- “这个懂车帝的投标，谁能帮上忙？”
- “最近 3 个月没联系但标记了待跟进的人有哪些？”

## 7. 日志可观测性要求

用户明确要求写代码时重视 log 可观测性。

### 7.1 总原则

日志要能帮助排查问题，但不能泄露隐私。

允许记录：

- 请求 ID。
- 用户 ID 或本地会话 ID。
- command / API 名称。
- 状态码或结果状态。
- 耗时。
- 查询类型。
- 候选数量、结果数量。
- 音频 / 图片大小。
- 任务状态。
- 敏感级别。
- 是否触发二次确认。
- 错误摘要和错误类型。

禁止记录：

- 密码。
- token。
- 数据库密钥。
- 电话。
- 邮箱。
- 完整姓名。
- 完整沟通内容。
- 完整语音转写文本。
- OCR 全文。
- 用户完整自然语言问题。

自然语言问题只记录长度、意图类型和脱敏后的过滤摘要。

### 7.2 历史已补强过日志的模块

历史上已为以下模块补充过结构化日志：

- `security.rs`
- `keychain.rs`
- `crypto.rs`
- `schema.rs`
- `person.rs`
- `relationship.rs`
- `interaction.rs`
- `graph.rs`
- `voice.rs`
- `nlq.rs`
- `ollama.ts`

典型日志 target：

- `security`
- `keychain`
- `crypto`
- `db`
- `person_cmd`
- `relationship_cmd`
- `interaction_cmd`
- `graph_cmd`
- `voice_cmd`
- `nlq`
- `ollama`

## 8. 当前已知验证状态

历史验证结果：

- `npm.cmd install` 曾成功。
- `npm.cmd run build` 曾通过，前端 TypeScript / Vite 构建成功。
- 构建时有 chunk 超过 500k 的提示，但不是阻塞错误。
- 当前环境曾缺少 `cargo`，所以 Rust 编译、Rust 单元测试、Tauri dev/build 未完成验证。
- 当前不是独立 Web Service，而是 Tauri Commands 架构。
- 普通浏览器访问 Vite dev server 不能完整使用后端功能。

历史建议的完整运行方式：

```powershell
cd c:\Users\Haifeng\Documents\SystemDebug\relationship-graph
npm.cmd run tauri dev
```

但迁移到 WSL 后，应改用 Linux shell 命令，并重新确认 `package.json` 中实际 scripts。

## 9. Web Service 状态

当前历史原型没有实现独立 HTTP Web Service。

已有形态：

```text
React 前端
→ Tauri invoke
→ Rust Tauri Commands
→ 本地加密数据库 / 本地 AI 工具
```

如果迁移到 WSL 后希望手机或浏览器完整访问，需要新增服务端形态，例如：

```text
React / PWA 前端
→ HTTPS / REST API
→ Rust Axum 服务端
→ PostgreSQL / SQLite / SQLCipher
→ Ollama / Whisper / OCR 服务
```

建议迁移后优先把业务逻辑抽到 service 层，使 Tauri Commands 和 HTTP API 可以复用同一套核心逻辑。

## 10. WSL 迁移建议

### 10.1 推荐迁移目标

建议新项目在 WSL 中采用以下方向：

```text
relationship-graph/
├── docs/
├── apps/
│   ├── web/                 # React / Vite / PWA 前端
│   └── desktop/             # 可选 Tauri Windows 增强客户端
├── services/
│   └── api/                 # Rust Axum 或其他后端 API
├── crates/
│   ├── core/                # 业务 service 层
│   ├── db/                  # 数据访问层
│   ├── nlq/                 # QueryIntent / NLQ
│   └── security/            # 加密、敏感级别、审计
└── infra/
    └── docker-compose.yml
```

如果短期只想复用当前原型，也可以先保持 Tauri 项目结构，等 Rust 编译通过后再重构。

### 10.2 WSL 环境准备

在 WSL Ubuntu 中建议安装：

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev cmake nasm
```

安装 Rust：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

安装 Node.js 建议使用 nvm：

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
source ~/.bashrc
nvm install --lts
node --version
npm --version
```

如果继续使用 npm：

```bash
npm install
npm run build
```

如果新项目改用 pnpm：

```bash
corepack enable
corepack prepare pnpm@latest --activate
pnpm install
pnpm build
```

注意：原项目历史上主要使用 `npm.cmd` 绕过 Windows PowerShell 限制；在 WSL 中不需要 `npm.cmd`，使用 `npm` 即可。

### 10.3 WSL 路径建议

不建议把主要代码长期放在 `/mnt/c/...` 下开发，原因是性能和文件监听可能较差。

推荐放在 WSL 原生文件系统：

```bash
mkdir -p ~/projects
cd ~/projects
git clone <repo-url> relationship-graph
cd relationship-graph
```

如果需要从 Windows 当前目录复制：

```bash
mkdir -p ~/projects/relationship-graph
cp -a /mnt/c/Users/Haifeng/Documents/SystemDebug/relationship-graph/. ~/projects/relationship-graph/
```

复制前请确认 Windows 侧项目目录真实存在。

### 10.4 SQLCipher 注意事项

如果继续使用 SQLCipher，需要在 WSL 中确认依赖：

```bash
sudo apt install -y sqlcipher libsqlcipher-dev
```

需要关注：

- Windows 和 WSL/Linux 的 SQLCipher 版本是否一致。
- 加密参数是否一致。
- 数据库文件是否跨平台可读。
- keychain 中保存的密钥不能自动从 Windows 迁到 WSL。

### 10.5 Keychain 注意事项

历史原型使用系统密钥链保存派生密钥。

迁移到 WSL 后需要重新评估：

- WSL 中是否可用 `libsecret` / `gnome-keyring`。
- 是否仍依赖 keyring crate。
- 是否改为主密码每次解锁。
- 是否提供加密导出和恢复流程。

高敏感数据迁移时不要直接把密钥写到配置文件、脚本或日志中。

### 10.6 Tauri 在 WSL 中的注意事项

如果继续跑 Tauri 桌面应用：

- Windows 11 WSLg 支持 Linux GUI，但体验和 Windows 原生 Tauri 不完全一样。
- 如果目标是 Windows 桌面应用，最终仍可能需要 Windows Rust/MSVC 工具链构建安装包。
- 如果目标是 Web/PWA + Linux 服务端，Tauri 可以暂时降级为后续增强客户端。

建议迁移后先做两个分支判断：

1. 继续修当前 Tauri 原型，目标是本地桌面可用。
2. 新建服务端 API，目标是手机和浏览器可用。

## 11. 建议的 WSL 新项目启动顺序

### 阶段 0：迁移资料

- 拷贝 v1.2 需求文档。
- 拷贝 v1.3 设计文档及 HTML。
- 拷贝开发计划及 HTML。
- 拷贝本交接文档。
- 拷贝现有代码仓库。

### 阶段 1：恢复可构建状态

- 安装 Rust、Node、系统依赖。
- 跑 `npm install` 或新包管理器安装。
- 跑 `npm run build` 验证前端。
- 跑 `cargo check` 验证 Rust。
- 跑 `cargo test` 验证 Rust 测试。
- 跑 `npm run tauri dev` 或实际 Tauri dev script。

### 阶段 2：确认产品方向

在动大代码前，先确认：

- 是继续 Tauri 本地原型，还是转成 Web Service。
- 高敏感数据如何备份和迁移。
- 高敏感互动记录是否允许上传服务端。
- Windows 增强客户端是否继续用 Tauri。
- 手机端第一版只访问中低敏感数据是否可接受。

### 阶段 3：抽 service 层

建议把原 Tauri Command 中的业务逻辑抽出来：

```text
commands/*.rs
→ 调用 service 层
→ service 层调用 db / nlq / security
```

后续如果新增 Axum API：

```text
api handlers
→ 调用同一 service 层
```

这样可以减少 Tauri 和 Web Service 两套逻辑分叉。

### 阶段 4：实现服务端 API

如果确认要浏览器和手机完整使用，建议新增：

- `GET /api/health`
- `POST /api/auth/login`
- `GET /api/persons`
- `POST /api/persons`
- `PATCH /api/persons/:id`
- `POST /api/interactions`
- `POST /api/relationships`
- `POST /api/nlq`
- `POST /api/import-tasks`
- `GET /api/import-tasks/:id`

所有接口都要带：

- 认证。
- 访问控制。
- 敏感字段脱敏。
- request_id。
- 隐私安全日志。

## 12. 迁移后第一轮验证清单

在 WSL 中建议依次执行：

```bash
node --version
npm --version
rustc --version
cargo --version
```

前端：

```bash
npm install
npm run build
```

Rust：

```bash
cd src-tauri
cargo check
cargo test
```

Tauri：

```bash
cd ..
npm run tauri dev
```

如果已经改成服务端 API：

```bash
cargo run
curl http://localhost:<port>/api/health
```

注意：具体命令以新项目 `package.json` 和 Cargo workspace 配置为准。

## 13. 已知问题和处理记录

历史中遇到过：

| 问题 | 原因 | 处理方式 |
|---|---|---|
| PowerShell 下 `ls -la` 失败 | PowerShell 不支持 Unix 参数 | 改用专用文件工具或 PowerShell 兼容命令 |
| `npm install` 经 `npm.ps1` 失败 | PowerShell 执行策略/约束语言模式 | 改用 `npm.cmd install` |
| `cargo --version` 失败 | 当前环境未安装 Rust | Rust 侧未能验证，需要迁移后安装工具链 |
| `pandoc` 不存在 | 本机未安装 pandoc | 用 Node 脚本生成 HTML |
| Vite 浏览器可访问但后端不可用 | 当前是 Tauri Commands，不是 Web Service | 完整功能需 Tauri runtime，或新增 HTTP API |
| 日志中曾出现不必要的 key_len | 密钥相关日志风险 | 已删除 key_len，只保留非敏感耗时和状态 |

## 14. Git 历史信息

历史上完成过的提交包括：

- `docs: update relationship graph design to v1.3`
- `docs: sync v1.3 design html`

当时还存在一个未提交的 `package-lock.json` 修改，未被混入文档提交。迁移前建议检查：

```bash
git status --short
git log --oneline -10
```

如果 `package-lock.json` 仍有修改，需要确认是依赖变更还是本地环境导致，再决定是否提交。

## 15. 新开发者快速阅读顺序

建议按以下顺序阅读：

1. 本文档。
2. `docs/superpowers/specs/需求文档_v1.2.md`。
3. `docs/superpowers/specs/2026-07-30-personal-relationship-graph-design.md`。
4. `docs/superpowers/plans/2026-07-30-personal-relationship-graph-plan.md`。
5. `package.json`。
6. `src-tauri/Cargo.toml`。
7. `src-tauri/src/main.rs`。
8. `src-tauri/src/commands/nlq.rs`。
9. `src-tauri/src/commands/security.rs`。
10. `src-tauri/src/db/schema.rs`。
11. `src/App.tsx`。
12. `src/services/ollama.ts`。

如果代码目录已经改成服务端 API 架构，则优先阅读：

1. API 路由入口。
2. service 层。
3. db 层。
4. nlq 层。
5. security / audit 层。

## 16. 建议保留的关键决策

迁移或重构时，建议不要丢掉这些决策：

1. NLQ 不让 LLM 直接生成 SQL，必须走 QueryIntent。
2. 关键写入先生成草稿，用户确认后再保存。
3. 高敏感数据默认本地加密，不上传服务端；备份方案保持 Open。
4. 高敏感互动记录是否上传服务端保持 Open。
5. 日志必须可观测，但不能泄露隐私。
6. 表单只是兜底，长期主交互是自然语言 / 语音。
7. Excel 导入必须支持字段映射、预览、去重，不应字段不匹配就失败。
8. 推断关系必须用户确认，不能默认当成事实。
9. Web/PWA 与 Tauri 不应各写一套业务逻辑，应通过 service 层复用。
10. 手机端第一版优先访问中低敏感数据，高敏感数据跨端访问需要进一步确认。

## 17. 本次会话环境说明

本次整理文档时，当前可见工作区为：

```text
c:\Users\Haifeng\Documents\SystemDebug
```

实际可见文件只有：

```text
System_Debug_问题流转流程.pptx
gen_system_debug_flow_pptx.py
转正答辩 - 李海峰.pptx
```

当前会话未能直接读取到历史项目目录：

```text
c:\Users\Haifeng\Documents\SystemDebug\relationship-graph
```

因此本文档先放在当前工作区根目录。等原项目目录重新可见后，建议移动到：

```text
relationship-graph/docs/superpowers/specs/WSL迁移交接文档.md
```

或新项目根目录：

```text
docs/WSL迁移交接文档.md
```
