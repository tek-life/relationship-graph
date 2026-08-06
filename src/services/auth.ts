import { api, setToken } from './api';
import type { User } from '../types';

interface AuthResponse {
  token: string;
  user: User;
}

/** 通过邀请码注册新用户 */
export async function register(
  username: string,
  password: string,
  inviteToken: string,
): Promise<AuthResponse> {
  const res = await api<AuthResponse>('/api/auth/register', {
    method: 'POST',
    // 后端 RegisterRequest 使用 camelCase 序列化（inviteToken）
    body: JSON.stringify({ username, password, inviteToken }),
  });
  setToken(res.token);
  return res;
}

/** 用户名 + 密码登录 */
export async function login(username: string, password: string): Promise<AuthResponse> {
  const res = await api<AuthResponse>('/api/auth/login', {
    method: 'POST',
    body: JSON.stringify({ username, password }),
  });
  setToken(res.token);
  return res;
}

/** 全新部署初始化：创建管理员账号（服务端同时生成密钥文件与加密库） */
export async function setupAdmin(username: string, password: string): Promise<AuthResponse> {
  const res = await api<AuthResponse>('/api/auth/setup', {
    method: 'POST',
    body: JSON.stringify({ username, password }),
  });
  setToken(res.token);
  return res;
}

/** 老库一次性迁移：用旧主密码 rekey 为服务端密钥文件，admin 密码对齐为主密码 */
export async function migrateLegacy(password: string): Promise<AuthResponse> {
  const res = await api<AuthResponse>('/api/auth/migrate', {
    method: 'POST',
    body: JSON.stringify({ password }),
  });
  setToken(res.token);
  return res;
}

/** 获取当前登录用户信息 */
export async function getCurrentUser(): Promise<User> {
  return api<User>('/api/auth/me');
}
