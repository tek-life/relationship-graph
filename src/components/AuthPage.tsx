import { useState, type ReactNode } from 'react';
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
          <OAuthButton label="微信" icon={<WechatIcon />} brandColor="#09B83E" />
          <OAuthButton label="钉钉" icon={<DingtalkIcon />} brandColor="#0089FF" />
          <OAuthButton label="飞书" icon={<FeishuIcon />} brandColor="#165DFF" />
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

function OAuthButton({ label, icon, brandColor }: { label: string; icon: ReactNode; brandColor: string }) {
  const [hovered, setHovered] = useState(false);

  const handleClick = () => {
    alert(`${label}登录即将支持`);
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="flex h-10 w-10 items-center justify-center rounded-full border transition hover:opacity-80"
      style={{
        borderColor: hovered ? brandColor : 'var(--border-color)',
        backgroundColor: 'var(--bg-secondary)',
      }}
      title={label}
    >
      {icon}
    </button>
  );
}

// ===== 品牌图标 =====

function WechatIcon() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path
        d="M9.5 4C5.36 4 2 6.69 2 10c0 1.89 1.08 3.56 2.78 4.66l-.7 2.1 2.46-1.23c.82.23 1.69.37 2.6.4-.16-.55-.25-1.13-.25-1.73C8.89 10.47 12.47 8 16.5 8c.34 0 .67.02 1 .05C16.77 5.57 13.41 4 9.5 4zm-3 4.5a.75.75 0 110-1.5.75.75 0 010 1.5zm5 0a.75.75 0 110-1.5.75.75 0 010 1.5z"
        fill="#09B83E"
      />
      <path
        d="M22 14.2c0-2.76-2.69-5-6-5s-6 2.24-6 5 2.69 5 6 5c.73 0 1.43-.11 2.08-.3l1.92.96-.55-1.65C21.08 17.2 22 15.78 22 14.2zm-8.5-1a.65.65 0 110-1.3.65.65 0 010 1.3zm5 0a.65.65 0 110-1.3.65.65 0 010 1.3z"
        fill="#09B83E"
      />
    </svg>
  );
}

function DingtalkIcon() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2z" fill="#0089FF" />
      <path
        d="M17.5 9.5l-2.7 1.2c.3-.5.5-1.1.5-1.7 0-1.7-1.3-3-3-3s-3 1.3-3 3c0 .6.2 1.2.5 1.7L7 9.5c-.3.1-.4.5-.2.7l3.2 4.3c.2.2.6.2.8 0l3.2-4.3c.2-.2.1-.6-.2-.7h-.3z"
        fill="white"
      />
    </svg>
  );
}

function FeishuIcon() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect width="24" height="24" rx="5" fill="#165DFF" />
      <path d="M7 7l5.5 4.5L7 17h2.5l4-3.5 4 3.5V7h-2.5l-3 3.5L9 7H7z" fill="white" />
    </svg>
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
