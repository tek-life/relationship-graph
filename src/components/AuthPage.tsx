import { useState } from 'react';
import type { RegisterRequest } from '../hooks/useAuth';

interface Props {
  onLogin: (username: string, password: string) => Promise<void>;
  onRegister: (req: RegisterRequest) => Promise<void>;
}

type TabMode = 'login' | 'register';

export default function AuthPage({ onLogin, onRegister }: Props) {
  const [tab, setTab] = useState<TabMode>('login');

  return (
    <div
      className="relative flex min-h-screen items-center justify-center p-4"
      style={{ backgroundColor: 'var(--bg-primary)' }}
    >
      <div
        className="w-full max-w-md rounded-2xl border p-8 shadow-lg"
        style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
      >
        {/* Logo / 标题 */}
        <div className="mb-6 text-center">
          <h1 className="text-2xl font-bold" style={{ color: 'var(--text-primary)' }}>
            个人关系图谱
          </h1>
          <p className="mt-1 text-sm" style={{ color: 'var(--text-secondary)' }}>
            加密存储、多端协同、智能辅助
          </p>
        </div>

        {/* Tab 切换 */}
        <div className="mb-6 flex rounded-lg p-1" style={{ backgroundColor: 'var(--bg-secondary)' }}>
          <button
            type="button"
            className="flex-1 rounded-md py-2 text-sm font-medium transition"
            style={
              tab === 'login'
                ? { backgroundColor: 'var(--bg-card)', color: 'var(--text-primary)', boxShadow: '0 1px 3px var(--shadow-color)' }
                : { color: 'var(--text-secondary)' }
            }
            onClick={() => setTab('login')}
          >
            登录
          </button>
          <button
            type="button"
            className="flex-1 rounded-md py-2 text-sm font-medium transition"
            style={
              tab === 'register'
                ? { backgroundColor: 'var(--bg-card)', color: 'var(--text-primary)', boxShadow: '0 1px 3px var(--shadow-color)' }
                : { color: 'var(--text-secondary)' }
            }
            onClick={() => setTab('register')}
          >
            注册
          </button>
        </div>

        {/* 表单区域 */}
        {tab === 'login' ? (
          <LoginForm onLogin={onLogin} />
        ) : (
          <RegisterForm onRegister={onRegister} />
        )}

        {/* 分割线 + 第三方登录 */}
        <div className="mt-6 flex items-center gap-3">
          <div className="flex-1 border-t" style={{ borderColor: 'var(--border-color)' }} />
          <span className="text-xs" style={{ color: 'var(--text-muted)' }}>第三方登录</span>
          <div className="flex-1 border-t" style={{ borderColor: 'var(--border-color)' }} />
        </div>
        <div className="mt-4 flex justify-center gap-4">
          <OAuthButton label="微信" icon="💬" brandColor="#09B83E" />
          <OAuthButton label="钉钉" icon="📌" brandColor="#0089FF" />
          <OAuthButton label="飞书" icon="🪶" brandColor="#165DFF" />
        </div>
      </div>

      {/* 帮助文档链接 */}
      <div className="absolute bottom-4 left-0 right-0 text-center">
        <a
          href="/docs/help/user-help.html"
          target="_blank"
          rel="noopener noreferrer"
          className="text-xs transition hover:underline"
          style={{ color: 'var(--text-muted)' }}
        >
          使用帮助
        </a>
      </div>
    </div>
  );
}

// ===== 登录表单 =====

function LoginForm({ onLogin }: { onLogin: (u: string, p: string) => Promise<void> }) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!username.trim() || !password) return;
    setError('');
    setLoading(true);
    try {
      await onLogin(username.trim(), password);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div>
        <label className="mb-1 block text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          用户名 / 邮箱
        </label>
        <input
          className="input"
          type="text"
          placeholder="请输入用户名或邮箱"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
          autoFocus
        />
      </div>
      <div>
        <label className="mb-1 block text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          密码
        </label>
        <input
          className="input"
          type="password"
          placeholder="请输入密码"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
        />
      </div>
      {error && (
        <div className="rounded-lg p-3 text-sm" style={{ backgroundColor: 'rgba(220,38,38,0.1)', color: 'var(--danger-color)' }}>
          {error}
        </div>
      )}
      <button
        type="submit"
        className="btn-primary flex w-full items-center justify-center gap-2"
        disabled={loading || !username.trim() || !password}
      >
        {loading && <Spinner />}
        {loading ? '登录中...' : '登录'}
      </button>
    </form>
  );
}

// ===== 注册表单 =====

function RegisterForm({ onRegister }: { onRegister: (req: RegisterRequest) => Promise<void> }) {
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [phone, setPhone] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [emailCode, setEmailCode] = useState('');
  const [phoneCode, setPhoneCode] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!username.trim() || !password) return;
    if (password !== confirmPassword) {
      setError('两次输入的密码不一致');
      return;
    }
    if (password.length < 6) {
      setError('密码至少需要6个字符');
      return;
    }
    setError('');
    setLoading(true);
    try {
      await onRegister({
        username: username.trim(),
        password,
        email: email.trim() || undefined,
        phone: phone.trim() || undefined,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleSendCode = (type: 'email' | 'phone') => {
    alert(`${type === 'email' ? '邮箱' : '手机号'}验证码功能即将支持`);
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div>
        <label className="mb-1 block text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          用户名 <span style={{ color: 'var(--danger-color)' }}>*</span>
        </label>
        <input
          className="input"
          type="text"
          placeholder="请输入用户名"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
          autoFocus
        />
      </div>
      <div>
        <label className="mb-1 block text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          邮箱 <span className="text-xs" style={{ color: 'var(--text-muted)' }}>（可选）</span>
        </label>
        <div className="flex gap-2">
          <input
            className="input flex-1"
            type="email"
            placeholder="请输入邮箱"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            autoComplete="email"
          />
          <button
            type="button"
            className="btn-secondary whitespace-nowrap text-xs"
            onClick={() => handleSendCode('email')}
          >
            发送验证码
          </button>
        </div>
        {email && (
          <input
            className="input mt-2"
            type="text"
            placeholder="请输入邮箱验证码"
            value={emailCode}
            onChange={(e) => setEmailCode(e.target.value)}
          />
        )}
      </div>
      <div>
        <label className="mb-1 block text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          手机号 <span className="text-xs" style={{ color: 'var(--text-muted)' }}>（可选）</span>
        </label>
        <div className="flex gap-2">
          <input
            className="input flex-1"
            type="tel"
            placeholder="请输入手机号"
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            autoComplete="tel"
          />
          <button
            type="button"
            className="btn-secondary whitespace-nowrap text-xs"
            onClick={() => handleSendCode('phone')}
          >
            发送验证码
          </button>
        </div>
        {phone && (
          <input
            className="input mt-2"
            type="text"
            placeholder="请输入手机验证码"
            value={phoneCode}
            onChange={(e) => setPhoneCode(e.target.value)}
          />
        )}
      </div>
      <div>
        <label className="mb-1 block text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          密码 <span style={{ color: 'var(--danger-color)' }}>*</span>
        </label>
        <input
          className="input"
          type="password"
          placeholder="请设置密码（至少6位）"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="new-password"
        />
      </div>
      <div>
        <label className="mb-1 block text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          确认密码 <span style={{ color: 'var(--danger-color)' }}>*</span>
        </label>
        <input
          className="input"
          type="password"
          placeholder="再次输入密码"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          autoComplete="new-password"
        />
      </div>
      {error && (
        <div className="rounded-lg p-3 text-sm" style={{ backgroundColor: 'rgba(220,38,38,0.1)', color: 'var(--danger-color)' }}>
          {error}
        </div>
      )}
      <button
        type="submit"
        className="btn-primary flex w-full items-center justify-center gap-2"
        disabled={loading || !username.trim() || !password || !confirmPassword}
      >
        {loading && <Spinner />}
        {loading ? '注册中...' : '注册'}
      </button>
    </form>
  );
}

// ===== 第三方登录按钮 =====

function OAuthButton({ label, icon, brandColor }: { label: string; icon: string; brandColor: string }) {
  const handleClick = () => {
    alert(`${label}登录即将支持`);
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      className="flex h-10 w-10 items-center justify-center rounded-full border text-lg transition hover:opacity-80"
      style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-secondary)', color: brandColor }}
      title={label}
    >
      {icon}
    </button>
  );
}

// ===== Spinner =====

function Spinner() {
  return (
    <svg className="h-4 w-4 animate-spin" viewBox="0 0 24 24" fill="none">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );
}
