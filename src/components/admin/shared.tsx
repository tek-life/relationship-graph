// 管理后台共享 UI 组件
// 说明：ConfirmDialog / LoadingSpinner / EmptyState / ErrorBanner / StatusBadge
// 已提升为全局组件基座（src/components/ui/），此处 re-export 保持原有
// 导出名与 props 签名完全不变，admin 各页面无需任何改动。
//
// UX P1-10：新增 sidebar IA（AdminSideNav）与统一页头范式（AdminPageHeader），
// 全部只使用设计令牌（CSS 变量），无硬编码色值。

export { ConfirmDialog, EmptyState, ErrorBanner, LoadingSpinner, StatusBadge } from '../ui';

/**
 * @deprecated UX P1-10 后管理后台已改为 sidebar IA（见 AdminSideNav），
 * 此 Tab 按钮不再被页面使用；仅为兼容既有测试断言保留导出。
 */
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
          ? { backgroundColor: 'var(--accent-color)', color: 'var(--text-primary)' }
          : { backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' }
      }
    >
      {children}
    </button>
  );
}

/** sidebar 导航单项定义（icon 为 lucide-react 图标组件） */
export interface AdminNavItem {
  key: string;
  label: string;
  icon: React.ComponentType<{ size?: number | string; className?: string; 'aria-hidden'?: boolean | 'true' | 'false' }>;
}

/** sidebar 导航分组定义 */
export interface AdminNavGroup {
  title: string;
  items: AdminNavItem[];
}

/**
 * 管理后台左侧 sidebar 导航（UX P1-10）。
 * 激活态：软背景（--accent-light）+ 左侧指示条（--accent-color），
 * hover 使用 --surface-hover 令牌，全部走设计令牌，三主题安全。
 */
export function AdminSideNav({
  groups,
  active,
  onSelect,
}: {
  groups: AdminNavGroup[];
  active: string;
  onSelect: (key: string) => void;
}) {
  return (
    <nav aria-label="管理后台导航" className="w-52 shrink-0">
      <div className="sticky top-6 space-y-5">
        {groups.map((group) => (
          <div key={group.title}>
            <div
              className="mb-1.5 px-3 text-xs font-medium"
              style={{ color: 'var(--text-muted)' }}
            >
              {group.title}
            </div>
            <div className="space-y-1">
              {group.items.map((item) => {
                const isActive = item.key === active;
                const Icon = item.icon;
                return (
                  <button
                    key={item.key}
                    type="button"
                    aria-current={isActive ? 'page' : undefined}
                    onClick={() => onSelect(item.key)}
                    className="relative flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-sm font-medium transition hover:bg-[var(--surface-hover)]"
                    style={
                      isActive
                        ? { backgroundColor: 'var(--accent-light)', color: 'var(--accent-color)' }
                        : { color: 'var(--text-secondary)' }
                    }
                  >
                    {isActive && (
                      <span
                        aria-hidden="true"
                        className="absolute left-0 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-full"
                        style={{ backgroundColor: 'var(--accent-color)' }}
                      />
                    )}
                    <Icon size={16} className="shrink-0" aria-hidden />
                    <span className="truncate">{item.label}</span>
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </div>
    </nav>
  );
}

/**
 * 管理子页统一页头范式（UX P1-10）：标题 + 描述 + 主操作区。
 * - title：页面标题（可含数量等动态内容）
 * - description：一句话说明该页职责与当前状态
 * - actions：右侧主操作区（主按钮/导入入口等），无操作时省略
 */
export function AdminPageHeader({
  title,
  description,
  actions,
}: {
  title: React.ReactNode;
  description?: React.ReactNode;
  actions?: React.ReactNode;
}) {
  return (
    <header
      className="flex flex-wrap items-start justify-between gap-3 border-b pb-4"
      style={{ borderColor: 'var(--border-color)' }}
    >
      <div className="min-w-0">
        <h3
          className="text-xl font-semibold leading-tight"
          style={{ color: 'var(--text-primary)' }}
        >
          {title}
        </h3>
        {description && (
          <p
            className="mt-1 text-sm leading-relaxed"
            style={{ color: 'var(--text-secondary)' }}
          >
            {description}
          </p>
        )}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2 pt-0.5">{actions}</div>}
    </header>
  );
}
