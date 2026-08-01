//! 语音转写端点：POST /api/voice/transcribe（multipart 字段 audio，限 10MB）。
//! 转写命令由环境变量 RG_WHISPER_CMD 配置，未配置时返回 501。
//! 日志只记录音频字节数与耗时，严禁记录转写文本内容。

use axum::extract::Multipart;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::time::Instant;
use uuid::Uuid;

/// 音频大小上限：10MB（超限返回 413）
pub const MAX_AUDIO_BYTES: usize = 10 * 1024 * 1024;

pub async fn transcribe(mut multipart: Multipart) -> Response {
    let started = Instant::now();

    // 读取 multipart 中的 audio 字段
    let mut audio: Option<(Vec<u8>, String)> = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(error) => {
                // 请求体超限等情况由 multipart 层给出对应状态码（含 413）
                let status = error.status();
                log::warn!(target: "voice_cmd", "transcribe_multipart_error status={}", status);
                return error_response(status, "音频上传解析失败");
            }
        };
        if field.name() != Some("audio") {
            continue;
        }
        let content_type = field.content_type().unwrap_or("audio/webm").to_string();
        match field.bytes().await {
            Ok(bytes) => {
                audio = Some((bytes.to_vec(), content_type));
                break;
            }
            Err(error) => {
                let status = error.status();
                log::warn!(target: "voice_cmd", "transcribe_read_error status={}", status);
                return error_response(status, "音频读取失败");
            }
        }
    }

    let Some((bytes, content_type)) = audio else {
        return error_response(StatusCode::BAD_REQUEST, "缺少 audio 字段");
    };
    if bytes.len() > MAX_AUDIO_BYTES {
        log::warn!(target: "voice_cmd", "transcribe_rejected reason=too_large bytes={}", bytes.len());
        return error_response(StatusCode::PAYLOAD_TOO_LARGE, "音频超过 10MB 限制");
    }

    let Ok(whisper_cmd) = std::env::var("RG_WHISPER_CMD") else {
        log::info!(target: "voice_cmd", "transcribe_not_configured bytes={}", bytes.len());
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(serde_json::json!({
                "error": "transcribe_not_configured",
                "message": "服务端未配置语音转写"
            })),
        )
            .into_response();
    };

    // 写入临时文件供转写命令读取，用完立即删除
    let ext = if content_type.contains("wav") { "wav" } else { "webm" };
    let temp_path = std::env::temp_dir().join(format!("rg-voice-{}.{}", Uuid::new_v4(), ext));
    if let Err(error) = std::fs::write(&temp_path, &bytes) {
        log::warn!(target: "voice_cmd", "transcribe_temp_write_failed error={}", error);
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "语音转写失败");
    }

    let output = run_whisper(&whisper_cmd, &temp_path).await;
    let _ = std::fs::remove_file(&temp_path);

    match output {
        Ok(text) => {
            log::info!(
                target: "voice_cmd",
                "transcribe_success bytes={} elapsed_ms={}",
                bytes.len(),
                started.elapsed().as_millis()
            );
            (StatusCode::OK, Json(serde_json::json!({ "text": text }))).into_response()
        }
        Err(reason) => {
            log::warn!(
                target: "voice_cmd",
                "transcribe_failed reason={} bytes={} elapsed_ms={}",
                reason,
                bytes.len(),
                started.elapsed().as_millis()
            );
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "语音转写失败")
        }
    }
}

/// 以 `{RG_WHISPER_CMD} {临时文件路径}` 方式执行命令，stdout 作为转写文本
async fn run_whisper(cmd: &str, audio_path: &std::path::Path) -> Result<String, &'static str> {
    let mut parts = cmd.split_whitespace();
    let program = parts.next().ok_or("empty_command")?;

    let output = tokio::process::Command::new(program)
        .args(parts)
        .arg(audio_path)
        .output()
        .await
        .map_err(|_| "spawn_failed")?;

    if !output.status.success() {
        return Err("non_zero_exit");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}
