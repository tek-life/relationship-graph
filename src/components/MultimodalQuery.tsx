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

    if (mentionResult.routeMode === 'relationship') {
      await submitRelationshipQuery(normalizedQuery, 'relationship');
      return;
    }

    try {
      const generalChatState = await runLangGraphWorkflow(normalizedQuery, { modeHint: 'general-chat' });
      const reply = generalChatState.assistantReply.trim() || '我暂时没生成有效回复，你可以换个说法再试一次。';
      if (isLongContent(reply)) {
        setChatPanelContent(reply);
        setPanelTitle('输出内容.md');
        setMessages((prev) => [
          ...prev,
          {
            id: `assistant-${Date.now()}`,
            role: 'assistant',
            content: '已为你生成一份较长回答，已附上 `输出内容.md`。',
            attachment: {
              title: '输出内容.md',
              content: reply,
            },
          },
        ]);
      } else {
        setChatPanelContent(null);
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

  return (
    <div className="w-full">
      <div className={`grid gap-4 ${chatPanelContent ? 'grid-cols-1 xl:grid-cols-[1.15fr_0.85fr]' : 'grid-cols-1'}`}>
        <section className="space-y-4">
          {!hasMessages ? (
            <div className="flex min-h-[56vh] flex-col justify-end">
              <div className="space-y-4">
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
          ) : (
            <div className="space-y-4">
              <AgentBar
                leadingMention={leadingMention}
                onPickMention={insertAgentMention}
                onPickExample={() => applyRelationshipExample('谁在上海做地产，和我关系比较近？')}
              />

              {error && <p className="rounded-2xl bg-red-50 p-3 text-sm text-red-700">{error}</p>}

              <div className="space-y-4">
                {messages.map((message) => (
                  <ChatBubble
                    key={message.id}
                    message={message}
                    onPersonClick={onPersonClick}
                    onShowPanel={(content, title) => {
                      setChatPanelContent(content);
                      setPanelTitle(title);
                    }}
                    onClosePanel={() => setChatPanelContent(null)}
                    onConfirm={handleConfirm}
                  />
                ))}
                <div ref={messagesEndRef} />
              </div>

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
                compact
              />
            </div>
          )}
        </section>

        {chatPanelContent && (
          <aside className="space-y-4">
            <div className="overflow-hidden rounded-3xl border shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
              <div className="flex items-center justify-between border-b px-4 py-3" style={{ borderColor: 'var(--border-color)' }}>
                <div>
                  <p className="text-xs uppercase tracking-[0.2em]" style={{ color: 'var(--text-secondary)' }}>附件</p>
                  <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>{panelTitle}</h3>
                </div>
                <span className="rounded-full bg-slate-100 px-3 py-1 text-xs text-slate-600">Markdown</span>
              </div>
              <div className="max-h-[78vh] overflow-y-auto px-5 py-4">
                <MarkdownContent content={chatPanelContent} className="space-y-4 text-[15px] leading-7" />
              </div>
            </div>
          </aside>
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
    <div className="rounded-3xl border p-3 shadow-sm" style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}>
      <div className="flex flex-wrap gap-2">
        {DIGITAL_AGENTS.map((agent) => {
          const active = leadingMention === agent.mention || agent.aliases.includes(leadingMention);
          return (
            <button
              key={agent.id}
              type="button"
              className="flex items-center gap-2 rounded-full border px-3 py-1.5 text-sm transition"
              style={{
                borderColor: active ? 'var(--accent-color)' : 'var(--border-color)',
                backgroundColor: active ? 'var(--surface-hover)' : 'var(--bg-primary)',
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
          className="rounded-full border px-3 py-1.5 text-sm transition"
          style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-primary)', color: 'var(--text-secondary)' }}
          onClick={onPickExample}
        >
          示例：@联系人管家
        </button>
      </div>
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
          className="w-full resize-none rounded-3xl bg-transparent px-4 pb-2 pt-4 text-base outline-none"
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
  return (
    <div
      className={`rounded-3xl border p-4 shadow-sm ${message.role === 'user' ? 'ml-10' : 'mr-10'}`}
      style={{
        borderColor: 'var(--border-color)',
        backgroundColor: message.role === 'user' ? 'var(--bg-secondary)' : 'var(--bg-card)',
      }}
    >
      <div className="mb-2 flex items-center justify-between">
        <span className="text-xs font-medium uppercase tracking-[0.2em]" style={{ color: 'var(--text-secondary)' }}>
          {message.role === 'user' ? '你' : '助理'}
        </span>
        {message.attachment && (
          <button
            type="button"
            className="flex items-center gap-2 rounded-full border px-3 py-1 text-xs transition hover:shadow-sm"
            style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-primary)', color: 'var(--text-secondary)' }}
            onClick={() => onShowPanel(message.attachment?.content ?? '', message.attachment?.title ?? '输出内容.md')}
          >
            <span>📎</span>
            <span>{message.attachment.title}</span>
          </button>
        )}
      </div>

      <div className="space-y-3">
        <MarkdownContent content={message.content} className="text-[15px] leading-7" />

        {message.attachment && (
          <button
            type="button"
            className="flex w-full items-center gap-3 rounded-2xl border px-4 py-3 text-left transition hover:bg-slate-50"
            style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-primary)' }}
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
          <div className="space-y-2">
            {message.results.map((result) => (
              <NlqResultCard key={result.personId} result={result} onPersonClick={onPersonClick} />
            ))}
          </div>
        )}
        {message.resultType === 'path' && message.response && <PathResultDisplay path={(message.response as Extract<NlqResponse, { intentType: 'findPath' }>).path} onPersonClick={onPersonClick} />}
        {message.resultType === 'draft' && message.response && <DraftConfirmation response={message.response as Extract<NlqResponse, { intentType: 'createPersonDraft' | 'updatePersonDraft' | 'addInteractionDraft' }>} onConfirm={onConfirm} onCancel={onClosePanel} />}
      </div>
    </div>
  );
}
