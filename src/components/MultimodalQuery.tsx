// 首页多模态查询框：文字 / 语音 / 图片 OCR 三种输入共享同一 query，
// 语音与 OCR 结果只追加进输入框，由用户确认后再提交查询。
import { useEffect, useRef, useState } from 'react';
import { useVoiceInput } from '../hooks/useVoiceInput';
import { nlqConfirm, nlqMulti } from '../services/db';
import type { NlqResponse, NlqResult } from '../types';
import DraftConfirmation from './DraftConfirmation';
import ImageOcrButton, { type ImageOcrHandle } from './ImageOcrButton';
import { NLQ_EXAMPLES } from './NaturalLanguageQuery';
import NlqResultCard from './NlqResultCard';
import PathResultDisplay from './PathResultDisplay';

interface MultimodalQueryProps {
  onPersonClick?: (personId: string) => void;
}

export default function MultimodalQuery({ onPersonClick }: MultimodalQueryProps) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<NlqResult[]>([]);
  const [searched, setSearched] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [nlqResponse, setNlqResponse] = useState<NlqResponse | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const ocrRef = useRef<ImageOcrHandle>(null);

  // textarea 高度自适应内容（含语音/OCR 追加的场景）
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 240)}px`;
  }, [query]);

  const appendText = (text: string) => {
    setQuery((prev) => {
      const trimmedPrev = prev.replace(/\s+$/, '');
      return trimmedPrev ? `${trimmedPrev} ${text}` : text;
    });
    textareaRef.current?.focus();
  };

  const voice = useVoiceInput(appendText);

  const submit = async () => {
    const trimmed = query.trim();
    if (!trimmed || loading) return;
    if (voice.recording) voice.stop();
    setLoading(true);
    setError('');
    setNlqResponse(null);
    const started = performance.now();
    try {
      const response = await nlqMulti(trimmed, false);
      setNlqResponse(response);
      if (response.intentType === 'searchPeople') {
        setResults(response.results);
      } else {
        setResults([]);
      }
      setSearched(true);
      console.info('[multimodal-query] done', {
        queryLength: trimmed.length,
        intentType: response.intentType,
        elapsedMs: Math.round(performance.now() - started),
      });
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleConfirm = async (intentType: string, data: Record<string, unknown>) => {
    try {
      await nlqConfirm(intentType, data);
      setNlqResponse(null);
      setQuery('');
    } catch (e) {
      console.error('确认失败:', e);
    }
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // 回车提交、Shift+Enter 换行；中文输入法候选态回车不触发提交
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      submit();
    }
  };

  const handlePaste = (event: React.ClipboardEvent<HTMLTextAreaElement>) => {
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
      <form
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <div className="rounded-2xl border shadow-sm transition focus-within:ring-2" style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}>
          <textarea
            ref={textareaRef}
            rows={2}
            className="w-full resize-none rounded-2xl bg-transparent px-4 pb-2 pt-4 text-base outline-none"
            style={{ color: 'var(--text-primary)' }}
            placeholder="问点什么，比如：谁在上海做地产，和我关系比较近？（Enter 提交，Shift+Enter 换行，可粘贴截图识别）"
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
            <button
              type="submit"
              className="btn-primary flex items-center justify-center rounded-full p-2"
              disabled={busy || !query.trim()}
            >
              {loading ? (
                <svg className="animate-spin h-5 w-5" viewBox="0 0 24 24" fill="none">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"/>
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
                </svg>
              ) : (
                <svg className="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="22" y1="2" x2="11" y2="13"/>
                  <polygon points="22 2 15 22 11 13 2 9 22 2"/>
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
            onClick={() => setQuery(example)}
          >
            {example}
          </button>
        ))}
      </div>

      {error && <p className="rounded bg-red-50 p-3 text-sm text-red-700">{error}</p>}

      <div className="space-y-3 text-left">
        {nlqResponse && nlqResponse.intentType === 'searchPeople' && (
          <>
            {searched && !loading && !error && results.length === 0 && (
              <p className="text-center text-sm" style={{ color: 'var(--text-secondary)' }}>没有找到匹配的联系人，换个问法试试。</p>
            )}
            {results.map((result) => (
              <NlqResultCard key={result.personId} result={result} onPersonClick={onPersonClick} />
            ))}
          </>
        )}
        {nlqResponse && (nlqResponse.intentType === 'createPersonDraft' || nlqResponse.intentType === 'updatePersonDraft' || nlqResponse.intentType === 'addInteractionDraft') && (
          <DraftConfirmation response={nlqResponse} onConfirm={handleConfirm} onCancel={() => setNlqResponse(null)} />
        )}
        {nlqResponse && nlqResponse.intentType === 'findPath' && (
          <PathResultDisplay path={nlqResponse.path} onPersonClick={onPersonClick} />
        )}
      </div>
    </div>
  );
}
