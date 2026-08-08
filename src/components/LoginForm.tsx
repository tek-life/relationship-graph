import { useState } from 'react';
import { login } from '../services/auth';
import type { User } from '../types';

interface LoginFormProps {
  onLoggedIn: (token: string, user: User) => void;
  /** 可选：切换回主密码解锁模式的回调 */
  onSwitchToMasterPassword?: () => void;
}

export default function LoginForm({ onLoggedIn, onSwitchToMasterPassword }: LoginFormProps) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!username.trim()) {
      setError('请输入用户名');
      return;
    }
    if (!password) {
      setError('请输入密码');
      return;
    }

    setSubmitting(true);
    try {
      const res = await login(username.trim(), password);
      onLoggedIn(res.token, res.user);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="flex min-h-screen items-center justify-center p-6" style={{ background: 'var(--bg-primary)' }}>
      <form
        onSubmit={handleSubmit}
        className="w-full max-w-md rounded-2xl p-8 shadow-lg"
        style={{ background: 'var(--bg-card)', boxShadow: '0 4px 24px var(--shadow-color)' }}
      >
        {/* 标题 */}
        <div className="mb-6 text-center">
          <div
            className="mx-auto mb-3 flex h-14 w-14 items-center justify-center rounded-full text-2xl"
            style={{ background: 'var(--accent-light)', color: 'var(--accent-color)' }}
          >
            ⚿
          </div>
          <h1 className="text-2xl font-bold" style={{ color: 'var(--text-primary)' }}>
            登录
          </h1>
          <p className="mt-1 text-sm" style={{ color: 'var(--text-secondary)' }}>
            使用账号密码登录
          </p>
        </div>

        {/* 用户名 */}
        <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
          用户名
        </label>
        <input
          className="input mb-4"
          type="text"
          placeholder="输入用户名"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
          autoFocus
        />

        {/* 密码 */}
        <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
          密码
        </label>
        <input
          className="input mb-4"
          type="password"
          placeholder="输入密码"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
        />

        {/* 错误提示 */}
        {error && (
          <div
            className="mb-4 rounded-lg p-3 text-sm"
            style={{ background: 'var(--danger-light)', color: 'var(--danger-color)' }}
          >
            {error}
          </div>
        )}

        {/* 登录按钮 */}
        <button className="btn-primary w-full" type="submit" disabled={submitting}>
          {submitting ? '登录中…' : '登录'}
        </button>

        {/* 切换回主密码解锁 */}
        {onSwitchToMasterPassword && (
          <button
            type="button"
            className="mt-4 w-full text-center text-sm"
            style={{ color: 'var(--accent-color)' }}
            onClick={onSwitchToMasterPassword}
          >
            使用主密码解锁
          </button>
        )}
      </form>
    </div>
  );
}
