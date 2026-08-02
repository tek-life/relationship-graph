import { useCallback, useEffect, useRef, useState } from 'react';
import { API_BASE, clearTokens, getAccessToken, getRefreshToken, getTokenExp, saveTokens } from '../services/token';

// ===== 类型定义 =====

export interface AuthUser {
  id: string;
  username: string;
  email?: string;
  phone?: string;
  displayName?: string;
}

export interface RegisterRequest {
  username: string;
  password: string;
  email?: string;
  phone?: string;
  displayName?: string;
}

interface AuthResponse {
  accessToken: string;
  refreshToken: string;
  user: AuthUser;
}

// 重新导出供外部使用
export { getAccessToken, getRefreshToken } from '../services/token';

// ===== Hook =====

export function useAuth() {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [loading, setLoading] = useState(true);
  const refreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 安排自动刷新：在过期前5分钟触发
  const scheduleRefresh = useCallback((accessToken: string) => {
    if (refreshTimerRef.current) {
      clearTimeout(refreshTimerRef.current);
      refreshTimerRef.current = null;
    }
    const exp = getTokenExp(accessToken);
    if (!exp) return;
    const now = Math.floor(Date.now() / 1000);
    const delay = (exp - now - 300) * 1000; // 提前5分钟
    if (delay <= 0) {
      // 已经需要刷新
      refreshToken();
      return;
    }
    refreshTimerRef.current = setTimeout(() => {
      refreshToken();
    }, delay);
  }, []);

  // 刷新token
  const refreshToken = useCallback(async (): Promise<boolean> => {
    const rt = getRefreshToken();
    if (!rt) {
      logout();
      return false;
    }
    try {
      const res = await fetch(`${API_BASE}/api/auth/refresh`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ refreshToken: rt }),
      });
      if (!res.ok) {
        logout();
        return false;
      }
      const data: AuthResponse = await res.json();
      saveTokens(data.accessToken, data.refreshToken);
      setUser(data.user);
      setIsAuthenticated(true);
      scheduleRefresh(data.accessToken);
      return true;
    } catch {
      logout();
      return false;
    }
  }, [scheduleRefresh]);

  // 登录
  const login = useCallback(async (username: string, password: string): Promise<void> => {
    const res = await fetch(`${API_BASE}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ login: username, password }),
    });
    if (!res.ok) {
      let message = `登录失败（${res.status}）`;
      try {
        const body = await res.json();
        if (body && typeof body.error === 'string') message = body.error;
      } catch { /* ignore */ }
      throw new Error(message);
    }
    const data: AuthResponse = await res.json();
    saveTokens(data.accessToken, data.refreshToken);
    setUser(data.user);
    setIsAuthenticated(true);
    scheduleRefresh(data.accessToken);
  }, [scheduleRefresh]);

  // 注册
  const register = useCallback(async (req: RegisterRequest): Promise<void> => {
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
    const data: AuthResponse = await res.json();
    saveTokens(data.accessToken, data.refreshToken);
    setUser(data.user);
    setIsAuthenticated(true);
    scheduleRefresh(data.accessToken);
  }, [scheduleRefresh]);

  // 退出
  const logout = useCallback(() => {
    clearTokens();
    setUser(null);
    setIsAuthenticated(false);
    if (refreshTimerRef.current) {
      clearTimeout(refreshTimerRef.current);
      refreshTimerRef.current = null;
    }
  }, []);

  // 启动时检查token有效性
  useEffect(() => {
    async function checkAuth() {
      const token = getAccessToken();
      if (!token) {
        setLoading(false);
        return;
      }
      try {
        const res = await fetch(`${API_BASE}/api/auth/me`, {
          headers: { Authorization: `Bearer ${token}` },
        });
        if (res.ok) {
          const userData: AuthUser = await res.json();
          setUser(userData);
          setIsAuthenticated(true);
          scheduleRefresh(token);
        } else if (res.status === 401) {
          // token过期，尝试refresh
          const refreshed = await refreshToken();
          if (!refreshed) {
            clearTokens();
          }
        } else {
          clearTokens();
        }
      } catch {
        // 网络错误时不清除token，允许离线状态
      } finally {
        setLoading(false);
      }
    }
    checkAuth();

    return () => {
      if (refreshTimerRef.current) {
        clearTimeout(refreshTimerRef.current);
      }
    };
  }, []);

  return {
    isAuthenticated,
    user,
    loading,
    login,
    register,
    logout,
    refreshToken,
  };
}
