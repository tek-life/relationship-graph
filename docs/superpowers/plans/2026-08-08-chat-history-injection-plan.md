# 聊天会话历史注入（多轮对话）实施计划

> 创建：2026-08-08。本计划为跨 session 交接文档，包含执行所需的全部背景与决策，新 session 无需额外上下文即可开工。
> 对应 AGENTS.md §8.1 P0-3「会话历史注入」，预估 2-3 人天。

---

## 1. 背景与问题（已核实，2026-08-08）

当前聊天为**单轮失忆**模式：历史消息只存不用。

- 前端 `useChat.ts` 每轮通过 `sessionApi.addMessage()` 持久化消息，切换会话时可回显；
- 但聊天请求体只有 `{ query, agentId?, webSearch?, documents? }`（`src/services/stream.ts` L76-79），**不带 sessionId**；
- 后端 `ChatRequest`（`server/src/types.rs` L5-16）同样无 session 字段；
- 三条 LLM 链路全部单轮：
  - `general_chat_stream` → 单 prompt（角色+技能+文档+本轮问题）；
  - `cloud_agent_stream` → `messages = [system, user]` 固定两条（`server/src/llm.rs` L990 附近）；
  - NLQ（联系人管家）为规则解析单轮文本，**设计上不需要历史，本次不改**。
- 已实现但**未被消费**的配套：`api/session.rs` 的 50 条压缩机制（`COMPRESSION_THRESHOLD=50`，`compress_context` 生成摘要存入 `chat_messages`，保留最近 10 条）——历史注入后该摘要应作为上下文一并注入。

**附带发现的越权漏洞（必须一并修复）**：`GET /api/sessions/:id/messages`（`server/src/api/session.rs` `list_messages`）无归属校验，任意登录用户可读他人会话。

## 2. 设计决策（已定，执行时勿改）

1. **sessionId 上行、后端权威组装**：前端只传 sessionId，历史由后端从 DB 读取组装，不接受前端传历史（防伪造/超长）。
2. **归属校验前置**：chat 链路拿到 session_id 后必须校验 `session.user_id == 当前登录用户`，不匹配按 404「数据不存在或无权访问」处理（与项目数据隔离惯例一致，不泄露存在性）。
3. **历史预算**：新增 env `RG_CHAT_HISTORY_CHARS`，默认 8000 字符；从新到旧累加，超出即止。
4. **摘要注入**：会话中若存在压缩摘要消息，作为 `[对话摘要]` system 段注入，顺序：`角色+技能+文档 → 摘要 → 最近历史轮次 → 本轮 query`。
5. **时序陷阱**：前端是「先 addMessage 落库本轮 user 消息、再发聊天请求」。后端组装历史时**必须排除本轮刚落库的 user 消息**（取历史后若末条为 user 且内容等于本轮 query 则丢弃；或改用 created_at 早于请求时刻），否则本轮问题重复出现两次。
6. **messages 数组化**：cloud 流式/Agent 链路改消息序列（rig openai provider 原生支持）；Ollama 降级路径改 `/api/chat` messages 格式。技能/文档仍拼在 system prompt 内，不逐轮重复。
7. **NLQ 链路不注入历史**。
8. **压缩竞态防护**（顺带修 §8.1 P2-10）：并发请求同时越过 50 条会重复压缩，加「压缩进行中」标记（内存即可，如 SharedState 中的 per-session 标志）。

## 3. 任务清单（按依赖顺序）

| # | 任务 | 关键文件 |
|---|---|---|
| h1 | 契约扩展：`ChatRequest` 加可选 `session_id`（serde camelCase `sessionId`，缺省向后兼容）；`stream.ts` payload 附带 sessionId；`useChat.ts` `runStreamRequest` 透传 | `server/src/types.rs`、`src/services/stream.ts`、`src/hooks/useChat.ts` |
| h2 | 安全：chat/chat-stream handler 校验会话归属；修复 `list_messages` 越权读（加 extract_user_id + 归属校验） | `server/src/api/mod.rs`、`server/src/api/session.rs` |
| h3 | 历史组装（核心）：新增纯函数 `resolve_chat_history(conn, session_id, budget) -> Vec<(role, content)>`，含摘要提取、预算截取、排除本轮消息；chat_handler / chat_stream_handler 接入 | `server/src/api/mod.rs` |
| h4 | LLM 层改 messages 数组：`general_chat_stream` / `cloud_chat_stream` / `cloud_agent_stream`（工具循环 messages = [system, ...history, user]）；Ollama 路径 `/api/chat` | `server/src/llm.rs` |
| h5 | 压缩竞态标记 | `server/src/api/session.rs`、`server/src/state.rs` |
| h6 | 回归单测 + README | 见 §4 |
| h7 | 本地重启验证，**经用户确认后**才发版 | — |

## 4. 测试要求（硬性：功能必须与单测同时交付）

- `resolve_chat_history` 纯函数单测：预算截取（从新到旧）、摘要提取与注入位置、空会话、排除本轮消息；
- 归属校验：跨用户 session_id 返回 404 语义；
- ChatRequest 序列化兼容：无 sessionId 的旧请求正常解析（`#[serde(default)]`）；
- 压缩竞态标记语义；
- 完成后 `cd server && cargo test`（当前基线 69 passed，新测试须全过）+ `npx tsc --noEmit` + `npm run build`。

## 5. 环境与流程约定（务必遵守）

- **云端通道已切换 Token Plan 专属网关**（2026-08-08）：默认 Base URL `https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1`，聊天/联网搜索统一 `qwen3.7-plus`（网关支持 enable_search），抽取模型 `qwen3.6-flash`。Key 在 `~/.config/rg-cloud-api-key`（sk-sp- 前缀，仅该网关生效）。
- **发版流程**：任何修改完成后**不得直接 release**；先 `bash ./scripts/restart-services.sh --build` 本地重启，告知用户修改点，用户在 localhost 测试通过并确认后，才执行 `RG_HOST=pap bash scripts/release.sh`。
- **Git**：每完成一个小功能立即 commit（AGENTS.md §12），无需请示。
- 发版到 ECS 时远端也需要放置同一把 Key（release 不传 Key）。

## 6. 验收标准（供用户测试）

1. 同一会话内多轮追问连贯：先问 A，再问「它/他/刚才那个…怎么样？」，模型能正确指代；
2. 切换会话后上下文不串（新会话不带旧历史）；
3. 联系人管家（@联系人管家）行为不变；
4. 联网搜索、文档上传、Agent 数据工具链路回归正常（思考步骤仍显示 `llm_call model=qwen3.7-plus`）；
5. 历史超预算时长对话不报错（观察截断生效、token 不爆炸）。

## 7. 注意事项

- `server/src/api/mod.rs` 近期有他人改动（skill-packages 路由等，见文件内 `/api/admin/skill-packages` 段），编辑时以当前文件实际内容为准，勿覆盖。
- DB 锁约定：锁内查数 → 立即 drop guard → 再做 LLM 调用（勿跨 await 持锁）。
- 日志脱敏：只记元数据（条数、字符数），不落对话内容。
