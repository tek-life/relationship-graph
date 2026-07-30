import { useEffect, useState } from 'react';
import { checkDbState, loadDatabaseFromKeychain, setupDatabase, unlockDatabase } from '../services/security';

interface Props {
  children: React.ReactNode;
}

export default function PasswordGate({ children }: Props) {
  const [loading, setLoading] = useState(true);
  const [mode, setMode] = useState<'setup' | 'unlock' | 'ready'>('unlock');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');

  useEffect(() => {
    async function bootstrap() {
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

  if (loading) {
    return <div className="flex min-h-screen items-center justify-center bg-slate-100 text-slate-600">正在加载本地数据库...</div>;
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
            ? '请设置主密码。数据库将使用 SQLCipher 加密，密钥会托管到系统密钥链。'
            : '请输入主密码解锁本地数据库。'}
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
      </form>
    </div>
  );
}
