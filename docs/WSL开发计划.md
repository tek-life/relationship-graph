# WSL 迁移开发计划

> 制定日期：2026-07-31
> 依据：需求文档 v1.2、设计文档 v1.3、WSL 迁移交接文档
> 目标架构：WSL 运行后端服务，Windows 浏览器 + 手机浏览器作为前端（Web/PWA）

## 一、环境现状（已勘察）

| 项目 | 状态 |
|---|---|
| Node.js / npm | ✅ v20.20.2 / 10.8.2 |
| npm 依赖 | ✅ 已在 Linux 下重装（原 node_modules 为 Windows 二进制，已重建） |
| 前端 tsc 校验 | ✅ 通过 |
| Rust 工具链 | 🔄 rustup 安装中（用户目录，无需 sudo） |
| gcc / cc | ✅ 13.3.0（可编译 bundled-sqlcipher） |
| Ollama | ✅ 已装，qwen2.5:7b 模型可用 |
| Python | ✅ 3.12（后续 faster-whisper 备用） |
| WSL IP | 172.31.225.119（手机/Windows 访问入口，可能随重启变化） |
| sudo | ⚠️ 需密码，apt 安装系统包时需用户手动执行 |

## 二、架构迁移方案

现状：`React → Tauri invoke → Rust Commands → SQLCipher 本地库`（桌面形态，浏览器无法调用后端）

目标：

```
Windows 浏览器 ─┐
                ├─ HTTP(S) ──> Axum 服务端 (WSL) ──> SQLCipher(SQLite) 数据库
手机浏览器    ─┘                    │
                                    ├──> Ollama (信息抽取 / NLQ / 摘要)
                                    └──> Whisper (语音转写，Phase 3)
```

技术选型（与需求文档十章对齐）：
- 后端：**Rust Axum**——最大化复用 src-tauri 中 ~1800 行已验证的 db/nlq/security 逻辑
- 数据库：MVP 沿用 **SQLite + SQLCipher**（rusqlite bundled-sqlcipher，无需系统库），后续可换 PostgreSQL
- 项目结构：新增 `server/` crate（Cargo workspace），src-tauri 暂保留不动，业务逻辑抽为可复用模块
- 前端：现有 React 代码保留，`services/db.ts` 从 `invoke()` 改为 `fetch()` HTTP 客户端

## 三、开发阶段

### Phase 1：服务端骨架 + 核心 API（当前阶段）
- [ ] Cargo workspace + `server/` Axum 骨架
- [ ] 移植 db 层：schema / crypto(Argon2id派生密钥) / person / relationship / interaction
- [ ] API：`GET /api/health`、`POST /api/auth/unlock`（主密码解锁，签发 token）
- [ ] API：persons / relationships / interactions CRUD、`GET /api/graph`
- [ ] 移植 NLQ：`POST /api/nlq`（QueryIntent 管线，服务端调用 Ollama）
- [ ] 结构化日志：request_id + 脱敏原则（沿用交接文档第 7 章禁录清单）
- 验收：`cargo test` 通过；curl 全部端点正常；错误响应统一 JSON

### Phase 2：前端 Web 化 + 多端访问
- [ ] `services/api.ts` HTTP 客户端替换 Tauri invoke（含 token、错误处理）
- [ ] PasswordGate 对接 `/api/auth/unlock`；SensitivityGuard 对接服务端脱敏
- [ ] Vite dev server `host: 0.0.0.0`，服务端 CORS 白名单
- [ ] 手机适配：响应式布局检查 + PWA manifest
- 验收：Windows 浏览器经 WSL IP 完整走通「录入→查询→图谱」；手机浏览器可访问

### Phase 3：语音录入 + 自然语言统一入口
- [ ] `POST /api/voice/transcribe`：音频上传 → Whisper 转写 → LLM 提炼 → 草稿确认
- [ ] 统一入口意图识别：create_person / update_person / add_interaction / search_people
- [ ] 关键写入先草稿、用户确认后落库（保留决策 #2）

### Phase 4：图谱增强 + 导入
- [x] 关系推断（同公司/同介绍人/同行业+同城）+ 用户确认机制（source/confidence/confirmation_status/inference_reason，2026-07-31 完成）
- [ ] CSV / vCard 导入：字段映射 → 预览 → 去重 → 确认写入
- [ ] 加密备份导出

## 四、关键决策保留清单（摘自交接文档第 16 章）

1. NLQ 不让 LLM 直接生成 SQL，必须走 QueryIntent 白名单管线
2. 关键写入先草稿、确认后保存
3. 日志可观测但不泄露隐私（禁录密码/token/姓名/电话/原文）
4. 表单只是兜底，主交互是自然语言/语音
5. 推断关系必须用户确认
6. 手机端第一版仅访问中低敏感数据

## 五、Open 问题（需用户拍板）

| 问题 | 当前假设 |
|---|---|
| 高敏感数据是否存入 WSL 服务端库 | WSL 与 Windows 同属本人物理机，MVP 暂存服务端 SQLCipher 库，但**手机端请求一律过滤高敏感数据**，高敏感详情需二次确认；若后续部署到远程服务器，再拆分本地库 |
| 认证方案 | MVP 单用户：主密码解锁即签发短期 token，无多用户体系 |
| HTTPS | 局域网 MVP 先用 HTTP；对外暴露前必须加 Caddy/Nginx + TLS |
| WSL IP 漂移 | 建议后续配置 Windows portproxy 或 mirrored 网络模式固定入口 |
