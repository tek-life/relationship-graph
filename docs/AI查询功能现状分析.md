# AI 查询功能现状分析

> 撰写日期：2026-07-31
> 依据代码：`server/src/nlq.rs`、`server/src/infer.rs`、`server/src/api/mod.rs`、`server/src/security/sensitivity.rs`、`src/services/ollama.ts`、`src/services/whisper.ts`、`src/components/NaturalLanguageQuery.tsx`、`src/components/SensitivityGuard.tsx`
> 本文所有行号均与撰写时的实际代码核对，仅描述现状，不包含任何代码改动。

---

## 1. 进度总览

| 能力模块 | 状态 | 实现位置 | 说明 |
| --- | --- | --- | --- |
| 自然语言查询（NLQ） | ✅ 已完成 | `server/src/nlq.rs` + `POST /api/nlq` | 纯规则白名单管线，含意图解析、校验、过滤、评分、脱敏，带单元测试 |
| NLQ 前端界面 | ✅ 已完成 | `src/components/NaturalLanguageQuery.tsx` | 查询输入 + 4 个示例问句 + 结果卡片 + 敏感信息守卫 |
| 关系推断 | ✅ 已完成 | `server/src/infer.rs` + `POST /api/relationships/infer` | 三条规则引擎，结果 pending 待用户确认，带单元测试 |
| 互动记录 AI 提取（Ollama） | ⚠️ 已实现但降级运行 | `src/services/ollama.ts` | 代码硬编码 `qwen2:7b`，而环境实际安装的是 `qwen2.5:7b`，模型名不匹配导致请求失败、长期走降级分支（另一任务正在修复） |
| 语音转写（Whisper） | 🚧 进行中（当前为存根） | `src/services/whisper.ts` | 仅抛出提示性错误，服务端 `/api/voice/transcribe` 端点尚未实现，本轮开发补齐 |
| 名片/截图 OCR | ❌ 未启动 | 无 | 代码库中完全缺失，本轮开发补齐 |

---

## 2. AI 集成事实

### 2.1 使用的技术与模型

- **本地 LLM 运行时**：Ollama，运行于本机 11434 端口，前端通过 `http://${window.location.hostname}:11434/api/generate` 直接调用（`ollama.ts` 第 21 行），未经过后端代理。
- **模型**：环境中实际安装的是 **qwen2.5:7b**；但代码中硬编码为 `'qwen2:7b'`（`ollama.ts` 第 18、25 行）。两者名称不一致导致 Ollama 返回错误，`extractFromText` 长期落入 catch 降级分支（第 65–75 行），仅返回原文前 80 字作为摘要、其余字段为空。该问题正在另一任务中修复，修复后 AI 提取才会真正生效。
- **调用方式**：单次非流式生成（`stream: false`），要求 JSON 输出（`format: 'json'`），Prompt 中固定了输出结构（第 28–36 行）。

### 2.2 AI 真正发挥作用的模块：互动记录信息提取

`src/services/ollama.ts` 的 `extractFromText(text)` 是**全项目唯一的 LLM 调用点**，用于从一段沟通记录文本中提取：

| 字段 | 含义 |
| --- | --- |
| `persons` | 人名/称呼提及，附置信度（`{ mention, confidence }`） |
| `topics` | 话题列表 |
| `actionItems` | 待办事项（兼容 `action_items` 蛇形命名，第 49–53 行） |
| `summary` | 一句话摘要 |

提取结果用于互动录入流程中的实体消解（`EntityResolver` → `POST /api/entity-mentions`），LLM 只产出"建议"，落库仍由用户确认。

### 2.3 非 AI 的"类 AI"模块及原因

以下两个模块从产品视角看像 AI 能力，但**实现上不含任何 LLM 调用**：

1. **NLQ 自然语言查询**（`server/src/nlq.rs`）——纯规则 `QueryIntent` 白名单管线。这是**明确的安全决策**：禁止 LLM 直接生成 SQL，自然语言必须先匹配预定义意图模式，再路由到固定的参数化 SQL 逻辑。前端界面也如实标注："后端只执行白名单规则，不让模型直接生成 SQL"（`NaturalLanguageQuery.tsx` 第 36 行）。
2. **关系推断**（`server/src/infer.rs`）——确定性规则引擎，三条规则及置信度（文件头注释第 1–6 行与实现一致）：
   - 同一公司 → 置信度 **0.8**，关系类型 `colleague`（第 28–57 行）；
   - 同一介绍人介绍认识 → 置信度 **0.6**，类型 `other`（第 59–93 行）；
   - 同行业标签 + 同城 → 置信度 **0.5**，类型 `other`（第 95–129 行）。

   防护性限制：单次最多入库 200 条（`MAX_CREATED_PER_RUN`），分组超过 15 人不做两两组合（`MAX_GROUP_SIZE`），已存在关系跳过（`try_create` 第 147–149 行）。**所有推断结果以 `pending` 状态入库，必须由用户逐条确认（confirmed）或否认（rejected）后才视为事实**；被否认的不会重复生成（有单元测试覆盖，第 214–216 行）。

   选择规则而非 LLM 的原因：推断依据（公司、介绍人、标签、城市）是强结构化字段，规则可给出**可解释的推断理由**（如"同一公司：万科"）与稳定置信度，且不引入隐私数据外流与幻觉风险。

---

## 3. AI 查询（NLQ）完整流程

请求入口：`POST /api/nlq`（`api/mod.rs` 第 42 行注册路由，第 472–480 行处理函数），经 Bearer Token 鉴权中间件（`require_auth`，第 97–121 行）后进入 `nlq::natural_language_query`（`nlq.rs` 第 83–127 行）。

### 3.1 流程管线

```
用户输入
  → parse_query_intent   （规则解析为 QueryIntent，nlq.rs 129-185）
  → validate_query_intent（白名单收紧与去重，187-196）
  → load_candidates      （加载候选人，LIMIT 500，198-250）
  → candidate_matches    （多维过滤，252-286）
  → score_candidate      （加权评分，288-330）
  → 按分数降序取 limit 条 → to_result 脱敏（332-355）
  → 前端 NaturalLanguageQuery 展示 + SensitivityGuard 二次确认
```

### 3.2 parse_query_intent：白名单意图解析（第 129–185 行）

对查询字符串做**字典包含匹配**，产出六类过滤条件：

| 维度 | 白名单枚举 |
| --- | --- |
| 城市 `locations` | 上海、北京、深圳、广州、杭州、苏州、南京（第 132 行） |
| 资源标签 `resource_tags` | 地产、政府资源、融资、设计、设计圈、汽车、投标、园区、招商（第 133–136 行） |
| 话题 `topics` | 融资、投标、懂车帝、园区、项目合作、地产、设计、招商（第 137 行） |
| 状态 `statuses` | "待跟进/没跟进/未跟进/还没跟进/该联系" → `follow-up`；"活跃" → `active`；"冷却" → `cold`（第 139–147 行） |
| 关系强度 `relationship_strengths` | "关系比较近/关系近/比较熟/熟/靠谱" → `strong`+`medium`；"关系强/中/弱" → 对应单值（第 149–158 行） |
| 时间窗 `last_interaction_older_than_days` | "3 个月没联系"类表述 → 90 天；"1 个月没联系"类 → 30 天（第 160–164 行） |

**置信度规则**（第 182 行）：命中过滤条件 ≥ 2 项 → confidence **85**；否则 **55**。
**needs_confirmation**（第 183 行）：命中 0 项过滤条件，或查询含"最近没联系"（无明确时间窗）时置为 true。
意图固定为 `search_people`，limit 固定 20，排序固定为 `match_score` desc → `relationship_strength` desc → `last_interaction_at` desc（第 173–181 行）。

### 3.3 validate_query_intent：二次收紧（第 187–196 行）

即使解析结果被篡改也会被拉回白名单：intent 强制重写为 `search_people`；limit 钳制到 1–50；城市/标签/话题去重；状态仅允许 `follow-up/active/cold`、强度仅允许 `strong/medium/weak`（`allow_only` 过滤，第 433–440 行）。

### 3.4 load_candidates：候选加载（第 198–250 行）

单条固定 SQL 从 `persons` 表按 `updated_at DESC` 取**最多 500 人**，子查询携带每人最近一次互动的时间与摘要。SQL 完全静态、无任何用户输入拼接。

### 3.5 candidate_matches：多维过滤（第 252–286 行）

各维度之间为 **AND** 关系，维度内部为 **OR**（任一匹配即通过）：城市子串匹配、资源标签子串匹配、状态精确匹配、强度精确匹配、时间窗（无互动记录视为"超期"，`is_older_than` 第 449–454 行对 None 返回 true）、话题需在该人的互动记录 `topics/content/summary` 中 LIKE 命中（`topic_match_count` 第 361–383 行，参数化查询）。

### 3.6 score_candidate：评分权重明细（第 288–330 行）

| 评分项 | 分值 |
| --- | --- |
| 关系强度 strong / medium / weak | +40 / +25 / +10 |
| 状态为 follow-up（待跟进） | +18 |
| 每命中一个资源标签过滤条件 | +12/个 |
| 每命中一个话题过滤条件 | +10/个 |
| 最近 30 天内有互动 | +8 |
| 敏感级别为 high | **−5**（高敏感联系人适度降权） |

### 3.7 脱敏与前端展示

- **后端脱敏**（`to_result` 第 332–355 行 + `security/sensitivity.rs`）：非 low 敏感级别且未显式 `revealSensitive` 时，`display_name` 返回第一个别名（无别名则返回"高敏感联系人"）；`real_name_hidden` 仅在级别为 **high** 且未 reveal 时为 true。即：medium 默认也以别名展示，但只有 high 会触发前端强制守卫。
- **前端二次确认**（`NaturalLanguageQuery.tsx` 第 70–78 行 + `SensitivityGuard.tsx`）：`realNameHidden` 为 true 的结果被 `SensitivityGuard` 包裹，默认显示"存在高敏感联系人，默认已脱敏"，用户必须点击"确认查看"按钮后才渲染卡片内容。
- 结果卡片展示：显示名、公司/职位、关系强度与状态（中文映射）、上次互动摘要、建议下一步（`build_suggestion` 第 385–397 行：优先使用 `next_step`，否则按状态给出模板建议）。

### 3.8 端到端示例："谁在上海做地产，和我关系比较近？"

该问句即前端示例问句之一，且被单元测试覆盖（`nlq.rs` 第 460–470 行）：

1. **解析**：命中城市"上海"、资源标签"地产"、强度短语"关系比较近" → `relationship_strengths = [strong, medium]`。命中 3 项 ≥ 2 → confidence 85，needs_confirmation = false。
2. **校验**：全部值本就在白名单内，去重后原样通过，limit = 20。
3. **加载**：取最近更新的 ≤500 位联系人。
4. **过滤**：location 含"上海" AND 任一资源标签含"地产" AND 强度 ∈ {strong, medium}。
5. **评分**：如某位 strong + 待跟进 + 命中"地产"标签 + 30 天内有互动的联系人得 40+18+12+8 = 78 分；若其敏感级别为 high 再 −5。
6. **脱敏**：高敏感者以别名出现，`realNameHidden = true`。
7. **展示**：按分数降序渲染卡片，高敏感卡片需点击"确认查看"，每张卡片附"建议下一步"。

---

## 4. 错误处理与降级策略

| 场景 | 行为 | 代码位置 |
| --- | --- | --- |
| NLQ 低置信度 / 无过滤条件 | `needs_confirmation = true`（confidence 55）；查询仍会执行并返回结果，该标志目前仅记入日志，前端尚未针对性提示（见第 5 节） | `nlq.rs` 第 182–183 行 |
| Ollama 不可用 / 模型不存在 / 返回非法 JSON | `extractFromText` 捕获一切异常，降级返回空提取结果 + 原文前 80 字作摘要，不阻断互动录入流程；当前因模型名不匹配长期处于该分支 | `ollama.ts` 第 40–42、65–75 行 |
| 数据库未解锁 | 错误消息含"尚未初始化或解锁"时映射为 HTTP **409 Conflict**（客户端可恢复），其余 DB 错误为 500 | `api/mod.rs` 第 80–95 行 |
| 未鉴权请求 | 中间件返回 401，日志仅记录 path，不记录 token 内容 | `api/mod.rs` 第 97–121 行 |
| 语音转写调用 | 直接抛出明确提示"将在 Phase 3 上线，当前请使用文字录入"，避免静默失败 | `whisper.ts` 第 3–5 行 |

**日志脱敏规范**（`api/mod.rs` 文件头声明：不记录密码、token、姓名、电话及内容原文）：

- NLQ 请求日志只记 `query_len`（字符数）、意图名、置信度、过滤条件**计数**（`safe_filter_summary` 第 399–409 行只输出各维度数量，状态/强度枚举值本身非敏感故可输出枚举名），绝不记录查询原文（`nlq.rs` 第 88–97 行）。
- NLQ 结果日志只记候选数、结果数、limit（第 117–124 行）；评分明细仅在 debug 级别输出 person_id 与分数，不含姓名（第 319–327 行）。
- 联系人搜索只记 `mention_len` 与结果数（`api/mod.rs` 第 312–317 行）；建库/解锁只记耗时与结果，不记密码。
- 前端 Ollama 日志只记文本长度、各字段计数与耗时，不记沟通记录原文（`ollama.ts` 第 18、56–63、66–70 行）。

---

## 5. 当前限制与演进方向

1. **候选 500 上限**：`load_candidates` 固定 `LIMIT 500` 且按 `updated_at DESC` 截断，联系人超过 500 时较久未更新的人会被排除在查询之外。演进方向：将白名单过滤条件下推为 SQL WHERE（仍保持参数化），仅对过滤后的集合评分。
2. **白名单枚举固定**：城市、资源标签、话题字典硬编码在 `parse_query_intent` 中，新增城市或标签需改代码发版。演进方向：从库中已有的 location/resource_tags 去重值动态构建字典，或提供可配置词表。
3. **规则解析表达力有限**：仅支持子串匹配，不理解否定（"不在上海"）、组合逻辑（"上海或北京但排除地产"）与任意时间表达（时间窗只有 30/90 天两档）。演进方向：可引入 LLM 做"自然语言 → QueryIntent JSON"的**结构化翻译**，但输出仍必须经过 `validate_query_intent` 白名单校验，坚持不让模型接触 SQL。
4. **needs_confirmation 未闭环**：后端已产出该标志但未包含在响应体中（`NlqResult` 无此字段），前端无法据此提示用户"条件不明确，请补充"。演进方向：在响应中透出置信度与解析出的条件摘要，供用户确认或修正。
5. **Ollama 前端直调待代理化**：`ollama.ts` 从浏览器直接访问 11434 端口，存在三个问题——模型名/地址硬编码在前端、PWA 从非本机访问时连不上用户手机上的 Ollama、沟通记录原文经浏览器直发本地端口而不受服务端日志/审计约束。演进方向：由 Axum 服务端提供 `/api/extract` 代理端点，统一配置模型名（并修正为 `qwen2.5:7b`）、超时与降级策略。
6. **模型名硬编码错误**：`'qwen2:7b'` ≠ 环境实际的 `qwen2.5:7b`，AI 提取长期降级，用户实际未享受到 LLM 能力；已另立任务修复。
7. **语音与 OCR 缺口**：whisper.ts 为存根、OCR 完全缺失，多模态录入（语音速记、名片/聊天截图导入）尚不可用，均在本轮开发计划中。
8. **推断规则覆盖面**：infer.rs 仅三条规则，未利用互动记录中的共同话题、共同提及等信号。演进方向：增加基于 entity_mentions 共现的推断规则，保持"pending 待确认 + 可解释理由"的既有约束。
