// 语义徽标 Badge：default / success / warning / danger / info 五种变体。
// 颜色走 CSS 变量；success/warning 优先使用新语义令牌，令牌未就绪时使用语义近似 fallback。

export type BadgeVariant = 'default' | 'success' | 'warning' | 'danger' | 'info';

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant;
  children: React.ReactNode;
}

interface BadgeStyle {
  backgroundColor: string;
  color: string;
}

// TODO: --success / --warning 语义令牌正由主题工程师在 index.css 添加，
// 就绪后可去掉 var() 的第二参 fallback（当前 fallback 为语义近似值，三主题下均可读）。
const VARIANT_STYLE: Record<BadgeVariant, BadgeStyle> = {
  default: { backgroundColor: 'var(--surface-hover)', color: 'var(--text-secondary)' },
  success: { backgroundColor: 'var(--success, rgba(22, 163, 74, 0.12))', color: 'var(--success-text, #16a34a)' },
  warning: { backgroundColor: 'var(--warning, rgba(217, 119, 6, 0.12))', color: 'var(--warning-text, #d97706)' },
  danger: { backgroundColor: 'rgba(220, 38, 38, 0.12)', color: 'var(--danger-color)' },
  info: { backgroundColor: 'var(--accent-light)', color: 'var(--accent-color)' },
};

export function Badge({ variant = 'default', children, className = '', style, ...rest }: BadgeProps) {
  return (
    <span className={`badge ${className}`} style={{ ...VARIANT_STYLE[variant], ...style }} {...rest}>
      {children}
    </span>
  );
}
