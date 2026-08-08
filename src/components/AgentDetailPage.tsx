import { useState, useEffect, useCallback } from 'react';
import { apiGet, apiPut, apiDelete } from '../services/api';
import { fetchDigitalAgentById, clearDigitalAgentsCache, type DigitalAgent } from '../services/digitalAgents';
import { extractSkillDescription } from '../services/skillDoc';
import SkillForm from './admin/SkillForm';
import { ConfirmDialog } from './ui';
import type { AgentSkill } from './admin/types';

// ============================================================
// 类型定义（对应后端 server/src/types.rs 的 camelCase 序列化）
// ============================================================

/** 后端数字人 DTO（admin 列表接口返回） */
interface DigitalAgentDto {
  id: string;
  displayName: string;
  mention: string;
  aliases: string[];
  routeMode: string;
  avatarUrl: string | null;
  description: string | null;
  skillDescription: string | null;
  isActive: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

/** 更新数字人请求体（对应后端 CreateDigitalAgentRequest） */
interface SaveAgentRequest {
  displayName: string;
  mention: string;
  aliases: string[];
  routeMode: string;
  avatarUrl: string | null;
  description: string | null;
  skillDescription: string | null;
  isActive: boolean;
  sortOrder: number;
}

// ============================================================
// 辅助函数
// ============================================================

/** 将后端 routeMode 归一化为前端合法值 */
function normalizeRouteMode(value: string): 'relationship' | 'chat' {
  return value === 'relationship' ? 'relationship' : 'chat';
}

/** 将后端 DTO 映射为前端 DigitalAgent */
function mapDtoToAgent(dto: DigitalAgentDto): DigitalAgent {
  return {
    id: dto.id,
    displayName: dto.displayName,
    mention: dto.mention,
    aliases: dto.aliases ?? [],
    routeMode: normalizeRouteMode(dto.routeMode),
    avatar: dto.avatarUrl || '',
    description: dto.description ?? undefined,
    skillDescription: dto.skillDescription ?? undefined,
    isActive: dto.isActive,
    sortOrder: dto.sortOrder,
  };
}

/** 路由模式可选值 */
const ROUTE_MODE_OPTIONS: { value: 'relationship' | 'chat'; label: string; desc: string }[] = [
  { value: 'relationship', label: '关系域', desc: '联系人查询、新增、更新、路径规划' },
  { value: 'chat', label: '通用聊天', desc: '开放式对话与问答' },
];

/** 技能触发场景可选值 */
const TRIGGER_SCENARIO_OPTIONS: { value: string; label: string }[] = [
  { value: 'always', label: '始终可用' },
  { value: 'manual', label: '手动触发' },
  { value: 'on_demand', label: '按需触发' },
  { value: 'new_user', label: '新用户引导' },
];

// ============================================================
// 组件
// ============================================================

interface AgentDetailPageProps {
  agentId: string;
  isAdmin: boolean;
  onBack: () => void;
}

export default function AgentDetailPage({ agentId, isAdmin, onBack }: AgentDetailPageProps) {
  const [agent, setAgent] = useState<DigitalAgent | null>(null);
  const [skills, setSkills] = useState<AgentSkill[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [editMode, setEditMode] = useState(false);

  // 数字人编辑表单状态
  const [agentForm, setAgentForm] = useState({
    displayName: '',
    mention: '',
    aliasesText: '',
    routeMode: 'chat' as string,
    description: '',
    skillDescription: '',
    avatarUrl: '',
    isActive: true,
  });
  const [savingAgent, setSavingAgent] = useState(false);
  const [agentFormError, setAgentFormError] = useState('');

  // 技能编辑状态：null=列表模式；{ skill: null }=新增；{ skill }=编辑
  const [skillEdit, setSkillEdit] = useState<{ skill: AgentSkill | null } | null>(null);

  /** 待删除技能（非空时渲染全局 ConfirmDialog，替代 window.confirm） */
  const [pendingDeleteSkill, setPendingDeleteSkill] = useState<{ id: string; name: string } | null>(null);

  // ============================================================
  // 数据加载
  // ============================================================

  const loadAgent = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      let found: DigitalAgent | undefined;

      if (isAdmin) {
        // 管理员从 admin API 获取（含未激活的数字人）
        try {
          const allAgents = await apiGet<DigitalAgentDto[]>('/api/admin/digital-agents');
          const dto = allAgents.find((a) => a.id === agentId);
          if (dto) {
            found = mapDtoToAgent(dto);
          }
        } catch {
          // admin API 不可用时回退到公开接口
          found = await fetchDigitalAgentById(agentId);
        }
      } else {
        found = await fetchDigitalAgentById(agentId);
      }

      if (!found) {
        setError('未找到该数字人');
        return;
      }

      setAgent(found);
      // 初始化编辑表单
      setAgentForm({
        displayName: found.displayName,
        mention: found.mention,
        aliasesText: found.aliases.join('、'),
        routeMode: found.routeMode,
        description: found.description ?? '',
        skillDescription: found.skillDescription ?? '',
        avatarUrl: found.avatar,
        isActive: found.isActive,
      });
    } catch (err) {
      setError(`加载数字人信息失败：${String(err)}`);
    } finally {
      setLoading(false);
    }
  }, [agentId, isAdmin]);

  const loadSkills = useCallback(async () => {
    if (!isAdmin) return;
    try {
      const data = await apiGet<AgentSkill[]>(`/api/admin/digital-agents/${agentId}/skills`);
      setSkills(data);
    } catch (err) {
      console.error('加载技能列表失败', err);
    }
  }, [agentId, isAdmin]);

  useEffect(() => {
    loadAgent();
    loadSkills();
  }, [loadAgent, loadSkills]);

  // ============================================================
  // 数字人保存
  // ============================================================

  const handleSaveAgent = async () => {
    if (!agent) return;

    // 基本输入验证
    const displayName = agentForm.displayName.trim();
    const mention = agentForm.mention.trim();
    if (!displayName) {
      setAgentFormError('名称不能为空');
      return;
    }
    if (!mention) {
      setAgentFormError('Mention 不能为空');
      return;
    }
    if (!mention.startsWith('@')) {
      setAgentFormError('Mention 必须以 @ 开头');
      return;
    }

    setAgentFormError('');
    setSavingAgent(true);

    try {
      const body: SaveAgentRequest = {
        displayName,
        mention,
        aliases: agentForm.aliasesText
          .split(/[、,，\s]+/)
          .map((s) => s.trim())
          .filter(Boolean),
        routeMode: agentForm.routeMode,
        avatarUrl: agentForm.avatarUrl.trim() || null,
        description: agentForm.description.trim() || null,
        skillDescription: agentForm.skillDescription.trim() || null,
        isActive: agentForm.isActive,
        sortOrder: agent.sortOrder,
      };

      await apiPut(`/api/admin/digital-agents/${agentId}`, body);
      // 清除数字人缓存以使下次获取到最新数据
      clearDigitalAgentsCache();
      // 重新加载数据
      await loadAgent();
      setEditMode(false);
    } catch (err) {
      setAgentFormError(`保存失败：${String(err)}`);
    } finally {
      setSavingAgent(false);
    }
  };

  // ============================================================
  // 技能删除（新增/编辑表单由共享组件 SkillForm 承担）
  // ============================================================

  const handleDeleteSkill = (skill: AgentSkill) => {
    setPendingDeleteSkill({ id: skill.id, name: skill.skillName });
  };

  const confirmDeleteSkill = async () => {
    if (!pendingDeleteSkill) return;
    const { id } = pendingDeleteSkill;
    setPendingDeleteSkill(null);
    try {
      await apiDelete(`/api/admin/agent-skills/${id}`);
      clearDigitalAgentsCache();
      await loadSkills();
    } catch (err) {
      console.error('删除技能失败', err);
    }
  };

  // ============================================================
  // 渲染辅助
  // ============================================================

  /** 头像展示：有 SVG/URL 用图片，否则用首字母圆形 */
  const renderAvatar = (size: 'large' | 'medium') => {
    const sizeClass = size === 'large' ? 'w-24 h-24' : 'w-12 h-12';
    const textClass = size === 'large' ? 'text-4xl' : 'text-xl';

    if (agent?.avatar) {
      return (
        <img
          src={agent.avatar}
          className={`${sizeClass} rounded-2xl object-cover`}
          alt={agent.displayName}
        />
      );
    }
    return (
      <div
        className={`${sizeClass} rounded-2xl flex items-center justify-center font-bold ${textClass}`}
        style={{
          background: 'linear-gradient(135deg, var(--accent-color), var(--accent-hover))',
          color: '#fff',
        }}
      >
        {agent?.displayName?.charAt(0) ?? '?'}
      </div>
    );
  };

  // ============================================================
  // 渲染
  // ============================================================

  if (loading) {
    return (
      <div className="mx-auto max-w-3xl p-6">
        <div className="flex items-center justify-center py-20">
          <span className="text-sm" style={{ color: 'var(--text-secondary)' }}>
            加载中...
          </span>
        </div>
      </div>
    );
  }

  if (error || !agent) {
    return (
      <div className="mx-auto max-w-3xl p-6">
        <button
          onClick={onBack}
          className="mb-4 text-sm transition hover:opacity-70"
          style={{ color: 'var(--accent-color)' }}
        >
          ← 返回
        </button>
        <div
          className="rounded-2xl border p-8 text-center"
          style={{
            backgroundColor: 'var(--bg-card)',
            borderColor: 'var(--border-color)',
            color: 'var(--text-secondary)',
          }}
        >
          {error || '未找到该数字人'}
        </div>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-4xl p-6">
      {/* 返回按钮 */}
      <button
        onClick={onBack}
        className="mb-4 text-sm transition hover:opacity-70"
        style={{ color: 'var(--accent-color)' }}
      >
        ← 返回
      </button>

      {/* ===== 数字人信息卡片 ===== */}
      <div
        className="rounded-2xl border p-6 shadow-sm"
        style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
      >
        {/* 头像 + 名称 + Mention */}
        <div className="flex items-center gap-5">
          {renderAvatar('large')}
          <div className="flex-1">
            <div className="flex items-center gap-3">
              <h2 className="text-2xl font-bold" style={{ color: 'var(--text-primary)' }}>
                {agent.displayName}
              </h2>
              {agent.isActive ? (
                <span
                  className="badge"
                  style={{
                    backgroundColor: 'var(--accent-light)',
                    color: 'var(--accent-color)',
                  }}
                >
                  已启用
                </span>
              ) : (
                <span
                  className="badge"
                  style={{
                    backgroundColor: 'var(--surface-hover)',
                    color: 'var(--text-muted)',
                  }}
                >
                  已停用
                </span>
              )}
            </div>
            <p className="mt-1 text-sm" style={{ color: 'var(--text-secondary)' }}>
              {agent.mention}
            </p>
            <div className="mt-2 flex items-center gap-2">
              <span
                className="rounded px-2 py-0.5 text-xs"
                style={{
                  backgroundColor: 'var(--surface-hover)',
                  color: 'var(--text-secondary)',
                }}
              >
                路由：{ROUTE_MODE_OPTIONS.find((o) => o.value === agent.routeMode)?.label ?? agent.routeMode}
              </span>
            </div>
          </div>
        </div>

        {/* 功能介绍 */}
        {agent.description && (
          <div className="mt-6">
            <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>
              功能介绍
            </h3>
            <p className="mt-2 text-sm leading-relaxed" style={{ color: 'var(--text-secondary)' }}>
              {agent.description}
            </p>
          </div>
        )}

        {/* 技能范围 */}
        {agent.skillDescription && (
          <div className="mt-4">
            <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>
              技能范围
            </h3>
            <p className="mt-2 text-sm leading-relaxed" style={{ color: 'var(--text-secondary)' }}>
              {agent.skillDescription}
            </p>
          </div>
        )}

        {/* 别名列表 */}
        {agent.aliases.length > 0 && (
          <div className="mt-4">
            <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>
              别名
            </h3>
            <div className="mt-2 flex flex-wrap gap-2">
              {agent.aliases.map((alias, idx) => (
                <span
                  key={idx}
                  className="badge"
                  style={{
                    backgroundColor: 'var(--surface-hover)',
                    color: 'var(--text-secondary)',
                  }}
                >
                  {alias}
                </span>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* ===== 管理员配置区域 ===== */}
      {isAdmin && (
        <>
          {/* 编辑按钮 / 取消按钮 */}
          <div className="mt-6 flex items-center justify-between">
            <h3 className="text-lg font-semibold" style={{ color: 'var(--text-primary)' }}>
              管理员配置
            </h3>
            {!editMode ? (
              <button
                onClick={() => setEditMode(true)}
                className="btn-secondary text-sm"
              >
                编辑配置
              </button>
            ) : (
              <button
                onClick={() => {
                  setEditMode(false);
                  setAgentFormError('');
                  // 恢复表单为当前值
                  setAgentForm({
                    displayName: agent.displayName,
                    mention: agent.mention,
                    aliasesText: agent.aliases.join('、'),
                    routeMode: agent.routeMode,
                    description: agent.description ?? '',
                    skillDescription: agent.skillDescription ?? '',
                    avatarUrl: agent.avatar,
                    isActive: agent.isActive,
                  });
                }}
                className="btn-secondary text-sm"
              >
                取消编辑
              </button>
            )}
          </div>

          {/* 编辑表单 */}
          {editMode && (
            <div
              className="mt-3 rounded-2xl border p-6 shadow-sm"
              style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
            >
              <div className="space-y-4">
                {/* 名称 + Mention */}
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <div>
                    <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                      名称 *
                    </label>
                    <input
                      className="input"
                      placeholder="如：联系人管家"
                      value={agentForm.displayName}
                      onChange={(e) => setAgentForm({ ...agentForm, displayName: e.target.value })}
                    />
                  </div>
                  <div>
                    <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                      Mention *
                    </label>
                    <input
                      className="input"
                      placeholder="如：@联系人管家"
                      value={agentForm.mention}
                      onChange={(e) => setAgentForm({ ...agentForm, mention: e.target.value })}
                    />
                  </div>
                </div>

                {/* 别名 */}
                <div>
                  <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                    别名（用顿号、逗号或空格分隔）
                  </label>
                  <input
                    className="input"
                    placeholder="如：@数字管家、@contact-manager"
                    value={agentForm.aliasesText}
                    onChange={(e) => setAgentForm({ ...agentForm, aliasesText: e.target.value })}
                  />
                </div>

                {/* 路由模式 */}
                <div>
                  <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                    路由模式
                  </label>
                  <div className="grid grid-cols-2 gap-3">
                    {ROUTE_MODE_OPTIONS.map((opt) => (
                      <button
                        key={opt.value}
                        type="button"
                        className="rounded-lg border p-3 text-left text-sm transition"
                        style={{
                          borderColor:
                            agentForm.routeMode === opt.value
                              ? 'var(--accent-color)'
                              : 'var(--border-color)',
                          backgroundColor:
                            agentForm.routeMode === opt.value
                              ? 'var(--accent-light)'
                              : 'var(--bg-card)',
                          color: 'var(--text-primary)',
                        }}
                        onClick={() => setAgentForm({ ...agentForm, routeMode: opt.value })}
                      >
                        <span className="font-medium">{opt.label}</span>
                        <p className="mt-1 text-xs" style={{ color: 'var(--text-secondary)' }}>
                          {opt.desc}
                        </p>
                      </button>
                    ))}
                  </div>
                </div>

                {/* 头像 URL */}
                <div>
                  <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                    头像 URL（可选）
                  </label>
                  <input
                    className="input"
                    placeholder="https://example.com/avatar.png"
                    value={agentForm.avatarUrl}
                    onChange={(e) => setAgentForm({ ...agentForm, avatarUrl: e.target.value })}
                  />
                </div>

                {/* 功能描述 */}
                <div>
                  <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                    功能描述
                  </label>
                  <textarea
                    className="input min-h-20"
                    placeholder="描述数字人的主要功能..."
                    value={agentForm.description}
                    onChange={(e) => setAgentForm({ ...agentForm, description: e.target.value })}
                  />
                </div>

                {/* 技能描述 */}
                <div>
                  <label className="mb-1 block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                    技能描述
                  </label>
                  <textarea
                    className="input min-h-20"
                    placeholder="描述数字人的技能范围..."
                    value={agentForm.skillDescription}
                    onChange={(e) => setAgentForm({ ...agentForm, skillDescription: e.target.value })}
                  />
                </div>

                {/* 启用/禁用开关 */}
                <div className="flex items-center gap-3">
                  <label className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
                    启用此数字人
                  </label>
                  <button
                    type="button"
                    className="relative inline-flex h-6 w-11 items-center rounded-full transition"
                    style={{
                      backgroundColor: agentForm.isActive
                        ? 'var(--accent-color)'
                        : 'var(--border-color)',
                    }}
                    onClick={() => setAgentForm({ ...agentForm, isActive: !agentForm.isActive })}
                  >
                    <span
                      className={`inline-block h-4 w-4 transform rounded-full bg-card transition ${
                        agentForm.isActive ? 'translate-x-6' : 'translate-x-1'
                      }`}
                    />
                  </button>
                </div>

                {/* 错误提示 */}
                {agentFormError && (
                  <div
                    className="rounded-lg border p-3 text-sm"
                    style={{
                      borderColor: 'var(--danger-color)',
                      color: 'var(--danger-color)',
                      backgroundColor: 'var(--bg-secondary)',
                    }}
                  >
                    {agentFormError}
                  </div>
                )}

                {/* 保存按钮 */}
                <div className="flex justify-end">
                  <button
                    onClick={handleSaveAgent}
                    disabled={savingAgent}
                    className="btn-primary"
                  >
                    {savingAgent ? '保存中...' : '保存配置'}
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* ===== 技能配置列表 ===== */}
          <div
            className="mt-6 rounded-2xl border p-6 shadow-sm"
            style={{ backgroundColor: 'var(--bg-card)', borderColor: 'var(--border-color)' }}
          >
            <div className="flex items-center justify-between">
              <h3 className="font-semibold" style={{ color: 'var(--text-primary)' }}>
                技能配置
              </h3>
              {!skillEdit && (
                <button onClick={() => setSkillEdit({ skill: null })} className="btn-primary text-sm">
                  + 新增技能
                </button>
              )}
            </div>

            {/* 技能编辑/新增表单（共享组件，自动探测 Markdown/旧 JSON 形态） */}
            {skillEdit && (
              <div className="mt-4">
                <SkillForm
                  key={skillEdit.skill?.id ?? 'new'}
                  agentId={agentId}
                  skill={skillEdit.skill}
                  onSaved={async () => {
                    setSkillEdit(null);
                    await loadSkills();
                  }}
                  onCancel={() => setSkillEdit(null)}
                />
              </div>
            )}

            {/* 技能列表 */}
            {!skillEdit && (
              <div className="mt-4 space-y-2">
                {skills.length === 0 ? (
                  <p className="py-4 text-center text-sm" style={{ color: 'var(--text-muted)' }}>
                    暂无技能配置，点击"新增技能"添加
                  </p>
                ) : (
                  skills.map((skill) => {
                    const description = extractSkillDescription(skill.skillMarkdown);
                    return (
                    <div
                      key={skill.id}
                      className="rounded-xl border p-3"
                      style={{
                        borderColor: 'var(--border-color)',
                        backgroundColor: 'var(--bg-secondary)',
                      }}
                    >
                      <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-sm" style={{ color: 'var(--text-primary)' }}>
                            {skill.skillName}
                          </span>
                          {!skill.isActive && (
                            <span
                              className="badge"
                              style={{
                                backgroundColor: 'var(--surface-hover)',
                                color: 'var(--text-muted)',
                              }}
                            >
                              已停用
                            </span>
                          )}
                        </div>
                        <p className="mt-1 text-xs" style={{ color: 'var(--text-secondary)' }}>
                          触发场景：{TRIGGER_SCENARIO_OPTIONS.find((o) => o.value === skill.triggerScenario)?.label ?? skill.triggerScenario ?? '未设置'}
                        </p>
                        {/* Markdown 形态技能：展示 frontmatter description 摘要；旧 JSON 技能不显示 */}
                        {description && (
                          <p className="mt-1 truncate text-xs" style={{ color: 'var(--text-muted)' }} title={description}>
                            {description}
                          </p>
                        )}
                      </div>
                      <div className="flex gap-2">
                        <button
                          onClick={() => setSkillEdit({ skill })}
                          className="rounded px-2 py-1 text-xs transition hover:opacity-70"
                          style={{
                            color: 'var(--accent-color)',
                          }}
                        >
                          编辑
                        </button>
                        <button
                          onClick={() => handleDeleteSkill(skill)}
                          className="rounded px-2 py-1 text-xs transition hover:opacity-70"
                          style={{
                            color: 'var(--danger-color)',
                          }}
                        >
                          删除
                        </button>
                      </div>
                      </div>
                    </div>
                    );
                  })
                )}
              </div>
            )}
          </div>
        </>
      )}

      {/* 删除技能确认弹窗（替代 window.confirm，danger 红键） */}
      <ConfirmDialog
        open={pendingDeleteSkill !== null}
        danger
        title="删除技能"
        message={`确定要删除技能「${pendingDeleteSkill?.name ?? ''}」吗？删除后不可恢复。`}
        confirmLabel="删除"
        onConfirm={() => void confirmDeleteSkill()}
        onCancel={() => setPendingDeleteSkill(null)}
      />
    </div>
  );
}
