/**
 * 个人画像构建占位组件
 * Task 13 会实现完整的画像问答页面
 */

interface ProfileQAProps {
  onComplete?: () => void;
}

export default function ProfileQA({ onComplete }: ProfileQAProps) {
  return (
    <div className="flex h-full items-center justify-center p-8">
      <div className="text-center">
        <h2 className="text-xl font-semibold" style={{ color: 'var(--text-primary)' }}>
          个人画像构建
        </h2>
        <p className="mt-2 text-sm" style={{ color: 'var(--text-secondary)' }}>
          画像问答功能开发中…（Task 13）
        </p>
        {onComplete && (
          <button
            type="button"
            className="mt-4 rounded-full px-4 py-2 text-sm transition"
            style={{ backgroundColor: 'var(--accent-color)', color: '#fff' }}
            onClick={onComplete}
          >
            跳过，返回首页
          </button>
        )}
      </div>
    </div>
  );
}
