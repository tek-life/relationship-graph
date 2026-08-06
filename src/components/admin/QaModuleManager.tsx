// QA 指令模块管理：CRUD + 排序调整

import { useCallback, useEffect, useState } from 'react';
import { apiDelete, apiGet, apiPost, apiPut } from '../../services/api';
import type { CreateQaInstructionModuleRequest, QaInstructionModule } from './types';
import { ConfirmDialog, EmptyState, ErrorBanner, LoadingSpinner, StatusBadge } from './shared';

/** 表单状态 */
interface QaFormState {
  name: string;
  description: string;
  systemPrompt: string;
  guidanceText: string;
  sortOrder: number;
  triggerScenario: string;
  isActive: boolean;
}

function emptyForm(): QaFormState {
  return {
    name: '',
    description: '',
    systemPrompt: '',
    guidanceText: '',
    sortOrder: 0,
    triggerScenario: 'new_user',
    isActive: true,
  };
}

function moduleToForm(m: QaInstructionModule): QaFormState {
  return {
    name: m.name,
    description: m.description ?? '',
    systemPrompt: m.systemPrompt,
    guidanceText: m.guidanceText ?? '',
    sortOrder: m.sortOrder,
    triggerScenario: m.triggerScenario,
    isActive: m.isActive,
  };
}

function formToRequest(form: QaFormState): CreateQaInstructionModuleRequest {
  return {
    name: form.name.trim(),
    description: form.description.trim() || undefined,
    systemPrompt: form.systemPrompt,
    guidanceText: form.guidanceText.trim() || undefined,
    sortOrder: form.sortOrder,
    triggerScenario: form.triggerScenario,
    isActive: form.isActive,
  };
}

export default function QaModuleManager() {
  const [modules, setModules] = useState<QaInstructionModule[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  // 编辑/创建
  const [editingId, setEditingId] = useState<string | null>(null);
  const [form, setForm] = useState<QaFormState>(emptyForm());
  const [submitting, setSubmitting] = useState(false);

  // 删除确认
  const [deleteTarget, setDeleteTarget] = useState<QaInstructionModule | null>(null);

  const fetchModules = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const list = await apiGet<QaInstructionModule[]>('/api/admin/qa-modules');
      // 后端已按 sort_order 排序，这里再做一次保险
      list.sort((a, b) => a.sortOrder - b.sortOrder);
      setModules(list);
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchModules();
  }, [fetchModules]);

  const startCreate = () => {
    setEditingId('new');
    setForm(emptyForm());
  };

  const startEdit = (m: QaInstructionModule) => {
    setEditingId(m.id);
    setForm(moduleToForm(m));
  };

  const cancelEdit = () => {
    setEditingId(null);
    setForm(emptyForm());
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.name.trim()) {
      setError('模块名称不能为空');
      return;
    }
    if (!form.systemPrompt.trim()) {
      setError('系统提示词不能为空');
      return;
    }
    setError('');
    setSubmitting(true);
    try {
      const req = formToRequest(form);
      if (editingId === 'new') {
        await apiPost<QaInstructionModule>('/api/admin/qa-modules', req);
      } else if (editingId) {
        await apiPut(`/api/admin/qa-modules/${editingId}`, req);
      }
      cancelEdit();
      await fetchModules();
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setSubmitting(false);
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await apiDelete(`/api/admin/qa-modules/${deleteTarget.id}`);
      setDeleteTarget(null);
      await fetchModules();
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
      setDeleteTarget(null);
    }
  };

  /** 调整排序：交换相邻两个模块的 sort_order */
  const handleMove = async (index: number, direction: 'up' | 'down') => {
    const targetIndex = direction === 'up' ? index - 1 : index + 1;
    if (targetIndex < 0 || targetIndex >= modules.length) return;
    const a = modules[index];
    const b = modules[targetIndex];
    // 交换两者的 sortOrder
    try {
      await apiPut(`/api/admin/qa-modules/${a.id}`, {
        ...formToRequest(moduleToForm(a)),
        sortOrder: b.sortOrder,
      });
      await apiPut(`/api/admin/qa-modules/${b.id}`, {
        ...formToRequest(moduleToForm(b)),
        sortOrder: a.sortOrder,
      });
      await fetchModules();
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
    }
  };

  const update = <K extends keyof QaFormState>(key: K, value: QaFormState[K]) => {
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  if (loading) return <LoadingSpinner text="正在加载 QA 模块…" />;

  return (
    <div className="space-y-4">
      {error && <ErrorBanner message={error} />}

      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold" style={{ color: 'var(--text-primary)' }}>
          QA 指令模块（{modules.length}）
        </h3>
        {editingId === null && (
          <button type="button" className="btn-primary" onClick={startCreate}>
            + 新建模块
          </button>
        )}
      </div>

      {/* 创建/编辑表单 */}
      {editingId !== null && (
        <form
          onSubmit={handleSubmit}
          className="space-y-3 rounded-xl border p-4"
          style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
        >
          <h4 className="font-medium" style={{ color: 'var(--text-primary)' }}>
            {editingId === 'new' ? '新建 QA 模块' : '编辑 QA 模块'}
          </h4>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
                模块名称 *
              </label>
              <input
                className="input"
                value={form.name}
                onChange={(e) => update('name', e.target.value)}
                placeholder="如：英雄之旅复盘"
                required
              />
            </div>
            <div>
              <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
                触发场景
              </label>
              <select
                className="input"
                value={form.triggerScenario}
                onChange={(e) => update('triggerScenario', e.target.value)}
              >
                <option value="new_user">新用户</option>
                <option value="always">始终</option>
                <option value="manual">手动</option>
              </select>
            </div>
          </div>
          <div>
            <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
              描述
            </label>
            <input
              className="input"
              value={form.description}
              onChange={(e) => update('description', e.target.value)}
              placeholder="模块简介"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
              系统提示词 *
            </label>
            <textarea
              className="input min-h-32 font-mono text-xs"
              value={form.systemPrompt}
              onChange={(e) => update('systemPrompt', e.target.value)}
              placeholder="LLM 系统提示词"
              required
            />
          </div>
          <div>
            <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
              引导文本
            </label>
            <textarea
              className="input min-h-20"
              value={form.guidanceText}
              onChange={(e) => update('guidanceText', e.target.value)}
              placeholder="用户可见的引导说明"
            />
          </div>
          <div className="flex items-center gap-4">
            <label className="flex items-center gap-2 text-sm" style={{ color: 'var(--text-secondary)' }}>
              <input
                type="checkbox"
                checked={form.isActive}
                onChange={(e) => update('isActive', e.target.checked)}
                className="h-4 w-4"
              />
              启用
            </label>
            <div>
              <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
                排序
              </label>
              <input
                type="number"
                className="input w-20"
                value={form.sortOrder}
                onChange={(e) => update('sortOrder', Number(e.target.value))}
              />
            </div>
          </div>
          <div className="flex gap-2">
            <button type="submit" className="btn-primary" disabled={submitting}>
              {submitting ? '保存中…' : '保存'}
            </button>
            <button type="button" className="btn-secondary" onClick={cancelEdit}>
              取消
            </button>
          </div>
        </form>
      )}

      {/* 模块列表表格 */}
      {modules.length === 0 ? (
        <EmptyState text="暂无 QA 模块，点击「新建模块」创建。" />
      ) : (
        <div className="overflow-hidden rounded-xl border" style={{ borderColor: 'var(--border-color)' }}>
          <table className="w-full text-sm">
            <thead>
              <tr style={{ backgroundColor: 'var(--bg-secondary)', color: 'var(--text-secondary)' }}>
                <th className="px-3 py-2 text-left font-medium">排序</th>
                <th className="px-3 py-2 text-left font-medium">名称</th>
                <th className="px-3 py-2 text-left font-medium">描述</th>
                <th className="px-3 py-2 text-left font-medium">触发</th>
                <th className="px-3 py-2 text-left font-medium">状态</th>
                <th className="px-3 py-2 text-left font-medium">操作</th>
              </tr>
            </thead>
            <tbody>
              {modules.map((m, index) => (
                <tr key={m.id} style={{ borderTop: '1px solid var(--border-color)' }}>
                  <td className="px-3 py-2">
                    <div className="flex items-center gap-1">
                      <span className="text-xs" style={{ color: 'var(--text-muted)' }}>
                        #{m.sortOrder}
                      </span>
                      <button
                        type="button"
                        className="rounded px-1 text-xs transition hover:bg-opacity-10"
                        style={{ color: 'var(--text-muted)' }}
                        disabled={index === 0}
                        onClick={() => handleMove(index, 'up')}
                        title="上移"
                      >
                        ▲
                      </button>
                      <button
                        type="button"
                        className="rounded px-1 text-xs transition"
                        style={{ color: 'var(--text-muted)' }}
                        disabled={index === modules.length - 1}
                        onClick={() => handleMove(index, 'down')}
                        title="下移"
                      >
                        ▼
                      </button>
                    </div>
                  </td>
                  <td className="px-3 py-2 font-medium" style={{ color: 'var(--text-primary)' }}>
                    {m.name}
                  </td>
                  <td className="px-3 py-2 max-w-xs truncate" style={{ color: 'var(--text-secondary)' }}>
                    {m.description || '—'}
                  </td>
                  <td className="px-3 py-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
                    {m.triggerScenario}
                  </td>
                  <td className="px-3 py-2">
                    <StatusBadge active={m.isActive} />
                  </td>
                  <td className="px-3 py-2">
                    <div className="flex gap-2">
                      <button
                        type="button"
                        className="text-xs transition hover:underline"
                        style={{ color: 'var(--accent-color)' }}
                        onClick={() => startEdit(m)}
                      >
                        编辑
                      </button>
                      <button
                        type="button"
                        className="text-xs transition hover:underline"
                        style={{ color: 'var(--danger-color)' }}
                        onClick={() => setDeleteTarget(m)}
                      >
                        删除
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* 删除确认 */}
      {deleteTarget && (
        <ConfirmDialog
          title="删除 QA 模块"
          message={`确认删除「${deleteTarget.name}」？此操作不可撤销。`}
          danger
          confirmLabel="删除"
          onConfirm={handleDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}
