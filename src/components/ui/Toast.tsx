// 全局 Toast 系统：ToastProvider + useToast。
// success / error / info 三类，右上角堆叠，自动消失 + 手动关闭。
// 颜色全部走 CSS 变量（含 --success/--warning 的 fallback），随三套主题切换。
// 入场动画通过组件内联 <style> 定义，避免修改全局 index.css。

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';

export type ToastType = 'success' | 'error' | 'info';

export interface ToastOptions {
  /** 自动消失时长（毫秒）；省略时 success/info 4000ms、error 6000ms */
  duration?: number;
  /** 可选标题（加粗首行） */
  title?: string;
}

export interface ToastApi {
  /** 通用入口，返回 toast id，可用于 dismiss */
  toast: (type: ToastType, message: string, options?: ToastOptions) => number;
  success: (message: string, options?: ToastOptions) => number;
  error: (message: string, options?: ToastOptions) => number;
  info: (message: string, options?: ToastOptions) => number;
  /** 手动关闭指定 toast */
  dismiss: (id: number) => void;
}

interface ToastItem {
  id: number;
  type: ToastType;
  message: string;
  title?: string;
  duration: number;
}

const ToastContext = createContext<ToastApi | null>(null);

const DEFAULT_DURATION: Record<ToastType, number> = {
  success: 4000,
  info: 4000,
  error: 6000,
};

// 各类型的强调色：error 用已有 --danger-color；
// TODO: --success / --warning 语义令牌由主题工程师在 index.css 添加，
// 就绪后可把 fallback 去掉（当前 fallback 为语义近似值，三主题下均可读）。
const TYPE_COLOR: Record<ToastType, string> = {
  success: 'var(--success, #16a34a)',
  error: 'var(--danger-color)',
  info: 'var(--accent-color)',
};

function ToastIcon({ type }: { type: ToastType }) {
  const color = TYPE_COLOR[type];
  if (type === 'success') {
    return (
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" style={{ flexShrink: 0, marginTop: 2 }}>
        <circle cx="8" cy="8" r="7" stroke={color} strokeWidth="1.5" />
        <path d="M5 8.2 7.2 10.4 11 5.8" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    );
  }
  if (type === 'error') {
    return (
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" style={{ flexShrink: 0, marginTop: 2 }}>
        <circle cx="8" cy="8" r="7" stroke={color} strokeWidth="1.5" />
        <path d="M8 4.5v4" stroke={color} strokeWidth="1.5" strokeLinecap="round" />
        <circle cx="8" cy="11" r="0.9" fill={color} />
      </svg>
    );
  }
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" style={{ flexShrink: 0, marginTop: 2 }}>
      <circle cx="8" cy="8" r="7" stroke={color} strokeWidth="1.5" />
      <path d="M8 7.5v4" stroke={color} strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="8" cy="5" r="0.9" fill={color} />
    </svg>
  );
}

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const idRef = useRef(0);
  const timersRef = useRef<Map<number, ReturnType<typeof setTimeout>>>(new Map());

  const dismiss = useCallback((id: number) => {
    const timer = timersRef.current.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      timersRef.current.delete(id);
    }
    setItems((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toast = useCallback(
    (type: ToastType, message: string, options?: ToastOptions) => {
      idRef.current += 1;
      const id = idRef.current;
      const duration = options?.duration ?? DEFAULT_DURATION[type];
      setItems((prev) => [...prev, { id, type, message, title: options?.title, duration }]);
      const timer = setTimeout(() => dismiss(id), duration);
      timersRef.current.set(id, timer);
      return id;
    },
    [dismiss]
  );

  // 卸载时清理所有未触发的定时器
  useEffect(() => {
    const timers = timersRef.current;
    return () => {
      timers.forEach((t) => clearTimeout(t));
      timers.clear();
    };
  }, []);

  const api = useMemo<ToastApi>(
    () => ({
      toast,
      success: (m, o) => toast('success', m, o),
      error: (m, o) => toast('error', m, o),
      info: (m, o) => toast('info', m, o),
      dismiss,
    }),
    [toast, dismiss]
  );

  return (
    <ToastContext.Provider value={api}>
      {children}
      {/* 入场动画 keyframes（局部样式，不改全局 index.css） */}
      <style>{`
        @keyframes ui-toast-in {
          from { opacity: 0; transform: translateX(16px); }
          to { opacity: 1; transform: translateX(0); }
        }
      `}</style>
      <div
        className="fixed right-4 top-4 z-[100] flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2"
        aria-label="通知"
      >
        {items.map((item) => (
          <div
            key={item.id}
            role={item.type === 'error' ? 'alert' : 'status'}
            className="flex items-start gap-2 rounded-lg border px-3 py-2.5 shadow-lg"
            style={{
              backgroundColor: 'var(--bg-card)',
              borderColor: 'var(--border-color)',
              boxShadow: '0 4px 12px var(--shadow-color)',
              animation: 'ui-toast-in 0.18s ease-out',
            }}
          >
            <ToastIcon type={item.type} />
            <div className="min-w-0 flex-1 text-sm" style={{ color: 'var(--text-primary)' }}>
              {item.title && <div className="font-medium">{item.title}</div>}
              <div style={{ color: item.title ? 'var(--text-secondary)' : undefined, wordBreak: 'break-word' }}>
                {item.message}
              </div>
            </div>
            <button
              type="button"
              aria-label="关闭提示"
              className="rounded p-0.5 transition hover:bg-black/5"
              style={{ color: 'var(--text-muted)' }}
              onClick={() => dismiss(item.id)}
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
                <path d="M3.5 3.5l7 7M10.5 3.5l-7 7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

/** 获取 Toast API；必须在 ToastProvider 内使用 */
export function useToast(): ToastApi {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error('useToast 必须在 <ToastProvider> 内使用');
  }
  return ctx;
}
