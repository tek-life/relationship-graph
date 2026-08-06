/**
 * 管理后台占位组件
 * Task 15 会实现完整的管理后台页面
 */

interface AdminPanelProps {
  userId?: string;
}

export default function AdminPanel({ userId }: AdminPanelProps) {
  return (
    <div className="flex h-full items-center justify-center p-8">
      <div className="text-center">
        <h2 className="text-xl font-semibold" style={{ color: 'var(--text-primary)' }}>
          管理后台
        </h2>
        <p className="mt-2 text-sm" style={{ color: 'var(--text-secondary)' }}>
          管理后台功能开发中…（Task 15）
        </p>
        {userId && (
          <p className="mt-1 text-xs" style={{ color: 'var(--text-muted)' }}>
            当前管理员 ID: {userId}
          </p>
        )}
      </div>
    </div>
  );
}
