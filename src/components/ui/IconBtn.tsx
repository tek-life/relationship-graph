// 统一图标按钮：size 变体 + title tooltip + disabled 态。
// 图标内容通过 children 传入（内联 SVG；lucide-react 由后续任务统一引入）。

export type IconBtnSize = 'sm' | 'md' | 'lg';

export interface IconBtnProps extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'title' | 'children'> {
  /** tooltip 文案，同时作为 aria-label 兜底 */
  title: string;
  /** 尺寸变体，默认 md（32px） */
  size?: IconBtnSize;
  /** 图标内容（内联 SVG 等） */
  children: React.ReactNode;
}

const SIZE_CLASS: Record<IconBtnSize, string> = {
  sm: 'h-7 w-7 rounded-md',
  md: 'h-8 w-8 rounded-lg',
  lg: 'h-9 w-9 rounded-lg',
};

const ICON_SIZE: Record<IconBtnSize, number> = {
  sm: 14,
  md: 16,
  lg: 18,
};

export function IconBtn({ title, size = 'md', children, className = '', style, disabled, ...rest }: IconBtnProps) {
  return (
    <button
      type="button"
      title={title}
      aria-label={rest['aria-label'] ?? title}
      disabled={disabled}
      className={`inline-flex items-center justify-center transition disabled:cursor-not-allowed disabled:opacity-40 ${SIZE_CLASS[size]} ${className}`}
      style={{ color: 'var(--text-secondary)', ...style }}
      {...rest}
      onMouseEnter={(e) => {
        if (!disabled) (e.currentTarget as HTMLButtonElement).style.backgroundColor = 'var(--surface-hover)';
        rest.onMouseEnter?.(e);
      }}
      onMouseLeave={(e) => {
        (e.currentTarget as HTMLButtonElement).style.backgroundColor = '';
        rest.onMouseLeave?.(e);
      }}
    >
      <span className="inline-flex items-center justify-center" style={{ width: ICON_SIZE[size], height: ICON_SIZE[size] }}>
        {children}
      </span>
    </button>
  );
}
