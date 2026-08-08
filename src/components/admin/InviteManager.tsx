// 邀请管理模块：生成邀请链接 + 邀请列表 + 一键复制

import { useCallback, useEffect, useState } from 'react';
import { apiGet, apiPost } from '../../services/api';
import type { CreateInviteResponse, InviteToken } from './types';
import { AdminPageHeader, EmptyState, ErrorBanner, LoadingSpinner } from './shared';

/** 邀请状态判定 */
function getInviteStatus(invite: InviteToken): { label: string; color: string } {
  const now = new Date();
  const expires = new Date(invite.expiresAt);

  if (invite.usedBy) {
    return { label: '已使用', color: 'var(--text-muted)' };
  }
  if (expires < now) {
    return { label: '已过期', color: 'var(--danger-color)' };
  }
  return { label: '有效', color: 'var(--accent-color)' };
}

export default function InviteManager() {
  const [invites, setInvites] = useState<InviteToken[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [creating, setCreating] = useState(false);

  // 新生成的邀请链接（显示在顶部，方便复制）
  const [newInvite, setNewInvite] = useState<CreateInviteResponse | null>(null);
  const [copied, setCopied] = useState(false);

  const fetchInvites = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const list = await apiGet<InviteToken[]>('/api/admin/invites');
      // 按创建时间倒序
      list.sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime());
      setInvites(list);
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchInvites();
  }, [fetchInvites]);

  const handleCreate = async () => {
    setCreating(true);
    setError('');
    try {
      const result = await apiPost<CreateInviteResponse>('/api/admin/invite');
      setNewInvite(result);
      setCopied(false);
      await fetchInvites();
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setCreating(false);
    }
  };

  /** 构建完整邀请链接（参数名需与 PasswordGate 读取的 invite 一致） */
  const buildInviteUrl = (token: string): string => {
    const base = `${window.location.origin}/?invite=${token}`;
    return base;
  };

  /** 复制到剪贴板 */
  const handleCopy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // 降级方案：使用 execCommand
      const textarea = document.createElement('textarea');
      textarea.value = text;
      document.body.appendChild(textarea);
      textarea.select();
      try {
        document.execCommand('copy');
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      } catch {
        setError('复制失败，请手动复制');
      }
      document.body.removeChild(textarea);
    }
  };

  /** 格式化日期 */
  const formatDate = (iso: string): string => {
    try {
      return new Date(iso).toLocaleString('zh-CN', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
      });
    } catch {
      return iso;
    }
  };

  if (loading) return <LoadingSpinner text="正在加载邀请列表…" />;

  return (
    <div className="space-y-4">
      {error && <ErrorBanner message={error} />}

      {/* 统一页头范式：标题 + 描述 + 主操作区 */}
      <AdminPageHeader
        title="邀请管理"
        description={`生成邀请制注册链接并跟踪使用状态（当前 ${invites.length} 条）`}
        actions={
          <button
            type="button"
            className="btn-primary"
            onClick={handleCreate}
            disabled={creating}
          >
            {creating ? '生成中…' : '+ 生成邀请链接'}
          </button>
        }
      />

      {/* 新生成的邀请链接卡片 */}
      {newInvite && (
        <div
          className="rounded-xl border p-4"
          style={{
            backgroundColor: 'var(--accent-light)',
            borderColor: 'var(--accent-color)',
          }}
        >
          <p className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
            新邀请链接已生成：
          </p>
          <div className="mt-2 flex items-center gap-2">
            <input
              className="input flex-1 font-mono text-xs"
              readOnly
              value={buildInviteUrl(newInvite.token)}
              onClick={(e) => (e.target as HTMLInputElement).select()}
            />
            <button
              type="button"
              className="btn-secondary shrink-0"
              onClick={() => handleCopy(buildInviteUrl(newInvite.token))}
            >
              {copied ? '已复制' : '复制'}
            </button>
          </div>
          <p className="mt-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
            过期时间：{formatDate(newInvite.expiresAt)}（7 天后）
          </p>
        </div>
      )}

      {/* 邀请列表表格 */}
      {invites.length === 0 ? (
        <EmptyState text="暂无邀请记录，点击「生成邀请链接」创建。" />
      ) : (
        <div className="overflow-hidden rounded-xl border" style={{ borderColor: 'var(--border-color)' }}>
          <table className="w-full text-sm">
            <thead>
              <tr style={{ backgroundColor: 'var(--bg-secondary)', color: 'var(--text-secondary)' }}>
                <th className="px-3 py-2 text-left font-medium">Token</th>
                <th className="px-3 py-2 text-left font-medium">创建者</th>
                <th className="px-3 py-2 text-left font-medium">使用者</th>
                <th className="px-3 py-2 text-left font-medium">过期时间</th>
                <th className="px-3 py-2 text-left font-medium">状态</th>
                <th className="px-3 py-2 text-left font-medium">操作</th>
              </tr>
            </thead>
            <tbody>
              {invites.map((invite) => {
                const status = getInviteStatus(invite);
                return (
                  <tr key={invite.id} style={{ borderTop: '1px solid var(--border-color)' }}>
                    <td className="px-3 py-2">
                      <span className="font-mono text-xs" style={{ color: 'var(--text-secondary)' }}>
                        {invite.token.slice(0, 12)}…
                      </span>
                    </td>
                    <td className="px-3 py-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
                      {invite.createdBy.slice(0, 8)}…
                    </td>
                    <td className="px-3 py-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
                      {invite.usedBy ? `${invite.usedBy.slice(0, 8)}…` : '—'}
                    </td>
                    <td className="px-3 py-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
                      {formatDate(invite.expiresAt)}
                    </td>
                    <td className="px-3 py-2">
                      <span
                        className="badge"
                        style={{
                          backgroundColor: 'var(--surface-hover)',
                          color: status.color,
                        }}
                      >
                        {status.label}
                      </span>
                    </td>
                    <td className="px-3 py-2">
                      {status.label === '有效' && (
                        <button
                          type="button"
                          className="text-xs transition hover:underline"
                          style={{ color: 'var(--accent-color)' }}
                          onClick={() => handleCopy(buildInviteUrl(invite.token))}
                        >
                          复制链接
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
