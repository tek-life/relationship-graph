import { useState } from 'react';
import { setupAdmin } from '../services/auth';
import type { User } from '../types';

interface AdminSetupFormProps {
  onCreated: (token: string, user: User) => void;
}

/** 全新部署首次访问：创建管理员账号（服务端同时生成密钥文件与加密库） */
export default function AdminSetupForm({ onCreated }: AdminSetupFormProps) {
  const [username, setUsername] = useState('admin');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const validate = (): string | null => {
    if (!username.trim()) return '请输入管理员用户名';
    if (username.trim().length < 2) return '用户名至少 2 个字符';
    if (!password) return '请输入密码';
    if (password.length < 8) return '密码长度至少 8 位';
    if (password !== confirmPassword) return '两次输入的密码不一致';
    return null;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    const validationError = validate();
    if (validationError) {
      setError(validationError);
      return;
    }

    setSubmitting(true);
    try {
      const res = await setupAdmin(username.trim(), password);
      onCreated(res.token, res.user);
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
        <div className="mb-6 text-center">
          <div
            className="mx-auto mb-3 flex h-14 w-14 items-center justify-center rounded-full text-2xl"
            style={{ background: 'var(--accent-light)', color: 'var(--accent-color)' }}
          >
            ⚿
          </div>
          <h1 className="text-2xl font-bold" style={{ color: 'var(--text-primary)' }}>
            初始化系统
          </h1>
          <p className="mt-1 text-sm" style={{ color: 'var(--text-secondary)' }}>
            首次使用，请创建管理员账号。数据将使用 SQLCipher 加密存储。
          </p>
        </div>

        <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
          管理员用户名
        </label>
        <input
          className="input mb-4"
          type="text"
          placeholder="如：admin"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
          autoFocus
        />

        <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
          密码（至少 8 位）
        </label>
        <input
          className="input mb-4"
          type="password"
          placeholder="输入密码"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="new-password"
        />

        <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
          确认密码
        </label>
        <input
          className="input mb-4"
          type="password"
          placeholder="再次输入密码"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          autoComplete="new-password"
        />

        {error && (
          <div
            className="mb-4 rounded-lg p-3 text-sm"
            style={{ background: 'rgba(220,38,38,0.08)', color: 'var(--danger-color)' }}
          >
            {error}
          </div>
        )}

        <button className="btn-primary w-full" type="submit" disabled={submitting}>
          {submitting ? '创建中…' : '创建管理员并进入'}
        </button>
      </form>
    </div>
  );
}
