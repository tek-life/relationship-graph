/**
 * 聊天会话管理 hook
 * 提供会话 CRUD、消息发送/接收、会话切换等核心聊天功能
 * 整合 chatRouter 路由分发与 session.ts API 客户端
 *
 * 通用聊天（默认与多智能体）走 SSE 流式接口（streamChat），
 * 联系人管家（contact_manager）保持 NLQ 同步接口不变。
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { routeQuery } from '../services/chatRouter';
import { nlqConfirm } from '../services/db';
import { resolveTextDisplay } from '../services/contentPolicy';
import { streamChat, type StreamStep } from '../services/stream';
import * as sessionApi from '../services/session';
import type {
  Session,
  ChatMessage,
  ChatRouterResponse,
  NlqResponse,
  NlqResult,
} from '../types';

// === 前端聊天消息（含 UI 渲染所需的富类型） ===

/** 模型思考过程：阶段步骤条 + 推理文本 */
export interface ChatThinking {
  steps: StreamStep[];
  reasoning: string;
}

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
  /** 模型思考过程（流式累积，历史消息从 metadata 还原） */
  thinking?: ChatThinking;
  /** 流式生成中 */
  streaming?: boolean;
  /** 错误消息（带"重试"按钮） */
  retryable?: boolean;
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
  /** 是否正在流式生成 */
  streaming: boolean;
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
  /** 更新会话标题 */
  updateSessionTitle: (sessionId: string, title: string) => Promise<void>;
  /** 发送消息 */
  sendMessage: (
    text: string,
    agentId?: string | null,
    activeAgentIds?: string[],
  ) => Promise<ChatRouterResponse | null>;
  /** 停止流式生成（保留已生成内容） */
  stopGeneration: () => void;
  /** 重试上一条失败的消息 */
  retryLast: () => Promise<void>;
  /** 确认草稿 */
  confirmDraft: (intentType: string, data: Record<string, unknown>) => Promise<void>;
}

// === 工具函数 ===

/** 附件模式下的默认标题 */
const ATTACHMENT_TITLE = '详细回复.md';

/** localStorage key 前缀：记录当前用户最后打开的会话，用于聊天页恢复 */
const LAST_SESSION_KEY_PREFIX = 'relationship-graph:last-session';

function lastSessionKey(userId?: string | null): string {
  return `${LAST_SESSION_KEY_PREFIX}:${userId || 'default'}`;
}

function readLastSessionId(userId?: string | null): string | null {
  try {
    return localStorage.getItem(lastSessionKey(userId));
  } catch {
    return null;
  }
}

/** id 为 null 时清除记录（如删除当前会话后） */
function writeLastSessionId(userId: string | null | undefined, sessionId: string | null): void {
  try {
    if (sessionId) {
      localStorage.setItem(lastSessionKey(userId), sessionId);
    } else {
      localStorage.removeItem(lastSessionKey(userId));
    }
  } catch {
    // localStorage 不可用时静默降级（不影响聊天主流程）
  }
}

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
    thinking: metadata?.thinking as ChatThinking | undefined,
  };
}

// === Hook 实现 ===

/**
 * @param userId 当前登录用户 ID，用于 localStorage 按用户区分会话恢复记录
 */
export function useChat(userId?: string | null): UseChatReturn {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [currentSessionId, setCurrentSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatDisplayMessage[]>([]);
  const [loading, setLoading] = useState(false);
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState('');
  const currentSessionRef = useRef<string | null>(null);
  /** 当前流式请求的中止控制器 */
  const abortRef = useRef<AbortController | null>(null);
  /** 最近一次请求参数（用于错误重试） */
  const lastRequestRef = useRef<{
    text: string;
    agentId?: string | null;
    activeAgentIds?: string[];
  } | null>(null);

  // 保持 ref 同步
  useEffect(() => {
    currentSessionRef.current = currentSessionId;
  }, [currentSessionId]);

  // 持久化当前会话 ID 到 localStorage（null 时清除），供下次进入聊天页恢复
  useEffect(() => {
    writeLastSessionId(userId, currentSessionId);
  }, [userId, currentSessionId]);

  // 加载会话列表（复用 session.ts）
  const loadSessions = useCallback(async () => {
    try {
      const list = await sessionApi.listSessions();
      setSessions(list);
    } catch {
      setSessions([]);
    }
  }, []);

  // 初始加载 + 会话恢复：
  // 1. 优先恢复 localStorage 中记录的会话（校验其仍存在）
  // 2. 已保存会话失效时降级为最近更新的会话
  // 3. 无任何会话时保持空态（首次发消息时惰性创建）
  useEffect(() => {
    let cancelled = false;
    (async () => {
      let list: Session[] = [];
      try {
        list = await sessionApi.listSessions();
      } catch {
        list = [];
      }
      if (cancelled) return;
      setSessions(list);

      const savedId = readLastSessionId(userId);
      const restored =
        savedId && list.some((s) => s.id === savedId) ? savedId : list[0]?.id ?? null;
      if (!restored) return;

      setCurrentSessionId(restored);
      currentSessionRef.current = restored;
      let serverMessages: ChatMessage[] = [];
      try {
        serverMessages = await sessionApi.getSessionMessages(restored);
      } catch {
        serverMessages = [];
      }
      if (!cancelled && currentSessionRef.current === restored) {
        setMessages(serverMessages.map(toDisplayMessage));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [userId]);

  // 创建新会话
  const createSession = useCallback(async (): Promise<string> => {
    const session = await sessionApi.createSession();
    setSessions((prev) => [session, ...prev]);
    setCurrentSessionId(session.id);
    setMessages([]);
    return session.id;
  }, []);

  // 切换会话（若正在流式生成则先中止，避免旧流污染新会话的消息列表）
  const switchSession = useCallback(async (sessionId: string) => {
    abortRef.current?.abort();
    setCurrentSessionId(sessionId);
    currentSessionRef.current = sessionId;
    try {
      const serverMessages = await sessionApi.getSessionMessages(sessionId);
      setMessages(serverMessages.map(toDisplayMessage));
    } catch {
      setMessages([]);
    }
  }, []);

  // 删除会话；若删除的是当前会话，自动切换到剩余最近更新的会话，
  // 无剩余会话则回到空态（localStorage 由持久化 effect 自动清除）
  const deleteSession = useCallback(
    async (sessionId: string) => {
      await sessionApi.deleteSession(sessionId);
      const remaining = sessions.filter((s) => s.id !== sessionId);
      setSessions(remaining);

      if (currentSessionId === sessionId) {
        abortRef.current?.abort();
        const next = [...remaining].sort(
          (a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime(),
        )[0];
        if (next) {
          await switchSession(next.id);
        } else {
          setCurrentSessionId(null);
          currentSessionRef.current = null;
          setMessages([]);
        }
      }
    },
    [sessions, currentSessionId, switchSession],
  );

  // 更新会话标题
  const updateSessionTitle = useCallback(
    async (sessionId: string, title: string) => {
      await sessionApi.updateSessionTitle(sessionId, title);
      // 更新本地列表中的标题
      setSessions((prev) =>
        prev.map((s) => (s.id === sessionId ? { ...s, title } : s)),
      );
    },
    [],
  );

  // === NLQ 同步分支（联系人管家） ===
  const runNlqRequest = useCallback(
    async (
      sessionId: string,
      text: string,
      agentId: string | null | undefined,
      activeAgentIds: string[] | undefined,
    ): Promise<ChatRouterResponse> => {
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

      // 长内容统一走 contentPolicy（结构化结果维持原渲染不变）
      if (response.fileContent) {
        assistantMsg.attachment = {
          title: response.fileTitle || ATTACHMENT_TITLE,
          content: response.fileContent,
        };
      } else if (!assistantMsg.resultType && response.type === 'chat') {
        const decision = resolveTextDisplay(response.reply);
        if (decision.mode === 'attachment') {
          assistantMsg.content = decision.summary ?? response.reply;
          assistantMsg.attachment = {
            title: ATTACHMENT_TITLE,
            content: response.reply,
          };
        }
        // collapsible / inline 模式由 ChatBubble 按 contentPolicy 渲染
      }

      setMessages((prev) => [...prev, assistantMsg]);

      // 持久化助手消息到后端
      const metadata = assistantMsg.resultType
        ? JSON.stringify({
            resultType: assistantMsg.resultType,
            results: assistantMsg.results,
            response: assistantMsg.response,
          })
        : undefined;
      await sessionApi.addMessage(sessionId, 'assistant', response.reply, metadata);

      // 刷新会话列表（更新排序和标题）
      await loadSessions();

      return response;
    },
    [loadSessions],
  );

  // === 流式聊天分支（默认与多智能体） ===
  const runStreamRequest = useCallback(
    async (sessionId: string, text: string, activeAgentIds?: string[]) => {
      // 多智能体协同模式：在消息中附带多 agent 上下文
      const query =
        activeAgentIds && activeAgentIds.length > 1
          ? `[Multi-Agent: ${activeAgentIds.join(', ')}] ${text}`
          : text;

      const assistantId = `assistant-${Date.now()}`;
      const steps: StreamStep[] = [];
      let reasoningBuf = '';
      let textBuf = '';
      let streamError = '';
      let aborted = false;
      let flushPending = false;

      const placeholder: ChatDisplayMessage = {
        id: assistantId,
        role: 'assistant',
        content: '',
        streaming: true,
        thinking: { steps: [], reasoning: '' },
      };
      setMessages((prev) => [...prev, placeholder]);
      setStreaming(true);

      // requestAnimationFrame 合并增量 setState，避免高频渲染
      const flush = () => {
        flushPending = false;
        setMessages((prev) =>
          prev.map((m) =>
            m.id === assistantId
              ? {
                  ...m,
                  content: textBuf,
                  thinking: { steps: [...steps], reasoning: reasoningBuf },
                }
              : m,
          ),
        );
      };
      const scheduleFlush = () => {
        if (flushPending) return;
        flushPending = true;
        requestAnimationFrame(flush);
      };

      const controller = new AbortController();
      abortRef.current = controller;

      let networkError = '';
      try {
        await streamChat(
          query,
          {
            onStep: (step) => {
              steps.push(step);
              scheduleFlush();
            },
            onThinking: (delta) => {
              reasoningBuf += delta;
              scheduleFlush();
            },
            onText: (delta) => {
              textBuf += delta;
              scheduleFlush();
            },
            onDone: () => {
              // 结束后由主流程统一落库
            },
            onError: (message) => {
              streamError = message;
            },
          },
          controller.signal,
        );
      } catch (err) {
        if (err instanceof Error && err.name === 'AbortError') {
          aborted = true;
        } else {
          networkError = err instanceof Error ? err.message : String(err);
        }
      } finally {
        abortRef.current = null;
        setStreaming(false);
      }

      // 最终同步一次 UI（rAF 可能尚未触发）
      flush();

      const failureMsg = networkError || streamError;
      const hasThinking = steps.length > 0 || reasoningBuf.length > 0;
      const thinking: ChatThinking | undefined = hasThinking
        ? { steps: [...steps], reasoning: reasoningBuf }
        : undefined;

      // 失败处理：错误气泡带"重试"按钮
      if (failureMsg) {
        setError(failureMsg);
        if (!textBuf.trim()) {
          // 无已生成内容：占位消息直接转为错误气泡
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId
                ? {
                    ...m,
                    content: `抱歉，处理失败：${failureMsg}`,
                    streaming: false,
                    retryable: true,
                    thinking,
                  }
                : m,
            ),
          );
        } else {
          // 已生成部分内容：保留内容并落库，另附错误气泡
          setMessages((prev) => [
            ...prev.map((m) =>
              m.id === assistantId ? { ...m, streaming: false, thinking } : m,
            ),
            {
              id: `assistant-error-${Date.now()}`,
              role: 'assistant' as const,
              content: `抱歉，处理失败：${failureMsg}`,
              retryable: true,
            },
          ]);
          const metadata = thinking ? JSON.stringify({ thinking }) : undefined;
          await sessionApi.addMessage(sessionId, 'assistant', textBuf, metadata);
          await loadSessions();
        }
        return;
      }

      // 用户中止且无任何内容：移除占位消息，不落库
      if (aborted && !textBuf.trim() && !hasThinking) {
        setMessages((prev) => prev.filter((m) => m.id !== assistantId));
        return;
      }

      // 长内容策略：≥1500 字转为摘要 + FilePanel 附件
      let bubbleContent = textBuf;
      let attachment: { title: string; content: string } | undefined;
      if (textBuf.trim()) {
        const decision = resolveTextDisplay(textBuf);
        if (decision.mode === 'attachment') {
          bubbleContent = decision.summary ?? textBuf;
          attachment = { title: ATTACHMENT_TITLE, content: textBuf };
        }
      }

      setMessages((prev) =>
        prev.map((m) =>
          m.id === assistantId
            ? { ...m, content: bubbleContent, attachment, streaming: false, thinking }
            : m,
        ),
      );

      // 落库：完整回复 + thinking 序列化进 metadataJson（中止时同样保留已生成内容）
      const metadata = thinking ? JSON.stringify({ thinking }) : undefined;
      await sessionApi.addMessage(sessionId, 'assistant', textBuf, metadata);
      await loadSessions();
    },
    [loadSessions],
  );

  // 发送消息
  const sendMessage = useCallback(
    async (
      text: string,
      agentId?: string | null,
      activeAgentIds?: string[],
    ): Promise<ChatRouterResponse | null> => {
      // 确保有活跃会话；createSession 失败时需要走统一的错误提示，不能静默抛出
      let sessionId = currentSessionRef.current;

      // 添加用户消息到 UI
      const userMsg: ChatDisplayMessage = {
        id: `user-${Date.now()}`,
        role: 'user',
        content: text,
      };
      setLoading(true);
      setError('');
      lastRequestRef.current = { text, agentId, activeAgentIds };

      try {
        if (!sessionId) {
          const session = await sessionApi.createSession();
          setSessions((prev) => [session, ...prev]);
          sessionId = session.id;
          setCurrentSessionId(sessionId);
        }
        setMessages((prev) => [...prev, userMsg]);
        // 持久化用户消息到后端
        await sessionApi.addMessage(sessionId!, 'user', text);

        // 联系人管家：NLQ 同步分支；其余走 SSE 流式
        if (agentId === 'contact_manager') {
          return await runNlqRequest(sessionId!, text, agentId, activeAgentIds);
        }
        await runStreamRequest(sessionId!, text, activeAgentIds);
        return null;
      } catch (err) {
        const errMsg = err instanceof Error ? err.message : String(err);
        setError(errMsg);
        const errorMsg: ChatDisplayMessage = {
          id: `assistant-error-${Date.now()}`,
          role: 'assistant',
          content: `抱歉，处理失败：${errMsg}`,
          retryable: true,
        };
        setMessages((prev) => [...prev, errorMsg]);
        return null;
      } finally {
        setLoading(false);
      }
    },
    [runNlqRequest, runStreamRequest],
  );

  // 停止流式生成（中止后保留已生成内容并按普通消息落库）
  const stopGeneration = useCallback(() => {
    abortRef.current?.abort();
  }, []);

  // 重试上一条失败的消息（复用最近一次请求参数，不重复发送用户消息）
  const retryLast = useCallback(async () => {
    const last = lastRequestRef.current;
    const sessionId = currentSessionRef.current;
    if (!last || !sessionId || loading) return;

    setLoading(true);
    setError('');
    // 移除错误气泡
    setMessages((prev) => prev.filter((m) => !m.retryable));

    try {
      if (last.agentId === 'contact_manager') {
        await runNlqRequest(sessionId, last.text, last.agentId, last.activeAgentIds);
      } else {
        await runStreamRequest(sessionId, last.text, last.activeAgentIds);
      }
    } catch (err) {
      const errMsg = err instanceof Error ? err.message : String(err);
      setError(errMsg);
      setMessages((prev) => [
        ...prev,
        {
          id: `assistant-error-${Date.now()}`,
          role: 'assistant',
          content: `抱歉，处理失败：${errMsg}`,
          retryable: true,
        },
      ]);
    } finally {
      setLoading(false);
    }
  }, [loading, runNlqRequest, runStreamRequest]);

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
    streaming,
    error,
    loadSessions,
    createSession,
    switchSession,
    deleteSession,
    updateSessionTitle,
    sendMessage,
    stopGeneration,
    retryLast,
    confirmDraft,
  };
}
