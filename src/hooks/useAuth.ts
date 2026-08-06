/**
 * 用户认证状态 hook
 * 管理当前登录用户信息，提供登录/登出/状态查询能力
 */
import { useCallback, useEffect, useState } from 'react';
import { clearToken } from '../services/api';
import { login as authLogin, getCurrentUser } from '../services/auth';
import type { User } from '../types';

export interface UseAuthReturn {
  /** 当前登录用户，未登录时为 null */
  user: User | null;
  /** 是否正在加载用户信息 */
  loading: boolean;
  /** 认证错误信息 */
  error: string;
  /** 是否已登录 */
  isLoggedIn: boolean;
  /** 是否为管理员 */
  isAdmin: boolean;
  /** 登录 */
  login: (username: string, password: string) => Promise<void>;
  /** 刷新当前用户信息 */
  refreshUser: () => Promise<void>;
  /** 清除用户状态（登出） */
  logout: () => void;
}

export function useAuth(): UseAuthReturn {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  // 从后端获取当前用户信息
  const refreshUser = useCallback(async () => {
    try {
      setLoading(true);
      setError('');
      const currentUser = await getCurrentUser();
      setUser(currentUser);
    } catch (err) {
      setUser(null);
      setError(err instanceof Error ? err.message : '获取用户信息失败');
    } finally {
      setLoading(false);
    }
  }, []);

  // 组件挂载时自动获取用户信息
  useEffect(() => {
    refreshUser();
  }, [refreshUser]);

  // 登录（auth.ts 的 login 会调用 setToken 保存 token）
  const login = useCallback(async (username: string, password: string) => {
    const res = await authLogin(username, password);
    setUser(res.user);
  }, []);

  // 登出：使用 api.ts 的 clearToken 清除 sessionStorage 中的 token
  const logout = useCallback(() => {
    clearToken();
    setUser(null);
  }, []);

  return {
    user,
    loading,
    error,
    isLoggedIn: user !== null,
    isAdmin: user?.role === 'admin',
    login,
    refreshUser,
    logout,
  };
}
