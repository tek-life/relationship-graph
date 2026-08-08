// 分段控件（Segmented Control）：受控 value/onChange，选项支持 disabled。
// 键盘可达（原生 button），颜色全部走 CSS 变量。

export interface SegmentedOption<T extends string = string> {
  label: React.ReactNode;
  value: T;
  disabled?: boolean;
}

export interface SegmentedProps<T extends string = string> {
  options: SegmentedOption<T>[];
  /** 受控当前值 */
  value: T;
  onChange: (value: T) => void;
  /** 无障碍名称 */
  ariaLabel?: string;
  className?: string;
}

export function Segmented<T extends string = string>({
  options,
  value,
  onChange,
  ariaLabel,
  className = '',
}: SegmentedProps<T>) {
  return (
    <div
      role="group"
      aria-label={ariaLabel}
      className={`inline-flex items-center gap-0.5 rounded-lg border p-0.5 ${className}`}
      style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-color)' }}
    >
      {options.map((opt) => {
        const active = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            aria-pressed={active}
            disabled={opt.disabled}
            className="rounded-md px-3 py-1 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-40"
            style={
              active
                ? {
                    backgroundColor: 'var(--bg-card)',
                    color: 'var(--text-primary)',
                    boxShadow: '0 1px 2px var(--shadow-color)',
                  }
                : { color: 'var(--text-secondary)' }
            }
            onClick={() => onChange(opt.value)}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
