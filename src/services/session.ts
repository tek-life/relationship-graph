/**
 * 会话 API 客户端
 * 封装与后端会话相关的 CRUD 操作
 */
import { apiGet, apiPost, apiPut, apiDelete } from './api';
import type { Session, ChatMessage } from '../types';

// 从 types 重新导出 Session 和 ChatMessage，方便外部引用
export type { Session, ChatMessage } from '../types';

// === API 方法 ===

/** 获取会话列表（按 updated_at 降序） */
export async function listSessions(): Promise<Session[]> {
  return apiGet<Session[]>('/api/sessions');
}

/** 创建新会话 */
export async function createSession(title?: string): Promise<Session> {
  return apiPost<Session>('/api/sessions', { title });
}

/** 获取指定会话的消息列表 */
export async function getSessionMessages(
  sessionId: string,
  limit = 100,
  offset = 0,
): Promise<ChatMessage[]> {
  return apiGet<ChatMessage[]>(
    `/api/sessions/${sessionId}/messages?limit=${limit}&offset=${offset}`,
  );
}

/** 向指定会话追加一条消息 */
export async function addMessage(
  sessionId: string,
  role: string,
  content: string,
  metadataJson?: string,
): Promise<ChatMessage> {
  return apiPost<ChatMessage>(`/api/sessions/${sessionId}/messages`, {
    sessionId,
    role,
    content,
    metadataJson,
  });
}

/** 更新会话标题 */
export async function updateSessionTitle(
  sessionId: string,
  title: string,
): Promise<void> {
  await apiPut(`/api/sessions/${sessionId}`, { title });
}

/** 删除会话 */
export async function deleteSession(sessionId: string): Promise<void> {
  await apiDelete(`/api/sessions/${sessionId}`);
}
