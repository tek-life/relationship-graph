// HTTP API 客户端：替代原 Tauri invoke，指向 WSL 上的 Axum 服务端。
// API 地址默认取当前页面 host（Windows/手机浏览器经局域网访问时自动匹配），端口 8790。

import { API_BASE, clearTokens, getAccessToken, getRefreshToken, saveTokens } from './token';

export { API_BASE } from './token';

// ===== 兼容旧模式的Token管理 =====
// 旧模式 (VITE_LEGACY_AUTH=true) 仍使用 sessionStorage 的 rg_token
const LEGACY_TOKEN_KEY = 'rg_token';

export function getToken(): string | null {
  // 优先使用新JWT token，回退到旧的sessionStorage token
  return getAccessToken() || sessionStorage.getItem(LEGACY_TOKEN_KEY);
}

export function setToken(token: string): void {
  sessionStorage.setItem(LEGACY_TOKEN_KEY, token);
}

export function clearToken(): void {
  sessionStorage.removeItem(LEGACY_TOKEN_KEY);
}

export function hasToken(): boolean {
  return getToken() !== null;
}

// ===== 自动刷新逻辑 =====

let isRefreshing = false;
let refreshPromise: Promise<boolean> | null = null;

async function tryRefreshToken(): Promise<boolean> {
  const rt = getRefreshToken();
  if (!rt) return false;
  try {
    const res = await fetch(`${API_BASE}/api/auth/refresh`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ refreshToken: rt }),
    });
    if (!res.ok) return false;
    const data = await res.json();
    saveTokens(data.accessToken, data.refreshToken);
    return true;
  } catch {
    return false;
  }
}

/** 确保同时只有一个refresh请求 */
function refreshOnce(): Promise<boolean> {
  if (isRefreshing && refreshPromise) return refreshPromise;
  isRefreshing = true;
  refreshPromise = tryRefreshToken().finally(() => {
    isRefreshing = false;
    refreshPromise = null;
  });
  return refreshPromise;
}

// ===== 核心API请求函数 =====

export async function api<T>(path: string, options: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options.headers as Record<string, string> | undefined),
  };
  const token = getToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const response = await fetch(`${API_BASE}${path}`, { ...options, headers });

  // 401时尝试自动refresh
  if (response.status === 401) {
    const refreshed = await refreshOnce();
    if (refreshed) {
      // 用新token重试请求
      const newToken = getAccessToken();
      if (newToken) {
        headers.Authorization = `Bearer ${newToken}`;
      }
      const retryResponse = await fetch(`${API_BASE}${path}`, { ...options, headers });
      if (!retryResponse.ok) {
        let message = `请求失败（${retryResponse.status}）`;
        try {
          const body = await retryResponse.json();
          if (body && typeof body.error === 'string') message = body.error;
        } catch { /* ignore */ }
        throw new Error(message);
      }
      return retryResponse.json() as Promise<T>;
    }
    // refresh失败，清除所有token，触发重新登录
    clearToken();
    clearTokens();
    throw new Error('登录已过期，请重新登录');
  }

  if (!response.ok) {
    let message = `请求失败（${response.status}）`;
    try {
      const body = await response.json();
      if (body && typeof body.error === 'string') {
        message = body.error;
      }
    } catch {
      // 忽略响应体解析失败，保留状态码信息
    }
    throw new Error(message);
  }
  return response.json() as Promise<T>;
}

export function apiGet<T>(path: string): Promise<T> {
  return api<T>(path);
}

export function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return api<T>(path, { method: 'POST', body: body === undefined ? undefined : JSON.stringify(body) });
}

export function apiPut<T>(path: string, body: unknown): Promise<T> {
  return api<T>(path, { method: 'PUT', body: JSON.stringify(body) });
}

export function apiDelete<T>(path: string): Promise<T> {
  return api<T>(path, { method: 'DELETE' });
}

// ===== Auth API 函数 =====

export interface AuthLoginRequest {
  login: string;
  password: string;
}

export interface AuthRegisterRequest {
  username: string;
  password: string;
  email?: string;
  phone?: string;
  displayName?: string;
}

export interface AuthResponse {
  accessToken: string;
  refreshToken: string;
  user: {
    id: string;
    username: string;
    email?: string;
    phone?: string;
    displayName?: string;
  };
}

export interface AuthMeResponse {
  id: string;
  username: string;
  email?: string;
  phone?: string;
  displayName?: string;
}

export async function authLogin(req: AuthLoginRequest): Promise<AuthResponse> {
  const res = await fetch(`${API_BASE}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
  });
  if (!res.ok) {
    let message = `登录失败（${res.status}）`;
    try {
      const body = await res.json();
      if (body && typeof body.error === 'string') message = body.error;
    } catch { /* ignore */ }
    throw new Error(message);
  }
  return res.json();
}

export async function authRegister(req: AuthRegisterRequest): Promise<AuthResponse> {
  const res = await fetch(`${API_BASE}/api/auth/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
  });
  if (!res.ok) {
    let message = `注册失败（${res.status}）`;
    try {
      const body = await res.json();
      if (body && typeof body.error === 'string') message = body.error;
    } catch { /* ignore */ }
    throw new Error(message);
  }
  return res.json();
}

export async function authRefresh(refreshToken: string): Promise<AuthResponse> {
  const res = await fetch(`${API_BASE}/api/auth/refresh`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ refreshToken: refreshToken }),
  });
  if (!res.ok) {
    throw new Error('刷新token失败');
  }
  return res.json();
}

export async function authMe(token: string): Promise<AuthMeResponse> {
  const res = await fetch(`${API_BASE}/api/auth/me`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) {
    throw new Error('获取用户信息失败');
  }
  return res.json();
}

export async function authLock(): Promise<void> {
  await apiPost('/api/auth/lock');
}
