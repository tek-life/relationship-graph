/**
 * 聊天会话管理 hook
 * 提供会话 CRUD、消息发送/接收、会话切换等核心聊天功能
 * 整合 chatRouter 路由分发与后端 session API
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { apiGet, apiPost, apiDelete } from '../services/api';
import { routeQuery } from '../services/chatRouter';
import { nlqConfirm } from '../services/db';
import type {
  Session,
  ChatMessage,
  ChatRouterResponse,
  NlqResponse,
  NlqResult,
} from '../types';

// === 前端聊天消息（含 UI 渲染所需的富类型） ===

export interface ChatDisplayMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  /** 附件（长文档展示在右侧面板） */
  attachment?: {
    title: string;
    content: string;
  };
  /** NLQ 结果类型 */
  resultType?: 'search' | 'draft' | 'path';
  /** NLQ 搜索结果列表 */
  results?: NlqResult[];
  /** 完整 NLQ 响应 */
  response?: NlqResponse;
  /** 路由响应 */
  routerResponse?: ChatRouterResponse;
}

export interface UseChatReturn {
  /** 所有会话列表 */
  sessions: Session[];
  /** 当前会话 ID */
  currentSessionId: string | null;
  /** 当前会话的消息列表 */
  messages: ChatDisplayMessage[];
  /** 是否正在发送消息 */
  loading: boolean;
  /** 错误信息 */
  error: string;
  /** 加载会话列表 */
  loadSessions: () => Promise<void>;
  /** 创建新会话 */
  createSession: () => Promise<string>;
  /** 切换当前会话 */
  switchSession: (sessionId: string) => Promise<void>;
  /** 删除会话 */
  deleteSession: (sessionId: string) => Promise<void>;
  /** 发送消息 */
  sendMessage: (
    text: string,
    agentId?: string | null,
    activeAgentIds?: string[],
  ) => Promise<void>;
  /** 确认草稿 */
  confirmDraft: (intentType: string, data: Record<string, unknown>) => Promise<void>;
}

// === 后端 API 调用封装 ===

async function fetchSessions(): Promise<Session[]> {
  try {
    return await apiGet<Session[]>('/api/sessions');
  } catch {
    return [];
  }
}

async function createSessionApi(title?: string): Promise<Session> {
  return apiPost<Session>('/api/sessions', { title });
}

async function fetchMessages(sessionId: string): Promise<ChatMessage[]> {
  try {
    return await apiGet<ChatMessage[]>(`/api/sessions/${sessionId}/messages`);
  } catch {
    return [];
  }
}

async function addMessageApi(
  sessionId: string,
  role: string,
  content: string,
  metadataJson?: string,
): Promise<ChatMessage> {
  return apiPost<ChatMessage>(`/api/sessions/${sessionId}/messages`, {
    role,
    content,
    metadataJson,
  });
}

async function deleteSessionApi(sessionId: string): Promise<void> {
  await apiDelete(`/api/sessions/${sessionId}`);
}

// === 工具函数 ===

/** 将后端 ChatMessage 转换为前端显示消息 */
function toDisplayMessage(msg: ChatMessage): ChatDisplayMessage {
  let metadata: Record<string, unknown> | null = null;
  if (msg.metadataJson) {
    try {
      metadata = JSON.parse(msg.metadataJson);
    } catch {
      // 忽略解析失败的 metadata
    }
  }

  return {
    id: msg.id,
    role: msg.role as 'user' | 'assistant',
    content: msg.content,
    resultType: metadata?.resultType as ChatDisplayMessage['resultType'],
    results: metadata?.results as NlqResult[] | undefined,
    response: metadata?.response as NlqResponse | undefined,
  };
}

/** 判断是否为长内容（需要右侧面板展示） */
function isLongContent(reply: string): boolean {
  const normalized = reply.trim();
  if (normalized.length >= 220) return true;
  if (normalized.split('\n').length >= 6) return true;
  return false;
}

/** 拆分长回复为预览 + 详情 */
function splitAssistantReply(reply: string): { preview: string; detail: string; attachmentTitle: string } {
  const normalized = reply.trim();
  const paragraphs = normalized.split(/\n{2,}/).map((p) => p.trim()).filter(Boolean);

  if (paragraphs.length > 1) {
    return {
      preview: paragraphs[0],
      detail: paragraphs.slice(1).join('\n\n').trim(),
      attachmentTitle: '输出内容.md',
    };
  }

  const cut = Math.min(Math.max(220, 120), normalized.length);
  return {
    preview: normalized.slice(0, cut).trim() || normalized,
    detail: normalized.slice(cut).trim(),
    attachmentTitle: '输出内容.md',
  };
}

// === Hook 实现 ===

export function useChat(): UseChatReturn {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [currentSessionId, setCurrentSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatDisplayMessage[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const currentSessionRef = useRef<string | null>(null);

  // 保持 ref 同步
  useEffect(() => {
    currentSessionRef.current = currentSessionId;
  }, [currentSessionId]);

  // 加载会话列表
  const loadSessions = useCallback(async () => {
    const list = await fetchSessions();
    setSessions(list);
  }, []);

  // 初始加载
  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // 创建新会话
  const createSession = useCallback(async (): Promise<string> => {
    const session = await createSessionApi();
    setSessions((prev) => [session, ...prev]);
    setCurrentSessionId(session.id);
    setMessages([]);
    return session.id;
  }, []);

  // 切换会话
  const switchSession = useCallback(async (sessionId: string) => {
    setCurrentSessionId(sessionId);
    const serverMessages = await fetchMessages(sessionId);
    setMessages(serverMessages.map(toDisplayMessage));
  }, []);

  // 删除会话
  const deleteSession = useCallback(
    async (sessionId: string) => {
      await deleteSessionApi(sessionId);
      setSessions((prev) => prev.filter((s) => s.id !== sessionId));
      if (currentSessionId === sessionId) {
        setCurrentSessionId(null);
        setMessages([]);
      }
    },
    [currentSessionId],
  );

  // 发送消息
  const sendMessage = useCallback(
    async (text: string, agentId?: string | null, activeAgentIds?: string[]) => {
      // 确保有活跃会话
      let sessionId = currentSessionRef.current;
      if (!sessionId) {
        const session = await createSessionApi();
        setSessions((prev) => [session, ...prev]);
        sessionId = session.id;
        setCurrentSessionId(sessionId);
      }

      // 添加用户消息到 UI
      const userMsg: ChatDisplayMessage = {
        id: `user-${Date.now()}`,
        role: 'user',
        content: text,
      };
      setMessages((prev) => [...prev, userMsg]);
      setLoading(true);
      setError('');

      try {
        // 持久化用户消息
        await addMessageApi(sessionId!, 'user', text);

        // 通过路由分发查询
        const response = await routeQuery(text, agentId, activeAgentIds);

        // 构建助手回复消息
        const assistantMsg: ChatDisplayMessage = {
          id: `assistant-${Date.now()}`,
          role: 'assistant',
          content: response.reply,
          routerResponse: response,
        };

        // 处理 NLQ 响应
        if (response.type === 'nlq' && response.nlqResponse) {
          const nlq = response.nlqResponse;
          switch (nlq.intentType) {
            case 'searchPeople':
              assistantMsg.resultType = 'search';
              assistantMsg.results = nlq.results;
              assistantMsg.response = nlq;
              break;
            case 'findPath':
              assistantMsg.resultType = 'path';
              assistantMsg.response = nlq;
              break;
            case 'createPersonDraft':
            case 'updatePersonDraft':
            case 'addInteractionDraft':
              assistantMsg.resultType = 'draft';
              assistantMsg.response = nlq;
              break;
          }
        }

        // 处理长内容（右侧面板）
        if (response.fileContent) {
          assistantMsg.attachment = {
            title: response.fileTitle || '输出内容.md',
            content: response.fileContent,
          };
        } else if (isLongContent(response.reply)) {
          const split = splitAssistantReply(response.reply);
          assistantMsg.content = split.preview;
          if (split.detail) {
            assistantMsg.attachment = {
              title: split.attachmentTitle,
              content: split.detail,
            };
          }
        }

        setMessages((prev) => [...prev, assistantMsg]);

        // 持久化助手消息
        const metadata = assistantMsg.resultType
          ? JSON.stringify({
              resultType: assistantMsg.resultType,
              results: assistantMsg.results,
              response: assistantMsg.response,
            })
          : undefined;
        await addMessageApi(sessionId!, 'assistant', response.reply, metadata);

        // 刷新会话列表（更新排序和标题）
        await loadSessions();
      } catch (err) {
        const errMsg = err instanceof Error ? err.message : String(err);
        setError(errMsg);
        const errorMsg: ChatDisplayMessage = {
          id: `assistant-error-${Date.now()}`,
          role: 'assistant',
          content: `抱歉，处理失败：${errMsg}`,
        };
        setMessages((prev) => [...prev, errorMsg]);
      } finally {
        setLoading(false);
      }
    },
    [loadSessions],
  );

  // 确认草稿
  const confirmDraft = useCallback(
    async (intentType: string, data: Record<string, unknown>) => {
      try {
        await nlqConfirm(intentType, data);
        const confirmMsg: ChatDisplayMessage = {
          id: `assistant-confirm-${Date.now()}`,
          role: 'assistant',
          content: '草稿已确认，后续可以继续补充或取消。',
        };
        setMessages((prev) => [...prev, confirmMsg]);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [],
  );

  return {
    sessions,
    currentSessionId,
    messages,
    loading,
    error,
    loadSessions,
    createSession,
    switchSession,
    deleteSession,
    sendMessage,
    confirmDraft,
  };
}
