// 首页多模态查询框：文字 / 语音 / 图片 OCR 三种输入共享同一 query，
// 语音与 OCR 结果只追加进输入框，由用户确认后再提交查询。
import { useEffect, useRef, useState, type ClipboardEvent, type FormEvent, type KeyboardEvent } from 'react';
import GraphView from './GraphView';
import { useVoiceInput } from '../hooks/useVoiceInput';
import { getGraphData, getPerson, listPersons, nlqConfirm, nlqMulti } from '../services/db';
import { classifyUserIntent } from '../services/intentRouter';
import type { GraphData, NlqResponse, NlqResult, Person } from '../types';
import DraftConfirmation from './DraftConfirmation';
import ImageOcrButton, { type ImageOcrHandle } from './ImageOcrButton';
import { NLQ_EXAMPLES } from './NaturalLanguageQuery';
import NlqResultCard from './NlqResultCard';
import PathResultDisplay from './PathResultDisplay';
import PersonDetail from './PersonDetail';

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

type SidePanelView = 'idle' | 'chat' | 'results' | 'summary' | 'detail' | 'graph';

export default function MultimodalQuery({ onPersonClick }: MultimodalQueryProps) {
  const [query, setQuery] = useState('');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [relationshipResults, setRelationshipResults] = useState<NlqResult[]>([]);
  const [selectedPersonId, setSelectedPersonId] = useState<string | null>(null);
  const [selectedPerson, setSelectedPerson] = useState<Person | null>(null);
  const [personsById, setPersonsById] = useState<Record<string, Person>>({});
  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], edges: [] });
  const [sidePanelView, setSidePanelView] = useState<SidePanelView>('idle');
  const [chatPanelContent, setChatPanelContent] = useState<string | null>(null);
  const [pendingIntent, setPendingIntent] = useState<ReturnType<typeof classifyUserIntent> | null>(null);
  const [pendingQuery, setPendingQuery] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const ocrRef = useRef<ImageOcrHandle>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const loadContext = async () => {
      try {
        const [people, graph] = await Promise.all([listPersons(), getGraphData()]);
        setPersonsById(Object.fromEntries(people.map((person) => [person.id, person])));
        setGraphData(graph);
      } catch (err) {
        setError(String(err));
      }
    };
    loadContext();
  }, []);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 240)}px`;
  }, [query]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useEffect(() => {
    if (!selectedPersonId) {
      setSelectedPerson(null);
      return;
    }

    const loadPerson = async () => {
      const detail = personsById[selectedPersonId] ?? (await getPerson(selectedPersonId));
      setSelectedPerson(detail ?? null);
    };
    loadPerson();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedPersonId, selectedPersonId ? personsById[selectedPersonId]?.id : undefined]);

  const appendText = (text: string) => {
    setQuery((prev) => {
      const trimmedPrev = prev.replace(/\s+$/, '');
      return trimmedPrev ? `${trimmedPrev} ${text}` : text;
    });
    textareaRef.current?.focus();
  };

  const voice = useVoiceInput(appendText);

  const showRelationshipPanel = (results: NlqResult[]) => {
    if (results.length > 1 || results.some((result) => result.realNameHidden || result.sensitivityLevel === 'high' || result.sensitivityLevel === 'medium')) {
      setSidePanelView('results');
      return;
    }
    setSidePanelView('idle');
  };

  const openPersonSummary = async (personId: string) => {
    setSelectedPersonId(personId);
    setSidePanelView('summary');
    onPersonClick?.(personId);
    if (!personsById[personId]) {
      try {
        const detail = await getPerson(personId);
        setSelectedPerson(detail ?? null);
      } catch (err) {
        setError(String(err));
      }
    }
  };

  const handleConfirm = async (intentType: string, data: Record<string, unknown>) => {
    try {
      await nlqConfirm(intentType, data);
      setQuery('');
      setSidePanelView('idle');
      setChatPanelContent(null);
      setMessages((prev) => [...prev, { id: `assistant-confirm-${Date.now()}`, role: 'assistant', content: '草稿已确认，后续可以继续补充或取消。' }]);
    } catch (e) {
      console.error('确认失败:', e);
    }
  };

  const handleIntentChoice = (mode: 'chat' | 'relationship') => {
    const queryText = pendingQuery;
    setPendingIntent(null);
    setPendingQuery('');
    if (mode === 'chat') {
      const reply = generateChatReply(queryText);
      const shouldPinToPanel = reply.shouldPinToPanel || (reply.panelContent ?? reply.content).length > 140;
      if (shouldPinToPanel) {
        setChatPanelContent(reply.panelContent ?? reply.content);
        setSidePanelView('chat');
      } else {
        setChatPanelContent(null);
      }
      setMessages((prev) => [...prev, { id: `assistant-${Date.now()}`, role: 'assistant', content: reply.content }]);
      return;
    }
    setChatPanelContent(null);
    void submitRelationshipQuery(queryText);
  };

  const submitRelationshipQuery = async (trimmed: string) => {
    try {
      setLoading(true);
      setError('');
      setChatPanelContent(null);
      const response = await nlqMulti(trimmed, false);
      if (response.intentType === 'searchPeople') {
        setRelationshipResults(response.results);
        showRelationshipPanel(response.results);
        setMessages((prev) => [
          ...prev,
          {
            id: `assistant-${Date.now()}`,
            role: 'assistant',
            content: `我按“维护/查询联系人”路径处理了这条请求，找到 ${response.results.length} 条结果。`,
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

    const userMessage: ChatMessage = { id: `user-${Date.now()}`, role: 'user', content: trimmed };
    setMessages((prev) => [...prev, userMessage]);
    setQuery('');
    setLoading(true);
    setError('');
    setRelationshipResults([]);

    const classification = classifyUserIntent(trimmed);
    if (classification.kind === 'uncertain') {
      setPendingIntent(classification);
      setPendingQuery(trimmed);
      setMessages((prev) => [...prev, { id: `assistant-clarify-${Date.now()}`, role: 'assistant', content: classification.reason }]);
      setLoading(false);
      return;
    }

    if (classification.kind === 'chat') {
      const reply = generateChatReply(trimmed);
      const shouldPinToPanel = reply.shouldPinToPanel || (reply.panelContent ?? reply.content).length > 140;
      if (shouldPinToPanel) {
        setChatPanelContent(reply.panelContent ?? reply.content);
        setSidePanelView('chat');
      } else {
        setChatPanelContent(null);
      }
      setMessages((prev) => [...prev, { id: `assistant-${Date.now()}`, role: 'assistant', content: reply.content }]);
      setLoading(false);
      return;
    }

    await submitRelationshipQuery(trimmed);
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
      <div className="grid grid-cols-1 gap-4 xl:grid-cols-[1.2fr_0.8fr]">
        <section className="space-y-4">
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
                placeholder="你可以直接聊天，或者说“谁在上海做地产，和我关系比较近？”这样进入联系人维护/查询流程。（Enter 提交，Shift+Enter 换行）"
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
                    <svg className="animate-spin h-5 w-5" viewBox="0 0 24 24" fill="none">
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

          {pendingIntent && (
            <div className="rounded-2xl border p-4" style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}>
              <p className="text-sm" style={{ color: 'var(--text-primary)' }}>{pendingIntent.reason}</p>
              <div className="mt-3 flex flex-wrap gap-2">
                <button type="button" className="rounded-full bg-blue-600 px-3 py-1.5 text-sm text-white" onClick={() => handleIntentChoice('chat')}>
                  继续聊天
                </button>
                <button type="button" className="rounded-full bg-purple-600 px-3 py-1.5 text-sm text-white" onClick={() => handleIntentChoice('relationship')}>
                  维护 / 查询联系人
                </button>
              </div>
            </div>
          )}

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
                onClick={() => setQuery(example)}
              >
                {example}
              </button>
            ))}
          </div>

          {error && <p className="rounded bg-red-50 p-3 text-sm text-red-700">{error}</p>}

          <div className="space-y-3 rounded-2xl border p-3" style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}>
            {messages.length === 0 ? (
              <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>先输入一句话，我会自动判断你是想聊天，还是想维护/查询联系人。</p>
            ) : (
              messages.map((message) => (
                <div key={message.id} className={`rounded-xl border p-3 ${message.role === 'user' ? 'ml-8' : 'mr-8'}`} style={{ borderColor: 'var(--border-color)', backgroundColor: message.role === 'user' ? 'var(--bg-secondary)' : 'var(--bg-primary)' }}>
                  <div className="mb-2 text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>{message.role === 'user' ? '你' : '助手'}</div>
                  <p className="text-sm" style={{ color: 'var(--text-primary)' }}>{message.content}</p>
                  {message.resultType === 'search' && message.results && message.results.length > 0 && (
                    <div className="mt-3 space-y-2">
                      {message.results.slice(0, 3).map((result) => (
                        <NlqResultCard key={result.personId} result={result} onPersonClick={openPersonSummary} />
                      ))}
                    </div>
                  )}
                  {message.resultType === 'path' && message.response && <PathResultDisplay path={(message.response as Extract<NlqResponse, { intentType: 'findPath' }>).path} onPersonClick={openPersonSummary} />}
                  {message.resultType === 'draft' && message.response && <DraftConfirmation response={message.response as Extract<NlqResponse, { intentType: 'createPersonDraft' | 'updatePersonDraft' | 'addInteractionDraft' }>} onConfirm={handleConfirm} onCancel={() => setSidePanelView('idle')} />}
                </div>
              ))
            )}
            <div ref={messagesEndRef} />
          </div>
        </section>

        <aside className="space-y-4">
          <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
            <h3 className="font-semibold">侧边栏</h3>
            <p className="mt-2 text-sm" style={{ color: 'var(--text-secondary)' }}>复杂联系人结果会在这里展开：你可以直接看名片、进入详情、或切到关系网络。</p>
          </div>

          {sidePanelView === 'chat' && chatPanelContent && (
            <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
              <h3 className="font-semibold">长回复</h3>
              <p className="mt-3 whitespace-pre-wrap text-sm" style={{ color: 'var(--text-secondary)' }}>{chatPanelContent}</p>
            </div>
          )}

          {sidePanelView === 'results' && relationshipResults.length > 0 && (
            <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
              <h3 className="font-semibold">联系结果</h3>
              <div className="mt-3 space-y-2">
                {relationshipResults.map((result) => (
                  <button key={result.personId} type="button" className="w-full rounded-lg border p-3 text-left" style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-secondary)' }} onClick={() => openPersonSummary(result.personId)}>
                    <div className="font-medium" style={{ color: 'var(--text-primary)' }}>{result.displayName}</div>
                    <div className="mt-1 text-sm" style={{ color: 'var(--text-secondary)' }}>{[result.company, result.title].filter(Boolean).join(' / ') || '未填写公司职位'}</div>
                  </button>
                ))}
              </div>
            </div>
          )}

          {sidePanelView === 'summary' && selectedPerson && (
            <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
              <div className="flex items-start justify-between gap-2">
                <div>
                  <h3 className="font-semibold">{selectedPerson.aliases[0] || selectedPerson.name}</h3>
                  <p className="mt-1 text-sm" style={{ color: 'var(--text-secondary)' }}>{selectedPerson.company || '未填写公司'}{selectedPerson.title ? ` · ${selectedPerson.title}` : ''}</p>
                </div>
                <span className="rounded-full px-2 py-0.5 text-xs" style={{ backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }}>{selectedPerson.status}</span>
              </div>
              <div className="mt-3 flex flex-wrap gap-2">
                <button type="button" className="rounded-full bg-blue-600 px-3 py-1.5 text-sm text-white" onClick={() => setSidePanelView('detail')}>
                  查看详情
                </button>
                <button type="button" className="rounded-full bg-purple-600 px-3 py-1.5 text-sm text-white" onClick={() => setSidePanelView('graph')}>
                  查看关系网络
                </button>
              </div>
            </div>
          )}

          {sidePanelView === 'detail' && selectedPersonId && (
            <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
              <PersonDetail
                personId={selectedPersonId}
                personsById={personsById}
                onBack={() => setSidePanelView(selectedPerson ? 'summary' : 'results')}
                onChanged={async () => {
                  const [people, graph] = await Promise.all([listPersons(), getGraphData()]);
                  setPersonsById(Object.fromEntries(people.map((person) => [person.id, person])));
                  setGraphData(graph);
                }}
                onOpenPerson={(personId) => openPersonSummary(personId)}
                onNetworkView={() => setSidePanelView('graph')}
              />
            </div>
          )}

          {sidePanelView === 'graph' && (
            <div className="rounded-2xl border p-4 shadow-sm" style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}>
              <div className="mb-3 flex items-center justify-between">
                <h3 className="font-semibold">关系网络</h3>
                {selectedPersonId && (
                  <button type="button" className="text-sm" style={{ color: 'var(--accent-color)' }} onClick={() => setSidePanelView('detail')}>
                    查看详情
                  </button>
                )}
              </div>
              <GraphView
                data={graphData}
                personsById={personsById}
                onNodeClick={(personId) => openPersonSummary(personId)}
                onRefresh={async () => {
                  const [people, graph] = await Promise.all([listPersons(), getGraphData()]);
                  setPersonsById(Object.fromEntries(people.map((person) => [person.id, person])));
                  setGraphData(graph);
                }}
                initialFocusId={selectedPersonId ?? undefined}
              />
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}

function generateChatReply(query: string): { content: string; panelContent?: string; shouldPinToPanel: boolean } {
  const normalized = query.trim().toLowerCase();
  if (normalized.includes('你好') || normalized.includes('hello')) {
    return {
      content: '你好，我现在可以帮你直接聊天，也可以切到联系人维护/查询流程。你可以直接问我“谁在上海做地产，和我关系比较近？”',
      shouldPinToPanel: false,
    };
  }
  if (normalized.includes('总结') || normalized.includes('最近')) {
    return {
      content: '我先按通用聊天理解：你可以把想法直接说给我，我会帮你整理成下一步。',
      shouldPinToPanel: false,
    };
  }

  const longFormSignals = ['写一篇', '写篇', '帮我写', '生成一篇', '生成文章', '写文章', '写邮件', '写一封', '撰写', '草拟', '提纲', '报告', '方案', '文案', '故事', '文章', '邮件', '公告', '文章'];
  const isLongForm = longFormSignals.some((signal) => normalized.includes(signal));
  if (isLongForm) {
    const topic = normalized
      .replace(/^(帮我写|写一篇|写篇|生成一篇|生成文章|写文章|写邮件|写一封|撰写|草拟|生成|帮我)/, '')
      .replace(/(文章|邮件|报告|方案|文案|故事|公告)$/g, '')
      .trim();
    const subject = topic || '这个主题';
    const panelContent = `下面是一份初稿，已整理到右侧面板里：\n\n《${subject}》\n\n${subject}是一个值得认真对待的议题。先从背景、关键问题与目标出发，再给出可执行的建议和下一步安排。\n\n1. 背景与现状\n- 说明这个话题为什么值得关注。\n- 提炼当前面临的核心矛盾或机会。\n\n2. 关键要点\n- 列出三到五条最重要的观点。\n- 每条都尽量和目标用户或场景相关。\n\n3. 可执行建议\n- 先从最小可行动作开始。\n- 把责任、时间和预期结果写清楚。\n\n4. 结语\n- 用一句简洁、有力量的话收尾。\n- 给出下一步行动建议。`;
    return {
      content: '我已经帮你生成了一份初稿，完整内容已经展开到右侧面板。',
      panelContent,
      shouldPinToPanel: true,
    };
  }

  return {
    content: '我先按通用聊天处理。你可以继续提问，或直接说“维护联系人 / 查联系人”切到关系模块。',
    shouldPinToPanel: false,
  };
}
