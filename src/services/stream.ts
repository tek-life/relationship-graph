/**
 * 流式聊天客户端（SSE）
 *
 * 契约（与后端逐字一致）：
 * POST /api/chat/stream，Bearer 鉴权，请求体 {"query": string}（显式指定数字人时附带 {"agentId": string}），
 * 响应 text/event-stream，事件格式 `event: <name>\ndata: <json>\n\n`：
 * - step：{"stage":"routing|llm_call","detail":"..."}
 * - thinking_delta：{"text":"..."}（模型推理增量）
 * - text_delta：{"text":"..."}（回答增量）
  * - done：{"usage":{"input":N,"output":N}|null,"backend":"rig"|"cloud"|"legacy"}
 * - error：{"message":"..."}
 *
 * 实现方式：fetch + ReadableStream 手工解析 SSE（按 \n\n 分帧，
 * 处理 event:/data: 行），不依赖 EventSource（EventSource 不支持 POST）。
 */
import { API_BASE, getToken, clearToken, AUTH_EXPIRED_EVENT } from './api';

export type StreamStage = 'routing' | 'llm_call' | 'web_search';

export interface StreamStep {
  stage: StreamStage;
  detail: string;
}

export interface StreamDone {
  usage: { input: number; output: number } | null;
  backend: 'rig' | 'cloud' | 'legacy';
}

export interface StreamDocument {
  fileName: string;
  content: string;
}

export interface StreamChatOptions {
  /** 是否开启联网搜索（缺省 false） */
  webSearch?: boolean;
  /** 随请求提交的文档附件（已在前端抽取为纯文本） */
  documents?: StreamDocument[];
}

export interface StreamChatCallbacks {
  /** step 事件：流程阶段提示（routing / llm_call） */
  onStep?: (step: StreamStep) => void;
  /** thinking_delta 事件：模型推理增量文本 */
  onThinking?: (text: string) => void;
  /** text_delta 事件：回答增量文本 */
  onText?: (text: string) => void;
  /** done 事件：生成结束（含 token 用量与后端实现标识） */
  onDone?: (done: StreamDone) => void;
  /** error 事件：服务端返回的业务错误 */
  onError?: (message: string) => void;
}

/**
 * 发起流式聊天请求，逐事件回调。
 * 网络层失败（非 AbortError）以异常抛出；业务 error 事件走 onError 回调。
 */
export async function streamChat(
  query: string,
  callbacks: StreamChatCallbacks,
  signal?: AbortSignal,
  agentId?: string,
  options?: StreamChatOptions,
): Promise<void> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    Accept: 'text/event-stream',
  };
  const token = getToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  // 请求体按需附带 webSearch / documents（后端契约：均为可选字段）
  const payload: Record<string, unknown> = { query };
  if (agentId) payload.agentId = agentId;
  if (options?.webSearch) payload.webSearch = true;
  if (options?.documents && options.documents.length > 0) payload.documents = options.documents;

  const response = await fetch(`${API_BASE}/api/chat/stream`, {
    method: 'POST',
    headers,
    body: JSON.stringify(payload),
    signal,
  });

  // 401：与 api() 保持一致的登录过期处理
  if (response.status === 401) {
    clearToken();
    sessionStorage.removeItem('rg_user');
    window.dispatchEvent(new CustomEvent(AUTH_EXPIRED_EVENT));
    if (window.location.pathname !== '/login') {
      window.location.href = '/login';
    }
    throw new Error('登录会话已过期，请重新登录。');
  }

  if (!response.ok) {
    if (response.status === 404) {
      throw new Error('当前服务端版本不支持流式接口 /api/chat/stream，请重启并升级后端服务。');
    }
    let message = `请求失败（${response.status}）`;
    try {
      const body = await response.json();
      if (body && typeof body.error === 'string') {
        message = body.error;
      }
    } catch {
      // 忽略响应体解析失败，保留状态码信息
    }
    throw new Error(message);
  }

  if (!response.body) {
    throw new Error('浏览器不支持 ReadableStream，无法接收流式响应。');
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder('utf-8');
  let buffer = '';

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });

    // 按空行分帧（兼容 \n\n 与 \r\n\r\n）
    let frameEnd = findFrameEnd(buffer);
    while (frameEnd !== -1) {
      const frame = buffer.slice(0, frameEnd.index);
      buffer = buffer.slice(frameEnd.index + frameEnd.length);
      dispatchFrame(frame, callbacks);
      frameEnd = findFrameEnd(buffer);
    }
  }

  // 流结束时冲刷残留字节与最后一帧（服务端未以空行收尾时的兜底）
  buffer += decoder.decode();
  if (buffer.trim()) {
    dispatchFrame(buffer, callbacks);
  }
}

/** 查找帧分隔符位置（优先 \n\n，兼容 \r\n\r\n） */
function findFrameEnd(buffer: string): { index: number; length: number } | -1 {
  const crlf = buffer.indexOf('\r\n\r\n');
  const lf = buffer.indexOf('\n\n');
  if (crlf !== -1 && (lf === -1 || crlf <= lf)) {
    return { index: crlf, length: 4 };
  }
  if (lf !== -1) {
    return { index: lf, length: 2 };
  }
  return -1;
}

/** 解析单帧 SSE：event: 行决定事件类型，data: 行拼接 JSON 载荷 */
function dispatchFrame(frame: string, callbacks: StreamChatCallbacks): void {
  let eventName = 'message';
  const dataLines: string[] = [];

  for (const rawLine of frame.split('\n')) {
    const line = rawLine.replace(/\r$/, '');
    if (!line || line.startsWith(':')) continue; // 空行与注释行
    if (line.startsWith('event:')) {
      eventName = line.slice('event:'.length).trim();
    } else if (line.startsWith('data:')) {
      dataLines.push(line.slice('data:'.length).trim());
    }
  }

  if (dataLines.length === 0) return;

  let data: Record<string, unknown>;
  try {
    data = JSON.parse(dataLines.join('\n')) as Record<string, unknown>;
  } catch {
    return; // 载荷非法时跳过该帧
  }

  switch (eventName) {
    case 'step':
      callbacks.onStep?.({
        stage: (data.stage as StreamStage) ?? 'llm_call',
        detail: typeof data.detail === 'string' ? data.detail : '',
      });
      break;
    case 'thinking_delta':
      callbacks.onThinking?.(typeof data.text === 'string' ? data.text : '');
      break;
    case 'text_delta':
      callbacks.onText?.(typeof data.text === 'string' ? data.text : '');
      break;
    case 'done':
      callbacks.onDone?.({
        usage: (data.usage as StreamDone['usage']) ?? null,
        backend: (data.backend as StreamDone['backend']) ?? 'legacy',
      });
      break;
    case 'error':
      callbacks.onError?.(typeof data.message === 'string' ? data.message : '生成失败');
      break;
    default:
      break; // 未知事件忽略
  }
}
