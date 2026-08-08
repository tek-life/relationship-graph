import { useState } from 'react';
import { Sparkles } from 'lucide-react';
import { register } from '../services/auth';
import type { User } from '../types';

interface RegisterFormProps {
  inviteToken: string;
  onRegistered: (token: string, user: User) => void;
}

export default function RegisterForm({ inviteToken, onRegistered }: RegisterFormProps) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const validate = (): string | null => {
    if (!username.trim()) return '请输入用户名';
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
      const res = await register(username.trim(), password, inviteToken);
      onRegistered(res.token, res.user);
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
            <Sparkles size={24} aria-hidden="true" />
          </div>
          <h1 className="text-2xl font-bold" style={{ color: 'var(--text-primary)' }}>
            欢迎加入
          </h1>
          <p className="mt-1 text-sm" style={{ color: 'var(--text-secondary)' }}>
            你已被邀请加入个人关系图谱系统
          </p>
        </div>

        {/* 邀请码（只读） */}
        <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
          邀请码
        </label>
        <input
          className="input mb-4 cursor-not-allowed opacity-70"
          value={inviteToken}
          readOnly
          tabIndex={-1}
        />

        {/* 用户名 */}
        <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
          用户名 <span style={{ color: 'var(--danger-color)' }}>*</span>
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
          密码 <span style={{ color: 'var(--danger-color)' }}>*</span>
        </label>
        <input
          className="input mb-4"
          type="password"
          placeholder="至少 6 位"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="new-password"
        />

        {/* 确认密码 */}
        <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
          确认密码 <span style={{ color: 'var(--danger-color)' }}>*</span>
        </label>
        <input
          className="input mb-4"
          type="password"
          placeholder="再次输入密码"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          autoComplete="new-password"
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

        {/* 注册按钮 */}
        <button className="btn-primary w-full" type="submit" disabled={submitting}>
          {submitting ? '注册中…' : '注册并进入'}
        </button>
      </form>
    </div>
  );
}
