import { useEffect, useState } from 'react';
import { AUTH_EXPIRED_EVENT } from '../services/api';
import { checkDbState, loadDatabaseFromKeychain, setupDatabase, unlockDatabase } from '../services/security';
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

type GateMode = 'setup' | 'unlock' | 'login' | 'register' | 'ready';

export default function PasswordGate({ children }: Props) {
  const [loading, setLoading] = useState(true);
  const [mode, setMode] = useState<GateMode>('unlock');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
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
        if (!state.initialized) {
          setMode('setup');
          return;
        }
        if (state.unlocked) {
          setMode('ready');
          return;
        }
        if (state.hasStoredKey) {
          await loadDatabaseFromKeychain();
          setMode('ready');
          return;
        }
        setMode('unlock');
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
      setMode('unlock');
      setPassword('');
      setError('登录会话已过期，请重新输入主密码解锁。');
    };
    window.addEventListener(AUTH_EXPIRED_EVENT, onAuthExpired);
    return () => window.removeEventListener(AUTH_EXPIRED_EVENT, onAuthExpired);
  }, []);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setError('');
    try {
      if (mode === 'setup') {
        await setupDatabase(password);
      } else {
        await unlockDatabase(password);
      }
      setMode('ready');
    } catch (err) {
      setError(String(err));
    }
  };

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

  // 邀请注册流程
  if (mode === 'register' && inviteToken) {
    return <RegisterForm inviteToken={inviteToken} onRegistered={handleAuthSuccess} />;
  }

  // 账号密码登录流程
  if (mode === 'login') {
    return (
      <LoginForm
        onLoggedIn={handleAuthSuccess}
        onSwitchToMasterPassword={() => {
          setMode('unlock');
          setError('');
        }}
      />
    );
  }

  if (loading) {
    return <div className="flex min-h-screen items-center justify-center bg-slate-100 text-slate-600">正在连接服务端...</div>;
  }

  if (mode === 'ready') {
    return <>{children}</>;
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-slate-100 p-6">
      <form onSubmit={handleSubmit} className="w-full max-w-md rounded-2xl bg-white p-8 shadow-lg">
        <h1 className="text-2xl font-bold text-slate-900">{mode === 'setup' ? '初始化加密数据库' : '解锁数据库'}</h1>
        <p className="mt-2 text-sm text-slate-500">
          {mode === 'setup'
            ? '请设置主密码。数据将在服务端使用 SQLCipher 加密存储，密码不会明文保存。'
            : '请输入主密码解锁数据库。'}
        </p>
        <input
          className="input mt-6"
          type="password"
          placeholder="主密码"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          autoFocus
        />
        {error && <p className="mt-3 rounded bg-red-50 p-3 text-sm text-red-700">{error}</p>}
        <button className="btn-primary mt-6 w-full" type="submit">
          {mode === 'setup' ? '创建数据库' : '解锁'}
        </button>

        {/* 仅在解锁模式下，且数据库已初始化时，显示切换到账号密码登录的入口 */}
        {mode === 'unlock' && (
          <button
            type="button"
            className="mt-4 w-full text-center text-sm"
            style={{ color: 'var(--accent-color)' }}
            onClick={() => {
              setMode('login');
              setError('');
            }}
          >
            使用账号密码登录
          </button>
        )}
      </form>
    </div>
  );
}
