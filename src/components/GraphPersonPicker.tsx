// UX P1-9：图谱工具栏联系人选择器，替代原生 datalist。
// 输入即时过滤候选（姓名/代称前缀与包含匹配），点击候选回填输入框；
// 文本 → id 的解析仍由上层 resolveInput（label 精确匹配）完成，逻辑不变。

import { useEffect, useMemo, useRef, useState } from 'react';

export interface PickerOption {
  id: string;
  label: string;
}

interface Props {
  options: PickerOption[];
  value: string;
  /** 文本变化回调（上层负责 label → id 解析） */
  onChange: (text: string) => void;
  placeholder?: string;
  disabled?: boolean;
  ariaLabel?: string;
}

const MAX_SUGGESTIONS = 20;

export default function GraphPersonPicker({ options, value, onChange, placeholder, disabled, ariaLabel }: Props) {
  const [open, setOpen] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);

  // 候选过滤：空输入展示前 N 个；有输入按包含匹配（忽略大小写）
  const matches = useMemo(() => {
    const query = value.trim().toLowerCase();
    if (!query) return options.slice(0, MAX_SUGGESTIONS);
    return options
      .filter((opt) => opt.label.toLowerCase().includes(query))
      .slice(0, MAX_SUGGESTIONS);
  }, [options, value]);

  // 点击组件外部时收起候选面板
  useEffect(() => {
    if (!open) return;
    const onDocDown = (event: PointerEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', onDocDown);
    return () => document.removeEventListener('pointerdown', onDocDown);
  }, [open]);

  const exactMatch = matches.length === 1 && matches[0].label === value.trim();
  const showList = open && !exactMatch && matches.length > 0;

  return (
    <div ref={wrapperRef} className="relative">
      <input
        type="text"
        role="combobox"
        aria-expanded={showList}
        aria-label={ariaLabel}
        className="input !w-40 !py-1.5"
        placeholder={placeholder}
        value={value}
        disabled={disabled}
        onChange={(event) => {
          onChange(event.target.value);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onKeyDown={(event) => {
          if (event.key === 'Escape') setOpen(false);
        }}
      />
      {showList && (
        <ul
          role="listbox"
          className="absolute left-0 top-full z-30 mt-1 max-h-56 w-48 overflow-y-auto rounded-lg border bg-card py-1 shadow-lg"
        >
          {matches.map((opt) => (
            <li key={opt.id} role="option" aria-selected={opt.label === value.trim()}>
              <button
                type="button"
                className={`block w-full truncate px-3 py-1.5 text-left text-sm ${
                  opt.label === value.trim()
                    ? 'bg-accent-light text-accent'
                    : 'text-text-primary hover:bg-surface'
                }`}
                onClick={() => {
                  onChange(opt.label);
                  setOpen(false);
                }}
              >
                {opt.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
