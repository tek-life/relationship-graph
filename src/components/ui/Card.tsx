// 容器卡片 Card：padding / 圆角 / 边框 / 阴影均走 CSS 变量（带默认值），随三套主题切换。

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  /** 内边距，省略时使用 --card-padding（默认 1rem） */
  padding?: string | number;
  /** 是否显示边框，默认 true */
  bordered?: boolean;
}

export function Card({ padding, bordered = true, children, className = '', style, ...rest }: CardProps) {
  return (
    <div
      className={`transition ${className}`}
      style={{
        backgroundColor: 'var(--bg-card)',
        borderRadius: 'var(--card-radius, 0.75rem)',
        border: bordered ? '1px solid var(--border-color)' : 'none',
        boxShadow: '0 1px 2px var(--shadow-color)',
        padding: padding ?? 'var(--card-padding, 1rem)',
        ...style,
      }}
      {...rest}
    >
      {children}
    </div>
  );
}
