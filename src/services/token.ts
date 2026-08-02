// Token 存储工具 — 被 api.ts 和 useAuth.ts 共同引用，避免循环依赖

export const ACCESS_TOKEN_KEY = 'rg_access_token';
export const REFRESH_TOKEN_KEY = 'rg_refresh_token';

export const API_BASE: string =
  (import.meta.env.VITE_API_BASE as string | undefined) ??
  `http://${window.location.hostname}:8790`;

export function getAccessToken(): string | null {
  return localStorage.getItem(ACCESS_TOKEN_KEY);
}

export function getRefreshToken(): string | null {
  return localStorage.getItem(REFRESH_TOKEN_KEY);
}

export function saveTokens(access: string, refresh: string): void {
  localStorage.setItem(ACCESS_TOKEN_KEY, access);
  localStorage.setItem(REFRESH_TOKEN_KEY, refresh);
}

export function clearTokens(): void {
  localStorage.removeItem(ACCESS_TOKEN_KEY);
  localStorage.removeItem(REFRESH_TOKEN_KEY);
}

/** 解析JWT payload中的exp字段（秒级时间戳） */
export function getTokenExp(token: string): number | null {
  try {
    const payload = JSON.parse(atob(token.split('.')[1]));
    return typeof payload.exp === 'number' ? payload.exp : null;
  } catch {
    return null;
  }
}
