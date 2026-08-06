// HTTP API 客户端：替代原 Tauri invoke，指向 WSL 上的 Axum 服务端。
// API 地址默认取当前页面 host（Windows/手机浏览器经局域网访问时自动匹配），端口 8790。
//
// 认证模型（密钥文件机制）：数据库密钥由服务端密钥文件保管，启动即自动解锁，
// 前端不再有任何"解锁数据库"步骤，用户统一走账号密码登录/邀请注册。

export const API_BASE: string =
  (import.meta.env.VITE_API_BASE as string | undefined) ??
  `http://${window.location.hostname}:8790`;

const TOKEN_KEY = 'rg_token';
const USER_KEY = 'rg_user';
export const AUTH_EXPIRED_EVENT = 'rg:auth-expired';

export function getToken(): string | null {
  return sessionStorage.getItem(TOKEN_KEY);
}

export function setToken(token: string): void {
  sessionStorage.setItem(TOKEN_KEY, token);
}

export function clearToken(): void {
  sessionStorage.removeItem(TOKEN_KEY);
}

export function hasToken(): boolean {
  return getToken() !== null;
}

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

  if (response.status === 401) {
    clearToken();
    sessionStorage.removeItem(USER_KEY);
    window.dispatchEvent(new CustomEvent(AUTH_EXPIRED_EVENT));
    // 未在登录页时自动跳转到登录页
    if (window.location.pathname !== '/login') {
      window.location.href = '/login';
    }
  }
  if (!response.ok) {
    if (response.status === 401) {
      throw new Error('登录会话已过期，请重新登录。');
    }
    if (response.status === 404 && path === '/api/chat') {
      throw new Error('当前服务端版本不支持通用问答接口 /api/chat，请重启并升级后端服务。');
    }

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
