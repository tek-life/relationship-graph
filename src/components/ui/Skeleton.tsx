// UX P2-12 骨架屏组件：列表 / 详情页显著加载态的占位呈现。
// 约定：
// 1. 全部走设计令牌（--surface-hover / --bg-card / --border-color），三主题无白块。
// 2. 动画统一用 Tailwind animate-pulse（opacity 呼吸），不引入额外 keyframes。
// 3. 纯展示组件：aria-hidden + role 由组合件声明，不承载交互。

interface SkeletonProps {
  /** 附加布局类（宽高 / 圆角 / 间距），如 "h-4 w-1/2" */
  className?: string;
  style?: React.CSSProperties;
}

/** 基础骨架条：单段 pulse 占位块，由 className 控制形状 */
export function Skeleton({ className = '', style }: SkeletonProps) {
  return (
    <div
      aria-hidden="true"
      className={`animate-pulse rounded ${className}`}
      style={{ backgroundColor: 'var(--surface-hover)', ...style }}
    />
  );
}

/** 联系人卡片骨架（对齐 PersonCard 的两列网格卡片结构） */
export function PersonCardSkeleton() {
  return (
    <div
      aria-hidden="true"
      className="rounded-card border border-line bg-card p-4"
    >
      <div className="flex items-center gap-3">
        <Skeleton className="h-10 w-10 rounded-full" />
        <div className="flex-1 space-y-2">
          <Skeleton className="h-4 w-1/3" />
          <Skeleton className="h-3 w-1/2" />
        </div>
        <Skeleton className="h-5 w-14 rounded-full" />
      </div>
      <div className="mt-4 space-y-2">
        <Skeleton className="h-3 w-5/6" />
        <Skeleton className="h-3 w-2/3" />
      </div>
      <div className="mt-4 flex gap-2">
        <Skeleton className="h-5 w-16 rounded-full" />
        <Skeleton className="h-5 w-16 rounded-full" />
      </div>
    </div>
  );
}

/** 联系人列表骨架（对齐 PersonList 的响应式两列网格） */
export function PersonListSkeleton({ count = 6 }: { count?: number }) {
  return (
    <div
      role="status"
      aria-label="联系人列表加载中"
      className="grid grid-cols-1 gap-4 xl:grid-cols-2"
    >
      {Array.from({ length: count }, (_, i) => (
        <PersonCardSkeleton key={i} />
      ))}
    </div>
  );
}

/** 会话列表骨架（对齐 SessionSidebar 的会话行结构：标题 + 时间） */
export function SessionListSkeleton({ count = 6 }: { count?: number }) {
  return (
    <div role="status" aria-label="会话列表加载中" className="space-y-1">
      {Array.from({ length: count }, (_, i) => (
        <div key={i} aria-hidden="true" className="rounded-lg px-3 py-2.5">
          <Skeleton className="h-3.5 w-3/4" />
          <Skeleton className="mt-1.5 h-2.5 w-1/4" />
        </div>
      ))}
    </div>
  );
}

/** 联系人详情页骨架（对齐 PersonDetail：头部操作条 + 资料卡 + 关系/互动双栏） */
export function PersonDetailSkeleton() {
  return (
    <div role="status" aria-label="联系人详情加载中" className="space-y-4">
      <div className="flex items-center justify-between">
        <Skeleton className="h-7 w-16" />
        <div className="flex gap-2">
          <Skeleton className="h-7 w-20" />
          <Skeleton className="h-7 w-20" />
        </div>
      </div>
      <div className="rounded-card border border-line bg-card p-6">
        <div className="flex items-start justify-between gap-3">
          <div className="flex-1 space-y-2">
            <Skeleton className="h-6 w-1/4" />
            <Skeleton className="h-3.5 w-1/3" />
          </div>
          <Skeleton className="h-6 w-16 rounded-full" />
        </div>
        <div className="mt-5 grid grid-cols-1 gap-x-8 gap-y-3 sm:grid-cols-2">
          {Array.from({ length: 8 }, (_, i) => (
            <div key={i} aria-hidden="true" className="space-y-1.5">
              <Skeleton className="h-3 w-16" />
              <Skeleton className="h-3.5 w-2/3" />
            </div>
          ))}
        </div>
      </div>
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        {[0, 1].map((col) => (
          <div key={col} aria-hidden="true" className="rounded-card border border-line bg-card p-4">
            <Skeleton className="h-4 w-24" />
            <div className="mt-3 space-y-2">
              <Skeleton className="h-14 w-full rounded-lg" />
              <Skeleton className="h-14 w-full rounded-lg" />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
