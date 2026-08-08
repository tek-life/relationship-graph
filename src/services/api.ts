// HTTP API 客户端：替代原 Tauri invoke，指向 WSL 上的 Axum 服务端。
// API 地址默认取当前页面 host（Windows/手机浏览器经局域网访问时自动匹配），端口 8790。
//
// 认证模型（密钥文件机制）：数据库密钥由服务端密钥文件保管，启动即自动解锁，
// 前端不再有任何"解锁数据库"步骤，用户统一走账号密码登录/邀请注册。
//
// API_BASE 覆盖优先级（便于联调）：
// 1. localStorage 'rg_api_base'（控制台执行 localStorage.setItem('rg_api_base', 'http://host:port') 即可切换）
// 2. import.meta.env.VITE_API_BASE
// 3. 页面经 Caddy 反代（端口 8080）服务时走同源（API_BASE=''，请求 /api/* 由 Caddy 转发到 8790，
//    无需对外暴露 8790，ECS/公网部署必须走这条）
// 4. 其余场景（Vite 开发 1420 / 无 Caddy 的静态回退）默认 http://{当前页面 host}:8790

function resolveApiBase(): string {
  try {
    const override = window.localStorage.getItem('rg_api_base');
    if (override) {
      return override.trim().replace(/\/+$/, '');
    }
  } catch {
    // localStorage 不可用（隐私模式等）时忽略
  }
  const envBase = import.meta.env.VITE_API_BASE as string | undefined;
  if (envBase) {
    return envBase;
  }
  if (window.location.port === '8080') {
    return '';
  }
  return `http://${window.location.hostname}:8790`;
}

export const API_BASE: string = resolveApiBase();

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
  // 204 / 空响应体（如 DELETE /api/sessions/:id 删除成功）：直接返回 null，
  // 避免 response.json() 解析空体抛错，中断调用方的成功分支
  if (response.status === 204) {
    return null as T;
  }
  const text = await response.text();
  if (!text) {
    return null as T;
  }
  return JSON.parse(text) as T;
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
