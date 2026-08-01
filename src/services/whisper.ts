// 语音转写：上传录音 Blob 到服务端 /api/voice/transcribe（Whisper/faster-whisper）。
// 鉴权与 api.ts 一致（Bearer token）；multipart 不能手动设 Content-Type，故不复用 api()。
import { API_BASE, clearToken, getToken } from './api';

export async function transcribeAudio(audio: Blob): Promise<string> {
  const form = new FormData();
  form.append('audio', audio, 'recording.webm');

  const headers: Record<string, string> = {};
  const token = getToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const started = performance.now();
  const response = await fetch(`${API_BASE}/api/voice/transcribe`, {
    method: 'POST',
    headers,
    body: form,
  });

  if (response.status === 401) {
    clearToken();
    throw new Error('登录已失效，请重新解锁后再试');
  }
  if (response.status === 501) {
    throw new Error('当前服务端未配置语音转写，请使用文字输入');
  }
  if (response.status === 413) {
    throw new Error('录音过长，超出服务端大小限制，请分段录制');
  }
  if (!response.ok) {
    let message = `语音转写失败（${response.status}）`;
    try {
      const body = await response.json();
      if (body && typeof body.message === 'string') {
        message = body.message;
      } else if (body && typeof body.error === 'string') {
        message = body.error;
      }
    } catch {
      // 忽略响应体解析失败，保留状态码信息
    }
    throw new Error(message);
  }

  const data = (await response.json()) as { text: string };
  console.info('[whisper] transcribe_done', {
    audioBytes: audio.size,
    textLength: data.text?.length ?? 0,
    elapsedMs: Math.round(performance.now() - started),
  });
  return data.text ?? '';
}
