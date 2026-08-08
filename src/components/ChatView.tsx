/**
 * 统一聊天视图组件
 * 合并 MultimodalQuery + AgentWorkspace 功能，作为应用主交互界面
 * 布局：消息流 + 右侧面板 + CouncilBar + 输入框
 */
import {
  forwardRef,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
  type RefObject,
} from 'react';
import { useChat, type ChatDisplayMessage, type ChatThinking } from '../hooks/useChat';
import CouncilBar from './CouncilBar';
import SessionSidebar from './SessionSidebar';
import DraftConfirmation from './DraftConfirmation';
import NlqResultCard from './NlqResultCard';
import PathResultDisplay from './PathResultDisplay';
import MarkdownContent from './MarkdownContent';
import ImageOcrButton, { type ImageOcrHandle } from './ImageOcrButton';
import DocumentAttachButton, { type DocumentAttachment } from './DocumentAttachButton';
import { useVoiceInput } from '../hooks/useVoiceInput';
import { parseAgentMention, withAgentMentionPrefix } from '../services/agentMention';
import { isLastAssistantMessage } from '../hooks/chatMessageOps';
import { DIGITAL_AGENTS, CONTACT_MANAGER_AGENT_ID, type DigitalAgent } from '../services/digitalAgents';
import { resolveTextDisplay } from '../services/contentPolicy';
import type { NlqResponse } from '../types';

// === 示例查询 ===

const NLQ_EXAMPLES = [
  '谁在上海做地产，和我关系比较近？',
  '上次聊过融资的人里，还没跟进的有谁？',
  '最近3个月没联系但标记了待跟进的人有哪些？',
  '这个懂车帝的投标，谁能帮上忙？',
];

/** 文档上传 feature flag：VITE_DOC_UPLOAD='0' 时隐藏文档按钮（默认启用） */
const DOC_UPLOAD_ENABLED = import.meta.env.VITE_DOC_UPLOAD !== '0';

/** 多附件总量软限（字符数），超限拒绝添加 */
const MAX_TOTAL_DOC_CHARS = 40000;

// === Props ===

interface ChatViewProps {
  onPersonClick?: (personId: string) => void;
  /** 当前登录用户 ID，用于会话恢复记录按用户区分 */
  userId?: string;
}

// === 主组件 ===

export default function ChatView({ onPersonClick, userId }: ChatViewProps) {
  const {
    sessions,
    currentSessionId,
    messages,
    loading,
    streaming,
    error,
    createSession,
    switchSession,
    deleteSession,
    updateSessionTitle,
    sendMessage,
    stopGeneration,
    retryLast,
    regenerate,
    editAndResend,
    confirmDraft,
  } = useChat(userId);

  const [query, setQuery] = useState('');
  const [selectedAgentIds, setSelectedAgentIds] = useState<string[]>([]);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  /** 联网搜索开关（默认关，仅影响本轮请求） */
  const [webSearchOn, setWebSearchOn] = useState(false);
  /** 待发送的文档附件（不注入 textarea，随请求以 documents 字段提交） */
  const [attachments, setAttachments] = useState<DocumentAttachment[]>([]);
  /** 附件操作提示（超限拒绝等，自动消失） */
  const [attachNotice, setAttachNotice] = useState('');
  const attachNoticeTimerRef = useRef<number | null>(null);

  const showAttachNotice = useCallback((message: string) => {
    setAttachNotice(message);
    if (attachNoticeTimerRef.current) window.clearTimeout(attachNoticeTimerRef.current);
    attachNoticeTimerRef.current = window.setTimeout(() => setAttachNotice(''), 4000);
  }, []);

  useEffect(
    () => () => {
      if (attachNoticeTimerRef.current) window.clearTimeout(attachNoticeTimerRef.current);
    },
    [],
  );

  // 文档抽取完成：校验总量软限后加入附件列表
  const handleAddDocument = useCallback(
    (doc: DocumentAttachment) => {
      if (attachments.some((d) => d.fileName === doc.fileName)) {
        showAttachNotice(`已存在同名附件「${doc.fileName}」，请先删除后再添加`);
        return;
      }
      const totalChars = attachments.reduce((sum, d) => sum + d.content.length, 0);
      if (totalChars + doc.content.length > MAX_TOTAL_DOC_CHARS) {
        showAttachNotice('附件总量超过 4 万字符上限，请删除部分附件后再添加');
        return;
      }
      setAttachments([...attachments, doc]);
    },
    [attachments, showAttachNotice],
  );

  const handleRemoveAttachment = useCallback((fileName: string) => {
    setAttachments((prev) => prev.filter((d) => d.fileName !== fileName));
  }, []);

  // 右侧面板状态
  const [panelContent, setPanelContent] = useState<string | null>(null);
  const [panelTitle, setPanelTitle] = useState('输出内容.md');
  const [panelMode, setPanelMode] = useState<'rendered' | 'source'>('rendered');
  const [panelFullscreen, setPanelFullscreen] = useState(false);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const ocrRef = useRef<ImageOcrHandle>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const contactManager = useMemo(
    () => DIGITAL_AGENTS.find((a) => a.id === CONTACT_MANAGER_AGENT_ID) ?? DIGITAL_AGENTS[0],
    [],
  );

  // textarea 自动增高
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 240)}px`;
  }, [query]);

  // 新消息自动滚动到底部
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // 语音输入回调
  const appendText = useCallback((text: string) => {
    setQuery((prev) => {
      const trimmedPrev = prev.replace(/\s+$/, '');
      return trimmedPrev ? `${trimmedPrev} ${text}` : text;
    });
    textareaRef.current?.focus();
  }, []);

  const voice = useVoiceInput(appendText);

  // 插入 @ 数字人 mention
  const insertAgentMention = useCallback((mention: string) => {
    setQuery((prev) => withAgentMentionPrefix(prev, mention));
    textareaRef.current?.focus();
  }, []);

  // 应用示例查询
  const applyExample = useCallback(
    (example: string) => {
      const mention = contactManager?.mention ?? '@联系人管家';
      setQuery(`${mention} ${example}`);
      textareaRef.current?.focus();
    },
    [contactManager],
  );

  // 选中数字人：插入 @ 提及并设为选中（单选）
  const handlePickAgent = useCallback(
    (agent: DigitalAgent) => {
      insertAgentMention(agent.mention);
      setSelectedAgentIds([agent.id]);
    },
    [insertAgentMention],
  );

  // 发送消息
  const handleSend = useCallback(async () => {
    const text = query.trim();
    if (!text || loading) return;
    if (voice.recording) voice.stop();

    // 解析 @ 数字人 mention
    const { agentId, cleanedQuery } = parseAgentMention(text);
    if (!cleanedQuery) {
      setQuery('');
      return;
    }

    // 构建 active agent IDs
    const activeIds = selectedAgentIds.length > 0 ? selectedAgentIds : undefined;

    // 附件标记仅用于气泡展示与持久化（不注入后端 query）；文档正文以 documents 字段提交
    const docs = attachments.length > 0 ? [...attachments] : undefined;
    const displayText = docs
      ? `${text}\n${docs.map((d) => `📎 ${d.fileName}`).join('\n')}`
      : text;

    setQuery('');
    setAttachments([]);
    // 后端只收剥离后的正文（agents.md §11.2）；气泡展示与持久化用含 @mention 的原文
    await sendMessage(cleanedQuery, agentId, activeIds, displayText, {
      webSearch: webSearchOn || undefined,
      documents: docs,
    });
  }, [query, loading, voice, selectedAgentIds, sendMessage, webSearchOn, attachments]);

  // Enter 发送，Shift+Enter 换行
  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLTextAreaElement>) => {
      if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
        event.preventDefault();
        void handleSend();
      }
    },
    [handleSend],
  );

  // 粘贴图片 OCR
  const handlePaste = useCallback(
    (event: ClipboardEvent<HTMLTextAreaElement>) => {
      const item = Array.from(event.clipboardData.items).find((it) => it.type.startsWith('image/'));
      const file = item?.getAsFile();
      if (file) {
        event.preventDefault();
        ocrRef.current?.processFile(file);
      }
    },
    [],
  );

  // 草稿确认
  const handleConfirm = useCallback(
    async (intentType: string, data: Record<string, unknown>) => {
      await confirmDraft(intentType, data);
      setPanelContent(null);
      setPanelTitle('输出内容.md');
      setPanelMode('rendered');
      setPanelFullscreen(false);
    },
    [confirmDraft],
  );

  // 会话重命名（侧边栏内联编辑提交）
  const handleRenameSession = useCallback(
    async (sessionId: string, title: string) => {
      await updateSessionTitle(sessionId, title);
    },
    [updateSessionTitle],
  );

  // 重新生成最后一条 assistant 回复（生成进行中禁用，由 ChatBubble 按钮展示）
  const handleRegenerate = useCallback(
    async (messageId: string) => {
      await regenerate(messageId);
    },
    [regenerate],
  );

  // 编辑重发 user 消息（截断后续消息后用新文本重新发送）
  const handleEditResend = useCallback(
    async (messageId: string, newText: string) => {
      await editAndResend(messageId, newText);
    },
    [editAndResend],
  );

  // 会话删除（先确认，删除当前会话后由 useChat 自动切换/回到空态）
  const handleDeleteSession = useCallback(
    async (sessionId: string) => {
      const target = sessions.find((s) => s.id === sessionId);
      const label = target?.title || '新会话';
      if (!window.confirm(`确定删除会话「${label}」吗？删除后不可恢复。`)) return;
      try {
        await deleteSession(sessionId);
      } catch {
        window.alert('删除失败，请重试');
      }
    },
    [sessions, deleteSession],
  );

  // 右侧面板操作
  const showPanel = useCallback((content: string, title: string) => {
    setPanelContent(content);
    setPanelTitle(title);
    setPanelMode('rendered');
    setPanelFullscreen(false);
  }, []);

  const closePanel = useCallback(() => {
    setPanelContent(null);
    setPanelMode('rendered');
    setPanelFullscreen(false);
  }, []);

  const busy = loading || voice.transcribing;
  const generating = loading || streaming;
  const hasMessages = messages.length > 0;
  const panelIsVisible = Boolean(panelContent);

  return (
    <div className="flex flex-col h-full w-full" style={{ minHeight: '70vh' }}>
      {/* 顶部工具栏 */}
      <div
        className="flex items-center gap-3 px-4 py-2 border-b shrink-0"
        style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}
      >
        <button
          type="button"
          onClick={() => setSidebarOpen(true)}
          className="rounded-lg p-2 transition hover:bg-gray-100"
          style={{ color: 'var(--text-secondary)' }}
          title="会话列表"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeWidth="2" d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>
        <span className="text-sm font-medium" style={{ color: 'var(--text-secondary)' }}>
          {currentSessionId
            ? sessions.find((s) => s.id === currentSessionId)?.title || '新会话'
            : '开始新对话'}
        </span>
      </div>

      {/* 会话侧边栏 */}
      <SessionSidebar
        sessions={sessions}
        currentSessionId={currentSessionId}
        onSelectSession={switchSession}
        onNewSession={createSession}
        onRenameSession={handleRenameSession}
        onDeleteSession={(sessionId) => void handleDeleteSession(sessionId)}
        isOpen={sidebarOpen}
        onClose={() => setSidebarOpen(false)}
      />

      {/* 主体区域：消息流 + 右侧面板 */}
      <div className="flex-1 flex overflow-hidden">
        <div
          className={`grid gap-0 flex-1 ${
            panelIsVisible ? 'xl:grid-cols-[minmax(0,1fr)_420px]' : 'grid-cols-1'
          }`}
        >
          {/* 消息流区域 */}
          <section className="flex flex-col min-h-0 overflow-hidden">
            <div className="flex-1 overflow-y-auto px-4 py-4">
              <div className={`mx-auto ${panelIsVisible ? 'w-full' : 'max-w-4xl'}`}>
                {hasMessages ? (
                  <div className="space-y-6">
                    {messages.map((message) => (
                      <ChatBubble
                        key={message.id}
                        message={message}
                        onPersonClick={onPersonClick}
                        onShowPanel={showPanel}
                        onClosePanel={closePanel}
                        onConfirm={handleConfirm}
                        onRetry={retryLast}
                        generating={generating}
                        canRegenerate={
                          !message.retryable &&
                          !message.streaming &&
                          isLastAssistantMessage(messages, message.id)
                        }
                        onRegenerate={handleRegenerate}
                        onEditResend={handleEditResend}
                      />
                    ))}
                    <div ref={messagesEndRef} />
                  </div>
                ) : (
                  <EmptyState
                    onApplyExample={applyExample}
                    onPickMention={insertAgentMention}
                  />
                )}
              </div>
            </div>

            {/* 错误提示 */}
            {error && (
              <div className="px-4 pb-2">
                <p className="rounded-2xl bg-red-50 px-4 py-3 text-sm text-red-700">{error}</p>
              </div>
            )}

            {/* 输入区域：幕僚团与输入框共用同一容器，保证左右边缘与垂直方向精确对齐 */}
            <div className="px-4 pb-4 pt-2 shrink-0">
              <div className={`${panelIsVisible ? '' : 'mx-auto max-w-4xl'}`}>
                <CouncilBar selectedAgentIds={selectedAgentIds} onPickAgent={handlePickAgent} />
                <Composer
                  ref={textareaRef}
                  query={query}
                  busy={busy}
                  loading={loading}
                  streaming={streaming}
                  voice={voice}
                  onChange={setQuery}
                  onKeyDown={handleKeyDown}
                  onPaste={handlePaste}
                  onSubmit={handleSend}
                  onToggleVoice={() => voice.toggle()}
                  onStopVoice={voice.stop}
                  onStopGeneration={stopGeneration}
                  onOcrText={appendText}
                  ocrRef={ocrRef}
                  webSearchOn={webSearchOn}
                  onToggleWebSearch={() => setWebSearchOn((prev) => !prev)}
                  docUploadEnabled={DOC_UPLOAD_ENABLED}
                  onAddDocument={handleAddDocument}
                  attachments={attachments}
                  onRemoveAttachment={handleRemoveAttachment}
                  attachNotice={attachNotice}
                />
              </div>
            </div>
          </section>

          {/* 右侧文件面板 */}
          {panelIsVisible && (
            <FilePanel
              title={panelTitle}
              content={panelContent!}
              viewMode={panelMode}
              fullscreen={panelFullscreen}
              onViewModeChange={setPanelMode}
              onToggleFullscreen={() => setPanelFullscreen((prev) => !prev)}
              onClose={closePanel}
            />
          )}
        </div>
      </div>
    </div>
  );
}

// === 空状态组件 ===

interface EmptyStateProps {
  onApplyExample: (example: string) => void;
  onPickMention: (mention: string) => void;
}

function EmptyState({ onApplyExample, onPickMention }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center min-h-[40vh] py-12">
      <h2
        className="text-2xl font-bold mb-2"
        style={{ color: 'var(--text-primary)' }}
      >
        你好，有什么可以帮你的？
      </h2>
      <p className="text-sm mb-8" style={{ color: 'var(--text-secondary)' }}>
        直接输入问题，或 @ 数字人开始专项工作
      </p>

      {/* 数字人快捷入口 */}
      <div className="flex flex-wrap justify-center gap-3 mb-6">
        {DIGITAL_AGENTS.map((agent) => (
          <button
            key={agent.id}
            type="button"
            className="flex items-center gap-2 rounded-full border px-4 py-2 text-sm 
                       transition shadow-sm hover:shadow-md"
            style={{
              borderColor: 'var(--border-color)',
              backgroundColor: 'var(--bg-card)',
              color: 'var(--text-primary)',
            }}
            onClick={() => onPickMention(agent.mention)}
          >
            <img
              src={agent.avatar}
              alt={agent.displayName}
              className="h-6 w-6 rounded-full border border-white/60 shadow-sm"
            />
            <span>{agent.displayName}</span>
          </button>
        ))}
      </div>

      {/* 示例查询 */}
      <div className="flex flex-wrap justify-center gap-2 max-w-2xl">
        {NLQ_EXAMPLES.map((example) => (
          <button
            key={example}
            type="button"
            className="rounded-full border px-3 py-1.5 text-xs transition hover:shadow-sm"
            style={{
              borderColor: 'var(--border-color)',
              backgroundColor: 'var(--bg-card)',
              color: 'var(--text-secondary)',
            }}
            onClick={() => onApplyExample(example)}
          >
            {example}
          </button>
        ))}
      </div>
    </div>
  );
}

// === 消息气泡组件 ===

interface ChatBubbleProps {
  message: ChatDisplayMessage;
  onPersonClick?: (personId: string) => void;
  onShowPanel: (content: string, title: string) => void;
  onClosePanel: () => void;
  onConfirm: (intentType: string, data: Record<string, unknown>) => Promise<void>;
  onRetry: () => Promise<void>;
  /** 生成进行中（loading 或 streaming），期间禁用重新生成/编辑重发 */
  generating: boolean;
  /** 是否为最后一条 assistant 文本消息（仅此条展示「重新生成」） */
  canRegenerate: boolean;
  onRegenerate: (messageId: string) => Promise<void>;
  onEditResend: (messageId: string, newText: string) => Promise<void>;
}

function ChatBubble({
  message,
  onPersonClick,
  onShowPanel,
  onClosePanel,
  onConfirm,
  onRetry,
  generating,
  canRegenerate,
  onRegenerate,
  onEditResend,
}: ChatBubbleProps) {
  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';

  // user 消息编辑态：进入后替换气泡内容为 textarea，提交后走编辑重发
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');

  const startEdit = useCallback(() => {
    setDraft(message.content);
    setEditing(true);
  }, [message.content]);

  const submitEdit = useCallback(async () => {
    const text = draft.trim();
    if (!text) return;
    setEditing(false);
    await onEditResend(message.id, text);
  }, [draft, message.id, onEditResend]);

  const handleEditKeyDown = useCallback(
    (event: KeyboardEvent<HTMLTextAreaElement>) => {
      if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
        event.preventDefault();
        void submitEdit();
      }
    },
    [submitEdit],
  );

  // 系统消息居中显示
  if (isSystem) {
    return (
      <div className="flex justify-center">
        <div className="rounded-full px-4 py-1.5 text-xs" style={{ backgroundColor: 'var(--surface-hover, #f5f5f5)', color: 'var(--text-secondary)' }}>
          {message.content}
        </div>
      </div>
    );
  }

  const bubbleStyle = isUser
    ? { borderColor: 'rgba(148,163,184,0.35)', backgroundColor: 'rgba(255,255,255,0.92)' }
    : { borderColor: 'transparent', backgroundColor: 'transparent' };

  // 助手纯文本消息（无结构化结果、非错误气泡）：应用长内容策略与工具条
  const isAssistantText = !isUser && !message.resultType && !message.retryable;
  const fullContent = message.attachment?.content ?? message.content;
  const decision = isAssistantText ? resolveTextDisplay(fullContent) : null;
  const isCollapsible = decision?.mode === 'collapsible' && !message.attachment;

  return (
    <div className={`group flex ${isUser ? 'justify-end' : 'justify-start'}`}>
      <div className={`flex w-full max-w-3xl gap-3 ${isUser ? 'flex-row-reverse' : 'flex-row'}`}>
        {/* 头像 */}
        <div
          className="mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-full border bg-white shadow-sm"
          style={{ borderColor: 'var(--border-color)' }}
        >
          {isUser ? (
            <span className="text-sm font-semibold" style={{ color: 'var(--accent-color)' }}>
              你
            </span>
          ) : (
            <AssistantAvatar />
          )}
        </div>

        {/* 消息体 */}
        <div className={`min-w-0 flex-1 ${isUser ? 'flex justify-end' : 'flex justify-start'}`}>
          <div className="space-y-2">
            <div className="flex items-center gap-2 px-1">
              <span className="text-sm font-medium" style={{ color: 'var(--text-secondary)' }}>
                {isUser ? '你' : '助理'}
              </span>
            </div>

            <div
              className={`rounded-3xl border px-4 py-3 shadow-sm ${
                isUser ? 'max-w-[34rem]' : 'max-w-[46rem]'
              }`}
              style={bubbleStyle}
            >
              {/* user 消息编辑态：textarea + 取消/发送 */}
              {isUser && editing ? (
                <div className="space-y-2 text-left">
                  <textarea
                    autoFocus
                    rows={3}
                    className="w-full min-w-[18rem] resize-y rounded-xl border bg-transparent px-3 py-2 text-[15px] leading-7 outline-none focus:ring-1"
                    style={{ borderColor: 'var(--border-color)', color: 'var(--text-primary)' }}
                    value={draft}
                    onChange={(event) => setDraft(event.target.value)}
                    onKeyDown={handleEditKeyDown}
                  />
                  <div className="flex justify-end gap-2">
                    <button
                      type="button"
                      className="rounded-full px-3 py-1 text-xs transition hover:bg-slate-100"
                      style={{ color: 'var(--text-secondary)' }}
                      onClick={() => setEditing(false)}
                    >
                      取消
                    </button>
                    <button
                      type="button"
                      className="rounded-full bg-slate-800 px-3 py-1 text-xs font-medium text-white transition hover:bg-slate-700 disabled:cursor-not-allowed disabled:opacity-40"
                      disabled={!draft.trim()}
                      onClick={() => void submitEdit()}
                    >
                      发送
                    </button>
                  </div>
                </div>
              ) : (
                <>
              {/* 思考过程（流式中默认展开，结束后折叠） */}
              {!isUser && message.thinking && (
                <ThinkingBlock thinking={message.thinking} streaming={Boolean(message.streaming)} />
              )}

              {isCollapsible ? (
                <CollapsibleMarkdown content={message.content} />
              ) : (
                message.content && (
                  <MarkdownContent
                    content={message.content}
                    className={`text-[15px] leading-7 ${isUser ? 'text-right' : 'text-left'}`}
                  />
                )
              )}

              {/* 流式生成中且暂无内容时的占位提示 */}
              {!isUser && message.streaming && !message.content && (
                <p className="animate-pulse text-sm text-slate-400">正在生成…</p>
              )}

              {/* 错误消息的重试按钮 */}
              {message.retryable && (
                <button
                  type="button"
                  className="mt-2 rounded-full border border-red-200 bg-red-50 px-3 py-1 text-xs font-medium text-red-600 transition hover:bg-red-100"
                  onClick={() => void onRetry()}
                >
                  重试
                </button>
              )}

              {/* 联网搜索提示（仅前端本地标记） */}
              {!isUser && message.webSearched && (
                <p className="mt-1 text-xs text-slate-400">本回答可能包含联网信息</p>
              )}

              {/* 附件按钮 */}
              {message.attachment && (
                <button
                  type="button"
                  className="mt-3 flex w-full items-center gap-3 rounded-2xl border px-4 py-3 text-left transition hover:bg-slate-50"
                  style={{ borderColor: 'var(--border-color)', backgroundColor: 'rgba(255,255,255,0.75)' }}
                  onClick={() =>
                    onShowPanel(message.attachment?.content ?? '', message.attachment?.title ?? '输出内容.md')
                  }
                >
                  <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 to-violet-500 text-white">
                    📄
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
                      {message.attachment.title}
                    </p>
                    <p className="truncate text-xs" style={{ color: 'var(--text-secondary)' }}>
                      点击查看完整 Markdown 输出
                    </p>
                  </div>
                </button>
              )}

              {/* NLQ 搜索结果 */}
              {message.resultType === 'search' && message.results && message.results.length > 0 && (
                <div className="mt-3 space-y-2">
                  {message.results.map((result) => (
                    <NlqResultCard key={result.personId} result={result} onPersonClick={onPersonClick} />
                  ))}
                </div>
              )}

              {/* 路径展示 */}
              {message.resultType === 'path' && message.response && (
                <div className="mt-3">
                  <PathResultDisplay
                    path={(message.response as Extract<NlqResponse, { intentType: 'findPath' }>).path}
                    onPersonClick={onPersonClick}
                  />
                </div>
              )}

              {/* 草稿确认 */}
              {message.resultType === 'draft' && message.response && (
                <div className="mt-3">
                  <DraftConfirmation
                    response={
                      message.response as Extract<
                        NlqResponse,
                        { intentType: 'createPersonDraft' | 'updatePersonDraft' | 'deletePersonDraft' | 'addInteractionDraft' }
                      >
                    }
                    onConfirm={onConfirm}
                    onCancel={onClosePanel}
                  />
                </div>
              )}
                </>
              )}
            </div>

            {/* user 消息悬停工具条：编辑重发入口 */}
            {isUser && !editing && (
              <div className="flex justify-end px-1 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
                <ToolbarActionButton onClick={startEdit} disabled={generating}>
                  编辑
                </ToolbarActionButton>
              </div>
            )}

            {/* 助手文本消息工具条：复制原文 / 下载 .md / 在面板打开 / 重新生成 */}
            {isAssistantText && (
              <MessageToolbar
                content={fullContent}
                title={message.attachment?.title ?? '回复内容.md'}
                onShowPanel={onShowPanel}
                canRegenerate={canRegenerate}
                regenerateDisabled={generating}
                onRegenerate={() => void onRegenerate(message.id)}
              />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// === 思考过程区块（步骤条 + 推理文本） ===

interface ThinkingBlockProps {
  thinking: ChatThinking;
  streaming: boolean;
}

const STAGE_LABELS: Record<string, string> = {
  routing: '意图路由',
  llm_call: '模型调用',
  web_search: '联网搜索',
};

function ThinkingBlock({ thinking, streaming }: ThinkingBlockProps) {
  // 流式中默认展开；流结束后自动折叠
  const [expanded, setExpanded] = useState(streaming);
  useEffect(() => {
    if (!streaming) setExpanded(false);
  }, [streaming]);

  const stepCount = thinking.steps.length;
  const hasContent = stepCount > 0 || thinking.reasoning.length > 0;
  if (!streaming && !hasContent) return null;

  const label = streaming ? '思考中…' : `已思考 · ${stepCount} 个步骤`;

  return (
    <div className="mb-2 rounded-2xl bg-slate-500/5 px-3 py-2">
      <button
        type="button"
        className="flex w-full items-center gap-1.5 text-xs italic text-slate-500"
        onClick={() => setExpanded((prev) => !prev)}
      >
        {/* 图标 */}
        <svg className="h-3.5 w-3.5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M12 2v3M12 19v3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M2 12h3M19 12h3M4.9 19.1L7 17M17 7l2.1-2.1" />
          <circle cx="12" cy="12" r="4" />
        </svg>
        <span>{label}</span>
        {streaming && (
          <span className="flex gap-0.5 pl-1" aria-hidden="true">
            <span className="h-1 w-1 animate-bounce rounded-full bg-slate-400" style={{ animationDelay: '0ms' }} />
            <span className="h-1 w-1 animate-bounce rounded-full bg-slate-400" style={{ animationDelay: '150ms' }} />
            <span className="h-1 w-1 animate-bounce rounded-full bg-slate-400" style={{ animationDelay: '300ms' }} />
          </span>
        )}
        <span className={`ml-auto transition-transform ${expanded ? 'rotate-180' : ''}`} aria-hidden="true">▾</span>
      </button>

      {expanded && hasContent && (
        <div className="mt-2 space-y-2 border-l-2 border-slate-200 pl-3">
          {/* 步骤条：✓ 已完成 → ● 进行中 */}
          {stepCount > 0 && (
            <ul className="space-y-1">
              {thinking.steps.map((step, index) => {
                const isCurrent = streaming && index === stepCount - 1;
                return (
                  <li key={index} className="flex items-center gap-1.5 text-xs text-slate-500">
                    <span className={isCurrent ? 'animate-pulse text-blue-500' : 'text-emerald-500'}>
                      {isCurrent ? '●' : '✓'}
                    </span>
                    <span className="shrink-0">{STAGE_LABELS[step.stage] ?? step.stage}</span>
                    {step.detail && (
                      <span className="truncate text-slate-400">{step.detail}</span>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
          {/* 推理文本（增量追加形成打字机效果，流式中显示光标） */}
          {thinking.reasoning && (
            <p className="whitespace-pre-wrap text-xs italic leading-5 text-slate-500">
              {thinking.reasoning}
              {streaming && <span className="animate-pulse not-italic">▍</span>}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

// === 可折叠长文本（400–1500 字区间） ===

function CollapsibleMarkdown({ content }: { content: string }) {
  const [expanded, setExpanded] = useState(true);

  const preview = useMemo(() => {
    const text = content.trim();
    return text.length > 200 ? `${text.slice(0, 200)}…` : text;
  }, [content]);

  return (
    <div className="text-left">
      <MarkdownContent
        content={expanded ? content : preview}
        className="text-[15px] leading-7"
      />
      <button
        type="button"
        className="mt-2 text-xs font-medium text-blue-600 transition hover:underline"
        onClick={() => setExpanded((prev) => !prev)}
      >
        {expanded ? '收起' : '展开全文'}
      </button>
    </div>
  );
}

// === 助手文本消息工具条 ===

interface MessageToolbarProps {
  content: string;
  title: string;
  onShowPanel: (content: string, title: string) => void;
  /** 是否为最后一条 assistant 回复（仅此条展示重新生成入口） */
  canRegenerate: boolean;
  /** 生成进行中时禁用重新生成 */
  regenerateDisabled: boolean;
  onRegenerate: () => void;
}

function MessageToolbar({
  content,
  title,
  onShowPanel,
  canRegenerate,
  regenerateDisabled,
  onRegenerate,
}: MessageToolbarProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // 剪贴板不可用时静默失败
    }
  }, [content]);

  return (
    <div className="flex items-center gap-1 px-1 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
      <ToolbarActionButton onClick={() => void handleCopy()}>
        {copied ? '已复制' : '复制原文'}
      </ToolbarActionButton>
      <ToolbarActionButton onClick={() => downloadMarkdown(title, content)}>
        下载 .md
      </ToolbarActionButton>
      <ToolbarActionButton onClick={() => onShowPanel(content, title)}>
        在面板打开
      </ToolbarActionButton>
      {canRegenerate && (
        <ToolbarActionButton onClick={onRegenerate} disabled={regenerateDisabled}>
          重新生成
        </ToolbarActionButton>
      )}
    </div>
  );
}

function ToolbarActionButton({
  children,
  onClick,
  disabled,
}: {
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className="rounded-full px-2 py-0.5 text-xs transition hover:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
      style={{ color: 'var(--text-secondary)' }}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

// === 助手头像 ===

function AssistantAvatar() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <circle cx="9" cy="7.2" r="4.2" stroke="currentColor" strokeWidth="1.5" />
      <path d="M3.5 15C3.5 12.5 5.7 11 9 11C12.3 11 14.5 12.5 14.5 15" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="7.6" cy="7.1" r="0.8" fill="currentColor" />
      <circle cx="10.4" cy="7.1" r="0.8" fill="currentColor" />
    </svg>
  );
}

// === 输入组件 ===

type VoiceState = ReturnType<typeof useVoiceInput>;

interface ComposerProps {
  query: string;
  busy: boolean;
  loading: boolean;
  streaming: boolean;
  voice: VoiceState;
  onChange: (value: string) => void;
  onKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => void;
  onPaste: (event: ClipboardEvent<HTMLTextAreaElement>) => void;
  onSubmit: () => void;
  onToggleVoice: () => void;
  onStopVoice: () => void;
  onStopGeneration: () => void;
  onOcrText: (text: string) => void;
  ocrRef: RefObject<ImageOcrHandle | null>;
  webSearchOn: boolean;
  onToggleWebSearch: () => void;
  docUploadEnabled: boolean;
  onAddDocument: (doc: DocumentAttachment) => void;
  attachments: DocumentAttachment[];
  onRemoveAttachment: (fileName: string) => void;
  attachNotice: string;
}

const Composer = forwardRef<HTMLTextAreaElement, ComposerProps>(function Composer(
  {
    query,
    busy,
    loading,
    streaming,
    voice,
    onChange,
    onKeyDown,
    onPaste,
    onSubmit,
    onToggleVoice,
    onStopVoice,
    onStopGeneration,
    onOcrText,
    ocrRef,
    webSearchOn,
    onToggleWebSearch,
    docUploadEnabled,
    onAddDocument,
    attachments,
    onRemoveAttachment,
    attachNotice,
  },
  ref,
) {
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        void onSubmit();
      }}
    >
      <div
        className="rounded-3xl border shadow-sm transition focus-within:ring-2"
        style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}
      >
        {/* 文档附件 chips（发送时随请求提交，不注入输入框） */}
        {(attachments.length > 0 || attachNotice) && (
          <div className="flex flex-wrap items-center gap-2 px-4 pt-3">
            {attachments.map((doc) => (
              <span
                key={doc.fileName}
                className="flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs"
                style={{
                  borderColor: 'var(--border-color)',
                  backgroundColor: 'var(--surface-hover, #f8fafc)',
                  color: 'var(--text-secondary)',
                }}
              >
                <span aria-hidden="true">📎</span>
                <span className="max-w-[12rem] truncate">{doc.fileName}</span>
                <span className="shrink-0 text-slate-400">{doc.content.length} 字</span>
                <button
                  type="button"
                  className="shrink-0 rounded-full px-0.5 text-slate-400 transition hover:text-red-500"
                  title="移除附件"
                  onClick={() => onRemoveAttachment(doc.fileName)}
                >
                  ✕
                </button>
              </span>
            ))}
            {attachNotice && <span className="text-xs text-amber-600">{attachNotice}</span>}
          </div>
        )}
        <textarea
          ref={ref}
          rows={2}
          className="w-full resize-none rounded-3xl bg-transparent px-4 pb-2 pt-4 text-[15px] outline-none"
          style={{ color: 'var(--text-primary)' }}
          placeholder="直接开始聊天；若要维护联系人，请输入 @联系人管家。"
          value={query}
          disabled={busy}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={onKeyDown}
          onPaste={onPaste}
        />
        <div className="flex items-center justify-between px-3 pb-3">
          <div className="flex items-center gap-1">
            {/* 语音按钮 */}
            <button
              type="button"
              title={
                !voice.supported
                  ? voice.unsupportedReason || '当前环境不支持语音输入'
                  : voice.recording
                    ? '点击停止录音'
                    : '语音输入'
              }
              className={`rounded-full p-2 text-lg leading-none transition disabled:cursor-not-allowed disabled:opacity-40 ${
                voice.recording ? 'animate-pulse bg-red-100 text-red-600' : 'hover:bg-slate-100'
              }`}
              style={!voice.supported ? { color: 'var(--text-tertiary, #aaa)' } : undefined}
              disabled={loading || !voice.supported || voice.transcribing}
              onClick={onToggleVoice}
            >
              {voice.recording ? '⏹' : '🎤'}
            </button>
            {voice.recording && (
              <button
                type="button"
                className="text-xs font-medium text-red-600"
                onClick={onStopVoice}
              >
                停止
              </button>
            )}
            {/* OCR 图片识别 */}
            <ImageOcrButton ref={ocrRef} onText={onOcrText} disabled={busy} />
            {/* 文档上传（feature flag：VITE_DOC_UPLOAD !== '0'） */}
            {docUploadEnabled && (
              <DocumentAttachButton onDocument={onAddDocument} disabled={busy} />
            )}
            {/* 联网搜索开关 */}
            <button
              type="button"
              title={webSearchOn ? '联网搜索已开启，点击关闭' : '开启联网搜索（回答可能引用实时网络信息）'}
              aria-pressed={webSearchOn}
              className={`flex items-center gap-1 rounded-full px-2.5 py-1.5 text-xs font-medium transition disabled:cursor-not-allowed disabled:opacity-50 ${
                webSearchOn
                  ? 'bg-blue-100 text-blue-700'
                  : 'hover:bg-slate-100'
              }`}
              style={!webSearchOn ? { color: 'var(--text-secondary)' } : undefined}
              disabled={busy}
              onClick={onToggleWebSearch}
            >
              <svg className="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <circle cx="12" cy="12" r="10" />
                <path d="M2 12h20M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z" />
              </svg>
              联网
            </button>
          </div>
          {/* 停止生成 + 发送按钮 */}
          <div className="flex items-center gap-2">
            {loading && streaming && (
              <button
                type="button"
                className="flex items-center gap-1.5 rounded-full bg-slate-800 px-3 py-1.5 text-xs font-medium text-white transition hover:bg-slate-700"
                onClick={onStopGeneration}
              >
                <span className="inline-block h-2 w-2 bg-white" aria-hidden="true" />
                停止生成
              </button>
            )}
            <button
              type="submit"
              className="btn-primary flex items-center justify-center rounded-full p-2"
              disabled={busy || !query.trim()}
            >
            {loading ? (
              <svg className="h-5 w-5 animate-spin" viewBox="0 0 24 24" fill="none">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path
                  className="opacity-75"
                  fill="currentColor"
                  d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                />
              </svg>
            ) : (
              <svg
                className="h-5 w-5"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <line x1="22" y1="2" x2="11" y2="13" />
                <polygon points="22 2 15 22 11 13 2 9 22 2" />
              </svg>
            )}
            </button>
          </div>
        </div>
      </div>
    </form>
  );
});

// === 右侧文件面板 ===

interface FilePanelProps {
  title: string;
  content: string;
  viewMode: 'rendered' | 'source';
  fullscreen: boolean;
  onViewModeChange: (mode: 'rendered' | 'source') => void;
  onToggleFullscreen: () => void;
  onClose: () => void;
}

function FilePanel({
  title,
  content,
  viewMode,
  fullscreen,
  onViewModeChange,
  onToggleFullscreen,
  onClose,
}: FilePanelProps) {
  const lineCount = content.split('\n').length;
  const panelClasses = fullscreen ? 'fixed inset-3 z-40' : 'h-full overflow-hidden border-l';

  return (
    <aside className={panelClasses} style={{ borderColor: fullscreen ? undefined : 'var(--border-color)' }}>
      <div
        className="flex h-full flex-col overflow-hidden rounded-3xl border shadow-lg"
        style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
      >
        {/* 面板头部 */}
        <div
          className="flex items-center justify-between border-b px-4 py-3"
          style={{ borderColor: 'var(--border-color)' }}
        >
          <div className="min-w-0">
            <p className="text-xs uppercase tracking-[0.2em]" style={{ color: 'var(--text-secondary)' }}>
              附件 · {lineCount} 行
            </p>
            <h3 className="truncate font-semibold" style={{ color: 'var(--text-primary)' }}>
              {title}
            </h3>
          </div>
          <div className="flex items-center gap-2">
            <PanelButton onClick={() => downloadMarkdown(title, content)}>下载</PanelButton>
            <PanelButton onClick={() => onViewModeChange(viewMode === 'rendered' ? 'source' : 'rendered')}>
              {viewMode === 'rendered' ? '看源码' : '渲染'}
            </PanelButton>
            <PanelButton onClick={onToggleFullscreen}>
              {fullscreen ? '退出全屏' : '全屏'}
            </PanelButton>
            <PanelButton onClick={onClose}>关闭</PanelButton>
          </div>
        </div>

        {/* 面板内容 */}
        <div
          className={`min-h-0 flex-1 overflow-y-auto ${
            viewMode === 'rendered' ? 'px-5 py-4' : 'bg-slate-950 px-0 py-0'
          }`}
        >
          {viewMode === 'rendered' ? (
            <MarkdownContent content={content} className="space-y-4 text-[15px] leading-7" />
          ) : (
            <SourceView content={content} />
          )}
        </div>
      </div>
    </aside>
  );
}

function PanelButton({ children, onClick }: { children: React.ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      className="rounded-full border px-3 py-1.5 text-xs transition hover:bg-gray-50"
      style={{ borderColor: 'var(--border-color)', color: 'var(--text-secondary)' }}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function SourceView({ content }: { content: string }) {
  const lines = content.split('\n');
  return (
    <pre className="h-full overflow-x-auto p-4 text-sm text-slate-100">
      <code>
        {lines.map((line, index) => (
          <div key={index} className="grid grid-cols-[3rem_minmax(0,1fr)] gap-3">
            <span className="select-none text-right text-slate-500">{index + 1}</span>
            <span className="whitespace-pre-wrap break-words">{line || ' '}</span>
          </div>
        ))}
      </code>
    </pre>
  );
}

function downloadMarkdown(filename: string, content: string) {
  const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename.endsWith('.md') ? filename : `${filename}.md`;
  anchor.click();
  URL.revokeObjectURL(url);
}
