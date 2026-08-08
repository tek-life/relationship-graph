// 反馈类轻量组件：LoadingSpinner / EmptyState / ErrorBanner / StatusBadge。
// 从 admin/shared.tsx 原样迁移（渲染结构与样式保持一致），供全站复用。

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

/** 状态徽标（启用/禁用） */
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
