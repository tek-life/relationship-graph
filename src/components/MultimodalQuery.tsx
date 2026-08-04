import { forwardRef, useEffect, useMemo, useRef, useState, type ClipboardEvent, type FormEvent, type KeyboardEvent, type RefObject } from 'react';
import { useVoiceInput } from '../hooks/useVoiceInput';
import { nlqConfirm, nlqMulti } from '../services/db';
import { parseAgentMention, withAgentMentionPrefix } from '../services/agentMention';
import { CONTACT_MANAGER_AGENT_ID, DIGITAL_AGENTS } from '../services/digitalAgents';
import { runLangGraphWorkflow } from '../services/langgraph';
import type { NlqResponse, NlqResult, NlqRouteMode } from '../types';
import DraftConfirmation from './DraftConfirmation';
import ImageOcrButton, { type ImageOcrHandle } from './ImageOcrButton';
import NlqResultCard from './NlqResultCard';
import MarkdownContent from './MarkdownContent';
import PathResultDisplay from './PathResultDisplay';

interface MultimodalQueryProps {
  onPersonClick?: (personId: string) => void;
}

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  attachment?: {
    title: string;
    content: string;
  };
  resultType?: 'search' | 'draft' | 'path';
  results?: NlqResult[];
  response?: NlqResponse;
}

export default function MultimodalQuery({ onPersonClick }: MultimodalQueryProps) {
  const [query, setQuery] = useState('');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [chatPanelContent, setChatPanelContent] = useState<string | null>(null);
  const [panelTitle, setPanelTitle] = useState('输出内容.md');
  const [panelMode, setPanelMode] = useState<'rendered' | 'source'>('rendered');
  const [panelFullscreen, setPanelFullscreen] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const ocrRef = useRef<ImageOcrHandle>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const leadingMention = query.trim().split(/\s+/, 1)[0];

  const contactManager = useMemo(
    () => DIGITAL_AGENTS.find((agent) => agent.id === CONTACT_MANAGER_AGENT_ID) ?? DIGITAL_AGENTS[0],
    [],
  );

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 240)}px`;
  }, [query]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const appendText = (text: string) => {
    setQuery((prev) => {
      const trimmedPrev = prev.replace(/\s+$/, '');
      return trimmedPrev ? `${trimmedPrev} ${text}` : text;
    });
    textareaRef.current?.focus();
  };

  const insertAgentMention = (mention: string) => {
    setQuery((prev) => withAgentMentionPrefix(prev, mention));
    textareaRef.current?.focus();
  };

  const applyRelationshipExample = (example: string) => {
    const mention = contactManager?.mention ?? '@联系人管家';
    setQuery(`${mention} ${example}`);
    textareaRef.current?.focus();
  };

  const voice = useVoiceInput(appendText);

  const handleConfirm = async (intentType: string, data: Record<string, unknown>) => {
    try {
      await nlqConfirm(intentType, data);
      setQuery('');
      setChatPanelContent(null);
      setPanelTitle('输出内容.md');
      setPanelMode('rendered');
      setPanelFullscreen(false);
      setMessages((prev) => [
        ...prev,
        {
          id: `assistant-confirm-${Date.now()}`,
          role: 'assistant',
          content: '草稿已确认，后续可以继续补充或取消。',
        },
      ]);
    } catch (e) {
      console.error('确认失败:', e);
    }
  };

  const submitRelationshipQuery = async (trimmed: string, routeMode: NlqRouteMode = 'auto') => {
    try {
      setLoading(true);
      setError('');
      setChatPanelContent(null);
      setPanelTitle('输出内容.md');
      setPanelMode('rendered');
      setPanelFullscreen(false);
      const response = await nlqMulti(trimmed, false, routeMode);
      if (response.intentType === 'searchPeople') {
        setMessages((prev) => [
          ...prev,
          {
            id: `assistant-${Date.now()}`,
            role: 'assistant',
            content: `我按“联系人管家”路径处理了这条请求，找到 ${response.results.length} 条结果。`,
            resultType: 'search',
            results: response.results,
            response,
          },
        ]);
        if (response.results.length === 0) {
          setMessages((prev) => [...prev, { id: `assistant-empty-${Date.now()}`, role: 'assistant', content: '没有找到匹配的联系人，换个说法再试试。' }]);
        }
      } else if (response.intentType === 'findPath') {
        setMessages((prev) => [...prev, { id: `assistant-path-${Date.now()}`, role: 'assistant', content: '我规划了一条可行路径，下面给你看路径建议。', resultType: 'path', response }]);
      } else {
        setMessages((prev) => [...prev, { id: `assistant-draft-${Date.now()}`, role: 'assistant', content: '我已经生成了一条可确认的草稿，接下来你可以直接确认。', resultType: 'draft', response }]);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const submit = async (event?: FormEvent) => {
    event?.preventDefault();
    const trimmed = query.trim();
    if (!trimmed || loading) return;
    if (voice.recording) voice.stop();

    const mentionResult = parseAgentMention(trimmed);
    const normalizedQuery = mentionResult.cleanedQuery;
    if (!normalizedQuery) {
      setMessages((prev) => [
        ...prev,
        { id: `assistant-mention-empty-${Date.now()}`, role: 'assistant', content: '你已选择联系人管家，请在 @ 后输入具体内容。' },
      ]);
      return;
    }

    const userMessage: ChatMessage = { id: `user-${Date.now()}`, role: 'user', content: trimmed };
    setMessages((prev) => [...prev, userMessage]);
    setQuery('');
    setLoading(true);
    setError('');
    setChatPanelContent(null);
    setPanelTitle('输出内容.md');
    setPanelMode('rendered');
    setPanelFullscreen(false);

    if (mentionResult.routeMode === 'relationship') {
    await submitRelationshipQuery(normalizedQuery, 'relationship');
    return;
    }

    try {
      const generalChatState = await runLangGraphWorkflow(normalizedQuery, { modeHint: 'general-chat' });
      const reply = generalChatState.assistantReply.trim() || '我暂时没生成有效回复，你可以换个说法再试一次。';
      if (isLongContent(reply)) {
        const split = splitAssistantReply(reply);
        setChatPanelContent(split.detail || split.preview);
        setPanelTitle(split.attachmentTitle);
        setPanelMode('rendered');
        setPanelFullscreen(false);
        setMessages((prev) => [
          ...prev,
          {
            id: `assistant-${Date.now()}`,
            role: 'assistant',
            content: split.preview,
            attachment: split.detail
              ? {
                  title: split.attachmentTitle,
                  content: split.detail,
                }
              : undefined,
          },
        ]);
      } else {
        setChatPanelContent(null);
        setPanelTitle('输出内容.md');
        setPanelMode('rendered');
        setPanelFullscreen(false);
        setMessages((prev) => [
          ...prev,
          {
            id: `assistant-${Date.now()}`,
            role: 'assistant',
            content: reply,
          },
        ]);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      void submit();
    }
  };

  const handlePaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const item = Array.from(event.clipboardData.items).find((it) => it.type.startsWith('image/'));
    const file = item?.getAsFile();
    if (file) {
      event.preventDefault();
      ocrRef.current?.processFile(file);
    }
  };

  const busy = loading || voice.transcribing;
  const hasMessages = messages.length > 0;
  const panelContent = chatPanelContent ?? '';
  const panelIsVisible = Boolean(chatPanelContent);

  return (
    <div className="w-full">
      <div className={`grid gap-5 ${panelIsVisible ? 'xl:grid-cols-[minmax(0,1fr)_420px]' : 'grid-cols-1'}`}>
        <section className={`min-h-[72vh] ${panelIsVisible ? 'w-full' : ''}`}>
          <div className={`flex min-h-[72vh] flex-col ${panelIsVisible ? 'w-full pr-2' : 'mx-auto max-w-4xl'}`}>
            <div className="flex-1 space-y-6 py-2">
              {hasMessages ? (
                <>
                  {messages.map((message) => (
                    <ChatBubble
                      key={message.id}
                      message={message}
                      onPersonClick={onPersonClick}
                      onShowPanel={(content, title) => {
                        setChatPanelContent(content);
                        setPanelTitle(title);
                        setPanelMode('rendered');
                        setPanelFullscreen(false);
                      }}
                      onClosePanel={() => setChatPanelContent(null)}
                      onConfirm={handleConfirm}
                    />
                  ))}
                  <div ref={messagesEndRef} />
                </>
              ) : (
                <div className="min-h-[42vh]" />
              )}
            </div>

            {error && <p className="mb-3 rounded-2xl bg-red-50 px-4 py-3 text-sm text-red-700">{error}</p>}

            <div className="space-y-3 pb-2">
              <AgentBar
                leadingMention={leadingMention}
                onPickMention={insertAgentMention}
                onPickExample={() => applyRelationshipExample('谁在上海做地产，和我关系比较近？')}
              />

              <Composer
                ref={textareaRef}
                query={query}
                busy={busy}
                loading={loading}
                voice={voice}
                onChange={setQuery}
                onKeyDown={handleKeyDown}
                onPaste={handlePaste}
                onSubmit={submit}
                onToggleVoice={() => voice.toggle()}
                onStopVoice={voice.stop}
                onOcrText={appendText}
                ocrRef={ocrRef}
              />
            </div>
          </div>
        </section>

        {panelIsVisible && (
          <FilePanel
            title={panelTitle}
            content={panelContent}
            viewMode={panelMode}
            fullscreen={panelFullscreen}
            onViewModeChange={setPanelMode}
            onToggleFullscreen={() => setPanelFullscreen((prev) => !prev)}
            onClose={() => {
              setChatPanelContent(null);
              setPanelMode('rendered');
              setPanelFullscreen(false);
            }}
          />
        )}
      </div>
    </div>
  );
}

function isLongContent(reply: string): boolean {
  const normalized = reply.trim();
  if (normalized.length >= 220) return true;
  if (normalized.split('\n').length >= 6) return true;
  return false;
}

type VoiceState = ReturnType<typeof useVoiceInput>;

interface AgentBarProps {
  leadingMention: string;
  onPickMention: (mention: string) => void;
  onPickExample: () => void;
}

function AgentBar({ leadingMention, onPickMention, onPickExample }: AgentBarProps) {
  return (
    <div className="flex flex-wrap items-center gap-3 px-1">
      <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>数字人</span>
        {DIGITAL_AGENTS.map((agent) => {
          const active = leadingMention === agent.mention || agent.aliases.includes(leadingMention);
          return (
            <button
              key={agent.id}
              type="button"
              className="flex items-center gap-2 rounded-full border px-3 py-2 text-sm transition shadow-sm"
              style={{
                borderColor: active ? 'var(--accent-color)' : 'var(--border-color)',
                backgroundColor: active ? 'var(--surface-hover)' : 'var(--bg-card)',
                color: 'var(--text-primary)',
              }}
              onClick={() => onPickMention(agent.mention)}
            >
              <img src={agent.avatar} alt={agent.displayName} className="h-7 w-7 rounded-full border border-white/60 shadow-sm" />
              <span>{agent.displayName}</span>
            </button>
          );
        })}
        <button
          type="button"
          className="rounded-full border px-3 py-2 text-sm transition"
          style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)', color: 'var(--text-secondary)' }}
          onClick={onPickExample}
        >
          示例：@联系人管家
        </button>
    </div>
  );
}

interface ComposerProps {
  query: string;
  busy: boolean;
  loading: boolean;
  voice: VoiceState;
  compact?: boolean;
  onChange: (value: string) => void;
  onKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => void;
  onPaste: (event: ClipboardEvent<HTMLTextAreaElement>) => void;
  onSubmit: (event?: FormEvent) => void;
  onToggleVoice: () => void;
  onStopVoice: () => void;
  onOcrText: (text: string) => void;
  ocrRef: RefObject<ImageOcrHandle | null>;
}

const Composer = forwardRef<HTMLTextAreaElement, ComposerProps>(function Composer(
  { query, busy, loading, voice, compact = false, onChange, onKeyDown, onPaste, onSubmit, onToggleVoice, onStopVoice, onOcrText, ocrRef },
  ref,
) {
  return (
    <form onSubmit={(event) => {
      event.preventDefault();
      void onSubmit(event);
    }}>
      <div className={`rounded-3xl border shadow-sm transition focus-within:ring-2 ${compact ? 'mt-1' : ''}`} style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}>
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
            <button
              type="button"
              title={
                !voice.supported
                  ? (voice.unsupportedReason || '当前环境不支持语音输入')
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
              <button type="button" className="text-xs font-medium text-red-600" onClick={onStopVoice}>
                停止
              </button>
            )}
            <ImageOcrButton ref={ocrRef} onText={onOcrText} disabled={busy} />
          </div>
          <button type="submit" className="btn-primary flex items-center justify-center rounded-full p-2" disabled={busy || !query.trim()}>
            {loading ? (
              <svg className="h-5 w-5 animate-spin" viewBox="0 0 24 24" fill="none">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
            ) : (
              <svg className="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <line x1="22" y1="2" x2="11" y2="13" />
                <polygon points="22 2 15 22 11 13 2 9 22 2" />
              </svg>
            )}
          </button>
        </div>
      </div>
    </form>
  );
});

interface ChatBubbleProps {
  message: ChatMessage;
  onPersonClick?: (personId: string) => void;
  onShowPanel: (content: string, title: string) => void;
  onClosePanel: () => void;
  onConfirm: (intentType: string, data: Record<string, unknown>) => Promise<void>;
}

function ChatBubble({ message, onPersonClick, onShowPanel, onClosePanel, onConfirm }: ChatBubbleProps) {
  const isUser = message.role === 'user';
  const bubbleStyle = isUser
    ? { borderColor: 'rgba(148,163,184,0.35)', backgroundColor: 'rgba(255,255,255,0.92)' }
    : { borderColor: 'transparent', backgroundColor: 'transparent' };

  return (
    <div className={`flex ${isUser ? 'justify-end' : 'justify-start'}`}>
      <div className={`flex w-full max-w-3xl gap-3 ${isUser ? 'flex-row-reverse' : 'flex-row'}`}>
        <div className="mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-full border bg-white shadow-sm" style={{ borderColor: 'var(--border-color)' }}>
          {isUser ? (
            <span className="text-sm font-semibold" style={{ color: 'var(--accent-color)' }}>你</span>
          ) : (
            <AssistantAvatar />
          )}
        </div>

        <div className={`min-w-0 flex-1 ${isUser ? 'flex justify-end' : 'flex justify-start'}`}>
          <div className="space-y-2">
            <div className="flex items-center gap-2 px-1">
              <span className="text-sm font-medium" style={{ color: 'var(--text-secondary)' }}>
                {isUser ? '你' : '助理'}
              </span>
            </div>

            <div
              className={`rounded-3xl border px-4 py-3 shadow-sm ${isUser ? 'max-w-[34rem]' : 'max-w-[46rem]'}`}
              style={bubbleStyle}
            >
              <MarkdownContent content={message.content} className={`text-[15px] leading-7 ${isUser ? 'text-right' : 'text-left'}`} />

              {message.attachment && (
                <button
                  type="button"
                  className="mt-3 flex w-full items-center gap-3 rounded-2xl border px-4 py-3 text-left transition hover:bg-slate-50"
                  style={{ borderColor: 'var(--border-color)', backgroundColor: 'rgba(255,255,255,0.75)' }}
                  onClick={() => onShowPanel(message.attachment?.content ?? '', message.attachment?.title ?? '输出内容.md')}
                >
                  <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 to-violet-500 text-white">
                    📄
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>{message.attachment.title}</p>
                    <p className="truncate text-xs" style={{ color: 'var(--text-secondary)' }}>点击查看完整 Markdown 输出</p>
                  </div>
                </button>
              )}

              {message.resultType === 'search' && message.results && message.results.length > 0 && (
                <div className="mt-3 space-y-2">
                  {message.results.map((result) => (
                    <NlqResultCard key={result.personId} result={result} onPersonClick={onPersonClick} />
                  ))}
                </div>
              )}
              {message.resultType === 'path' && message.response && <div className="mt-3"><PathResultDisplay path={(message.response as Extract<NlqResponse, { intentType: 'findPath' }>).path} onPersonClick={onPersonClick} /></div>}
              {message.resultType === 'draft' && message.response && <div className="mt-3"><DraftConfirmation response={message.response as Extract<NlqResponse, { intentType: 'createPersonDraft' | 'updatePersonDraft' | 'addInteractionDraft' }>} onConfirm={onConfirm} onCancel={onClosePanel} /></div>}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

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

interface SplitAssistantReply {
  preview: string;
  detail: string;
  attachmentTitle: string;
}

function splitAssistantReply(reply: string): SplitAssistantReply {
  const normalized = reply.trim();
  const paragraphs = normalized
    .split(/\n{2,}/)
    .map((part) => part.trim())
    .filter(Boolean);

  if (paragraphs.length > 1) {
    const preview = paragraphs[0];
    const detail = paragraphs.slice(1).join('\n\n').trim();
    return {
      preview,
      detail,
      attachmentTitle: '输出内容.md',
    };
  }

  const cut = findSentenceBreak(normalized, 220);
  const preview = normalized.slice(0, cut).trim();
  const detail = normalized.slice(cut).trim();

  return {
    preview: preview || normalized,
    detail,
    attachmentTitle: '输出内容.md',
  };
}

function findSentenceBreak(text: string, targetIndex: number): number {
  const safeTarget = Math.min(Math.max(targetIndex, 120), Math.max(text.length - 1, 0));
  const windowStart = safeTarget;
  const windowEnd = Math.min(text.length, safeTarget + 120);
  const punctuation = ['。', '！', '？', '；', '\n'];

  for (const mark of punctuation) {
    const idx = text.indexOf(mark, windowStart);
    if (idx !== -1 && idx <= windowEnd) {
      return idx + 1;
    }
  }

  return Math.min(safeTarget, text.length);
}

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
  const panelClasses = fullscreen
    ? 'fixed inset-3 z-40'
    : 'space-y-4';

  return (
    <aside className={panelClasses}>
      <div
        className="flex h-full flex-col overflow-hidden rounded-3xl border shadow-lg"
        style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
      >
        <div className="flex items-center justify-between border-b px-4 py-3" style={{ borderColor: 'var(--border-color)' }}>
          <div className="min-w-0">
            <p className="text-xs uppercase tracking-[0.2em]" style={{ color: 'var(--text-secondary)' }}>
              附件 · {lineCount} 行
            </p>
            <h3 className="truncate font-semibold" style={{ color: 'var(--text-primary)' }}>
              {title}
            </h3>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              className="rounded-full border px-3 py-1.5 text-xs transition"
              style={{ borderColor: 'var(--border-color)', color: 'var(--text-secondary)' }}
              onClick={() => downloadMarkdown(title, content)}
            >
              下载
            </button>
            <button
              type="button"
              className="rounded-full border px-3 py-1.5 text-xs transition"
              style={{ borderColor: 'var(--border-color)', color: 'var(--text-secondary)' }}
              onClick={() => onViewModeChange(viewMode === 'rendered' ? 'source' : 'rendered')}
            >
              {viewMode === 'rendered' ? '看源码' : '渲染'}
            </button>
            <button
              type="button"
              className="rounded-full border px-3 py-1.5 text-xs transition"
              style={{ borderColor: 'var(--border-color)', color: 'var(--text-secondary)' }}
              onClick={onToggleFullscreen}
            >
              {fullscreen ? '退出全屏' : '全屏'}
            </button>
            <button
              type="button"
              className="rounded-full border px-3 py-1.5 text-xs transition"
              style={{ borderColor: 'var(--border-color)', color: 'var(--text-secondary)' }}
              onClick={onClose}
            >
              关闭
            </button>
          </div>
        </div>

        <div className={`min-h-0 flex-1 overflow-y-auto ${viewMode === 'rendered' ? 'px-5 py-4' : 'bg-slate-950 px-0 py-0'}`}>
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
