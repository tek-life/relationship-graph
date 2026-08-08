// 管理后台共享 UI 组件
// 说明：ConfirmDialog / LoadingSpinner / EmptyState / ErrorBanner / StatusBadge
// 已提升为全局组件基座（src/components/ui/），此处 re-export 保持原有
// 导出名与 props 签名完全不变，admin 各页面无需任何改动。

export { ConfirmDialog, EmptyState, ErrorBanner, LoadingSpinner, StatusBadge } from '../ui';

/** 管理后台 Tab 按钮（admin 专用，暂不纳入全局基座） */
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
