// 用户管理模块：用户列表 + 角色切换

import { useCallback, useEffect, useState } from 'react';
import { apiGet, apiPut } from '../../services/api';
import type { AdminUser } from './types';
import { ConfirmDialog, EmptyState, ErrorBanner, LoadingSpinner } from './shared';

/** 角色徽标颜色 */
function RoleBadge({ role }: { role: string }) {
  const isAdmin = role === 'admin';
  return (
    <span
      className="badge"
      style={{
        backgroundColor: isAdmin ? 'var(--accent-light)' : 'var(--surface-hover)',
        color: isAdmin ? 'var(--accent-color)' : 'var(--text-secondary)',
      }}
    >
      {isAdmin ? '管理员' : '普通用户'}
    </span>
  );
}

export default function UserManager() {
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  // 角色切换确认
  const [roleTarget, setRoleTarget] = useState<AdminUser | null>(null);
  const [switching, setSwitching] = useState(false);

  const fetchUsers = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const list = await apiGet<AdminUser[]>('/api/admin/users');
      setUsers(list);
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchUsers();
  }, [fetchUsers]);

  const handleRoleSwitch = async () => {
    if (!roleTarget) return;
    const newRole = roleTarget.role === 'admin' ? 'user' : 'admin';
    setSwitching(true);
    try {
      await apiPut(`/api/admin/users/${roleTarget.id}/role`, { role: newRole });
      setRoleTarget(null);
      await fetchUsers();
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
      setRoleTarget(null);
    } finally {
      setSwitching(false);
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

  if (loading) return <LoadingSpinner text="正在加载用户列表…" />;

  return (
    <div className="space-y-4">
      {error && <ErrorBanner message={error} />}

      <h3 className="text-lg font-semibold" style={{ color: 'var(--text-primary)' }}>
        用户列表（{users.length}）
      </h3>

      {users.length === 0 ? (
        <EmptyState text="暂无用户。" />
      ) : (
        <div className="overflow-hidden rounded-xl border" style={{ borderColor: 'var(--border-color)' }}>
          <table className="w-full text-sm">
            <thead>
              <tr style={{ backgroundColor: 'var(--bg-secondary)', color: 'var(--text-secondary)' }}>
                <th className="px-3 py-2 text-left font-medium">用户名</th>
                <th className="px-3 py-2 text-left font-medium">显示名称</th>
                <th className="px-3 py-2 text-left font-medium">角色</th>
                <th className="px-3 py-2 text-left font-medium">画像完成</th>
                <th className="px-3 py-2 text-left font-medium">创建时间</th>
                <th className="px-3 py-2 text-left font-medium">操作</th>
              </tr>
            </thead>
            <tbody>
              {users.map((user) => (
                <tr key={user.id} style={{ borderTop: '1px solid var(--border-color)' }}>
                  <td className="px-3 py-2 font-medium" style={{ color: 'var(--text-primary)' }}>
                    {user.username}
                  </td>
                  <td className="px-3 py-2" style={{ color: 'var(--text-secondary)' }}>
                    {user.displayName || '—'}
                  </td>
                  <td className="px-3 py-2">
                    <RoleBadge role={user.role} />
                  </td>
                  <td className="px-3 py-2">
                    <span
                      className="text-xs"
                      style={{
                        color: user.profileCompleted
                          ? 'var(--accent-color)'
                          : 'var(--text-muted)',
                      }}
                    >
                      {user.profileCompleted ? '已完成' : '未完成'}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
                    {formatDate(user.createdAt)}
                  </td>
                  <td className="px-3 py-2">
                    <button
                      type="button"
                      className="text-xs transition hover:underline"
                      style={{ color: 'var(--accent-color)' }}
                      onClick={() => setRoleTarget(user)}
                    >
                      {user.role === 'admin' ? '降为普通用户' : '升为管理员'}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* 角色切换确认 */}
      {roleTarget && (
        <ConfirmDialog
          title="切换用户角色"
          message={`确认将「${roleTarget.username}」的角色从${
            roleTarget.role === 'admin' ? '管理员' : '普通用户'
          }切换为${roleTarget.role === 'admin' ? '普通用户' : '管理员'}？`}
          confirmLabel={switching ? '切换中…' : '确认切换'}
          onConfirm={handleRoleSwitch}
          onCancel={() => setRoleTarget(null)}
        />
      )}
    </div>
  );
}
