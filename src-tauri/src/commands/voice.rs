use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
}

#[tauri::command]
pub fn transcribe_audio(audio_path: String) -> Result<TranscriptionResult, String> {
    let started = Instant::now();
    let model_path = default_model_path()?;
    log::info!(
        target: "voice_cmd",
        "transcribe_audio_start audio_path_len={} model_ext={:?}",
        audio_path.chars().count(),
        model_path.extension().and_then(|ext| ext.to_str())
    );
    let output = Command::new("whisper-cli")
        .arg("-m")
        .arg(model_path)
        .arg("-f")
        .arg(&audio_path)
        .arg("--language")
        .arg("zh")
        .output()
        .map_err(|e| {
            log::warn!(target: "voice_cmd", "transcribe_audio_spawn_failed error={}", e);
            format!("无法执行 whisper-cli，请确认已安装并加入 PATH：{}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        log::warn!(
            target: "voice_cmd",
            "transcribe_audio_failed status={:?} stderr_len={} elapsed_ms={}",
            output.status.code(),
            stderr.chars().count(),
            started.elapsed().as_millis()
        );
        return Err(stderr);
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    log::info!(
        target: "voice_cmd",
        "transcribe_audio_success text_len={} elapsed_ms={}",
        text.chars().count(),
        started.elapsed().as_millis()
    );
    Ok(TranscriptionResult { text })
}

fn default_model_path() -> Result<PathBuf, String> {
    let path = dirs::data_dir()
        .ok_or_else(|| "无法定位系统数据目录".to_string())?
        .join("relationship-graph")
        .join("models")
        .join("ggml-base.bin");
    Ok(path)
}
