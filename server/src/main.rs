mod api;
mod data_tools;
mod db;
mod document;
mod infer;
mod llm;
mod nlq;
mod security;
mod state;
mod types;

use state::AppState;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let data_dir = std::env::var("RG_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .expect("无法定位系统数据目录")
                .join("relationship-graph")
        });
    let port: u16 = std::env::var("RG_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8790);

    let state = Arc::new(AppState::new(data_dir));

    // 注册云端 API Key 的 DB settings 层读取器（第三层来源，env > 文件 > DB）。
    // 短持锁读 settings 后立刻释放；任何失败降级为无值（不阻断聊天链路）；
    // 读取器内部不打日志，Key 不落日志。
    {
        let reader_state = state.clone();
        llm::register_cloud_key_db_reader(Box::new(move || {
            let guard = reader_state.db.lock().ok()?;
            let conn = db::get_conn(&guard).ok()?;
            db::setting::get_setting_value::<String>(conn, db::setting::KEY_CLOUD_API_KEY)
                .ok()
                .flatten()
        }));
    }

    // 注册场景模型配置的 DB 读取器（P1-7：env > DB > 默认）。短持锁读
    // model_configs 表后立刻释放；失败降级为无值（回退 env/默认，行为
    // 与改造前一致），读取器内部不打日志。
    {
        let reader_state = state.clone();
        llm::register_model_config_reader(Box::new(move |scenario| {
            let guard = reader_state.db.lock().ok()?;
            let conn = db::get_conn(&guard).ok()?;
            db::model_config::get_model(conn, scenario).ok().flatten()
        }));
    }

    // 注册 LLM usage 落库写入器（P1-7：只落 token 数/耗时等元数据，
    // 绝不落对话内容）。短持锁写入，失败记 warn 不阻断聊天链路。
    {
        let writer_state = state.clone();
        llm::register_usage_writer(Box::new(move |record| {
            let guard = match writer_state.db.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    log::warn!(target: "llm", "usage_write_skipped reason=db_lock_poisoned");
                    return;
                }
            };
            let conn = match db::get_conn(&guard) {
                Ok(conn) => conn,
                Err(_) => return, // 库未解锁/不可用时静默丢弃，不影响主链路
            };
            let insert = db::model_config::UsageInsert {
                scenario: record.scenario,
                channel: record.channel,
                model: &record.model,
                fn_name: &record.fn_name,
                prompt_tokens: record.prompt_tokens.map(|v| v as i64),
                completion_tokens: record.completion_tokens.map(|v| v as i64),
                elapsed_ms: record.elapsed_ms as i64,
            };
            if let Err(e) = db::model_config::insert_usage(conn, &insert) {
                log::warn!(target: "llm", "usage_write_failed error={}", e);
            }
        }));
    }

    // 启动自动解锁：存在密钥文件即用其打开加密库，无需任何人工解锁步骤。
    // 密钥文件缺失但库存在（老库）时保持锁定，等待 /api/auth/migrate 一次性迁移。
    match std::fs::read_to_string(state.key_file_path()) {
        Ok(key_hex) => {
            match db::crypto::open_encrypted_db(state.db_path(), key_hex.trim())
                .and_then(|conn| db::crypto::validate_encrypted_db(&conn).map(|_| conn))
            {
                Ok(conn) => {
                    if let Err(e) = db::schema::migrate(&conn) {
                        log::error!(target: "server", "startup_migrate_failed error={}", e);
                    } else {
                        let mut guard = state.db.lock().expect("db mutex poisoned");
                        *guard = Some(conn);
                        log::info!(target: "server", "startup_auto_unlock success");
                    }
                }
                Err(e) => {
                    log::error!(
                        target: "server",
                        "startup_auto_unlock_failed error={}（密钥文件与数据库不匹配，请检查数据目录）",
                        e
                    );
                }
            }
        }
        Err(_) => {
            log::info!(
                target: "server",
                "startup_no_key_file db_exists={}（老库需迁移或全新部署）",
                state.db_path().exists()
            );
        }
    }

    // MVP 阶段面向局域网开放 CORS；对公网暴露前必须收紧为可信域名白名单并启用 HTTPS
    let app = api::router(state)
        .layer(CorsLayer::very_permissive())
        .layer(TraceLayer::new_for_http());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    log::info!(target: "server", "server_start addr={}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("端口绑定失败");
    axum::serve(listener, app).await.expect("服务启动失败");
}
