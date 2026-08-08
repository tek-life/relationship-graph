import { useEffect, useRef, useState } from 'react';
import { ChevronDown, Compass, LogOut, ShieldCheck } from 'lucide-react';

/**
 * UX P0-5：顶栏右侧用户菜单。
 * 导航 IA 收敛后，「内观画像」从顶栏 tabs 降级到此菜单；
 * 「管理后台」（仅 admin 可见）也从此菜单进入。既有路由均不变。
 */
interface UserMenuProps {
  displayName: string;
  isAdmin: boolean;
  onProfile: () => void;
  onAdmin: () => void;
  onLogout: () => void;
}

export default function UserMenu({ displayName, isAdmin, onProfile, onAdmin, onLogout }: UserMenuProps) {
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

  const trigger = (action: () => void) => () => {
    setOpen(false);
    action();
  };

  const itemClass =
    'flex w-full items-center gap-2 rounded-control px-3 py-2 text-body text-text-primary transition-colors hover:bg-surface';

  return (
    <div ref={containerRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        aria-haspopup="menu"
        aria-expanded={open}
        className="flex items-center gap-1.5 rounded-control border border-line bg-card px-3 py-1.5 text-body text-text-primary transition-colors hover:bg-surface"
      >
        <span className="max-w-[10rem] truncate">{displayName}</span>
        <ChevronDown size={14} aria-hidden="true" className="text-muted" />
      </button>

      {open && (
        <div
          role="menu"
          className="absolute right-0 top-full z-50 mt-1 min-w-[160px] rounded-card border border-line bg-card p-1.5 shadow-pop"
        >
          <button type="button" role="menuitem" className={itemClass} onClick={trigger(onProfile)}>
            <Compass size={16} aria-hidden="true" className="text-text-secondary" />
            内观画像
          </button>
          {isAdmin && (
            <button type="button" role="menuitem" className={itemClass} onClick={trigger(onAdmin)}>
              <ShieldCheck size={16} aria-hidden="true" className="text-text-secondary" />
              管理后台
            </button>
          )}
          <div className="my-1 h-px bg-line" aria-hidden="true" />
          <button
            type="button"
            role="menuitem"
            className="flex w-full items-center gap-2 rounded-control px-3 py-2 text-body text-danger transition-colors hover:bg-danger-light"
            onClick={trigger(onLogout)}
          >
            <LogOut size={16} aria-hidden="true" />
            注销
          </button>
        </div>
      )}
    </div>
  );
}
