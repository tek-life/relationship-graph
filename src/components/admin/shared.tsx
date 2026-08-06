// 管理后台共享 UI 组件

import { useEffect } from 'react';

/** 管理后台 Tab 按钮 */
export function AdminTabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-full px-3 py-1.5 text-sm font-medium transition"
      style={
        active
          ? { backgroundColor: 'var(--accent-color)', color: '#fff' }
          : { backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }
      }
    >
      {children}
    </button>
  );
}

/** 确认弹窗 */
export function ConfirmDialog({
  title,
  message,
  confirmLabel = '确认',
  cancelLabel = '取消',
  danger = false,
  onConfirm,
  onCancel,
}: {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  // Esc 关闭
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [onCancel]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: 'rgba(0,0,0,0.4)' }}
      onClick={onCancel}
    >
      <div
        className="w-full max-w-sm rounded-xl border p-5 shadow-lg"
        style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-base font-semibold" style={{ color: 'var(--text-primary)' }}>
          {title}
        </h3>
        <p className="mt-2 text-sm" style={{ color: 'var(--text-secondary)' }}>
          {message}
        </p>
        <div className="mt-4 flex justify-end gap-2">
          <button type="button" className="btn-secondary" onClick={onCancel}>
            {cancelLabel}
          </button>
          <button
            type="button"
            className="rounded-lg px-4 py-2 text-sm font-medium text-white transition"
            style={{ backgroundColor: danger ? 'var(--danger-color)' : 'var(--accent-color)' }}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

/** 加载中提示 */
export function LoadingSpinner({ text = '加载中…' }: { text?: string }) {
  return (
    <div className="flex items-center justify-center py-12">
      <span className="text-sm" style={{ color: 'var(--text-muted)' }}>
        {text}
      </span>
    </div>
  );
}

/** 空状态 */
export function EmptyState({ text }: { text: string }) {
  return (
    <div
      className="rounded-lg border border-dashed p-8 text-center text-sm"
      style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}
    >
      {text}
    </div>
  );
}

/** 错误提示条 */
export function ErrorBanner({ message }: { message: string }) {
  return (
    <div
      className="rounded-lg border px-3 py-2 text-sm"
      style={{
        backgroundColor: 'var(--danger-color)',
        borderColor: 'var(--danger-color)',
        color: '#fff',
      }}
    >
      {message}
    </div>
  );
}

/** 状态徽标 */
export function StatusBadge({ active }: { active: boolean }) {
  return (
    <span
      className="badge"
      style={{
        backgroundColor: active ? 'var(--accent-light)' : 'var(--surface-hover)',
        color: active ? 'var(--accent-color)' : 'var(--text-muted)',
      }}
    >
      {active ? '启用' : '禁用'}
    </span>
  );
}
