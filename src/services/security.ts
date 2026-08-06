import { apiGet } from './api';

export interface DbState {
  /** 系统已初始化（密钥文件 + 数据库齐备），可直接登录 */
  initialized: boolean;
  /** 老库（主密码派生密钥）尚未迁移到密钥文件机制，需一次性迁移 */
  needsMigration: boolean;
}

export async function checkDbState(): Promise<DbState> {
  return apiGet<DbState>('/api/auth/state');
}
