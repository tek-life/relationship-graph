/**
 * 用户认证状态 hook
 * 管理当前登录用户信息，提供登录/登出/状态查询能力
 */
import { useCallback, useEffect, useState } from 'react';
import { getCurrentUser } from '../services/auth';
import type { User } from '../types';

export interface UseAuthReturn {
  /** 当前登录用户，未登录时为 null */
  user: User | null;
  /** 是否正在加载用户信息 */
  loading: boolean;
  /** 认证错误信息 */
  error: string;
  /** 刷新当前用户信息 */
  refresh: () => Promise<void>;
  /** 设置用户（登录成功后调用） */
  setUser: (user: User) => void;
  /** 清除用户状态（登出） */
  logout: () => void;
}

export function useAuth(): UseAuthReturn {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const refresh = useCallback(async () => {
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
    refresh();
  }, [refresh]);

  const logout = useCallback(() => {
    setUser(null);
    localStorage.removeItem('token');
  }, []);

  return { user, loading, error, refresh, setUser, logout };
}
