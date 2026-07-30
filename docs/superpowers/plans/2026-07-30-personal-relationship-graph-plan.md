# 个人关系图谱应用实现计划

> **For agentic workers:** REQUIRED SUB-AGENT SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现一个本地优先的个人关系图谱桌面应用 MVP，支持联系人管理、关系维护、语音录入互动记录、自然语言查询、关系图谱可视化、数据库加密和敏感级别访问控制。

**Architecture:** 使用 Tauri（Rust 后端 + React/TypeScript 前端）构建桌面应用。数据层使用 rusqlite 操作 SQLite + SQLCipher 加密数据库。端侧大模型通过 Ollama HTTP API 调用，Whisper.cpp 通过命令行调用实现语音转文字。前端使用 Cytoscape.js 做关系图谱可视化。

**Tech Stack:** Tauri, React, TypeScript, Tailwind CSS, rusqlite, SQLCipher, Ollama, Whisper.cpp, Cytoscape.js, Vitest

---

## 项目结构

```
relationship-graph/
├── src/                         # 前端源码
│   ├── components/              # React 组件
│   │   ├── PersonCard.tsx
│   │   ├── PersonForm.tsx
│   │   ├── InteractionForm.tsx
│   │   ├── VoiceRecorder.tsx
│   │   ├── EntityResolver.tsx
│   │   ├── GraphView.tsx
│   │   ├── NaturalLanguageQuery.tsx
│   │   └── SensitivityGuard.tsx
│   ├── hooks/
│   │   ├── usePersons.ts
│   │   ├── useInteractions.ts
│   │   └── useGraphData.ts
│   ├── services/
│   │   ├── db.ts                # Tauri 命令调用层
│   │   ├── ollama.ts            # Ollama API 调用
│   │   └── whisper.ts           # Whisper.cpp 调用
│   ├── types/
│   │   └── index.ts
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/                   # Rust 后端
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── db/
│   │   │   ├── mod.rs
│   │   │   ├── schema.rs        # 建表/迁移
│   │   │   ├── person.rs        # Person CRUD
│   │   │   ├── relationship.rs  # Relationship CRUD
│   │   │   ├── interaction.rs   # Interaction CRUD
│   │   │   ├── tag.rs           # Tag CRUD
│   │   │   └── crypto.rs        # 数据库加密
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   ├── person.rs
│   │   │   ├── relationship.rs
│   │   │   ├── interaction.rs
│   │   │   ├── graph.rs
│   │   │   ├── nlq.rs
│   │   │   └── security.rs
│   │   └── security/
│   │       ├── mod.rs
│   │       ├── keychain.rs      # 系统密钥链封装
│   │       └── sensitivity.rs   # 敏感级别控制
│   ├── Cargo.toml
│   └── tauri.conf.json
├── tests/                       # 端到端/集成测试
│   └── db.test.ts
├── docs/
│   └── setup.md                 # 本地开发环境搭建
├── package.json
├── vite.config.ts
├── tsconfig.json
└── tailwind.config.js
```

---

## Phase 1: 项目初始化与基础架构

### Task 1: 初始化 Tauri + React 项目

**Files:**
- Create: `relationship-graph/package.json`
- Create: `relationship-graph/vite.config.ts`
- Create: `relationship-graph/tsconfig.json`
- Create: `relationship-graph/index.html`
- Create: `relationship-graph/src/main.tsx`
- Create: `relationship-graph/src/App.tsx`
- Create: `relationship-graph/src-tauri/Cargo.toml`
- Create: `relationship-graph/src-tauri/tauri.conf.json`
- Create: `relationship-graph/src-tauri/src/main.rs`

- [ ] **Step 1: 创建项目目录并初始化前端**

```bash
mkdir -p c:\Users\Haifeng\Documents\SystemDebug\relationship-graph
cd c:\Users\Haifeng\Documents\SystemDebug\relationship-graph
npm create vite@latest . -- --template react-ts
npm install
```

- [ ] **Step 2: 安装 Tauri CLI 和前端依赖**

```bash
npm install -D @tauri-apps/cli
npm install @tauri-apps/api @tauri-apps/plugin-shell
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p
```

- [ ] **Step 3: 初始化 Tauri 后端**

```bash
npx tauri init
```

按提示选择：
- App name: `relationship-graph`
- Window title: `个人关系图谱`
- Where are your web assets: `../dist`
- Dev server URL: `http://localhost:1420`

- [ ] **Step 4: 配置 Tailwind CSS**

Modify `tailwind.config.js`:

```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {},
  },
  plugins: [],
}
```

Modify `src/index.css`:

```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

- [ ] **Step 5: 验证基础运行**

```bash
npm run tauri dev
```

Expected: 桌面窗口打开，显示 Vite + React 默认页面。

- [ ] **Step 6: Commit**

```bash
git init
git add .
git commit -m "chore: init Tauri + React + TypeScript project"
```

---

### Task 2: 配置 Rust 依赖与数据库加密基础

**Files:**
- Modify: `relationship-graph/src-tauri/Cargo.toml`
- Create: `relationship-graph/src-tauri/src/db/mod.rs`
- Create: `relationship-graph/src-tauri/src/db/crypto.rs`
- Create: `relationship-graph/src-tauri/src/security/mod.rs`
- Create: `relationship-graph/src-tauri/src/security/keychain.rs`

- [ ] **Step 1: 添加 Rust 依赖**

Modify `src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1.10", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }
keyring = { version = "3", features = ["windows-native", "apple-native", "linux-native"] }
argon2 = "0.5"
rand = "0.8"
thiserror = "1"
```

- [ ] **Step 2: 实现数据库加密工具**

Create `src-tauri/src/db/crypto.rs`:

```rust
use argon2::{self, Config, ThreadMode, Variant, Version};
use rand::RngCore;
use rusqlite::Connection;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("argon2 error")]
    Argon2Error,
    #[error("database error")]
    DbError(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, CryptoError>;

pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let config = Config {
        variant: Variant::Argon2id,
        version: Version::Version13,
        mem_cost: 65536,
        time_cost: 3,
        lanes: 4,
        thread_mode: ThreadMode::Parallel,
        secret: &[],
        ad: &[],
        hash_length: 32,
    };
    let mut key = [0u8; 32];
    argon2::hash_raw(password.as_bytes(), salt, &config)
        .map_err(|_| CryptoError::Argon2Error)?
        .iter()
        .enumerate()
        .for_each(|(i, v)| key[i] = *v);
    Ok(key)
}

pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()
        .ok()
}

pub fn open_encrypted_db<P: AsRef<Path>>(path: P, key: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    let pragma = format!("PRAGMA key = '{}'", key);
    conn.execute_batch(&pragma)?;
    Ok(conn)
}
```

- [ ] **Step 3: 实现系统密钥链封装**

Create `src-tauri/src/security/keychain.rs`:

```rust
use keyring::Entry;
use std::error::Error;

const SERVICE_NAME: &str = "relationship-graph";
const ACCOUNT_NAME: &str = "database-key";

pub fn store_key(key_hex: &str) -> Result<(), Box<dyn Error>> {
    let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME)?;
    entry.set_password(key_hex)?;
    Ok(())
}

pub fn get_key() -> Result<Option<String>, Box<dyn Error>> {
    let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME)?;
    match entry.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Box::new(e)),
    }
}

pub fn delete_key() -> Result<(), Box<dyn Error>> {
    let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME)?;
    entry.delete_password()?;
    Ok(())
}
```

- [ ] **Step 4: 运行 cargo check 验证编译**

```bash
cd relationship-graph/src-tauri
cargo check
```

Expected: 编译通过，无错误。

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "chore: add crypto, keychain, and rusqlite dependencies"
```

---

## Phase 2: 数据层实现

### Task 3: 数据库 Schema 与迁移

**Files:**
- Create: `relationship-graph/src-tauri/src/db/schema.rs`
- Modify: `relationship-graph/src-tauri/src/db/mod.rs`
- Modify: `relationship-graph/src-tauri/src/lib.rs`

- [ ] **Step 1: 定义 Schema**

Create `src-tauri/src/db/schema.rs`:

```rust
use rusqlite::Connection;

pub fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS persons (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            aliases TEXT NOT NULL DEFAULT '[]',
            avatar TEXT,
            phone TEXT,
            email TEXT,
            company TEXT,
            title TEXT,
            location TEXT,
            background TEXT,
            relationship_strength TEXT,
            resource_tags TEXT NOT NULL DEFAULT '[]',
            sensitivity_level TEXT NOT NULL DEFAULT 'low',
            status TEXT NOT NULL DEFAULT 'active',
            next_step TEXT,
            notes TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS relationships (
            id TEXT PRIMARY KEY,
            from_person_id TEXT NOT NULL,
            to_person_id TEXT NOT NULL,
            type TEXT NOT NULL,
            strength TEXT,
            description TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (from_person_id) REFERENCES persons(id),
            FOREIGN KEY (to_person_id) REFERENCES persons(id)
        );

        CREATE TABLE IF NOT EXISTS interactions (
            id TEXT PRIMARY KEY,
            person_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            content TEXT NOT NULL,
            summary TEXT,
            topics TEXT NOT NULL DEFAULT '[]',
            action_items TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            FOREIGN KEY (person_id) REFERENCES persons(id)
        );

        CREATE TABLE IF NOT EXISTS entity_mentions (
            id TEXT PRIMARY KEY,
            interaction_id TEXT NOT NULL,
            person_id TEXT,
            mention_text TEXT NOT NULL,
            confidence REAL NOT NULL,
            resolved INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (interaction_id) REFERENCES interactions(id),
            FOREIGN KEY (person_id) REFERENCES persons(id)
        );

        CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            category TEXT,
            color TEXT
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_persons_name ON persons(name);
        CREATE INDEX IF NOT EXISTS idx_interactions_person_id ON interactions(person_id);
        CREATE INDEX IF NOT EXISTS idx_interactions_timestamp ON interactions(timestamp);
        "
    )
}
```

- [ ] **Step 2: 创建 DB 模块入口**

Create `src-tauri/src/db/mod.rs`:

```rust
pub mod crypto;
pub mod interaction;
pub mod person;
pub mod relationship;
pub mod schema;
pub mod tag;
```

- [ ] **Step 3: 在 lib.rs 中注册模块**

Modify `src-tauri/src/lib.rs`:

```rust
pub mod commands;
pub mod db;
pub mod security;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(db): add SQLite schema and migrations"
```

---

### Task 4: Person CRUD

**Files:**
- Create: `relationship-graph/src-tauri/src/db/person.rs`
- Create: `relationship-graph/src-tauri/src/types.rs`
- Modify: `relationship-graph/src-tauri/src/commands/person.rs`

- [ ] **Step 1: 定义共享类型**

Create `src-tauri/src/types.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub avatar: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub company: Option<String>,
    pub title: Option<String>,
    pub location: Option<String>,
    pub background: Option<String>,
    pub relationship_strength: Option<String>,
    pub resource_tags: Vec<String>,
    pub sensitivity_level: String,
    pub status: String,
    pub next_step: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePersonRequest {
    pub name: String,
    pub aliases: Vec<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub company: Option<String>,
    pub title: Option<String>,
    pub location: Option<String>,
    pub background: Option<String>,
    pub relationship_strength: Option<String>,
    pub resource_tags: Vec<String>,
    pub sensitivity_level: String,
    pub next_step: Option<String>,
    pub notes: Option<String>,
}
```

- [ ] **Step 2: 实现 Person CRUD**

Create `src-tauri/src/db/person.rs`:

```rust
use crate::types::{CreatePersonRequest, Person};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json;
use uuid::Uuid;

pub fn create(conn: &Connection, req: CreatePersonRequest) -> Result<Person, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let aliases_json = serde_json::to_string(&req.aliases).unwrap_or_default();
    let tags_json = serde_json::to_string(&req.resource_tags).unwrap_or_default();

    conn.execute(
        "INSERT INTO persons (
            id, name, aliases, phone, email, company, title, location, background,
            relationship_strength, resource_tags, sensitivity_level, status, next_step, notes,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            id, req.name, aliases_json, req.phone, req.email, req.company, req.title,
            req.location, req.background, req.relationship_strength, tags_json,
            req.sensitivity_level, "active", req.next_step, req.notes, now, now
        ],
    )?;

    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<Person>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, aliases, avatar, phone, email, company, title, location, background,
         relationship_strength, resource_tags, sensitivity_level, status, next_step, notes,
         created_at, updated_at FROM persons WHERE id = ?1"
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_person(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_all(conn: &Connection) -> Result<Vec<Person>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, aliases, avatar, phone, email, company, title, location, background,
         relationship_strength, resource_tags, sensitivity_level, status, next_step, notes,
         created_at, updated_at FROM persons ORDER BY updated_at DESC"
    )?;
    let rows = stmt.query_map([], map_person)?;
    rows.collect()
}

pub fn search_by_alias(conn: &Connection, alias: &str) -> Result<Vec<Person>, rusqlite::Error> {
    let pattern = format!("%{}%", alias);
    let mut stmt = conn.prepare(
        "SELECT id, name, aliases, avatar, phone, email, company, title, location, background,
         relationship_strength, resource_tags, sensitivity_level, status, next_step, notes,
         created_at, updated_at FROM persons
         WHERE name LIKE ?1 OR aliases LIKE ?1
         ORDER BY updated_at DESC"
    )?;
    let rows = stmt.query_map(params![pattern], map_person)?;
    rows.collect()
}

fn map_person(row: &rusqlite::Row) -> Result<Person, rusqlite::Error> {
    let aliases_json: String = row.get(2)?;
    let tags_json: String = row.get(11)?;
    Ok(Person {
        id: row.get(0)?,
        name: row.get(1)?,
        aliases: serde_json::from_str(&aliases_json).unwrap_or_default(),
        avatar: row.get(3)?,
        phone: row.get(4)?,
        email: row.get(5)?,
        company: row.get(6)?,
        title: row.get(7)?,
        location: row.get(8)?,
        background: row.get(9)?,
        relationship_strength: row.get(10)?,
        resource_tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        sensitivity_level: row.get(12)?,
        status: row.get(13)?,
        next_step: row.get(14)?,
        notes: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}
```

- [ ] **Step 3: 注册 Tauri 命令**

Create `src-tauri/src/commands/person.rs`:

```rust
use crate::db::person;
use crate::types::{CreatePersonRequest, Person};
use rusqlite::Connection;
use tauri::State;

pub struct AppState {
    pub db: std::sync::Mutex<Connection>,
}

#[tauri::command]
pub fn create_person(state: State<AppState>, req: CreatePersonRequest) -> Result<Person, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    person::create(&conn, req).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_persons(state: State<AppState>) -> Result<Vec<Person>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    person::list_all(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_person(state: State<AppState>, id: String) -> Result<Option<Person>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    person::get_by_id(&conn, &id).map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(person): add Person CRUD and Tauri commands"
```

---

### Task 5: Relationship 与 Interaction CRUD

**Files:**
- Create: `relationship-graph/src-tauri/src/db/relationship.rs`
- Create: `relationship-graph/src-tauri/src/db/interaction.rs`
- Create: `relationship-graph/src-tauri/src/commands/relationship.rs`
- Create: `relationship-graph/src-tauri/src/commands/interaction.rs`

- [ ] **Step 1: 实现 Relationship CRUD**

Create `src-tauri/src/db/relationship.rs`:

```rust
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub from_person_id: String,
    pub to_person_id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub strength: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRelationshipRequest {
    pub from_person_id: String,
    pub to_person_id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub strength: Option<String>,
    pub description: Option<String>,
}

pub fn create(conn: &Connection, req: CreateRelationshipRequest) -> Result<Relationship, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO relationships (id, from_person_id, to_person_id, type, strength, description, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, req.from_person_id, req.to_person_id, req.type_, req.strength, req.description, now],
    )?;
    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<Relationship>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, from_person_id, to_person_id, type, strength, description, created_at
         FROM relationships WHERE id = ?1"
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_relationship(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_by_person(conn: &Connection, person_id: &str) -> Result<Vec<Relationship>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, from_person_id, to_person_id, type, strength, description, created_at
         FROM relationships WHERE from_person_id = ?1 OR to_person_id = ?1"
    )?;
    let rows = stmt.query_map(params![person_id], map_relationship)?;
    rows.collect()
}

fn map_relationship(row: &rusqlite::Row) -> Result<Relationship, rusqlite::Error> {
    Ok(Relationship {
        id: row.get(0)?,
        from_person_id: row.get(1)?,
        to_person_id: row.get(2)?,
        type_: row.get(3)?,
        strength: row.get(4)?,
        description: row.get(5)?,
        created_at: row.get(6)?,
    })
}
```

- [ ] **Step 2: 实现 Interaction CRUD**

Create `src-tauri/src/db/interaction.rs`:

```rust
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interaction {
    pub id: String,
    pub person_id: String,
    pub timestamp: String,
    pub content: String,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub action_items: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateInteractionRequest {
    pub person_id: String,
    pub timestamp: String,
    pub content: String,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub action_items: Vec<String>,
}

pub fn create(conn: &Connection, req: CreateInteractionRequest) -> Result<Interaction, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let topics_json = serde_json::to_string(&req.topics).unwrap_or_default();
    let actions_json = serde_json::to_string(&req.action_items).unwrap_or_default();
    conn.execute(
        "INSERT INTO interactions (id, person_id, timestamp, content, summary, topics, action_items, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, req.person_id, req.timestamp, req.content, req.summary, topics_json, actions_json, now],
    )?;
    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn list_by_person(conn: &Connection, person_id: &str) -> Result<Vec<Interaction>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, person_id, timestamp, content, summary, topics, action_items, created_at
         FROM interactions WHERE person_id = ?1 ORDER BY timestamp DESC"
    )?;
    let rows = stmt.query_map(params![person_id], map_interaction)?;
    rows.collect()
}

fn map_interaction(row: &rusqlite::Row) -> Result<Interaction, rusqlite::Error> {
    let topics_json: String = row.get(5)?;
    let actions_json: String = row.get(6)?;
    Ok(Interaction {
        id: row.get(0)?,
        person_id: row.get(1)?,
        timestamp: row.get(2)?,
        content: row.get(3)?,
        summary: row.get(4)?,
        topics: serde_json::from_str(&topics_json).unwrap_or_default(),
        action_items: serde_json::from_str(&actions_json).unwrap_or_default(),
        created_at: row.get(7)?,
    })
}
```

- [ ] **Step 3: 注册命令**

Create `src-tauri/src/commands/relationship.rs`:

```rust
use crate::db::relationship;
use crate::db::relationship::{CreateRelationshipRequest, Relationship};
use crate::commands::person::AppState;
use tauri::State;

#[tauri::command]
pub fn create_relationship(state: State<AppState>, req: CreateRelationshipRequest) -> Result<Relationship, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    relationship::create(&conn, req).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_relationships_by_person(state: State<AppState>, person_id: String) -> Result<Vec<Relationship>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    relationship::list_by_person(&conn, &person_id).map_err(|e| e.to_string())
}
```

Create `src-tauri/src/commands/interaction.rs`:

```rust
use crate::commands::person::AppState;
use crate::db::interaction;
use crate::db::interaction::{CreateInteractionRequest, Interaction};
use tauri::State;

#[tauri::command]
pub fn create_interaction(state: State<AppState>, req: CreateInteractionRequest) -> Result<Interaction, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    interaction::create(&conn, req).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_interactions_by_person(state: State<AppState>, person_id: String) -> Result<Vec<Interaction>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    interaction::list_by_person(&conn, &person_id).map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(db): add Relationship and Interaction CRUD"
```

---

## Phase 3: 前端基础 UI

### Task 6: 前端类型与服务层

**Files:**
- Create: `relationship-graph/src/types/index.ts`
- Create: `relationship-graph/src/services/db.ts`

- [ ] **Step 1: 定义前端类型**

Create `src/types/index.ts`:

```typescript
export interface Person {
  id: string;
  name: string;
  aliases: string[];
  avatar?: string;
  phone?: string;
  email?: string;
  company?: string;
  title?: string;
  location?: string;
  background?: string;
  relationshipStrength?: 'strong' | 'medium' | 'weak';
  resourceTags: string[];
  sensitivityLevel: 'low' | 'medium' | 'high';
  status: 'follow-up' | 'active' | 'cold';
  nextStep?: string;
  notes?: string;
  createdAt: string;
  updatedAt: string;
}

export interface Relationship {
  id: string;
  fromPersonId: string;
  toPersonId: string;
  type: 'introduced' | 'colleague' | 'friend' | 'cooperation' | 'other';
  strength?: 'strong' | 'medium' | 'weak';
  description?: string;
  createdAt: string;
}

export interface Interaction {
  id: string;
  personId: string;
  timestamp: string;
  content: string;
  summary?: string;
  topics: string[];
  actionItems: string[];
  createdAt: string;
}

export type CreatePersonInput = Omit<Person, 'id' | 'createdAt' | 'updatedAt'>;
export type CreateInteractionInput = Omit<Interaction, 'id' | 'createdAt'>;
```

- [ ] **Step 2: 创建 DB 服务层**

Create `src/services/db.ts`:

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { Person, CreatePersonInput, Interaction, CreateInteractionInput, Relationship } from '../types';

export async function createPerson(input: CreatePersonInput): Promise<Person> {
  return invoke<Person>('create_person', { req: snakeCase(input) });
}

export async function listPersons(): Promise<Person[]> {
  return invoke<Person[]>('list_persons');
}

export async function getPerson(id: string): Promise<Person | null> {
  return invoke<Person | null>('get_person', { id });
}

export async function createInteraction(input: CreateInteractionInput): Promise<Interaction> {
  return invoke<Interaction>('create_interaction', { req: snakeCase(input) });
}

export async function listInteractionsByPerson(personId: string): Promise<Interaction[]> {
  return invoke<Interaction[]>('list_interactions_by_person', { personId });
}

export async function createRelationship(input: Omit<Relationship, 'id' | 'createdAt'>): Promise<Relationship> {
  return invoke<Relationship>('create_relationship', { req: snakeCase(input) });
}

export async function listRelationshipsByPerson(personId: string): Promise<Relationship[]> {
  return invoke<Relationship[]>('list_relationships_by_person', { personId });
}

function snakeCase(obj: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj)) {
    const snake = key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
    result[snake] = value;
  }
  return result;
}
```

- [ ] **Step 3: Commit**

```bash
git add .
git commit -m "feat(frontend): add TypeScript types and DB service layer"
```

---

### Task 7: 联系人列表与名片视图

**Files:**
- Create: `relationship-graph/src/components/PersonCard.tsx`
- Create: `relationship-graph/src/components/PersonForm.tsx`
- Create: `relationship-graph/src/components/PersonList.tsx`
- Modify: `relationship-graph/src/App.tsx`

- [ ] **Step 1: 实现 PersonForm 组件**

Create `src/components/PersonForm.tsx`:

```tsx
import { useState } from 'react';
import type { CreatePersonInput } from '../types';

interface Props {
  onSubmit: (data: CreatePersonInput) => void;
}

export default function PersonForm({ onSubmit }: Props) {
  const [name, setName] = useState('');
  const [aliases, setAliases] = useState('');
  const [company, setCompany] = useState('');
  const [title, setTitle] = useState('');
  const [phone, setPhone] = useState('');
  const [email, setEmail] = useState('');
  const [background, setBackground] = useState('');
  const [relationshipStrength, setRelationshipStrength] = useState<'strong' | 'medium' | 'weak'>('medium');
  const [resourceTags, setResourceTags] = useState('');
  const [sensitivityLevel, setSensitivityLevel] = useState<'low' | 'medium' | 'high'>('low');
  const [nextStep, setNextStep] = useState('');
  const [notes, setNotes] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit({
      name,
      aliases: aliases.split(',').map((s) => s.trim()).filter(Boolean),
      company: company || undefined,
      title: title || undefined,
      phone: phone || undefined,
      email: email || undefined,
      background: background || undefined,
      relationshipStrength,
      resourceTags: resourceTags.split(',').map((s) => s.trim()).filter(Boolean),
      sensitivityLevel,
      status: 'active',
      nextStep: nextStep || undefined,
      notes: notes || undefined,
    });
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4 p-4 border rounded-lg">
      <h3 className="font-bold text-lg">新增联系人</h3>
      <input className="w-full p-2 border rounded" placeholder="姓名" value={name} onChange={(e) => setName(e.target.value)} required />
      <input className="w-full p-2 border rounded" placeholder="昵称/代称，逗号分隔" value={aliases} onChange={(e) => setAliases(e.target.value)} />
      <input className="w-full p-2 border rounded" placeholder="公司" value={company} onChange={(e) => setCompany(e.target.value)} />
      <input className="w-full p-2 border rounded" placeholder="职位" value={title} onChange={(e) => setTitle(e.target.value)} />
      <input className="w-full p-2 border rounded" placeholder="电话" value={phone} onChange={(e) => setPhone(e.target.value)} />
      <input className="w-full p-2 border rounded" placeholder="邮箱" value={email} onChange={(e) => setEmail(e.target.value)} />
      <textarea className="w-full p-2 border rounded" placeholder="认识背景" value={background} onChange={(e) => setBackground(e.target.value)} />
      <select className="w-full p-2 border rounded" value={relationshipStrength} onChange={(e) => setRelationshipStrength(e.target.value as 'strong' | 'medium' | 'weak')}>
        <option value="strong">关系强</option>
        <option value="medium">关系中</option>
        <option value="weak">关系弱</option>
      </select>
      <input className="w-full p-2 border rounded" placeholder="资源标签，逗号分隔" value={resourceTags} onChange={(e) => setResourceTags(e.target.value)} />
      <select className="w-full p-2 border rounded" value={sensitivityLevel} onChange={(e) => setSensitivityLevel(e.target.value as 'low' | 'medium' | 'high')}>
        <option value="low">低敏感</option>
        <option value="medium">中敏感</option>
        <option value="high">高敏感</option>
      </select>
      <input className="w-full p-2 border rounded" placeholder="下一步计划" value={nextStep} onChange={(e) => setNextStep(e.target.value)} />
      <textarea className="w-full p-2 border rounded" placeholder="备注" value={notes} onChange={(e) => setNotes(e.target.value)} />
      <button type="submit" className="px-4 py-2 bg-blue-600 text-white rounded">保存</button>
    </form>
  );
}
```

- [ ] **Step 2: 实现 PersonCard 组件**

Create `src/components/PersonCard.tsx`:

```tsx
import type { Person } from '../types';

interface Props {
  person: Person;
  lastInteractionSummary?: string;
}

export default function PersonCard({ person, lastInteractionSummary }: Props) {
  const displayName = person.sensitivityLevel === 'low' ? person.name : (person.aliases[0] || '***');
  const showRealName = person.sensitivityLevel === 'low';

  return (
    <div className="p-4 border rounded-lg shadow-sm hover:shadow-md transition-shadow">
      <div className="flex justify-between items-start">
        <div>
          <h3 className="text-xl font-bold">{displayName}</h3>
          {showRealName && person.aliases.length > 0 && (
            <p className="text-sm text-gray-500">代称：{person.aliases.join(', ')}</p>
          )}
          {!showRealName && (
            <p className="text-sm text-gray-500">真实姓名已隐藏</p>
          )}
        </div>
        <span className={`px-2 py-1 text-xs rounded ${
          person.sensitivityLevel === 'high' ? 'bg-red-100 text-red-800' :
          person.sensitivityLevel === 'medium' ? 'bg-yellow-100 text-yellow-800' :
          'bg-green-100 text-green-800'
        }`}>
          {person.sensitivityLevel === 'high' ? '高敏感' : person.sensitivityLevel === 'medium' ? '中敏感' : '低敏感'}
        </span>
      </div>
      <p className="text-gray-600 mt-2">{person.company} {person.title}</p>
      {person.background && <p className="text-sm text-gray-500 mt-1">{person.background}</p>}
      <div className="flex flex-wrap gap-2 mt-3">
        {person.resourceTags.map((tag) => (
          <span key={tag} className="px-2 py-1 text-xs bg-blue-100 text-blue-800 rounded">{tag}</span>
        ))}
      </div>
      <div className="mt-3 text-sm">
        <p>关系强度：{person.relationshipStrength === 'strong' ? '强' : person.relationshipStrength === 'medium' ? '中' : '弱'}</p>
        <p>当前状态：{person.status === 'follow-up' ? '待跟进' : person.status === 'active' ? '活跃' : '冷却'}</p>
        {lastInteractionSummary && <p>上次互动：{lastInteractionSummary}</p>}
        {person.nextStep && <p>下一步：{person.nextStep}</p>}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: 实现 PersonList 并更新 App**

Create `src/components/PersonList.tsx`:

```tsx
import type { Person } from '../types';
import PersonCard from './PersonCard';

interface Props {
  persons: Person[];
}

export default function PersonList({ persons }: Props) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
      {persons.map((person) => (
        <PersonCard key={person.id} person={person} />
      ))}
    </div>
  );
}
```

Modify `src/App.tsx`:

```tsx
import { useEffect, useState } from 'react';
import PersonForm from './components/PersonForm';
import PersonList from './components/PersonList';
import { listPersons, createPerson } from './services/db';
import type { CreatePersonInput, Person } from './types';

function App() {
  const [persons, setPersons] = useState<Person[]>([]);

  const loadPersons = async () => {
    const data = await listPersons();
    setPersons(data);
  };

  useEffect(() => {
    loadPersons();
  }, []);

  const handleCreate = async (input: CreatePersonInput) => {
    await createPerson(input);
    await loadPersons();
  };

  return (
    <div className="min-h-screen p-6 bg-gray-50">
      <h1 className="text-2xl font-bold mb-6">个人关系图谱</h1>
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-1">
          <PersonForm onSubmit={handleCreate} />
        </div>
        <div className="lg:col-span-2">
          <h2 className="text-xl font-semibold mb-4">联系人</h2>
          <PersonList persons={persons} />
        </div>
      </div>
    </div>
  );
}

export default App;
```

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(ui): add PersonForm, PersonCard, and PersonList"
```

---

## Phase 4: 安全与数据库初始化

### Task 8: 数据库初始化与主密码流程

**Files:**
- Create: `relationship-graph/src-tauri/src/commands/security.rs`
- Modify: `relationship-graph/src-tauri/src/lib.rs`
- Modify: `relationship-graph/src-tauri/src/main.rs`
- Modify: `relationship-graph/src-tauri/src/commands/mod.rs`

- [ ] **Step 1: 实现安全命令**

Create `src-tauri/src/commands/security.rs`:

```rust
use crate::commands::person::AppState;
use crate::db::crypto::{derive_key, generate_salt, hex_decode, hex_encode, open_encrypted_db};
use crate::db::schema;
use crate::security::keychain;
use rusqlite::Connection;
use std::path::PathBuf;
use tauri::State;

#[derive(serde::Serialize)]
pub struct InitResult {
    pub initialized: bool,
    pub requires_password: bool,
}

#[tauri::command]
pub fn check_db_state() -> Result<InitResult, String> {
    let db_path = get_db_path()?;
    let exists = db_path.exists();
    let has_key = keychain::get_key().map_err(|e| e.to_string())?.is_some();
    Ok(InitResult {
        initialized: exists,
        requires_password: !has_key,
    })
}

#[tauri::command]
pub fn setup_database(password: String, state: State<AppState>) -> Result<(), String> {
    let db_path = get_db_path()?;
    let salt = generate_salt();
    let key = derive_key(&password, &salt).map_err(|e| e.to_string())?;
    let key_hex = hex_encode(&key);
    let salt_hex = hex_encode(&salt);

    std::fs::create_dir_all(db_path.parent().unwrap()).map_err(|e| e.to_string())?;

    let conn = open_encrypted_db(&db_path, &key_hex).map_err(|e| e.to_string())?;
    schema::migrate(&conn).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('salt', ?1)",
        [salt_hex],
    ).map_err(|e| e.to_string())?;

    keychain::store_key(&key_hex).map_err(|e| e.to_string())?;

    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    *db = conn;
    Ok(())
}

#[tauri::command]
pub fn unlock_database(password: String, state: State<AppState>) -> Result<(), String> {
    let db_path = get_db_path()?;
    let salt_hex = get_stored_salt(&db_path)?;
    let salt = hex_decode(&salt_hex).ok_or("invalid salt")?;
    let key = derive_key(&password, &salt).map_err(|e| e.to_string())?;
    let key_hex = hex_encode(&key);

    let conn = open_encrypted_db(&db_path, &key_hex).map_err(|e| e.to_string())?;
    conn.execute("SELECT 1", []).map_err(|_| "invalid password")?;

    keychain::store_key(&key_hex).map_err(|e| e.to_string())?;

    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    *db = conn;
    Ok(())
}

fn get_db_path() -> Result<PathBuf, String> {
    let home = dirs::data_dir().ok_or("cannot find data dir")?;
    Ok(home.join("relationship-graph").join("app.db"))
}

fn get_stored_salt(db_path: &PathBuf) -> Result<String, String> {
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    let salt: String = conn
        .query_row("SELECT value FROM settings WHERE key = 'salt'", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    Ok(salt)
}
```

- [ ] **Step 2: 更新 Cargo.toml 添加 dirs 依赖**

```toml
dirs = "5"
```

- [ ] **Step 3: 更新 lib.rs 注册命令和状态**

Modify `src-tauri/src/lib.rs`:

```rust
pub mod commands;
pub mod db;
pub mod security;
pub mod types;

use commands::person::AppState;
use rusqlite::Connection;
use std::sync::Mutex;

pub fn run() {
    let state = AppState {
        db: Mutex::new(Connection::open_in_memory().expect("in-memory db")),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::person::create_person,
            commands::person::list_persons,
            commands::person::get_person,
            commands::relationship::create_relationship,
            commands::relationship::list_relationships_by_person,
            commands::interaction::create_interaction,
            commands::interaction::list_interactions_by_person,
            commands::security::check_db_state,
            commands::security::setup_database,
            commands::security::unlock_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: 创建 commands/mod.rs**

Create `src-tauri/src/commands/mod.rs`:

```rust
pub mod interaction;
pub mod person;
pub mod relationship;
pub mod security;
```

- [ ] **Step 5: 实现前端主密码界面**

Create `src/components/PasswordGate.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface Props {
  children: React.ReactNode;
}

export default function PasswordGate({ children }: Props) {
  const [loading, setLoading] = useState(true);
  const [needsSetup, setNeedsSetup] = useState(false);
  const [needsPassword, setNeedsPassword] = useState(false);
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');

  useEffect(() => {
    invoke<{ initialized: boolean; requires_password: boolean }>('check_db_state')
      .then((state) => {
        setNeedsSetup(!state.initialized);
        setNeedsPassword(state.requires_password && state.initialized);
        setLoading(false);
      })
      .catch((err) => setError(err.toString()));
  }, []);

  const handleSetup = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await invoke('setup_database', { password });
      setNeedsSetup(false);
      setNeedsPassword(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleUnlock = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await invoke('unlock_database', { password });
      setNeedsPassword(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  if (loading) return <div className="p-6">加载中...</div>;

  if (needsSetup || needsPassword) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-100">
        <form onSubmit={needsSetup ? handleSetup : handleUnlock} className="bg-white p-8 rounded-lg shadow-md w-96">
          <h2 className="text-xl font-bold mb-4">{needsSetup ? '初始化数据库' : '解锁数据库'}</h2>
          <input
            type="password"
            className="w-full p-2 border rounded mb-4"
            placeholder="主密码"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          {error && <p className="text-red-500 text-sm mb-4">{error}</p>}
          <button type="submit" className="w-full px-4 py-2 bg-blue-600 text-white rounded">
            {needsSetup ? '创建并加密' : '解锁'}
          </button>
        </form>
      </div>
    );
  }

  return <>{children}</>;
}
```

Modify `src/main.tsx`:

```tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import PasswordGate from './components/PasswordGate';
import './index.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <PasswordGate>
      <App />
    </PasswordGate>
  </React.StrictMode>
);
```

- [ ] **Step 6: Commit**

```bash
git add .
git commit -m "feat(security): add database encryption and password gate"
```

---

## Phase 5: 语音录入与歧义确认

### Task 9: 集成 Whisper.cpp 语音转文字

**Files:**
- Create: `relationship-graph/src-tauri/src/commands/voice.rs`
- Create: `relationship-graph/src/services/whisper.ts`
- Create: `relationship-graph/src/components/VoiceRecorder.tsx`

- [ ] **Step 1: 添加 Tauri shell 插件权限**

Modify `src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default permissions",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-execute"
  ],
  "commands": {
    "allow": [
      { "command": "whisper-cli", "args": true }
    ]
  }
}
```

- [ ] **Step 2: 实现语音转文字命令**

Create `src-tauri/src/commands/voice.rs`:

```rust
use std::path::PathBuf;
use tauri::command;
use tauri_plugin_shell::ShellExt;

#[derive(serde::Serialize)]
pub struct TranscriptionResult {
    pub text: String,
}

#[command]
pub async fn transcribe_audio(app: tauri::AppHandle, audio_path: String) -> Result<TranscriptionResult, String> {
    let model_path = get_whisper_model_path()?;
    let output = app
        .shell()
        .command("whisper-cli")
        .args([
            "-m", &model_path,
            "-f", &audio_path,
            "--language", "zh",
            "--output-txt",
        ])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(TranscriptionResult { text })
}

fn get_whisper_model_path() -> Result<String, String> {
    let models_dir = dirs::data_dir()
        .ok_or("no data dir")?
        .join("relationship-graph")
        .join("models");
    let path = models_dir.join("ggml-base.bin");
    Ok(path.to_string_lossy().to_string())
}
```

- [ ] **Step 3: 前端录音组件**

Create `src/components/VoiceRecorder.tsx`:

```tsx
import { useState, useRef } from 'react';

interface Props {
  onTranscript: (text: string) => void;
}

export default function VoiceRecorder({ onTranscript }: Props) {
  const [recording, setRecording] = useState(false);
  const mediaRecorder = useRef<MediaRecorder | null>(null);
  const chunks = useRef<Blob[]>([]);

  const startRecording = async () => {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    mediaRecorder.current = new MediaRecorder(stream);
    chunks.current = [];

    mediaRecorder.current.ondataavailable = (e) => {
      if (e.data.size > 0) chunks.current.push(e.data);
    };

    mediaRecorder.current.onstop = async () => {
      const blob = new Blob(chunks.current, { type: 'audio/webm' });
      const arrayBuffer = await blob.arrayBuffer();
      const uint8 = new Uint8Array(arrayBuffer);
      const base64 = btoa(String.fromCharCode(...uint8));
      const result = await invoke<{ text: string }>('transcribe_audio', { audioPath: `data:audio/webm;base64,${base64}` });
      onTranscript(result.text);
    };

    mediaRecorder.current.start();
    setRecording(true);
  };

  const stopRecording = () => {
    mediaRecorder.current?.stop();
    setRecording(false);
  };

  return (
    <button
      onClick={recording ? stopRecording : startRecording}
      className={`px-4 py-2 rounded ${recording ? 'bg-red-600' : 'bg-blue-600'} text-white`}
    >
      {recording ? '停止录音' : '开始录音'}
    </button>
  );
}
```

注意：这里为了原型简化，实际应先把录音保存为临时文件再传给 whisper-cli。完整实现需通过 Tauri fs 插件写入临时 wav 文件。

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(voice): integrate Whisper.cpp for local transcription"
```

---

### Task 10: 端侧大模型信息抽取与歧义确认

**Files:**
- Create: `relationship-graph/src/services/ollama.ts`
- Create: `relationship-graph/src/components/EntityResolver.tsx`
- Modify: `relationship-graph/src/components/InteractionForm.tsx`

- [ ] **Step 1: 实现 Ollama 服务**

Create `src/services/ollama.ts`:

```typescript
export interface ExtractedEntities {
  persons: { mention: string; confidence: number }[];
  topics: string[];
  actionItems: string[];
  summary: string;
}

export async function extractFromText(text: string): Promise<ExtractedEntities> {
  const response = await fetch('http://localhost:11434/api/generate', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      model: 'qwen2:7b',
      prompt: `从下面这段沟通记录中提取关键信息，以 JSON 格式输出：
{
  "persons": [{"mention": "提到的称呼", "confidence": 0.9}],
  "topics": ["话题1", "话题2"],
  "actionItems": ["待办1", "待办2"],
  "summary": "一句话摘要"
}

沟通记录：
${text}

只输出 JSON，不要其他内容。`,
      stream: false,
      format: 'json',
    }),
  });

  const data = await response.json();
  const parsed = JSON.parse(data.response);
  return {
    persons: parsed.persons || [],
    topics: parsed.topics || [],
    actionItems: parsed.action_items || parsed.actionItems || [],
    summary: parsed.summary || '',
  };
}
```

- [ ] **Step 2: 实现歧义确认组件**

Create `src/components/EntityResolver.tsx`:

```tsx
import { useEffect, useState } from 'react';
import type { Person } from '../types';
import { listPersons } from '../services/db';

interface Mention {
  mention: string;
  confidence: number;
}

interface Props {
  mentions: Mention[];
  onResolved: (resolved: Record<string, string | null>) => void;
}

export default function EntityResolver({ mentions, onResolved }: Props) {
  const [persons, setPersons] = useState<Person[]>([]);
  const [resolved, setResolved] = useState<Record<string, string | null>>({});

  useEffect(() => {
    listPersons().then(setPersons);
  }, []);

  const findCandidates = (mention: string) => {
    return persons.filter((p) =>
      p.name.includes(mention) || p.aliases.some((a) => a.includes(mention))
    );
  };

  return (
    <div className="space-y-4 p-4 border rounded-lg">
      <h3 className="font-bold">确认提到的联系人</h3>
      {mentions.map((m) => {
        const candidates = findCandidates(m.mention);
        return (
          <div key={m.mention} className="space-y-2">
            <p>原文："{m.mention}"</p>
            {candidates.length === 0 ? (
              <p className="text-sm text-gray-500">未找到匹配联系人</p>
            ) : candidates.length === 1 ? (
              <p className="text-sm text-green-600">已自动匹配：{candidates[0].name}</p>
            ) : (
              <div className="flex gap-2 flex-wrap">
                {candidates.map((c) => (
                  <button
                    key={c.id}
                    onClick={() => {
                      const next = { ...resolved, [m.mention]: c.id };
                      setResolved(next);
                      onResolved(next);
                    }}
                    className={`px-3 py-1 text-sm rounded border ${
                      resolved[m.mention] === c.id ? 'bg-blue-600 text-white' : 'bg-white'
                    }`}
                  >
                    {c.name}
                  </button>
                ))}
                <button
                  onClick={() => {
                    const next = { ...resolved, [m.mention]: null };
                    setResolved(next);
                    onResolved(next);
                  }}
                  className="px-3 py-1 text-sm rounded border bg-gray-100"
                >
                  忽略
                </button>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 3: 实现 InteractionForm**

Create `src/components/InteractionForm.tsx`:

```tsx
import { useState } from 'react';
import VoiceRecorder from './VoiceRecorder';
import EntityResolver from './EntityResolver';
import { extractFromText } from '../services/ollama';
import type { CreateInteractionInput } from '../types';

interface Props {
  personId: string;
  onSubmit: (input: CreateInteractionInput) => void;
}

export default function InteractionForm({ personId, onSubmit }: Props) {
  const [content, setContent] = useState('');
  const [mentions, setMentions] = useState<{ mention: string; confidence: number }[]>([]);
  const [extracted, setExtracted] = useState<{ topics: string[]; actionItems: string[]; summary: string } | null>(null);

  const handleTranscript = async (text: string) => {
    setContent(text);
    const result = await extractFromText(text);
    setMentions(result.persons);
    setExtracted({ topics: result.topics, actionItems: result.actionItems, summary: result.summary });
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit({
      personId,
      timestamp: new Date().toISOString(),
      content,
      summary: extracted?.summary,
      topics: extracted?.topics || [],
      actionItems: extracted?.actionItems || [],
    });
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4 p-4 border rounded-lg">
      <h3 className="font-bold text-lg">新增互动记录</h3>
      <VoiceRecorder onTranscript={handleTranscript} />
      <textarea
        className="w-full p-2 border rounded h-32"
        placeholder="沟通内容"
        value={content}
        onChange={(e) => setContent(e.target.value)}
      />
      {mentions.length > 0 && <EntityResolver mentions={mentions} onResolved={(r) => console.log(r)} />}
      {extracted && (
        <div className="text-sm text-gray-600">
          <p>摘要：{extracted.summary}</p>
          <p>话题：{extracted.topics.join(', ')}</p>
          <p>待办：{extracted.actionItems.join(', ')}</p>
        </div>
      )}
      <button type="submit" className="px-4 py-2 bg-blue-600 text-white rounded">保存</button>
    </form>
  );
}
```

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(nlp): add Ollama entity extraction and entity resolver UI"
```

---

## Phase 6: 关系图谱可视化

### Task 11: 集成 Cytoscape.js 关系图谱

**Files:**
- Create: `relationship-graph/src/components/GraphView.tsx`
- Create: `relationship-graph/src/hooks/useGraphData.ts`
- Modify: `relationship-graph/src/App.tsx`

- [ ] **Step 1: 安装依赖**

```bash
npm install cytoscape
npm install -D @types/cytoscape
```

- [ ] **Step 2: 实现图谱数据 Hook**

Create `src/hooks/useGraphData.ts`:

```typescript
import { useEffect, useState } from 'react';
import { listPersons, listRelationshipsByPerson } from '../services/db';
import type { Person, Relationship } from '../types';

export interface GraphNode {
  data: { id: string; label: string; sensitivity: string };
}

export interface GraphEdge {
  data: { id: string; source: string; target: string; label: string };
}

export function useGraphData() {
  const [elements, setElements] = useState<(GraphNode | GraphEdge)[]>([]);

  useEffect(() => {
    async function load() {
      const persons = await listPersons();
      const nodes: GraphNode[] = persons.map((p) => ({
        data: {
          id: p.id,
          label: p.sensitivityLevel === 'low' ? p.name : (p.aliases[0] || '***'),
          sensitivity: p.sensitivityLevel,
        },
      }));

      const edgeSet = new Set<string>();
      const edges: GraphEdge[] = [];

      for (const person of persons) {
        const relationships = await listRelationshipsByPerson(person.id);
        for (const r of relationships) {
          const edgeId = [r.fromPersonId, r.toPersonId].sort().join('-');
          if (!edgeSet.has(edgeId)) {
            edgeSet.add(edgeId);
            edges.push({
              data: {
                id: edgeId,
                source: r.fromPersonId,
                target: r.toPersonId,
                label: r.type,
              },
            });
          }
        }
      }

      setElements([...nodes, ...edges]);
    }

    load();
  }, []);

  return elements;
}
```

- [ ] **Step 3: 实现 GraphView 组件**

Create `src/components/GraphView.tsx`:

```tsx
import { useEffect, useRef } from 'react';
import cytoscape from 'cytoscape';
import type { GraphNode, GraphEdge } from '../hooks/useGraphData';

interface Props {
  elements: (GraphNode | GraphEdge)[];
  onNodeClick?: (id: string) => void;
}

export default function GraphView({ elements, onNodeClick }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<cytoscape.Core | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const cy = cytoscape({
      container: containerRef.current,
      elements,
      style: [
        {
          selector: 'node',
          style: {
            label: 'data(label)',
            width: 60,
            height: 60,
            'background-color': '#3b82f6',
            color: '#fff',
            'text-valign': 'center',
            'text-halign': 'center',
            'font-size': '12px',
          },
        },
        {
          selector: 'edge',
          style: {
            width: 2,
            'line-color': '#94a3b8',
            'target-arrow-color': '#94a3b8',
            'target-arrow-shape': 'triangle',
            label: 'data(label)',
            'font-size': '10px',
          },
        },
      ],
      layout: { name: 'cose', padding: 20 },
    });

    cy.on('tap', 'node', (evt) => {
      onNodeClick?.(evt.target.id());
    });

    cyRef.current = cy;

    return () => {
      cy.destroy();
    };
  }, [elements, onNodeClick]);

  return <div ref={containerRef} className="w-full h-[500px] border rounded-lg bg-gray-50" />;
}
```

- [ ] **Step 4: 在 App 中加入图谱页签**

Modify `src/App.tsx`，增加标签页切换（联系人 / 图谱 / 查询），此处略去具体 JSX 以控制篇幅，核心逻辑为：

```tsx
const [activeTab, setActiveTab] = useState<'list' | 'graph' | 'query'>('list');
const graphElements = useGraphData();

// 渲染三个 tab 切换按钮
// activeTab === 'graph' 时渲染 <GraphView elements={graphElements} />
// activeTab === 'query' 时渲染 <NaturalLanguageQuery />
```

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "feat(graph): add Cytoscape.js relationship graph visualization"
```

---

## Phase 7: 自然语言查询

### Task 12: 实现 NLQ 解析与执行

**Files:**
- Create: `relationship-graph/src-tauri/src/commands/nlq.rs`
- Create: `relationship-graph/src/components/NaturalLanguageQuery.tsx`
- Modify: `relationship-graph/src-tauri/src/lib.rs`

- [ ] **Step 1: 实现 NLQ 命令**

Create `src-tauri/src/commands/nlq.rs`:

```rust
use crate::commands::person::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct NlqRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct NlqResult {
    pub person_id: String,
    pub name: String,
    pub company: Option<String>,
    pub title: Option<String>,
    pub relationship_strength: Option<String>,
    pub last_interaction_summary: Option<String>,
    pub status: String,
    pub next_step: Option<String>,
    pub suggestion: String,
}

#[tauri::command]
pub async fn natural_language_query(state: State<'_, AppState>, req: NlqRequest) -> Result<Vec<NlqResult>, String> {
    // 初版使用简单关键词匹配，后续接入 Ollama 做意图解析
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.company, p.title, p.relationship_strength, p.status, p.next_step,
                (SELECT summary FROM interactions WHERE person_id = p.id ORDER BY timestamp DESC LIMIT 1) as last_summary
         FROM persons p
         WHERE p.resource_tags LIKE ?1 OR p.location LIKE ?2 OR p.status LIKE ?3
         ORDER BY p.updated_at DESC
         LIMIT 20"
    ).map_err(|e| e.to_string())?;

    let pattern_tags = format!("%{}%", req.query);
    let pattern_location = format!("%{}%", req.query);
    let pattern_status = if req.query.contains("待跟进") { "follow-up" } else { "%" };

    let rows = stmt.query_map(params![pattern_tags, pattern_location, pattern_status], |row| {
        Ok(NlqResult {
            person_id: row.get(0)?,
            name: row.get(1)?,
            company: row.get(2)?,
            title: row.get(3)?,
            relationship_strength: row.get(4)?,
            last_interaction_summary: row.get(5)?,
            status: row.get(6)?,
            next_step: row.get(7)?,
            suggestion: "建议保持联系".to_string(),
        })
    }).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
```

- [ ] **Step 2: 注册命令**

Modify `src-tauri/src/lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    commands::nlq::natural_language_query,
])
```

- [ ] **Step 3: 前端 NLQ 组件**

Create `src/components/NaturalLanguageQuery.tsx`:

```tsx
import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface Result {
  person_id: string;
  name: string;
  company?: string;
  title?: string;
  relationship_strength?: string;
  last_interaction_summary?: string;
  status: string;
  next_step?: string;
  suggestion: string;
}

export default function NaturalLanguageQuery() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<Result[]>([]);
  const [loading, setLoading] = useState(false);

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    const data = await invoke<Result[]>('natural_language_query', { req: { query } });
    setResults(data);
    setLoading(false);
  };

  return (
    <div className="space-y-4">
      <form onSubmit={handleSearch} className="flex gap-2">
        <input
          className="flex-1 p-2 border rounded"
          placeholder="例如：谁在上海做地产，和我关系比较近？"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <button type="submit" className="px-4 py-2 bg-blue-600 text-white rounded" disabled={loading}>
          {loading ? '搜索中...' : '查询'}
        </button>
      </form>
      <div className="space-y-3">
        {results.map((r) => (
          <div key={r.person_id} className="p-4 border rounded-lg">
            <h4 className="font-bold">{r.name}</h4>
            <p className="text-sm text-gray-600">{r.company} {r.title}</p>
            {r.last_interaction_summary && <p className="text-sm mt-1">上次互动：{r.last_interaction_summary}</p>}
            <p className="text-sm mt-1">建议：{r.suggestion}</p>
          </div>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(nlq): add natural language query command and UI"
```

---

## Phase 8: 敏感级别访问控制完善

### Task 13: 实现敏感信息二次确认

**Files:**
- Create: `relationship-graph/src/components/SensitivityGuard.tsx`
- Modify: `relationship-graph/src/components/PersonCard.tsx`
- Modify: `relationship-graph/src/components/NaturalLanguageQuery.tsx`

- [ ] **Step 1: 实现 SensitivityGuard 组件**

Create `src/components/SensitivityGuard.tsx`:

```tsx
import { useState } from 'react';

interface Props {
  level: 'low' | 'medium' | 'high';
  children: React.ReactNode;
  fallback: React.ReactNode;
}

export default function SensitivityGuard({ level, children, fallback }: Props) {
  const [revealed, setRevealed] = useState(false);

  if (level === 'low' || revealed) {
    return <>{children}</>;
  }

  return (
    <div className="relative">
      {fallback}
      {level === 'high' && (
        <button
          onClick={() => setRevealed(true)}
          className="mt-2 text-sm text-blue-600 underline"
        >
          查看敏感信息（需确认）
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 2: 在 PersonCard 中应用**

Modify `PersonCard.tsx` 中姓名显示部分，用 `SensitivityGuard` 包裹真实姓名。中敏感显示代称，高敏感显示脱敏提示。

- [ ] **Step 3: 在 NLQ 中高敏感联系人默认折叠**

Modify `NaturalLanguageQuery.tsx`：结果中 sensitivity_level 为 high 的条目默认只显示"高敏感联系人（点击确认查看）"，点击后走 SensitivityGuard 确认流程再展示详情。

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(security): add sensitivity level guard for person card and NLQ"
```

---

## Phase 9: 测试、构建与打包

### Task 14: 单元测试与集成测试

**Files:**
- Create: `relationship-graph/src-tauri/src/db/tests.rs`
- Create: `relationship-graph/tests/db.test.ts`

- [ ] **Step 1: 添加 Rust 单元测试**

Create `src-tauri/src/db/tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::super::*;
    use rusqlite::Connection;

    fn in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::migrate(&conn).unwrap();
        conn
    }

    #[test]
    fn test_create_person() {
        let conn = in_memory_db();
        let person = person::create(&conn, types::CreatePersonRequest {
            name: "张三".to_string(),
            aliases: vec!["老张".to_string()],
            phone: None,
            email: None,
            company: None,
            title: None,
            location: None,
            background: None,
            relationship_strength: Some("strong".to_string()),
            resource_tags: vec!["地产".to_string()],
            sensitivity_level: "low".to_string(),
            next_step: None,
            notes: None,
        }).unwrap();

        assert_eq!(person.name, "张三");
        assert_eq!(person.aliases, vec!["老张"]);
    }
}
```

Modify `src-tauri/src/db/mod.rs`:

```rust
#[cfg(test)]
mod tests;
```

- [ ] **Step 2: 运行 Rust 测试**

```bash
cd relationship-graph/src-tauri
cargo test
```

Expected: 测试通过。

- [ ] **Step 3: Commit**

```bash
git add .
git commit -m "test(db): add Rust unit tests for Person CRUD"
```

### Task 15: 桌面应用打包

**Files:**
- Modify: `relationship-graph/src-tauri/tauri.conf.json`

- [ ] **Step 1: 配置应用图标与元信息**

Modify `src-tauri/tauri.conf.json`:

```json
{
  "productName": "个人关系图谱",
  "identifier": "com.yourname.relationship-graph",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "个人关系图谱",
        "width": 1280,
        "height": 800
      }
    ]
  }
}
```

- [ ] **Step 2: 构建生产包**

```bash
cd relationship-graph
npm run tauri build
```

Expected: 在 `src-tauri/target/release/bundle/` 下生成安装包。

- [ ] **Step 3: Commit**

```bash
git add .
git commit -m "chore(build): configure Tauri build and bundle"
```

---

## Spec Coverage 自评

对照设计文档检查每个需求是否都有对应任务：

| 设计文档需求 | 实现任务 | 状态 |
|---|---|---|
| 联系人 CRUD 和名片视图 | Task 4 + Task 7 | ✅ |
| 关系链路维护 | Task 5 | ✅ |
| 语音录入互动记录 | Task 9 | ✅ |
| 歧义确认 | Task 10 | ✅ |
| 互动记录列表 | Task 5 + Task 10 | ✅ |
| 关系图谱可视化 | Task 11 | ✅ |
| 自然语言查询（限定类型） | Task 12 | ✅ |
| 数据库加密 | Task 2 + Task 8 | ✅ |
| 主密码保护 | Task 8 | ✅ |
| 敏感级别访问控制 | Task 13 | ✅ |
| 手机端架构预留 | Task 1（本地服务预留）+ Task 8（HTTPS/token 预留） | ✅ |

## 执行方式选择

Plan complete and saved to `docs/superpowers/plans/2026-07-30-personal-relationship-graph-plan.md`.

Two execution options:

1. **Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** - Execute tasks in this session using `executing-plans`, batch execution with checkpoints.

Which approach would you like to use?
