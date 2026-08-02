# Relationship Graph — 智能体交接文档

> 本文档供新环境中的 AI 智能体阅读，以快速了解项目现状并接管开发工作。

## 一、项目概述

- **项目名称**: Relationship Graph（关系图谱）
- **定位**: 个人关系网络管理系统，帮助用户以图谱和通讯录形式管理联系人、关系、互动记录，并通过自然语言查询和 AI 推断提升效率。
- **核心功能**: 联系人 CRUD、关系管理、互动记录、关系推断引擎、图谱可视化（Cytoscape）、拼音分组通讯录视图、NLQ 自然语言查询（多意图）、Excel 批量导入、语音输入、OCR 图片识别、多模态输入框、多主题切换、多用户认证与数据隔离。
- **技术栈概要**:
  - 后端: Rust + Axum 0.7 + rusqlite (SQLCipher 加密) + JSON Web Token
  - 前端: React 18 + TypeScript 5 + Tailwind CSS 3 + Vite 5 + Cytoscape 3
  - 数据库: SQLite (SQLCipher 透明加密)
  - LLM: Ollama (本地) / OpenAI 兼容 API (可选)，带规则降级
- **架构概要**: 前后端分离架构。后端为 Axum HTTP 服务，集中化部署，支持多用户数据隔离（owner_id）。前端为 SPA，通过 Vite 开发服务器代理后端 API。数据库使用 SQLCipher 加密，每个用户的数据通过 owner_id 隔离。支持 JWT 认证（Access 2h / Refresh 30d），向后兼容旧主密码模式。

## 二、当前版本状态

- **总提交数**: 40 个 commit
- **最新 commit hash**: `edef79a` (edef79ac1fd334eda0d3702b7b89b35c03eb5e2a)
- **最新 commit message**: `docs: 添加完整部署与运行指南`
- **编译/构建状态**: `cargo build` + `tsc --noEmit` + `vite build` 均已通过
- **运行方式**: `npm run dev` 一键启动前后端（使用 concurrently 并行启动）
  - 前端开发服务器: Vite，端口 **1420**，监听 0.0.0.0
  - 后端 API 服务: Axum，端口 **8790**，监听 0.0.0.0
- **前端访问地址**: `http://localhost:1420`
- **后端 API 地址**: `http://localhost:8790`

## 三、已完成的功能开发（按时间线）

### 3.1 早期功能（会话之前已完成）

以下功能在本次会话之前已开发完成，为基础功能模块：

- **联系人 CRUD**: 创建/查看/编辑/删除联系人，含姓名、电话、邮箱、公司、职位、城市、备注等字段
- **关系管理**: 创建和管理联系人之间的关系，支持多种关系类型
- **互动记录**: 记录与联系人的互动历史（电话、邮件、会面等）
- **关系推断引擎** (`infer.rs`): 基于规则从现有关系推断潜在关系，生成待确认的推断结果
- **图谱可视化** (`GraphView.tsx`): 使用 Cytoscape.js 渲染关系网络图，支持节点拖拽、缩放、点击查看详情
- **拼音分组通讯录视图** (`PersonList.tsx`): 按拼音首字母分组的网格通讯录布局
- **NLQ 自然语言查询（单意图）** (`nlq.rs`): 解析自然语言查询，识别意图并执行对应数据库操作
- **Excel 导入** (`ImportWizard.tsx` + `api/import.rs`): 支持 .xlsx 文件批量导入联系人，带预览确认
- **语音输入** (`VoiceRecorder.tsx` + `useVoiceInput.ts`): Web Speech API 语音转文字
- **OCR 图片识别** (`ImageOcrButton.tsx`): 使用 Tesseract.js 从图片中提取联系人信息
- **多模态输入框** (`MultimodalQuery.tsx`): 整合文字输入、语音录制、OCR 识别三种输入方式
- **多主题切换** (`ThemeSelector.tsx` + `useTheme.ts`): 浅色/深色/高对比度三种主题
- **NLQ 多意图扩展**: 5 种意图类型（查找联系人/查找关系/记录互动/查询互动历史/查找路径）+ 草稿确认机制 + 路径结果展示

### 3.2 全栈架构升级（本次会话完成，5 个阶段）

#### 阶段一：多用户认证与数据隔离

**目标**: 从单用户主密码模式升级为多用户 JWT 认证 + 数据隔离架构。

**关键改动**:
- **数据库 schema** (`db/schema.rs`): 新增 `users` 表（id, username, password_hash, created_at）；为 `persons`、`relationships`、`interactions` 表添加 `owner_id` 列
- **用户 CRUD** (`db/user.rs`): 用户注册（argon2id 密码哈希）、登录验证、按用户名查询
- **JWT 基础设施** (`security/auth.rs`): `JwtManager` 管理 Access Token (2h) 和 Refresh Token (30d) 的签发与验证；保留旧 `TokenStore` 向后兼容 unlock 流程
- **API 端点**: `POST /api/auth/register`、`POST /api/auth/login`、`POST /api/auth/refresh`、`POST /api/auth/oauth/:provider`（OAuth mock）
- **require_auth 中间件** (`api/mod.rs`): 从 Authorization header 提取 JWT，验证后注入 `UserId` 到请求扩展
- **owner_id 数据隔离** (`db/person.rs`、`db/relationship.rs`、`db/interaction.rs`): 所有 CRUD 操作均按 `owner_id` 过滤，确保用户间数据隔离
- **前端 AuthPage** (`components/AuthPage.tsx`): 登录/注册页面，含第三方登录按钮（微信/钉钉/飞书，当前为 mock）
- **useAuth hook** (`hooks/useAuth.ts`): JWT token 管理、自动刷新、登录状态维护
- **API 拦截器** (`services/api.ts`): HTTP 客户端 401 自动 refresh token
- **Token 存储** (`services/token.ts`): 提取 token 存储逻辑为独立模块
- **向后兼容**: `VITE_LEGACY_AUTH=true` 时启用旧主密码模式

#### 阶段二：LLM Provider 抽象

**目标**: 将 LLM 调用从硬编码 Ollama 升级为可插拔的多 Provider 架构，支持降级链。

**关键改动**:
- **模块目录重构**: 从单文件 `llm.rs` 重构为 `llm/` 模块目录
- **LlmProvider trait** (`llm/mod.rs`): 定义 4 个 async 方法（`infer_relationships`、`parse_nlq`、`generate_response`、`name`），使用 `async-trait`
- **OllamaProvider** (`llm/ollama.rs`): 迁移现有 Ollama 调用逻辑到 trait 实现
- **OpenAiProvider** (`llm/openai.rs`): 支持任意 OpenAI 兼容 API（通过 `RG_OPENAI_BASE_URL` 配置）
- **RuleFallback** (`llm/fallback.rs`): 纯正则规则降级，无需 LLM 服务也能工作
- **FallbackChain** (`llm/chain.rs`): 降级链实现，按优先级依次尝试 Provider，失败自动降级
- **环境变量配置**: `RG_LLM_PROVIDER=ollama,fallback` 指定降级链顺序
- **AppState 集成** (`state.rs`): `llm: Arc<dyn LlmProvider>` 注入到 AppState，所有 handler 通过 state 调用
- **构建函数** (`llm/mod.rs`): `build_llm_chain()` 根据环境变量构建降级链

#### 阶段三：NLQ 意图识别优化

**目标**: 将 NLQ 关键词从硬编码提取为可配置的 JSON 文件，支持热加载和置信度评分。

**关键改动**:
- **关键词配置文件** (`server/config/nlq_keywords.json`): 外部化所有意图识别关键词
- **NlqKeywords 模块** (`nlq_config.rs`): 加载和解析关键词配置，返回 `Arc<NlqKeywords>` 供 AppState 使用
- **置信度评分机制** (`nlq.rs`): `classify_intent` 使用 `NlqKeywords` 进行多关键词匹配，计算置信度分数
- **热加载端点**: `POST /api/admin/reload-keywords` 支持运行时重新加载关键词配置，无需重启服务
- **AppState 集成** (`state.rs`): `nlq_keywords: RwLock<Arc<NlqKeywords>>`，通过 RwLock 实现热加载

#### 阶段四：商业关系类型扩展

**目标**: 扩展关系类型体系以支持商业场景，增加关系强度评分。

**关键改动**:
- **Schema 扩展**: 关系类型从基础的几种扩展到 19 种（含商业关系如合作、竞争、投资等）；新增 7 种建立方式（如介绍、引荐、偶遇等）；新增强度评分字段
- **后端类型** (`types.rs`): 扩展关系类型枚举和建立方式枚举
- **DB 操作层** (`db/relationship.rs`): 适配新的类型字段
- **前端类型** (`types/index.ts`): 扩展 TypeScript 类型定义和常量映射
- **GraphView 边样式** (`GraphView.tsx`): 根据关系类型渲染不同样式的边（颜色、线型、宽度）
- **边点击详情卡片** (`GraphView.tsx`): 点击边显示关系详情卡片
- **RelationshipForm 增强** (`RelationshipForm.tsx`): 表单支持选择 19 种关系类型、7 种建立方式、强度评分

#### 阶段五：图谱优化 + 新用户引导

**目标**: 优化图谱可视化体验，为新用户提供引导流程。

**关键改动**:
- **节点自适应大小** (`GraphView.tsx`): 节点大小按 degree（连接数）自适应，重要节点更大
- **分层渲染** (`GraphView.tsx`): 节点按 degree 分层渲染，提升大量节点时的渲染性能
- **缩放字号动态调整** (`GraphView.tsx`): 节点标签字号随缩放级别动态调整
- **过滤面板** (`GraphFilter.tsx`): 按公司/城市/强度过滤图谱节点
- **适配视窗按钮** (`GraphView.tsx`): 一键适配所有节点到可视区域
- **OnboardingWizard** (`OnboardingWizard.tsx`): 四步引导流程（欢迎 → 创建联系人 → 添加关系 → 图谱介绍），`isOnboardingCompleted()` 判断是否完成
- **App 集成** (`App.tsx`): 首次登录时自动展示引导向导

### 3.3 Bug 修复与改进（本次会话）

- **前端认证字段与后端 camelCase 序列化对齐**: 修复注册/登录请求中 `username`/`password` 字段名与后端 serde 序列化不匹配的问题（commit `bdecdc1`）
- **App.tsx Hooks 顺序违例导致白屏**: 修复 React Hooks 调用顺序问题（条件渲染导致 Hooks 在不同次渲染中数量不一致），登录后白屏（commit `505aef2`）
- **OAuth 按钮 emoji 替换为品牌 SVG 图标**: 将第三方登录按钮中的 emoji 替换为标准品牌 SVG 图标（微信/钉钉/飞书），提升专业度（commit `53521f7`）
- **文案更新（移除"本地优先"）**: 移除前端中"本地优先"相关文案，适配多用户集中化架构（commit `be3a963`）
- **npm run dev 联动启动**: 配置 `concurrently` 实现前后端一键启动，前端和后端日志分别带颜色标记（commit `39eb850`）

### 3.4 文档产出（本次会话）

- **演示文档更新** (`docs/demo/当前版本功能介绍.md` + `.html`): 更新为架构升级后的完整功能介绍（commit `5a0f887`）
- **用户帮助文档** (`public/docs/help/user-help.html`): 面向终端用户的操作指南（commit `c62329d`）
- **OAuth 品牌调研报告** (`docs/research/oauth-brand-and-integration-guide.html`): 微信/钉钉/飞书 OAuth 接入流程和品牌图标调研（commit `d33cf9c`）
- **部署与运行指南** (`docs/deployment-guide.md`): 完整的环境准备、安装、配置、运行、测试指南（commit `edef79a`）

## 四、已知问题与待办事项

### 4.1 已发现但未修复的 Bug

- 无（所有已知 bug 均已修复）

### 4.2 待完成的功能

- **OAuth 第三方登录真实对接**: 微信/钉钉/飞书，当前为 mock 模式（`oauth_callback` handler 返回模拟数据）
- **移动端适配优化**: 图谱视图、导航栏为优先优化项
- **关系推断引擎适配 owner_id 隔离**: `infer.rs` 中的推断逻辑需确保按 owner_id 隔离

### 4.3 技术债务

- **OAuth mock 模式**: `api/mod.rs` 中的 `oauth_callback` 需对接真实 OAuth provider（微信/钉钉/飞书）
- **RG_JWT_SECRET**: 生产环境必须设置为安全随机字符串，当前未设置时使用随机值（重启后 token 失效）
- **git remote 已被移除**: 使用 `git filter-repo` 清理历史后 remote 被移除，需要用户提供 URL 重新添加
- **CORS 配置**: 当前为 `CorsLayer::very_permissive()`（`main.rs` 第 50 行），生产环境需收紧为可信域名白名单并启用 HTTPS
- **screenshots/ 目录**: 包含调试截图（`after-login.png`、`auth-page.png`），应从仓库排除或清理
- **src-tauri/ 目录**: Tauri 桌面端配置和代码仍存在但未维护，服务端已迁移为 Axum HTTP 模式

### 4.4 设计文档中的 Open 问题

- **跨设备数据同步方案**: 受限于"数据不出本地"约束，多设备同步方案待设计（见 `docs/superpowers/specs/` 中的设计文档）
- **服务端 LLM 部署 vs 本地隐私保护**: 使用服务端 LLM（如 OpenAI）会将查询内容发送到外部，与隐私保护目标冲突，需权衡

## 五、关键文件索引

### 后端 (server/src/)

| 文件 | 说明 |
|------|------|
| [main.rs](server/src/main.rs) | 入口，环境变量读取（RG_PORT/RG_DATA_DIR/RG_JWT_SECRET），启动 HTTP 服务 |
| [state.rs](server/src/state.rs) | AppState 结构体（db, tokens, jwt, data_dir, nlq_keywords, llm）|
| [api/mod.rs](server/src/api/mod.rs) | 路由定义 + require_auth 中间件 + 所有 HTTP handler |
| [api/import.rs](server/src/api/import.rs) | Excel 导入 API（preview + commit）|
| [api/voice.rs](server/src/api/voice.rs) | 语音转写 API |
| [db/schema.rs](server/src/db/schema.rs) | 数据库迁移（users 表 + owner_id 列）|
| [db/user.rs](server/src/db/user.rs) | 用户 CRUD（argon2id 密码哈希）|
| [db/person.rs](server/src/db/person.rs) | 联系人 CRUD（含 owner_id 隔离）|
| [db/relationship.rs](server/src/db/relationship.rs) | 关系 CRUD（含 owner_id 隔离）|
| [db/interaction.rs](server/src/db/interaction.rs) | 互动记录 CRUD（含 owner_id 隔离）|
| [db/crypto.rs](server/src/db/crypto.rs) | SQLCipher 加密/解密（密钥派生、salt 生成）|
| [security/auth.rs](server/src/security/auth.rs) | JWT 管理器（Access/Refresh）+ 旧 TokenStore |
| [security/sensitivity.rs](server/src/security/sensitivity.rs) | 敏感信息检测（日志脱敏）|
| [llm/mod.rs](server/src/llm/mod.rs) | LlmProvider trait 定义 + build_llm_chain() |
| [llm/ollama.rs](server/src/llm/ollama.rs) | OllamaProvider 实现 |
| [llm/openai.rs](server/src/llm/openai.rs) | OpenAiProvider 实现（兼容任意 OpenAI API）|
| [llm/fallback.rs](server/src/llm/fallback.rs) | RuleFallback 纯正则规则降级 |
| [llm/chain.rs](server/src/llm/chain.rs) | FallbackChain 降级链实现 |
| [nlq.rs](server/src/nlq.rs) | NLQ 意图分类（置信度评分）+ 查询处理 |
| [nlq_config.rs](server/src/nlq_config.rs) | NLQ 关键词配置加载（NlqKeywords 结构）|
| [infer.rs](server/src/infer.rs) | 关系推断引擎（规则推断 + 待确认机制）|
| [types.rs](server/src/types.rs) | 共享类型定义（请求/响应 DTO、枚举）|

### 前端 (src/)

| 文件 | 说明 |
|------|------|
| [App.tsx](src/App.tsx) | 入口（App 认证守卫 + AppContent 主逻辑 + 导航 + Tab 路由）|
| [components/AuthPage.tsx](src/components/AuthPage.tsx) | 登录/注册页面 + OAuth 品牌按钮 |
| [components/OnboardingWizard.tsx](src/components/OnboardingWizard.tsx) | 新用户四步引导向导 |
| [components/MultimodalQuery.tsx](src/components/MultimodalQuery.tsx) | 多模态输入框（文字/语音/OCR）|
| [components/GraphView.tsx](src/components/GraphView.tsx) | 图谱可视化（Cytoscape）+ 通讯录视图 + 边样式 |
| [components/GraphFilter.tsx](src/components/GraphFilter.tsx) | 图谱过滤面板（公司/城市/强度）|
| [components/PersonList.tsx](src/components/PersonList.tsx) | 联系人列表（拼音分组）|
| [components/PersonDetail.tsx](src/components/PersonDetail.tsx) | 联系人详情卡片 |
| [components/PersonForm.tsx](src/components/PersonForm.tsx) | 联系人表单（创建/编辑）|
| [components/RelationshipForm.tsx](src/components/RelationshipForm.tsx) | 关系表单（19种类型 + 7种建立方式 + 强度）|
| [components/InteractionForm.tsx](src/components/InteractionForm.tsx) | 互动记录表单 |
| [components/DraftConfirmation.tsx](src/components/DraftConfirmation.tsx) | NLQ 草稿确认组件 |
| [components/PathResultDisplay.tsx](src/components/PathResultDisplay.tsx) | NLQ 路径查询结果展示 |
| [components/ImportWizard.tsx](src/components/ImportWizard.tsx) | Excel 导入向导（预览 + 确认）|
| [components/NaturalLanguageQuery.tsx](src/components/NaturalLanguageQuery.tsx) | NLQ 查询界面 |
| [components/NlqResultCard.tsx](src/components/NlqResultCard.tsx) | NLQ 结果卡片 |
| [components/EntityResolver.tsx](src/components/EntityResolver.tsx) | 实体解析组件 |
| [components/SensitivityGuard.tsx](src/components/SensitivityGuard.tsx) | 敏感信息检测提示 |
| [components/ImageOcrButton.tsx](src/components/ImageOcrButton.tsx) | OCR 图片识别按钮 |
| [components/VoiceRecorder.tsx](src/components/VoiceRecorder.tsx) | 语音录制组件 |
| [components/ThemeSelector.tsx](src/components/ThemeSelector.tsx) | 主题选择器（浅色/深色/高对比度）|
| [components/PasswordGate.tsx](src/components/PasswordGate.tsx) | 旧模式密码守卫 |
| [hooks/useAuth.ts](src/hooks/useAuth.ts) | JWT 认证 hook（登录/注册/刷新/登出）|
| [hooks/useTheme.ts](src/hooks/useTheme.ts) | 主题管理 hook |
| [hooks/useVoiceInput.ts](src/hooks/useVoiceInput.ts) | 语音输入 hook（Web Speech API）|
| [services/api.ts](src/services/api.ts) | HTTP API 客户端 + 401 自动刷新 |
| [services/token.ts](src/services/token.ts) | Token 存储管理（localStorage）|
| [services/db.ts](src/services/db.ts) | 数据库操作封装（调用后端 API）|
| [services/ollama.ts](src/services/ollama.ts) | Ollama 直连服务（旧逻辑保留）|
| [services/security.ts](src/services/security.ts) | 前端敏感信息检测 |
| [services/whisper.ts](src/services/whisper.ts) | Whisper 语音服务 |
| [types/index.ts](src/types/index.ts) | TypeScript 类型定义 |

### 配置与文档

| 文件 | 说明 |
|------|------|
| [server/config/nlq_keywords.json](server/config/nlq_keywords.json) | NLQ 关键词配置（可热加载）|
| [docs/deployment-guide.md](docs/deployment-guide.md) | 完整部署与运行指南（902 行）|
| [docs/demo/当前版本功能介绍.md](docs/demo/当前版本功能介绍.md) | 演示文档 Markdown 版 |
| [docs/demo/当前版本功能介绍.html](docs/demo/当前版本功能介绍.html) | 演示文档 HTML 版 |
| [docs/research/oauth-brand-and-integration-guide.html](docs/research/oauth-brand-and-integration-guide.html) | OAuth 品牌调研报告 |
| [public/docs/help/user-help.html](public/docs/help/user-help.html) | 用户帮助文档 |
| [docs/superpowers/specs/2026-07-30-personal-relationship-graph-design.md](docs/superpowers/specs/2026-07-30-personal-relationship-graph-design.md) | 项目设计文档 v1.3 |
| [docs/superpowers/plans/2026-07-30-personal-relationship-graph-plan.md](docs/superpowers/plans/2026-07-30-personal-relationship-graph-plan.md) | 项目开发计划 |
| [package.json](package.json) | 前端依赖和脚本 |
| [server/Cargo.toml](server/Cargo.toml) | 后端依赖（Rust crates）|
| [vite.config.ts](vite.config.ts) | Vite 构建配置（端口 1420，监听 0.0.0.0）|

## 六、环境变量速查

### 后端

| 变量名 | 说明 | 默认值 | 必填 |
|--------|------|--------|------|
| `RG_PORT` | 服务端口 | `8790` | 否 |
| `RG_JWT_SECRET` | JWT 密钥 | 随机生成（重启后失效） | 生产必填 |
| `RG_DATA_DIR` | 数据目录 | `~/.local/share/relationship-graph` | 否 |
| `RG_LLM_PROVIDER` | LLM 降级链（逗号分隔） | `ollama,fallback` | 否 |
| `RG_OLLAMA_URL` | Ollama 服务地址 | `http://localhost:11434` | 否 |
| `RG_OLLAMA_MODEL` | Ollama 模型名 | `qwen2.5:7b` | 否 |
| `RG_OPENAI_API_KEY` | OpenAI API 密钥 | — | 用 OpenAI 时必填 |
| `RG_OPENAI_BASE_URL` | OpenAI 基础 URL | `https://api.openai.com/v1` | 否 |
| `RG_OPENAI_MODEL` | OpenAI 模型名 | `gpt-4o-mini` | 否 |
| `RG_NLQ_KEYWORDS_PATH` | NLQ 关键词配置路径 | `config/nlq_keywords.json` | 否 |
| `RUST_LOG` | 日志级别 | `info` | 否 |

### 前端 (.env.local)

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `VITE_API_BASE` | 后端 API 地址 | `http://<hostname>:8790` |
| `VITE_LEGACY_AUTH` | 启用旧主密码模式 | 未设置（关闭）|

## 七、测试与验证

- **后端测试**: `cd server && cargo test`（18 个测试）
- **前端类型检查**: `npx tsc --noEmit`
- **前端构建**: `npx vite build`
- **一键启动**: `npm run dev`（前端 Vite + 后端 Cargo 并行启动）
- **测试账号**: `demo` / `demo123456`
  - 首次使用需先用主密码 `12345678` 解锁数据库（旧模式），或通过注册页面创建新用户
- **健康检查**: `GET http://localhost:8790/api/health`

## 八、Git 提交约定

- **增量提交**: 每完成一个小功能/子步骤就 `git commit`，保持提交历史清晰
- **commit message 格式**: `type: description`（英文描述为主，中文亦可）
  - `feat`: 新功能
  - `fix`: Bug 修复
  - `docs`: 文档变更
  - `refactor`: 代码重构（不改变功能）
  - `test`: 测试相关
  - `chore`: 构建/配置/杂项
- **不要提交** `server/target/`（已在 `.gitignore` 中）
- **不要提交** `node_modules/`（已在 `.gitignore` 中）
- **git remote 已被移除**: 使用 `git filter-repo` 清理历史后 remote 被移除，需要用户提供 URL 执行 `git remote add origin <url>` 重新添加

## 九、新智能体接管建议

1. **先阅读本文件和** [`docs/deployment-guide.md`](docs/deployment-guide.md) 了解部署流程
2. **验证系统可启动**: 运行 `npm run dev`，访问 `http://localhost:1420` 确认前端加载，`http://localhost:8790/api/health` 确认后端运行
3. **验证编译**: 运行 `cd server && cargo test && cd .. && npx tsc --noEmit && npx vite build`
4. **检查"已知问题与待办事项"**（本文件第四节）确定下一步工作优先级
5. **所有代码修改完成后务必 `git commit`**，遵循第八节的提交约定
6. **修改 NLQ 关键词时**，编辑 `server/config/nlq_keywords.json` 后调用 `POST /api/admin/reload-keywords` 热加载，无需重启
7. **添加新 API 端点时**，在 `server/src/api/mod.rs` 的 `router()` 函数中注册路由，protected 路由需加 `.route_layer(middleware::from_fn_with_state(...))`
8. **添加新前端组件时**，在 `src/App.tsx` 的 AppContent 中集成，遵循现有的 Tab 路由模式
