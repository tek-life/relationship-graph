import { apiGet, apiPost, hasToken, setToken } from './api';

export interface DbState {
  initialized: boolean;
  hasStoredKey: boolean;
  unlocked: boolean;
}

interface AuthStateResponse {
  initialized: boolean;
  unlocked: boolean;
}

interface TokenResponse {
  token: string;
}

export async function checkDbState(): Promise<DbState> {
  const state = await apiGet<AuthStateResponse>('/api/auth/state');
  return {
    initialized: state.initialized,
    // Web 端没有系统密钥链，改为检查本会话是否已持有 token
    hasStoredKey: state.unlocked && hasToken(),
    unlocked: state.unlocked && hasToken(),
  };
}

export async function setupDatabase(password: string): Promise<void> {
  const { token } = await apiPost<TokenResponse>('/api/auth/setup', { password });
  setToken(token);
}

export async function unlockDatabase(password: string): Promise<void> {
  const { token } = await apiPost<TokenResponse>('/api/auth/unlock', { password });
  setToken(token);
}

export async function loadDatabaseFromKeychain(): Promise<void> {
  // Web 端无系统密钥链；若本会话已有 token 即视为已解锁
  if (!hasToken()) {
    throw new Error('当前会话尚未解锁，请输入主密码');
  }
}

export async function forgetStoredKey(): Promise<void> {
  await apiPost<{ locked: boolean }>('/api/auth/lock');
}
