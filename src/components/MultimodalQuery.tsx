import { useEffect, useMemo, useRef, useState, type ClipboardEvent, type FormEvent, type KeyboardEvent } from 'react';
import { useVoiceInput } from '../hooks/useVoiceInput';
import { nlqConfirm, nlqMulti } from '../services/db';
import { parseAgentMention, withAgentMentionPrefix } from '../services/agentMention';
import { CONTACT_MANAGER_AGENT_ID, DIGITAL_AGENTS } from '../services/digitalAgents';
import { runLangGraphWorkflow } from '../services/langgraph';
import type { NlqResponse, NlqResult, NlqRouteMode } from '../types';
import DraftConfirmation from './DraftConfirmation';
import ImageOcrButton, { type ImageOcrHandle } from './ImageOcrButton';
import { NLQ_EXAMPLES } from './NaturalLanguageQuery';
import NlqResultCard from './NlqResultCard';
import PathResultDisplay from './PathResultDisplay';

interface MultimodalQueryProps {
  onPersonClick?: (personId: string) => void;
}

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
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
      setMessages((prev) => [...prev, { id: `assistant-confirm-${Date.now()}`, role: 'assistant', content: '草稿已确认，后续可以继续补充或取消。' }]);
    } catch (e) {
      console.error('确认失败:', e);
    }
  };

  const submitRelationshipQuery = async (trimmed: string, routeMode: NlqRouteMode = 'auto') => {
    try {
      setLoading(true);
      setError('');
      setChatPanelContent(null);
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
        setMessages((prev) => [...prev, { id: `assistant-${Date.now()}`, role: 'assistant', content: '已为你生成长内容，完整内容已展开在右侧面板。' }]);
      } else {
        setChatPanelContent(null);
        setMessages((prev) => [...prev, { id: `assistant-${Date.now()}`, role: 'assistant', content: reply }]);
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

  return (
    <div className="w-full space-y-4">
      <div className={`grid gap-4 ${chatPanelContent ? 'grid-cols-1 xl:grid-cols-[1.2fr_0.8fr]' : 'grid-cols-1'}`}>
        <section className="space-y-4">
          <div className="rounded-2xl border p-3" style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}>
            <p className="mb-2 text-xs" style={{ color: 'var(--text-secondary)' }}>数字人：点击头像后，会像微信一样在输入框自动插入 @</p>
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
                    onClick={() => insertAgentMention(agent.mention)}
                  >
                    <img src={agent.avatar} alt={agent.displayName} className="h-6 w-6 rounded-full" />
                    <span>{agent.displayName}</span>
                  </button>
                );
              })}
            </div>
          </div>

          <form
            onSubmit={(event) => {
              event.preventDefault();
              void submit(event);
            }}
          >
            <div className="rounded-2xl border shadow-sm transition focus-within:ring-2" style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}>
              <textarea
                ref={textareaRef}
                rows={2}
                className="w-full resize-none rounded-2xl bg-transparent px-4 pb-2 pt-4 text-base outline-none"
                style={{ color: 'var(--text-primary)' }}
                placeholder="你可以直接问问题；若要维护联系人，可点头像或输入 @联系人管家。（Enter 提交，Shift+Enter 换行）"
                value={query}
                disabled={busy}
                onChange={(event) => setQuery(event.target.value)}
                onKeyDown={handleKeyDown}
                onPaste={handlePaste}
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
                    onClick={() => voice.toggle()}
                  >
                    {voice.recording ? '⏹' : '🎤'}
                  </button>
                  {voice.recording && (
                    <button type="button" className="text-xs font-medium text-red-600" onClick={voice.stop}>
                      停止
                    </button>
                  )}
                  <ImageOcrButton ref={ocrRef} onText={appendText} disabled={busy} />
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

          {voice.recording && (
            <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>
              正在聆听{voice.interimText ? `：${voice.interimText}` : '（通过 MediaRecorder 录音，停止后上传转写）...'}
            </p>
          )}
          {voice.transcribing && <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>录音上传转写中...</p>}
          {voice.error && <p className="rounded bg-amber-50 p-3 text-sm text-amber-700">{voice.error}</p>}

          <div className="flex flex-wrap justify-center gap-2">
            {NLQ_EXAMPLES.map((example) => (
              <button
                key={example}
                type="button"
                className="rounded-full px-3 py-1 text-xs transition"
                style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }}
                onClick={() => applyRelationshipExample(example)}
              >
                {example}
              </button>
            ))}
          </div>

          {error && <p className="rounded bg-red-50 p-3 text-sm text-red-700">{error}</p>}

          <div className="space-y-3 rounded-2xl border p-3" style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}>
            {messages.length === 0 ? (
              <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>先输入一句话；若要联系人维护，请先 @联系人管家。</p>
            ) : (
              messages.map((message) => (
                <div key={message.id} className={`rounded-xl border p-3 ${message.role === 'user' ? 'ml-8' : 'mr-8'}`} style={{ borderColor: 'var(--border-color)', backgroundColor: message.role === 'user' ? 'var(--bg-secondary)' : 'var(--bg-primary)' }}>
                  <div className="mb-2 text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>{message.role === 'user' ? '你' : '助理'}</div>
                  <p className="text-sm" style={{ color: 'var(--text-primary)' }}>{message.content}</p>
                  {message.resultType === 'search' && message.results && message.results.length > 0 && (
                    <div className="mt-3 space-y-2">
                      {message.results.map((result) => (
                        <NlqResultCard key={result.personId} result={result} onPersonClick={onPersonClick} />
                      ))}
                    </div>
                  )}
                  {message.resultType === 'path' && message.response && <PathResultDisplay path={(message.response as Extract<NlqResponse, { intentType: 'findPath' }>).path} onPersonClick={onPersonClick} />}
                  {message.resultType === 'draft' && message.response && <DraftConfirmation response={message.response as Extract<NlqResponse, { intentType: 'createPersonDraft' | 'updatePersonDraft' | 'addInteractionDraft' }>} onConfirm={handleConfirm} onCancel={() => setChatPanelContent(null)} />}
                </div>
              ))
            )}
            <div ref={messagesEndRef} />
          </div>
        </section>

        {chatPanelContent && (
          <aside className="space-y-4">
            <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
              <h3 className="font-semibold">长内容面板</h3>
              <p className="mt-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
                当助理判断回复较长（如文章/邮件/方案）时，会自动在右侧展开完整内容。
              </p>
              <p className="mt-3 whitespace-pre-wrap text-sm" style={{ color: 'var(--text-primary)' }}>{chatPanelContent}</p>
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
