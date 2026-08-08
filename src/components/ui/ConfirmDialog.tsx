// 全局确认弹窗：从 admin/shared.tsx 提升而来，props 为原 admin 版的超集。
// 默认行为与 admin 版完全一致（Esc 关闭、点击遮罩关闭、danger 红色确认键）。

import { useEffect } from 'react';

export interface ConfirmDialogProps {
  /** 弹窗标题 */
  title: string;
  /** 正文内容（兼容原 admin 版 string） */
  message: React.ReactNode;
  /** 确认按钮文案 */
  confirmLabel?: string;
  /** 取消按钮文案 */
  cancelLabel?: string;
  /** 危险操作：确认按钮使用 --danger-color */
  danger?: boolean;
  /** 是否展示；false 时不渲染（便于常驻挂载一个实例）。默认 true，与 admin 版行为一致 */
  open?: boolean;
  /** 禁用确认按钮（如等待勾选协议），默认 false */
  confirmDisabled?: boolean;
  /** 正文之下的补充说明（可选扩展） */
  description?: React.ReactNode;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  title,
  message,
  confirmLabel = '确认',
  cancelLabel = '取消',
  danger = false,
  open = true,
  confirmDisabled = false,
  description,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  // Esc 关闭
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onCancel]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: 'rgba(0,0,0,0.4)' }}
      onClick={onCancel}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className="w-full max-w-sm rounded-xl border p-5 shadow-lg"
        style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-base font-semibold" style={{ color: 'var(--text-primary)' }}>
          {title}
        </h3>
        <div className="mt-2 text-sm" style={{ color: 'var(--text-secondary)' }}>
          {message}
        </div>
        {description != null && (
          <div className="mt-1 text-xs" style={{ color: 'var(--text-muted)' }}>
            {description}
          </div>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <button type="button" className="btn-secondary" onClick={onCancel}>
            {cancelLabel}
          </button>
          <button
            type="button"
            className="rounded-lg px-4 py-2 text-sm font-medium text-white transition disabled:cursor-not-allowed disabled:opacity-50"
            style={{ backgroundColor: danger ? 'var(--danger-color)' : 'var(--accent-color)' }}
            disabled={confirmDisabled}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
