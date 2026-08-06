import { useEffect, useState } from 'react';
import { AUTH_EXPIRED_EVENT } from '../services/api';
import { migrateLegacy } from '../services/auth';
import { checkDbState } from '../services/security';
import AdminSetupForm from './AdminSetupForm';
import RegisterForm from './RegisterForm';
import LoginForm from './LoginForm';
import type { User } from '../types';

interface Props {
  children: React.ReactNode;
}

const USER_KEY = 'rg_user';

/** 保存当前用户信息到 sessionStorage，供 App 层读取 */
export function saveUser(user: User): void {
  sessionStorage.setItem(USER_KEY, JSON.stringify(user));
}

export function loadUser(): User | null {
  const raw = sessionStorage.getItem(USER_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as User;
  } catch {
    return null;
  }
}

type GateMode = 'setup-admin' | 'migrate' | 'login' | 'register' | 'ready';

export default function PasswordGate({ children }: Props) {
  const [loading, setLoading] = useState(true);
  const [mode, setMode] = useState<GateMode>('login');
  const [error, setError] = useState('');
  const [migratePassword, setMigratePassword] = useState('');
  const [inviteToken] = useState<string | null>(() => {
    const params = new URLSearchParams(window.location.search);
    return params.get('invite');
  });

  useEffect(() => {
    async function bootstrap() {
      // 如果 URL 带有邀请码，直接进入注册流程
      if (inviteToken) {
        setMode('register');
        setLoading(false);
        return;
      }

      try {
        const state = await checkDbState();
        if (state.needsMigration) {
          setMode('migrate');
        } else if (!state.initialized) {
          setMode('setup-admin');
        } else {
          // 如果 sessionStorage 中有已保存的用户信息，直接恢复会话（避免深链刷新显示登录页）
          const savedUser = loadUser();
          if (savedUser) {
            setMode('ready');
          } else {
            setMode('login');
          }
        }
      } catch (err) {
        setError(String(err));
      } finally {
        setLoading(false);
      }
    }
    bootstrap();
  }, [inviteToken]);

  useEffect(() => {
    const onAuthExpired = () => {
      setMode('login');
      setError('登录会话已过期，请重新登录。');
    };
    window.addEventListener(AUTH_EXPIRED_EVENT, onAuthExpired);
    return () => window.removeEventListener(AUTH_EXPIRED_EVENT, onAuthExpired);
  }, []);

  const handleAuthSuccess = (_token: string, user: User) => {
    saveUser(user);
    // 注册/登录成功后清除 URL 中的 invite 参数
    if (inviteToken) {
      const url = new URL(window.location.href);
      url.searchParams.delete('invite');
      window.history.replaceState({}, '', url.toString());
    }
    setMode('ready');
  };

  if (loading) {
    return <div className="flex min-h-screen items-center justify-center bg-slate-100 text-slate-600">正在连接服务端...</div>;
  }

  if (mode === 'ready') {
    return <>{children}</>;
  }

  // 邀请注册流程
  if (mode === 'register' && inviteToken) {
    return <RegisterForm inviteToken={inviteToken} onRegistered={handleAuthSuccess} />;
  }

  // 全新部署：创建管理员账号
  if (mode === 'setup-admin') {
    return <AdminSetupForm onCreated={handleAuthSuccess} />;
  }

  // 老库一次性迁移
  if (mode === 'migrate') {
    const handleMigrate = async (event: React.FormEvent) => {
      event.preventDefault();
      setError('');
      try {
        const res = await migrateLegacy(migratePassword);
        handleAuthSuccess(res.token, res.user);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    };
    return (
      <div className="flex min-h-screen items-center justify-center p-6" style={{ background: 'var(--bg-primary)' }}>
        <form
          onSubmit={handleMigrate}
          className="w-full max-w-md rounded-2xl p-8 shadow-lg"
          style={{ background: 'var(--bg-card)', boxShadow: '0 4px 24px var(--shadow-color)' }}
        >
          <h1 className="text-2xl font-bold" style={{ color: 'var(--text-primary)' }}>系统升级（一次性）</h1>
          <p className="mt-2 text-sm" style={{ color: 'var(--text-secondary)' }}>
            检测到旧版本数据库。请输入原主密码完成升级：数据库将改用服务端密钥保管，
            之后统一使用账号密码登录，主密码同时成为管理员（admin）账号的登录密码。
          </p>
          <input
            className="input mt-6"
            type="password"
            placeholder="原主密码"
            value={migratePassword}
            onChange={(event) => setMigratePassword(event.target.value)}
            autoFocus
          />
          {error && <p className="mt-3 rounded bg-red-50 p-3 text-sm text-red-700">{error}</p>}
          <button className="btn-primary mt-6 w-full" type="submit">
            完成升级并登录
          </button>
        </form>
      </div>
    );
  }

  // 账号密码登录流程
  if (mode === 'login') {
    return <LoginForm onLoggedIn={handleAuthSuccess} />;
  }

  // 兜底：网络错误等
  return (
    <div className="flex min-h-screen items-center justify-center bg-slate-100 p-6">
      <div className="w-full max-w-md rounded-2xl bg-white p-8 shadow-lg">
        <h1 className="text-2xl font-bold text-slate-900">无法连接服务端</h1>
        <p className="mt-2 text-sm text-red-600">{error}</p>
        <button
          className="btn-primary mt-6 w-full"
          type="button"
          onClick={() => window.location.reload()}
        >
          重试
        </button>
      </div>
    </div>
  );
}
