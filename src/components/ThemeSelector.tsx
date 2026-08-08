import { useRef, useState, useEffect, type ReactNode } from 'react';
import { Sun, Moon, Contrast, Check } from 'lucide-react';
import type { Theme } from '../hooks/useTheme';

interface ThemeSelectorProps {
  theme: Theme;
  setTheme: (t: Theme) => void;
}

const THEME_OPTIONS: { value: Theme; icon: ReactNode; label: string }[] = [
  { value: 'light', icon: <Sun size={16} aria-hidden="true" />, label: '浅色' },
  { value: 'dark', icon: <Moon size={16} aria-hidden="true" />, label: '深色' },
  { value: 'high-contrast', icon: <Contrast size={16} aria-hidden="true" />, label: '高对比度' },
];

export default function ThemeSelector({ theme, setTheme }: ThemeSelectorProps) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // 点击外部关闭下拉
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  const current = THEME_OPTIONS.find((o) => o.value === theme) || THEME_OPTIONS[0];

  return (
    <div ref={containerRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        className="flex items-center gap-1 rounded-control px-3 py-1.5 text-body transition"
        style={{
          backgroundColor: 'var(--bg-card)',
          border: '1px solid var(--border-color)',
          color: 'var(--text-primary)',
        }}
        title="切换主题"
      >
        <span className="flex items-center">{current.icon}</span>
        <span className="hidden sm:inline">{current.label}</span>
      </button>

      {open && (
        <div
          className="absolute right-0 top-full z-50 mt-1 min-w-[140px] overflow-hidden rounded-card border shadow-pop"
          style={{
            backgroundColor: 'var(--bg-card)',
            borderColor: 'var(--border-color)',
          }}
        >
          {THEME_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              onClick={() => {
                setTheme(option.value);
                setOpen(false);
              }}
              className="flex w-full items-center gap-2 px-3 py-2 text-sm transition"
              style={{
                color: theme === option.value ? 'var(--accent-color)' : 'var(--text-primary)',
                backgroundColor: theme === option.value ? 'var(--accent-light)' : 'transparent',
              }}
              onMouseEnter={(e) => {
                if (theme !== option.value) {
                  e.currentTarget.style.backgroundColor = 'var(--surface-hover)';
                }
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor =
                  theme === option.value ? 'var(--accent-light)' : 'transparent';
              }}
            >
              <span className="flex items-center">{option.icon}</span>
              <span>{option.label}</span>
              {theme === option.value && <Check size={14} className="ml-auto" aria-hidden="true" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
