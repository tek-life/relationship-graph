// 管理后台 sidebar IA + 页头范式用例（UX P1-10）。
// 不依赖 @testing-library，直接用 react-dom/client + act 驱动，
// 与 ui.test.tsx 的测试风格保持一致。

// 声明为 React act 测试环境，消除 "not configured to support act" 警告
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { Bot } from 'lucide-react';

// MarkdownContent 间接依赖 react-syntax-highlighter（CJS 引 ESM refractor），
// 在 vitest 收集阶段会报错；本用例不验证 Markdown 渲染，直接 mock 掉。
vi.mock('../MarkdownContent', () => ({ default: () => null }));

import AdminPanel from '../AdminPanel';
import { AdminPageHeader, AdminSideNav } from './shared';
import type { AdminNavGroup } from './shared';

let container: HTMLDivElement;
let root: Root;

function render(ui: React.ReactElement) {
  act(() => root.render(ui));
}

/** 等待若干轮微任务，让 fetch mock 驱动的 state 更新落地 */
async function flush(turns = 4) {
  for (let i = 0; i < turns; i += 1) {
    await act(async () => {
      await Promise.resolve();
    });
  }
}

function click(el: Element | null) {
  expect(el).not.toBeNull();
  act(() => {
    el!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  });
}

function buttonByText(text: string): HTMLButtonElement | null {
  return Array.from(container.querySelectorAll('button')).find((b) =>
    (b.textContent ?? '').includes(text),
  ) as HTMLButtonElement | null;
}

const originalFetch = globalThis.fetch;

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);

  // 管理后台各子页挂载即 fetch；统一返回空数据，
  // /api/admin/config 返回最小可用的 SystemConfig 结构。
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      const body =
        url.includes('/api/admin/config') && !url.includes('cloud-api-key')
          ? { cloudApiKey: { configured: false, dbConfigured: false, mask: null, source: null } }
          : [];
      return {
        ok: true,
        status: 200,
        text: async () => JSON.stringify(body),
        json: async () => body,
      };
    }),
  );
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.unstubAllGlobals();
  globalThis.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('AdminPageHeader（统一页头范式）', () => {
  it('渲染标题 + 描述 + 主操作区', () => {
    render(
      <AdminPageHeader
        title="数字人管理"
        description="配置数字人档案（当前 2 个）"
        actions={<button type="button">+ 新建数字人</button>}
      />,
    );
    const header = container.querySelector('header')!;
    expect(header).not.toBeNull();
    expect(header.querySelector('h3')!.textContent).toBe('数字人管理');
    expect(header.querySelector('p')!.textContent).toBe('配置数字人档案（当前 2 个）');
    expect(header.textContent).toContain('+ 新建数字人');
  });

  it('actions 省略时不渲染主操作区', () => {
    render(<AdminPageHeader title="系统设置" description="全局配置" />);
    const header = container.querySelector('header')!;
    // 仅标题块一个子元素
    expect(header.children.length).toBe(1);
  });
});

const TEST_GROUPS: AdminNavGroup[] = [
  {
    title: '智能配置',
    items: [
      { key: 'agents', label: '数字人管理', icon: Bot },
      { key: 'skill-packages', label: '技能包', icon: Bot },
    ],
  },
  { title: '系统管理', items: [{ key: 'users', label: '用户管理', icon: Bot }] },
];

describe('AdminSideNav（sidebar IA 基座）', () => {
  it('按分组渲染导航项并标记激活态', () => {
    render(<AdminSideNav groups={TEST_GROUPS} active="agents" onSelect={() => {}} />);
    expect(container.textContent).toContain('智能配置');
    expect(container.textContent).toContain('系统管理');

    const nav = container.querySelector('nav')!;
    const buttons = Array.from(nav.querySelectorAll('button'));
    expect(buttons.length).toBe(3);

    const activeBtn = buttons.find((b) => b.textContent!.includes('数字人管理'))!;
    expect(activeBtn.getAttribute('aria-current')).toBe('page');
    // 激活态软背景走设计令牌
    expect(activeBtn.style.backgroundColor).toBe('var(--accent-light)');
    // 激活项含左侧指示条
    expect(activeBtn.querySelector('span[aria-hidden]')).not.toBeNull();

    const idleBtn = buttons.find((b) => b.textContent!.includes('用户管理'))!;
    expect(idleBtn.getAttribute('aria-current')).toBeNull();
    expect(idleBtn.style.backgroundColor).toBe('');
  });

  it('点击导航项回调对应 key', () => {
    const onSelect = vi.fn();
    render(<AdminSideNav groups={TEST_GROUPS} active="agents" onSelect={onSelect} />);
    click(buttonByText('用户管理'));
    expect(onSelect).toHaveBeenCalledWith('users');
  });
});

describe('AdminPanel（sidebar IA 整合）', () => {
  it('渲染分组 sidebar，默认激活数字人管理', async () => {
    render(<AdminPanel />);
    await flush();

    const nav = container.querySelector('nav[aria-label="管理后台导航"]')!;
    expect(nav).not.toBeNull();
    for (const label of ['数字人管理', '技能包', '内观画像指令', '用户管理', '邀请管理', '系统设置']) {
      expect(nav.textContent).toContain(label);
    }

    // 默认页：数字人管理激活，内容区展示其页头
    const activeBtn = Array.from(nav.querySelectorAll('button')).find(
      (b) => b.getAttribute('aria-current') === 'page',
    )!;
    expect(activeBtn.textContent).toContain('数字人管理');
    expect(container.querySelector('main h3')!.textContent).toBe('数字人管理');
  });

  it('点击 sidebar 切换子页并渲染对应页头', async () => {
    render(<AdminPanel />);
    await flush();

    click(buttonByText('系统设置'));
    await flush();
    expect(container.querySelector('main h3')!.textContent).toBe('系统设置');

    click(buttonByText('用户管理'));
    await flush();
    expect(container.querySelector('main h3')!.textContent).toBe('用户管理');

    click(buttonByText('邀请管理'));
    await flush();
    expect(container.querySelector('main h3')!.textContent).toBe('邀请管理');

    click(buttonByText('技能包'));
    await flush();
    expect(container.querySelector('main h3')!.textContent).toBe('技能包');

    click(buttonByText('内观画像指令'));
    await flush();
    expect(container.querySelector('main h3')!.textContent).toBe('内观画像指令');
  });

  it('不使用 window.confirm（验收红线）', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm');
    render(<AdminPanel />);
    await flush();
    click(buttonByText('用户管理'));
    await flush();
    expect(confirmSpy).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });
});
