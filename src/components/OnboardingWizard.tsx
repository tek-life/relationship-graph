import { useState } from 'react';
import ImportWizard from './ImportWizard';

type Step = 'welcome' | 'import' | 'examples' | 'done';

interface Props {
  onComplete: () => void;
  onManualAdd: () => void;
  onQuerySubmit: (query: string) => void;
}

const STORAGE_KEY = 'rg_onboarding_completed';

export function isOnboardingCompleted(): boolean {
  return localStorage.getItem(STORAGE_KEY) === 'true';
}

export function markOnboardingCompleted(): void {
  localStorage.setItem(STORAGE_KEY, 'true');
}

const EXAMPLE_QUERIES = [
  '新加一个联系人叫张三',
  '找找在腾讯的朋友',
  '最近和谁聊过',
];

export default function OnboardingWizard({ onComplete, onManualAdd, onQuerySubmit }: Props) {
  const [step, setStep] = useState<Step>('welcome');
  const [showImport, setShowImport] = useState(false);
  const [importChoice, setImportChoice] = useState<'excel' | 'manual' | null>(null);

  const handleFinish = () => {
    markOnboardingCompleted();
    onComplete();
  };

  const handleSkip = () => {
    markOnboardingCompleted();
    onComplete();
  };

  const handleExcelImport = () => {
    setShowImport(true);
    setImportChoice('excel');
  };

  const handleManualAdd = () => {
    setImportChoice('manual');
    markOnboardingCompleted();
    onManualAdd();
  };

  const handleLater = () => {
    markOnboardingCompleted();
    onComplete();
  };

  const handleImportDone = () => {
    setShowImport(false);
    setStep('examples');
  };

  const handleQueryClick = (query: string) => {
    markOnboardingCompleted();
    onQuerySubmit(query);
  };

  const goToExamples = () => {
    setStep('examples');
  };

  // 步骤索引用于指示器
  const stepIndex = step === 'welcome' ? 0 : step === 'import' ? 1 : step === 'examples' ? 2 : 3;
  const totalSteps = 4;

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div
        className="relative mx-4 w-full max-w-lg rounded-2xl shadow-2xl overflow-hidden"
        style={{ backgroundColor: 'var(--bg-card, #fff)' }}
      >
        {/* 跳过按钮 */}
        {step !== 'done' && (
          <button
            type="button"
            onClick={handleSkip}
            className="absolute right-4 top-4 text-sm transition hover:opacity-80"
            style={{ color: 'var(--text-muted, #94a3b8)' }}
          >
            跳过
          </button>
        )}

        {/* 内容区域 */}
        <div className="p-8 pt-10">
          {/* Welcome */}
          {step === 'welcome' && (
            <div className="text-center animate-fade-in">
              <div className="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-gradient-to-br from-blue-500 to-indigo-600">
                <svg xmlns="http://www.w3.org/2000/svg" className="h-10 w-10 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
                </svg>
              </div>
              <h2 className="text-2xl font-bold" style={{ color: 'var(--text-primary, #0f172a)' }}>
                欢迎使用人脉关系图谱
              </h2>
              <p className="mt-3 text-sm leading-relaxed" style={{ color: 'var(--text-secondary, #64748b)' }}>
                管理你的人脉网络，记录互动历程，用 AI 自然语言查询快速找到你需要的联系人。
                加密存储、多端协同、智能辅助。
              </p>
              <div className="mt-4 grid grid-cols-3 gap-3 text-xs" style={{ color: 'var(--text-muted, #94a3b8)' }}>
                <div className="rounded-lg p-2" style={{ backgroundColor: 'var(--bg-secondary, #f8fafc)' }}>
                  <span className="text-lg">🔒</span>
                  <p className="mt-1">加密存储</p>
                </div>
                <div className="rounded-lg p-2" style={{ backgroundColor: 'var(--bg-secondary, #f8fafc)' }}>
                  <span className="text-lg">🤖</span>
                  <p className="mt-1">AI 查询</p>
                </div>
                <div className="rounded-lg p-2" style={{ backgroundColor: 'var(--bg-secondary, #f8fafc)' }}>
                  <span className="text-lg">📊</span>
                  <p className="mt-1">可视化图谱</p>
                </div>
              </div>
            </div>
          )}

          {/* Import */}
          {step === 'import' && !showImport && (
            <div className="animate-fade-in">
              <h2 className="text-xl font-bold text-center" style={{ color: 'var(--text-primary, #0f172a)' }}>
                添加你的联系人
              </h2>
              <p className="mt-2 text-center text-sm" style={{ color: 'var(--text-secondary, #64748b)' }}>
                选择一种方式开始构建你的人脉网络
              </p>
              <div className="mt-6 space-y-3">
                <button
                  type="button"
                  onClick={handleExcelImport}
                  className="flex w-full items-center gap-4 rounded-xl border p-4 text-left transition hover:shadow-md"
                  style={{ borderColor: 'var(--border-color, #e2e8f0)', backgroundColor: 'var(--bg-secondary, #f8fafc)' }}
                >
                  <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-lg bg-green-100 text-green-600">
                    <svg xmlns="http://www.w3.org/2000/svg" className="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                    </svg>
                  </div>
                  <div>
                    <p className="font-medium" style={{ color: 'var(--text-primary, #0f172a)' }}>从 Excel 导入</p>
                    <p className="text-xs" style={{ color: 'var(--text-muted, #94a3b8)' }}>批量导入现有联系人数据</p>
                  </div>
                </button>
                <button
                  type="button"
                  onClick={handleManualAdd}
                  className="flex w-full items-center gap-4 rounded-xl border p-4 text-left transition hover:shadow-md"
                  style={{ borderColor: 'var(--border-color, #e2e8f0)', backgroundColor: 'var(--bg-secondary, #f8fafc)' }}
                >
                  <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-lg bg-blue-100 text-blue-600">
                    <svg xmlns="http://www.w3.org/2000/svg" className="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M12 4v16m8-8H4" />
                    </svg>
                  </div>
                  <div>
                    <p className="font-medium" style={{ color: 'var(--text-primary, #0f172a)' }}>手动添加</p>
                    <p className="text-xs" style={{ color: 'var(--text-muted, #94a3b8)' }}>逐个添加联系人</p>
                  </div>
                </button>
                <button
                  type="button"
                  onClick={handleLater}
                  className="flex w-full items-center gap-4 rounded-xl border p-4 text-left transition hover:shadow-md"
                  style={{ borderColor: 'var(--border-color, #e2e8f0)', backgroundColor: 'var(--bg-secondary, #f8fafc)' }}
                >
                  <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-lg bg-slate-100 text-slate-500">
                    <svg xmlns="http://www.w3.org/2000/svg" className="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                  </div>
                  <div>
                    <p className="font-medium" style={{ color: 'var(--text-primary, #0f172a)' }}>稍后再说</p>
                    <p className="text-xs" style={{ color: 'var(--text-muted, #94a3b8)' }}>先看看再决定</p>
                  </div>
                </button>
              </div>
            </div>
          )}

          {/* Inline ImportWizard */}
          {step === 'import' && showImport && (
            <div className="animate-fade-in">
              <div className="mb-3 flex items-center justify-between">
                <h3 className="font-semibold" style={{ color: 'var(--text-primary, #0f172a)' }}>Excel 导入</h3>
                <button
                  type="button"
                  onClick={() => setShowImport(false)}
                  className="text-sm hover:underline"
                  style={{ color: 'var(--text-muted, #94a3b8)' }}
                >
                  返回
                </button>
              </div>
              <div className="max-h-[400px] overflow-y-auto">
                <ImportWizard onImported={handleImportDone} />
              </div>
            </div>
          )}

          {/* Examples */}
          {step === 'examples' && (
            <div className="text-center animate-fade-in">
              <h2 className="text-xl font-bold" style={{ color: 'var(--text-primary, #0f172a)' }}>
                试试 AI 查询
              </h2>
              <p className="mt-2 text-sm" style={{ color: 'var(--text-secondary, #64748b)' }}>
                点击下面的示例体验自然语言查询
              </p>
              <div className="mt-6 space-y-3">
                {EXAMPLE_QUERIES.map((query) => (
                  <button
                    key={query}
                    type="button"
                    onClick={() => handleQueryClick(query)}
                    className="w-full rounded-xl border p-3 text-left text-sm transition hover:shadow-md"
                    style={{ borderColor: 'var(--border-color, #e2e8f0)', color: 'var(--text-primary, #0f172a)' }}
                  >
                    <span className="mr-2 text-blue-500">💬</span>
                    "{query}"
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Done */}
          {step === 'done' && (
            <div className="text-center animate-fade-in">
              <div className="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-gradient-to-br from-green-400 to-emerald-600">
                <svg xmlns="http://www.w3.org/2000/svg" className="h-10 w-10 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                </svg>
              </div>
              <h2 className="text-2xl font-bold" style={{ color: 'var(--text-primary, #0f172a)' }}>
                一切就绪！
              </h2>
              <p className="mt-3 text-sm" style={{ color: 'var(--text-secondary, #64748b)' }}>
                开始构建你的人脉网络吧
              </p>
            </div>
          )}
        </div>

        {/* 底部导航 */}
        <div className="border-t px-8 py-4" style={{ borderColor: 'var(--border-color, #e2e8f0)' }}>
          {/* 步骤指示器 */}
          <div className="mb-3 flex justify-center gap-2">
            {Array.from({ length: totalSteps }).map((_, i) => (
              <div
                key={i}
                className={`h-2 w-2 rounded-full transition-all ${i === stepIndex ? 'w-6 bg-blue-600' : 'bg-slate-300'}`}
              />
            ))}
          </div>
          {/* 按钮 */}
          <div className="flex justify-between">
            {step !== 'welcome' && step !== 'done' && !showImport ? (
              <button
                type="button"
                onClick={() => setStep(step === 'examples' ? 'import' : 'welcome')}
                className="rounded-lg px-4 py-2 text-sm font-medium transition"
                style={{ color: 'var(--text-secondary, #64748b)' }}
              >
                上一步
              </button>
            ) : (
              <div />
            )}
            {step === 'welcome' && (
              <button
                type="button"
                onClick={() => setStep('import')}
                className="rounded-lg bg-blue-600 px-6 py-2 text-sm font-medium text-white transition hover:bg-blue-700"
              >
                开始
              </button>
            )}
            {step === 'examples' && (
              <button
                type="button"
                onClick={() => setStep('done')}
                className="rounded-lg bg-blue-600 px-6 py-2 text-sm font-medium text-white transition hover:bg-blue-700"
              >
                下一步
              </button>
            )}
            {step === 'done' && (
              <button
                type="button"
                onClick={handleFinish}
                className="mx-auto rounded-lg bg-blue-600 px-8 py-2.5 text-sm font-medium text-white transition hover:bg-blue-700"
              >
                开始使用
              </button>
            )}
            {step === 'import' && !showImport && importChoice === 'excel' && (
              <button
                type="button"
                onClick={goToExamples}
                className="rounded-lg bg-blue-600 px-6 py-2 text-sm font-medium text-white transition hover:bg-blue-700"
              >
                下一步
              </button>
            )}
          </div>
          {/* 帮助文档链接 */}
          <div className="mt-3 text-center">
            <a
              href="/docs/help/user-help.html"
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs transition hover:underline"
              style={{ color: 'var(--text-muted, #94a3b8)' }}
            >
              使用帮助
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}
