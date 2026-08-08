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
