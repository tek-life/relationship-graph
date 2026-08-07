/**
 * 内观画像构建 — 逐问逐答对话式 UI
 * 三阶段流程：英雄之旅复盘 → 芒格多元思维 → 内观画像生成
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { apiPost, apiGet } from '../services/api';
import MarkdownContent from './MarkdownContent';

// === 类型定义 ===

interface QaExchange {
  question: string;
  answer: string;
  moduleId: string;
}

interface NextQuestionApiResponse {
  question: string;
  moduleName: string;
  moduleIndex: number;
  isModuleComplete: boolean;
  isFlowComplete: boolean;
}

interface GenerateProfileApiResponse {
  profileDoc: string;
}

interface QaModuleInfo {
  id: string;
  name: string;
  description?: string;
}

// === 组件 ===

interface ProfileQAProps {
  onComplete?: () => void;
  /** 已保存的内观画像文档（已完成用户进入时直接查看） */
  initialProfileDoc?: string | null;
  /** 当前用户是否已完成内观画像 */
  initialCompleted?: boolean;
}

export default function ProfileQA({ onComplete, initialProfileDoc, initialCompleted }: ProfileQAProps) {
  const [stages, setStages] = useState<QaModuleInfo[]>([
    { id: 'hero_journey', name: '英雄之旅复盘' },
    { id: 'munger_thinking', name: '芒格多元思维' },
    { id: 'profile_generate', name: '内观画像生成' },
  ]);
  const [stageIndex, setStageIndex] = useState(0);
  const [history, setHistory] = useState<QaExchange[]>([]);
  const [currentQuestion, setCurrentQuestion] = useState('');
  const [answer, setAnswer] = useState('');
  const [loading, setLoading] = useState(false);
  const [profileDoc, setProfileDoc] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');
  const [isComposing, setIsComposing] = useState(false);
  // 已完成用户重新作答模式（默认展示已有内观画像，点击“重新作答”后进入问卷）
  const hasExistingDoc = Boolean(initialCompleted && initialProfileDoc);
  const [redoing, setRedoing] = useState(false);
  const showExistingDoc = hasExistingDoc && !redoing && !profileDoc;

  const chatEndRef = useRef<HTMLDivElement>(null);

  const currentStageId = stages[stageIndex]?.id ?? '';

  // 自动滚动到最新消息
  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [history, currentQuestion]);

  // 加载内观画像指令模块列表
  useEffect(() => {
    apiGet<{ modules: QaModuleInfo[] }>('/api/profile-qa/modules')
      .then((res) => {
        if (res.modules.length > 0) {
          setStages(res.modules);
        }
      })
      .catch(() => {
        // 使用默认阶段名称
      });
  }, []);

  // 获取下一个问题
  const fetchNextQuestion = useCallback(
    async (mIndex: number, hist: QaExchange[]) => {
      setLoading(true);
      setError('');
      try {
        const res = await apiPost<NextQuestionApiResponse>('/api/profile-qa/next', {
          moduleIndex: mIndex,
          history: hist,
        });

        if (res.question) {
          setCurrentQuestion(res.question);
        }

        // 更新当前阶段名称
        if (res.moduleName) {
          setStages((prev) =>
            prev.map((s, i) => (i === mIndex ? { ...s, name: res.moduleName } : s)),
          );
        }

        if (res.isModuleComplete && !res.isFlowComplete) {
          // 当前模块完成，进入下一阶段
          const nextIndex = mIndex + 1;
          setStageIndex(nextIndex);
          // 自动获取下一阶段第一个问题
          setTimeout(() => fetchNextQuestion(nextIndex, hist), 500);
        } else if (res.isFlowComplete) {
          // 整个流程完成，生成内观画像
          await generateProfile(hist);
        }
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : '未知错误';
        setError(`获取问题失败：${msg}`);
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  // 初始加载第一个问题；已有内观画像的用户先展示现有内观画像，不自动开问
  useEffect(() => {
    if (!hasExistingDoc) {
      fetchNextQuestion(0, []);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 已完成用户选择重新作答：重置状态并从第一问开始
  const handleRestart = () => {
    setRedoing(true);
    setStageIndex(0);
    setHistory([]);
    setProfileDoc(null);
    setSaved(false);
    setError('');
    setAnswer('');
    void fetchNextQuestion(0, []);
  };

  // 生成内观画像文档
  const generateProfile = async (hist: QaExchange[]) => {
    setLoading(true);
    setError('');
    try {
      const res = await apiPost<GenerateProfileApiResponse>('/api/profile-qa/generate', {
        history: hist,
      });
      setProfileDoc(res.profileDoc);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : '未知错误';
      setError(`生成内观画像失败：${msg}`);
    } finally {
      setLoading(false);
    }
  };

  // 提交回答
  const handleSubmit = async () => {
    const trimmed = answer.trim();
    if (!trimmed || loading) return;

    const exchange: QaExchange = {
      question: currentQuestion,
      answer: trimmed,
      moduleId: currentStageId,
    };

    const newHistory = [...history, exchange];
    setHistory(newHistory);
    setAnswer('');
    setCurrentQuestion('');

    await fetchNextQuestion(stageIndex, newHistory);
  };

  // 保存内观画像
  const handleSave = async () => {
    if (!profileDoc) return;
    setSaving(true);
    setError('');
    try {
      await apiPost('/api/profile-qa/save', { profileDoc });
      setSaved(true);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : '未知错误';
      setError(`保存失败：${msg}`);
    } finally {
      setSaving(false);
    }
  };

  // 键盘事件：Enter 发送，Shift+Enter 换行，输入法组合中不触发
  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey && !isComposing) {
      e.preventDefault();
      handleSubmit();
    }
  };

  // === 渲染 ===

  return (
    <div className="flex h-full flex-col">
      {/* 阶段进度指示器 */}
      <div className="shrink-0 border-b px-6 py-4" style={{ borderColor: 'var(--border-color, #e2e8f0)' }}>
        <div className="flex items-center justify-center gap-2">
          {stages.map((stage, i) => (
            <div key={stage.id} className="flex items-center gap-2">
              <div
                className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-sm font-medium transition-all duration-300"
                style={{
                  backgroundColor:
                    i < stageIndex
                      ? '#22c55e'
                      : i === stageIndex
                        ? 'var(--accent-color)'
                        : 'var(--bg-secondary, #f1f5f9)',
                  color:
                    i <= stageIndex ? '#fff' : 'var(--text-secondary, #64748b)',
                }}
              >
                {i < stageIndex ? '✓' : i + 1}
              </div>
              <span
                className="text-sm whitespace-nowrap"
                style={{
                  color:
                    i === stageIndex
                      ? 'var(--text-primary, #1e293b)'
                      : 'var(--text-secondary, #64748b)',
                  fontWeight: i === stageIndex ? 600 : 400,
                }}
              >
                {stage.name}
              </span>
              {i < stages.length - 1 && (
                <div
                  className="h-px w-6"
                  style={{
                    backgroundColor:
                      i < stageIndex
                        ? '#22c55e'
                        : 'var(--border-color, #e2e8f0)',
                  }}
                />
              )}
            </div>
          ))}
        </div>
      </div>

      {/* 错误提示 */}
      {error && (
        <div
          className="mx-6 mt-3 rounded-lg px-4 py-2 text-sm"
          style={{ backgroundColor: '#fef2f2', color: '#dc2626' }}
        >
          {error}
        </div>
      )}

      {/* 已有内观画像查看 / 对话流区域 / 内观画像预览 */}
      {showExistingDoc ? (
        <div className="flex-1 overflow-y-auto px-6 py-6">
          <div className="mx-auto max-w-3xl">
            <div
              className="mb-6 rounded-xl p-6"
              style={{
                backgroundColor: 'var(--bg-secondary, #f8fafc)',
                border: '1px solid var(--border-color, #e2e8f0)',
              }}
            >
              <h2
                className="mb-1 text-lg font-semibold"
                style={{ color: 'var(--text-primary, #1e293b)' }}
              >
                我的内观画像
              </h2>
              <p
                className="mb-4 text-xs"
                style={{ color: 'var(--text-muted, #94a3b8)' }}
              >
                如需更新内观画像，可重新回答问卷生成新版本并覆盖保存。
              </p>
              <MarkdownContent
                content={initialProfileDoc ?? ''}
                className="prose-sm space-y-3"
              />
            </div>

            <div className="flex gap-3">
              <button
                type="button"
                className="rounded-full px-6 py-2.5 text-sm font-medium text-white transition-opacity hover:opacity-90"
                style={{ backgroundColor: 'var(--accent-color, #3b82f6)' }}
                onClick={handleRestart}
              >
                重新作答更新内观画像
              </button>
              {onComplete && (
                <button
                  type="button"
                  className="rounded-full px-6 py-2.5 text-sm transition"
                  style={{
                    color: 'var(--text-secondary, #64748b)',
                    border: '1px solid var(--border-color, #e2e8f0)',
                  }}
                  onClick={onComplete}
                >
                  返回首页
                </button>
              )}
            </div>
          </div>
        </div>
      ) : profileDoc ? (
        <div className="flex-1 overflow-y-auto px-6 py-6">
          <div className="mx-auto max-w-3xl">
            <div
              className="mb-6 rounded-xl p-6"
              style={{
                backgroundColor: 'var(--bg-secondary, #f8fafc)',
                border: '1px solid var(--border-color, #e2e8f0)',
              }}
            >
              <h2
                className="mb-4 text-lg font-semibold"
                style={{ color: 'var(--text-primary, #1e293b)' }}
              >
                内观画像文档
              </h2>
              <MarkdownContent
                content={profileDoc}
                className="prose-sm space-y-3"
              />
            </div>

            <div className="flex gap-3">
              {!saved ? (
                <button
                  type="button"
                  disabled={saving}
                  className="rounded-full px-6 py-2.5 text-sm font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
                  style={{ backgroundColor: 'var(--accent-color, #3b82f6)' }}
                  onClick={handleSave}
                >
                  {saving ? '保存中…' : '确认保存内观画像'}
                </button>
              ) : (
                <span
                  className="flex items-center gap-1 text-sm font-medium"
                  style={{ color: '#22c55e' }}
                >
                  ✓ 内观画像已保存
                </span>
              )}
              {onComplete && (
                <button
                  type="button"
                  className="rounded-full px-6 py-2.5 text-sm transition"
                  style={{
                    color: 'var(--text-secondary, #64748b)',
                    border: '1px solid var(--border-color, #e2e8f0)',
                  }}
                  onClick={onComplete}
                >
                  返回首页
                </button>
              )}
            </div>
          </div>
        </div>
      ) : (
        <div className="flex-1 space-y-4 overflow-y-auto px-6 py-6">
          {history.map((exchange, i) => (
            <div key={i} className="space-y-3">
              {/* 教练问题（左侧） */}
              <div className="flex gap-3">
                <div
                  className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-xs text-white"
                  style={{ backgroundColor: 'var(--accent-color, #3b82f6)' }}
                >
                  练
                </div>
                <div
                  className="max-w-[75%] rounded-2xl rounded-tl-sm px-4 py-3 text-sm leading-relaxed"
                  style={{
                    backgroundColor: 'var(--bg-secondary, #f1f5f9)',
                    color: 'var(--text-primary, #1e293b)',
                  }}
                >
                  {/* 教练问题来自后端内观画像指令模块，可能含 Markdown */}
                  <MarkdownContent content={exchange.question} className="qa-question-md" />
                </div>
              </div>
              {/* 用户回答（右侧） */}
              <div className="flex justify-end gap-3">
                <div
                  className="max-w-[75%] rounded-2xl rounded-tr-sm px-4 py-3 text-sm leading-relaxed text-white"
                  style={{ backgroundColor: 'var(--accent-color, #3b82f6)' }}
                >
                  <div className="whitespace-pre-wrap">{exchange.answer}</div>
                </div>
                <div
                  className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-xs"
                  style={{
                    backgroundColor: 'var(--bg-secondary, #f1f5f9)',
                    color: 'var(--text-primary, #1e293b)',
                  }}
                >
                  我
                </div>
              </div>
            </div>
          ))}

          {/* 当前问题 */}
          {currentQuestion && !loading && (
            <div className="flex gap-3">
              <div
                className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-xs text-white"
                style={{ backgroundColor: 'var(--accent-color, #3b82f6)' }}
              >
                练
              </div>
              <div
                className="max-w-[75%] rounded-2xl rounded-tl-sm px-4 py-3 text-sm leading-relaxed"
                style={{
                  backgroundColor: 'var(--bg-secondary, #f1f5f9)',
                  color: 'var(--text-primary, #1e293b)',
                }}
              >
                {/* 当前问题（含引导语/追问）可能含 Markdown */}
                <MarkdownContent content={currentQuestion} className="qa-question-md" />
              </div>
            </div>
          )}

          {/* 加载动画 */}
          {loading && (
            <div className="flex gap-3">
              <div
                className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-xs text-white"
                style={{ backgroundColor: 'var(--accent-color, #3b82f6)' }}
              >
                练
              </div>
              <div
                className="rounded-2xl rounded-tl-sm px-4 py-3 text-sm"
                style={{
                  backgroundColor: 'var(--bg-secondary, #f1f5f9)',
                  color: 'var(--text-secondary, #64748b)',
                }}
              >
                <span className="inline-flex gap-1">
                  <span className="animate-bounce" style={{ animationDelay: '0ms' }}>
                    ●
                  </span>
                  <span className="animate-bounce" style={{ animationDelay: '150ms' }}>
                    ●
                  </span>
                  <span className="animate-bounce" style={{ animationDelay: '300ms' }}>
                    ●
                  </span>
                </span>
              </div>
            </div>
          )}

          <div ref={chatEndRef} />
        </div>
      )}

      {/* 底部输入区域（仅在未预览内观画像时显示） */}
      {!profileDoc && currentQuestion && !loading && (
        <div
          className="shrink-0 border-t px-6 py-4"
          style={{ borderColor: 'var(--border-color, #e2e8f0)' }}
        >
          <div className="mx-auto flex max-w-3xl gap-3">
            <textarea
              value={answer}
              onChange={(e) => setAnswer(e.target.value)}
              onKeyDown={handleKeyDown}
              onCompositionStart={() => setIsComposing(true)}
              onCompositionEnd={() => setIsComposing(false)}
              placeholder="请输入你的回答…（Enter 发送，Shift+Enter 换行）"
              rows={3}
              className="flex-1 resize-none rounded-xl px-4 py-3 text-sm transition focus:outline-none"
              style={{
                backgroundColor: 'var(--bg-secondary, #f8fafc)',
                color: 'var(--text-primary, #1e293b)',
                border: '1px solid var(--border-color, #e2e8f0)',
              }}
              disabled={loading}
            />
            <button
              type="button"
              onClick={handleSubmit}
              disabled={!answer.trim() || loading}
              className="self-end rounded-xl px-5 py-3 text-sm font-medium text-white transition hover:opacity-90 disabled:opacity-50"
              style={{ backgroundColor: 'var(--accent-color, #3b82f6)' }}
            >
              发送
            </button>
          </div>
        </div>
      )}

      {/* 跳过按钮 */}
      {onComplete && !profileDoc && !showExistingDoc && (
        <div className="shrink-0 px-6 pb-3 text-center">
          <button
            type="button"
            className="text-xs transition hover:underline"
            style={{ color: 'var(--text-secondary, #64748b)' }}
            onClick={onComplete}
          >
            {hasExistingDoc ? '返回首页' : '跳过内观画像构建，返回首页'}
          </button>
        </div>
      )}
    </div>
  );
}
