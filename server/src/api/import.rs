//! Excel/CSV 批量导入：预检（校验+查重）与提交（事务批量写入，逐行容错）。
//! 日志只记录数量与耗时，不记录姓名、电话等原文。

use crate::db::{get_conn, person};
use crate::state::SharedState;
use crate::types::CreatePersonRequest;
use axum::extract::State;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use super::{ApiError, AuthUser};

const MAX_ROWS: usize = 5000;

const STRENGTH_WHITELIST: [&str; 3] = ["strong", "medium", "weak"];
const SENSITIVITY_WHITELIST: [&str; 3] = ["low", "medium", "high"];
const STATUS_WHITELIST: [&str; 3] = ["follow-up", "active", "cold"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewRequest {
    pub rows: Vec<CreatePersonRequest>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowIssue {
    pub index: usize,
    pub reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateInfo {
    pub index: usize,
    /// exact = 姓名+电话均相同；name_only = 仅姓名相同
    pub match_type: String,
    /// db = 与库内已有联系人重复；batch = 与本批次前面的行重复
    pub source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewResponse {
    pub total: usize,
    pub valid: usize,
    pub invalid: Vec<RowIssue>,
    pub duplicates: Vec<DuplicateInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCommitRequest {
    pub rows: Vec<CreatePersonRequest>,
    #[serde(default)]
    pub skip_indices: Vec<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCommitResponse {
    pub imported: usize,
    pub skipped: usize,
    pub failed: Vec<RowIssue>,
    pub elapsed_ms: u128,
}

pub async fn preview(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<ImportPreviewRequest>,
) -> Result<Json<ImportPreviewResponse>, ApiError> {
    let owner_id = super::require_user_id(user)?;
    let started = Instant::now();
    check_row_limit(req.rows.len())?;

    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;

    // 库内已有 (name, phone) 索引（仅限归属当前用户的联系人）
    let mut db_name_phone: HashSet<(String, String)> = HashSet::new();
    let mut db_names: HashSet<String> = HashSet::new();
    {
        let mut stmt = conn
            .prepare("SELECT name, COALESCE(phone, '') FROM persons WHERE owner_id = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![owner_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (name, phone) = row.map_err(|e| e.to_string())?;
            db_names.insert(name.clone());
            db_name_phone.insert((name, phone));
        }
    }

    let mut invalid = Vec::new();
    let mut duplicates = Vec::new();
    let mut batch_name_phone: HashMap<(String, String), usize> = HashMap::new();
    let mut batch_names: HashSet<String> = HashSet::new();

    for (index, row) in req.rows.iter().enumerate() {
        let name = row.name.trim().to_string();
        if name.is_empty() {
            invalid.push(RowIssue { index, reason: "姓名为空".to_string() });
            continue;
        }
        let phone = row.phone.clone().unwrap_or_default().trim().to_string();
        let key = (name.clone(), phone.clone());

        if !phone.is_empty() && db_name_phone.contains(&key) {
            duplicates.push(DuplicateInfo { index, match_type: "exact".into(), source: "db".into() });
        } else if !phone.is_empty() && batch_name_phone.contains_key(&key) {
            duplicates.push(DuplicateInfo { index, match_type: "exact".into(), source: "batch".into() });
        } else if db_names.contains(&name) {
            duplicates.push(DuplicateInfo { index, match_type: "name_only".into(), source: "db".into() });
        } else if batch_names.contains(&name) {
            duplicates.push(DuplicateInfo { index, match_type: "name_only".into(), source: "batch".into() });
        }

        batch_name_phone.insert(key, index);
        batch_names.insert(name);
    }

    let valid = req.rows.len() - invalid.len();
    log::info!(
        target: "import_cmd",
        "import_preview_success total={} valid={} invalid={} duplicates={} elapsed_ms={}",
        req.rows.len(),
        valid,
        invalid.len(),
        duplicates.len(),
        started.elapsed().as_millis()
    );

    Ok(Json(ImportPreviewResponse { total: req.rows.len(), valid, invalid, duplicates }))
}

pub async fn commit(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<ImportCommitRequest>,
) -> Result<Json<ImportCommitResponse>, ApiError> {
    let owner_id = super::require_user_id(user)?;
    let started = Instant::now();
    check_row_limit(req.rows.len())?;

    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let skip: HashSet<usize> = req.skip_indices.into_iter().collect();

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut failed = Vec::new();

    for (index, row) in req.rows.into_iter().enumerate() {
        if skip.contains(&index) {
            skipped += 1;
            continue;
        }
        if row.name.trim().is_empty() {
            failed.push(RowIssue { index, reason: "姓名为空".to_string() });
            continue;
        }
        match person::create(&tx, &owner_id, sanitize(row)) {
            Ok(_) => imported += 1,
            Err(error) => failed.push(RowIssue { index, reason: error.to_string() }),
        }
    }

    tx.commit().map_err(|e| e.to_string())?;

    log::info!(
        target: "import_cmd",
        "import_commit_success imported={} skipped={} failed={} elapsed_ms={}",
        imported,
        skipped,
        failed.len(),
        started.elapsed().as_millis()
    );

    Ok(Json(ImportCommitResponse {
        imported,
        skipped,
        failed,
        elapsed_ms: started.elapsed().as_millis(),
    }))
}

fn check_row_limit(count: usize) -> Result<(), ApiError> {
    if count == 0 {
        return Err(ApiError::bad_request("导入数据为空"));
    }
    if count > MAX_ROWS {
        return Err(ApiError::bad_request(format!("单次最多导入 {} 行，请拆分文件", MAX_ROWS)));
    }
    Ok(())
}

/// 枚举值白名单兜底：非法值回落到默认，避免脏数据阻断整批导入
fn sanitize(mut row: CreatePersonRequest) -> CreatePersonRequest {
    row.name = row.name.trim().to_string();
    if let Some(strength) = &row.relationship_strength {
        if !STRENGTH_WHITELIST.contains(&strength.as_str()) {
            row.relationship_strength = None;
        }
    }
    if !SENSITIVITY_WHITELIST.contains(&row.sensitivity_level.as_str()) {
        row.sensitivity_level = "low".to_string();
    }
    if let Some(status) = &row.status {
        if !STATUS_WHITELIST.contains(&status.as_str()) {
            row.status = None;
        }
    }
    row
}
