// 全局组件基座基本渲染 / 交互用例。
// 不依赖 @testing-library，直接用 react-dom/client + act 驱动。

// 声明为 React act 测试环境，消除 "not configured to support act" 警告
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { Badge } from './Badge';
import { Card } from './Card';
import { ConfirmDialog } from './ConfirmDialog';
import { IconBtn } from './IconBtn';
import { Segmented } from './Segmented';
import { ToastProvider, useToast } from './Toast';
import {
  Skeleton,
  PersonCardSkeleton,
  PersonListSkeleton,
  SessionListSkeleton,
  PersonDetailSkeleton,
} from './Skeleton';

let container: HTMLDivElement;
let root: Root;

function render(ui: React.ReactElement) {
  act(() => root.render(ui));
}

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.restoreAllMocks();
});

function click(el: Element | null) {
  expect(el).not.toBeNull();
  act(() => {
    el!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  });
}

function buttonByText(text: string): HTMLButtonElement | null {
  return Array.from(container.querySelectorAll('button')).find(
    (b) => b.textContent === text
  ) as HTMLButtonElement | null;
}

describe('Badge', () => {
  it('渲染内容并支持变体切换', () => {
    render(<Badge variant="success">已激活</Badge>);
    expect(container.textContent).toContain('已激活');
    const span = container.querySelector('span')!;
    expect(span.style.backgroundColor).toBe('var(--success, rgba(22, 163, 74, 0.12))');
    render(<Badge>默认</Badge>);
    expect(container.querySelector('span')!.style.backgroundColor).toBe('var(--surface-hover)');
  });
});

describe('Card', () => {
  it('渲染子内容，padding 可自定义', () => {
    render(<Card padding="8px">卡片内容</Card>);
    const div = container.firstElementChild as HTMLElement;
    expect(div.textContent).toContain('卡片内容');
    expect(div.style.padding).toBe('8px');
    expect(div.style.borderRadius).toBe('var(--card-radius, 0.75rem)');
  });
});

describe('IconBtn', () => {
  it('点击触发 onClick，title 同时作为 tooltip 与 aria-label', () => {
    const onClick = vi.fn();
    render(
      <IconBtn title="删除" onClick={onClick}>
        <svg data-testid="icon" />
      </IconBtn>
    );
    const btn = container.querySelector('button')!;
    expect(btn.getAttribute('title')).toBe('删除');
    expect(btn.getAttribute('aria-label')).toBe('删除');
    click(btn);
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('disabled 时不响应点击', () => {
    const onClick = vi.fn();
    render(
      <IconBtn title="删除" disabled onClick={onClick}>
        <svg />
      </IconBtn>
    );
    click(container.querySelector('button'));
    expect(onClick).not.toHaveBeenCalled();
  });
});

describe('Segmented', () => {
  it('受控渲染：高亮当前项，点击回调传出 value', () => {
    const onChange = vi.fn();
    render(
      <Segmented
        options={[
          { label: '全部', value: 'all' },
          { label: '待跟进', value: 'follow' },
        ]}
        value="all"
        onChange={onChange}
      />
    );
    const all = buttonByText('全部')!;
    const follow = buttonByText('待跟进')!;
    expect(all.getAttribute('aria-pressed')).toBe('true');
    expect(follow.getAttribute('aria-pressed')).toBe('false');
    click(follow);
    expect(onChange).toHaveBeenCalledWith('follow');
  });

  it('disabled 选项不可点击', () => {
    const onChange = vi.fn();
    render(
      <Segmented
        options={[
          { label: 'A', value: 'a' },
          { label: 'B', value: 'b', disabled: true },
        ]}
        value="a"
        onChange={onChange}
      />
    );
    click(buttonByText('B'));
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe('ConfirmDialog', () => {
  it('渲染标题与正文，确认/取消按钮分别回调', () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(
      <ConfirmDialog
        title="删除联系人"
        message="删除后不可恢复"
        confirmLabel="删除"
        cancelLabel="再想想"
        danger
        onConfirm={onConfirm}
        onCancel={onCancel}
      />
    );
    expect(container.textContent).toContain('删除联系人');
    expect(container.textContent).toContain('删除后不可恢复');
    click(buttonByText('删除'));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    click(buttonByText('再想想'));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('Esc 键触发 onCancel', () => {
    const onCancel = vi.fn();
    render(
      <ConfirmDialog title="t" message="m" onConfirm={() => {}} onCancel={onCancel} />
    );
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('open=false 时不渲染且不响应 Esc', () => {
    const onCancel = vi.fn();
    render(
      <ConfirmDialog title="t" message="m" open={false} onConfirm={() => {}} onCancel={onCancel} />
    );
    expect(container.textContent).toBe('');
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('confirmDisabled 时确认按钮禁用', () => {
    render(
      <ConfirmDialog title="t" message="m" confirmDisabled onConfirm={() => {}} onCancel={() => {}} />
    );
    expect(buttonByText('确认')!.disabled).toBe(true);
  });
});

describe('Toast', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function ToastHarness() {
    const { success, error } = useToast();
    return (
      <div>
        <button type="button" onClick={() => success('已保存')}>
          触发成功
        </button>
        <button type="button" onClick={() => error('出错了', { duration: 1000 })}>
          触发错误
        </button>
      </div>
    );
  }

  it('success 提示出现并在默认时长后自动消失', () => {
    render(
      <ToastProvider>
        <ToastHarness />
      </ToastProvider>
    );
    click(buttonByText('触发成功'));
    expect(document.body.textContent).toContain('已保存');
    act(() => {
      vi.advanceTimersByTime(3999);
    });
    expect(document.body.textContent).toContain('已保存');
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(document.body.textContent).not.toContain('已保存');
  });

  it('error 提示支持自定义时长，且 role=alert', () => {
    render(
      <ToastProvider>
        <ToastHarness />
      </ToastProvider>
    );
    click(buttonByText('触发错误'));
    expect(document.querySelector('[role="alert"]')).not.toBeNull();
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(document.body.textContent).not.toContain('出错了');
  });

  it('点击关闭按钮可手动关闭', () => {
    render(
      <ToastProvider>
        <ToastHarness />
      </ToastProvider>
    );
    click(buttonByText('触发成功'));
    expect(document.body.textContent).toContain('已保存');
    const closeBtn = Array.from(document.querySelectorAll('button[aria-label]')).find(
      (b) => b.getAttribute('aria-label') === '关闭提示'
    );
    click(closeBtn ?? null);
    expect(document.body.textContent).not.toContain('已保存');
  });

  it('多条 toast 可堆叠共存', () => {
    render(
      <ToastProvider>
        <ToastHarness />
      </ToastProvider>
    );
    click(buttonByText('触发成功'));
    click(buttonByText('触发错误'));
    expect(document.body.textContent).toContain('已保存');
    expect(document.body.textContent).toContain('出错了');
  });

  it('useToast 在 Provider 外调用时抛错', () => {
    function Orphan() {
      useToast();
      return null;
    }
    // React 会把 hook 抛出的错误向上传播，这里直接断言渲染报错
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<Orphan />)).toThrow('useToast 必须在 <ToastProvider> 内使用');
    spy.mockRestore();
  });
});

describe('admin/shared re-export 兼容性', () => {
  it('ConfirmDialog 等可从 admin/shared 引入且行为一致', async () => {
    const shared = await import('../admin/shared');
    const ui = await import('./index');
    expect(shared.ConfirmDialog).toBe(ui.ConfirmDialog);
    expect(shared.LoadingSpinner).toBe(ui.LoadingSpinner);
    expect(shared.EmptyState).toBe(ui.EmptyState);
    expect(shared.ErrorBanner).toBe(ui.ErrorBanner);
    expect(shared.StatusBadge).toBe(ui.StatusBadge);
    expect(typeof shared.AdminTabButton).toBe('function');

    render(<shared.EmptyState text="暂无数据" />);
    expect(container.textContent).toContain('暂无数据');
    render(<shared.ErrorBanner message="加载失败" />);
    expect(container.textContent).toContain('加载失败');
    render(<shared.StatusBadge active={false} />);
    expect(container.textContent).toContain('禁用');
  });
});

describe('Skeleton（UX P2-12 骨架屏）', () => {
  it('基础骨架条走令牌底色 + pulse 动画，且 aria-hidden', () => {
    render(<Skeleton className="h-4 w-1/2" />);
    const el = container.firstElementChild as HTMLElement;
    expect(el.getAttribute('aria-hidden')).toBe('true');
    expect(el.className).toContain('animate-pulse');
    expect(el.style.backgroundColor).toBe('var(--surface-hover)');
    expect(el.className).toContain('h-4');
  });

  it('PersonListSkeleton 按 count 渲染卡片骨架并声明加载语义', () => {
    render(<PersonListSkeleton count={4} />);
    const status = container.querySelector('[role="status"]');
    expect(status?.getAttribute('aria-label')).toBe('联系人列表加载中');
    // 每张卡片骨架内含一个圆形头像占位（rounded-full 头像宽 h-10）
    expect(container.querySelectorAll('.h-10.w-10').length).toBe(4);
  });

  it('SessionListSkeleton 默认渲染 6 行骨架', () => {
    render(<SessionListSkeleton />);
    const status = container.querySelector('[role="status"]');
    expect(status?.getAttribute('aria-label')).toBe('会话列表加载中');
    expect(status?.children.length).toBe(6);
  });

  it('PersonDetailSkeleton 声明详情加载语义且不含真实文案', () => {
    render(<PersonDetailSkeleton />);
    const status = container.querySelector('[role="status"]');
    expect(status?.getAttribute('aria-label')).toBe('联系人详情加载中');
    expect(container.textContent).toBe('');
  });

  it('骨架组件从 ui/index 统一导出', async () => {
    const ui = await import('./index');
    expect(ui.Skeleton).toBe(Skeleton);
    expect(ui.PersonCardSkeleton).toBe(PersonCardSkeleton);
    expect(ui.PersonListSkeleton).toBe(PersonListSkeleton);
    expect(ui.SessionListSkeleton).toBe(SessionListSkeleton);
    expect(ui.PersonDetailSkeleton).toBe(PersonDetailSkeleton);
  });
});
