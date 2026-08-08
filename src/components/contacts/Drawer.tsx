// UX P1-8：右侧抽屉容器（联系人页表单抽屉化）。
// overlay 与面板全部走设计令牌（--bg-primary / --bg-card / --border-color / --shadow-color），
// 三主题下无硬编码色值；进出场用 translate-x + opacity 过渡，无需额外 CSS。

import { useEffect, useState } from 'react';
import { X } from 'lucide-react';
import { IconBtn } from '../ui';

interface DrawerProps {
  /** 是否展开；false 时不渲染（便于常驻挂载一个实例） */
  open: boolean;
  /** 抽屉标题 */
  title: string;
  onClose: () => void;
  /** 面板最大宽度，默认 26rem */
  width?: string;
  children: React.ReactNode;
}

export function Drawer({ open, title, onClose, width = '26rem', children }: DrawerProps) {
  // 挂载后下一帧再切换可见态，触发 CSS 过渡
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    if (!open) {
      setVisible(false);
      return;
    }
    const frame = requestAnimationFrame(() => setVisible(true));
    return () => cancelAnimationFrame(frame);
  }, [open]);

  // Esc 关闭
  useEffect(() => {
    if (!open) return;
    const handler = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className={`fixed inset-0 z-50 transition-opacity duration-200 ${visible ? 'opacity-100' : 'opacity-0'}`}
      style={{ backgroundColor: 'color-mix(in srgb, var(--bg-primary) 55%, transparent)' }}
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={`absolute inset-y-0 right-0 flex w-full flex-col border-l shadow-pop transition-transform duration-200 ease-out ${visible ? 'translate-x-0' : 'translate-x-full'}`}
        style={{ maxWidth: width, backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex shrink-0 items-center justify-between border-b px-4 py-3" style={{ borderColor: 'var(--border-color)' }}>
          <h2 className="text-lead font-semibold text-text-primary">{title}</h2>
          <IconBtn title="关闭" onClick={onClose}>
            <X size={16} aria-hidden="true" />
          </IconBtn>
        </div>
        <div className="flex-1 overflow-y-auto p-4">{children}</div>
      </div>
    </div>
  );
}
