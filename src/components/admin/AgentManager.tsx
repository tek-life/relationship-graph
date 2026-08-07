// 数字人管理模块：数字人 CRUD + 技能子管理

import { useCallback, useEffect, useState } from 'react';
import { apiDelete, apiGet, apiPost, apiPut } from '../../services/api';
import { clearDigitalAgentsCache } from '../../services/digitalAgents';
import { extractSkillDescription } from '../../services/skillDoc';
import type {
  AgentSkill,
  CreateDigitalAgentRequest,
  DigitalAgent,
} from './types';
import { ROUTE_MODES } from './types';
import { ConfirmDialog, EmptyState, ErrorBanner, LoadingSpinner, StatusBadge } from './shared';
import SkillForm from './SkillForm';

/** 逗号分隔 → 数组 */
function splitList(value: string): string[] {
  return value.split(/[,，]/).map((s) => s.trim()).filter(Boolean);
}

/** 空串 → null */
function emptyToNull(value: string): string | null {
  const v = value.trim();
  return v ? v : null;
}

/** 数字人表单初始值 */
interface AgentFormState {
  displayName: string;
  mention: string;
  aliases: string;
  routeMode: string;
  avatarUrl: string;
  description: string;
  skillDescription: string;
  isActive: boolean;
  sortOrder: number;
}

function emptyAgentForm(): AgentFormState {
  return {
    displayName: '',
    mention: '',
    aliases: '',
    routeMode: 'chat',
    avatarUrl: '',
    description: '',
    skillDescription: '',
    isActive: true,
    sortOrder: 0,
  };
}

function agentToForm(agent: DigitalAgent): AgentFormState {
  return {
    displayName: agent.displayName,
    mention: agent.mention,
    aliases: agent.aliases.join(', '),
    routeMode: agent.routeMode,
    avatarUrl: agent.avatarUrl ?? '',
    description: agent.description ?? '',
    skillDescription: agent.skillDescription ?? '',
    isActive: agent.isActive,
    sortOrder: agent.sortOrder,
  };
}

function formToRequest(form: AgentFormState): CreateDigitalAgentRequest {
  return {
    displayName: form.displayName.trim(),
    mention: form.mention.trim(),
    aliases: splitList(form.aliases),
    routeMode: form.routeMode,
    avatarUrl: emptyToNull(form.avatarUrl),
    description: emptyToNull(form.description),
    skillDescription: emptyToNull(form.skillDescription),
    isActive: form.isActive,
    sortOrder: form.sortOrder,
  };
}

export default function AgentManager() {
  const [agents, setAgents] = useState<DigitalAgent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  // 编辑/创建表单状态
  const [editingId, setEditingId] = useState<string | null>(null); // null=未编辑, 'new'=新建, id=编辑
  const [agentForm, setAgentForm] = useState<AgentFormState>(emptyAgentForm());
  const [submitting, setSubmitting] = useState(false);

  // 删除确认
  const [deleteTarget, setDeleteTarget] = useState<DigitalAgent | null>(null);

  // 展开的技能面板
  const [expandedAgentId, setExpandedAgentId] = useState<string | null>(null);

  const fetchAgents = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const list = await apiGet<DigitalAgent[]>('/api/admin/digital-agents');
      setAgents(list);
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAgents();
  }, [fetchAgents]);

  const startCreate = () => {
    setEditingId('new');
    setAgentForm(emptyAgentForm());
  };

  const startEdit = (agent: DigitalAgent) => {
    setEditingId(agent.id);
    setAgentForm(agentToForm(agent));
  };

  const cancelEdit = () => {
    setEditingId(null);
    setAgentForm(emptyAgentForm());
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    // 基本验证
    if (!agentForm.displayName.trim()) {
      setError('显示名称不能为空');
      return;
    }
    if (!agentForm.mention.trim()) {
      setError('mention 不能为空');
      return;
    }
    setError('');
    setSubmitting(true);
    try {
      const req = formToRequest(agentForm);
      if (editingId === 'new') {
        await apiPost<DigitalAgent>('/api/admin/digital-agents', req);
      } else if (editingId) {
        await apiPut(`/api/admin/digital-agents/${editingId}`, req);
      }
      cancelEdit();
      await fetchAgents();
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setSubmitting(false);
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await apiDelete(`/api/admin/digital-agents/${deleteTarget.id}`);
      setDeleteTarget(null);
      await fetchAgents();
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
      setDeleteTarget(null);
    }
  };

  const update = <K extends keyof AgentFormState>(key: K, value: AgentFormState[K]) => {
    setAgentForm((prev) => ({ ...prev, [key]: value }));
  };

  if (loading) return <LoadingSpinner text="正在加载数字人列表…" />;

  return (
    <div className="space-y-4">
      {error && <ErrorBanner message={error} />}

      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold" style={{ color: 'var(--text-primary)' }}>
          数字人列表（{agents.length}）
        </h3>
        {editingId === null && (
          <button type="button" className="btn-primary" onClick={startCreate}>
            + 新建数字人
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
            {editingId === 'new' ? '新建数字人' : '编辑数字人'}
          </h4>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
                显示名称 *
              </label>
              <input
                className="input"
                value={agentForm.displayName}
                onChange={(e) => update('displayName', e.target.value)}
                placeholder="如：联系人管家"
                required
              />
            </div>
            <div>
              <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
                Mention *
              </label>
              <input
                className="input"
                value={agentForm.mention}
                onChange={(e) => update('mention', e.target.value)}
                placeholder="如：@联系人管家"
                required
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
                别名（逗号分隔）
              </label>
              <input
                className="input"
                value={agentForm.aliases}
                onChange={(e) => update('aliases', e.target.value)}
                placeholder="@数字管家, @contact-manager"
              />
            </div>
            <div>
              <label className="mb-1 block text-xs" style={{ color: 'var(--text-secondary)' }}>
                路由模式
              </label>
              <select
                className="input"
                value={agentForm.routeMode}
                onChange={(e) => update('routeMode', e.target.value)}
              >
                {ROUTE_MODES.map((mode) => (
                  <option key={mode} value={mode}>
                    {mode === 'relationship' ? '关系图谱' : '通用聊天'}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <input
            className="input"
            value={agentForm.avatarUrl}
            onChange={(e) => update('avatarUrl', e.target.value)}
            placeholder="头像 URL（可选）"
          />
          <textarea
            className="input min-h-16"
            value={agentForm.description}
            onChange={(e) => update('description', e.target.value)}
            placeholder="数字人描述"
          />
          <textarea
            className="input min-h-16"
            value={agentForm.skillDescription}
            onChange={(e) => update('skillDescription', e.target.value)}
            placeholder="技能描述"
          />
          <div className="flex items-center gap-4">
            <label className="flex items-center gap-2 text-sm" style={{ color: 'var(--text-secondary)' }}>
              <input
                type="checkbox"
                checked={agentForm.isActive}
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
                value={agentForm.sortOrder}
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

      {/* 数字人列表表格 */}
      {agents.length === 0 ? (
        <EmptyState text="暂无数字人，点击「新建数字人」创建。" />
      ) : (
        <div className="overflow-hidden rounded-xl border" style={{ borderColor: 'var(--border-color)' }}>
          <table className="w-full text-sm">
            <thead>
              <tr style={{ backgroundColor: 'var(--bg-secondary)', color: 'var(--text-secondary)' }}>
                <th className="px-3 py-2 text-left font-medium">名称</th>
                <th className="px-3 py-2 text-left font-medium">Mention</th>
                <th className="px-3 py-2 text-left font-medium">路由</th>
                <th className="px-3 py-2 text-left font-medium">状态</th>
                <th className="px-3 py-2 text-left font-medium">操作</th>
              </tr>
            </thead>
            <tbody>
              {agents.map((agent) => (
                <AgentRow
                  key={agent.id}
                  agent={agent}
                  expanded={expandedAgentId === agent.id}
                  onToggleExpand={() =>
                    setExpandedAgentId(expandedAgentId === agent.id ? null : agent.id)
                  }
                  onEdit={() => startEdit(agent)}
                  onDelete={() => setDeleteTarget(agent)}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* 删除确认弹窗 */}
      {deleteTarget && (
        <ConfirmDialog
          title="删除数字人"
          message={`确认删除「${deleteTarget.displayName}」？该数字人下的所有技能也会一并删除。`}
          danger
          confirmLabel="删除"
          onConfirm={handleDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}

/** 单行数字人 + 可展开的技能面板 */
function AgentRow({
  agent,
  expanded,
  onToggleExpand,
  onEdit,
  onDelete,
}: {
  agent: DigitalAgent;
  expanded: boolean;
  onToggleExpand: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  return (
    <>
      <tr style={{ borderTop: '1px solid var(--border-color)' }}>
        <td className="px-3 py-2" style={{ color: 'var(--text-primary)' }}>
          <div className="flex items-center gap-2">
            {agent.avatarUrl && (
              <img src={agent.avatarUrl} alt="" className="h-6 w-6 rounded-full object-cover" />
            )}
            <span className="font-medium">{agent.displayName}</span>
          </div>
        </td>
        <td className="px-3 py-2" style={{ color: 'var(--text-secondary)' }}>
          {agent.mention}
        </td>
        <td className="px-3 py-2" style={{ color: 'var(--text-secondary)' }}>
          {agent.routeMode}
        </td>
        <td className="px-3 py-2">
          <StatusBadge active={agent.isActive} />
        </td>
        <td className="px-3 py-2">
          <div className="flex gap-2">
            <button
              type="button"
              className="text-xs transition hover:underline"
              style={{ color: 'var(--accent-color)' }}
              onClick={onToggleExpand}
            >
              {expanded ? '收起技能' : '技能'}
            </button>
            <button
              type="button"
              className="text-xs transition hover:underline"
              style={{ color: 'var(--accent-color)' }}
              onClick={onEdit}
            >
              编辑
            </button>
            <button
              type="button"
              className="text-xs transition hover:underline"
              style={{ color: 'var(--danger-color)' }}
              onClick={onDelete}
            >
              删除
            </button>
          </div>
        </td>
      </tr>
      {expanded && (
        <tr>
          <td colSpan={5} className="px-3 py-3" style={{ backgroundColor: 'var(--bg-secondary)' }}>
            <SkillPanel agentId={agent.id} agentName={agent.displayName} />
          </td>
        </tr>
      )}
    </>
  );
}

/** 技能管理子面板 */
function SkillPanel({ agentId, agentName }: { agentId: string; agentName: string }) {
  const [skills, setSkills] = useState<AgentSkill[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  // null=未编辑；{ skill: null }=新建；{ skill }=编辑
  const [editing, setEditing] = useState<{ skill: AgentSkill | null } | null>(null);
  const [deleteSkillTarget, setDeleteSkillTarget] = useState<AgentSkill | null>(null);

  const fetchSkills = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const list = await apiGet<AgentSkill[]>(`/api/admin/digital-agents/${agentId}/skills`);
      setSkills(list);
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setLoading(false);
    }
  }, [agentId]);

  useEffect(() => {
    fetchSkills();
  }, [fetchSkills]);

  const handleSkillDelete = async () => {
    if (!deleteSkillTarget) return;
    try {
      await apiDelete(`/api/admin/agent-skills/${deleteSkillTarget.id}`);
      setDeleteSkillTarget(null);
      clearDigitalAgentsCache();
      await fetchSkills();
    } catch (err) {
      setError(String(err instanceof Error ? err.message : err));
      setDeleteSkillTarget(null);
    }
  };

  if (loading) return <LoadingSpinner text="加载技能中…" />;

  return (
    <div className="space-y-3">
      {error && <ErrorBanner message={error} />}
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
          {agentName} 的技能（{skills.length}）
        </span>
        {!editing && (
          <button
            type="button"
            className="text-xs transition hover:underline"
            style={{ color: 'var(--accent-color)' }}
            onClick={() => setEditing({ skill: null })}
          >
            + 添加技能
          </button>
        )}
      </div>

      {/* 技能表单（Markdown/旧 JSON 形态由 SkillForm 内部探测） */}
      {editing && (
        <SkillForm
          key={editing.skill?.id ?? 'new'}
          agentId={agentId}
          skill={editing.skill}
          onSaved={async () => {
            setEditing(null);
            await fetchSkills();
          }}
          onCancel={() => setEditing(null)}
        />
      )}

      {/* 技能列表 */}
      {skills.length === 0 ? (
        <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
          暂无技能
        </p>
      ) : (
        <div className="space-y-2">
          {skills.map((skill) => {
            const description = extractSkillDescription(skill.skillMarkdown);
            return (
              <div
                key={skill.id}
                className="rounded-lg border px-3 py-2"
                style={{ borderColor: 'var(--border-color)', backgroundColor: 'var(--bg-card)' }}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <span className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
                      {skill.skillName}
                    </span>
                    <span className="text-xs" style={{ color: 'var(--text-muted)' }}>
                      {skill.triggerScenario ?? 'always'}
                    </span>
                    <StatusBadge active={skill.isActive} />
                  </div>
                  <div className="flex gap-2">
                    <button
                      type="button"
                      className="text-xs transition hover:underline"
                      style={{ color: 'var(--accent-color)' }}
                      onClick={() => setEditing({ skill })}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      className="text-xs transition hover:underline"
                      style={{ color: 'var(--danger-color)' }}
                      onClick={() => setDeleteSkillTarget(skill)}
                    >
                      删除
                    </button>
                  </div>
                </div>
                {/* Markdown 形态技能：展示 frontmatter description 摘要；旧 JSON 技能不显示 */}
                {description && (
                  <p className="mt-1 truncate text-xs" style={{ color: 'var(--text-muted)' }} title={description}>
                    {description}
                  </p>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* 删除技能确认 */}
      {deleteSkillTarget && (
        <ConfirmDialog
          title="删除技能"
          message={`确认删除技能「${deleteSkillTarget.skillName}」？`}
          danger
          confirmLabel="删除"
          onConfirm={handleSkillDelete}
          onCancel={() => setDeleteSkillTarget(null)}
        />
      )}
    </div>
  );
}
