mod api;
mod db;
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
