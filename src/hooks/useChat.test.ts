/**
 * useChat.loadSessions 静默刷新（silent）语义单测。
 *
 * 背景：消息落库后的例行会话列表刷新若置 sessionsLoading=true，
 * SessionSidebar 会整体替换为骨架屏，导致每轮聊天闪一次。
 * 约定：silent 刷新不置 sessionsLoading、失败时保留现有列表；
 * 仅非 silent（首屏冷启动语义）才展示骨架屏并在失败时清空。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createElement, act } from 'react';
import { createRoot, type Root } from 'react-dom/client';

vi.mock('../services/session', () => ({
  listSessions: vi.fn(),
  getSessionMessages: vi.fn(),
  createSession: vi.fn(),
  addMessage: vi.fn(),
  updateSessionTitle: vi.fn(),
  deleteSession: vi.fn(),
}));

import * as sessionApi from '../services/session';
import { useChat } from './useChat';
import type { Session } from '../types';

(globalThis as Record<string, unknown>).IS_REACT_ACT_ENVIRONMENT = true;

const listSessionsMock = vi.mocked(sessionApi.listSessions);
const getSessionMessagesMock = vi.mocked(sessionApi.getSessionMessages);

/** 快速构造测试会话 */
function session(id: string): Session {
  return {
    id,
    userId: 'u1',
    title: `会话${id}`,
    createdAt: '2026-08-08T00:00:00Z',
    updatedAt: '2026-08-08T00:00:00Z',
  };
}

/** 可手动 resolve 的 Promise（用于在请求挂起期间断言中间态） */
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

/** 极简 renderHook：不引入 @testing-library/react，直接 createRoot + act */
function renderHook<T>(hook: () => T) {
  const result = { current: null as unknown as T };
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root: Root = createRoot(container);
  function Probe() {
    result.current = hook();
    return null;
  }
  act(() => {
    root.render(createElement(Probe));
  });
  const cleanup = () => {
    act(() => {
      root.unmount();
    });
    container.remove();
  };
  return { result, cleanup };
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  getSessionMessagesMock.mockResolvedValue([]);
});

describe('loadSessions silent 语义', () => {
  it('静默刷新全程不置 sessionsLoading，且结果正常更新列表', async () => {
    const first = deferred<Session[]>();
    listSessionsMock.mockReturnValueOnce(first.promise);
    const { result, cleanup } = renderHook(() => useChat('u1'));

    // 首屏冷启动：初始 loading 为 true（骨架屏覆盖）
    expect(result.current.sessionsLoading).toBe(true);
    await act(async () => {
      first.resolve([session('s1')]);
    });
    expect(result.current.sessionsLoading).toBe(false);
    expect(result.current.sessions.map((s) => s.id)).toEqual(['s1']);

    // 模拟消息落库后的静默刷新：请求挂起期间 loading 保持 false
    const pending = deferred<Session[]>();
    listSessionsMock.mockReturnValueOnce(pending.promise);
    await act(async () => {
      void result.current.loadSessions({ silent: true });
    });
    expect(result.current.sessionsLoading).toBe(false);
    // 刷新未返回前保留现有列表（不清空、不闪骨架屏）
    expect(result.current.sessions.map((s) => s.id)).toEqual(['s1']);

    // 静默刷新返回后列表更新，loading 仍为 false
    await act(async () => {
      pending.resolve([session('s1'), session('s2')]);
    });
    expect(result.current.sessionsLoading).toBe(false);
    expect(result.current.sessions.map((s) => s.id)).toEqual(['s1', 's2']);
    cleanup();
  });

  it('静默刷新失败时保留现有列表，不触发骨架屏', async () => {
    listSessionsMock.mockResolvedValueOnce([session('s1')]);
    const { result, cleanup } = renderHook(() => useChat('u1'));
    await act(async () => {});
    expect(result.current.sessions.map((s) => s.id)).toEqual(['s1']);
    expect(result.current.sessionsLoading).toBe(false);

    listSessionsMock.mockRejectedValueOnce(new Error('network'));
    await act(async () => {
      await result.current.loadSessions({ silent: true });
    });
    expect(result.current.sessionsLoading).toBe(false);
    expect(result.current.sessions.map((s) => s.id)).toEqual(['s1']);
    cleanup();
  });

  it('默认（非静默）刷新仍会置 sessionsLoading，供首屏冷启动展示骨架屏', async () => {
    const first = deferred<Session[]>();
    listSessionsMock.mockReturnValueOnce(first.promise);
    const { result, cleanup } = renderHook(() => useChat('u1'));
    await act(async () => {
      first.resolve([]);
    });
    expect(result.current.sessionsLoading).toBe(false);

    const pending = deferred<Session[]>();
    listSessionsMock.mockReturnValueOnce(pending.promise);
    await act(async () => {
      void result.current.loadSessions();
    });
    // 非静默刷新挂起期间展示骨架屏
    expect(result.current.sessionsLoading).toBe(true);
    await act(async () => {
      pending.resolve([session('s1')]);
    });
    expect(result.current.sessionsLoading).toBe(false);
    expect(result.current.sessions.map((s) => s.id)).toEqual(['s1']);
    cleanup();
  });
});
